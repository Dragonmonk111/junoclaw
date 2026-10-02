//! Crafted-input fuzzing for the Bud weight arithmetic.
//!
//! The contract's weight ledger is a conservation invariant: the sum of all
//! member weights must equal `TOTAL_WEIGHT` (10_000) after EVERY operation.
//! `execute_bud` moves weight parent→child; `execute_break_channel` prunes a
//! subtree and returns its weight to the root. Any underflow, overflow, or
//! conservation leak here corrupts cw4 voting power accounting.
//!
//! Deterministic seeded xorshift — no external fuzz infra. A random op
//! sequence is replayed against cw-multi-test while a shadow model tracks
//! expected weights; every op is cross-checked against the live contract.

use cosmwasm_std::{Addr, Empty};
use cw_multi_test::{App, ContractWrapper, Executor, IntoAddr};

use jclaw_credential::contract::{execute, instantiate, migrate, query};
use jclaw_credential::msg::{
    ExecuteMsg, InstantiateMsg, ListMembersResponse, QueryMsg,
};
use jclaw_credential::state::TOTAL_WEIGHT;

/// xorshift64* — deterministic, cheap, good enough for input generation.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, range: u64) -> u64 {
        self.next() % range.max(1)
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len().max(1) as u64) as usize]
    }
}

fn contract() -> impl cw_multi_test::Contract<Empty> {
    ContractWrapper::new(execute, instantiate, query).with_migrate(migrate)
}

fn setup() -> (App, Addr, Addr, Addr) {
    let mut app = App::default();
    let code_id = app.store_code(Box::new(contract()));
    let owner = "owner".into_addr();
    let genesis = "genesis".into_addr();
    let contract = app
        .instantiate_contract(
            code_id,
            owner.clone(),
            &InstantiateMsg {
                admin: Some(owner.to_string()),
                genesis: Some(genesis.to_string()),
                sunset_grace_seconds: 10,
            },
            &[],
            "jclaw-credential",
            None,
        )
        .unwrap();
    (app, contract, owner, genesis)
}

/// Shadow model of member weights for cross-checking.
struct Model {
    /// addr -> weight
    weights: std::collections::HashMap<String, u64>,
    /// child -> parent (needed for subtree removal in the model)
    parents: std::collections::HashMap<String, String>,
    members: Vec<String>,
    next_child: u64,
}

impl Model {
    fn new(genesis: &str) -> Self {
        let mut weights = std::collections::HashMap::new();
        weights.insert(genesis.to_string(), TOTAL_WEIGHT);
        Self {
            weights,
            parents: std::collections::HashMap::new(),
            members: vec![genesis.to_string()],
            next_child: 0,
        }
    }
    fn sum(&self) -> u64 {
        self.weights.values().sum()
    }
}

fn assert_weight_conserved(app: &App, contract: &Addr, model: &Model, ctx: &str) {
    let res: ListMembersResponse = app
        .wrap()
        .query_wasm_smart(
            contract,
            &QueryMsg::ListMembers {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    let live_sum: u64 = res.members.iter().map(|m| m.weight).sum();
    assert_eq!(
        live_sum, TOTAL_WEIGHT,
        "{ctx}: live weight sum {live_sum} != {TOTAL_WEIGHT}"
    );
    assert_eq!(
        model.sum(),
        TOTAL_WEIGHT,
        "{ctx}: model weight sum {} != {TOTAL_WEIGHT}",
        model.sum()
    );
    // Cross-check every member individually.
    for m in &res.members {
        assert_eq!(
            Some(&m.weight),
            model.weights.get(m.addr.as_str()),
            "{ctx}: weight mismatch for {}",
            m.addr
        );
    }
    assert_eq!(
        res.members.len(),
        model.members.len(),
        "{ctx}: member count diverged"
    );
}

/// Random Bud/BreakChannel op sequence — the conservation invariant must
/// hold after every single op, valid or rejected.
#[test]
fn fuzz_bud_weight_conservation() {
    let (mut app, contract, owner, genesis) = setup();
    let mut rng = Rng::new(0xB0D);
    // Model keys must be the bech32 form the contract stores/returns.
    let genesis_addr = genesis.to_string();
    let mut model = Model::new(&genesis_addr);

    for step in 0..512 {
        let op = rng.below(10);

        if op < 8 {
            // Bud: pick a parent, generate adversarial child_weight.
            let parent = rng.pick(&model.members).clone();
            let parent_w = *model.weights.get(&parent).unwrap();
            let child_weight = match rng.below(8) {
                0 => 0,                                      // zero-weight bud
                1 => parent_w,                               // drain parent fully
                2 => parent_w.saturating_add(1),             // overdraw
                3 => u64::MAX,                               // overflow probe
                4 => TOTAL_WEIGHT,                           // whole supply
                5 => parent_w / 2.max(1),                    // typical split
                6 => 1,                                      // dust
                _ => rng.below(parent_w.saturating_add(2)),  // random around edge
            };
            // cw-multi-test's mock api validates bech32 — raw strings like
            // "child0" fail addr_validate. IntoAddr produces a valid bech32
            // address from any input string.
            let child = format!("child{}", model.next_child)
                .into_addr()
                .to_string();
            model.next_child += 1;

            let res = app.execute_contract(
                owner.clone(),
                contract.clone(),
                &ExecuteMsg::Bud {
                    parent: parent.clone(),
                    child: child.clone(),
                    child_weight,
                    mayo_pk: None,
                },
                &[],
            );

            if child_weight <= parent_w {
                res.unwrap_or_else(|e| panic!("step {step}: valid bud failed: {e:?}"));
                *model.weights.get_mut(&parent).unwrap() -= child_weight;
                model.weights.insert(child.clone(), child_weight);
                model.parents.insert(child.clone(), parent.clone());
                model.members.push(child);
            } else {
                assert!(
                    res.is_err(),
                    "step {step}: overdrawing bud ({child_weight} > {parent_w}) accepted"
                );
            }
        } else {
            // BreakChannel: prune a random non-root member — its subtree
            // weight returns to root.
            let candidates: Vec<String> = model
                .members
                .iter()
                .filter(|m| *m != &genesis_addr)
                .cloned()
                .collect();
            if candidates.is_empty() {
                continue;
            }
            let target = rng.pick(&candidates).clone();

            let res = app.execute_contract(
                owner.clone(),
                contract.clone(),
                &ExecuteMsg::BreakChannel {
                    addr: target.clone(),
                },
                &[],
            );
            res.unwrap_or_else(|e| panic!("step {step}: break_channel failed: {e}"));

            // Model subtree removal: a member is in target's subtree iff its
            // ancestry chain reaches target. We track parents in a second map.
            let mut to_remove = vec![target.clone()];
            let mut i = 0;
            while i < to_remove.len() {
                let p = to_remove[i].clone();
                for (child, parent) in model.parents.iter() {
                    if parent == &p && !to_remove.contains(child) {
                        to_remove.push(child.clone());
                    }
                }
                i += 1;
            }
            let mut removed_w = 0u64;
            for addr in &to_remove {
                removed_w += model.weights.remove(addr).unwrap_or(0);
                model.members.retain(|m| m != addr);
                model.parents.remove(addr);
            }
            *model.weights.get_mut(&genesis_addr).unwrap() += removed_w;
        }

        assert_weight_conserved(&app, &contract, &model, &format!("step {step}"));
    }
}
