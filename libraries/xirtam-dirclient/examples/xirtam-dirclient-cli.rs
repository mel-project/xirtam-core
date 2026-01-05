use clap::Parser;
use nanorpc_reqwest::ReqwestTransport;
use tracing_subscriber::EnvFilter;
use xirtam_crypt::signing::SigningPublic;
use xirtam_dirclient::DirClient;

#[derive(Parser, Debug)]
#[command(name = "xirtam-dirclient-cli")]
struct Args {
    #[arg(long)]
    endpoint: String,
    #[arg(long)]
    public_key: String,
    #[arg(long, default_value = "sqlite://dirclient.db")]
    db_path: String,
    key: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("xirtam_dirclient=trace"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let args = Args::parse();
    let pk_bytes = hex::decode(args.public_key)?;
    if pk_bytes.len() != 32 {
        anyhow::bail!("public key must be 32 bytes hex");
    }
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&pk_bytes);
    let anchor_pk = SigningPublic::from_bytes(buf)?;

    let pool = sqlx::SqlitePool::connect(&args.db_path).await?;
    let transport = ReqwestTransport::new(reqwest::Client::new(), args.endpoint);
    let client = DirClient::new(transport, anchor_pk, pool).await?;

    let listing = client.query_raw(args.key).await?;
    println!("owners: {}", listing.owners.len());
    for owner in listing.owners {
        println!("owner: {}", hex::encode(owner.to_bytes()));
    }
    match listing.latest {
        Some(msg) => {
            println!("latest.kind: {}", msg.kind);
            println!("latest.bytes: {}", msg.inner.len());
        }
        None => println!("latest: <none>"),
    }
    Ok(())
}
