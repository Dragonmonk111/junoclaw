//! Cross-contract integration tests for the v5 hardening pass.
//!
//! These tests spin up **real** `agent-company`, `task-ledger`, `escrow`,
//! and `agent-registry` contracts inside `cw_multi_test` and drive the
//! full governance-to-settlement pipeline end-to-end. They are the
//! regression barrier for the C1/C2/C3/H2 fixes:
//!
//!   * C1 — `UpdateRegistry` is the wire-up execute for post-deploy
//!     registries; instantiate-time mirroring covers the common pointers.
//!   * C2 — direct fields (`agent_registry`, `task_ledger`) stay in
//!     lockstep with `registry.*`.
//!   * C3 — `proposal_id` flows `WavsPush → SubmitTask → TaskRecord`, and
//!     the `CompleteTask → Escrow::Confirm` callback uses
//!     `task.proposal_id.unwrap_or(task_id)` so the obligation keyed at
//!     `proposal_id` (which `agent-company::Authorize` used) resolves.
//!   * H2 — `submit_attestation` queries `GetTaskByProposal` and really
//!     gates on `TaskStatus::Completed`.
//!
//! Whenever any of these regresses the asserts here fail hard.

use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::msg::{ExecuteMsg, InstantiateMsg, MemberInput, ProposalKindMsg, QueryMsg};
use crate::state::{MemberRole, Proposal, ProposalStatus, VoteOption};

use junoclaw_common::{
    ContractRegistry, ExecutionTier, ObligationStatus, PaymentObligation, TaskRecord, TaskStatus,
};

const UJUNO: &str = "ujuno";

