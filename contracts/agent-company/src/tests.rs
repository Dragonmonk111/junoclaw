use cosmwasm_std::{coins, Addr, Uint128};
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MemberInput, ProposalKindMsg, QueryMsg};
use crate::state::{MemberRole, PaymentRecord, Proposal, ProposalStatus, VoteOption};

const UJUNO: &str = "ujuno";

/// Replicates the on-chain SHA-256 attestation hash for test use.
fn compute_attestation_hash(task_type: &str, data_hash: &str) -> String {
    use sha2::{Sha256, Digest};
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

fn mk(app: &App, label: &str) -> Addr { app.api().addr_make(label) }

fn two_member_msg(alice: &Addr, bob: &Addr) -> Vec<MemberInput> {
    vec![
        MemberInput { addr: alice.to_string(), weight: 6000, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 4000, role: MemberRole::Agent },
    ]
}

fn store_and_instantiate(
    app: &mut App,
    admin: &Addr,
    members: Vec<MemberInput>,
    governance: Option<String>,
) -> Addr {
    store_and_instantiate_with_overrides(app, admin, members, governance, None, None)
}

fn store_and_instantiate_with_overrides(
    app: &mut App,
    admin: &Addr,
    members: Vec<MemberInput>,
    governance: Option<String>,
    wavs_operator: Option<String>,
    task_ledger: Option<String>,
) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    let escrow = mk(app, "escrow");
    let registry = mk(app, "registry");
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            name: "TestCo".to_string(),
            admin: None,
            governance,
            wavs_operator,
            escrow_contract: escrow.to_string(),
            agent_registry: registry.to_string(),
            task_ledger,
            nois_proxy: None,
            members,
            denom: Some(UJUNO.to_string()),
            voting_period_blocks: Some(100),
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
    .unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.name, "TestCo");
    assert_eq!(cfg.total_weight, 10_000);
    assert_eq!(cfg.members.len(), 2);
}

#[test]
fn test_invalid_weights_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let bad_members = vec![
        MemberInput { addr: alice.to_string(), weight: 5000, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 3000, role: MemberRole::Agent },
        // total = 8000, not 10000
    ];
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    let escrow = mk(&app, "escrow");
    let registry = mk(&app, "registry");
    let err = app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            name: "BadCo".to_string(),
            admin: None,
            governance: None,
            wavs_operator: None,
            escrow_contract: escrow.to_string(),
            agent_registry: registry.to_string(),
            task_ledger: None,
            nois_proxy: None,
            members: bad_members,
            denom: Some(UJUNO.to_string()),
            voting_period_blocks: None,
            quorum_percent: None,
            adaptive_threshold_blocks: None,
            adaptive_min_blocks: None,
            verification: None,
            supermajority_quorum_percent: None,
        },
        &[],
        "bad-company",
        None,
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::InvalidWeights { .. }));
}

#[test]
fn test_distribute_payment_split() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");

    // v6 F2: DistributePayment is admin/member-gated. Fund the admin (not
    // a separate `payer`) so the call passes the gate.
    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &admin, coins(10_000_000, UJUNO)).unwrap();
    });

    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Distribute 1_000_000 ujuno for task 1
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DistributePayment { task_id: 1 },
        &coins(1_000_000, UJUNO),
    )
    .unwrap();

    // Alice (60%) gets 600_000, Bob (40%) gets 400_000
    let alice_bal = app.wrap().query_balance(alice.to_string(), UJUNO).unwrap().amount;
    let bob_bal = app.wrap().query_balance(bob.to_string(), UJUNO).unwrap().amount;
    assert_eq!(alice_bal, Uint128::from(600_000u128));
    assert_eq!(bob_bal, Uint128::from(400_000u128));

    // Payment record stored
    let record: PaymentRecord = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetPaymentRecord { task_id: 1 })
        .unwrap();
    assert_eq!(record.total_amount, Uint128::from(1_000_000u128));
}

#[test]
fn test_distribute_no_funds_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DistributePayment { task_id: 1 },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NoFunds {}));
}

#[test]
fn test_double_distribute_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");

    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &admin, coins(10_000_000, UJUNO)).unwrap();
    });

    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DistributePayment { task_id: 1 },
        &coins(1_000_000, UJUNO),
    ).unwrap();

    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DistributePayment { task_id: 1 },
        &coins(1_000_000, UJUNO),
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AlreadyDistributed { .. }));
}

