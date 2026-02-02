use anyctx::AnyCtx;
use nullspace_rpc_pool::RpcPool;

use crate::config::{Config, Ctx};

pub static RPC_POOL: Ctx<RpcPool> =
    |_ctx: &AnyCtx<Config>| RpcPool::builder().max_concurrency(1).build();
