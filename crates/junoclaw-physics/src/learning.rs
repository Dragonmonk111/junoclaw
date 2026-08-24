//! Reinforcement Learning from Truth (RL-TF) — the learning loop that
//! adapts robot behavior based on truth market verdicts.
//!
//! This module implements the core insight of the JunoClaw trust layer:
//! the robot's safety envelope should *tighten* when truth verdicts are
//! negative (red/yellow) and can *relax* (within DAO bounds) when verdicts
//! are consistently positive (green).
//!
//! ## How it works
//!
//! 1. After each reflex batch attestation is anchored on-chain, the truth
//!    market produces a verdict about the robot's behavior.
//! 2. The `TrustLearner` accumulates these verdicts and computes a trust
//!    score (exponential moving average).
//! 3. Based on the trust score, the learner produces an `AdjustedEnvelope`
//!    that modifies the base (DAO-governed) safety envelope.
//! 4. **Critical invariant**: the adjusted envelope can only be *stricter*
//!    than the base envelope. The learner can never relax beyond the
//!    DAO-approved limits. This ensures the learning loop cannot be
//!    exploited to weaken safety.
//! 5. The adjusted envelope is fed into the next `BatchConfig`, closing
//!    the loop: truth verdict → trust score → adjusted envelope → behavior.

use junoclaw_coordination::SafetyEnvelope;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Pipeline: Buzz Agent → Akash GPU → J-Lens → TrustLearner
// ---------------------------------------------------------------------------

/// Source of a truth verdict — traces the provenance of each verdict
/// through the meta-chain pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerdictSource {
    /// Nostr keypair of the Buzz agent that submitted the verdict
    pub agent_pubkey: String,
    /// Buzz relay event ID (kind 38402 task-discovery or kind 1 response)
    pub buzz_event_id: String,
    /// Akash lease ID for the GPU compute used (if hired via escrow)
    pub akash_lease_id: Option<String>,
    /// J-Lens Brainmaxx trace reference (Moultbook entry ID)
    pub brainmaxx_trace_ref: Option<String>,
    /// Open-weight model used (e.g. "qwen2.5-7b", "llama3-8b")
    pub model_id: String,
    /// Whether the J-Lens D1 probe was active during inference
    pub jlens_probe_active: bool,
    /// TEE attestation hash (SGX/Nitro quote hash)
    pub tee_attestation_hash: Option<String>,
}

/// A verdict from the truth market, with full provenance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerdictWithProvenance {
    /// The trust verdict (green/yellow/red)
    pub verdict: TrustVerdict,
    /// Robot ID being verified
    pub robot_id: String,
    /// Batch ID (Merkle root of the reflex batch)
    pub batch_merkle_root: String,
    /// Block height where the attestation was anchored
    pub anchor_block_height: u64,
    /// Consensus ratio (matching / total operators)
    pub consensus_ratio: f64,
    /// Number of matching operators
    pub matching_operators: u32,
    /// Number of diverging operators
    pub diverging_operators: u32,
    /// Violated invariants (if any)
    pub violated_invariants: Vec<String>,
    /// Full provenance chain
    pub source: VerdictSource,
}

/// Summary of a batch of verdicts from the truth market.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerdictBatchSummary {
    /// Robot ID
    pub robot_id: String,
    /// Block height range
    pub from_block: u64,
    pub to_block: u64,
    /// All verdicts in the range
    pub verdicts: Vec<VerdictWithProvenance>,
    /// Aggregate consensus (weighted by consensus_ratio)
    pub aggregate_consensus: f64,
    /// Dominant verdict
    pub dominant_verdict: TrustVerdict,
}

