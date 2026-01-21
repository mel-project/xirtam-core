use anyctx::AnyCtx;
use anyhow::Context;
use nullpoint_structs::server::{AuthToken, ServerClient, ServerName};

use crate::config::Config;
use crate::database::DATABASE;
use crate::directory::DIR_CLIENT;
use crate::identity::{Identity, store_server_name};

pub(super) async fn device_auth(
    client: &ServerClient,
    identity: &Identity,
) -> anyhow::Result<AuthToken> {
    client
        .v1_device_auth(identity.username.clone(), identity.cert_chain.clone())
        .await?
        .map_err(|err| anyhow::anyhow!(err.to_string()))
}

pub(super) async fn own_server_name(
    ctx: &AnyCtx<Config>,
    identity: &Identity,
) -> anyhow::Result<ServerName> {
    if let Some(server_name) = identity.server_name.clone() {
        return Ok(server_name);
    }
    let db = ctx.get(DATABASE);
    refresh_own_server_name(ctx, db, identity).await
}

pub(super) async fn refresh_own_server_name(
    ctx: &AnyCtx<Config>,
    db: &sqlx::SqlitePool,
    identity: &Identity,
) -> anyhow::Result<ServerName> {
    let dir = ctx.get(DIR_CLIENT);
    let descriptor = dir
        .get_user_descriptor(&identity.username)
        .await?
        .context("identity username not in directory")?;
    store_server_name(db, &descriptor.server_name).await?;
    Ok(descriptor.server_name)
}
