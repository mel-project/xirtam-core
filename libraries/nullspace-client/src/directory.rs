use anyctx::AnyCtx;
use nullspace_dirclient::DirClient;

use crate::config::{Config, Ctx};
use crate::database::DATABASE;
use crate::rpc_pool::RPC_POOL;

pub static DIR_CLIENT: Ctx<DirClient> = |ctx: &AnyCtx<Config>| {
    let transport = ctx.get(RPC_POOL).rpc(ctx.init().dir_endpoint.clone());
    pollster::block_on(async {
        DirClient::new(
            transport,
            ctx.init().dir_anchor_pk,
            ctx.get(DATABASE).clone(),
        )
        .await
    })
    .expect("failed to initialize directory client")
};