#[test]
fn test_distribute_payment_rejects_non_member_caller() {
    // v6 F2 — DistributePayment is gated to admin or DAO members. A
    // public caller used to be able to pre-empt a pending distribution by
    // calling with 1 ujunox against a target `task_id`, permanently
    // locking the legitimate distribution out via the
    // `PAYMENT_HISTORY.has` idempotency key — a 1-ujunox grief of
    // arbitrarily large payments. v6 closes that with an explicit gate.
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let griefer = mk(&app, "griefer");

    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &griefer, coins(10, UJUNO)).unwrap();
    });

    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app
        .execute_contract(
            griefer,
            contract,
            &ExecuteMsg::DistributePayment { task_id: 42 },
            &coins(1, UJUNO),
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_member_earnings_accumulate() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let payer = mk(&app, "payer");

    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &payer, coins(10_000_000, UJUNO)).unwrap();
    });

    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // v6 F2: re-funnel payments through a member (Alice) so the gate
    // passes. Alice ends up both funding and receiving a 60 % share, and
    // Bob still accrues 40 % of every round — which is what the test is
    // really asserting (accumulation across rounds, not the flow of any
    // single payer).
    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &alice, coins(10_000_000, UJUNO)).unwrap();
    });
    let _ = payer; // retained to keep the fixture shape symmetric

    for task_id in 1..=3u64 {
        app.execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::DistributePayment { task_id },
            &coins(1_000_000, UJUNO),
        ).unwrap();
    }

    let alice_earnings: Uint128 = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetMemberEarnings { addr: alice.to_string() })
        .unwrap();
    assert_eq!(alice_earnings, Uint128::from(1_800_000u128)); // 3 × 600_000
}

#[test]
fn test_update_members_no_governance() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let charlie = mk(&app, "charlie");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let new_members = vec![
        MemberInput { addr: alice.to_string(), weight: 5000, role: MemberRole::Human },
        MemberInput { addr: charlie.to_string(), weight: 5000, role: MemberRole::Agent },
    ];

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::UpdateMembers { members: new_members },
        &[],
    ).unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.members.len(), 2);
    assert!(cfg.members.iter().any(|m| m.addr == charlie));
}

#[test]
fn test_update_members_unauthorized_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::UpdateMembers { members: two_member_msg(&alice, &bob) },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_submit_attestation_wavs_operator_authorized() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let wavs_operator = mk(&app, "wavs-operator");
    let task_ledger = mk(&app, "task-ledger");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate_with_overrides(
        &mut app,
        &admin,
        members,
        None,
        Some(wavs_operator.to_string()),
        Some(task_ledger.to_string()),
    );

    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::OutcomeCreate {
                question: "Test question".to_string(),
                resolution_criteria: "Test criteria".to_string(),
                deadline_block: 999999,
            },
        },
        &[],
    ).unwrap();
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    let task_type = "outcome_verify";
    let data_hash = "hash-operator";
    let att_hash = compute_attestation_hash(task_type, data_hash);
    app.execute_contract(
        wavs_operator.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: task_type.to_string(),
            data_hash: data_hash.to_string(),
            attestation_hash: att_hash.clone(),
        },
        &[],
    ).unwrap();

    let att: Option<crate::state::Attestation> = app.wrap().query_wasm_smart(
        contract.clone(),
        &QueryMsg::GetAttestation { proposal_id: 1 },
    ).unwrap();
    assert_eq!(att.unwrap().submitter, wavs_operator);
}

#[test]
fn test_submit_attestation_task_ledger_not_authorized_when_wavs_operator_set() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let wavs_operator = mk(&app, "wavs-operator");
    let task_ledger = mk(&app, "task-ledger");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate_with_overrides(
        &mut app,
        &admin,
        members,
        None,
        Some(wavs_operator.to_string()),
        Some(task_ledger.to_string()),
    );

    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::OutcomeCreate {
                question: "Test question".to_string(),
                resolution_criteria: "Test criteria".to_string(),
                deadline_block: 999999,
            },
        },
        &[],
    ).unwrap();
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    let err = app.execute_contract(
        task_ledger,
        contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: "outcome_verify".to_string(),
            data_hash: "fake".to_string(),
            attestation_hash: "fake".to_string(),
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_rotate_wavs_operator_admin_only() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let operator_v1 = mk(&app, "operator-v1");
    let operator_v2 = mk(&app, "operator-v2");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate_with_overrides(
        &mut app,
        &admin,
        members,
        None,
        Some(operator_v1.to_string()),
        None,
    );

    // Non-admin cannot rotate.
    let err = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::RotateWavsOperator {
                new_operator: Some(operator_v2.to_string()),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));

    // Admin can rotate.
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::RotateWavsOperator {
            new_operator: Some(operator_v2.to_string()),
        },
        &[],
    )
    .unwrap();
    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.wavs_operator, Some(operator_v2));

    // Admin can clear (to lock out WAVS entirely).
    app.execute_contract(
        admin,
        contract.clone(),
        &ExecuteMsg::RotateWavsOperator { new_operator: None },
        &[],
    )
    .unwrap();
    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.wavs_operator, None);
}

