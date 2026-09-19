use std::collections::HashMap;
use parking_lot::RwLock;
use crate::transition::{Transition, TransitionId};
use crate::prediction::{Prediction, OutcomeDistribution, Confidence};
use junoclaw_memory::MemoryIndex;

/// Configuration for the world model.
#[derive(Debug, Clone)]
pub struct WorldModelConfig {
    /// Minimum similarity score to include a transition in the prediction
    pub min_similarity: f32,
    /// Maximum number of transitions to consider (top-k)
    pub top_k: usize,
    /// Minimum evidence count for a high-confidence prediction
    pub min_evidence: u32,
}

impl Default for WorldModelConfig {
    fn default() -> Self {
        Self {
            min_similarity: 0.5,
            top_k: 10,
            min_evidence: 3,
        }
    }
}

/// L2 World Model — predicts consequences of candidate actions.
///
/// The model is trained on Merkle-verified transitions from the L1 Memory
/// Index. When queried with (state, action), it finds the most similar
/// past transitions and returns a prediction with confidence.
///
/// Cold-start: uses nearest-neighbor lookup (no training required).
/// Warm-start: would use a trained model (MLP/transformer) on the
/// transition dataset.
pub struct WorldModel {
    /// All known transitions, indexed by TransitionId
    transitions: RwLock<HashMap<TransitionId, Transition>>,
    /// Index: state_desc → list of transition IDs
    by_state: RwLock<HashMap<String, Vec<TransitionId>>>,
    /// Configuration
    config: WorldModelConfig,
}

impl WorldModel {
    pub fn new(config: WorldModelConfig) -> Self {
        Self {
            transitions: RwLock::new(HashMap::new()),
            by_state: RwLock::new(HashMap::new()),
            config,
        }
    }

    pub fn with_default_config() -> Self {
        Self::new(WorldModelConfig::default())
    }

    /// Ingest memory entries from the L1 Memory Index.
    /// Each entry becomes a transition. Entries with the same (state, action)
    /// are merged, incrementing the count.
    pub fn ingest_from_memory(&self, memory: &MemoryIndex) {
        let entries = memory.recent(memory.len());
        for entry in entries {
            self.ingest_entry(&entry);
        }
    }

    /// Ingest a single memory entry as a transition.
    pub fn ingest_entry(&self, entry: &junoclaw_memory::MemoryEntry) {
        let transition = Transition::from_memory(entry);
        let mut transitions = self.transitions.write();
        let mut by_state = self.by_state.write();

        let id = transition.id.clone();
        let state_desc = transition.state_desc.clone();

        if let Some(existing) = transitions.get_mut(&id) {
            existing.merge(&transition);
        } else {
            by_state.entry(state_desc).or_default().push(id.clone());
            transitions.insert(id, transition);
        }
    }

