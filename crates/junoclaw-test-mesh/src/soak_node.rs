//! Soak-test node — a real commonware-p2p coordination node process.
//!
//! Each invocation of this binary is one node in the 4-node soak-test mesh.
//! It joins the authenticated `lookup` P2P mesh, broadcasts a heartbeat
//! message on a fixed interval, and logs every message it receives.
//!
//! Usage:
//!
//!   # Step 1: bootstrap — print each node's deterministic public key
//!   soak-node --seed 1 --print-pubkey
//!   soak-node --seed 2 --print-pubkey
//!   soak-node --seed 3 --print-pubkey
//!   soak-node --seed 4 --print-pubkey
//!
//!   # Step 2: run the real mesh (one process per node, same host or LAN)
//!   soak-node --label node1 --seed 1 --listen-addr 127.0.0.1:4001 \
//!       --peer <pk2_hex>@127.0.0.1:4002 \
//!       --peer <pk3_hex>@127.0.0.1:4003 \
//!       --peer <pk4_hex>@127.0.0.1:4004
//!
//! Build with: cargo build --release --features p2p -p junoclaw-test-mesh

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Parser;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

use junoclaw_coordination::{AgentMessage, CoordinationConfig, CoordinationNode};

#[derive(Parser, Debug)]
#[command(name = "soak-node", about = "Real P2P soak-test node for junoclaw-coordination")]
struct Args {
    /// Human-readable label for log lines (default derived from seed)
    #[arg(long, default_value = "")]
    label: String,

    /// Deterministic identity seed (must be unique per node in the mesh)
    #[arg(long)]
    seed: u64,

    /// This node's listen address, e.g. 127.0.0.1:4001
    #[arg(long, default_value = "127.0.0.1:4001")]
    listen_addr: String,

    /// Peer spec: PUBKEY_HEX@HOST:PORT — repeat once per other peer
    #[arg(long = "peer")]
    peers: Vec<String>,

    /// P2P namespace for cryptographic domain separation
    #[arg(long, default_value = "junoclaw-soak-v1")]
    namespace: String,

    /// Seconds between heartbeat broadcasts
    #[arg(long, default_value_t = 10)]
    heartbeat_secs: u64,

    /// Only derive and print this node's public key (hex), then exit.
    /// Used to bootstrap the peer list before starting the real mesh.
    #[arg(long, default_value_t = false)]
    print_pubkey: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn parse_peers(specs: &[String]) -> Result<Vec<(String, String)>> {
    let mut peers = Vec::with_capacity(specs.len());
    for spec in specs {
        let parts: Vec<&str> = spec.splitn(2, '@').collect();
        if parts.len() != 2 {
            anyhow::bail!(
                "invalid --peer spec '{}', expected PUBKEY_HEX@HOST:PORT",
                spec
            );
        }
        peers.push((parts[0].to_string(), parts[1].to_string()));
    }
    Ok(peers)
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    let label = if args.label.is_empty() {
        format!("node-seed{}", args.seed)
    } else {
        args.label.clone()
    };

    let peers = parse_peers(&args.peers)?;

    let config = CoordinationConfig {
        listen_addr: args.listen_addr.clone(),
        peers,
        max_message_size: 1_048_576,
        recv_buffer: 1024,
        namespace: args.namespace.as_bytes().to_vec(),
    };

    let node = CoordinationNode::from_seed(&label, args.seed, config)?;

    if args.print_pubkey {
        println!("{}", node.public_key_hex());
        return Ok(());
    }

    info!(
        "soak-node '{}' starting: pk={} listen={} peers={}",
        label,
        node.public_key_hex(),
        args.listen_addr,
        args.peers.len()
    );

    // Grab handles before `run()` consumes the node.
    let sender = node.sender_handle();
    let recv_handle: Arc<RwLock<Option<tokio::sync::mpsc::Receiver<AgentMessage>>>> =
        node.recv_handle();
    let my_pk = node.public_key_vec();

    // Drive the P2P runtime in the background.
    let run_label = label.clone();
    tokio::spawn(async move {
        if let Err(e) = node.run().await {
            error!("[{}] P2P run() exited with error: {}", run_label, e);
        }
    });

    // Recv loop — log every inbound message.
    let recv_label = label.clone();
    tokio::spawn(async move {
        loop {
            let msg_opt = {
                let mut guard = recv_handle.write().await;
                match guard.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => None,
                }
            };
            match msg_opt {
                Some(msg) => {
                    info!(
                        "[{}] recv from={} bytes={} content={:?}",
                        recv_label,
                        hex::encode(&msg.from),
                        msg.content.len(),
                        String::from_utf8_lossy(&msg.content)
                    );
                }
                None => {
                    warn!("[{}] recv channel closed, stopping recv loop", recv_label);
                    break;
                }
            }
        }
    });

    // Heartbeat loop — broadcast a message every `heartbeat_secs`.
    let mut tick = interval(Duration::from_secs(args.heartbeat_secs));
    let mut counter: u64 = 0;
    loop {
        tick.tick().await;
        counter += 1;
        let content = format!("heartbeat-{}-{}", label, counter).into_bytes();
        let msg = AgentMessage::new(my_pk.clone(), vec![], content, now_ms());

        match sender.send(msg).await {
            Ok(()) => info!("[{}] sent heartbeat #{}", label, counter),
            Err(e) => warn!("[{}] failed to queue heartbeat #{}: {}", label, counter, e),
        }
    }
}
