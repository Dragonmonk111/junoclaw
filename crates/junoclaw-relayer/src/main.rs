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
mod breaker;
mod executor;
mod market;
mod moult;
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

        /// Optional moultbook-v0 contract address. When set, every settled
        /// batch also gets a moultbook entry (semantic on-chain index).
        #[arg(long)]
        moultbook: Option<String>,

        /// Topic namespace for moultbook entries (e.g. "pipeline-A12").
        /// Required when --moultbook is set.
        #[arg(long, requires = "moultbook")]
        topic: Option<String>,

        /// Enable Layer 5 (Execution Bridge): submit extracted tasks
        /// from settled batches to the task-ledger contract.
        #[arg(long)]
        execute: bool,

        /// Task-ledger contract address on Juno.
        /// Required when --execute is set.
        #[arg(long, requires = "execute")]
        task_ledger: Option<String>,

        /// Agent-registry contract address (for agent validation in executor).
        /// Optional when --execute is set.
        #[arg(long)]
        agent_registry: Option<String>,

        /// Optional truth-market contract address. When set, the relayer
        /// finalizes eval epochs after each batch settlement (Layer 6).
        #[arg(long)]
        truth_market: Option<String>,

        /// Per-batch verification fee to pay to the truth market (in ujuno).
        /// When >0, the relayer calls PayVerificationFee before finalizing
        /// each epoch, routing the fee into the reward pool.
        /// Only used when --truth-market is set. Default: 0 (skip fee).
        #[arg(long)]
        verification_fee: Option<u128>,

        /// Optional circuit-breaker contract address. When set, the relayer
        /// submits TripBreaker transactions for any BreakerActions detected
        /// in finalized batches.
        #[arg(long)]
        breaker: Option<String>,
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
            moultbook,
            topic,
            execute,
            task_ledger,
            agent_registry,
            truth_market,
            verification_fee,
            breaker,
        } => {
            info!("Starting JunoClaw relayer daemon");
            info!("  RPC: {}", rpc);
            info!("  Contract: {}", contract);
            info!("  Coordination endpoint: {}", coordination_endpoint);
            info!("  Poll interval: {}s", poll_interval);

            let moult = moultbook.map(|addr| {
                let namespace = topic
                    .expect("--topic is required when --moultbook is set");
                info!(
                    "  Moultbook: {} (topic: {})",
                    addr,
                    moult::topic_hash(&namespace)
                );
                moult::MoultConfig {
                    moultbook_addr: addr,
                    topic_namespace: namespace,
                }
            });

            let executor_cfg = if execute {
                let tl = task_ledger
                    .expect("--task-ledger is required when --execute is set");
                info!("  Executor: enabled (task-ledger: {})", tl);
                Some(executor::ExecutorConfig {
                    task_ledger_addr: tl,
                    agent_registry_addr: agent_registry
                        .unwrap_or_default(),
                    enabled: true,
                })
            } else {
                None
            };

            let market_cfg = truth_market.map(|addr| {
                let fee = verification_fee.unwrap_or(0);
                if fee > 0 {
                    info!("  Truth Market: {} (verification fee: {} ujuno)", addr, fee);
                } else {
                    info!("  Truth Market: {} (no verification fee)", addr);
                }
                market::MarketConfig {
                    truth_market_addr: addr,
                    verification_fee: fee,
                    robot_id: None,
                }
            });

            let breaker_cfg = breaker.map(|addr| {
                info!("  Circuit Breaker: {}", addr);
                breaker::BreakerConfig {
                    breaker_addr: addr,
                }
            });

            let config = watcher::WatcherConfig {
                rpc_endpoint: rpc,
                contract_addr: contract,
                relayer_key: key,
                coordination_endpoint,
                poll_interval_secs: poll_interval,
                moult,
                executor: executor_cfg,
                market: market_cfg,
                breaker: breaker_cfg,
            };

            let watcher = watcher::BatchWatcher::new(config);
            watcher.run().await?;
        }
    }

    Ok(())
}
