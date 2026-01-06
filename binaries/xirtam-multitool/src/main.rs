use clap::{Parser, Subcommand};
use serde::Serialize;
use xirtam_crypt::{
    dh::{DhPublic, DhSecret},
    signing::{SigningPublic, SigningSecret},
};

#[derive(Parser)]
#[command(name = "xirtam-multitool")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Keygen,
}

#[derive(Serialize)]
struct Output {
    signing: SigningKeyPair,
    dh: DhKeyPair,
}

#[derive(Serialize)]
struct SigningKeyPair {
    public: SigningPublic,
    secret: SigningSecret,
}

#[derive(Serialize)]
struct DhKeyPair {
    public: DhPublic,
    secret: DhSecret,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Keygen => {
            let signing_secret = SigningSecret::random();
            let dh_secret = DhSecret::random();
            let output = Output {
                signing: SigningKeyPair {
                    public: signing_secret.public_key(),
                    secret: signing_secret,
                },
                dh: DhKeyPair {
                    public: dh_secret.public_key(),
                    secret: dh_secret,
                },
            };
            let yaml = serde_yml::to_string(&output)?;
            print!("{yaml}");
        }
    }
    Ok(())
}
