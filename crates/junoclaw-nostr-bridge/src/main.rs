//! Daemon entrypoint for the JunoClaw Nostr bridge.
//!
//! Wires the Tendermint websocket subscriber to the Nostr publisher:
//!
//! 1. Load [`BridgeConfig`] from the environment (12-factor).
//! 2. Connect the Nostr publisher to the configured relays.
//! 3. Subscribe to `task-ledger` `post_task` events over the chain websocket.
//! 4. For each new task, publish a kind 38402 event to the relays.
//!
//! The subscriber callback is synchronous (`Fn(TaskInfo)`), but publishing is
//! async, so an unbounded mpsc channel bridges the two: the callback enqueues
//! tasks and a dedicated task drains the queue and publishes.
//!
//! The websocket is the unreliable part of the system, so the subscriber runs
//! inside a reconnect loop with exponential backoff (reset after a connection
//! has been stable for a while). Shutdown is graceful: on Ctrl+C or SIGTERM the
//! loop stops, the channel is closed, and the publisher task drains in-flight
//! tasks before the process exits.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use junoclaw_nostr_bridge::config::BridgeConfig;
use junoclaw_nostr_bridge::publisher::NostrPublisher;
use junoclaw_nostr_bridge::subscriber::subscribe_task_ledger;
use junoclaw_nostr_bridge::types::TaskInfo;

/// Backoff applied to the very first reconnect attempt.
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
/// Cap on the exponential reconnect backoff.
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// A connection that survived at least this long is considered "stable",
/// so the backoff is reset to [`INITIAL_BACKOFF`] when it eventually drops.
const STABLE_CONNECTION: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> Result<()> {
    let config = BridgeConfig::from_env().context("failed to load bridge config from env")?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("JunoClaw Nostr bridge starting");
    info!("  chain_id = {}", config.chain_id);
    info!("  contract = {}", config.contract);
    info!("  rpc_url  = {}", config.rpc_url);
    info!("  ws_url   = {}", config.ws_url());
    info!("  relays   = {:?}", config.relays);

    // Connect to relays before we start watching the chain, so the first task
    // we see can be published immediately.
    let publisher = NostrPublisher::new(&config)
        .await
        .context("failed to initialise Nostr publisher")?;
    info!("bridge pubkey = {}", publisher.pubkey_hex());

    // Sync (subscriber callback) -> async (publisher) bridge.
    let (tx, mut rx) = mpsc::unbounded_channel::<TaskInfo>();

    let publish_handle = tokio::spawn(async move {
        while let Some(task) = rx.recv().await {
            if let Err(e) = publisher.publish_task(&task).await {
                warn!("publish error for task {}: {e}", task.task_id);
            }
        }
        info!("publish queue drained; publisher task exiting");
    });

    // A single shutdown future, polled across every loop iteration.
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    let ws_url = config.ws_url();
    let mut backoff = INITIAL_BACKOFF;

    loop {
        let tx_clone = tx.clone();
        let started = Instant::now();

        let sub = subscribe_task_ledger(
            &ws_url,
            &config.contract,
            &config.chain_id,
            &config.zk_verifier,
            move |task| {
                // Send fails only if the receiver is gone (shutdown in progress).
                if let Err(e) = tx_clone.send(task) {
                    warn!("failed to enqueue task for publishing: {e}");
                }
            },
        );

        tokio::select! {
            res = sub => {
                match res {
                    Ok(()) => warn!("subscriber connection closed by peer"),
                    Err(e) => error!("subscriber error: {e}"),
                }
            }
            _ = &mut shutdown => {
                info!("shutdown signal received; stopping subscriber");
                break;
            }
        }

        // Reset backoff if the connection had been stable; otherwise grow it.
        if started.elapsed() >= STABLE_CONNECTION {
            backoff = INITIAL_BACKOFF;
        }
        warn!("reconnecting in {:?}", backoff);

        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = &mut shutdown => {
                info!("shutdown during reconnect backoff; exiting");
                break;
            }
        }

        backoff = (backoff * 2).min(MAX_BACKOFF);
    }

    // Closing the last sender lets the publisher task drain and finish.
    drop(tx);
    if let Err(e) = publish_handle.await {
        warn!("publisher task join error: {e}");
    }

    info!("JunoClaw Nostr bridge stopped cleanly");
    Ok(())
}

/// Resolves when the process receives Ctrl+C (all platforms) or SIGTERM (unix).
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        signal(SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("received Ctrl+C"),
        _ = terminate => info!("received SIGTERM"),
    }
}
