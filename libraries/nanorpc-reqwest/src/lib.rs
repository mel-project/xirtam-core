use async_trait::async_trait;
use nanorpc::{JrpcRequest, JrpcResponse, RpcTransport};

#[derive(Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    endpoint: String,
}

impl ReqwestTransport {
    pub fn new(client: reqwest::Client, endpoint: impl Into<String>) -> Self {
        Self {
            client,
            endpoint: endpoint.into(),
        }
    }
}

#[async_trait]
impl RpcTransport for ReqwestTransport {
    type Error = anyhow::Error;

    async fn call_raw(&self, req: JrpcRequest) -> Result<JrpcResponse, Self::Error> {
        let resp = self
            .client
            .post(&self.endpoint)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json::<JrpcResponse>().await?)
    }
}
