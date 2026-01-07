use std::collections::BTreeMap;

use axum::{
    http::{StatusCode, header},
    response::IntoResponse,
};
use bytes::Bytes;
use nanorpc::{JrpcRequest, RpcService};
use xirtam_structs::certificate::CertificateChain;
use xirtam_structs::gateway::{
    AuthToken, GatewayProtocol, GatewayServerError, GatewayService, MailboxAclEdit, MailboxEntry,
    MailboxId, MailboxRecvArgs,
};
use xirtam_structs::{Message, handle::Handle};

#[derive(Clone, Default)]
pub struct GatewayServer;

pub async fn handle_rpc(body: Bytes) -> impl IntoResponse {
    let Ok(req) = serde_json::from_slice::<JrpcRequest>(&body) else {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain")],
            Vec::new(),
        );
    };
    let service = GatewayService(GatewayServer::default());
    let response = service.respond_raw(req).await;
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_vec(&response).unwrap(),
    )
}

#[async_trait::async_trait]
impl GatewayProtocol for GatewayServer {
    async fn v1_device_auth(
        &self,
        _handle: Handle,
        _cert: CertificateChain,
    ) -> Result<(), GatewayServerError> {
        Err(GatewayServerError::RetryLater)
    }

    async fn v1_device_list(
        &self,
        _handle: Handle,
    ) -> Result<Option<CertificateChain>, GatewayServerError> {
        Err(GatewayServerError::RetryLater)
    }

    async fn v1_mailbox_send(
        &self,
        _auth: AuthToken,
        _mailbox: MailboxId,
        _message: Message,
    ) -> Result<(), GatewayServerError> {
        Err(GatewayServerError::RetryLater)
    }

    async fn v1_mailbox_multirecv(
        &self,
        _args: Vec<MailboxRecvArgs>,
        _timeout_ms: u64,
    ) -> Result<BTreeMap<MailboxId, Vec<MailboxEntry>>, GatewayServerError> {
        Err(GatewayServerError::RetryLater)
    }

    async fn v1_mailbox_acl_edit(
        &self,
        _auth: AuthToken,
        _arg: MailboxAclEdit,
    ) -> Result<(), GatewayServerError> {
        Err(GatewayServerError::RetryLater)
    }
}