/// The meta-chain pipeline: Buzz → Akash → J-Lens → TrustLearner.
///
/// This struct wires together the four layers:
/// 1. Buzz relay (agent coordination, task discovery)
/// 2. Akash GPU (compute lease for open-weight inference)
/// 3. J-Lens (open-weight model probing, Brainmaxx trace)
/// 4. TrustLearner (robot behavior adaptation)
///
/// In production, steps 1-3 happen off-chain via the WAVS invoke API.
/// This struct provides the interface for consuming their outputs.
pub struct BuzzAkashPipeline {
    /// The trust learner that accumulates verdicts
    learner: TrustLearner,
    /// History of all verdicts consumed (with provenance)
    verdict_history: Vec<VerdictWithProvenance>,
    /// Whether the pipeline requires J-Lens provenance for acceptance
    require_jlens_provenance: bool,
}

impl BuzzAkashPipeline {
    /// Create a new pipeline with the given base envelope and learning config.
    pub fn new(
        base_envelope: SafetyEnvelope,
        learning_config: LearningConfig,
        require_jlens_provenance: bool,
    ) -> Self {
        Self {
            learner: TrustLearner::new(base_envelope, learning_config),
            verdict_history: Vec::new(),
            require_jlens_provenance,
        }
    }

    /// Consume a verdict from the truth market.
    /// 
    /// If `require_jlens_provenance` is true, rejects verdicts without
    /// a Brainmaxx trace reference (i.e. from closed-weight models).
    pub fn consume_verdict(&mut self, v: VerdictWithProvenance) -> Result<(), String> {
        // Validate provenance
        if self.require_jlens_provenance {
            if !v.source.jlens_probe_active {
                return Err(format!(
                    "rejected verdict: J-Lens probe not active (agent={})",
                    &v.source.agent_pubkey[..16.min(v.source.agent_pubkey.len())]
                ));
            }
            if v.source.brainmaxx_trace_ref.is_none() {
                return Err(format!(
                    "rejected verdict: no Brainmaxx trace (agent={})",
                    &v.source.agent_pubkey[..16.min(v.source.agent_pubkey.len())]
                ));
            }
        }

        // Validate consensus threshold
        if v.consensus_ratio < 0.5 {
            warn!(
                "low consensus verdict: ratio={:.2}, matching={}, diverging={}",
                v.consensus_ratio, v.matching_operators, v.diverging_operators
            );
        }

        info!(
            "RL-TF pipeline: consuming verdict {:?} for robot={} batch={} consensus={:.2} model={} jlens={}",
            v.verdict, v.robot_id, &v.batch_merkle_root[..16.min(v.batch_merkle_root.len())],
            v.consensus_ratio, v.source.model_id, v.source.jlens_probe_active
        );

        // Feed to learner
        self.learner.observe(v.verdict.clone());

        // Store in history
        self.verdict_history.push(v);

        Ok(())
    }

    /// Consume a batch of verdicts and return the summary.
    pub fn consume_batch(&mut self, verdicts: Vec<VerdictWithProvenance>) -> Result<VerdictBatchSummary, String> {
        if verdicts.is_empty() {
            return Err("empty verdict batch".to_string());
        }

        let robot_id = verdicts[0].robot_id.clone();
        let from_block = verdicts.iter().map(|v| v.anchor_block_height).min().unwrap_or(0);
        let to_block = verdicts.iter().map(|v| v.anchor_block_height).max().unwrap_or(0);

        let mut green = 0u32;
        let mut yellow = 0u32;
        let mut red = 0u32;
        let mut consensus_sum = 0.0f64;

        for v in &verdicts {
            match v.verdict {
                TrustVerdict::Green => green += 1,
                TrustVerdict::Yellow => yellow += 1,
                TrustVerdict::Red => red += 1,
            }
            consensus_sum += v.consensus_ratio;
            self.consume_verdict(v.clone())?;
        }

        let aggregate_consensus = consensus_sum / verdicts.len() as f64;
        let dominant = if green >= yellow && green >= red {
            TrustVerdict::Green
        } else if yellow >= red {
            TrustVerdict::Yellow
        } else {
            TrustVerdict::Red
        };

        Ok(VerdictBatchSummary {
            robot_id,
            from_block,
            to_block,
            verdicts,
            aggregate_consensus,
            dominant_verdict: dominant,
        })
    }