fn mk(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

/// Handle bundle returned from [`deploy_full_stack`].
struct Deployment {
    agent_company: Addr,
    task_ledger: Addr,
    escrow: Addr,
    agent_registry: Addr,
    admin: Addr,
    alice: Addr,
    bob: Addr,
}

/// Store all four contract code-ids, instantiate them in dependency order,
/// fund the admin with some native tokens, and wire the cross-contract
/// registries via the new `UpdateRegistry` execute. Returns a bundle of
/// addresses so each test case can drive the on-chain state it cares about.
fn deploy_full_stack(app: &mut App) -> Deployment {
    let admin = mk(app, "admin");
    let alice = mk(app, "alice");
    let bob = mk(app, "bob");

    // Seed the admin with some native tokens so any proposal-creation funds
    // checks (currently none, but future-proof) don't trip.
    app.init_modules(|router, _api, storage| {
        router
            .bank
            .init_balance(storage, &admin, coins(10_000_000, UJUNO))
            .unwrap();
    });

    // ── Code uploads ──
    let agent_registry_code = app.store_code(Box::new(ContractWrapper::new(
        agent_registry::contract::execute,
        agent_registry::contract::instantiate,
        agent_registry::contract::query,
    )));
    let task_ledger_code = app.store_code(Box::new(ContractWrapper::new(
        task_ledger::contract::execute,
        task_ledger::contract::instantiate,
        task_ledger::contract::query,
    )));
    let escrow_code = app.store_code(Box::new(ContractWrapper::new(
        escrow::contract::execute,
        escrow::contract::instantiate,
        escrow::contract::query,
    )));
    let agent_company_code = app.store_code(Box::new(ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )));

    // ── 1. Agent registry ──
    let agent_registry_addr = app
        .instantiate_contract(
            agent_registry_code,
            admin.clone(),
            &agent_registry::msg::InstantiateMsg {
                admin: Some(admin.to_string()),
                max_agents: 100,
                registration_fee_ujuno: Uint128::zero(),
                denom: Some(UJUNO.to_string()),
                registry: None,
            },
            &[],
            "agent-registry",
            Some(admin.to_string()),
        )
        .unwrap();

    // ── 2. Task ledger (admin is an operator so it can CompleteTask) ──
    let task_ledger_addr = app
        .instantiate_contract(
            task_ledger_code,
            admin.clone(),
            &task_ledger::msg::InstantiateMsg {
                admin: Some(admin.to_string()),
                agent_registry: agent_registry_addr.to_string(),
                operators: Some(vec![admin.to_string()]),
                registry: None, // mirroring will fill registry.agent_registry
            },
            &[],
            "task-ledger",
            Some(admin.to_string()),
        )
        .unwrap();

    // ── 3. Escrow ──
    let escrow_addr = app
        .instantiate_contract(
            escrow_code,
            admin.clone(),
            &escrow::msg::InstantiateMsg {
                admin: Some(admin.to_string()),
                task_ledger: task_ledger_addr.to_string(),
                timeout_blocks: 1_000,
                denom: Some(UJUNO.to_string()),
                registry: None, // mirroring will fill registry.task_ledger
            },
            &[],
            "escrow",
            Some(admin.to_string()),
        )
        .unwrap();

    // ── 4. Agent company (admin is a member so it can execute passed proposals under H1) ──
    let members = vec![
        MemberInput {
            addr: admin.to_string(),
            weight: 6000,
            role: MemberRole::Human,
        },
        MemberInput {
            addr: alice.to_string(),
            weight: 3000,
            role: MemberRole::Human,
        },
        MemberInput {
            addr: bob.to_string(),
            weight: 1000,
            role: MemberRole::Agent,
        },
    ];
    let agent_company_addr = app
        .instantiate_contract(
            agent_company_code,
            admin.clone(),
            &InstantiateMsg {
                name: "TestCo".to_string(),
                admin: Some(admin.to_string()),
                governance: None,
                escrow_contract: escrow_addr.to_string(),
                agent_registry: agent_registry_addr.to_string(),
                task_ledger: Some(task_ledger_addr.to_string()),
                nois_proxy: None,
                members,
                denom: Some(UJUNO.to_string()),
                voting_period_blocks: Some(50),
                quorum_percent: Some(51),
                adaptive_threshold_blocks: Some(10),
                adaptive_min_blocks: Some(13),
                verification: None,
                supermajority_quorum_percent: None,
            },
            &[],
            "agent-company",
            Some(admin.to_string()),
        )
        .unwrap();

    // ── 5. Wire the registries that instantiate-time mirroring couldn't cover ──
    // Fresh-deployment happy path: task-ledger needs `registry.escrow`,
    // agent-registry needs `registry.task_ledger`. The other pointers are
    // already filled in by the required-field mirroring we added in
    // task-ledger/contract.rs and escrow/contract.rs.
    app.execute_contract(
        admin.clone(),
        task_ledger_addr.clone(),
        &task_ledger::msg::ExecuteMsg::UpdateRegistry {
            agent_registry: None,
            task_ledger: None,
            escrow: Some(escrow_addr.to_string()),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        admin.clone(),
        agent_registry_addr.clone(),
        &agent_registry::msg::ExecuteMsg::UpdateRegistry {
            agent_registry: None,
            task_ledger: Some(task_ledger_addr.to_string()),
            escrow: None,
        },
        &[],
    )
    .unwrap();

    Deployment {
        agent_company: agent_company_addr,
        task_ledger: task_ledger_addr,
        escrow: escrow_addr,
        agent_registry: agent_registry_addr,
        admin,
        alice,
        bob,
    }
}

// ──────────────────────────────────────────────────────────────────────
// Primary regression: full WavsPush → SubmitTask → Authorize →
// CompleteTask → Confirm pipeline, with coherence via proposal_id.
// ──────────────────────────────────────────────────────────────────────
#[test]
fn wavs_push_full_lifecycle_settles_via_proposal_id() {
    let mut app = App::default();
    let d = deploy_full_stack(&mut app);

    // ── Step 1: create a WavsPush proposal with a non-zero escrow_amount so
    //           `Authorize` fires at execute time.
    let escrow_amount = Uint128::from(750_000u128);
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::WavsPush {
                task_description: "Deploy weekly summary to Akash".to_string(),
                execution_tier: ExecutionTier::Akash,
                escrow_amount,
            },
        },
        &[],
    )
    .unwrap();
    let proposal_id: u64 = 1;

    // Snapshot the task-ledger's next-task-id BEFORE the proposal executes so
    // we can prove `task_id != proposal_id` and still have settlement resolve.
    // (With `NEXT_TASK_ID.save(&1u64)` at instantiate and no prior submits,
    // the first task_id will be 1 — but the proposal_id here is also 1, so
    // we submit a throwaway daemon task first to break the coincidence.)
    app.execute_contract(
        d.admin.clone(),
        d.task_ledger.clone(),
        &task_ledger::msg::ExecuteMsg::SubmitTask {
            agent_id: 42,
            input_hash: "daemon_noise".to_string(),
            execution_tier: ExecutionTier::Local,
            proposal_id: None,
        },
        &[],
    )
    .unwrap();
    // task_id 1 now belongs to the daemon task. The upcoming governance task
    // will receive task_id 2, so any code-path that naively uses
    // `task_id == proposal_id == 1` will fail against a live obligation at
    // proposal_id 1.

    // ── Step 2: vote through. Admin alone holds 6000 bps (60%) which
    //           already clears the 51% participation quorum with yes > no,
    //           so the cast_vote auto-tally flips status to Passed after a
    //           single Yes. Any further vote would hit `ProposalNotOpen`.
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::CastVote {
            proposal_id,
            vote: VoteOption::Yes,
        },
        &[],
    )
    .unwrap();
    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&d.agent_company, &QueryMsg::GetProposal { proposal_id })
        .unwrap();
    assert_eq!(
        proposal.status,
        ProposalStatus::Passed,
        "60% Yes on an ordinary proposal must auto-flip to Passed"
    );

    app.update_block(|b| b.height += 60);

    // ── Step 3: execute the proposal. This atomically fires:
    //           (a) task-ledger.SubmitTask { ..., proposal_id: Some(1) }
    //           (b) escrow.Authorize { task_id: 1, ... }
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id },
        &[],
    )
    .unwrap();

    // ── Assertions after execute ──
    // (a) The governance task exists with proposal_id tagged.
    let task: TaskRecord = app
        .wrap()
        .query_wasm_smart(
            &d.task_ledger,
            &task_ledger::msg::QueryMsg::GetTask { task_id: 2 },
        )
        .unwrap();
    assert_eq!(task.id, 2, "governance task gets fresh autoincrement id");
    assert_eq!(
        task.proposal_id,
        Some(proposal_id),
        "C3: TaskRecord must carry the governance proposal_id"
    );
    assert_eq!(task.status, TaskStatus::Running);
    assert_ne!(
        task.id, proposal_id,
        "test premise: task_id (2) must differ from proposal_id (1) to exercise C3"
    );

    // (b) The reverse index resolves proposal_id → task_id 2.
    let task_by_proposal: Option<TaskRecord> = app
        .wrap()
        .query_wasm_smart(
            &d.task_ledger,
            &task_ledger::msg::QueryMsg::GetTaskByProposal { proposal_id },
        )
        .unwrap();
    let resolved = task_by_proposal.expect("GetTaskByProposal must find the governance task");
    assert_eq!(resolved.id, 2);
    assert_eq!(resolved.proposal_id, Some(proposal_id));

    // (c) The escrow obligation is keyed by proposal_id (the id `agent-company`
    //     used for `Authorize`), not by task_id.
    let obligation_at_proposal_id: Option<PaymentObligation> = app
        .wrap()
        .query_wasm_smart(
            &d.escrow,
            &escrow::msg::QueryMsg::GetObligationByTask { task_id: proposal_id },
        )
        .unwrap();
    let obligation = obligation_at_proposal_id
        .expect("escrow obligation must be registered at proposal_id key");
    assert_eq!(obligation.task_id, proposal_id);
    assert_eq!(obligation.amount, escrow_amount);
    assert_eq!(obligation.status, ObligationStatus::Pending);

    // Sanity: no obligation exists at the task_id key — it was never used.
    let obligation_at_task_id: Option<PaymentObligation> = app
        .wrap()
        .query_wasm_smart(
            &d.escrow,
            &escrow::msg::QueryMsg::GetObligationByTask { task_id: task.id },
        )
        .unwrap();
    assert!(
        obligation_at_task_id.is_none(),
        "no obligation should exist at task_id (governance path uses proposal_id)"
    );

    // ── Step 4: complete the task via task-ledger. This is the moment the
    //           v4 code silently failed — it would call
    //           `Escrow::Confirm { task_id: 2 }` which would 404. v5 routes
    //           via `task.proposal_id.unwrap_or(task_id)` = 1, matching the
    //           obligation key above, and atomically confirms it.
    app.execute_contract(
        d.admin.clone(),
        d.task_ledger.clone(),
        &task_ledger::msg::ExecuteMsg::CompleteTask {
            task_id: task.id,
            output_hash: "akash_deploy_output_sha".to_string(),
            cost_ujuno: Some(Uint128::from(500_000u128)),
        },
        &[],
    )
    .expect("CompleteTask + atomic Escrow::Confirm callback must succeed under C3 routing");

    // ── Assertions after complete ──
    let obligation_after: Option<PaymentObligation> = app
        .wrap()
        .query_wasm_smart(
            &d.escrow,
            &escrow::msg::QueryMsg::GetObligationByTask { task_id: proposal_id },
        )
        .unwrap();
    let confirmed = obligation_after.unwrap();
    assert_eq!(
        confirmed.status,
        ObligationStatus::Confirmed,
        "C3: task completion must atomically flip escrow Pending → Confirmed via proposal_id"
    );
    assert!(
        confirmed.settled_at.is_some(),
        "settled_at should be populated after Confirm fires"
    );

    // ── Step 5: submit_attestation exercises H2 (GetTaskByProposal coherence).
    //           task.status is now Completed, so attestation must go through.
    let task_type = "wavs_push".to_string();
    let data_hash = "0xdeadbeef".to_string();
    let attestation_hash = compute_attestation_hash(&task_type, &data_hash);
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id,
            task_type,
            data_hash,
            attestation_hash,
        },
        &[],
    )
    .expect("attestation must pass H2 coherence because task is Completed");
}

