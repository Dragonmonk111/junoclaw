//! exec — sign + broadcast a CosmWasm execute message via g1 gRPC.
//!
//! Usage:
//!   JUNOCLAW_MNEMONIC=... cargo run -p junoclaw-chain-grpc --example exec -- \
//!     --grpc 51.195.201.74:9090 --contract juno1... --msg '{"verify_proof":{...}}'
//!
//! Skips simulation (static gas) — the mcp wallet path's auto-fee simulate
//! consumed the account sequence on g1, so this path is the reliable one.

use clap::Parser;
use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "51.195.201.74:9090")]
    grpc: String,
    #[arg(long, default_value = "junoclaw-g1")]
    chain_id: String,
    #[arg(long, default_value = "0.001ujclaw")]
    gas_price: String,
    #[arg(long, default_value_t = 400_000)]
    gas_limit: u64,
    #[arg(long, env = "MNEMONIC_ENV", default_value = "JUNOCLAW_MNEMONIC")]
    mnemonic_env: String,
    #[arg(long)]
    contract: String,
    #[arg(long)]
    msg: String,
    /// Added to the fetched account sequence (mempool-pending drift).
    #[arg(long, default_value_t = 0)]
    seq_offset: u64,
    /// Attach native funds: "ujclaw:100000" (repeatable).
    #[arg(long)]
    funds: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let a = Args::parse();
    let mut c = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: a.grpc,
        chain_id: a.chain_id,
        gas_price: a.gas_price,
        gas_limit: a.gas_limit,
        mnemonic_env: Some(a.mnemonic_env),
        account_prefix: "junoclaw".into(),
        hd_path: "m/44'/118'/0'/0/0".into(),
    })
    .await?;
    c.seq_offset = a.seq_offset;
    eprintln!("signer: {}", c.signer_address().unwrap_or_default());
    let msg: serde_json::Value = serde_json::from_str(&a.msg)?;
    let funds: Vec<(String, u128)> = a
        .funds
        .iter()
        .map(|f| {
            let (d, amt) = f.split_once(':').expect("funds format denom:amount");
            (d.to_string(), amt.parse().expect("funds amount"))
        })
        .collect();
    let out = c.execute_contract_funded(&a.contract, &msg, funds).await?;
    println!("tx={} height={}", out.tx_hash, out.height);
    Ok(())
}
