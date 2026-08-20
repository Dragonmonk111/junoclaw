//! Fleet coordinator — multi-robot intent aggregation, routing, and breaker
//! distribution.
//!
//! The fleet coordinator sits between multiple robot controllers and the
//! consensus engine. It:
//!
//! 1. **Aggregates** intents from multiple robots into a single submission
//!    stream to the consensus engine, batching them by block boundary.
//! 2. **Routes** breaker actions back to specific robots after consensus
//!    finalization, maintaining per-robot breaker state.
//! 3. **Tracks** fleet status — which robots are operational, locked, or
//!    in safe-hold — and exposes this via REST endpoints.
//! 4. **Throttles** per-robot intent submission rate to prevent flooding.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::message::{
    AgentMessage, BreakerAction, CircuitBreakerState,
};

/// Configuration for the fleet coordinator.
#[derive(Clone, Debug)]
pub struct FleetConfig {
    /// Maximum intents per robot per second (rate limiting)
    pub max_intents_per_robot_per_sec: usize,
    /// Maximum robots in the fleet
    pub max_fleet_size: usize,
    /// How long to wait before aggregating a batch (ms)
    pub aggregation_window_ms: u64,
}

impl Default for FleetConfig {
    fn default() -> Self {
        Self {
            max_intents_per_robot_per_sec: 10,
            max_fleet_size: 1000,
            aggregation_window_ms: 250,
        }
    }
}

/// Per-robot tracking state.
#[derive(Clone, Debug)]
pub struct RobotState {
    /// Robot ID
    pub robot_id: String,
    /// Current circuit breaker state
    pub breaker_state: CircuitBreakerState,
    /// Number of intents submitted (total)
    pub total_intents: u64,
    /// Number of intents blocked by gate
    pub blocked_intents: u64,
    /// Number of breaker trips
    pub trip_count: u64,
    /// Last intent timestamp (ms)
    pub last_intent_at: u64,
    /// Whether the robot is currently registered with the fleet
    pub registered: bool,
}

/// Fleet status snapshot for REST API.
#[derive(Clone, Debug, serde::Serialize)]
pub struct FleetStatus {
    pub total_robots: usize,
    pub operational: usize,
    pub locked: usize,
    pub total_intents: u64,
    pub total_blocked: u64,
    pub total_trips: u64,
}

/// Per-robot status entry for REST API.
#[derive(Clone, Debug, serde::Serialize)]
pub struct RobotStatusEntry {
    pub robot_id: String,
    pub breaker_state: String,
    pub total_intents: u64,
    pub blocked_intents: u64,
    pub trip_count: u64,
    pub last_intent_at: u64,
}

/// Rate limiter entry.
struct RateLimitEntry {
    count: usize,
    window_start: Instant,
}

/// The fleet coordinator — aggregates multi-robot intents, routes breaker
/// actions, and tracks fleet status.
pub struct FleetCoordinator {
    config: FleetConfig,
    /// Per-robot state
    robots: Mutex<HashMap<String, RobotState>>,
    /// Rate limiter per robot
    rate_limits: Mutex<HashMap<String, RateLimitEntry>>,
    /// Pending intents waiting for aggregation window
    pending_intents: Mutex<Vec<AgentMessage>>,
    /// Aggregation window deadline
    window_deadline: Mutex<Option<Instant>>,
}

impl FleetCoordinator {
    /// Create a new fleet coordinator.
    pub fn new(config: FleetConfig) -> Self {
        Self {
            config,
            robots: Mutex::new(HashMap::new()),
            rate_limits: Mutex::new(HashMap::new()),
            pending_intents: Mutex::new(Vec::new()),
            window_deadline: Mutex::new(None),
        }
    }