// ──────────────────────────────────────────────────────────────────────
// Negative case: if somebody tries to attest a WavsPush whose task is
// still Running, H2 must reject. Proves the coherence check is not
// silently skipped.
// ──────────────────────────────────────────────────────────────────────
#[test]
fn attestation_rejects_while_task_still_running() {
    let mut app = App::default();
    let d = deploy_full_stack(&mut app);

    // Create + pass + execute a WavsPush, but do NOT complete the task.
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::WavsPush {
                task_description: "Unfinished work".to_string(),
                execution_tier: ExecutionTier::Local,
                escrow_amount: Uint128::zero(),
            },
        },
        &[],
    )
    .unwrap();
    let proposal_id: u64 = 1;

    // Admin's 60% Yes alone clears quorum and flips the tally to Passed.
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::CastVote {
            proposal_id,
            vote: VoteOption::Yes,
        },
        &[],
    )
    .unwrap();
    app.update_block(|b| b.height += 60);
    app.execute_contract(
        d.admin.clone(),
        d.agent_company.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id },
        &[],
    )
    .unwrap();

    // Task exists and is Running (not Completed).
    let task_type = "wavs_push".to_string();
    let data_hash = "running_but_attested".to_string();
    let attestation_hash = compute_attestation_hash(&task_type, &data_hash);
    let err = app
        .execute_contract(
            d.admin.clone(),
            d.agent_company.clone(),
            &ExecuteMsg::SubmitAttestation {
                proposal_id,
                task_type,
                data_hash,
                attestation_hash,
            },
            &[],
        )
        .unwrap_err();
    let root = format!("{:?}", err.root_cause());
    assert!(
        root.contains("not Completed"),
        "H2: attestation must reject while task is Running; got: {}",
        root
    );
}