#[test]
fn test_legacy_propose_weight_change_disabled() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let charlie = mk(&app, "charlie");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let new_members = vec![
        MemberInput { addr: alice.to_string(), weight: 3334, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 3333, role: MemberRole::Agent },
        MemberInput { addr: charlie.to_string(), weight: 3333, role: MemberRole::Agent },
    ];

    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ProposeWeightChange { members: new_members },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::LegacyWeightChangeDisabled {}));
}

#[test]
fn test_legacy_execute_weight_proposal_disabled() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteWeightProposal {},
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::LegacyWeightChangeDisabled {}));
}

#[test]
fn test_legacy_cancel_weight_proposal_disabled() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::CancelWeightProposal {},
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::LegacyWeightChangeDisabled {}));
}

#[test]
fn test_transfer_admin() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let new_admin = mk(&app, "newadmin");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::TransferAdmin { new_admin: new_admin.to_string() },
        &[],
    ).unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, new_admin);
}

// ──────────────────────────────────────────────
// New general governance tests
// ──────────────────────────────────────────────

#[test]
fn test_create_proposal_and_vote() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Alice creates a FreeText proposal
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Test Proposal".to_string(),
                description: "Testing governance".to_string(),
            },
        },
        &[],
    ).unwrap();

    // Query the proposal
    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.id, 1);
    assert_eq!(proposal.proposer, alice);
    assert_eq!(proposal.status, ProposalStatus::Open);

    // Alice votes Yes (weight 6000)
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.yes_weight, 6000);
    // 6000/10000 = 60% > 51% quorum, yes > no → Passed
    assert_eq!(proposal.status, ProposalStatus::Passed);
}

#[test]
fn test_vote_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Reject me".to_string(),
                description: "Should fail".to_string(),
            },
        },
        &[],
    ).unwrap();

    // Alice votes No (6000), Bob votes No (4000) → all voted, no > yes → Rejected
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::No },
        &[],
    ).unwrap();
    app.execute_contract(
        bob.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::No },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.status, ProposalStatus::Rejected);
}

#[test]
fn test_double_vote_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Double vote".to_string(),
                description: "test".to_string(),
            },
        },
        &[],
    ).unwrap();

    // Use Abstain so quorum is met but proposal stays Open (abstain doesn't tip yes>no)
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Abstain },
        &[],
    ).unwrap();

    let err = app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::No },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::AlreadyVoted { .. }));
}

#[test]
fn test_non_member_cannot_propose() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let outsider = mk(&app, "outsider");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let err = app.execute_contract(
        outsider.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Intruder".to_string(),
                description: "test".to_string(),
            },
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::NotMember { .. }));
}

#[test]
fn test_adaptive_block_reduction() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    // Equal weights so single vote (5000) doesn't hit 51% quorum alone
    let members = vec![
        MemberInput { addr: alice.to_string(), weight: 5000, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 5000, role: MemberRole::Agent },
    ];
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create proposal at block 12345
    app.update_block(|b| { b.height = 12345; });

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Adaptive test".to_string(),
                description: "test".to_string(),
            },
        },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.voting_deadline_block, 12345 + 100); // default

    // Alice votes Yes at block 12350 (5 blocks in, < threshold=10)
    app.update_block(|b| { b.height = 12350; });
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    // Proposal still Open (5000/10000 = 50% < 51% quorum)
    let mid: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(mid.status, ProposalStatus::Open);

    // Bob votes Yes at same block — all voted within threshold
    app.execute_contract(
        bob.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    // Deadline should have shrunk: max(12350+3, 12345+13) = max(12353, 12358) = 12358
    assert_eq!(proposal.voting_deadline_block, 12358);
    assert_eq!(proposal.status, ProposalStatus::Passed);
}

#[test]
fn test_execute_passed_proposal() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let charlie = mk(&app, "charlie");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Propose weight redistribution via the governance system.
    let new_members = vec![
        MemberInput { addr: alice.to_string(), weight: 4000, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 3000, role: MemberRole::Agent },
        MemberInput { addr: charlie.to_string(), weight: 3000, role: MemberRole::Agent },
    ];

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::WeightChange { members: new_members },
        },
        &[],
    ).unwrap();

    // `WeightChange` is constitutional — requires the 67% supermajority
    // threshold (same bar as `CodeUpgrade`). Alice's 6000 bps alone is below
    // 6700; Bob's 4000 must also vote Yes to reach 10000 > 6700 and pass.
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();
    app.execute_contract(
        bob.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    // Advance past deadline
    app.update_block(|b| { b.height += 101; });

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.members.len(), 3);
    assert!(cfg.members.iter().any(|m| m.addr == charlie));
}

