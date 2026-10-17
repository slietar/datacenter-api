use crate::handlers::{IpmiState, read_host_state};

use ipmi_rs::Ipmi;
use ipmi_rs::rmcp::Rmcp;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Notify, broadcast};

pub type IpmiReading = Result<IpmiState, String>;

/// Polls a host's IPMI state on a fixed interval over a single long-lived
/// connection, broadcasting each reading to subscribers. The polling task
/// starts on the first subscriber and stops once the last one goes away.
#[derive(Debug)]
pub struct IpmiManager {
    address: String,
    username: String,
    password: Vec<u8>,
    interval: Duration,
    sender: broadcast::Sender<Arc<IpmiReading>>,
    running: Mutex<bool>,
    refresh: Notify,
}

impl IpmiManager {
    pub fn new(address: String, username: String, password: Vec<u8>, interval: Duration) -> Self {
        let (sender, _) = broadcast::channel(1);

        Self {
            address,
            username,
            password,
            interval,
            sender,
            running: Mutex::new(false),
            refresh: Notify::new(),
        }
    }

    pub fn subscribe(self: &Arc<Self>) -> broadcast::Receiver<Arc<IpmiReading>> {
        let receiver = self.sender.subscribe();

        let mut running = self.running.lock().unwrap();

        if !*running {
            *running = true;
            let this = Arc::clone(self);
            tokio::spawn(this.poll_loop());
        } else {
            // The poller is already running and possibly sleeping between
            // reads; wake it so the new subscriber gets a fresh reading
            // instead of waiting out the rest of the interval.
            self.refresh.notify_one();
        }

        receiver
    }

    fn connect(&self) -> Result<Ipmi<Rmcp>, String> {
        let ipv6 = match self.address.parse().unwrap() {
            IpAddr::V4(v4) => v4.to_ipv6_mapped(),
            IpAddr::V6(v6) => v6,
        };

        let mut rmcp =
            Rmcp::new((ipv6, 623), Duration::from_secs(1)).map_err(|e| format!("{:?}", e))?;

        rmcp.activate(true, Some(&self.username), Some(&self.password))
            .map_err(|e| format!("{:?}", e))?;

        Ok(Ipmi::new(rmcp))
    }

    async fn poll_loop(self: Arc<Self>) {
        let connect_self = Arc::clone(&self);
        let mut ipmi = match tokio::task::spawn_blocking(move || connect_self.connect())
            .await
            .unwrap()
        {
            Ok(ipmi) => ipmi,
            Err(e) => {
                let _ = self.sender.send(Arc::new(Err(e)));
                *self.running.lock().unwrap() = false;
                return;
            }
        };

        loop {
            let (next_ipmi, reading) = tokio::task::spawn_blocking(move || {
                let reading = read_host_state(&mut ipmi).map_err(|e| format!("{:?}", e));
                (ipmi, reading)
            })
            .await
            .unwrap();
            ipmi = next_ipmi;

            let _ = self.sender.send(Arc::new(reading));

            tokio::select! {
                _ = tokio::time::sleep(self.interval) => {},
                _ = self.refresh.notified() => {},
            }

            // Re-check under the same lock used by `subscribe` so a new
            // subscriber arriving between the send above and this check is
            // never missed.
            let mut running = self.running.lock().unwrap();
            if self.sender.receiver_count() == 0 {
                *running = false;
                return;
            }
        }
    }
}
