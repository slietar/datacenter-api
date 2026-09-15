use crate::config::Config;
use crate::ipmi::interface::{ChassisControl, GetChassisStatus, PowerRestorePolicy, ipmi_do};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use futures::FutureExt;
use futures::TryFutureExt;
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use ipmi_rs::sensor_event::GetSensorReading;

#[derive(Debug, Clone, Serialize)]
pub struct ClientError {
    message: String,
    reason: String,
}

#[derive(Debug, Clone, Serialize)]
// #[serde(untagged)]
pub enum ClientResult<T> {
    Ok(T),
    Err(ClientError),
}

impl<T> std::convert::From<anyhow::Result<T>> for ClientResult<T> {
    fn from(result: anyhow::Result<T>) -> Self {
        match result {
            Ok(value) => ClientResult::Ok(value),
            Err(err) => ClientResult::Err(ClientError {
                message: err.to_string(),
                reason: err.root_cause().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct HostClientState {
    comin: Option<ClientResult<crate::comin::client::ClientState>>,
    headscale: Option<ClientResult<Option<crate::headscale::client::NodeState>>>,
    ipmi: Option<IpmiState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpmiState {
    power_is_on: bool,
    power_restore_policy: String,
    sensors: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostCommand {
    power: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct Either<T, U>(#[serde(with = "either::serde_untagged")] either::Either<T, U>)
where
    T: Serialize + for<'a> Deserialize<'a>,
    U: Serialize + for<'a> Deserialize<'a>;

impl<T, U> Either<T, U>
where
    T: Serialize + for<'a> Deserialize<'a>,
    U: Serialize + for<'a> Deserialize<'a>,
{
    fn left(v: T) -> Either<T, U> {
        Either(either::Either::Left(v))
    }
    fn right(v: U) -> Either<T, U> {
        Either(either::Either::Right(v))
    }
}

// #[derive(Debug, Clone, Serialize)]
// pub struct HostList {
//     hosts: HashMap<String, Either<HostClientState, Error>>,
// }

use ipmi_rs::Ipmi;
use ipmi_rs::rmcp::Rmcp;
use ipmi_rs::sensor_event::ThresholdReading;
use ipmi_rs::storage::sdr::Record;
use ipmi_rs::storage::sdr::event_reading_type_code::EventReadingTypeCodes;

pub(crate) fn read_host_state(ipmi: &mut Ipmi<Rmcp>) -> anyhow::Result<IpmiState> {
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

pub async fn host_get_handler(
    Path(name): Path<String>,
    State(state): State<crate::state::State>,
) -> Result<Json<HostClientState>, StatusCode> {
    let host_manager = state
        .host_managers
        .get(&name)
        .ok_or(StatusCode::NOT_FOUND)?;

    let comin_state = host_manager.get_comin_state().await.map(ClientResult::from);

    let headscale_state =
        futures::future::OptionFuture::from(state.headscale_manager.as_ref().map(
            async |manager|  {
                let state = manager.get_state().await?;

                Ok(state
                    .nodes
                    .iter()
                    .find(|node_state| {
                        node_state
                            .ip_addresses
                            .contains(&host_manager.host_config.address)
                    })
                    .map(|node_state| crate::headscale::client::NodeState {
                        node: node_state.into(),
                        user: (&node_state.user).into(),
                    }))
            },
        ))
        .await
        .map(ClientResult::from);

    let ipmi_state = host_manager
        .get_ipmi_state()
        .await
        .transpose()
        .map_err(|e| {
            tracing::error!("Error getting IPMI state for host {}: {:?}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(HostClientState {
        comin: comin_state,
        ipmi: ipmi_state,
        headscale: headscale_state,
    }))
}

// pub async fn ipmi_hosts_handler(State(config): State<Config>) -> Json<HostList> {
//     let hosts = stream::iter(config.host)
//         .map(|(hostname, host)| {
//             ipmi_do(
//                 &host.address,
//                 &config.ipmi.username,
//                 config.ipmi.password.as_ref().unwrap().as_bytes(),
//                 read_host_state,
//             )
//             .map_ok(|ipmi| HostState { ipmi: Some(ipmi) })
//             .map_err(|e| Error {
//                 error: format!("{:?}", e),
//             })
//             .map_ok_or_else(Either::right, Either::left)
//             .map(move |v| (hostname, v))
//         })
//         .buffer_unordered(4)
//         .collect()
//         .await;

//     Json(HostList { hosts })
// }

// pub async fn ipmi_host_put_handler(
//     Path(hostname): Path<String>,
//     State(config): State<Config>,
//     Json(body): Json<HostCommand>,
// ) {
//     let host = &config.host[&hostname];

//     let cmd = match body.power {
//         Some(true) => Some(ChassisControl::PowerUp),
//         Some(false) => Some(ChassisControl::PowerDown),
//         None => None,
//     };

//     if let Some(cmd) = cmd {
//         ipmi_do(
//             &host.address,
//             &config.ipmi.username,
//             config.ipmi.password.as_ref().unwrap().as_bytes(),
//             move |ipmi| ipmi.send_recv(cmd).map_err(|e| anyhow::anyhow!("{:?}", e)),
//         )
//         .await
//         .unwrap();
//     }
// }

// #[derive(Debug, Clone, Serialize)]
// pub struct Whoami {
//     name: String,
//     email: String,
// }

// pub async fn whoami_handler(headers: HeaderMap) -> Result<Json<Whoami>, StatusCode> {
//     let header_str = |name: &str| {
//         headers
//             .get(name)
//             .and_then(|v| v.to_str().ok())
//             .map(str::to_owned)
//     };

//     let name = header_str("Remote-Name");
//     let email = header_str("Remote-Email");

//     match (name, email) {
//         (Some(name), Some(email)) => Ok(Json(Whoami { name, email })),
//         _ => Err(StatusCode::SERVICE_UNAVAILABLE),
//     }
// }