    /// Get the current adjusted envelope (the output of the RL-TF loop).
    pub fn adjusted_envelope(&self) -> AdjustedEnvelope {
        self.learner.current_adjusted_envelope()
    }

    /// Get the current trust score.
    pub fn trust_score(&self) -> f64 {
        self.learner.trust_score()
    }

    /// Get the verdict history (with provenance).
    pub fn verdict_history(&self) -> &[VerdictWithProvenance] {
        &self.verdict_history
    }

    /// Get the underlying learner.
    pub fn learner(&self) -> &TrustLearner {
        &self.learner
    }

    /// Reset the pipeline (keeping the base envelope).
    pub fn reset(&mut self) {
        self.learner.reset();
        self.verdict_history.clear();
    }

    /// Verify that a specific training moment can be proven via Merkle branch.
    /// 
    /// In production, this would query the on-chain attestation and request
    /// the Merkle proof from the rosbag. Here we just validate the structure.
    pub fn verify_training_moment(
        &self,
        batch_merkle_root: &str,
        cycle_hash: &str,
        merkle_proof: &[String],
    ) -> bool {
        // The Merkle proof verification is: leaf + proof → root
        // This is a structural check — in production, use compute_merkle_proof
        // from the merkle module to verify.
        if merkle_proof.is_empty() {
            return false;
        }
        
        // Simple structural validation
        !batch_merkle_root.is_empty() 
            && batch_merkle_root.len() == 64 
            && !cycle_hash.is_empty()
            && cycle_hash.len() == 64
    }
}

/// A truth market verdict about a robot's behavior in a reflex batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum TrustVerdict {
    /// All invariants maintained, behavior aligned with intent.
    /// Trust score increases.
    Green,
    /// Minor concerns — some invariants near limits, or intent-behavior
    /// gap detected. Trust score decreases slightly.
    Yellow,
    /// Safety violation or significant intent-behavior mismatch.
    /// Trust score decreases sharply, envelope tightens aggressively.
    Red,
}

impl TrustVerdict {
    /// Numeric score contribution: green=+1, yellow=-0.5, red=-2.
    fn score_delta(&self) -> f64 {
        match self {
            TrustVerdict::Green => 1.0,
            TrustVerdict::Yellow => -0.5,
            TrustVerdict::Red => -2.0,
        }
    }

    /// Map verdict to a DOGZILLA-Lite screen expression.
    pub fn to_expression(&self) -> &'static str {
        match self {
            TrustVerdict::Green => "happy",
            TrustVerdict::Yellow => "alert",
            TrustVerdict::Red => "angry",
        }
    }
}

/// Configuration for the trust learning loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningConfig {
    /// Exponential moving average decay factor (0.0–1.0).
    /// Higher = faster adaptation. Default: 0.1 (slow, conservative).
    pub ema_alpha: f64,
    /// Initial trust score (0.0–1.0). Default: 0.5 (neutral).
    pub initial_trust: f64,
    /// Minimum trust score (floor). Default: 0.0.
    pub min_trust: f64,
    /// Maximum trust score (ceiling). Default: 1.0.
    pub max_trust: f64,
    /// How much to tighten when trust is low.
    /// At trust=0, envelope params are multiplied by (1 - tightening_factor).
    /// At trust=1, envelope params are at their DAO-approved values.
    pub tightening_factor: f64,
    /// Number of consecutive green verdicts required before any relaxation.
    pub green_streak_threshold: u32,
    /// Number of red verdicts in window before circuit breaker recommendation.
    pub red_threshold: usize,
    /// Sliding window size for verdict history.
    pub window_size: usize,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            ema_alpha: 0.1,
            initial_trust: 0.5,
            min_trust: 0.0,
            max_trust: 1.0,
            tightening_factor: 0.5,
            green_streak_threshold: 10,
            red_threshold: 3,
            window_size: 50,
        }
    }
}

