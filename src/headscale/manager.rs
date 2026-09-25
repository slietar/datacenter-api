use std::time::Duration;

use url::Url;

use crate::headscale::node::NodesResponse;
use crate::live_state::{self, Subscription};
use crate::polling_state::{PollingLiveState, Poller};

pub type HeadscaleReading = live_state::Reading<NodesResponse, anyhow::Error>;

const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Polls a headscale (https://headscale.net) API for its node list, see
/// [`PollingLiveState`] for when it is connected to.
#[derive(Debug, Clone)]
pub struct HeadscaleManager {
    state: PollingLiveState<HeadscaleSource>,
}

impl HeadscaleManager {
    pub fn new(url: Url, api_key: String) -> Self {
        Self {
            state: PollingLiveState::new(
                HeadscaleSource {
                    url,
                    api_key,
                    client: reqwest::Client::new(),
                },
                POLL_INTERVAL,
            )
            .instrument(tracing::info_span!("headscale")),
        }
    }

    /// Returns a subscription which immediately holds the current reading,
    /// waiting for it to become available if it isn't already.
    pub async fn subscribe(&self) -> Subscription<NodesResponse, anyhow::Error> {
        self.state.subscribe().await
    }

    pub async fn get(&self) -> HeadscaleReading {
        self.state.get().await
    }
}

#[derive(Debug)]
struct HeadscaleSource {
    url: Url,
    api_key: String,
    client: reqwest::Client,
}

impl Poller for HeadscaleSource {
    type State = NodesResponse;
    type Error = anyhow::Error;

    async fn poll(&self) -> anyhow::Result<NodesResponse> {
        let response = self
            .client
            .get(self.url.join("/api/v1/node")?)
            .bearer_auth(&self.api_key)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }
}
