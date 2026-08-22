//! junoclaw-miner — truth market miner CLI
//!
//! Commands:
//!   register   — register as a truth market operator (stake + fingerprint)
//!   run        — start the mining loop (watch batches, evaluate, submit verdicts)
//!   status     — show miner status (verdicts, rewards, slashing)
//!   unstake    — request unstake and withdraw from truth market
//!   identity   — show or create miner identity

use std::sync::Arc;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use tracing::{info, warn};

use junoclaw_miner::{
    evaluator::{OpenWeightEvaluator, RuleBasedEvaluator, TruthEvaluator},
    identity::MinerIdentity,
    miner::{Miner, MinerConfig},
};

#[derive(Parser)]
#[command(name = "junoclaw-miner")]
#[command(about = "Truth market miner — evaluate robot decisions, earn rewards")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Coordination REST API endpoint
    #[arg(long, env = "JUNOCLAW_COORDINATION_API", default_value = "http://localhost:8080")]
    coordination_api: String,

    /// Juno RPC endpoint
    #[arg(long, env = "JUNO_RPC", default_value = "https://juno.rpc.t.stavr.tech")]
    juno_rpc: String,

    /// Juno REST/LCD endpoint
    #[arg(long, env = "JUNO_REST", default_value = "https://juno-testnet-api.cogwheel.zone")]
    juno_rest: String,

    /// Truth market contract address
    #[arg(long, env = "TRUTH_MARKET_CONTRACT",
          default_value = "juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p")]
    truth_market_contract: String,

    /// Miner Juno wallet address
    #[arg(long, env = "MINER_ADDRESS")]
    address: Option<String>,

    /// Miner wallet mnemonic (for signing transactions)
    #[arg(long, env = "MINER_MNEMONIC")]
    mnemonic: Option<String>,

    /// Enable on-chain submission (default: dry run)
    #[arg(long, env = "SUBMIT_ON_CHAIN")]
    submit_on_chain: bool,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Register as a truth market operator
    Register {
        /// Stake amount in ujunox
        #[arg(long, default_value = "1000000")]
        stake: u128,

        /// Model identifier (e.g., "llama-70b", "qwen-3b", "rule-v1")
        #[arg(long, env = "MINER_MODEL")]
        model: String,

        /// Hardware identifier (e.g., "jetson-orin", "dgx-spark", "cloud")
        #[arg(long, env = "MINER_HARDWARE", default_value = "any")]
        hardware: String,

        /// Identity type: robot, gpu, cloud
        #[arg(long, env = "MINER_IDENTITY_TYPE", default_value = "gpu")]
        identity_type: String,

        /// Optional jclaw-credential token ID (for robot miners)
        #[arg(long)]
        credential_token_id: Option<u64>,
    },

    /// Start the mining loop
    Run {
        /// Evaluator type: rule, llm, local
        #[arg(long, env = "EVALUATOR_TYPE", default_value = "rule")]
        evaluator: String,

        /// LLM API endpoint (for llm/local evaluators)
        #[arg(long, env = "LLM_ENDPOINT")]
        llm_endpoint: Option<String>,

        /// LLM API key (for Akash TEE authenticated endpoints)
        #[arg(long, env = "LLM_API_KEY")]
        llm_api_key: Option<String>,

        /// Open-weight model name (e.g., "qwen-3b", "llama-70b", "mistral-8x22b")
        #[arg(long, env = "LLM_MODEL", default_value = "qwen-3b")]
        llm_model: String,

        /// Polling interval in seconds
        #[arg(long, env = "POLL_INTERVAL_SECS", default_value = "10")]
        poll_interval: u64,

        /// Model identifier for fingerprinting
        #[arg(long, env = "MINER_MODEL", default_value = "rule-v1")]
        model: String,

        /// Hardware identifier for fingerprinting
        #[arg(long, env = "MINER_HARDWARE", default_value = "any")]
        hardware: String,
    },

    /// Show miner status
    Status,

    /// Request unstake from truth market
    Unstake,

    /// Withdraw unstaked funds after cooldown
    Withdraw,

    /// Deposit funds into the reward pool
    DepositRewards {
        /// Amount to deposit in ujunox
        #[arg(long)]
        amount: u128,
    },

    /// Show or create miner identity
    Identity {
        /// Model identifier
        #[arg(long, env = "MINER_MODEL", default_value = "rule-v1")]
        model: String,

        /// Hardware identifier
        #[arg(long, env = "MINER_HARDWARE", default_value = "any")]
        hardware: String,

        /// Identity type: robot, gpu, akash-tee
        #[arg(long, env = "MINER_IDENTITY_TYPE", default_value = "gpu")]
        identity_type: String,

        /// Optional jclaw-credential token ID (for robot miners)
        #[arg(long)]
        credential_token_id: Option<u64>,

        /// Optional TEE attestation hash (for Akash TEE miners)
        #[arg(long)]
        tee_attestation: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = MinerConfig {
        coordination_api: cli.coordination_api,
        juno_rpc: cli.juno_rpc,
        juno_rest: cli.juno_rest,
        truth_market_contract: cli.truth_market_contract,
        mnemonic: cli.mnemonic,
        submit_on_chain: cli.submit_on_chain,
        ..Default::default()
    };

    match cli.command {
        Commands::Register {
            stake,
            model,
            hardware,
            identity_type,
            credential_token_id,
        } => {
            let address = cli.address.expect("--address or MINER_ADDRESS is required for registration");
            let identity = build_identity(&address, &model, &hardware, &identity_type, credential_token_id, None);
            let fingerprint = identity.fingerprint_hash();

            println!("═══ Register Truth Market Operator ═══");
            println!("Address:     {}", identity.address);
            println!("Type:        {:?}", identity.identity_type);
            println!("Model:       {}", identity.model_id);
            println!("Hardware:    {}", identity.hardware_id);
            println!("Fingerprint: {}", fingerprint);
            println!("Stake:       {} ujunox", stake);
            println!("Contract:    {}", config.truth_market_contract);
            println!();

            if config.submit_on_chain {
                let wallet_id = config.mnemonic.as_deref().expect("--mnemonic or MINER_MNEMONIC (wallet ID) is required for on-chain submission");
                println!("Submitting registration on-chain via wallet '{}'...", wallet_id);

                let msg = serde_json::json!({
                    "register_operator": {
                        "fingerprint": fingerprint,
                    }
                });
                let msg_json = serde_json::to_string(&msg)?;
                let funds_str = format!("{}ujunox", stake);

                let mcp_path = std::env::var("MCP_CLI_PATH")
                    .unwrap_or_else(|_| "node".to_string());

                let output = tokio::process::Command::new(&mcp_path)
                    .arg("mcp/dist/index.js")
                    .arg("wallet")
                    .arg("exec")
                    .arg(wallet_id)
                    .arg(&config.truth_market_contract)
                    .arg(&msg_json)
                    .arg("--rpc")
                    .arg(&config.juno_rpc)
                    .arg("--funds")
                    .arg(&funds_str)
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        println!("✓ Registration submitted: {}", stdout.trim());
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!("✗ Registration failed: {}", stderr.trim());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to spawn cosmos-mcp CLI: {}", e);
                    }
                }
            } else {
                println!("Dry run — set --submit-on-chain to register on-chain");
                println!("Message: {{\"register_operator\": {{\"fingerprint\": \"{}\"}}}}", fingerprint);
                println!("Funds:  {}ujunox", stake);
            }
        }

        Commands::Run {
            evaluator,
            llm_endpoint,
            llm_api_key,
            llm_model,
            poll_interval,
            model,
            hardware,
        } => {
            let address = cli.address.unwrap_or_else(|| {
                "juno1unregistered0000000000000000000000000000".to_string()
            });

            let identity = build_identity(&address, &model, &hardware, "gpu", None, None);

            let eval: Arc<dyn TruthEvaluator> = match evaluator.as_str() {
                "rule" => {
                    info!("using rule-based evaluator");
                    Arc::new(RuleBasedEvaluator::new())
                }
                "local" | "open-weight" => {
                    let endpoint = llm_endpoint.unwrap_or_else(|| "http://localhost:11434".to_string());
                    let eval = OpenWeightEvaluator::local(&endpoint, &llm_model, &hardware);
                    info!(model = %llm_model, endpoint = %endpoint, "using local open-weight evaluator");
                    Arc::new(eval)
                }
                "akash-tee" => {
                    let endpoint = llm_endpoint.expect("--llm-endpoint required for akash-tee evaluator");
                    let key = llm_api_key.expect("--llm-api-key required for akash-tee evaluator");
                    let eval = OpenWeightEvaluator::akash_tee(&endpoint, &key, &llm_model, &hardware);
                    info!(model = %llm_model, endpoint = %endpoint, "using Akash TEE open-weight evaluator");
                    Arc::new(eval)
                }
                other => {
                    anyhow::bail!("unknown evaluator type: {other} (use: rule, local, akash-tee)");
                }
            };

            let mut miner = Miner::new(
                MinerConfig {
                    poll_interval_secs: poll_interval,
                    ..config
                },
                identity,
                eval,
            );

            miner.run().await?;
        }

        Commands::Status => {
            println!("═══ Miner Status ═══");
            println!("Coordination API: {}", config.coordination_api);
            println!("Juno RPC:         {}", config.juno_rpc);
            println!("Truth Market:     {}", config.truth_market_contract);
            println!("Submit on-chain:  {}", config.submit_on_chain);
            println!();
            println!("Run `junoclaw-miner run` to start mining.");
        }

        Commands::Unstake => {
            let address = cli.address.expect("--address or MINER_ADDRESS is required");
            println!("═══ Request Unstake ═══");
            println!("Address: {}", address);
            if config.submit_on_chain {
                let wallet_id = config.mnemonic.as_deref().expect("--mnemonic or MINER_MNEMONIC (wallet ID) is required");
                let msg = serde_json::json!({"request_unstake": {}});
                let msg_json = serde_json::to_string(&msg)?;

                let mcp_path = std::env::var("MCP_CLI_PATH")
                    .unwrap_or_else(|_| "node".to_string());

                let output = tokio::process::Command::new(&mcp_path)
                    .arg("mcp/dist/index.js")
                    .arg("wallet")
                    .arg("exec")
                    .arg(wallet_id)
                    .arg(&config.truth_market_contract)
                    .arg(&msg_json)
                    .arg("--rpc")
                    .arg(&config.juno_rpc)
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        println!("✓ Unstake request submitted: {}", stdout.trim());
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!("✗ Unstake failed: {}", stderr.trim());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to spawn cosmos-mcp CLI: {}", e);
                    }
                }
            } else {
                println!("Dry run — set --submit-on-chain to submit on-chain");
            }
        }

        Commands::Withdraw => {
            let address = cli.address.expect("--address or MINER_ADDRESS is required");
            println!("═══ Withdraw Unstaked Funds ═══");
            println!("Address: {}", address);
            if config.submit_on_chain {
                let wallet_id = config.mnemonic.as_deref().expect("--mnemonic or MINER_MNEMONIC (wallet ID) is required");
                let msg = serde_json::json!({"withdraw_unstake": {}});
                let msg_json = serde_json::to_string(&msg)?;

                let mcp_path = std::env::var("MCP_CLI_PATH")
                    .unwrap_or_else(|_| "node".to_string());

                let output = tokio::process::Command::new(&mcp_path)
                    .arg("mcp/dist/index.js")
                    .arg("wallet")
                    .arg("exec")
                    .arg(wallet_id)
                    .arg(&config.truth_market_contract)
                    .arg(&msg_json)
                    .arg("--rpc")
                    .arg(&config.juno_rpc)
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        println!("✓ Withdrawal submitted: {}", stdout.trim());
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!("✗ Withdraw failed: {}", stderr.trim());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to spawn cosmos-mcp CLI: {}", e);
                    }
                }
            } else {
                println!("Dry run — set --submit-on-chain to submit on-chain");
            }
        }

        Commands::DepositRewards { amount } => {
            println!("═══ Deposit Rewards ═══");
            println!("Amount:   {} ujunox", amount);
            println!("Contract: {}", config.truth_market_contract);
            if config.submit_on_chain {
                let wallet_id = config.mnemonic.as_deref().expect("--mnemonic or MINER_MNEMONIC (wallet ID) is required");
                let msg = serde_json::json!({"deposit_rewards": {}});
                let msg_json = serde_json::to_string(&msg)?;
                let funds_str = format!("{}ujunox", amount);

                let mcp_path = std::env::var("MCP_CLI_PATH")
                    .unwrap_or_else(|_| "node".to_string());

                let output = tokio::process::Command::new(&mcp_path)
                    .arg("mcp/dist/index.js")
                    .arg("wallet")
                    .arg("exec")
                    .arg(wallet_id)
                    .arg(&config.truth_market_contract)
                    .arg(&msg_json)
                    .arg("--rpc")
                    .arg(&config.juno_rpc)
                    .arg("--funds")
                    .arg(&funds_str)
                    .output()
                    .await;

                match output {
                    Ok(out) if out.status.success() => {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        println!("✓ Rewards deposited: {}", stdout.trim());
                    }
                    Ok(out) => {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        eprintln!("✗ Deposit failed: {}", stderr.trim());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to spawn cosmos-mcp CLI: {}", e);
                    }
                }
            } else {
                println!("Dry run — set --submit-on-chain to deposit on-chain");
            }
        }

        Commands::Identity {
            model,
            hardware,
            identity_type,
            credential_token_id,
            tee_attestation,
        } => {
            let address = cli.address.unwrap_or_else(|| {
                "juno1unregistered0000000000000000000000000000".to_string()
            });
            let identity = build_identity(&address, &model, &hardware, &identity_type, credential_token_id, tee_attestation);

            println!("═══ Miner Identity ═══");
            println!("Address:        {}", identity.address);
            println!("Type:           {:?}", identity.identity_type);
            println!("Weight type:    {:?}", identity.weight_type);
            println!("Model:          {}", identity.model_id);
            println!("Hardware:       {}", identity.hardware_id);
            if let Some(ref token) = identity.credential_token_id {
                println!("Credential ID:  {}", token);
            }
            if let Some(ref attestation) = identity.tee_attestation {
                println!("TEE Attestation: {}", attestation);
            }
            println!("Verifiable:     {}", if identity.is_verifiable() { "yes" } else { "no" });
            println!("Fingerprint:    {}", identity.fingerprint_hash());
            println!("Description:    {}", identity.description());
        }
    }

    Ok(())
}

fn build_identity(
    address: &str,
    model: &str,
    hardware: &str,
    identity_type: &str,
    credential_token_id: Option<u64>,
    tee_attestation: Option<String>,
) -> MinerIdentity {
    match identity_type.to_lowercase().as_str() {
        "robot" => MinerIdentity::robot(address, model, hardware, credential_token_id),
        "gpu" => MinerIdentity::gpu_miner(address, model, hardware),
        "akash-tee" | "akash" | "tee" => {
            let attestation = tee_attestation.unwrap_or_else(|| "unverified".to_string());
            MinerIdentity::akash_tee_miner(address, model, hardware, &attestation)
        }
        other => {
            warn!("unknown identity type: {other}, defaulting to gpu");
            MinerIdentity::gpu_miner(address, model, hardware)
        }
    }
}
