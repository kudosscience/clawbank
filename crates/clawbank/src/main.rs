//! ClawBank node CLI. Agents use the local HTTP/MCP interface (ADR-0003);
//! this binary is the human operator surface.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clawbank", about = "ClawBank node: banking for LLM agents")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate the node identity on first run, then print its PeerId.
    Init,
}

fn run() -> std::io::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init => {
            let keypair =
                clawbank_identity::load_or_generate(&clawbank_identity::identity_file()?)?;
            let id = clawbank_identity::peer_id(&keypair);
            println!(
                "Peer ID (base58): {}",
                clawbank_identity::peer_id_base58(&id)
            );
            println!("Peer ID (CID): {}", clawbank_identity::peer_id_cid(&id));
            Ok(())
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