#[test]
fn test_execute_before_deadline_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Too early".to_string(),
                description: "test".to_string(),
            },
        },
        &[],
    ).unwrap();

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    // Don't advance blocks — should fail
    let err = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::VotingNotEnded { .. }));
}

#[test]
fn test_expire_proposal() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Will expire".to_string(),
                description: "test".to_string(),
            },
        },
        &[],
    ).unwrap();

    // Don't vote — advance past deadline
    app.update_block(|b| { b.height += 101; });

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExpireProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.status, ProposalStatus::Expired);
}

#[test]
fn test_list_proposals() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    for i in 0..3 {
        app.execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::CreateProposal {
                kind: ProposalKindMsg::FreeText {
                    title: format!("Proposal {}", i),
                    description: "test".to_string(),
                },
            },
            &[],
        ).unwrap();
    }

    let proposals: Vec<Proposal> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::ListProposals { start_after: None, limit: None })
        .unwrap();
    assert_eq!(proposals.len(), 3);
}

#[test]
fn test_config_change_proposal() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let new_admin = mk(&app, "newadmin");
    let wavs_operator = mk(&app, "wavs-operator");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::ConfigChange {
                new_admin: Some(new_admin.to_string()),
                new_governance: None,
                new_wavs_operator: Some(wavs_operator.to_string()),
            },
        },
        &[],
    ).unwrap();

    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    app.update_block(|b| { b.height += 101; });

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.admin, new_admin);
    assert_eq!(cfg.wavs_operator, Some(wavs_operator));
}

#[test]
fn test_dust_goes_to_last_member() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let payer = mk(&app, "payer");

    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &payer, coins(10_000_000, UJUNO)).unwrap();
    });

    // 3333 + 6667 = 10000 but 3 ujuno / 10000 has remainder
    let members = vec![
        MemberInput { addr: alice.to_string(), weight: 3333, role: MemberRole::Human },
        MemberInput { addr: bob.to_string(), weight: 6667, role: MemberRole::Agent },
    ];
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // v6 F2: route through admin (gate-allowed) rather than an external payer.
    app.init_modules(|router, _, storage| {
        router.bank.init_balance(storage, &admin, coins(10_000_000, UJUNO)).unwrap();
    });
    let _ = payer;
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::DistributePayment { task_id: 1 },
        &coins(3, UJUNO),
    ).unwrap();

    // Total paid out should be exactly 3 (no dust stuck in contract)
    let alice_bal = app.wrap().query_balance(alice.to_string(), UJUNO).unwrap().amount;
    let bob_bal = app.wrap().query_balance(bob.to_string(), UJUNO).unwrap().amount;
    assert_eq!(alice_bal + bob_bal, Uint128::from(3u128));
}

// ──────────────────────────────────────────────
// Sortition / Randomness tests
// ──────────────────────────────────────────────

fn five_member_msg(app: &App) -> (Vec<MemberInput>, Vec<Addr>) {
    let names = ["alice", "bob", "carol", "dave", "eve"];
    let addrs: Vec<Addr> = names.iter().map(|n| mk(app, n)).collect();
    let members: Vec<MemberInput> = addrs.iter().map(|a| {
        MemberInput { addr: a.to_string(), weight: 2000, role: MemberRole::Human }
    }).collect();
    (members, addrs)
}

