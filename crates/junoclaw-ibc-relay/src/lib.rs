//! JunoClaw IBC Task Relay — ICS-20 + PFM memo-based cross-chain task operations.
//!
//! Enables agents on any Cosmos chain to participate in JunoClaw tasks on Juno
//! without bridging assets or maintaining a Juno key. The agent sends an ICS-20
//! transfer with a structured JSON memo; PFM forwards it to `ibc-task-host` on
//! Juno, which decodes the memo and dispatches to `task-ledger` / `escrow`.
//!
//! # Wire format
//!
//! The memo field of the ICS-20 transfer carries a JunoClaw operation:
//!
//! ```json
//! {
//!   "wasm": {
//!     "contract": "juno1...ibc-task-host",
//!     "msg": {
//!       "junoclaw_v1": {
//!         "accept_task": {
//!           "task_id": 42,
//!           "agent_addr": "juno1...",
//!           "agent_origin_chain": "osmosis-1",
//!           "agent_origin_addr": "osmo1..."
//!         }
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! Four operations in v2.1:
//! - **`accept_task`** — agent registers as worker for an open task
//! - **`submit_proof`** — agent submits Groth16 proof; triggers zk-verifier
//! - **`reclaim_expired`** — DAO reclaims escrow on expired tasks
//! - **`swap`** — cross-chain autonomous Junoswap swap (agent-operated liquidity)
//!
//! # Security
//!
//! Security rests on IBC's standard light-client model (Tendermint headers,
//! validator set verification). No multisig bridges, no centralized relayer
//! trust. The relayer process is permissionless — anyone can run it.
//!
//! # Scope
//!
//! This crate is **v2 scope** — implementation begins after Juno v31 mainnet.
//! It is scaffolded now for interface stability and to enable integration testing
//! against mock IBC environments.

pub mod config;
pub mod memo;
pub mod relay;
pub mod error;

pub use config::RelayConfig;
pub use memo::{JunoClawMemo, JunoClawOp, AcceptTask, SubmitProof, ReclaimExpired, SwapOp};
pub use error::RelayError;