/// An adjusted safety envelope — the DAO-approved envelope with
/// learning-based tightening applied.
#[derive(Clone, Debug)]
pub struct AdjustedEnvelope {
    /// The adjusted safety envelope (always ≤ base envelope in permissiveness)
    pub envelope: SafetyEnvelope,
    /// Current trust score (0.0–1.0)
    pub trust_score: f64,
    /// Number of verdicts processed
    pub verdict_count: u64,
    /// Current consecutive green streak
    pub green_streak: u32,
    /// Whether the learner recommends triggering the circuit breaker
    pub recommend_circuit_breaker: bool,
}

/// The trust learner — accumulates truth verdicts and produces adjusted envelopes.
///
/// This is the core of the RL-TF loop. It maintains:
/// - An exponential moving average of trust (the trust score)
/// - A sliding window of recent verdicts
/// - A streak counter for consecutive green verdicts
///
/// The learner is deterministic given the same sequence of verdicts,
/// making it auditable and replayable.
pub struct TrustLearner {
    config: LearningConfig,
    trust_score: f64,
    verdict_history: Vec<TrustVerdict>,
    green_streak: u32,
    total_verdicts: u64,
    /// The base (DAO-governed) safety envelope that adjustments are applied to.
    base_envelope: SafetyEnvelope,
}

impl TrustLearner {
    pub fn new(base_envelope: SafetyEnvelope, config: LearningConfig) -> Self {
        let initial_trust = config.initial_trust;
        Self {
            config,
            trust_score: initial_trust,
            verdict_history: Vec::new(),
            green_streak: 0,
            total_verdicts: 0,
            base_envelope,
        }
    }

    /// Create a learner with default config.
    pub fn with_defaults(base_envelope: SafetyEnvelope) -> Self {
        Self::new(base_envelope, LearningConfig::default())
    }

    /// Process a new truth verdict and update the trust score.
    pub fn observe(&mut self, verdict: TrustVerdict) {
        let delta = verdict.score_delta();
        
        // EMA update: trust = (1 - alpha) * trust + alpha * (trust + delta)
        // But we need to normalize: each verdict contributes a bounded delta
        let target = (self.trust_score + delta).clamp(self.config.min_trust, self.config.max_trust);
        self.trust_score = (1.0 - self.config.ema_alpha) * self.trust_score
            + self.config.ema_alpha * target;
        self.trust_score = self.trust_score.clamp(self.config.min_trust, self.config.max_trust);

        // Update streak
        match verdict {
            TrustVerdict::Green => self.green_streak += 1,
            _ => self.green_streak = 0,
        }

        // Update history
        self.verdict_history.push(verdict.clone());
        if self.verdict_history.len() > self.config.window_size {
            self.verdict_history.remove(0);
        }

        self.total_verdicts += 1;

        info!(
            "RL-TF: verdict={:?}, trust={:.3}, streak={}, total={}",
            verdict, self.trust_score, self.green_streak, self.total_verdicts
        );

        if matches!(verdict, TrustVerdict::Red) {
            let red_count = self.verdict_history.iter()
                .filter(|v| matches!(v, TrustVerdict::Red))
                .count();
            if red_count >= self.config.red_threshold {
                warn!(
                    "RL-TF: circuit breaker recommended ({} red verdicts in last {})",
                    red_count, self.config.window_size
                );
            }
        }
    }