#[test]
fn test_sortition_proposal_and_wavs_resolve() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (members, addrs) = five_member_msg(&app);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Alice creates a sortition proposal to select 2 from 5
    let res = app.execute_contract(
        addrs[0].clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest {
                count: 2,
                purpose: "Assembly Term 1".to_string(),
            },
        },
        &[],
    ).unwrap();
    let pid: u64 = res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "proposal_id")
        .unwrap().value.parse().unwrap();

    // 3 of 5 members vote yes (6000/10000 = 60% > 51% quorum → auto-passes)
    for addr in &addrs[0..3] {
        app.execute_contract(
            addr.clone(),
            contract.clone(),
            &ExecuteMsg::CastVote { proposal_id: pid, vote: VoteOption::Yes },
            &[],
        ).unwrap();
    }

    // Advance blocks past deadline
    app.update_block(|b| b.height += 200);

    // Execute the proposal — creates pending sortition
    let exec_res = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: pid },
        &[],
    ).unwrap();

    // Extract job_id from response attributes
    let job_id = exec_res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "job_id")
        .unwrap().value.clone();

    // Verify pending sortition exists
    let pending: Option<crate::state::PendingSortition> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetPendingSortition { job_id: job_id.clone() })
        .unwrap();
    assert!(pending.is_some());
    assert_eq!(pending.unwrap().count, 2);

    // Admin submits WAVS-attested drand randomness
    let randomness_hex = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string();
    let resolve_res = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::SubmitRandomness {
            job_id: job_id.clone(),
            randomness_hex: randomness_hex.clone(),
            attestation_hash: "wavs_attest_001".to_string(),
        },
        &[],
    ).unwrap();

    // Verify sortition resolved
    let round_id: u64 = resolve_res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "round_id")
        .unwrap().value.parse().unwrap();

    let round: crate::state::SortitionRound = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetSortitionRound { round_id })
        .unwrap();
    assert_eq!(round.selected.len(), 2);
    assert_eq!(round.pool_size, 5);
    assert_eq!(round.purpose, "Assembly Term 1");

    // All selected members must be from the original pool
    for selected in &round.selected {
        assert!(addrs.contains(selected));
    }

    // Pending sortition should be cleared
    let pending_after: Option<crate::state::PendingSortition> = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetPendingSortition { job_id })
        .unwrap();
    assert!(pending_after.is_none());
}

#[test]
fn test_sortition_deterministic() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (members, addrs) = five_member_msg(&app);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create, vote (3 of 5 → passes quorum), execute sortition proposal
    app.execute_contract(
        addrs[0].clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest { count: 3, purpose: "Test".to_string() },
        },
        &[],
    ).unwrap();
    for addr in &addrs[0..3] {
        app.execute_contract(
            addr.clone(), contract.clone(),
            &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
            &[],
        ).unwrap();
    }
    app.update_block(|b| b.height += 200);
    let exec_res = app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();
    let job_id = exec_res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "job_id")
        .unwrap().value.clone();

    // Submit randomness and verify deterministic selection with no duplicates
    let hex = "0000000000000000000000000000000000000000000000000000000000000001";
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::SubmitRandomness {
            job_id: job_id.clone(),
            randomness_hex: hex.to_string(),
            attestation_hash: "a".to_string(),
        },
        &[],
    ).unwrap();

    let round1: crate::state::SortitionRound = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetSortitionRound { round_id: 1 })
        .unwrap();
    assert_eq!(round1.selected.len(), 3);
    // No duplicates
    for i in 0..round1.selected.len() {
        for j in (i + 1)..round1.selected.len() {
            assert_ne!(round1.selected[i], round1.selected[j], "duplicate in selection");
        }
    }
}

#[test]
fn test_sortition_unauthorized_randomness_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (members, addrs) = five_member_msg(&app);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create, vote (3 of 5), execute sortition proposal
    app.execute_contract(
        addrs[0].clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest { count: 1, purpose: "Auth test".to_string() },
        },
        &[],
    ).unwrap();
    for addr in &addrs[0..3] {
        app.execute_contract(
            addr.clone(), contract.clone(),
            &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
            &[],
        ).unwrap();
    }
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    // Random stranger tries to submit randomness — should fail
    let stranger = mk(&app, "stranger");
    let err = app.execute_contract(
        stranger,
        contract.clone(),
        &ExecuteMsg::SubmitRandomness {
            job_id: "sortition_1_201".to_string(),
            randomness_hex: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            attestation_hash: "fake".to_string(),
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::UnauthorizedRandomness {}));
}

#[test]
fn test_sortition_wavs_operator_authorized() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let wavs_operator = mk(&app, "wavs-operator");
    let task_ledger = mk(&app, "task-ledger");
    let (members, addrs) = five_member_msg(&app);
    let contract = store_and_instantiate_with_overrides(
        &mut app,
        &admin,
        members,
        None,
        Some(wavs_operator.to_string()),
        Some(task_ledger.to_string()),
    );

    app.execute_contract(
        addrs[0].clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest { count: 1, purpose: "WAVS auth".to_string() },
        },
        &[],
    ).unwrap();
    for addr in &addrs[0..3] {
        app.execute_contract(
            addr.clone(), contract.clone(),
            &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
            &[],
        ).unwrap();
    }
    app.update_block(|b| b.height += 200);
    let exec_res = app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();
    let job_id = exec_res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "job_id")
        .unwrap().value.clone();

    app.execute_contract(
        wavs_operator,
        contract.clone(),
        &ExecuteMsg::SubmitRandomness {
            job_id,
            randomness_hex: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
            attestation_hash: "wavs-auth".to_string(),
        },
        &[],
    ).unwrap();

    let round: crate::state::SortitionRound = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetSortitionRound { round_id: 1 })
        .unwrap();
    assert_eq!(round.selected.len(), 1);
}

