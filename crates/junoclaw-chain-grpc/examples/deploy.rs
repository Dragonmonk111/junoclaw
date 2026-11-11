//! deploy.rs — store + instantiate (+ optional transfer_admin) a wasm
//! contract on g1 using the shared gRPC client.
//!
//! Usage:
//!   JUNOCLAW_DEPLOY_MNEMONIC="..." cargo run -p junoclaw-chain-grpc \
//!     --example deploy -- \
//!     --wasm faucet.wasm --msg '{"drip_amount":"50000","denom":"ujclaw"}' \
//!     --label g1-faucet [--admin juno1...] [--transfer-admin juno1...]

use anyhow::Context;
use clap::Parser;
use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    wasm: String,
    /// Skip store; instantiate this existing code_id.
    #[arg(long)]
    code_id: Option<u64>,
    #[arg(long)]
    msg: String,
    #[arg(long)]
    label: String,
    /// Migration admin (defaults to deployer).
    #[arg(long)]
    admin: Option<String>,
    /// Transfer contract admin after instantiate.
    #[arg(long)]
    transfer_admin: Option<String>,
    #[arg(long, env = "JUNOCLAW_GRPC", default_value = "51.195.201.74:9090")]
    grpc: String,
    #[arg(long, env = "JUNOCLAW_CHAIN_ID", default_value = "junoclaw-g1")]
    chain_id: String,
    #[arg(long, env = "JUNOCLAW_GAS_PRICE", default_value = "0.025ujclaw")]
    gas_price: String,
    #[arg(long, default_value_t = 3_000_000)]
    gas_limit: u64,
    #[arg(long, default_value = "JUNOCLAW_DEPLOY_MNEMONIC")]
    mnemonic_env: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let wasm = std::fs::read(&args.wasm).context("read wasm")?;
    let msg: serde_json::Value = serde_json::from_str(&args.msg).context("parse msg")?;

    let mut client = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: args.grpc,
        chain_id: args.chain_id,
        gas_price: args.gas_price,
        gas_limit: args.gas_limit,
        mnemonic_env: Some(args.mnemonic_env),
        account_prefix: "junoclaw".into(),
        hd_path: "m/44'/118'/0'/0/0".into(),
    })
    .await?;
    println!("signer: {}", client.signer_address().unwrap_or_default());

    let code_id = match args.code_id {
        Some(id) => id,
        None => {
            let (id, out) = client.store_code(&wasm).await?;
            println!("stored code_id={id} tx={} height={}", out.tx_hash, out.height);
            id
        }
    };

    let (addr, out) = client
        .instantiate_contract(code_id, &msg, &args.label, args.admin.as_deref(), vec![])
        .await?;
    println!("instantiated contract={addr} tx={} height={}", out.tx_hash, out.height);

    if let Some(new_admin) = &args.transfer_admin {
        let out = client
            .execute_contract(&addr, &serde_json::json!({"transfer_admin": {"new_admin": new_admin}}))
            .await?;
        println!("admin transferred to {new_admin} tx={}", out.tx_hash);
    }
    Ok(())
}
