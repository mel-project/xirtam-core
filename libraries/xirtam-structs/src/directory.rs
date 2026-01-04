use std::collections::BTreeMap;

use async_trait::async_trait;
use bytes::Bytes;
use nanorpc::nanorpc_derive;
use serde::{Deserialize, Serialize};
use serde_with::{FromInto, IfIsHumanReadable, base64::Base64, serde_as};
use smol_str::SmolStr;
use thiserror::Error;
use xirtam_crypt::{
    hash::Hash,
    signing::{Signature, SigningError, SigningPublic, SigningSecret},
};

use crate::{Message, timestamp::Timestamp};

#[nanorpc_derive]
#[async_trait]
/// The RPC protocol for accessing the directory.
pub trait DirectoryProtocol {
    async fn v1_get_pow_seed(&self) -> PowSeed;
    async fn v1_get_anchor(&self) -> Result<DirectoryAnchor, DirectoryErr>;
    async fn v1_get_chunk(&self, height: u64) -> Result<DirectoryChunk, DirectoryErr>;
    async fn v1_get_headers(
        &self,
        first: u64,
        last: u64,
    ) -> Result<Vec<DirectoryHeader>, DirectoryErr>;
    async fn v1_get_item(&self, key: String) -> Result<DirectoryResponse, DirectoryErr>;
    async fn v1_insert_update(
        &self,
        key: String,
        update: DirectoryUpdate,
        pow: PowSolution,
    ) -> Result<(), DirectoryErr>;
}

#[derive(Error, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryErr {
    #[error("retry later")]
    RetryLater,

    #[error("update rejected (reason: {0})")]
    UpdateRejected(String),
}

/// A proof-of-work seed.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PowSeed {
    pub algo: PowAlgo,
    pub seed: Hash,
    pub use_before: Timestamp,
}

#[derive(Clone, Debug, Serialize, Deserialize, Copy, PartialEq, Eq, Hash)]
pub enum PowAlgo {
    EquiX { effort: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PowSolution {
    pub seed: Hash,
    pub nonce: u64,
    pub solution: Bytes,
}

/// The trust anchor of the entire directory at a given time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryAnchor {
    pub directory_id: SmolStr,
    pub last_header_height: u64,
    pub last_header_hash: Hash,
    pub signature: Signature,
}

impl DirectoryAnchor {
    pub fn verify(&self, anchor_pk: SigningPublic) -> Result<(), SigningError> {
        anchor_pk.verify(&self.signature, &self.to_sign())
    }

    pub fn sign(&mut self, anchor_sk: &SigningSecret) {
        self.signature = anchor_sk.sign(&self.to_sign());
    }

    fn to_sign(&self) -> Vec<u8> {
        bcs::to_bytes(&(
            &self.directory_id,
            &self.last_header_height,
            &self.last_header_hash,
        ))
        .unwrap()
    }
}

/// The header of the a directory snapshot at a particular time.
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryHeader {
    pub prev: Hash,
    pub smt_root: Hash,
    pub time_unix: u64,
}

/// A whole chunk of directory updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryChunk {
    pub header: DirectoryHeader,
    pub updates: BTreeMap<String, Vec<DirectoryUpdate>>,
}

/// A whole chunk of directory updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryUpdate {
    pub prev_update_hash: Hash,
    pub update_type: DirectoryUpdateType,
    pub content: Message,
    pub signature: Signature,
}

impl DirectoryUpdate {
    pub fn verify(&self, signer_pk: SigningPublic) -> Result<(), SigningError> {
        signer_pk.verify(&self.signature, &self.to_sign())
    }

    pub fn sign(&mut self, signer_sk: &SigningSecret) {
        self.signature = signer_sk.sign(&self.to_sign());
    }

    fn to_sign(&self) -> Vec<u8> {
        bcs::to_bytes(&(&self.prev_update_hash, &self.update_type, &self.content)).unwrap()
    }

    fn hash(&self) -> Hash {
        Hash::digest(&bcs::to_bytes(self).unwrap())
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub enum DirectoryUpdateType {
    AddOwner(SigningPublic),
    DelOwner(SigningPublic),
    Update,
}

#[derive(Error, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryHistoryError {
    #[error("no owners available to verify update")]
    NoOwners,
    #[error("invalid update signature")]
    InvalidSignature,
    #[error("update hash does not link to previous update")]
    InvalidPrevHash,
}

pub trait DirectoryHistoryIterExt<'a>: Iterator<Item = &'a DirectoryUpdate> + Sized {
    fn verify_history(self) -> Result<(), DirectoryHistoryError>;
}

impl<'a, I> DirectoryHistoryIterExt<'a> for I
where
    I: Iterator<Item = &'a DirectoryUpdate>,
{
    fn verify_history(self) -> Result<(), DirectoryHistoryError> {
        let mut owners: Vec<SigningPublic> = Vec::new();
        let zero_hash = Hash::from_bytes([0u8; 32]);
        let mut prev_hash: Option<Hash> = None;
        for update in self {
            match prev_hash {
                Some(expected) => {
                    if update.prev_update_hash != expected {
                        return Err(DirectoryHistoryError::InvalidPrevHash);
                    }
                }
                None => {
                    if update.prev_update_hash != zero_hash {
                        return Err(DirectoryHistoryError::InvalidPrevHash);
                    }
                }
            };

            let (is_valid, can_verify) = if owners.is_empty() {
                match update.update_type {
                    DirectoryUpdateType::AddOwner(owner_pk) => {
                        (update.verify(owner_pk).is_ok(), true)
                    }
                    _ => (false, false),
                }
            } else {
                (
                    owners.iter().any(|owner| update.verify(*owner).is_ok()),
                    true,
                )
            };

            if !is_valid {
                return Err(if can_verify {
                    DirectoryHistoryError::InvalidSignature
                } else {
                    DirectoryHistoryError::NoOwners
                });
            }

            match update.update_type {
                DirectoryUpdateType::AddOwner(owner_pk) => {
                    if !owners.contains(&owner_pk) {
                        owners.push(owner_pk);
                    }
                }
                DirectoryUpdateType::DelOwner(owner_pk) => {
                    owners.retain(|existing| *existing != owner_pk);
                }
                DirectoryUpdateType::Update => {}
            }

            prev_hash = Some(update.hash());
        }
        Ok(())
    }
}

/// A response to a directory query.
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryResponse {
    pub history: Vec<DirectoryUpdate>,
    pub proof_height: u64,
    #[serde_as(as = "IfIsHumanReadable<Base64, FromInto<Vec<u8>>>")]
    pub proof_merkle_branch: Bytes,
}
