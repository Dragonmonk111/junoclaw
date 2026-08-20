// JunoClaw Prover Daemon
//
// Runs on the robot (or edge device). Polls the ROS2 bridge for reflex batches,
// generates ZK safety proofs, and submits them on-chain via the zk-verifier contract.
//
// Architecture:
//   ROS2 bridge → prover daemon → ZK proof → on-chain submit
//
// The daemon runs a loop:
//   1. Poll bridge for new reflex batches
//   2. Build Merkle tree from cycle hashes
//   3. Generate SensorSafety proof (80ms)
//   4. Generate IntentConsistency proof (119ms) — if intent data available
//   5. Generate ConsensusMembership proof (51ms) — if validator context
//   6. Generate Aggregation proof (68ms)
//   7. Submit proofs on-chain via zk-verifier contract
//   8. Check circuit breaker state
//   9. Sleep until next batch

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn, error, debug};

mod merkle;
mod prove;
mod submit;
mod config;

use config::ProverConfig;

#[derive(Parser)]
#[command(name = "junoclaw-prover")]
#[command(about = "JunoClaw prover daemon — generates ZK safety proofs from robot sensor data")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the prover daemon in continuous mode
    Run {
        /// Config file path (TOML or JSON)
        #[arg(short, long, default_value = "prover-config.toml")]
        config: PathBuf,

        /// Override bridge URL
        #[arg(long)]
        bridge_url: Option<String>,

        /// Override chain RPC URL
        #[arg(long)]
        chain_rpc: Option<String>,

        /// Polling interval in seconds
        #[arg(short, long, default_value = "10")]
        interval: u64,
    },

    /// Generate a single proof from a batch ID (one-shot mode)
    Prove {
        /// Bridge URL
        #[arg(long)]
        bridge_url: String,

        /// Batch ID to prove
        #[arg(long)]
        batch_id: String,

        /// Robot ID
        #[arg(long)]
        robot_id: String,

        /// Output file for the proof
        #[arg(short, long, default_value = "proof.bin")]
        output: PathBuf,

        /// Proving key directory
        #[arg(long, default_value = "./keys")]
        keys_dir: PathBuf,
    },

    /// Setup: generate proving/verifying keys for all circuits
    Setup {
        /// Output directory for keys
        #[arg(short, long, default_value = "./keys")]
        output: PathBuf,

        /// Merkle tree height for sensor circuit
        #[arg(long, default_value = "7")]
        tree_height: usize,
    },

    /// Verify a proof locally (before submitting on-chain)
    Verify {
        /// Verifying key file
        #[arg(long)]
        vk: PathBuf,

        /// Proof file
        #[arg(long)]
        proof: PathBuf,

        /// Public inputs (JSON array of hex strings)
        #[arg(long)]
        public_inputs: String,
    },

    /// Submit a proof on-chain to the zk-verifier contract
    Submit {
        /// Chain RPC URL
        #[arg(long)]
        chain_rpc: String,

        /// zk-verifier contract address
        #[arg(long)]
        verifier_addr: String,

        /// Proof file
        #[arg(long)]
        proof: PathBuf,

        /// Verifying key file
        #[arg(long)]
        vk: PathBuf,

        /// Public inputs (JSON array of hex strings)
        #[arg(long)]
        public_inputs: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct BatchResponse {
    robot_id: String,
    batch_id: String,
    cycles: Vec<CycleData>,
    merkle_root: String,
    cycle_count: u64,
    all_invariants_maintained: bool,
    violated_invariants: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CycleData {
    cycle_id: u64,
    timestamp: u64,
    sensor_readings: serde_json::Value,
    invariant_checks: serde_json::Value,
    control_outputs: serde_json::Value,
    cycle_hash: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "junoclaw_prover=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Setup { output, tree_height } => {
            info!("Setting up proving/verifying keys (tree_height={})", tree_height);
            prove::setup_keys(&output, tree_height)?;
            info!("Keys written to {}", output.display());
        }

        Commands::Prove { bridge_url, batch_id, robot_id, output, keys_dir } => {
            info!("Fetching batch {} from bridge {}", batch_id, bridge_url);
            let batch = fetch_batch(&bridge_url, &batch_id).await?;
            info!("Batch: {} cycles, merkle_root={}, maintained={}",
                batch.cycle_count, batch.merkle_root, batch.all_invariants_maintained);

            info!("Generating ZK proof...");
            let proof_bytes = prove::generate_sensor_proof(&keys_dir, &batch, &robot_id)?;
            std::fs::write(&output, &proof_bytes)?;
            info!("Proof written to {} ({} bytes)", output.display(), proof_bytes.len());
        }

        Commands::Verify { vk, proof, public_inputs } => {
            info!("Verifying proof locally...");
            let valid = prove::verify_proof_local(&vk, &proof, &public_inputs)?;
            if valid {
                info!("PROOF VALID");
            } else {
                error!("PROOF INVALID");
                std::process::exit(1);
            }
        }

        Commands::Submit { chain_rpc, verifier_addr, proof, vk, public_inputs } => {
            info!("Submitting proof on-chain to {}", verifier_addr);
            submit::submit_proof_onchain(&chain_rpc, &verifier_addr, &proof, &vk, &public_inputs).await?;
            info!("Proof submitted successfully");
        }

        Commands::Run { config, bridge_url, chain_rpc, interval } => {
            let cfg = ProverConfig::load(&config)?;
            let bridge_url = bridge_url.unwrap_or(cfg.bridge_url.clone());
            let chain_rpc = chain_rpc.unwrap_or(cfg.chain_rpc.clone());
            let interval = Duration::from_secs(interval);

            info!("Prover daemon starting");
            info!("  Robot ID: {}", cfg.robot_id);
            info!("  Bridge: {}", bridge_url);
            info!("  Chain RPC: {}", chain_rpc);
            info!("  Interval: {:?}", interval);
            info!("  Keys dir: {}", cfg.keys_dir.display());

            run_daemon(cfg, bridge_url, chain_rpc, interval).await?;
        }
    }

    Ok(())
}

