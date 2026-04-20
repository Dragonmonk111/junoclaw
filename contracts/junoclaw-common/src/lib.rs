use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Deps, Env, QueryRequest, Uint128, WasmQuery, to_json_binary};

// ──────────────────────────────────────────────
// Agent Registry Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct AgentProfile {
    pub owner: Addr,
    pub name: String,
    pub description: String,
    pub capabilities_hash: String,
    pub model: String,
    pub registered_at: u64,
    pub is_active: bool,
    pub total_tasks: u64,
    pub successful_tasks: u64,
    /// On-chain reputation score. Incremented on success, decremented on slash.
    pub trust_score: u64,
}

// ──────────────────────────────────────────────
// Task Ledger Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct TaskRecord {
    pub id: u64,
    pub agent_id: u64,
    pub submitter: Addr,
    pub input_hash: String,
    pub output_hash: Option<String>,
    pub execution_tier: ExecutionTier,
    pub status: TaskStatus,
    pub submitted_at: u64,
    pub completed_at: Option<u64>,
    pub cost_ujuno: Option<Uint128>,
    /// Optional governance correlation id. When a task is spawned by an
    /// `agent-company` `WavsPush` proposal, this is set to the proposal id so
    /// downstream contracts (e.g. `escrow`) can be addressed by the same key
    /// the governance layer used for `Authorize`. Defaults to `None` for tasks
    /// submitted directly by a daemon wallet, preserving backward compatibility
    /// with pre-v5 stored records.
    #[serde(default)]
    pub proposal_id: Option<u64>,
    /// State constraints evaluated *before* `CompleteTask` performs the
    /// status transition. A violation reverts the whole completion tx
    /// atomically (no escrow callback, no registry callback, no state
    /// change). Defaults to empty Vec so pre-v7 stored records deserialise
    /// unchanged.
    #[serde(default)]
    pub pre_hooks: Vec<Constraint>,
    /// State constraints evaluated *after* the status transition but before
    /// the completion sub-messages (escrow::Confirm, registry::Increment)
    /// fire. A violation reverts the whole tx atomically. Useful for
    /// invariants that depend on the task's new Completed state.
    #[serde(default)]
    pub post_hooks: Vec<Constraint>,
}

#[cw_serde]
pub enum ExecutionTier {
    Local,
    Akash,
}

#[cw_serde]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

// ──────────────────────────────────────────────
// Payment Ledger Types (non-custodial)
// ──────────────────────────────────────────────

/// A payment obligation records that a payer owes a payee some amount.
/// The contract never holds funds — it only tracks the obligation state.
/// Actual transfers happen via the payer's wallet signature.
#[cw_serde]
pub struct PaymentObligation {
    pub id: u64,
    pub payer: Addr,
    pub payee: Addr,
    pub task_id: u64,
    pub amount: Uint128,
    pub denom: String,
    pub status: ObligationStatus,
    pub created_at: u64,
    pub settled_at: Option<u64>,
    /// WAVS attestation hash proving the obligation is valid
    pub attestation_hash: Option<String>,
}

#[cw_serde]
pub enum ObligationStatus {
    /// Obligation recorded, awaiting payer confirmation
    Pending,
    /// Payer confirmed and sent funds directly to payee
    Confirmed,
    /// Obligation disputed by payer
    Disputed,
    /// Obligation cancelled (mutual or admin)
    Cancelled,
    /// Verified by WAVS attestation
    Verified,
}

// ── Deprecated aliases for migration ──
pub type EscrowDeposit = PaymentObligation;
pub type EscrowStatus = ObligationStatus;

// ──────────────────────────────────────────────
// Junoswap v2 DEX Types
// ──────────────────────────────────────────────

#[cw_serde]
pub struct PairInfo {
    pub pair_addr: Addr,
    pub token_a: AssetInfo,
    pub token_b: AssetInfo,
    pub lp_token: Addr,
    pub total_fee_bps: u16,
    pub wavs_verified: bool,
}

#[cw_serde]
pub enum AssetInfo {
    Native(String),
    Cw20(Addr),
}

impl AssetInfo {
    pub fn denom_key(&self) -> String {
        match self {
            AssetInfo::Native(d) => d.clone(),
            AssetInfo::Cw20(a) => a.to_string(),
        }
    }
}

#[cw_serde]
pub struct Asset {
    pub info: AssetInfo,
    pub amount: Uint128,
}

#[cw_serde]
pub struct SwapEvent {
    pub pair: String,
    pub sender: String,
    pub offer_asset: String,
    pub offer_amount: Uint128,
    pub return_asset: String,
    pub return_amount: Uint128,
    pub spread_amount: Uint128,
    pub fee_amount: Uint128,
    pub block_height: u64,
    pub timestamp: u64,
}

