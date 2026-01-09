use std::sync::mpsc::Receiver;

use anyctx::AnyCtx;
use async_trait::async_trait;
use nanorpc::{JrpcRequest, JrpcResponse};
use tokio::sync::oneshot;

use crate::{Config, database::DATABASE, internal::InternalProtocol};

pub async fn main_loop(
    cfg: Config,
    _recv_rpc: Receiver<(JrpcRequest, oneshot::Sender<JrpcResponse>)>,
) {
    let ctx = AnyCtx::new(cfg);
    let _db = ctx.get(DATABASE);
    let internal = InternalImpl(ctx);
    let _ = internal.echo(0).await;
    todo!()
}

struct InternalImpl(AnyCtx<Config>);

#[async_trait]
impl InternalProtocol for InternalImpl {
    async fn echo(&self, s: u64) -> u64 {
        let _ = self.0.init();
        s
    }
}
