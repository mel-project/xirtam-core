use std::str::FromStr;

use clap::{Parser, Subcommand};
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
use xirtam_nanorpc::Transport;
use xirtam_structs::gateway::GatewayName;
use xirtam_structs::handle::{Handle, HandleDescriptor};
use url::Url;

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
        gateway_name: GatewayName,
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
    let endpoint = Url::parse(&args.endpoint)?;
    let transport = Transport::new(endpoint);
    let client = DirClient::new(transport, anchor_pk, pool).await?;

    match args.command {
        Command::Query { handle } => {
            let descriptor = client.get_handle_descriptor(&handle).await?;
            match descriptor {
                Some(descriptor) => {
                    println!(
                        "{} {}",
                        descriptor.gateway_name, descriptor.root_cert_hash
                    );
                }
                None => println!("<none>"),
            }
        }
        Command::Insert {
            handle,
            gateway_name,
            roothash,
            secret_key,
        } => {
            let descriptor = HandleDescriptor {
                gateway_name,
                root_cert_hash: roothash,
            };
            client
                .insert_handle_descriptor(&handle, &descriptor, &secret_key)
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
