use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Notify, watch};

pub mod protobuf {
    tonic::include_proto!("protobuf");
}

use protobuf::State;
use protobuf::comin_client::CominClient;
use protobuf::event::Type as EventType;

pub type CominStateUpdate = Result<State, String>;

/// Connects to a `comin` (https://github.com/nlewo/comin) gRPC endpoint,
/// broadcasting the current state and every subsequent update it streams.
/// The connection is established on the first subscriber and torn down
/// once the last one goes away.
#[derive(Debug)]
pub struct CominManager {
    address: String,
    sender: watch::Sender<Option<Arc<CominStateUpdate>>>,
    running: AtomicBool,
    /// Notified when a subscriber drops, so `run` can stop promptly instead
    /// of waiting for the current event stream to end on its own.
    stop: Notify,
}

impl CominManager {
    pub fn new(address: String) -> Self {
        let (sender, _) = watch::channel(None);

        Self {
            address,
            sender,
            running: AtomicBool::new(false),
            stop: Notify::new(),
        }
    }

    pub async fn get_state(&self) -> anyhow::Result<State> {
        let mut client = self.connect().await?;
        let state = client.get_state(()).await?.into_inner();

        Ok(state)
    }

    async fn connect(&self) -> anyhow::Result<CominClient<tonic::transport::Channel>> {
        let client = CominClient::connect(format!("http://{}:50051", self.address)).await?;

        Ok(client)
    }

    /// Returns a receiver which immediately holds the current state, waiting
    /// for it to become available if it isn't already.
    pub async fn subscribe(self: &Arc<Self>) -> CominStateReceiver {
        let mut receiver = self.sender.subscribe();

        if self
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let this = Arc::clone(self);
            tokio::spawn(this.run());
        }

        if receiver.borrow().is_none() {
            // The sender is only ever dropped along with `self`, so this can
            // only fail to observe a value if `run` has not sent one yet.
            let _ = receiver.changed().await;
        }

        CominStateReceiver {
            manager: Arc::clone(self),
            receiver,
        }
    }

    async fn run(self: Arc<Self>) {
        loop {
            if self.sender.receiver_count() == 0 {
                self.running.store(false, Ordering::Release);
                return;
            }

            let result: anyhow::Result<()> = async {
                let mut client = self.connect().await?;

                let state = client.get_state(()).await?.into_inner();
                let _ = self.sender.send(Some(Arc::new(Ok(state))));

                let mut events = client.events(()).await?.into_inner();

                loop {
                    tokio::select! {
                        event = events.message() => {
                            let Some(event) = event? else { break };

                            if let Some(EventType::ManagerState(manager_state)) = event.r#type
                                && let Some(state) = manager_state.state
                            {
                                let _ = self.sender.send(Some(Arc::new(Ok(state))));
                            }
                        }
                        _ = self.stop.notified() => return Ok(()),
                    }
                }

                Ok(())
            }
            .await;

            if let Err(error) = result {
                let _ = self.sender.send(Some(Arc::new(Err(error.to_string()))));
            }

            if self.sender.receiver_count() == 0 {
                self.running.store(false, Ordering::Release);
                return;
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = self.stop.notified() => {},
            }
        }
    }
}

/// A [`watch::Receiver`] guaranteed to hold a real state once obtained from
/// [`CominManager::subscribe`], hiding the transient `None` the underlying
/// channel starts with before the first state arrives. Dropping the last of
/// these wakes up [`CominManager::run`] so it can stop right away.
pub struct CominStateReceiver {
    manager: Arc<CominManager>,
    receiver: watch::Receiver<Option<Arc<CominStateUpdate>>>,
}

impl CominStateReceiver {
    pub fn borrow(&self) -> Arc<CominStateUpdate> {
        self.receiver
            .borrow()
            .clone()
            .expect("subscribe waits for the first state")
    }

    pub async fn changed(&mut self) -> Result<Arc<CominStateUpdate>, watch::error::RecvError> {
        self.receiver.changed().await?;

        Ok(self.borrow())
    }
}

impl Drop for CominStateReceiver {
    fn drop(&mut self) {
        // `self.receiver` is dropped right after this, so a count of 1 here
        // means it will be the last one gone.
        if self.manager.sender.receiver_count() <= 1 {
            self.manager.stop.notify_one();
        }
    }
}
