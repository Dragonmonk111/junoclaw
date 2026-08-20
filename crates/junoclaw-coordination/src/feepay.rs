//! FeePay pool monitoring — tracks FeePay pool balances, registered wallets,
//! and usage for the fleet coordinator's REST API.
//!
//! This module does NOT interact with the FeePay module directly. It caches
//! pool state queried from the chain (via LCD/RPC) and provides alerting
//! thresholds for the fleet dashboard.

use std::collections::HashMap;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

/// Alert level for a FeePay pool.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    Green,
    Yellow,
    Red,
    Critical,
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertLevel::Green => write!(f, "green"),
            AlertLevel::Yellow => write!(f, "yellow"),
            AlertLevel::Red => write!(f, "red"),
            AlertLevel::Critical => write!(f, "critical"),
        }
    }
}

/// FeePay pool state for a single registered contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeePayPoolState {
    /// Contract address registered with FeePay
    pub contract: String,
    /// Current pool balance (in ujuno)
    pub balance: u128,
    /// Recommended pool size (in ujuno) — based on fleet scale
    pub recommended: u128,
    /// Registered wallet addresses with per-wallet limits
    pub registered_wallets: Vec<RegisteredWallet>,
    /// Total transactions sponsored this epoch
    pub txs_this_epoch: u64,
    /// Epoch number
    pub epoch: u64,
    /// Last updated timestamp (ms)
    pub last_updated: u64,
}

/// A wallet registered with FeePay for a specific contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisteredWallet {
    pub address: String,
    /// Max txs per epoch for this wallet
    pub limit: u64,
    /// Txs used this epoch
    pub used: u64,
}

/// Funding/withdrawal history entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolHistoryEntry {
    pub tx_hash: String,
    pub contract: String,
    /// "fund" or "withdraw"
    pub action: String,
    /// Amount in ujuno
    pub amount: u128,
    pub timestamp: u64,
    pub from: String,
}

/// Alert for a FeePay pool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeePayAlert {
    pub contract: String,
    pub level: AlertLevel,
    pub balance: u128,
    pub recommended: u128,
    pub percentage: f64,
    pub message: String,
}

impl FeePayPoolState {
    /// Compute the alert level for this pool.
    pub fn alert_level(&self) -> AlertLevel {
        if self.recommended == 0 {
            return AlertLevel::Green;
        }
        let pct = self.balance as f64 / self.recommended as f64;
        if self.balance == 0 {
            AlertLevel::Critical
        } else if pct < 0.2 {
            AlertLevel::Red
        } else if pct < 0.5 {
            AlertLevel::Yellow
        } else {
            AlertLevel::Green
        }
    }

    /// Compute percentage of recommended.
    pub fn percentage(&self) -> f64 {
        if self.recommended == 0 {
            return 100.0;
        }
        (self.balance as f64 / self.recommended as f64) * 100.0
    }
}

/// FeePay monitor — tracks pool state for multiple contracts.
pub struct FeePayMonitor {
    pools: Mutex<HashMap<String, FeePayPoolState>>,
    history: Mutex<Vec<PoolHistoryEntry>>,
}

impl FeePayMonitor {
    pub fn new() -> Self {
        Self {
            pools: Mutex::new(HashMap::new()),
            history: Mutex::new(Vec::new()),
        }
    }

    /// Register or update a pool's state.
    pub async fn update_pool(&self, state: FeePayPoolState) {
        let contract = state.contract.clone();
        let mut pools = self.pools.lock().await;
        pools.insert(contract, state);
    }

    /// Get a pool's state.
    pub async fn get_pool(&self, contract: &str) -> Option<FeePayPoolState> {
        let pools = self.pools.lock().await;
        pools.get(contract).cloned()
    }

    /// Get all pool states.
    pub async fn all_pools(&self) -> Vec<FeePayPoolState> {
        let pools = self.pools.lock().await;
        pools.values().cloned().collect()
    }

    /// Add a history entry.
    pub async fn add_history(&self, entry: PoolHistoryEntry) {
        let mut history = self.history.lock().await;
        history.push(entry);
        // Keep last 1000 entries
        if history.len() > 1000 {
            let drain = history.len() - 1000;
            history.drain(0..drain);
        }
    }

    /// Get history for a specific contract.
    pub async fn history_for(&self, contract: &str) -> Vec<PoolHistoryEntry> {
        let history = self.history.lock().await;
        history.iter().filter(|h| h.contract == contract).cloned().collect()
    }

