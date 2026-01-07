use std::{fs, net::SocketAddr, path::PathBuf, sync::LazyLock};

use clap::Parser;
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(name = "xirtam-gateway")]
struct Args {
    #[arg(long)]
    config: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub listen: SocketAddr,
    pub db_path: PathBuf,
}

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| {
    let args = Args::parse();
    let raw = fs::read_to_string(&args.config).unwrap_or_else(|err| {
        panic!("failed to read config {}: {err}", args.config.display());
    });
    toml::from_str(&raw).unwrap_or_else(|err| {
        panic!("failed to parse config {}: {err}", args.config.display());
    })
});
