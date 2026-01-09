use anyctx::AnyCtx;
use xirtam_dirclient::DirClient;
use xirtam_nanorpc::Transport;

use crate::config::{Config, Ctx};
use crate::database::DATABASE;

pub static DIR_CLIENT: Ctx<DirClient> = |ctx: &AnyCtx<Config>| {
    let transport = Transport::new(ctx.init().dir_endpoint.clone());
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