// ──────────────────────────────────────────────
// Contract Registry (V30-safe inter-contract refs)
// ──────────────────────────────────────────────

#[cw_serde]
pub struct ContractRegistry {
    pub agent_registry: Option<Addr>,
    pub task_ledger: Option<Addr>,
    pub escrow: Option<Addr>,
}

// ──────────────────────────────────────────────
// Task Hook Constraints (v7)
// ──────────────────────────────────────────────
//
// Hook constraints are pure read-only predicates evaluated at
// `CompleteTask`. They let a task submitter (typically `agent-company`
// via a governance proposal) attach invariants the chain must verify
// before accepting the completion as valid. Failed evaluation reverts
// the whole completion tx atomically, preserving the all-or-nothing
// semantics that `escrow::Confirm` and `agent-registry::Increment`
// already rely on.
//
// These are *trust* primitives, not *coordination* primitives: they
// describe what counts as a valid end-state, not how to reach it.

#[cw_serde]
pub enum Constraint {
    /// The named agent's `trust_score` in `agent-registry` must be
    /// greater than or equal to `min_score`. Useful for expressing
    /// "do not let a low-reputation agent be rewarded further".
    ///
    /// Evaluated by querying `agent-registry` at the address supplied
    /// by the calling contract's `ContractRegistry`.
    AgentTrustAtLeast { agent_id: u64, min_score: u64 },

    /// The bank balance of `who` in `denom` must be >= `amount`.
    /// Useful for "task may only complete if treasury still holds X".
    BalanceAtLeast {
        who: Addr,
        denom: String,
        amount: Uint128,
    },

    /// The junoswap-pair at `pair` must have strictly positive
    /// reserves on both sides. A liveness invariant: "this task must
    /// not drain the pool".
    PairReservesPositive { pair: Addr },

    /// Another task in the same task-ledger must have the stated
    /// status. Useful for expressing task dependencies
    /// ("complete B only if A is Completed"). The `task_ledger` address
    /// resolves against the calling contract's own storage when equal
    /// to `env.contract.address`; otherwise queried cross-contract.
    TaskStatusIs {
        task_ledger: Addr,
        task_id: u64,
        status: TaskStatus,
    },

    /// The current block time must be at or after `unix_seconds`
    /// (seconds since Unix epoch). A pure block-context check —
    /// no cross-contract query. Useful for vesting / timelock
    /// completion: "this task cannot settle before T".
    TimeAfter { unix_seconds: u64 },

    /// The current block height must be at least `height`. Analogous
    /// to `TimeAfter` but denominated in blocks instead of wall-clock
    /// seconds — useful when the invariant is "wait N blocks for
    /// finality" rather than "wait until time T".
    BlockHeightAtLeast { height: u64 },

    /// The escrow contract at `escrow` must report the obligation
    /// keyed by `task_id` as `Confirmed`. Useful for making a task's
    /// completion conditional on payment having settled: "do not
    /// accept the completion of task T unless the escrow for T is
    /// already Confirmed". Returns an explicit failure if the
    /// obligation does not exist, so callers cannot accidentally
    /// pass this constraint on an unfunded task.
    EscrowObligationConfirmed { escrow: Addr, task_id: u64 },
}

