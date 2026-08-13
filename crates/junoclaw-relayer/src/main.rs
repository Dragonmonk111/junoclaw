//! JunoClaw Relayer Daemon
//!
//! Bridges Commonware coordination network certificates to the
//! coordination-settler CosmWasm contract on Juno.
//!
//! Flow:
//! 1. Listen for finalized batches from the coordination network
//! 2. Package certificate + messages_hash into a SubmitBatch tx
//! 3. Submit to the coordination-settler contract on Juno
//! 4. Confirm settlement on-chain

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;

mod bridge;
mod watcher;

#[derive(Parser)]
#[command(name = "junoclaw-relayer")]
#[command(about = "Relayer daemon for Commonware → Juno settlement bridge")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the relayer daemon
    Run {
        /// Juno RPC endpoint (e.g. https://juno-rpc.polkachu.com:443)
        #[arg(long, default_value = "https://juno-rpc.polkachu.com:443")]
        rpc: String,

        /// Coordination-settler contract address on Juno
        #[arg(long)]
        contract: String,

        /// Relayer wallet keyfile or mnemonic (env: RELAYER_KEY)
        #[arg(long, env = "RELAYER_KEY")]
        key: String,

        /// Commonware coordination node endpoint to watch for finalized batches
        #[arg(long, default_value = "http://127.0.0.1:4001")]
        coordination_endpoint: String,

        /// Poll interval in seconds
        #[arg(long, default_value_t = 5)]
        poll_interval: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            rpc,
            contract,
            key,
            coordination_endpoint,
            poll_interval,
        } => {
            info!("Starting JunoClaw relayer daemon");
            info!("  RPC: {}", rpc);
            info!("  Contract: {}", contract);
            info!("  Coordination endpoint: {}", coordination_endpoint);
            info!("  Poll interval: {}s", poll_interval);

            let config = watcher::WatcherConfig {
                rpc_endpoint: rpc,
                contract_addr: contract,
                relayer_key: key,
                coordination_endpoint,
                poll_interval_secs: poll_interval,
            };

            let watcher = watcher::BatchWatcher::new(config);
            watcher.run().await?;
        }
    }

    Ok(())
}
