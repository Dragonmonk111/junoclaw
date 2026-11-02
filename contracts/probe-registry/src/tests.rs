use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{
    ActiveBankResponse, BankHistoryResponse, ExecuteMsg, InstantiateMsg, QueryMsg,
};
use crate::state::{BankStatus, ConceptThreshold, ProbeBank};

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg {
            admin: Some(admin.to_string()),
            min_concepts: Some(2),
            require_calibration: Some(true),
        },
        &[],
        "probe-registry",
        Some(admin.to_string()),
    )
    .unwrap()
}

fn bank_msg(hash: &str, model_id: &str, weights_hash: &str, concepts: usize) -> ExecuteMsg {
    ExecuteMsg::RegisterProbeBank {
        model_id: model_id.to_string(),
        model_revision: "Qwen/Qwen2.5-0.5B-Instruct@rev1".to_string(),
        weights_hash: weights_hash.to_string(),
        layers: vec![12],
        concepts: (0..concepts).map(|i| format!("concept_{}", i)).collect(),
        method: "contrastive-dom-v0.1".to_string(),
        bank_hash: hash.to_string(),
        uri: "ipfs://bank".to_string(),
        calibration_hash: "cal-hash".to_string(),
        thresholds: vec![ConceptThreshold {
            concept: "concept_0".to_string(),
            threshold_bp: 7000,
        }],
    }
}

fn active_bank(app: &App, contract: &Addr, model_id: &str, weights_hash: &str) -> Option<ProbeBank> {
    app.wrap()
        .query_wasm_smart::<ActiveBankResponse>(
            contract,
            &QueryMsg::ActiveBank {
                model_id: model_id.to_string(),
                weights_hash: weights_hash.to_string(),
            },
        )
        .unwrap()
        .bank
}

#[test]
fn register_proposed_then_activate() {
    let mut app = App::default();
    let admin = app.api().addr_make("dao-admin");
    let proposer = app.api().addr_make("proposer");
    let mallory = app.api().addr_make("mallory");
    let contract = store_and_instantiate(&mut app, &admin);

    app.execute_contract(
        proposer.clone(),
        contract.clone(),
        &bank_msg("bank1", "qwen-0.5b", "weights-1", 2),
        &[],
    )
    .unwrap();

    // Not active yet
    assert!(active_bank(&app, &contract, "qwen-0.5b", "weights-1").is_none());

    // Non-admin cannot activate
    let err = app
        .execute_contract(
            mallory.clone(),
            contract.clone(),
            &ExecuteMsg::ActivateProbeBank {
                bank_hash: "bank1".to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert_eq!(
        ContractError::Unauthorized {}.to_string(),
        err.root_cause().to_string()
    );

    // Admin activates → query resolves
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ActivateProbeBank {
            bank_hash: "bank1".to_string(),
        },
        &[],
    )
    .unwrap();

    let bank = active_bank(&app, &contract, "qwen-0.5b", "weights-1").unwrap();
    assert_eq!(bank.status, BankStatus::Active);
    assert_eq!(bank.weights_hash, "weights-1");

    // Same model, different weights → no bank (fine-tune doesn't qualify)
    assert!(active_bank(&app, &contract, "qwen-0.5b", "weights-2-evil-ft").is_none());
}

#[test]
fn second_bank_activation_deprecates_first() {
    let mut app = App::default();
    let admin = app.api().addr_make("dao-admin");
    let proposer = app.api().addr_make("proposer");
    let contract = store_and_instantiate(&mut app, &admin);

    for h in ["bank1", "bank2"] {
        app.execute_contract(
            proposer.clone(),
            contract.clone(),
            &bank_msg(h, "qwen-0.5b", "weights-1", 2),
            &[],
        )
        .unwrap();
    }
    for h in ["bank1", "bank2"] {
        app.execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::ActivateProbeBank {
                bank_hash: h.to_string(),
            },
            &[],
        )
        .unwrap();
    }

    // ActiveBank resolves bank2 only
    assert_eq!(
        active_bank(&app, &contract, "qwen-0.5b", "weights-1")
            .unwrap()
            .bank_hash,
        "bank2"
    );

    // History shows both
    let hist: BankHistoryResponse = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::BankHistory {
                model_id: "qwen-0.5b".to_string(),
            },
        )
        .unwrap();
    assert_eq!(hist.banks.len(), 2);
    assert_eq!(hist.banks[0].status, BankStatus::Deprecated);
    assert_eq!(hist.banks[1].status, BankStatus::Active);
}

#[test]
fn too_few_concepts_rejected_at_activation() {
    let mut app = App::default();
    let admin = app.api().addr_make("dao-admin");
    let proposer = app.api().addr_make("proposer");
    let contract = store_and_instantiate(&mut app, &admin);

    app.execute_contract(
        proposer.clone(),
        contract.clone(),
        &bank_msg("weak", "qwen-0.5b", "weights-1", 1), // 1 concept < min 2
        &[],
    )
    .unwrap();

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::ActivateProbeBank {
                bank_hash: "weak".to_string(),
            },
            &[],
        )
        .unwrap_err();
    assert!(err.root_cause().to_string().contains("too few concepts"));
}

#[test]
fn revoke_clears_active() {
    let mut app = App::default();
    let admin = app.api().addr_make("dao-admin");
    let proposer = app.api().addr_make("proposer");
    let contract = store_and_instantiate(&mut app, &admin);

    app.execute_contract(
        proposer.clone(),
        contract.clone(),
        &bank_msg("bank1", "qwen-0.5b", "weights-1", 2),
        &[],
    )
    .unwrap();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::ActivateProbeBank {
            bank_hash: "bank1".to_string(),
        },
        &[],
    )
    .unwrap();
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::RevokeBank {
            bank_hash: "bank1".to_string(),
            reason: "evasion found".to_string(),
        },
        &[],
    )
    .unwrap();

    assert!(active_bank(&app, &contract, "qwen-0.5b", "weights-1").is_none());

    let bank: ProbeBank = app
        .wrap()
        .query_wasm_smart(
            &contract,
            &QueryMsg::GetBank {
                bank_hash: "bank1".to_string(),
            },
        )
        .unwrap();
    assert_eq!(bank.status, BankStatus::Revoked);
}

#[test]
fn duplicate_bank_hash_rejected() {
    let mut app = App::default();
    let admin = app.api().addr_make("dao-admin");
    let proposer = app.api().addr_make("proposer");
    let contract = store_and_instantiate(&mut app, &admin);

    app.execute_contract(
        proposer.clone(),
        contract.clone(),
        &bank_msg("bank1", "qwen-0.5b", "weights-1", 2),
        &[],
    )
    .unwrap();
    let err = app
        .execute_contract(
            proposer.clone(),
            contract.clone(),
            &bank_msg("bank1", "other-model", "other-weights", 2),
            &[],
        )
        .unwrap_err();
    assert!(err
        .root_cause()
        .to_string()
        .contains("already registered"));
}
