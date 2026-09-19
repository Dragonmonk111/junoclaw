use serde::{Deserialize, Serialize};
use junoclaw_memory::MemoryEntry;

/// A transition is a (state, action) → outcome tuple, extracted from
/// Merkle-verified memory entries in the L1 Memory Index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    pub id: TransitionId,
    pub robot_id: String,
    pub state_desc: String,
    pub action_cmd: String,
    pub outcome_desc: String,
    pub safe: bool,
    /// Number of times this transition has been observed
    pub count: u32,
    /// Source memory entry IDs that produced this transition
    pub source_entries: Vec<String>,
}

/// Transition identifier (hash of state + action for deduplication)
pub type TransitionId = String;

impl Transition {
    /// Extract a transition from a memory entry.
    pub fn from_memory(entry: &MemoryEntry) -> Self {
        let state_desc = entry.state.description.clone();
        let action_cmd = entry.action.command.clone();
        let outcome_desc = entry.outcome.description.clone();
        let id = format!("{}|{}|{}", state_desc, action_cmd, outcome_desc);

        Self {
            id,
            robot_id: entry.robot_id.clone(),
            state_desc,
            action_cmd,
            outcome_desc,
            safe: entry.outcome.safe,
            count: 1,
            source_entries: vec![entry.id.clone()],
        }
    }

    /// Merge another transition into this one (same state + action).
    pub fn merge(&mut self, other: &Transition) {
        self.count += other.count;
        self.source_entries.extend(other.source_entries.clone());
        // Keep the most recent outcome
        if other.count > 0 {
            self.outcome_desc = other.outcome_desc.clone();
            self.safe = other.safe;
        }
    }

    /// Check if this transition matches a state + action query.
    pub fn matches(&self, state_desc: &str, action_cmd: &str) -> bool {
        self.state_desc == state_desc && self.action_cmd == action_cmd
    }

    /// Similarity score between this transition's state and a query state.
    /// Returns 0.0–1.0. Currently uses simple string equality (1.0 or 0.0),
    /// but this is where semantic similarity (embedding distance) would go.
    pub fn state_similarity(&self, query: &str) -> f32 {
        if self.state_desc == query {
            1.0
        } else if self.state_desc.contains(query) || query.contains(&self.state_desc) {
            0.5
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_memory() {
        let entry = MemoryEntry::new("robot-1", "standing", "walk-forward", "moved 0.3m");
        let t = Transition::from_memory(&entry);
        assert_eq!(t.state_desc, "standing");
        assert_eq!(t.action_cmd, "walk-forward");
        assert_eq!(t.outcome_desc, "moved 0.3m");
        assert!(t.safe);
        assert_eq!(t.count, 1);
    }

    #[test]
    fn test_merge() {
        let entry1 = MemoryEntry::new("robot-1", "standing", "walk", "moved 0.3m");
        let entry2 = MemoryEntry::new("robot-1", "standing", "walk", "moved 0.5m");
        let mut t1 = Transition::from_memory(&entry1);
        let t2 = Transition::from_memory(&entry2);
        t1.merge(&t2);
        assert_eq!(t1.count, 2);
        assert_eq!(t1.source_entries.len(), 2);
    }

    #[test]
    fn test_state_similarity() {
        let entry = MemoryEntry::new("robot-1", "standing", "walk", "moved");
        let t = Transition::from_memory(&entry);
        assert_eq!(t.state_similarity("standing"), 1.0);
        assert_eq!(t.state_similarity("stand"), 0.5);
        assert_eq!(t.state_similarity("fallen"), 0.0);
    }
}
