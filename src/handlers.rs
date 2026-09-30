use crate::client::{ClientResult, HostClientState};
use crate::comin;
use crate::config::Config;
use crate::ipmi::interface::{ChassisControl, GetChassisStatus, PowerRestorePolicy, ipmi_do};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::FutureExt;
use futures::TryFutureExt;
use futures::stream::{self, BoxStream, Stream, StreamExt};
use ipmi_rs::sensor_event::GetSensorReading;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    let headscale_state = match &state.headscale_manager {
        Some(manager) => Some(crate::headscale::client::NodeState::from_reading(
            &manager.get().await,
            &host_manager.host_config.address,
        )),
        None => None,
    };

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

enum HostStateUpdate {
    Comin(comin::manager::CominReading),
    Headscale(crate::headscale::manager::HeadscaleReading),
}

pub async fn host_sse_handler(
    Path(name): Path<String>,
    State(state): State<crate::state::State>,
) -> Result<Sse<impl Stream<Item = Result<Event, axum::Error>>>, StatusCode> {
    let host_manager = state
        .host_managers
        .get(&name)
        .ok_or(StatusCode::NOT_FOUND)?;

    let comin_manager = host_manager.comin_manager.clone();
    let address = host_manager.host_config.address.clone();
    let headscale_manager = state.headscale_manager.clone();

    let has_comin = comin_manager.is_some();
    let has_headscale = headscale_manager.is_some();

    let comin_stream = match comin_manager {
        Some(comin_manager) => comin_manager
            .subscribe()
            .await
            .into_stream()
            .map(HostStateUpdate::Comin)
            .boxed(),
        None => stream::empty().boxed(),
    };

    let headscale_stream = match headscale_manager {
        Some(headscale_manager) => headscale_manager
            .subscribe()
            .await
            .into_stream()
            .map(HostStateUpdate::Headscale)
            .boxed(),
        None => stream::empty().boxed(),
    };

    let stream: BoxStream<'static, Result<Event, axum::Error>> = if has_comin || has_headscale {
        stream::select(comin_stream, headscale_stream)
            .scan(
                (None, None),
                move |(comin_state, headscale_state), update| {
                    match update {
                        HostStateUpdate::Comin(reading) => *comin_state = Some(reading),
                        HostStateUpdate::Headscale(reading) => *headscale_state = Some(reading),
                    }

                    let comin = comin_state.clone().map(ClientResult::from);
                    let headscale = headscale_state.as_ref().map(|reading| {
                        crate::headscale::client::NodeState::from_reading(reading, &address)
                    });

                    futures::future::ready(Some(Event::default().json_data(&HostClientState {
                        comin,
                        ipmi: None,
                        headscale,
                    })))
                },
            )
            .boxed()
    } else {
        stream::once(async {
            Event::default().json_data(&HostClientState {
                comin: None,
                ipmi: None,
                headscale: None,
            })
        })
        .boxed()
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
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
