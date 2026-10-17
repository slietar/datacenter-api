use crate::client::{ClientResult, HostClientState};

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::FutureExt;
use futures::stream::{self, BoxStream, Stream, StreamExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    error: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostCommand {
    power: Option<bool>,
}

// #[derive(Debug, Clone, Serialize)]
// pub struct HostList {
//     hosts: HashMap<String, Either<HostClientState, Error>>,
// }

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

    let ipmi_state = host_manager.get_ipmi_state().await.inspect(|reading| {
        if let Some(error) = reading.error() {
            tracing::error!("Error getting IPMI state for host {}: {:?}", name, error);
        }
    });
    let ipmi_state = ipmi_state.map(ClientResult::from);

    Ok(Json(HostClientState {
        comin: comin_state,
        ipmi: ipmi_state,
        headscale: headscale_state,
    }))
}

enum HostStateUpdate {
    Ipmi(crate::ipmi::manager::IpmiReading),
    Comin(crate::comin::manager::CominReading),
    Headscale(crate::headscale::manager::HeadscaleReading),
}

/// Readings after the one held at subscription time.
fn updates<T, E>(
    subscription: Option<crate::live_state::Subscription<T, E>>,
) -> BoxStream<'static, crate::live_state::Reading<T, E>>
where
    T: Send + Sync + 'static,
    E: Send + Sync + 'static,
{
    match subscription {
        Some(sub) => sub.into_stream().skip(1).boxed(),
        None => stream::empty().boxed(),
    }
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
    let ipmi_manager = host_manager.ipmi_manager.clone();
    let address = host_manager.host_config.address.clone();
    let headscale_manager = state.headscale_manager.clone();

    let (mut comin_sub, mut ipmi_sub, mut headscale_sub) = (None, None, None);
    if let Some(manager) = comin_manager {
        comin_sub = Some(manager.subscribe().await);
    }
    if let Some(manager) = ipmi_manager {
        ipmi_sub = Some(manager.subscribe().await);
    }
    if let Some(manager) = headscale_manager {
        headscale_sub = Some(manager.subscribe().await);
    }

    // Subscriptions already hold a reading, so collect them all into a single
    // initial event and only stream the changes that follow.
    let mut comin_state = comin_sub.as_mut().map(|sub| sub.get());
    let mut ipmi_state = ipmi_sub.as_mut().map(|sub| sub.get());
    let mut headscale_state = headscale_sub.as_mut().map(|sub| sub.get());

    let to_event = move |comin: &Option<_>, ipmi: &Option<_>, headscale: &Option<_>| {
        Event::default().json_data(&HostClientState {
            comin: comin.clone().map(ClientResult::from),
            ipmi: ipmi.clone().map(ClientResult::from),
            headscale: headscale.as_ref().map(|reading| {
                crate::headscale::client::NodeState::from_reading(reading, &address)
            }),
        })
    };

    let initial = to_event(&comin_state, &ipmi_state, &headscale_state);

    let comin_stream = updates(comin_sub).map(HostStateUpdate::Comin).boxed();
    let ipmi_stream = updates(ipmi_sub).map(HostStateUpdate::Ipmi).boxed();
    let headscale_stream = updates(headscale_sub).map(HostStateUpdate::Headscale).boxed();

    let changes = stream::select_all([comin_stream, ipmi_stream, headscale_stream]).map(
        move |update| {
            match update {
                HostStateUpdate::Comin(reading) => comin_state = Some(reading),
                HostStateUpdate::Ipmi(reading) => ipmi_state = Some(reading),
                HostStateUpdate::Headscale(reading) => headscale_state = Some(reading),
            }
            to_event(&comin_state, &ipmi_state, &headscale_state)
        },
    );

    let stream = stream::once(async { initial }).chain(changes);

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
