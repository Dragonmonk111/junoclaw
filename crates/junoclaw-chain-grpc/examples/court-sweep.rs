//! court-sweep — finalize matured thought-court attestations.
//!
//! Bonded-tier operator bonds stay locked until `finalize_attestation`
//! runs. This sweeps `status: pending` attestations whose
//! challenge_deadline has passed and finalizes them, returning bonds.
//!
//! Usage:
//!   JUNOCLAW_MNEMONIC="..." cargo run -p junoclaw-chain-grpc \
//!     --example court-sweep -- --contract juno1... [--dry-run]

use clap::Parser;
use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    contract: String,
    /// Print what would be finalized without sending txs.
    #[arg(long)]
    dry_run: bool,
    #[arg(long, env = "JUNOCLAW_GRPC", default_value = "51.195.201.74:9090")]
    grpc: String,
    #[arg(long, env = "JUNOCLAW_CHAIN_ID", default_value = "junoclaw-g1")]
    chain_id: String,
    #[arg(long, env = "JUNOCLAW_GAS_PRICE", default_value = "0.025ujclaw")]
    gas_price: String,
    #[arg(long, default_value_t = 400_000)]
    gas_limit: u64,
    #[arg(long, env = "MNEMONIC_ENV", default_value = "JUNOCLAW_MNEMONIC")]
    mnemonic_env: String,
}

#[derive(serde::Deserialize)]
struct Att {
    id: String,
    challenge_deadline: u64,
    #[allow(dead_code)]
    status: String,
}
#[derive(serde::Deserialize)]
struct ListResp {
    attestations: Vec<Att>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a = Args::parse();
    let mut c = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: a.grpc,
        chain_id: a.chain_id,
        gas_price: a.gas_price,
        gas_limit: a.gas_limit,
        mnemonic_env: if a.dry_run { None } else { Some(a.mnemonic_env) },
        account_prefix: "junoclaw".into(),
        hd_path: "m/44'/118'/0'/0/0".into(),
    })
    .await?;

    // Chain time — block time, not wall clock (timed ops follow blocks).
    let latest = c.latest().await?;
    let now = latest.timestamp_nanos / 1_000_000_000;

    let resp: ListResp = c
        .query_smart_typed(
            &a.contract,
            &serde_json::json!({"list_attestations": {"status": "pending", "limit": 200}}),
        )
        .await?;

    let matured: Vec<&Att> = resp
        .attestations
        .iter()
        .filter(|att| att.challenge_deadline <= now)
        .collect();
    println!(
        "chain_time={} pending={} matured={}",
        now,
        resp.attestations.len(),
        matured.len()
    );
    for att in matured {
        if a.dry_run {
            println!("would finalize {} (deadline {})", att.id, att.challenge_deadline);
            continue;
        }
        let out = c
            .execute_contract(
                &a.contract,
                &serde_json::json!({"finalize_attestation": {"attestation_id": att.id}}),
            )
            .await;
        match out {
            Ok(o) => println!("finalized {} tx={} h={}", att.id, o.tx_hash, o.height),
            Err(e) => println!("finalize {} FAILED: {}", att.id, e),
        }
    }
    Ok(())
}
