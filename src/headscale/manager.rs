use url::Url;

use crate::headscale::node::NodesResponse;

#[derive(Debug, Clone)]
pub struct HeadscaleManager {
    url: Url,
    api_key: String,
    client: reqwest::Client,
}

impl HeadscaleManager {
    pub fn new(url: Url, api_key: String) -> Self {
        Self {
            url,
            api_key,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_state(&self) -> anyhow::Result<NodesResponse> {
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
