#![doc = include_str!(concat!(env!("OUT_DIR"), "/README-rustdocified.md"))]

use nanorpc::{DynRpcTransport, RpcTransport};
use sqlx::SqlitePool;
use xirtam_crypt::{hash::Hash, signing::{Signable, SigningPublic}};
use xirtam_structs::Message;
use xirtam_structs::directory::{
    DirectoryClient, DirectoryHistoryIterExt, DirectoryResponse, DirectoryUpdate,
    DirectoryUpdateType, PowSolution,
};
mod header_sync;

/// High-level directory client with local header storage and proof checks.
pub struct DirClient {
    raw: DirectoryClient<DynRpcTransport>,
    anchor_pk: SigningPublic,
    pool: SqlitePool,
}

/// Derived listing information for a directory key.
#[derive(Clone, Debug)]
pub struct DirectoryListing {
    /// Latest content message for the key, if present.
    pub latest: Option<Message>,
    /// Current owners for the key, after applying ownership updates in order.
    pub owners: Vec<SigningPublic>,
}

impl DirClient {
    /// Create a new client and ensure the local header schema is initialized.
    pub async fn new<T>(
        transport: T,
        anchor_pk: SigningPublic,
        pool: SqlitePool,
    ) -> anyhow::Result<Self>
    where
        T: RpcTransport,
        T::Error: Into<anyhow::Error>,
    {
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self {
            raw: DirectoryClient::from(transport),
            anchor_pk,
            pool,
        })
    }

    /// Access the raw RPC client when direct protocol calls are needed.
    pub fn raw(&self) -> &DirectoryClient<DynRpcTransport> {
        &self.raw
    }

    /// Fetch and verify a directory entry by key.
    ///
    /// This validates the signed anchor, syncs headers, checks the SMT proof,
    /// and returns a derived listing.
    pub async fn query_raw(&self, key: impl Into<String>) -> anyhow::Result<DirectoryListing> {
        let key = key.into();
        let anchor = self
            .raw
            .v1_get_anchor()
            .await?
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        anchor.verify(self.anchor_pk)?;
        header_sync::sync_headers(&self.raw, &self.pool, &anchor).await?;
        let response = self
            .raw
            .v1_get_item(key.clone())
            .await?
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        verify_response(&self.pool, &key, &anchor, &response).await?;
        let listing = build_listing(&response)?;
        Ok(listing)
    }

    /// Submit a raw directory update for a key.
    pub async fn insert_raw(
        &self,
        key: impl Into<String>,
        update: DirectoryUpdate,
        pow: PowSolution,
    ) -> anyhow::Result<()> {
        self.raw
            .v1_insert_update(key.into(), update, pow)
            .await?
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        Ok(())
    }

    /// Report local header sync progress as `(stored_height, anchor_height)`.
    pub async fn sync_progress(&self) -> anyhow::Result<(u64, u64)> {
        let stored = header_sync::max_stored_height(&self.pool)
            .await?
            .unwrap_or_default();
        let anchor = self
            .raw
            .v1_get_anchor()
            .await?
            .map_err(|err| anyhow::anyhow!(err.to_string()))?
            .last_header_height;
        Ok((stored, anchor))
    }
}

async fn verify_response(
    pool: &SqlitePool,
    key: &str,
    anchor: &xirtam_structs::directory::DirectoryAnchor,
    response: &DirectoryResponse,
) -> anyhow::Result<()> {
    if response.proof_height > anchor.last_header_height {
        anyhow::bail!("header chain mismatch");
    }
    let header = header_sync::load_header(pool, response.proof_height).await?;
    let root = header.smt_root;
    let compressed = novasmt::CompressedProof(response.proof_merkle_branch.to_vec());
    let proof = compressed
        .decompress()
        .ok_or_else(|| anyhow::anyhow!("failed to decompress proof"))?;
    let key_hash = Hash::digest(key.as_bytes());
    // an empty history means simply *absence* in the SMT
    let value = if response.history.is_empty() {
        vec![]
    } else {
        bcs::to_bytes(&response.history)?
    };
    if !proof.verify(root.to_bytes(), key_hash.to_bytes(), &value) {
        anyhow::bail!("invalid proof");
    }
    Ok(())
}

fn build_listing(response: &DirectoryResponse) -> anyhow::Result<DirectoryListing> {
    response
        .history
        .iter()
        .verify_history()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let mut owners = Vec::new();
    let mut latest = None;
    for update in &response.history {
        match update.update_type {
            DirectoryUpdateType::AddOwner(owner) => {
                if !owners.contains(&owner) {
                    owners.push(owner);
                }
            }
            DirectoryUpdateType::DelOwner(owner) => {
                owners.retain(|existing| *existing != owner);
            }
            DirectoryUpdateType::Update => {
                latest = Some(update.content.clone());
            }
        }
    }
    Ok(DirectoryListing { latest, owners })
}