    /// Get all active alerts across all pools.
    pub async fn alerts(&self) -> Vec<FeePayAlert> {
        let pools = self.pools.lock().await;
        pools
            .values()
            .filter(|p| p.alert_level() != AlertLevel::Green)
            .map(|p| {
                let level = p.alert_level();
                let pct = p.percentage();
                let message = match level {
                    AlertLevel::Critical => format!(
                        "Pool exhausted for {} — robot operators must pay own gas until refill",
                        p.contract
                    ),
                    AlertLevel::Red => format!(
                        "Pool critically low for {} — {:.1}% of recommended. Refill immediately.",
                        p.contract, pct
                    ),
                    AlertLevel::Yellow => format!(
                        "Pool low for {} — {:.1}% of recommended. Plan refill soon.",
                        p.contract, pct
                    ),
                    AlertLevel::Green => String::new(),
                };
                FeePayAlert {
                    contract: p.contract.clone(),
                    level,
                    balance: p.balance,
                    recommended: p.recommended,
                    percentage: pct,
                    message,
                }
            })
            .collect()
    }

    /// Check if new robot onboarding should be paused due to low pools.
    pub async fn should_pause_onboarding(&self) -> bool {
        let pools = self.pools.lock().await;
        pools.values().any(|p| {
            p.alert_level() == AlertLevel::Red || p.alert_level() == AlertLevel::Critical
        })
    }
}

impl Default for FeePayMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pool(contract: &str, balance: u128, recommended: u128) -> FeePayPoolState {
        FeePayPoolState {
            contract: contract.to_string(),
            balance,
            recommended,
            registered_wallets: vec![],
            txs_this_epoch: 0,
            epoch: 1,
            last_updated: 1000,
        }
    }

    #[tokio::test]
    async fn test_alert_green() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 80_000, 100_000)).await;
        let alerts = mon.alerts().await;
        assert!(alerts.is_empty(), "no alerts when pool > 50%");
    }

    #[tokio::test]
    async fn test_alert_yellow() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 35_000, 100_000)).await;
        let alerts = mon.alerts().await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Yellow);
    }

    #[tokio::test]
    async fn test_alert_red() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 15_000, 100_000)).await;
        let alerts = mon.alerts().await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Red);
    }

    #[tokio::test]
    async fn test_alert_critical() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 0, 100_000)).await;
        let alerts = mon.alerts().await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Critical);
    }

    #[tokio::test]
    async fn test_pause_onboarding() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 10_000, 100_000)).await;
        assert!(mon.should_pause_onboarding().await);

        mon.update_pool(make_pool("juno1...merkle", 80_000, 100_000)).await;
        assert!(!mon.should_pause_onboarding().await);
    }

    #[tokio::test]
    async fn test_history() {
        let mon = FeePayMonitor::new();
        mon.add_history(PoolHistoryEntry {
            tx_hash: "abc".to_string(),
            contract: "juno1...merkle".to_string(),
            action: "fund".to_string(),
            amount: 10_000,
            timestamp: 1000,
            from: "juno1...operator".to_string(),
        }).await;
        mon.add_history(PoolHistoryEntry {
            tx_hash: "def".to_string(),
            contract: "juno1...zk".to_string(),
            action: "fund".to_string(),
            amount: 5_000,
            timestamp: 2000,
            from: "juno1...operator".to_string(),
        }).await;

        let hist = mon.history_for("juno1...merkle").await;
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].amount, 10_000);
    }

    #[tokio::test]
    async fn test_multiple_pools() {
        let mon = FeePayMonitor::new();
        mon.update_pool(make_pool("juno1...merkle", 80_000, 100_000)).await;
        mon.update_pool(make_pool("juno1...zk", 10_000, 50_000)).await;
        mon.update_pool(make_pool("juno1...moult", 0, 20_000)).await;

        let alerts = mon.alerts().await;
        assert_eq!(alerts.len(), 2); // zk (red) + moult (critical)

        let all = mon.all_pools().await;
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_percentage() {
        let pool = make_pool("juno1...merkle", 25_000, 100_000);
        assert!((pool.percentage() - 25.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_zero_recommended() {
        let pool = make_pool("juno1...merkle", 0, 0);
        assert_eq!(pool.alert_level(), AlertLevel::Green);
        assert!((pool.percentage() - 100.0).abs() < 0.01);
    }
}