#[test]
fn test_sortition_count_exceeds_pool_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Try to select 5 from a 2-member DAO
    let err = app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest { count: 5, purpose: "Overflow".to_string() },
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::SortitionCountExceedsPool { count: 5, pool_size: 2 }));
}

#[test]
fn test_sortition_invalid_randomness_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (members, addrs) = five_member_msg(&app);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create, vote (3 of 5), execute sortition proposal
    app.execute_contract(
        addrs[0].clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::SortitionRequest { count: 1, purpose: "Bad rand".to_string() },
        },
        &[],
    ).unwrap();
    for addr in &addrs[0..3] {
        app.execute_contract(
            addr.clone(), contract.clone(),
            &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
            &[],
        ).unwrap();
    }
    app.update_block(|b| b.height += 200);
    let exec_res = app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();
    let job_id = exec_res.events.iter()
        .flat_map(|e| e.attributes.iter())
        .find(|a| a.key == "job_id")
        .unwrap().value.clone();

    // Submit too-short randomness
    let err = app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::SubmitRandomness {
            job_id,
            randomness_hex: "abcdef".to_string(),
            attestation_hash: "a".to_string(),
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::InvalidRandomness { .. }));
}

// ── WAVS Attestation Tests ──

#[test]
fn test_submit_attestation_for_outcome_create() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create OutcomeCreate proposal (no cross-contract calls)
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::OutcomeCreate {
                question: "Will BTC exceed 100k?".to_string(),
                resolution_criteria: "CoinGecko spot price at block deadline".to_string(),
                deadline_block: 999999,
            },
        },
        &[],
    ).unwrap();

    // Vote yes (alice=6000 > 51% of 10000)
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    // Advance past voting deadline and execute
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    // Submit attestation from admin (authorized) — with correctly computed hash
    let task_type = "outcome_verify";
    let data_hash = "abc123def456";
    let att_hash = compute_attestation_hash(task_type, data_hash);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: task_type.to_string(),
            data_hash: data_hash.to_string(),
            attestation_hash: att_hash.clone(),
        },
        &[],
    ).unwrap();

    // Query attestation
    let att: Option<crate::state::Attestation> = app.wrap().query_wasm_smart(
        contract.clone(),
        &QueryMsg::GetAttestation { proposal_id: 1 },
    ).unwrap();
    let att = att.unwrap();
    assert_eq!(att.proposal_id, 1);
    assert_eq!(att.task_type, task_type);
    assert_eq!(att.data_hash, data_hash);
    assert_eq!(att.attestation_hash, att_hash);
    assert_eq!(att.submitter, admin);
}

#[test]
fn test_submit_attestation_unauthorized_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let stranger = mk(&app, "stranger");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create, vote, execute OutcomeCreate
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::OutcomeCreate {
                question: "Test question".to_string(),
                resolution_criteria: "Test criteria".to_string(),
                deadline_block: 999999,
            },
        },
        &[],
    ).unwrap();
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    // Stranger tries to submit attestation — should fail
    let err = app.execute_contract(
        stranger.clone(), contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: "outcome_verify".to_string(),
            data_hash: "fake".to_string(),
            attestation_hash: "fake".to_string(),
        },
        &[],
    ).unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_submit_attestation_duplicate_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create, vote, execute OutcomeCreate
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::OutcomeCreate {
                question: "Test question".to_string(),
                resolution_criteria: "Test criteria".to_string(),
                deadline_block: 999999,
            },
        },
        &[],
    ).unwrap();
    app.execute_contract(
        alice.clone(), contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();
    app.update_block(|b| b.height += 200);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    // First attestation — succeeds (with correctly computed hash)
    let task_type = "outcome_verify";
    let data_hash = "hash1";
    let att_hash = compute_attestation_hash(task_type, data_hash);
    app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: task_type.to_string(),
            data_hash: data_hash.to_string(),
            attestation_hash: att_hash,
        },
        &[],
    ).unwrap();

    // Second attestation — should fail (duplicate)
    let att_hash2 = compute_attestation_hash(task_type, "hash2");
    let err = app.execute_contract(
        admin.clone(), contract.clone(),
        &ExecuteMsg::SubmitAttestation {
            proposal_id: 1,
            task_type: task_type.to_string(),
            data_hash: "hash2".to_string(),
            attestation_hash: att_hash2,
        },
        &[],
    ).unwrap_err();
    // Should be a StdError::GenericErr for duplicate
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("attestation already submitted"), "unexpected error: {}", err_str);
}

// ──────────────────────────────────────────────
// CodeUpgrade proposal tests
// ──────────────────────────────────────────────

