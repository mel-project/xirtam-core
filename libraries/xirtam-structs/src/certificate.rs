use anyhow::bail;
use serde::{Deserialize, Serialize};
use xirtam_crypt::{
    dh::DhPublic,
    hash::{BcsHashExt, Hash},
    signing::{Signable, Signature, SigningPublic},
};

use crate::timestamp::Timestamp;

#[derive(Clone, Debug, Serialize, Deserialize)]
/// The identity public key of a device, that never changes throughout the lifetime of the device.
pub struct DevicePublic {
    pub sign_pk: SigningPublic,
    pub long_pk: DhPublic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
/// The certificate of a particular device, which includes the public key and the capabilities of the device.
pub struct DeviceCertificate {
    pub pk: DevicePublic,
    pub signed_by: Hash,
    pub expiry: Timestamp,
    pub can_sign: bool,
    pub signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
/// A chain a certificates that ultimately represents a set of authorized devices.
pub struct CertificateChain(pub Vec<DeviceCertificate>);

impl Signable for DeviceCertificate {
    fn signed_value(&self) -> Vec<u8> {
        bcs::to_bytes(&(&self.pk, &self.signed_by, &self.expiry, &self.can_sign))
            .expect("bcs serialization failed")
    }

    fn signature_mut(&mut self) -> &mut Signature {
        &mut self.signature
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }
}

impl CertificateChain {
    /// Verify the chain and return the non-expired certificates.
    pub fn verify(&self, trusted_pk_hash: Hash) -> anyhow::Result<Vec<DeviceCertificate>> {
        let now = unix_time();
        let mut trusted_signers: Vec<(Hash, SigningPublic)> = Vec::new();
        let mut valid = Vec::new();

        for (idx, cert) in self.0.iter().enumerate() {
            let signer = trusted_signers
                .iter()
                .find(|(hash, _)| *hash == cert.signed_by)
                .map(|(_, pk)| *pk);

            let signer = match signer {
                Some(pk) => pk,
                None if cert.signed_by == trusted_pk_hash => {
                    let cert_hash = cert.pk.sign_pk.bcs_hash();
                    if cert_hash != trusted_pk_hash {
                        bail!("certificate {} signed by unknown trusted key", idx);
                    }
                    cert.pk.sign_pk
                }
                None => bail!("certificate {} signed by unknown key", idx),
            };

            cert.verify(signer)
                .map_err(|err| anyhow::anyhow!(err.to_string()))?;

            if cert.expiry > now {
                trusted_signers.push((cert.pk.sign_pk.bcs_hash(), cert.pk.sign_pk));
                valid.push(cert.clone());
            }
        }

        Ok(valid)
    }
}

fn unix_time() -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Timestamp(seconds)
}
