//! JunoClaw Nostr Bridge — kind 38402 task discovery layer.
//!
//! Watches the JunoClaw `task-ledger` for new tasks via Tendermint websocket
//! and publishes Nostr kind 38402 (parametrized replaceable event) to a
//! configured set of relays. Agents subscribe to these events to discover
//! tasks without polling chain RPC directly.
//!
//! # Event kind 38402
//!
//! Kind 38402 is in the experimental range, mnemonic-aligned with HTTP 402.
//! The `d` tag uniquely identifies the event as `{chain_id}:{contract}:{task_id}`.
//! Being a parametrized replaceable event, if the task state changes (e.g. claimed),
//! the bridge publishes a replacement event with the same `d` tag.
//!
//! # Running the bridge
//!
//! ```bash
//! JUNOCLAW_NOSTR_PRIVKEY=<hex_privkey> \
//! JUNOCLAW_RPC=https://rpc.juno.strange.love:443 \
//! JUNOCLAW_CONTRACT=juno1... \
//! JUNOCLAW_CHAIN_ID=juno-1 \
//! junoclaw-nostr-bridge
//! ```
//!
//! Multiple bridge instances can run simultaneously — relay deduplication
//! by event ID ensures agents see each task exactly once.

pub mod config;
pub mod event;
pub mod subscriber;
pub mod publisher;
pub mod types;

pub use config::BridgeConfig;
pub use event::TaskEvent;
pub use types::{TaskInfo, TaskStatus};
