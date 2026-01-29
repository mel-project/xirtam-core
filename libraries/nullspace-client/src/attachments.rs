use std::path::{Path, PathBuf};

use anyctx::AnyCtx;
use bytes::Bytes;
use nullspace_crypt::aead::AeadKey;
use nullspace_crypt::hash::{BcsHashExt, Hash};
use nullspace_structs::fragment::{Fragment, FragmentLeaf, FragmentNode, FragmentRoot};
use nullspace_structs::username::UserName;
use smol_str::SmolStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::Config;
use crate::database::{DATABASE, identity_exists};
use crate::directory::DIR_CLIENT;
use crate::events::emit_event;
use crate::identity::Identity;
use crate::internal::{Event, InternalRpcError, device_auth, internal_err};
use crate::server::get_server_client;

const CHUNK_SIZE_BYTES: usize = 256 * 1024;
const MAX_FANOUT: usize = 4096;

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

fn nonce_for_leaf(index: u64) -> [u8; 24] {
    let mut nonce = [0u8; 24];
    nonce[..8].copy_from_slice(b"NSFRAG01");
    nonce[8..16].copy_from_slice(&index.to_le_bytes());
    nonce
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

    let filename = file_basename(&absolute_path)?;
    let mut file = tokio::fs::File::open(&absolute_path)
        .await
        .map_err(internal_err)?;
    let total_size = file.metadata().await.map_err(internal_err)?.len();
    let content_key = AeadKey::random();
    let mut uploaded_size = 0u64;
    let mut leaf_index = 0u64;
    let mut current_level: Vec<ChildRef> = Vec::new();
    let mut buf = vec![0u8; CHUNK_SIZE_BYTES];

    loop {
        let read = file.read(&mut buf).await.map_err(internal_err)?;
        if read == 0 {
            break;
        }
        let nonce = nonce_for_leaf(leaf_index);
        let ciphertext = content_key
            .encrypt(nonce, &buf[..read], &[])
            .map_err(|_| InternalRpcError::Other("chunk encryption failed".into()))?;
        let leaf = FragmentLeaf {
            data: Bytes::from(ciphertext),
        };
        let hash = Fragment::Leaf(leaf.clone()).bcs_hash();
        current_level.push(ChildRef {
            hash,
            size: read as u64,
        });
        let response = client
            .v1_upload_frag(auth, Fragment::Leaf(leaf), 0)
            .await
            .map_err(internal_err)?;
        if let Err(err) = response {
            return Err(InternalRpcError::Other(err.to_string()));
        }
        uploaded_size = uploaded_size.saturating_add(read as u64);
        emit_event(
            ctx,
            Event::UploadProgress {
                id: upload_id,
                uploaded_size,
                total_size,
            },
        );
        leaf_index = leaf_index.saturating_add(1);
    }

    let mut nodes: Vec<FragmentNode> = Vec::new();
    while current_level.len() > MAX_FANOUT {
        let mut next_level = Vec::new();
        for group in current_level.chunks(MAX_FANOUT) {
            let pointers: Vec<nullspace_crypt::hash::Hash> =
                group.iter().map(|child| child.hash).collect();
            let size = group.iter().map(|child| child.size).sum();
            let node = FragmentNode { size, pointers };
            let hash = Fragment::Node(node.clone()).bcs_hash();
            next_level.push(ChildRef { hash, size });
            nodes.push(node);
        }
        current_level = next_level;
    }

    for node in nodes {
        let response = client
            .v1_upload_frag(auth, Fragment::Node(node), 0)
            .await
            .map_err(internal_err)?;
        if let Err(err) = response {
            return Err(InternalRpcError::Other(err.to_string()));
        }
    }

    let root = FragmentRoot {
        filename: SmolStr::new(filename),
        mime,
        total_size,
        pointers: current_level.into_iter().map(|child| child.hash).collect(),
        content_key: Some(content_key),
    };

    emit_event(
        ctx,
        Event::UploadDone {
            id: upload_id,
            root,
        },
    );
    Ok(())
}

pub async fn download_start(
    ctx: &AnyCtx<Config>,
    attachment_id: Hash,
    save_dir: PathBuf,
) -> Result<i64, InternalRpcError> {
    if !save_dir.is_absolute() {
        return Err(InternalRpcError::Other("save dir must be absolute".into()));
    }
    let download_id = rand::random();
    let ctx = ctx.clone();
    tokio::spawn(async move {
        if let Err(err) = download_inner(&ctx, attachment_id, save_dir, download_id).await {
            emit_event(
                &ctx,
                Event::DownloadFailed {
                    id: download_id,
                    error: err.to_string(),
                },
            );
        }
    });
    Ok(download_id)
}

