//! tx-events — print decoded events for a tx hash (read-only, no signer).
//!
//! Usage:
//!   cargo run -p junoclaw-chain-grpc --example tx-events -- <TXHASH>

use junoclaw_chain_grpc::{ClientConfig, GrpcClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let hash = std::env::args().nth(1).expect("usage: tx-events <TXHASH>");
    let mut c = GrpcClient::connect(&ClientConfig {
        grpc_endpoint: "51.195.201.74:9090".into(),
        chain_id: "junoclaw-g1".into(),
        gas_price: "0.025ujclaw".into(),
        gas_limit: 400_000,
        mnemonic_env: None,
        account_prefix: "juno".into(),
        hd_path: "m/44'/118'/0'/0/0".into(),
    })
    .await?;

    let resp = c.get_tx(&hash).await?;
    if let Some(tr) = resp.tx_response {
        println!("code={} height={} raw_log={}", tr.code, tr.height, tr.raw_log);
        for ev in tr.events {
            for attr in ev.attributes {
                println!("{}: {}={}", ev.r#type, attr.key, attr.value);
            }
        }
    } else {
        println!("no tx_response");
    }
    Ok(())
}