    /// Register a robot with the fleet.
    pub async fn register_robot(&self, robot_id: &str) -> Result<()> {
        let mut robots = self.robots.lock().await;
        if robots.len() >= self.config.max_fleet_size && !robots.contains_key(robot_id) {
            return Err(anyhow::anyhow!(
                "fleet at capacity (max {})",
                self.config.max_fleet_size
            ));
        }
        robots.insert(
            robot_id.to_string(),
            RobotState {
                robot_id: robot_id.to_string(),
                breaker_state: CircuitBreakerState::Closed,
                total_intents: 0,
                blocked_intents: 0,
                trip_count: 0,
                last_intent_at: 0,
                registered: true,
            },
        );
        info!("Registered robot {} with fleet (size={})", robot_id, robots.len());
        Ok(())
    }

    /// Deregister a robot from the fleet.
    pub async fn deregister_robot(&self, robot_id: &str) -> Result<()> {
        let mut robots = self.robots.lock().await;
        if let Some(state) = robots.get_mut(robot_id) {
            state.registered = false;
            info!("Deregistered robot {} from fleet", robot_id);
        }
        Ok(())
    }

    /// Submit an intent from a robot. Returns Ok(Some(messages)) when the
    /// aggregation window expires and messages are ready for consensus,
    /// or Ok(None) if the window is still open.
    ///
    /// Checks:
    /// 1. Robot is registered
    /// 2. Robot's breaker is not tripped
    /// 3. Rate limit is not exceeded
    pub async fn submit_intent(
        &self,
        robot_id: &str,
        msg: AgentMessage,
    ) -> Result<Option<Vec<AgentMessage>>> {
        // 1. Check registration and breaker state
        {
            let robots = self.robots.lock().await;
            let state = robots.get(robot_id).ok_or_else(|| {
                anyhow::anyhow!("robot {} not registered with fleet", robot_id)
            })?;
            if !state.registered {
                return Err(anyhow::anyhow!("robot {} not registered", robot_id));
            }
            if state.breaker_state.is_tripped() {
                return Err(anyhow::anyhow!(
                    "robot {} breaker is tripped — intent rejected",
                    robot_id
                ));
            }
        }

        // 2. Rate limit check
        {
            let mut limits = self.rate_limits.lock().await;
            let now = Instant::now();
            let entry = limits.entry(robot_id.to_string()).or_insert(RateLimitEntry {
                count: 0,
                window_start: now,
            });

            // Reset window if 1 second has passed
            if now.duration_since(entry.window_start) >= Duration::from_secs(1) {
                entry.count = 0;
                entry.window_start = now;
            }

            if entry.count >= self.config.max_intents_per_robot_per_sec {
                return Err(anyhow::anyhow!(
                    "robot {} rate limit exceeded ({} intents/sec)",
                    robot_id,
                    self.config.max_intents_per_robot_per_sec
                ));
            }
            entry.count += 1;
        }

        // 3. Update robot state
        {
            let mut robots = self.robots.lock().await;
            if let Some(state) = robots.get_mut(robot_id) {
                state.total_intents += 1;
                state.last_intent_at = msg.timestamp;
            }
        }

        // 4. Add to pending and check aggregation window
        let ready = {
            let mut pending = self.pending_intents.lock().await;
            let mut deadline = self.window_deadline.lock().await;

            pending.push(msg);

            // Start window if not started
            if deadline.is_none() {
                *deadline = Some(Instant::now() + Duration::from_millis(
                    self.config.aggregation_window_ms,
                ));
            }

            // Check if window expired
            if let Some(dl) = *deadline {
                if Instant::now() >= dl {
                    let messages = pending.drain(..).collect::<Vec<_>>();
                    *deadline = None;
                    Some(messages)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(ref msgs) = ready {
            info!(
                "Fleet aggregation window expired: {} intents ready for consensus",
                msgs.len()
            );
        }

        Ok(ready)
    }

    /// Force-flush pending intents (for shutdown or immediate consensus).
    pub async fn flush_pending(&self) -> Option<Vec<AgentMessage>> {
        let mut pending = self.pending_intents.lock().await;
        let mut deadline = self.window_deadline.lock().await;
        if pending.is_empty() {
            return None;
        }
        let messages = pending.drain(..).collect::<Vec<_>>();
        *deadline = None;
        Some(messages)
    }

    /// Apply breaker actions from a finalized batch. Updates per-robot
    /// breaker state and returns the list of affected robot IDs.
    pub async fn apply_breaker_actions(
        &self,
        actions: &[BreakerAction],
    ) -> Vec<String> {
        let mut affected = Vec::new();
        let mut robots = self.robots.lock().await;

        for action in actions {
            let state = robots.entry(action.robot_id.clone()).or_insert(RobotState {
                robot_id: action.robot_id.clone(),
                breaker_state: CircuitBreakerState::Closed,
                total_intents: 0,
                blocked_intents: 0,
                trip_count: 0,
                last_intent_at: 0,
                registered: true,
            });

            state.breaker_state = CircuitBreakerState::Tripped {
                reason: action.reason.clone(),
                tripped_at: action.emitted_at,
                cause_ref: action.cause_ref.clone(),
            };
            state.trip_count += 1;

            warn!(
                "Fleet: breaker tripped for robot {} (trips={}, cause={})",
                action.robot_id, state.trip_count, action.cause_ref
            );

            affected.push(action.robot_id.clone());
        }

        affected
    }

    /// Record that a robot's intent was blocked by the gate.
    pub async fn record_blocked(&self, robot_id: &str) {
        let mut robots = self.robots.lock().await;
        if let Some(state) = robots.get_mut(robot_id) {
            state.blocked_intents += 1;
        }
    }

    /// Reset a robot's circuit breaker (governance or operator action).
    pub async fn reset_breaker(&self, robot_id: &str, reset_by: &str) -> Result<()> {
        let mut robots = self.robots.lock().await;
        let state = robots.get_mut(robot_id).ok_or_else(|| {
            anyhow::anyhow!("robot {} not found in fleet", robot_id)
        })?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        state.breaker_state = CircuitBreakerState::Reset {
            reset_at: now,
            reset_by: reset_by.to_string(),
        };

        info!(
            "Fleet: breaker reset for robot {} by {}",
            robot_id, reset_by
        );

        Ok(())
    }

    /// Get a robot's current state.
    pub async fn get_robot_state(&self, robot_id: &str) -> Option<RobotState> {
        let robots = self.robots.lock().await;
        robots.get(robot_id).cloned()
    }

    /// Get fleet status snapshot.
    pub async fn fleet_status(&self) -> FleetStatus {
        let robots = self.robots.lock().await;
        let mut operational = 0;
        let mut locked = 0;
        let mut total_intents = 0u64;
        let mut total_blocked = 0u64;
        let mut total_trips = 0u64;

        for state in robots.values() {
            if state.breaker_state.is_closed() {
                operational += 1;
            } else if state.breaker_state.is_tripped() {
                locked += 1;
            }
            total_intents += state.total_intents;
            total_blocked += state.blocked_intents;
            total_trips += state.trip_count;
        }

        FleetStatus {
            total_robots: robots.len(),
            operational,
            locked,
            total_intents,
            total_blocked,
            total_trips,
        }
    }

    /// Get per-robot status entries for REST API.
    pub async fn robot_status_list(&self) -> Vec<RobotStatusEntry> {
        let robots = self.robots.lock().await;
        robots
            .values()
            .map(|state| RobotStatusEntry {
                robot_id: state.robot_id.clone(),
                breaker_state: format!("{:?}", state.breaker_state),
                total_intents: state.total_intents,
                blocked_intents: state.blocked_intents,
                trip_count: state.trip_count,
                last_intent_at: state.last_intent_at,
            })
            .collect()
    }

    /// Get the number of pending intents waiting for aggregation.
    pub async fn pending_count(&self) -> usize {
        self.pending_intents.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::IntentMessage;

    fn make_intent(robot_id: &str) -> AgentMessage {
        let intent = IntentMessage {
            robot_id: robot_id.to_string(),
            action: "navigate".to_string(),
            params: serde_json::json!({"x": 100, "y": 200}),
            sensor_snapshot_hash: "abc123".to_string(),
            controller_timestamp: 1000,
            rationale: Some("test".to_string()),
            execution_proof_ref: Some("proof1".to_string()),
        };
        intent.into_agent_message(vec![1; 32], vec![], 1000).unwrap()
    }

    #[tokio::test]
    async fn test_register_and_submit() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();

        let result = fc.submit_intent("robot-1", make_intent("robot-1")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_unregistered_robot_rejected() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        let result = fc.submit_intent("unknown", make_intent("unknown")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not registered"));
    }

    #[tokio::test]
    async fn test_breaker_tripped_blocks_intent() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();

        // Trip the breaker
        let action = BreakerAction {
            robot_id: "robot-1".to_string(),
            reason: "test violation".to_string(),
            cause_ref: "batch:1:test".to_string(),
            batch_height: 1,
            emitted_at: 1000,
        };
        fc.apply_breaker_actions(&[action]).await;

        // Intent should be rejected
        let result = fc.submit_intent("robot-1", make_intent("robot-1")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("breaker is tripped"));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let fc = FleetCoordinator::new(FleetConfig {
            max_intents_per_robot_per_sec: 2,
            max_fleet_size: 100,
            aggregation_window_ms: 10000, // long window so we don't trigger flush
        });
        fc.register_robot("robot-1").await.unwrap();

        // First two should succeed
        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();

        // Third should be rate limited
        let result = fc.submit_intent("robot-1", make_intent("robot-1")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rate limit"));
    }

    #[tokio::test]
    async fn test_aggregation_window() {
        let fc = FleetCoordinator::new(FleetConfig {
            max_intents_per_robot_per_sec: 100,
            max_fleet_size: 100,
            aggregation_window_ms: 10, // very short window
        });
        fc.register_robot("robot-1").await.unwrap();
        fc.register_robot("robot-2").await.unwrap();

        // Submit first intent — starts the window
        let r1 = fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        assert!(r1.is_none()); // window not expired yet

        // Submit second intent
        let r2 = fc.submit_intent("robot-2", make_intent("robot-2")).await.unwrap();
        assert!(r2.is_none());

        // Wait for window to expire
        tokio::time::sleep(Duration::from_millis(15)).await;

        // Next intent should trigger flush of pending + new intent
        let r3 = fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        assert!(r3.is_some());
        let msgs = r3.unwrap();
        assert_eq!(msgs.len(), 3); // 2 pending + 1 new
    }

    #[tokio::test]
    async fn test_flush_pending() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();

        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();

        let pending = fc.flush_pending().await;
        assert!(pending.is_some());
        assert_eq!(pending.unwrap().len(), 2);

        // Second flush should be empty
        let pending2 = fc.flush_pending().await;
        assert!(pending2.is_none());
    }

    #[tokio::test]
    async fn test_reset_breaker() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();

        let action = BreakerAction {
            robot_id: "robot-1".to_string(),
            reason: "test".to_string(),
            cause_ref: "batch:1".to_string(),
            batch_height: 1,
            emitted_at: 1000,
        };
        fc.apply_breaker_actions(&[action]).await;

        // Verify tripped
        let state = fc.get_robot_state("robot-1").await.unwrap();
        assert!(state.breaker_state.is_tripped());
        assert_eq!(state.trip_count, 1);

        // Reset
        fc.reset_breaker("robot-1", "operator-1").await.unwrap();

        // Verify reset
        let state = fc.get_robot_state("robot-1").await.unwrap();
        assert!(!state.breaker_state.is_tripped());
        assert!(!state.breaker_state.is_closed()); // Reset state, not Closed

        // Intent should work now
        let result = fc.submit_intent("robot-1", make_intent("robot-1")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fleet_status() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();
        fc.register_robot("robot-2").await.unwrap();
        fc.register_robot("robot-3").await.unwrap();

        // Trip breaker on robot-2
        let action = BreakerAction {
            robot_id: "robot-2".to_string(),
            reason: "test".to_string(),
            cause_ref: "batch:1".to_string(),
            batch_height: 1,
            emitted_at: 1000,
        };
        fc.apply_breaker_actions(&[action]).await;

        // Submit intents from robot-1
        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        fc.record_blocked("robot-1").await;

        let status = fc.fleet_status().await;
        assert_eq!(status.total_robots, 3);
        assert_eq!(status.operational, 2); // robot-1 and robot-3
        assert_eq!(status.locked, 1); // robot-2
        assert_eq!(status.total_intents, 1);
        assert_eq!(status.total_blocked, 1);
        assert_eq!(status.total_trips, 1);
    }

    #[tokio::test]
    async fn test_robot_status_list() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();
        fc.register_robot("robot-2").await.unwrap();

        let list = fc.robot_status_list().await;
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|r| r.robot_id == "robot-1"));
        assert!(list.iter().any(|r| r.robot_id == "robot-2"));
    }

    #[tokio::test]
    async fn test_fleet_capacity() {
        let fc = FleetCoordinator::new(FleetConfig {
            max_intents_per_robot_per_sec: 100,
            max_fleet_size: 2,
            aggregation_window_ms: 1000,
        });
        fc.register_robot("robot-1").await.unwrap();
        fc.register_robot("robot-2").await.unwrap();

        // Third robot should fail
        let result = fc.register_robot("robot-3").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("capacity"));
    }

