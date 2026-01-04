use nanorpc::{DynRpcTransport, RpcTransport};
use sqlx::SqlitePool;
use xirtam_crypt::{hash::Hash, signing::SigningPublic};
use xirtam_structs::Message;
use xirtam_structs::directory::{
    DirectoryClient, DirectoryHistoryIterExt, DirectoryResponse, DirectoryUpdate,
    DirectoryUpdateType, PowSolution,
};
mod header_sync;

pub struct DirClient {
    raw: DirectoryClient<DynRpcTransport>,
    anchor_pk: SigningPublic,
    pool: SqlitePool,
}

#[derive(Clone, Debug)]
pub struct DirectoryListing {
    pub latest: Option<Message>,
    pub owners: Vec<SigningPublic>,
}

impl DirClient {
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

    pub fn raw(&self) -> &DirectoryClient<DynRpcTransport> {
        &self.raw
    }

    pub async fn query(&self, key: impl Into<String>) -> anyhow::Result<DirectoryListing> {
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
