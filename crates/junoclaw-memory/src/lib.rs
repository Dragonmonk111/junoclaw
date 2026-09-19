//! JunoClaw L1 Memory Index
//!
//! Merkle-verified memory layer that lets any robot recall any past state
//! in ~12ms. Memory entries are stored as leaves in a Merkle tree. The root
//! is committed to the blockchain (via coordination-settler or directly),
//! making the memory tamper-evident and shareable across robot owners.
//!
//! Architecture:
//! - `MemoryEntry` — a single cycle record (state + action + outcome + timestamp)
//! - `MerkleTree` — append-only Merkle tree of memory entries
//! - `MemoryIndex` — in-memory index for O(log n) lookup by key
//! - `Proof` — Merkle inclusion proof for a single entry
//!
//! Usage:
//! ```
//! use junoclaw_memory::{MemoryIndex, MemoryEntry};
//!
//! let mut index = MemoryIndex::new();
//! let entry = MemoryEntry::new("robot-1", "standing", "walk-forward", "moved 0.3m");
//! let proof = index.insert(entry);
//! // proof can be verified against the on-chain root
//! ```

pub mod entry;
pub mod merkle;
pub mod index;
pub mod proof;

pub use entry::{MemoryEntry, CycleState, CycleAction, CycleOutcome, SensorSnapshot};
pub use merkle::MerkleTree;
pub use index::MemoryIndex;
pub use proof::{MerkleProof, ProofVerification};