impl Constraint {
    /// Pure read-only evaluator. Returns `Ok(())` if the constraint
    /// holds, `Err(reason)` with a human-readable explanation
    /// otherwise. Callers are expected to wrap the error into their
    /// own `ContractError` variant so the revert message is
    /// contract-specific.
    ///
    /// `agent_registry` is required because `AgentTrustAtLeast`
    /// resolves against the registry by cross-contract query. Pass
    /// the address your contract has in its own `ContractRegistry`.
    /// If `None`, `AgentTrustAtLeast` always fails with an explicit
    /// "registry not wired" message — no silent success.
    ///
    /// `env` is required because `TimeAfter` and `BlockHeightAtLeast`
    /// read `env.block` — callers should pass the same `env` their
    /// entry point was invoked with so the check uses the current
    /// block context.
    pub fn evaluate(
        &self,
        deps: Deps,
        env: &Env,
        agent_registry: Option<&Addr>,
    ) -> Result<(), String> {
        match self {
            Constraint::AgentTrustAtLeast { agent_id, min_score } => {
                let registry = agent_registry.ok_or_else(|| {
                    format!(
                        "AgentTrustAtLeast({}, {}): agent-registry not wired",
                        agent_id, min_score
                    )
                })?;
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum AgentRegistryQuery {
                    GetAgent { agent_id: u64 },
                }
                let req = QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: registry.to_string(),
                    msg: to_json_binary(&AgentRegistryQuery::GetAgent {
                        agent_id: *agent_id,
                    })
                    .map_err(|e| format!("encode GetAgent: {}", e))?,
                });
                let profile: AgentProfile = deps
                    .querier
                    .query(&req)
                    .map_err(|e| format!("query agent {}: {}", agent_id, e))?;
                if profile.trust_score < *min_score {
                    return Err(format!(
                        "AgentTrustAtLeast: agent {} trust {} < {}",
                        agent_id, profile.trust_score, min_score
                    ));
                }
                Ok(())
            }
            Constraint::BalanceAtLeast { who, denom, amount } => {
                let balance = deps
                    .querier
                    .query_balance(who.to_string(), denom.clone())
                    .map_err(|e| format!("query_balance({}, {}): {}", who, denom, e))?;
                if balance.amount < *amount {
                    return Err(format!(
                        "BalanceAtLeast: {} has {} {}, need {}",
                        who, balance.amount, denom, amount
                    ));
                }
                Ok(())
            }
            Constraint::PairReservesPositive { pair } => {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum PairQuery {
                    Pool {},
                }
                #[derive(serde::Deserialize)]
                struct PoolReservesView {
                    reserve_a: Uint128,
                    reserve_b: Uint128,
                }
                let req = QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: pair.to_string(),
                    msg: to_json_binary(&PairQuery::Pool {})
                        .map_err(|e| format!("encode Pool: {}", e))?,
                });
                let pool: PoolReservesView = deps
                    .querier
                    .query(&req)
                    .map_err(|e| format!("query pair {}: {}", pair, e))?;
                if pool.reserve_a.is_zero() || pool.reserve_b.is_zero() {
                    return Err(format!(
                        "PairReservesPositive: {} has reserves ({}, {})",
                        pair, pool.reserve_a, pool.reserve_b
                    ));
                }
                Ok(())
            }
            Constraint::TaskStatusIs {
                task_ledger,
                task_id,
                status,
            } => {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum TaskLedgerQuery {
                    GetTask { task_id: u64 },
                }
                let req = QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: task_ledger.to_string(),
                    msg: to_json_binary(&TaskLedgerQuery::GetTask { task_id: *task_id })
                        .map_err(|e| format!("encode GetTask: {}", e))?,
                });
                let task: TaskRecord = deps
                    .querier
                    .query(&req)
                    .map_err(|e| format!("query task {}: {}", task_id, e))?;
                if &task.status != status {
                    return Err(format!(
                        "TaskStatusIs: task {} is {:?}, expected {:?}",
                        task_id, task.status, status
                    ));
                }
                Ok(())
            }
            Constraint::TimeAfter { unix_seconds } => {
                let now = env.block.time.seconds();
                if now < *unix_seconds {
                    return Err(format!(
                        "TimeAfter: block time {} < required {}",
                        now, unix_seconds
                    ));
                }
                Ok(())
            }
            Constraint::BlockHeightAtLeast { height } => {
                let now = env.block.height;
                if now < *height {
                    return Err(format!(
                        "BlockHeightAtLeast: block height {} < required {}",
                        now, height
                    ));
                }
                Ok(())
            }
            Constraint::EscrowObligationConfirmed { escrow, task_id } => {
                #[derive(serde::Serialize)]
                #[serde(rename_all = "snake_case")]
                enum EscrowQuery {
                    GetObligationByTask { task_id: u64 },
                }
                let req = QueryRequest::Wasm(WasmQuery::Smart {
                    contract_addr: escrow.to_string(),
                    msg: to_json_binary(&EscrowQuery::GetObligationByTask { task_id: *task_id })
                        .map_err(|e| format!("encode GetObligationByTask: {}", e))?,
                });
                let obligation: Option<PaymentObligation> = deps
                    .querier
                    .query(&req)
                    .map_err(|e| format!("query escrow {} for task {}: {}", escrow, task_id, e))?;
                match obligation {
                    None => Err(format!(
                        "EscrowObligationConfirmed: no obligation for task {}",
                        task_id
                    )),
                    Some(o) if o.status != ObligationStatus::Confirmed => Err(format!(
                        "EscrowObligationConfirmed: task {} obligation is {:?}, expected Confirmed",
                        task_id, o.status
                    )),
                    Some(_) => Ok(()),
                }
            }
        }
    }
}

/// Helper that evaluates a batch of constraints and returns the first
/// violation, or `Ok(())` if all pass. Preserves declaration order so
/// error messages are deterministic.
pub fn evaluate_all(
    deps: Deps,
    env: &Env,
    constraints: &[Constraint],
    agent_registry: Option<&Addr>,
) -> Result<(), String> {
    for (i, c) in constraints.iter().enumerate() {
        c.evaluate(deps, env, agent_registry)
            .map_err(|e| format!("hook[{}]: {}", i, e))?;
    }
    Ok(())
}