#[test]
fn test_code_upgrade_requires_supermajority() {
    use crate::state::CodeUpgradeAction;

    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create a CodeUpgrade proposal with SetDexFactory action
    let factory_addr = mk(&app, "junoswap_factory");
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::CodeUpgrade {
                title: "Deploy Junoswap v2".to_string(),
                description: "Bundle Junoswap factory + pair deployment with DEX config wiring".to_string(),
                actions: vec![
                    CodeUpgradeAction::SetDexFactory {
                        factory_addr: factory_addr.to_string(),
                    },
                ],
            },
        },
        &[],
    ).unwrap();

    // Alice votes Yes (weight 6000). 6000/10000 = 60% < 67% supermajority → still Open
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    // 60% yes doesn't meet 67% supermajority — should still be Open
    assert_eq!(proposal.status, ProposalStatus::Open,
        "CodeUpgrade should require supermajority (67%), not pass at 60%");

    // Bob votes Yes (weight 4000). Total = 10000/10000 = 100% → Passed
    app.execute_contract(
        bob.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    assert_eq!(proposal.status, ProposalStatus::Passed);

    // Advance past deadline and execute
    app.update_block(|b| { b.height += 101; });

    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    ).unwrap();

    // Verify dex_factory was set in config
    let cfg: crate::state::Config = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetConfig {})
        .unwrap();
    assert_eq!(cfg.dex_factory, Some(factory_addr));
    assert_eq!(cfg.supermajority_quorum_percent, 67);
}

#[test]
fn test_code_upgrade_empty_actions_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Empty actions should fail
    let err = app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::CodeUpgrade {
                title: "Bad proposal".to_string(),
                description: "No actions".to_string(),
                actions: vec![],
            },
        },
        &[],
    ).unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("at least one action"), "unexpected error: {}", err_str);
}

#[test]
fn test_code_upgrade_supermajority_blocks_minority() {
    use crate::state::CodeUpgradeAction;

    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob); // alice=6000, bob=4000
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let factory_addr = mk(&app, "junoswap_factory");
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::CodeUpgrade {
                title: "Contested upgrade".to_string(),
                description: "Alice yes, Bob no".to_string(),
                actions: vec![
                    CodeUpgradeAction::SetDexFactory {
                        factory_addr: factory_addr.to_string(),
                    },
                ],
            },
        },
        &[],
    ).unwrap();

    // Alice votes Yes (6000)
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    ).unwrap();

    // Bob votes No (4000) — all voted, yes > no, but check quorum
    app.execute_contract(
        bob.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::No },
        &[],
    ).unwrap();

    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    // Under the constitutional **yes-ratio** gate (v5+), a `CodeUpgrade`
    // passes only when `yes_weight * 100 >= total_weight * 67`. Alice's
    // 6000 bps Yes amounts to 60% of total_weight — strictly below the 67%
    // threshold — so Bob's 40% No correctly blocks passage, living up to
    // this test's name. (Under the old participation-quorum gate this
    // 60/40 split would have passed, which was the exact exploit C4 fixed.)
    assert_eq!(proposal.status, ProposalStatus::Rejected,
        "60% Yes falls below the 67% supermajority threshold and must be Rejected");
}

// ─────────────────────────────────────────────────────────────────────
// v5 — M3 / M4 / L5 regression tests
// ─────────────────────────────────────────────────────────────────────

/// M3: `CodeUpgradeAction::ExecuteContract` with malformed `msg_json`
/// must be rejected at proposal creation, not after a voting cycle.
#[test]
fn test_m3_execute_contract_rejects_malformed_msg_json_at_create() {
    use crate::state::CodeUpgradeAction;
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    let target = mk(&app, "some_target");
    let err = app
        .execute_contract(
            alice.clone(),
            contract.clone(),
            &ExecuteMsg::CreateProposal {
                kind: ProposalKindMsg::CodeUpgrade {
                    title: "Malformed JSON".to_string(),
                    description: "payload is not valid JSON".to_string(),
                    actions: vec![CodeUpgradeAction::ExecuteContract {
                        contract_addr: target.to_string(),
                        msg_json: "{ this is not valid json".to_string(),
                    }],
                },
            },
            &[],
        )
        .unwrap_err();
    let root = format!("{:?}", err.root_cause());
    assert!(
        root.contains("malformed msg_json"),
        "M3: malformed JSON in ExecuteContract must be rejected at creation; got: {}",
        root
    );
}