    /// Predict the outcome of taking `action` in `state`.
    ///
    /// This is the core "imagine" function: the robot asks "what happens
    /// if I do X?" and gets back a prediction with confidence.
    pub fn predict(&self, state: &str, action: &str) -> Prediction {
        let transitions = self.transitions.read();
        let by_state = self.by_state.read();

        // 1. Find exact matches: same state + same action
        let exact: Vec<&Transition> = by_state
            .get(state)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| {
                        let t = transitions.get(id)?;
                        if t.action_cmd == action { Some(t) } else { None }
                    })
                    .collect()
            })
            .unwrap_or_default();

        if !exact.is_empty() {
            let total_count: u32 = exact.iter().map(|t| t.count).sum();
            let best = exact.iter().max_by_key(|t| t.count).unwrap();
            let confidence = if total_count >= self.config.min_evidence {
                Confidence::high()
            } else {
                Confidence::medium()
            };
            return Prediction {
                outcome: best.outcome_desc.clone(),
                confidence,
                safe: best.safe,
                evidence_count: total_count,
                action: action.to_string(),
                state: state.to_string(),
            };
        }

        // 2. Find similar transitions (nearest neighbor)
        let similar: Vec<(&Transition, f32)> = by_state
            .keys()
            .filter_map(|s| {
                let sim = s_similarity(s, state);
                if sim >= self.config.min_similarity {
                    by_state.get(s).map(|ids| {
                        ids.iter().filter_map(|id| transitions.get(id)).map(|t| (t, sim)).collect::<Vec<_>>()
                    })
                } else {
                    None
                }
            })
            .flatten()
            .filter(|(t, _)| t.action_cmd == action)
            .take(self.config.top_k)
            .collect();

        if similar.is_empty() {
            return Prediction {
                outcome: "unknown".to_string(),
                confidence: Confidence::none(),
                safe: false,
                evidence_count: 0,
                action: action.to_string(),
                state: state.to_string(),
            };
        }

        // Weighted prediction based on similarity
        let total_weight: f32 = similar.iter().map(|(_, sim)| sim).sum();
        let total_count: u32 = similar.iter().map(|(t, _)| t.count).sum();
        let best = similar.iter().max_by(|a, b| {
            (a.1 * a.0.count as f32).partial_cmp(&(b.1 * b.0.count as f32)).unwrap()
        }).unwrap();

        let confidence_value = (best.1 * (total_count as f32 / self.config.min_evidence as f32)).min(1.0);
        let confidence = Confidence(confidence_value);

        Prediction {
            outcome: best.0.outcome_desc.clone(),
            confidence,
            safe: best.0.safe,
            evidence_count: total_count,
            action: action.to_string(),
            state: state.to_string(),
        }
    }

    /// Get the full outcome distribution for (state, action).
    pub fn predict_distribution(&self, state: &str, action: &str) -> OutcomeDistribution {
        let transitions = self.transitions.read();
        let by_state = self.by_state.read();

        let matching: Vec<&Transition> = by_state
            .get(state)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| {
                        let t = transitions.get(id)?;
                        if t.action_cmd == action { Some(t) } else { None }
                    })
                    .collect()
            })
            .unwrap_or_default();

        if matching.is_empty() {
            return OutcomeDistribution {
                outcomes: vec![("unknown".to_string(), 1.0)],
                all_safe: false,
                total_count: 0,
            };
        }

        let total: u32 = matching.iter().map(|t| t.count).sum();
        let mut outcome_map: HashMap<String, (f32, bool, u32)> = HashMap::new();

        for t in &matching {
            let prob = t.count as f32 / total as f32;
            let entry = outcome_map.entry(t.outcome_desc.clone()).or_insert((0.0, true, 0));
            entry.0 += prob;
            entry.1 &= t.safe;
            entry.2 += t.count;
        }

        let mut outcomes: Vec<(String, f32)> = outcome_map
            .into_iter()
            .map(|(desc, (prob, safe, _))| (desc, prob))
            .collect();
        outcomes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let all_safe = matching.iter().all(|t| t.safe);

        OutcomeDistribution {
            outcomes,
            all_safe,
            total_count: total,
        }
    }

    /// Get the number of known transitions.
    pub fn len(&self) -> usize {
        self.transitions.read().len()
    }

    /// Check if the model has any transitions.
    pub fn is_empty(&self) -> bool {
        self.transitions.read().is_empty()
    }

    /// Get all known actions for a given state.
    pub fn actions_for_state(&self, state: &str) -> Vec<String> {
        let transitions = self.transitions.read();
        let by_state = self.by_state.read();
        by_state
            .get(state)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| transitions.get(id).map(|t| t.action_cmd.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Simple string similarity for state matching.
fn s_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.contains(b) || b.contains(a) {
        return 0.7;
    }
    // Simple word overlap
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let overlap = words_a.intersection(&words_b).count();
    let total = words_a.union(&words_b).count();
    if total == 0 {
        return 0.0;
    }
    overlap as f32 / total as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use junoclaw_memory::MemoryEntry;

    #[test]
    fn test_predict_exact_match() {
        let model = WorldModel::with_default_config();
        for _ in 0..5 {
            model.ingest_entry(&MemoryEntry::new("robot-1", "standing", "walk-forward", "moved 0.3m"));
        }
        let pred = model.predict("standing", "walk-forward");
        assert_eq!(pred.outcome, "moved 0.3m");
        assert!(pred.confidence.is_reliable());
        assert!(pred.safe);
        assert_eq!(pred.evidence_count, 5);
    }

    #[test]
    fn test_predict_no_data() {
        let model = WorldModel::with_default_config();
        let pred = model.predict("standing", "walk-forward");
        assert_eq!(pred.outcome, "unknown");
        assert!(!pred.confidence.is_reliable());
        assert!(!pred.safe);
    }

    #[test]
    fn test_predict_distribution() {
        let model = WorldModel::with_default_config();
        for _ in 0..7 {
            model.ingest_entry(&MemoryEntry::new("robot-1", "standing", "walk", "moved 0.3m"));
        }
        for _ in 0..3 {
            model.ingest_entry(&MemoryEntry::new("robot-1", "standing", "walk", "moved 0.5m"));
        }
        let dist = model.predict_distribution("standing", "walk");
        assert_eq!(dist.outcomes.len(), 2);
        assert_eq!(dist.total_count, 10);
        assert!(dist.all_safe);
    }

    #[test]
    fn test_ingest_from_memory() {
        let memory = MemoryIndex::new();
        memory.insert(MemoryEntry::new("robot-1", "standing", "walk", "moved"));
        memory.insert(MemoryEntry::new("robot-1", "walking", "halt", "stopped"));

        let model = WorldModel::with_default_config();
        model.ingest_from_memory(&memory);
        assert_eq!(model.len(), 2);

        let actions = model.actions_for_state("standing");
        assert_eq!(actions, vec!["walk"]);
    }

    #[test]
    fn test_similar_state_prediction() {
        let model = WorldModel::with_default_config();
        for _ in 0..5 {
            model.ingest_entry(&MemoryEntry::new("robot-1", "standing still", "walk", "moved 0.3m"));
        }
        // Query with similar but not exact state
        let pred = model.predict("standing", "walk");
        assert_ne!(pred.outcome, "unknown");
        assert!(pred.evidence_count > 0);
    }
}