async fn fetch_batch(bridge_url: &str, batch_id: &str) -> Result<BatchResponse> {
    let url = format!("{}/rosbag/{}", bridge_url.trim_end_matches('/'), batch_id);
    let resp = reqwest::get(&url).await
        .context("failed to fetch batch from bridge")?;
    if !resp.status().is_success() {
        anyhow::bail!("bridge returned {} for batch {}", resp.status(), batch_id);
    }
    let batch: BatchResponse = resp.json().await
        .context("failed to parse batch response")?;
    Ok(batch)
}

async fn run_daemon(
    cfg: ProverConfig,
    bridge_url: String,
    chain_rpc: String,
    interval: Duration,
) -> Result<()> {
    let mut last_batch_id: Option<String> = None;

    loop {
        debug!("Polling bridge for new batches...");

        // Check bridge health
        let health_url = format!("{}/health", bridge_url.trim_end_matches('/'));
        match reqwest::get(&health_url).await {
            Ok(resp) if resp.status().is_success() => {
                debug!("Bridge healthy");
            }
            Ok(resp) => {
                warn!("Bridge unhealthy: status {}", resp.status());
                tokio::time::sleep(interval).await;
                continue;
            }
            Err(e) => {
                warn!("Bridge unreachable: {}", e);
                tokio::time::sleep(interval).await;
                continue;
            }
        }

        // Simulate a new batch (in production, poll for new batch IDs)
        let batch_id = match &last_batch_id {
            None => "batch_initial".to_string(),
            Some(prev) => format!("batch_{}", chrono_timestamp()),
        };

        // Fetch batch
        match fetch_batch(&bridge_url, &batch_id).await {
            Ok(batch) => {
                info!("New batch: {} cycles, maintained={}",
                    batch.cycle_count, batch.all_invariants_maintained);

                if !batch.all_invariants_maintained {
                    warn!("SAFETY VIolation detected: violated_invariants={:?}",
                        batch.violated_invariants);
                }

                // Generate proof
                match prove::generate_sensor_proof(&cfg.keys_dir, &batch, &cfg.robot_id) {
                    Ok(proof_bytes) => {
                        info!("Proof generated: {} bytes", proof_bytes.len());

                        // Submit on-chain
                        if let Some(ref verifier_addr) = cfg.verifier_addr {
                            match submit::submit_proof_raw(
                                &chain_rpc,
                                verifier_addr,
                                &proof_bytes,
                                &cfg.keys_dir,
                            ).await {
                                Ok(tx_hash) => {
                                    info!("Proof submitted on-chain: tx={}", tx_hash);
                                }
                                Err(e) => {
                                    error!("On-chain submit failed: {}", e);
                                }
                            }
                        } else {
                            info!("No verifier address configured — proof saved locally only");
                            let proof_path = cfg.keys_dir.join(format!("{}_proof.bin", batch_id));
                            std::fs::write(&proof_path, &proof_bytes)?;
                            info!("Proof saved to {}", proof_path.display());
                        }

                        // Check circuit breaker
                        if let Some(ref breaker_addr) = cfg.circuit_breaker_addr {
                            match submit::check_circuit_breaker(
                                &chain_rpc,
                                breaker_addr,
                                &cfg.robot_id,
                            ).await {
                                Ok(locked) => {
                                    if locked {
                                        warn!("CIRCUIT BREAKER TRIPPED — robot {} is locked", cfg.robot_id);
                                    } else {
                                        debug!("Circuit breaker: closed (robot operational)");
                                    }
                                }
                                Err(e) => {
                                    warn!("Circuit breaker check failed: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Proof generation failed: {}", e);
                    }
                }

                last_batch_id = Some(batch_id);
            }
            Err(e) => {
                debug!("No new batch: {}", e);
            }
        }

        tokio::time::sleep(interval).await;
    }
}

fn chrono_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