    /// Produce the current adjusted envelope based on trust score.
    pub fn current_adjusted_envelope(&self) -> AdjustedEnvelope {
        // Tightening: at trust=1.0, no tightening. At trust=0.0, max tightening.
        // tightening = factor * (1 - trust_score)
        let tighten = self.config.tightening_factor * (1.0 - self.trust_score);

        // Only relax if we have enough consecutive green verdicts
        let can_relax = self.green_streak >= self.config.green_streak_threshold;
        let effective_tighten = if can_relax {
            // If we've earned trust, reduce tightening further
            tighten * 0.5
        } else {
            tighten
        };

        // Apply tightening: adjusted = base * (1 - effective_tighten)
        // This can only make parameters *stricter* (lower limits), never looser.
        let scale = 1.0 - effective_tighten;

        let adjusted = SafetyEnvelope {
            robot_id: self.base_envelope.robot_id.clone(),
            max_speed: self.base_envelope.max_speed * scale,
            max_force: self.base_envelope.max_force * scale,
            min_collision_distance: self.base_envelope.min_collision_distance
                + (self.base_envelope.min_collision_distance * effective_tighten),
            max_tilt_degrees: self.base_envelope.max_tilt_degrees * scale,
            max_acceleration: self.base_envelope.max_acceleration * scale,
            human_proximity_allowed: self.base_envelope.human_proximity_allowed
                && self.trust_score > 0.3,
            max_arm_force: self.base_envelope.max_arm_force * scale,
            max_joint_torque: self.base_envelope.max_joint_torque * scale,
            version: self.base_envelope.version,
        };

        let red_count = self.verdict_history.iter()
            .filter(|v| matches!(v, TrustVerdict::Red))
            .count();

        AdjustedEnvelope {
            envelope: adjusted,
            trust_score: self.trust_score,
            verdict_count: self.total_verdicts,
            green_streak: self.green_streak,
            recommend_circuit_breaker: red_count >= self.config.red_threshold,
        }
    }

    /// Get the current trust score.
    pub fn trust_score(&self) -> f64 {
        self.trust_score
    }

    /// Get the current green streak.
    pub fn green_streak(&self) -> u32 {
        self.green_streak
    }

    /// Get the verdict history (sliding window).
    pub fn verdict_history(&self) -> &[TrustVerdict] {
        &self.verdict_history
    }

    /// Get the base (DAO-approved) envelope.
    pub fn base_envelope(&self) -> &SafetyEnvelope {
        &self.base_envelope
    }

    /// Reset the learner to initial state (keeping the base envelope).
    pub fn reset(&mut self) {
        self.trust_score = self.config.initial_trust;
        self.verdict_history.clear();
        self.green_streak = 0;
        self.total_verdicts = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_envelope() -> SafetyEnvelope {
        SafetyEnvelope {
            robot_id: "dogzilla-lite-001".to_string(),
            max_speed: 1.5,
            max_force: 30.0,
            min_collision_distance: 0.15,
            max_tilt_degrees: 35.0,
            max_acceleration: 2.0,
            human_proximity_allowed: true,
            max_arm_force: 10.0,
            max_joint_torque: 5.0,
            version: 1,
        }
    }

    #[test]
    fn test_trust_learner_starts_neutral() {
        let learner = TrustLearner::with_defaults(test_envelope());
        assert!((learner.trust_score() - 0.5).abs() < 1e-6);
        assert_eq!(learner.green_streak(), 0);
    }

    #[test]
    fn test_green_verdicts_increase_trust() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..50 {
            learner.observe(TrustVerdict::Green);
        }
        assert!(learner.trust_score() > 0.5,
            "trust should increase after green verdicts, got {}", learner.trust_score());
    }