// ──────────────────────────────────────────────────────────────────────
// Admin-only UpdateRegistry gate (C1 authorisation sanity-check).
// ──────────────────────────────────────────────────────────────────────
#[test]
fn update_registry_requires_admin() {
    let mut app = App::default();
    let d = deploy_full_stack(&mut app);

    // A non-admin cannot rewire the registry.
    let err = app
        .execute_contract(
            d.alice.clone(),
            d.task_ledger.clone(),
            &task_ledger::msg::ExecuteMsg::UpdateRegistry {
                agent_registry: None,
                task_ledger: None,
                escrow: Some(d.escrow.to_string()),
            },
            &[],
        )
        .unwrap_err();
    let root = format!("{:?}", err.root_cause());
    assert!(
        root.to_lowercase().contains("unauthorized"),
        "non-admin must not be able to call UpdateRegistry; got: {}",
        root
    );

    // Same gate on escrow.
    let err = app
        .execute_contract(
            d.bob.clone(),
            d.escrow.clone(),
            &escrow::msg::ExecuteMsg::UpdateRegistry {
                agent_registry: None,
                task_ledger: None,
                escrow: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err.root_cause())
        .to_lowercase()
        .contains("unauthorized"));

    // Same gate on agent-registry.
    let err = app
        .execute_contract(
            d.alice.clone(),
            d.agent_registry.clone(),
            &agent_registry::msg::ExecuteMsg::UpdateRegistry {
                agent_registry: None,
                task_ledger: None,
                escrow: None,
            },
            &[],
        )
        .unwrap_err();
    assert!(format!("{:?}", err.root_cause())
        .to_lowercase()
        .contains("unauthorized"));
}

