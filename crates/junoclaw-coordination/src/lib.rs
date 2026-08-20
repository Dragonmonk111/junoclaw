//! JunoClaw Coordination Layer
//!
//! Commonware-based P2P mesh for authenticated, ordered agent messaging.
//! Sits between the Truth layer (J-Lens) and the Settlement layer (Juno).
//!
//! Architecture:
//! - `message` — AgentMessage protocol struct (always available)
//! - `gate` — J-Lens truth gate integration (always available)
//! - `network` — P2P mesh using commonware-p2p (requires `p2p` feature)
//! - `node` — Coordination node (requires `p2p` feature)

pub mod message;
pub mod gate;
pub mod consensus;
pub mod context;
pub mod api;
pub mod fleet;
pub mod feepay;

#[cfg(feature = "p2p")]
pub mod network;

#[cfg(feature = "p2p")]
pub mod node;

pub use message::{
    AgentMessage, Batch, GateVerdict, GateResult,
    IntentMessage, IntentOutcome,
    SafetyEnvelope, ReflexBatchAttestation, CircuitBreakerState,
    BreakerAction, ProofContext,
};
pub use gate::{GateConfig, JLensGate, ProofVerifier, MockProofVerifier, OnChainProofVerifier, OnChainProofVerifierConfig};
pub use consensus::{ConsensusEngine, ConsensusConfig, FinalizedBlock};
pub use context::{ContextFetcher, ContextSummary, MoultbookContextFetcher, MockContextFetcher};
pub use api::{ApiConfig, serve as serve_api};
pub use fleet::{FleetCoordinator, FleetConfig, FleetStatus, RobotState, RobotStatusEntry};
pub use feepay::{FeePayMonitor, FeePayPoolState, FeePayAlert, AlertLevel, RegisteredWallet, PoolHistoryEntry};

#[cfg(feature = "p2p")]
pub use network::{CoordinationConfig, CoordinationNetwork};

#[cfg(feature = "p2p")]
pub use node::CoordinationNode;
