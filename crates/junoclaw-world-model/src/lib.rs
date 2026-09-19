//! JunoClaw L2 World Model
//!
//! Predicts the consequences of candidate actions in ~100ms.
//! Trained on Merkle-verified transitions from the L1 Memory Index,
//! the world model lets a robot "imagine" what happens next before acting.
//!
//! Architecture:
//! - `Transition` — (state, action) → outcome, sourced from L1 Memory Index
//! - `WorldModel` — lookup + nearest-neighbor prediction over transitions
//! - `Prediction` — predicted outcome with confidence score
//!
//! The model is intentionally simple: nearest-neighbor lookup over the
//! Merkle-verified transition history. When the robot asks "what happens
//! if I take action X in state Y?", the world model finds the most similar
//! past transitions and returns the outcome distribution.
//!
//! This is the L2 tier in the latency stack:
//!   L0 (1ms)   — Reflex (classical control)
//!   L1 (12ms)  — Memory fetch (Merkle-verified recall)
//!   L2 (100ms) — World model (this crate: predict consequences)
//!   L3 (300ms) — Settlement (on-chain)
//!
//! In production, this would be replaced by a trained neural network
//! (e.g., a small transformer or MLP) that takes (state, action) as input
//! and outputs predicted outcome + confidence. The nearest-neighbor
//! approach here is the cold-start baseline that works before enough
//! data is collected for supervised learning.

pub mod transition;
pub mod model;
pub mod prediction;

pub use transition::{Transition, TransitionId};
pub use model::{WorldModel, WorldModelConfig};
pub use prediction::{Prediction, OutcomeDistribution, Confidence};
