use anyctx::AnyCtx;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub db_path: PathBuf,
}

pub type Ctx<T> = fn(&AnyCtx<Config>) -> T;
