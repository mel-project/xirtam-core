use std::str::FromStr;

use clap::{Parser, Subcommand};
use nanorpc_reqwest::ReqwestTransport;
use sqlx::{
    ConnectOptions,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use tracing_subscriber::EnvFilter;
use xirtam_crypt::{
    hash::Hash,
    signing::{SigningPublic, SigningSecret},
};
use xirtam_dirclient::DirClient;
use xirtam_structs::handle::Handle;

#[derive(Parser, Debug)]
#[command(name = "xirtam-dirclient-cli")]
struct Args {
    #[arg(long)]
    endpoint: String,
    #[arg(long)]
    public_key: SigningPublic,
    #[arg(long, default_value = ":memory:")]
    db_path: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Query {
        #[arg(long)]
        handle: Handle,
    },
    Insert {
        #[arg(long)]
        handle: Handle,
        #[arg(long)]
        roothash: Hash,
        #[arg(long)]
        secret_key: SigningSecret,
    },
    AddOwner {
        #[arg(long)]
        handle: Handle,
        #[arg(long)]
        owner: SigningPublic,
        #[arg(long)]
        secret_key: SigningSecret,
    },
    DelOwner {
        #[arg(long)]
        handle: Handle,
        #[arg(long)]
        owner: SigningPublic,
        #[arg(long)]
        secret_key: SigningSecret,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("xirtam_dirclient=trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let anchor_pk = args.public_key;

    let opts = SqliteConnectOptions::from_str(&args.db_path)?
        .create_if_missing(true)
        .shared_cache(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1) // IMPORTANT: keep at least one connection open
        .connect_with(opts)
        .await?;
    let transport = ReqwestTransport::new(reqwest::Client::new(), args.endpoint);
    let client = DirClient::new(transport, anchor_pk, pool).await?;

    match args.command {
        Command::Query { handle } => {
            let roothash = client.get_roothash(&handle).await?;
            match roothash {
                Some(hash) => println!("{}", hash),
                None => println!("<none>"),
            }
        }
        Command::Insert {
            handle,
            roothash,
            secret_key,
        } => {
            client
                .insert_roothash(&handle, roothash, &secret_key)
                .await?;
            println!("ok");
        }
        Command::AddOwner {
            handle,
            owner,
            secret_key,
        } => {
            client.add_owner(&handle, owner, &secret_key).await?;
            println!("ok");
        }
        Command::DelOwner {
            handle,
            owner,
            secret_key,
        } => {
            client.del_owner(&handle, owner, &secret_key).await?;
            println!("ok");
        }
    }
    Ok(())
}