/// M4: `ExecuteProposal` at block == voting_deadline_block must fail.
/// Strict `>` deadline closes the single-block vote/execute race.
#[test]
fn test_m4_execute_at_exact_deadline_is_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);
    let contract = store_and_instantiate(&mut app, &admin, members, None);

    // Create a FreeText proposal (ordinary quorum; alice's 6000 auto-passes).
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CreateProposal {
            kind: ProposalKindMsg::FreeText {
                title: "Exactly at deadline".to_string(),
                description: "race-guard test".to_string(),
            },
        },
        &[],
    )
    .unwrap();

    // Alice's 6000 bps alone clears 51% participation quorum and flips to Passed.
    app.execute_contract(
        alice.clone(),
        contract.clone(),
        &ExecuteMsg::CastVote { proposal_id: 1, vote: VoteOption::Yes },
        &[],
    )
    .unwrap();

    // Look up the actual deadline — adaptive logic may have reduced it.
    let proposal: Proposal = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::GetProposal { proposal_id: 1 })
        .unwrap();
    let deadline = proposal.voting_deadline_block;

    // Advance to the EXACT deadline block.
    app.update_block(|b| b.height = deadline);

    // Execute at block == deadline must now fail (M4 strict-`>`).
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
            &[],
        )
        .unwrap_err();
    let root = format!("{:?}", err.root_cause());
    assert!(
        root.to_lowercase().contains("voting") || root.to_lowercase().contains("deadline"),
        "M4: execute at height == deadline must reject with VotingNotEnded; got: {}",
        root
    );

    // One more block should let it through.
    app.update_block(|b| b.height = deadline + 1);
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ExecuteProposal { proposal_id: 1 },
        &[],
    )
    .expect("M4: execute at height > deadline must succeed");
}

/// L5: Governance-percent bounds rejected at instantiate.
#[test]
fn test_l5_governance_percent_bounds_rejected_at_instantiate() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let alice = mk(&app, "alice");
    let bob = mk(&app, "bob");
    let members = two_member_msg(&alice, &bob);

    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    let escrow = mk(&app, "escrow");
    let registry = mk(&app, "registry");

    // (a) quorum_percent > 100 — freezes governance (threshold > total_weight).
    let err = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                name: "BadCo1".to_string(),
                admin: None,
                governance: None,
                wavs_operator: None,
                escrow_contract: escrow.to_string(),
                agent_registry: registry.to_string(),
                task_ledger: None,
                nois_proxy: None,
                members: members.clone(),
                denom: Some(UJUNO.to_string()),
                voting_period_blocks: Some(100),
                quorum_percent: Some(150),
                adaptive_threshold_blocks: None,
                adaptive_min_blocks: None,
                verification: None,
                supermajority_quorum_percent: None,
            },
            &[],
            "bad-co-1",
            None,
        )
        .unwrap_err();
    assert!(
        format!("{:?}", err.root_cause()).contains("quorum_percent must be in 1..=100"),
        "L5: quorum_percent > 100 must be rejected"
    );

    // (b) supermajority < quorum — incoherent (constitutional weaker than ordinary).
    let err = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                name: "BadCo2".to_string(),
                admin: None,
                governance: None,
                wavs_operator: None,
                escrow_contract: escrow.to_string(),
                agent_registry: registry.to_string(),
                task_ledger: None,
                nois_proxy: None,
                members: members.clone(),
                denom: Some(UJUNO.to_string()),
                voting_period_blocks: Some(100),
                quorum_percent: Some(60),
                adaptive_threshold_blocks: None,
                adaptive_min_blocks: None,
                verification: None,
                supermajority_quorum_percent: Some(50),
            },
            &[],
            "bad-co-2",
            None,
        )
        .unwrap_err();
    assert!(
        format!("{:?}", err.root_cause()).contains("must be >= quorum_percent"),
        "L5: supermajority_quorum_percent < quorum_percent must be rejected"
    );

    // (c) quorum_percent = 0 — auto-pass on zero votes, catastrophic.
    let err = app
        .instantiate_contract(
            code_id,
            admin.clone(),
            &InstantiateMsg {
                name: "BadCo3".to_string(),
                admin: None,
                governance: None,
                wavs_operator: None,
                escrow_contract: escrow.to_string(),
                agent_registry: registry.to_string(),
                task_ledger: None,
                nois_proxy: None,
                members,
                denom: Some(UJUNO.to_string()),
                voting_period_blocks: Some(100),
                quorum_percent: Some(0),
                adaptive_threshold_blocks: None,
                adaptive_min_blocks: None,
                verification: None,
                supermajority_quorum_percent: None,
            },
            &[],
            "bad-co-3",
            None,
        )
        .unwrap_err();
    assert!(
        format!("{:?}", err.root_cause()).contains("quorum_percent must be in 1..=100"),
        "L5: quorum_percent = 0 must be rejected"
    );
}
