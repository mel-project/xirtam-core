use std::path::{Path, PathBuf};

use anyctx::AnyCtx;
use memmap2::Mmap;
use nullspace_structs::fragment::{Fragment, file_into_fragments};
use smol_str::SmolStr;

use crate::Config;
use crate::database::{DATABASE, identity_exists};
use crate::events::emit_event;
use crate::identity::Identity;
use crate::internal::{Event, InternalRpcError, device_auth, internal_err};
use crate::server::get_server_client;

pub async fn upload_start(
    ctx: &AnyCtx<Config>,
    absolute_path: PathBuf,
    mime: SmolStr,
) -> Result<i64, InternalRpcError> {
    let upload_id = rand::random();
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(err) = upload_inner(&ctx, absolute_path, mime, upload_id).await {
            emit_event(
                &ctx,
                Event::UploadFailed {
                    id: upload_id,
                    error: err.to_string(),
                },
            );
        }
    });
    Ok(upload_id)
}

async fn upload_inner(
    ctx: &AnyCtx<Config>,
    absolute_path: PathBuf,
    mime: SmolStr,
    upload_id: i64,
) -> Result<(), InternalRpcError> {
    let db = ctx.get(DATABASE);
    if !identity_exists(db).await.map_err(internal_err)? {
        return Err(InternalRpcError::NotReady);
    }

    let identity = Identity::load(db).await.map_err(internal_err)?;
    let server_name = identity
        .server_name
        .clone()
        .ok_or_else(|| InternalRpcError::Other("server name not available".into()))?;
    let client = get_server_client(ctx, &server_name)
        .await
        .map_err(internal_err)?;
    let auth = device_auth(&client, &identity.username, &identity.cert_chain).await?;

    let file = std::fs::File::open(&absolute_path).map_err(internal_err)?;
    let map = unsafe { Mmap::map(&file).map_err(internal_err)? };
    let filename = file_basename(&absolute_path)?;
    let (root, fragments) = file_into_fragments(&filename, mime.as_str(), &map);
    let total_size = root.total_size;
    let mut uploaded_size = 0u64;

    for frag in fragments {
        let leaf_size = match &frag {
            Fragment::Leaf(leaf) => Some(leaf.data.len() as u64),
            Fragment::Node(_) => None,
        };
        let response = client
            .v1_upload_frag(auth, frag.to_static(), 0)
            .await
            .map_err(internal_err)?;
        if let Err(err) = response {
            return Err(InternalRpcError::Other(err.to_string()));
        }
        if let Some(size) = leaf_size {
            uploaded_size = uploaded_size.saturating_add(size);
            emit_event(
                ctx,
                Event::UploadProgress {
                    id: upload_id,
                    uploaded_size,
                    total_size,
                },
            );
        }
    }

    emit_event(
        ctx,
        Event::UploadDone {
            id: upload_id,
            root,
        },
    );
    Ok(())
}

fn file_basename(path: &Path) -> Result<String, InternalRpcError> {
    let name = path
        .file_name()
        .ok_or_else(|| InternalRpcError::Other("missing filename".into()))?;
    let name = name
        .to_str()
        .ok_or_else(|| InternalRpcError::Other("filename is not valid utf-8".into()))?;
    Ok(name.to_string())
}