// ──────────────────────────────────────────────────────────────────────
// Instantiate-time registry snapshot is honoured (C1 explicit path).
// ──────────────────────────────────────────────────────────────────────
#[test]
fn instantiate_time_registry_snapshot_is_honoured() {
    let mut app = App::default();
    let admin = mk(&app, "admin");

    // Pre-derive two arbitrary sibling addresses to feed through the
    // ContractRegistry JSON — these don't need to be real contracts for this
    // test; we're only asserting that the supplied registry survives the
    // instantiate path with proper validation and is exposed via GetConfig.
    let fake_ar = mk(&app, "fake-agent-registry");
    let fake_tl = mk(&app, "fake-task-ledger");
    let fake_es = mk(&app, "fake-escrow");

    let escrow_code = app.store_code(Box::new(ContractWrapper::new(
        escrow::contract::execute,
        escrow::contract::instantiate,
        escrow::contract::query,
    )));
    let escrow_addr = app
        .instantiate_contract(
            escrow_code,
            admin.clone(),
            &escrow::msg::InstantiateMsg {
                admin: Some(admin.to_string()),
                task_ledger: fake_tl.to_string(),
                timeout_blocks: 10,
                denom: Some(UJUNO.to_string()),
                registry: Some(ContractRegistry {
                    agent_registry: Some(fake_ar.clone()),
                    task_ledger: Some(fake_tl.clone()),
                    escrow: Some(fake_es.clone()),
                }),
            },
            &[],
            "escrow",
            Some(admin.to_string()),
        )
        .unwrap();

    let cfg: escrow::state::Config = app
        .wrap()
        .query_wasm_smart(&escrow_addr, &escrow::msg::QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.registry.agent_registry, Some(fake_ar));
    assert_eq!(cfg.registry.task_ledger, Some(fake_tl));
    assert_eq!(cfg.registry.escrow, Some(fake_es));
}

/// Mirrors the SHA-256 attestation hash computed on-chain by
/// `agent-company::submit_attestation` (Variable 1).
fn compute_attestation_hash(task_type: &str, data_hash: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"junoclaw-wavs-v0.1.0");
    hasher.update(task_type.as_bytes());
    hasher.update(data_hash.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest.iter() {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", byte);
    }
    hex
}