    #[tokio::test]
    async fn test_deregister_robot() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();

        fc.deregister_robot("robot-1").await.unwrap();

        let result = fc.submit_intent("robot-1", make_intent("robot-1")).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not registered"));
    }

    #[tokio::test]
    async fn test_apply_breaker_actions_multiple() {
        let fc = FleetCoordinator::new(FleetConfig::default());
        fc.register_robot("robot-1").await.unwrap();
        fc.register_robot("robot-2").await.unwrap();

        let actions = vec![
            BreakerAction {
                robot_id: "robot-1".to_string(),
                reason: "violation A".to_string(),
                cause_ref: "batch:1".to_string(),
                batch_height: 1,
                emitted_at: 1000,
            },
            BreakerAction {
                robot_id: "robot-2".to_string(),
                reason: "violation B".to_string(),
                cause_ref: "batch:1".to_string(),
                batch_height: 1,
                emitted_at: 1000,
            },
        ];

        let affected = fc.apply_breaker_actions(&actions).await;
        assert_eq!(affected.len(), 2);
        assert!(affected.contains(&"robot-1".to_string()));
        assert!(affected.contains(&"robot-2".to_string()));

        // Both robots should be tripped
        let s1 = fc.get_robot_state("robot-1").await.unwrap();
        let s2 = fc.get_robot_state("robot-2").await.unwrap();
        assert!(s1.breaker_state.is_tripped());
        assert!(s2.breaker_state.is_tripped());
    }

    #[tokio::test]
    async fn test_apply_breaker_action_unregistered_robot() {
        let fc = FleetCoordinator::new(FleetConfig::default());

        // Apply breaker action for unregistered robot — should auto-register
        let action = BreakerAction {
            robot_id: "ghost-robot".to_string(),
            reason: "unknown robot violation".to_string(),
            cause_ref: "batch:1".to_string(),
            batch_height: 1,
            emitted_at: 1000,
        };

        let affected = fc.apply_breaker_actions(&[action]).await;
        assert_eq!(affected.len(), 1);

        let state = fc.get_robot_state("ghost-robot").await.unwrap();
        assert!(state.breaker_state.is_tripped());
    }

    #[tokio::test]
    async fn test_pending_count() {
        let fc = FleetCoordinator::new(FleetConfig {
            max_intents_per_robot_per_sec: 100,
            max_fleet_size: 100,
            aggregation_window_ms: 10000,
        });
        fc.register_robot("robot-1").await.unwrap();

        assert_eq!(fc.pending_count().await, 0);

        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();
        fc.submit_intent("robot-1", make_intent("robot-1")).await.unwrap();

        assert_eq!(fc.pending_count().await, 2);
    }
}
