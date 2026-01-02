use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "xirtam-directory")]
pub struct Args {
    #[arg(long, default_value = "0.0.0.0:8000")]
    pub listen: SocketAddr,
    #[arg(long)]
    pub db_path: PathBuf,
    #[arg(long)]
    pub secret_key: PathBuf,
}
