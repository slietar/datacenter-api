use crate::client::{ClientResult, HostClientState};
use crate::comin;
use crate::config::Config;
use crate::ipmi::client::IpmiState;
use crate::ipmi::interface::{ChassisControl, GetChassisStatus, PowerRestorePolicy, ipmi_do};

use ipmi_rs::Ipmi;
use ipmi_rs::rmcp::Rmcp;
use ipmi_rs::sensor_event::{GetSensorReading, ThresholdReading};
use ipmi_rs::storage::sdr::Record;
use ipmi_rs::storage::sdr::event_reading_type_code::EventReadingTypeCodes;

use crate::live_state::{self, LiveState, StateSource, Subscription};

use futures::stream::{BoxStream, StreamExt};
use std::net::IpAddr;
use std::time::Duration;

pub type IpmiReading = live_state::Reading<IpmiState, anyhow::Error>;

const POLL_INTERVAL: Duration = Duration::from_secs(30);

/// Follows the state of a host's IPMI interface, see [`LiveState`] for when
/// it is connected to. IPMI has no change notifications, so the state is
/// polled over a single long-lived session.
#[derive(Debug)]
pub struct IpmiManager {
    state: LiveState<IpmiSource>,
}

impl IpmiManager {
    pub fn new(name: String, address: String, username: String, password: Vec<u8>) -> Self {
        let span = tracing::info_span!("ipmi", host_name = %name);

        Self {
            state: LiveState::new(IpmiSource {
                address,
                username,
                password,
            })
            .instrument(span),
        }
    }

    /// Returns a subscription which immediately holds the current reading,
    /// waiting for it to become available if it isn't already.
    pub async fn subscribe(&self) -> Subscription<IpmiState, anyhow::Error> {
        self.state.subscribe().await
    }

    pub async fn get(&self) -> IpmiReading {
        self.state.get().await
    }
}

fn open(address: &str, username: &str, password: &[u8]) -> anyhow::Result<Ipmi<Rmcp>> {
    let ipv6 = match address.parse::<IpAddr>()? {
        IpAddr::V4(v4) => v4.to_ipv6_mapped(),
        IpAddr::V6(v6) => v6,
    };

    let mut rmcp = Rmcp::new((ipv6, 623), Duration::from_secs(1))
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    rmcp.activate(true, Some(username), Some(password))
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;

    Ok(Ipmi::new(rmcp))
}

pub fn read_host_state(ipmi: &mut Ipmi<Rmcp>) -> anyhow::Result<IpmiState> {
    let chassis = ipmi
        .send_recv(GetChassisStatus)
        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
    let sensors: Vec<_> = ipmi.sdrs().collect();

    let extract_sensor = |s: &Record| {
        let common = s.common_data()?;
        if common.event_reading_type_code != EventReadingTypeCodes::Threshold {
            return None;
        }

        let cmd = GetSensorReading::for_sensor_key(&common.key);
        let raw = ipmi
            .send_recv(cmd)
            .map_err(|e| anyhow::anyhow!("{:?}", e))
            .ok()?;
        let reading = ThresholdReading::from(&raw);

        let display = s.full_sensor()?.display_reading(reading.reading?)?;
        Some((s.id()?.to_string(), display))
    };

    let sensor_values = sensors.iter().filter_map(extract_sensor).collect();

    Ok(IpmiState {
        power_is_on: chassis.power_is_on,
        power_restore_policy: match chassis.power_restore_policy {
            PowerRestorePolicy::AlwaysOn => "always-on".to_owned(),
            PowerRestorePolicy::AlwaysOff => "always-off".to_owned(),
            PowerRestorePolicy::Previous => "previous".to_owned(),
        },
        sensors: sensor_values,
    })
}

/// Reads the state on a blocking thread, handing the session back so it can
/// be reused.
async fn read(mut ipmi: Ipmi<Rmcp>) -> anyhow::Result<(IpmiState, Ipmi<Rmcp>)> {
    tokio::task::spawn_blocking(move || {
        let state = read_host_state(&mut ipmi)?;
        Ok((state, ipmi))
    })
    .await?
}

#[derive(Debug)]
struct IpmiSource {
    address: String,
    username: String,
    password: Vec<u8>,
}

impl StateSource for IpmiSource {
    type State = IpmiState;
    type Error = anyhow::Error;

    async fn connect(
        &self,
    ) -> anyhow::Result<(IpmiState, BoxStream<'_, anyhow::Result<IpmiState>>)> {
        // A blocking task can't borrow `self`, so give it its own copy of the
        // (small) connection parameters.
        let (address, username, password) = (
            self.address.clone(),
            self.username.clone(),
            self.password.clone(),
        );

        let ipmi = tokio::task::spawn_blocking(move || open(&address, &username, &password))
            .await??;

        let (state, ipmi) = read(ipmi).await?;

        // An error ends the stream, which makes the session be opened anew.
        let updates = futures::stream::try_unfold(ipmi, |ipmi| async move {
            tokio::time::sleep(POLL_INTERVAL).await;
            let (state, ipmi) = read(ipmi).await?;
            Ok(Some((state, ipmi)))
        })
        .boxed();

        Ok((state, updates))
    }
}
