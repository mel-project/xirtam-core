use anyhow::Context;
use sqlx::SqlitePool;

use xirtam_crypt::dh::DhSecret;
use xirtam_structs::certificate::{CertificateChain, DeviceSecret};
use xirtam_structs::handle::Handle;

#[derive(Clone)]
pub struct Identity {
    pub handle: Handle,
    pub device_secret: DeviceSecret,
    pub cert_chain: CertificateChain,
    pub medium_sk_current: DhSecret,
    pub medium_sk_prev: DhSecret,
}

impl Identity {
    pub async fn load(db: &SqlitePool) -> anyhow::Result<Self> {
        let row = sqlx::query_as::<_, (String, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)>(
            "SELECT handle, device_secret, cert_chain, medium_sk_current, medium_sk_prev \
             FROM client_identity WHERE id = 1",
        )
        .fetch_optional(db)
        .await?;
        let Some((handle, device_secret, cert_chain, medium_sk_current, medium_sk_prev)) = row else {
            anyhow::bail!("client identity not initialized");
        };
        let handle = Handle::parse(handle).context("invalid stored handle")?;
        let device_secret: DeviceSecret = bcs::from_bytes(&device_secret)?;
        let cert_chain: CertificateChain = bcs::from_bytes(&cert_chain)?;
        let medium_sk_current: DhSecret = bcs::from_bytes(&medium_sk_current)?;
        let medium_sk_prev: DhSecret = bcs::from_bytes(&medium_sk_prev)?;
        Ok(Self {
            handle,
            device_secret,
            cert_chain,
            medium_sk_current,
            medium_sk_prev,
        })
    }

}