    #[test]
    fn test_red_verdicts_decrease_trust() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..10 {
            learner.observe(TrustVerdict::Red);
        }
        assert!(learner.trust_score() < 0.5,
            "trust should decrease after red verdicts, got {}", learner.trust_score());
    }

    #[test]
    fn test_yellow_verdicts_decrease_trust_slightly() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..20 {
            learner.observe(TrustVerdict::Yellow);
        }
        assert!(learner.trust_score() < 0.5,
            "trust should decrease after yellow verdicts, got {}", learner.trust_score());
    }

    #[test]
    fn test_adjusted_envelope_stricter_when_low_trust() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..20 {
            learner.observe(TrustVerdict::Red);
        }
        let adjusted = learner.current_adjusted_envelope();
        assert!(adjusted.envelope.max_speed < test_envelope().max_speed,
            "adjusted max_speed should be stricter, got {} vs {}",
            adjusted.envelope.max_speed, test_envelope().max_speed);
        assert!(adjusted.envelope.max_joint_torque < test_envelope().max_joint_torque,
            "adjusted max_joint_torque should be stricter");
    }

    #[test]
    fn test_adjusted_envelope_relaxes_after_green_streak() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        // First tank the trust
        for _ in 0..20 {
            learner.observe(TrustVerdict::Red);
        }
        let low_trust_adjusted = learner.current_adjusted_envelope();

        // Then build it back up with green streak
        for _ in 0..100 {
            learner.observe(TrustVerdict::Green);
        }
        let high_trust_adjusted = learner.current_adjusted_envelope();

        assert!(high_trust_adjusted.envelope.max_speed > low_trust_adjusted.envelope.max_speed,
            "envelope should relax after green streak");
        assert!(high_trust_adjusted.green_streak >= 10,
            "should have sufficient green streak");
    }

    #[test]
    fn test_envelope_never_exceeds_base() {
        let base = test_envelope();
        let mut learner = TrustLearner::with_defaults(base.clone());
        // Even with 1000 green verdicts, adjusted should not exceed base
        for _ in 0..1000 {
            learner.observe(TrustVerdict::Green);
        }
        let adjusted = learner.current_adjusted_envelope();
        assert!(adjusted.envelope.max_speed <= base.max_speed + 1e-6,
            "adjusted max_speed should never exceed base, got {} vs {}",
            adjusted.envelope.max_speed, base.max_speed);
        assert!(adjusted.envelope.max_arm_force <= base.max_arm_force + 1e-6,
            "adjusted max_arm_force should never exceed base");
    }

    #[test]
    fn test_circuit_breaker_recommendation() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        // 3 red verdicts should trigger circuit breaker recommendation
        learner.observe(TrustVerdict::Red);
        learner.observe(TrustVerdict::Red);
        assert!(!learner.current_adjusted_envelope().recommend_circuit_breaker,
            "should not recommend after 2 reds");

        learner.observe(TrustVerdict::Red);
        assert!(learner.current_adjusted_envelope().recommend_circuit_breaker,
            "should recommend after 3 reds");
    }

    #[test]
    fn test_human_proximity_disabled_at_low_trust() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..20 {
            learner.observe(TrustVerdict::Red);
        }
        let adjusted = learner.current_adjusted_envelope();
        assert!(!adjusted.envelope.human_proximity_allowed,
            "human proximity should be disabled at low trust");
    }

    #[test]
    fn test_human_proximity_enabled_at_high_trust() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..100 {
            learner.observe(TrustVerdict::Green);
        }
        let adjusted = learner.current_adjusted_envelope();
        assert!(adjusted.envelope.human_proximity_allowed,
            "human proximity should be enabled at high trust");
    }

    #[test]
    fn test_verdict_to_expression() {
        assert_eq!(TrustVerdict::Green.to_expression(), "happy");
        assert_eq!(TrustVerdict::Yellow.to_expression(), "alert");
        assert_eq!(TrustVerdict::Red.to_expression(), "angry");
    }

    #[test]
    fn test_learner_deterministic() {
        let base = test_envelope();
        let config = LearningConfig::default();

        let mut l1 = TrustLearner::new(base.clone(), config.clone());
        let mut l2 = TrustLearner::new(base.clone(), config.clone());

        let verdicts = [
            TrustVerdict::Green, TrustVerdict::Green, TrustVerdict::Red,
            TrustVerdict::Yellow, TrustVerdict::Green,
        ];

        for v in &verdicts {
            l1.observe(v.clone());
            l2.observe(v.clone());
        }

        assert_eq!(l1.trust_score(), l2.trust_score(),
            "same verdicts should produce same trust score");
    }

    #[test]
    fn test_reset() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        for _ in 0..50 {
            learner.observe(TrustVerdict::Green);
        }
        assert!(learner.trust_score() > 0.5);
        learner.reset();
        assert!((learner.trust_score() - 0.5).abs() < 1e-6);
        assert_eq!(learner.green_streak(), 0);
        assert_eq!(learner.total_verdicts, 0);
    }

    #[test]
    fn test_min_collision_distance_increases_when_low_trust() {
        let base = test_envelope();
        let mut learner = TrustLearner::with_defaults(base.clone());
        for _ in 0..20 {
            learner.observe(TrustVerdict::Red);
        }
        let adjusted = learner.current_adjusted_envelope();
        assert!(adjusted.envelope.min_collision_distance > base.min_collision_distance,
            "min_collision_distance should increase (stricter) at low trust, got {} vs {}",
            adjusted.envelope.min_collision_distance, base.min_collision_distance);
    }

    #[test]
    fn test_trust_score_bounded() {
        let mut learner = TrustLearner::with_defaults(test_envelope());
        // Push trust very high
        for _ in 0..1000 {
            learner.observe(TrustVerdict::Green);
        }
        assert!(learner.trust_score() <= 1.0, "trust should not exceed 1.0");

        // Push trust very low
        learner.reset();
        for _ in 0..1000 {
            learner.observe(TrustVerdict::Red);
        }
        assert!(learner.trust_score() >= 0.0, "trust should not go below 0.0");
    }

    // --- BuzzAkashPipeline tests ---

    fn make_verdict(verdict: TrustVerdict, jlens: bool, trace: bool) -> VerdictWithProvenance {
        VerdictWithProvenance {
            verdict,
            robot_id: "dogzilla-lite-001".to_string(),
            batch_merkle_root: "a".repeat(64),
            anchor_block_height: 12345,
            consensus_ratio: 0.87,
            matching_operators: 7,
            diverging_operators: 1,
            violated_invariants: vec![],
            source: VerdictSource {
                agent_pubkey: "npub1abc123def456".to_string(),
                buzz_event_id: "evt_001".to_string(),
                akash_lease_id: Some("akash-lease-001".to_string()),
                brainmaxx_trace_ref: if trace { Some("moult:trace001".to_string()) } else { None },
                model_id: "qwen2.5-7b".to_string(),
                jlens_probe_active: jlens,
                tee_attestation_hash: Some("0xabc123".to_string()),
            },
        }
    }

    #[test]
    fn test_pipeline_consumes_jlens_verdict() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true, // require J-Lens
        );
        let v = make_verdict(TrustVerdict::Green, true, true);
        assert!(pipeline.consume_verdict(v).is_ok());
        assert_eq!(pipeline.verdict_history().len(), 1);
    }

    #[test]
    fn test_pipeline_rejects_no_jlens_probe() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let v = make_verdict(TrustVerdict::Green, false, true);
        assert!(pipeline.consume_verdict(v).is_err());
        assert_eq!(pipeline.verdict_history().len(), 0);
    }

    #[test]
    fn test_pipeline_rejects_no_brainmaxx_trace() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let v = make_verdict(TrustVerdict::Green, true, false);
        assert!(pipeline.consume_verdict(v).is_err());
        assert_eq!(pipeline.verdict_history().len(), 0);
    }

    #[test]
    fn test_pipeline_accepts_without_jlens_when_not_required() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            false, // don't require J-Lens
        );
        let v = make_verdict(TrustVerdict::Green, false, false);
        assert!(pipeline.consume_verdict(v).is_ok());
    }

    #[test]
    fn test_pipeline_trust_score_updates() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let initial = pipeline.trust_score();
        for _ in 0..50 {
            let v = make_verdict(TrustVerdict::Green, true, true);
            pipeline.consume_verdict(v).unwrap();
        }
        assert!(pipeline.trust_score() > initial,
            "trust should increase after green verdicts");
    }

    #[test]
    fn test_pipeline_adjusted_envelope_stricter_after_red() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        for _ in 0..20 {
            let v = make_verdict(TrustVerdict::Red, true, true);
            pipeline.consume_verdict(v).unwrap();
        }
        let adjusted = pipeline.adjusted_envelope();
        assert!(adjusted.envelope.max_speed < test_envelope().max_speed,
            "envelope should be stricter after red verdicts");
        assert!(adjusted.recommend_circuit_breaker,
            "should recommend circuit breaker after 20 reds");
    }

    #[test]
    fn test_pipeline_batch_consumption() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let verdicts: Vec<VerdictWithProvenance> = (0..10).map(|i| {
            let v = if i < 7 { TrustVerdict::Green } else if i < 9 { TrustVerdict::Yellow } else { TrustVerdict::Red };
            make_verdict(v, true, true)
        }).collect();
        let summary = pipeline.consume_batch(verdicts).unwrap();
        assert_eq!(summary.dominant_verdict, TrustVerdict::Green);
        assert!(summary.aggregate_consensus > 0.0);
        assert_eq!(pipeline.verdict_history().len(), 10);
    }

    #[test]
    fn test_pipeline_empty_batch_rejected() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        assert!(pipeline.consume_batch(vec![]).is_err());
    }

    #[test]
    fn test_pipeline_reset() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        for _ in 0..10 {
            pipeline.consume_verdict(make_verdict(TrustVerdict::Green, true, true)).unwrap();
        }
        assert!(!pipeline.verdict_history().is_empty());
        pipeline.reset();
        assert!(pipeline.verdict_history().is_empty());
        assert!((pipeline.trust_score() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_pipeline_merkle_verification() {
        let pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let root = "a".repeat(64);
        let leaf = "b".repeat(64);
        let proof = vec!["c".repeat(64)];
        assert!(pipeline.verify_training_moment(&root, &leaf, &proof));
        assert!(!pipeline.verify_training_moment("", &leaf, &proof));
        assert!(!pipeline.verify_training_moment(&root, "", &proof));
        assert!(!pipeline.verify_training_moment(&root, &leaf, &[]));
    }

    #[test]
    fn test_pipeline_closed_model_rejected_when_jlens_required() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        // Simulate a closed-model agent (GPT-4) — no J-Lens, no trace
        let v = VerdictWithProvenance {
            verdict: TrustVerdict::Green,
            robot_id: "dog-1".to_string(),
            batch_merkle_root: "a".repeat(64),
            anchor_block_height: 100,
            consensus_ratio: 0.9,
            matching_operators: 9,
            diverging_operators: 1,
            violated_invariants: vec![],
            source: VerdictSource {
                agent_pubkey: "npub1gpt4user".to_string(),
                buzz_event_id: "evt_002".to_string(),
                akash_lease_id: None,
                brainmaxx_trace_ref: None,
                model_id: "gpt-4".to_string(),
                jlens_probe_active: false,
                tee_attestation_hash: None,
            },
        };
        let result = pipeline.consume_verdict(v);
        assert!(result.is_err(), "closed-model verdict should be rejected");
        assert!(result.unwrap_err().contains("J-Lens"),
            "error should mention J-Lens");
    }

    #[test]
    fn test_pipeline_provenance_preserved() {
        let mut pipeline = BuzzAkashPipeline::new(
            test_envelope(),
            LearningConfig::default(),
            true,
        );
        let v = make_verdict(TrustVerdict::Yellow, true, true);
        pipeline.consume_verdict(v.clone()).unwrap();
        let history = pipeline.verdict_history();
        assert_eq!(history[0].source.model_id, "qwen2.5-7b");
        assert_eq!(history[0].source.agent_pubkey, "npub1abc123def456");
        assert!(history[0].source.akash_lease_id.is_some());
        assert!(history[0].source.brainmaxx_trace_ref.is_some());
    }
}
