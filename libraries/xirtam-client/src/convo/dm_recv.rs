use std::time::Duration;

use anyctx::AnyCtx;
use anyhow::Context;
use tracing::warn;
use xirtam_crypt::hash::BcsHashExt;
use xirtam_structs::Blob;
use xirtam_structs::envelope::Envelope;
use xirtam_structs::msg_content::MessageContent;
use xirtam_structs::server::{MailboxId, ServerName};
use xirtam_structs::timestamp::NanoTimestamp;

use crate::config::Config;
use crate::database::{
    DATABASE, DbNotify, ensure_convo_id, ensure_mailbox_state, load_mailbox_after,
    update_mailbox_after,
};
use crate::identity::Identity;
use crate::long_poll::LONG_POLLER;
use crate::server::get_server_client;

use super::dm_common::{device_auth, refresh_own_server_name};

pub(super) async fn dm_recv_loop(ctx: &AnyCtx<Config>) {
    loop {
        if let Err(err) = dm_recv_loop_once(ctx).await {
            tracing::error!(error = %err, "dm recv loop error");
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn dm_recv_loop_once(ctx: &AnyCtx<Config>) -> anyhow::Result<()> {
    let db = ctx.get(DATABASE);
    let identity = Identity::load(db).await?;
    let server_name = match refresh_own_server_name(ctx, db, &identity).await {
        Ok(name) => name,
        Err(err) => match identity.server_name.clone() {
            Some(name) => {
                tracing::warn!(error = %err, "failed to refresh server name");
                name
            }
            None => return Err(err),
        },
    };
    let server = get_server_client(ctx, &server_name).await?;
    let auth = device_auth(server.as_ref(), &identity).await?;
    let mailbox = MailboxId::direct(&identity.username);
    ensure_mailbox_state(db, &server_name, mailbox, NanoTimestamp(0)).await?;
    let mut after = load_mailbox_after(db, &server_name, mailbox).await?;
    let poller = ctx.get(LONG_POLLER);
    loop {
        let entry = match poller.recv(server.clone(), auth, mailbox, after).await {
            Ok(entry) => entry,
            Err(err) => {
                tracing::warn!(error = %err, "mailbox recv error");
                continue;
            }
        };
        after = entry.received_at;
        if let Err(err) = process_mailbox_entry(ctx, &server_name, mailbox, entry).await {
            tracing::warn!(error = %err, "failed to process mailbox entry");
        }
        // notify once to prevent thrashing
        DbNotify::touch();
    }
}

async fn process_mailbox_entry(
    ctx: &AnyCtx<Config>,
    server_name: &ServerName,
    mailbox: MailboxId,
    entry: xirtam_structs::server::MailboxEntry,
) -> anyhow::Result<()> {
    let db = ctx.get(DATABASE);
    let dir = ctx.get(crate::directory::DIR_CLIENT);
    let identity = Identity::load(db).await?;
    update_mailbox_after(db, server_name, mailbox, entry.received_at).await?;
    let message = entry.message;
    if message.kind != Blob::V1_DIRECT_MESSAGE {
        warn!(kind = %message.kind, "ignoring non-dm mailbox entry");
        return Ok(());
    }
    let envelope: Envelope = bcs::from_bytes(&message.inner)?;
    let device_pk = identity.device_secret.public();
    let device_hash = device_pk.bcs_hash();
    let header_count = envelope.headers.len();
    let has_header = envelope.headers.contains_key(&device_hash);
    tracing::debug!(
        received_at = entry.received_at.0,
        header_count,
        has_header,
        device_hash = %device_hash,
        "dm envelope received",
    );
    let decrypted = match envelope.decrypt_message(&device_pk, &identity.medium_sk_current) {
        Ok(decrypted) => {
            tracing::debug!("dm decrypt with current medium key ok");
            decrypted
        }
        Err(err) => {
            tracing::debug!(error = %err, "dm decrypt with current medium key failed");
            match envelope.decrypt_message(&device_pk, &identity.medium_sk_prev) {
                Ok(decrypted) => {
                    tracing::debug!("dm decrypt with previous medium key ok");
                    decrypted
                }
                Err(err) => {
                    tracing::warn!(error = %err, "dm decrypt with previous medium key failed");
                    return Err(anyhow::anyhow!("failed to decrypt envelope"));
                }
            }
        }
    };
    let sender_username = decrypted.username().clone();
    let sender_descriptor = dir
        .get_user_descriptor(&sender_username)
        .await?
        .context("sender username not in directory")?;
    let message = decrypted
        .verify(sender_descriptor.root_cert_hash)
        .map_err(|_| anyhow::anyhow!("failed to verify envelope"))?;
    if message.kind != Blob::V1_MESSAGE_CONTENT {
        warn!(kind = %message.kind, "ignoring non-message-content dm");
        return Ok(());
    }
    let content: MessageContent = bcs::from_bytes(&message.inner)?;
    if content.recipient != identity.username && sender_username != identity.username {
        warn!(
            sender = %sender_username,
            recipient = %content.recipient,
            "ignoring dm with mismatched recipient",
        );
        return Ok(());
    }
    let peer_username = if sender_username == identity.username {
        content.recipient.clone()
    } else {
        sender_username.clone()
    };
    let mut tx = db.begin().await?;
    let convo_id = ensure_convo_id(tx.as_mut(), "direct", peer_username.as_str()).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO convo_messages \
         (convo_id, sender_username, mime, body, received_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(convo_id)
    .bind(sender_username.as_str())
    .bind(content.mime.as_str())
    .bind(content.body.to_vec())
    .bind(entry.received_at.0 as i64)
    .execute(tx.as_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}
