use std::time::Duration;

use futures::stream::{BoxStream, StreamExt};
use url::Url;

use crate::headscale::node::NodesResponse;
use crate::live_state::{LiveState, Reading, StateSource, Subscription};

pub type HeadscaleReading = Reading<NodesResponse, anyhow::Error>;

const POLL_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct HeadscaleManager {
    state: LiveState<HeadscaleSource>,
}

impl HeadscaleManager {
    pub fn new(url: Url, api_key: String) -> Self {
        Self {
            state: LiveState::new(HeadscaleSource {
                url,
                api_key,
                client: reqwest::Client::new(),
            })
            .instrument(tracing::info_span!("headscale")),
        }
    }

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

impl HeadscaleSource {
    async fn fetch(&self) -> anyhow::Result<NodesResponse> {
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

impl StateSource for HeadscaleSource {
    type State = NodesResponse;
    type Error = anyhow::Error;

    async fn connect(
        &self,
    ) -> anyhow::Result<(NodesResponse, BoxStream<'_, anyhow::Result<NodesResponse>>)> {
        let state = self.fetch().await?;

        let updates = futures::stream::try_unfold((), move |()| async move {
            tokio::time::sleep(POLL_INTERVAL).await;
            Ok(Some((self.fetch().await?, ())))
        })
        .boxed();

        Ok((state, updates))
    }
}
