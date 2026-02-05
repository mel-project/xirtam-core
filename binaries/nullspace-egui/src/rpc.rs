use std::sync::OnceLock;

use nullspace_client::InternalClient;

static RPC: OnceLock<InternalClient> = OnceLock::new();

pub fn init_rpc(rpc: InternalClient) {
    if RPC.set(rpc).is_err() {
        panic!("rpc already initialized");
    }
}

pub fn get_rpc() -> &'static InternalClient {
    RPC.get().expect("rpc not initialized")
}
