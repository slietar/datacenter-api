use std::future;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{BoxStream, StreamExt};
use tonic::transport::{Channel, Endpoint};

use crate::live_state::{LiveState, StateSource, Subscription};
use crate::protobuf::State;
use crate::protobuf::comin_client::CominClient;
use crate::protobuf::event::Type as EventType;

pub type CominStateReading = anyhow::Result<State>;

/// Follows the state of a `comin` (https://github.com/nlewo/comin) gRPC
/// endpoint, see [`LiveState`] for when it is connected to.
#[derive(Debug)]
pub struct CominManager {
    state: LiveState<CominSource>,
}

impl CominManager {
    pub fn new(name: String, address: String) -> Self {
        let span = tracing::info_span!("comin", host_name = %name);

        Self {
            state: LiveState::new(CominSource { address }).instrument(span),
        }
    }

    /// Returns a subscription which immediately holds the current state,
    /// waiting for it to become available if it isn't already.
    pub async fn subscribe(&self) -> Subscription<State> {
        self.state.subscribe().await
    }

    pub async fn get(&self) -> Arc<CominStateReading> {
        self.state.get().await
    }
}

#[derive(Debug)]
struct CominSource {
    address: String,
}

impl CominSource {
    async fn client(&self) -> anyhow::Result<CominClient<Channel>> {
        let channel = Endpoint::from_shared(format!("http://{}:50051", self.address))?
            .connect_timeout(Duration::from_secs(5))
            .connect()
            .await?;

        Ok(CominClient::new(channel))
    }
}

impl StateSource for CominSource {
    type State = State;

    async fn connect(&self) -> anyhow::Result<(State, BoxStream<'_, anyhow::Result<State>>)> {
        let mut client = self.client().await?;

        // Listen before fetching so that no update falls in between.
        let events = client.events(()).await?.into_inner();
        let state = client.get_state(()).await?.into_inner();

        let updates = events.filter_map(|event| {
            future::ready(match event {
                Ok(event) => match event.r#type {
                    Some(EventType::ManagerState(manager_state)) => manager_state.state.map(Ok),
                    _ => None,
                },
                Err(status) => Some(Err(status.into())),
            })
        });

        Ok((state, updates.boxed()))
    }
}
