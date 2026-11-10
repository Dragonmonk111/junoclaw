//! junoclaw-attest-gateway — customer-facing /attest ingestion surface.
//!
//! See `docs/ATTEST_GATEWAY_SPEC.md` for the protocol spec. The gateway
//! accepts records of agent actions, batches them into Merkle windows,
//! anchors the root via `anchor_root` on the deployed merkle-verifier
//! contract, and returns inclusion proofs plus the covering block
//! certificate height.

pub mod batcher;
pub mod canon;
pub mod config;
pub mod error;
pub mod merkle;
pub mod routes;
pub mod store;
pub mod types;

pub use junoclaw_chain_grpc as chain;
