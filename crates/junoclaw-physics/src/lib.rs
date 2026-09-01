//! JunoClaw Physics Simulation — real reflex cycle hashes from physics state.
//!
//! This crate provides the physics layer that sits between a robot's
//! hardware controller and the reflex-tier trust stack. It:
//!
//! 1. **Simulates** robot physics at reflex-tier frequency (1000Hz default)
//!    — either via a built-in rigid-body simulator or MuJoCo (when available)
//! 2. **Hashes** each cycle's physics state (joint positions, velocities,
//!    contacts, IMU readings, sensor readings) with SHA-256
//! 3. **Checks** safety invariants against a `SafetyEnvelope` every cycle
//! 4. **Builds** a Merkle tree from cycle hashes
//! 5. **Produces** a `ReflexBatchAttestation` with the Merkle root, ready
//!    for on-chain anchoring via the merkle-verifier contract
//!
//! The key insight: reflexes run at hardware speed (8-12ms) and cannot
//! wait for consensus. But after a batch of cycles, the controller
//! produces a cryptographic proof (Merkle root) that the safety envelope
//! was maintained. This is post-hoc verifiable — if an incident occurs,
//! the full physics log can be compared against the anchored root.
//!
//! ## Backends
//!
//! - `simulated` (default): Built-in rigid-body dynamics. No external deps.
//!   Produces realistic physics state (gravity, joint limits, contacts).
//! - `mujoco` (feature flag): Wraps MuJoCo Rust bindings. Requires MuJoCo SDK.
//!   (Not yet wired — placeholder for when `mujoco-rs` crate stabilizes.)

pub mod state;
pub mod simulator;
pub mod merkle;
pub mod attestation;
pub mod learning;
pub mod memory;
pub mod worldmodel;
pub mod pipeline;
pub mod dataset;
pub mod replay;
pub mod watchdog;
pub mod audit;
pub mod fleet;
pub mod skill;

pub use state::{PhysicsState, SensorReadings, JointState, ContactInfo, ImuReading};
pub use simulator::{PhysicsSimulator, SimulatedBackend, SimConfig, QuadrupedBackend, QuadrupedConfig, QUADRUPED_JOINT_NAMES};
pub use merkle::{compute_merkle_root, compute_merkle_proof, verify_merkle_proof};
pub use attestation::{BatchConfig, BatchResult, run_reflex_batch, check_invariants};
pub use learning::{TrustLearner, TrustVerdict, LearningConfig, AdjustedEnvelope, BuzzAkashPipeline, VerdictSource, VerdictWithProvenance, VerdictBatchSummary};
pub use memory::{StateFeatures, MemoryRecord, MemoryHit, MemoryIndex, RootCache, MemoryFetch};
pub use worldmodel::{TransitionSample, ActionVector, PredictedState, ActionEvaluation, WorldModel};
pub use pipeline::{ReflexPipeline, PipelineConfig, PipelineStepResult};
pub use dataset::{DatasetRecord, DatasetStats, DatasetExporter};
pub use replay::{ReplayCycle, ReplayLog, Recorder, ReplayVerification, replay};
pub use watchdog::{redundant_check, dual_channel_check, WatchdogVerdict};
pub use audit::{AuditBundle, AuditVerification, SampleProof};
pub use fleet::{FleetRegistry, ContributorStats, FleetRejection, SyncSummary};
pub use skill::{Skill, SkillManifest, SkillRecorder, RetargetedSkill, RetargetReport, SkillGate, GatedFrameDecision};
