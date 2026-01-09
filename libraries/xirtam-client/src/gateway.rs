use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::Context;
use moka::future::Cache;
use xirtam_dirclient::DirClient;
use xirtam_nanorpc::Transport;
use xirtam_structs::gateway::{GatewayClient, GatewayName};

use crate::config::Config;
use crate::database::DATABASE;

static GATEWAY_CACHE: LazyLock<Cache<GatewayName, Arc<GatewayClient>>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_idle(Duration::from_secs(12 * 60 * 60))
        .build()
});

pub async fn get_gateway_client(
    ctx: &anyctx::AnyCtx<Config>,
    name: &GatewayName,
) -> anyhow::Result<Arc<GatewayClient>> {
    GATEWAY_CACHE
        .try_get_with(name.clone(), async {
            let transport = Transport::new(ctx.init().dir_endpoint.clone());
            let dir = DirClient::new(
                transport,
                ctx.init().dir_anchor_pk,
                ctx.get(DATABASE).clone(),
            )
            .await?;
            let descriptor = dir
                .get_gateway_descriptor(name)
                .await?
                .context("gateway not in directory")?;
            let endpoint = descriptor
                .public_urls
                .first()
                .cloned()
                .context("gateway has no public URLs")?;
            Ok(Arc::new(GatewayClient::from(Transport::new(endpoint))))
        })
        .await
        .map_err(|err: Arc<anyhow::Error>| anyhow::anyhow!(err.to_string()))
}
