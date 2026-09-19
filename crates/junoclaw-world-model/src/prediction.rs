use serde::{Deserialize, Serialize};

/// A prediction for what happens if an action is taken in a given state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Predicted outcome description
    pub outcome: String,
    /// Confidence score (0.0–1.0)
    pub confidence: Confidence,
    /// Whether the predicted outcome is safe
    pub safe: bool,
    /// Number of similar transitions found
    pub evidence_count: u32,
    /// The candidate action that was queried
    pub action: String,
    /// The state that was queried
    pub state: String,
}

/// Confidence level for a prediction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Confidence(pub f32);

impl Confidence {
    pub fn high() -> Self { Self(0.8) }
    pub fn medium() -> Self { Self(0.5) }
    pub fn low() -> Self { Self(0.2) }
    pub fn none() -> Self { Self(0.0) }

    pub fn value(&self) -> f32 { self.0 }
    pub fn is_reliable(&self) -> bool { self.0 >= 0.5 }
}

/// Distribution over possible outcomes for a given (state, action) pair.
/// When multiple transitions are found with different outcomes, this
/// captures the full distribution rather than just the most likely outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeDistribution {
    /// (outcome_description, probability) pairs, sorted by probability descending
    pub outcomes: Vec<(String, f32)>,
    /// Whether all outcomes in the distribution are safe
    pub all_safe: bool,
    /// Total evidence count across all outcomes
    pub total_count: u32,
}

impl OutcomeDistribution {
    pub fn most_likely(&self) -> Option<&(String, f32)> {
        self.outcomes.first()
    }

    pub fn is_certain(&self) -> bool {
        self.outcomes.len() == 1 && self.outcomes[0].1 >= 0.95
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_levels() {
        assert!(Confidence::high().is_reliable());
        assert!(Confidence::medium().is_reliable());
        assert!(!Confidence::low().is_reliable());
        assert!(!Confidence::none().is_reliable());
    }

    #[test]
    fn test_outcome_distribution() {
        let dist = OutcomeDistribution {
            outcomes: vec![
                ("moved 0.3m".to_string(), 0.7),
                ("moved 0.5m".to_string(), 0.3),
            ],
            all_safe: true,
            total_count: 10,
        };
        assert!(!dist.is_certain());
        assert_eq!(dist.most_likely().unwrap().0, "moved 0.3m");
    }
}