async fn download_inner(
    ctx: &AnyCtx<Config>,
    attachment_id: Hash,
    save_dir: PathBuf,
    download_id: i64,
) -> Result<(), InternalRpcError> {
    let db = ctx.get(DATABASE);
    if !identity_exists(db).await.map_err(internal_err)? {
        return Err(InternalRpcError::NotReady);
    }
    let (sender_username, root) = load_attachment_root(db, attachment_id).await?;
    let sender = UserName::parse(&sender_username).map_err(internal_err)?;
    let server_name = ctx
        .get(DIR_CLIENT)
        .get_user_descriptor(&sender)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| InternalRpcError::Other("sender not in directory".into()))?
        .server_name;
    let client = get_server_client(ctx, &server_name)
        .await
        .map_err(internal_err)?;

    tokio::fs::create_dir_all(&save_dir)
        .await
        .map_err(internal_err)?;
    let filename = sanitize_filename(root.filename.as_str());
    let final_path = unique_path(&save_dir, &filename).await?;
    let part_path = final_path.with_extension("part");
    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(internal_err)?;

    if root.pointers.is_empty() {
        file.flush().await.map_err(internal_err)?;
        tokio::fs::rename(&part_path, &final_path)
            .await
            .map_err(internal_err)?;
        emit_event(
            ctx,
            Event::DownloadDone {
                id: download_id,
                absolute_path: final_path,
            },
        );
        return Ok(());
    }

    let mut stack: Vec<Hash> = root.pointers.iter().rev().cloned().collect();
    let mut leaf_index = 0u64;
    let mut downloaded_size = 0u64;

    while let Some(hash) = stack.pop() {
        let response = client.v1_download_frag(hash).await.map_err(internal_err)?;
        let frag = response
            .map_err(|err| InternalRpcError::Other(err.to_string()))?
            .ok_or_else(|| InternalRpcError::Other("missing fragment".into()))?;
        if frag.bcs_hash() != hash {
            return Err(InternalRpcError::Other("fragment hash mismatch".into()));
        }
        match frag {
            Fragment::Node(node) => {
                for child in node.pointers.iter().rev() {
                    stack.push(*child);
                }
            }
            Fragment::Leaf(leaf) => {
                let plaintext = if let Some(key) = &root.content_key {
                    let nonce = nonce_for_leaf(leaf_index);
                    key.decrypt(nonce, &leaf.data, &[])
                        .map_err(|_| InternalRpcError::Other("chunk decryption failed".into()))?
                } else {
                    leaf.data.to_vec()
                };
                file.write_all(&plaintext).await.map_err(internal_err)?;
                downloaded_size = downloaded_size.saturating_add(plaintext.len() as u64);
                emit_event(
                    ctx,
                    Event::DownloadProgress {
                        id: download_id,
                        downloaded_size,
                        total_size: root.total_size,
                    },
                );
                leaf_index = leaf_index.saturating_add(1);
            }
        }
    }

    if downloaded_size != root.total_size {
        return Err(InternalRpcError::Other("download size mismatch".into()));
    }

    file.flush().await.map_err(internal_err)?;
    tokio::fs::rename(&part_path, &final_path)
        .await
        .map_err(internal_err)?;
    emit_event(
        ctx,
        Event::DownloadDone {
            id: download_id,
            absolute_path: final_path,
        },
    );
    Ok(())
}

pub async fn store_attachment_root(
    db: &sqlx::SqlitePool,
    sender: &UserName,
    root: &FragmentRoot,
) -> anyhow::Result<Hash> {
    let mut conn = db.acquire().await?;
    store_attachment_root_conn(&mut conn, sender, root).await
}

pub async fn store_attachment_root_conn(
    conn: &mut sqlx::SqliteConnection,
    sender: &UserName,
    root: &FragmentRoot,
) -> anyhow::Result<Hash> {
    let hash = root.bcs_hash();
    let root_bcs = bcs::to_bytes(root)?;
    sqlx::query(
        "INSERT INTO attachment_roots (hash, root, sender_username) \
         VALUES (?, ?, ?) \
         ON CONFLICT(hash) DO UPDATE SET \
           root = excluded.root, \
           sender_username = excluded.sender_username",
    )
    .bind(hash.to_bytes().to_vec())
    .bind(root_bcs)
    .bind(sender.as_str())
    .execute(conn)
    .await?;
    Ok(hash)
}

pub async fn load_attachment_root(
    db: &sqlx::SqlitePool,
    attachment_id: Hash,
) -> Result<(String, FragmentRoot), InternalRpcError> {
    let row = sqlx::query_as::<_, (String, Vec<u8>)>(
        "SELECT sender_username, root FROM attachment_roots WHERE hash = ?",
    )
    .bind(attachment_id.to_bytes().to_vec())
    .fetch_optional(db)
    .await
    .map_err(internal_err)?;
    let Some((sender_username, root_bytes)) = row else {
        return Err(InternalRpcError::Other("attachment not found".into()));
    };
    let root: FragmentRoot = bcs::from_bytes(&root_bytes).map_err(internal_err)?;
    Ok((sender_username, root))
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

fn sanitize_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len().max(12));
    for ch in name.chars() {
        if ch == '/' || ch == '\\' || ch.is_control() {
            continue;
        }
        out.push(ch);
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "attachment.bin".to_string()
    } else {
        trimmed.to_string()
    }
}

async fn unique_path(dir: &Path, filename: &str) -> Result<PathBuf, InternalRpcError> {
    let base = dir.join(filename);
    if tokio::fs::try_exists(&base).await.map_err(internal_err)? == false {
        return Ok(base);
    }
    let (stem, ext) = split_extension(filename);
    for i in 1..=9999 {
        let candidate = if ext.is_empty() {
            dir.join(format!("{stem} ({i})"))
        } else {
            dir.join(format!("{stem} ({i}).{ext}"))
        };
        if tokio::fs::try_exists(&candidate)
            .await
            .map_err(internal_err)?
            == false
        {
            return Ok(candidate);
        }
    }
    Err(InternalRpcError::Other(
        "could not pick unique filename".into(),
    ))
}

fn split_extension(filename: &str) -> (&str, &str) {
    let Some(pos) = filename.rfind('.') else {
        return (filename, "");
    };
    let (stem, ext) = filename.split_at(pos);
    let ext = ext.trim_start_matches('.');
    if stem.is_empty() || ext.is_empty() {
        (filename, "")
    } else {
        (stem, ext)
    }
}

struct ChildRef {
    hash: nullspace_crypt::hash::Hash,
    size: u64,
}
