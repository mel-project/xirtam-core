use async_trait::async_trait;
use nanorpc::nanorpc_derive;

/// The internal JSON-RPC interface exposed by xirtam-client.
#[nanorpc_derive]
#[async_trait]
pub trait InternalProtocol {
    async fn echo(&self, s: u64) -> u64;
}
