use std::time::Duration;

use async_trait::async_trait;
use nanorpc::{JrpcRequest, JrpcResponse, RpcTransport};
use url::Url;

#[derive(Clone)]
pub struct Transport {
    client: reqwest::Client,
    endpoint: Url,
}

impl Transport {
    pub fn new(endpoint: Url) -> Self {
        Self {
            client: reqwest::ClientBuilder::new()
                .timeout(Duration::from_secs(600))
                .build()
                .unwrap(),
            endpoint,
        }
    }
}

#[async_trait]
impl RpcTransport for Transport {
    type Error = anyhow::Error;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        match self.endpoint.scheme() {
            "http" | "https" => {
                let resp = self
                    .client
                    .post(self.endpoint.clone())
                    .json(&req)
                    .send()
                    .await?
                    .error_for_status()?;
                Ok(resp.json::<JrpcResponse>().await?)
            }
            scheme => anyhow::bail!("unsupported RPC endpoint scheme: {scheme}"),
        }
    }
}
