use cosmwasm_std::Addr;
use cw_multi_test::{App, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};
use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, VkStatusResponse};

fn mk(app: &App, label: &str) -> Addr {
    app.api().addr_make(label)
}

fn store_and_instantiate(app: &mut App, admin: &Addr) -> Addr {
    let code = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
    let code_id = app.store_code(Box::new(code));
    app.instantiate_contract(
        code_id,
        admin.clone(),
        &InstantiateMsg { admin: None },
        &[],
        "zk-verifier",
        Some(admin.to_string()),
    )
    .unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // No VK stored yet
    let status: VkStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::VkStatus {})
        .unwrap();
    assert!(!status.has_vk);
    assert_eq!(status.vk_size_bytes, 0);
}

#[test]
fn test_store_vk_unauthorized() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let stranger = mk(&app, "stranger");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            stranger.clone(),
            contract.clone(),
            &ExecuteMsg::StoreVk {
                vk_base64: "AAAA".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let contract_err = err.downcast::<ContractError>().unwrap();
    assert!(matches!(contract_err, ContractError::Unauthorized {}));
}

#[test]
fn test_verify_without_vk_fails() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                proof_base64: "AAAA".to_string(),
                public_inputs_base64: "AAAA".to_string(),
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("no verification key"),
        "unexpected error: {}",
        err_str
    );
}

/// Generate a real Groth16 proof for the circuit: x * x = y (prove knowledge of sqrt)
/// Then verify it on-chain via the contract.
#[test]
fn test_full_groth16_verify() {
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
    use ark_serialize::CanonicalSerialize;
    use ark_snark::SNARK;

    // Minimal circuit: x * x = y
    #[derive(Clone)]
    struct SquareCircuit {
        x: Option<Fr>,
    }

    impl ConstraintSynthesizer<Fr> for SquareCircuit {
        fn generate_constraints(
            self,
            cs: ConstraintSystemRef<Fr>,
        ) -> Result<(), SynthesisError> {
            let x_val = self.x.unwrap_or(Fr::from(1u64));
            let y_val = x_val * x_val;

            // Private witness: x
            let x_var = cs.new_witness_variable(|| Ok(x_val))?;
            // Public input: y = x^2
            let y_var = cs.new_input_variable(|| Ok(y_val))?;

            // Constraint: x * x = y
            cs.enforce_constraint(
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + y_var,
            )?;
            Ok(())
        }
    }

    use ark_std::rand::{SeedableRng, rngs::StdRng};
    let mut rng = StdRng::seed_from_u64(42);

    // Trusted setup
    let circuit = SquareCircuit { x: None };
    let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng).unwrap();

    // Generate proof for x=3 (public input y=9)
    let circuit = SquareCircuit {
        x: Some(Fr::from(3u64)),
    };
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Serialize VK, proof, and public inputs
    let mut vk_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&vk, &mut vk_bytes).unwrap();
    let vk_b64 = base64_encode(&vk_bytes);

    let mut proof_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&proof, &mut proof_bytes).unwrap();
    let proof_b64 = base64_encode(&proof_bytes);

    let public_input = Fr::from(9u64); // y = 3^2 = 9
    let mut input_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&public_input, &mut input_bytes).unwrap();
    let inputs_b64 = base64_encode(&input_bytes);

    // Now test via the contract
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // Store VK
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::StoreVk {
            vk_base64: vk_b64,
        },
        &[],
    )
    .unwrap();

    // Verify VK is stored
    let status: VkStatusResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::VkStatus {})
        .unwrap();
    assert!(status.has_vk);
    assert!(status.vk_size_bytes > 0);

    // Verify proof
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::VerifyProof {
            proof_base64: proof_b64,
            public_inputs_base64: inputs_b64,
        },
        &[],
    )
    .unwrap();

    // Check last verification
    let last: crate::msg::LastVerifyResponse = app
        .wrap()
        .query_wasm_smart(&contract, &QueryMsg::LastVerify {})
        .unwrap();
    assert!(last.verified);
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Shared helper: generate VK, proof, and public inputs for SquareCircuit (x=3, y=9).
/// Returns (vk_b64, proof_b64, inputs_b64).
fn generate_test_proof_data() -> (String, String, String) {
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
    use ark_serialize::CanonicalSerialize;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};

    #[derive(Clone)]
    struct SquareCircuit { x: Option<Fr> }

    impl ConstraintSynthesizer<Fr> for SquareCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let x_val = self.x.unwrap_or(Fr::from(1u64));
            let y_val = x_val * x_val;
            let x_var = cs.new_witness_variable(|| Ok(x_val))?;
            let y_var = cs.new_input_variable(|| Ok(y_val))?;
            cs.enforce_constraint(
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + y_var,
            )?;
            Ok(())
        }
    }

    let mut rng = StdRng::seed_from_u64(42);
    let circuit = SquareCircuit { x: None };
    let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng).unwrap();
    let circuit = SquareCircuit { x: Some(Fr::from(3u64)) };
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    let mut vk_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&vk, &mut vk_bytes).unwrap();
    let mut proof_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&proof, &mut proof_bytes).unwrap();
    let public_input = Fr::from(9u64);
    let mut input_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&public_input, &mut input_bytes).unwrap();

    (base64_encode(&vk_bytes), base64_encode(&proof_bytes), base64_encode(&input_bytes))
}

/// Store a VK on a fresh contract and return the contract address.
fn setup_contract_with_vk(app: &mut App, admin: &Addr, vk_b64: &str) -> Addr {
    let contract = store_and_instantiate(app, admin);
    app.execute_contract(
        admin.clone(),
        contract.clone(),
        &ExecuteMsg::StoreVk { vk_base64: vk_b64.to_string() },
        &[],
    ).unwrap();
    contract
}

// ── Benchmark: estimate CosmWasm gas from CPU time ──

#[test]
fn test_benchmark_verification_time() {
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::Groth16;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};
    use std::time::Instant;

    // Reuse the same circuit + setup as generate_test_proof_data (seed=42)
    use ark_groth16::{ProvingKey, VerifyingKey};
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

    #[derive(Clone)]
    struct SquareCircuit { x: Option<Fr> }

    impl ConstraintSynthesizer<Fr> for SquareCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let x_val = self.x.unwrap_or(Fr::from(1u64));
            let y_val = x_val * x_val;
            let x_var = cs.new_witness_variable(|| Ok(x_val))?;
            let y_var = cs.new_input_variable(|| Ok(y_val))?;
            cs.enforce_constraint(
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + y_var,
            )?;
            Ok(())
        }
    }

    let mut rng = StdRng::seed_from_u64(42);
    let circuit = SquareCircuit { x: None };
    let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng).unwrap();
    let circuit = SquareCircuit { x: Some(Fr::from(3u64)) };
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();
    let public_inputs = vec![Fr::from(9u64)];

    // Warmup
    let pvk = ark_groth16::prepare_verifying_key(&vk);
    let _ = Groth16::<Bn254>::verify_proof(&pvk, &proof, &public_inputs);

    // Benchmark: run verification 10 times, take median
    let mut times_us = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let valid = Groth16::<Bn254>::verify_proof(&pvk, &proof, &public_inputs).unwrap();
        let elapsed = start.elapsed();
        assert!(valid);
        times_us.push(elapsed.as_micros());
    }
    times_us.sort();
    let median_us = times_us[5];
    let median_ms = median_us as f64 / 1000.0;

    // Gas estimation methodology:
    // CosmWasm Wasm gas: ~1 Teragas (10^12) per ms of CPU time on reference hardware.
    // Wasm is ~3-5x slower than native optimized code (interpreter + metering overhead).
    // Juno DefaultGasMultiplier = 100 → SDK gas = Wasm gas / 100.
    //
    // Conservative estimate: native_ms × 4 (wasm overhead) × 2_000_000 SDK gas/ms
    // This formula aligns with empirical wasmd benchmarks showing ~2M SDK gas per ms
    // of Wasm execution. Real on-chain measurement will be the definitive answer.
    let estimated_wasm_ms = median_ms * 4.0;
    let estimated_sdk_gas = (estimated_wasm_ms * 2_000_000.0) as u64;

    println!("\n══════════════════════════════════════════════");
    println!("  GROTH16 BN254 VERIFICATION BENCHMARK");
    println!("══════════════════════════════════════════════");
    println!("  Native CPU time (median of 10): {:.3} ms", median_ms);
    println!("  Estimated Wasm time (4x):       {:.3} ms", estimated_wasm_ms);
    println!("  Estimated SDK gas (~2M/ms):     {:>12}", estimated_sdk_gas);
    println!("  Juno tx gas limit:              ~10,000,000");
    println!("  With BN254 precompile:             ~187,000");
    if estimated_sdk_gas > 187_000 {
        println!("  Ratio (pure/precompile):         {:.0}x", estimated_sdk_gas as f64 / 187_000.0);
    }
    println!("══════════════════════════════════════════════");
    println!("  All 10 runs (us): {:?}", times_us);
    println!("  NOTE: Run with --release for realistic numbers.");
    println!("  Definitive answer: deploy to uni-7.");
    println!("══════════════════════════════════════════════");

    // Sanity: verification should be sub-100ms native
    assert!(median_ms < 100.0, "verification took too long: {:.3}ms", median_ms);
}

// ── Adversarial tests ──

#[test]
fn test_invalid_vk_bytes_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let contract = store_and_instantiate(&mut app, &admin);

    // Random bytes that aren't a valid VerifyingKey<Bn254>
    let garbage_b64 = base64_encode(&[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33]);
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::StoreVk { vk_base64: garbage_b64 },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("invalid verification key") || err_str.contains("deserialization"),
        "expected VK deserialization error, got: {}",
        err_str
    );
}

#[test]
fn test_wrong_public_input_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (vk_b64, proof_b64, _correct_inputs) = generate_test_proof_data();
    let contract = setup_contract_with_vk(&mut app, &admin, &vk_b64);

    // Submit wrong public input: y=16 (x=4) instead of y=9 (x=3)
    use ark_bn254::Fr;
    use ark_serialize::CanonicalSerialize;
    let wrong_input = Fr::from(16u64);
    let mut wrong_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&wrong_input, &mut wrong_bytes).unwrap();
    let wrong_inputs_b64 = base64_encode(&wrong_bytes);

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                proof_base64: proof_b64,
                public_inputs_base64: wrong_inputs_b64,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("proof verification failed") || err_str.contains("ProofInvalid"),
        "expected ProofInvalid, got: {}",
        err_str
    );
}

#[test]
fn test_tampered_proof_rejected() {
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let (vk_b64, proof_b64, inputs_b64) = generate_test_proof_data();
    let contract = setup_contract_with_vk(&mut app, &admin, &vk_b64);

    // Decode proof, flip a byte, re-encode
    let proof_bytes_orig = cosmwasm_std::Binary::from_base64(&proof_b64).unwrap().to_vec();
    let mut tampered = proof_bytes_orig.clone();
    // Flip the last byte of the first G1 point (byte index 10)
    tampered[10] ^= 0xFF;

    // The tampered bytes may not even deserialize to a valid proof point,
    // which is also a valid rejection. Either deserialization error or proof invalid.
    let tampered_b64 = base64_encode(&tampered);
    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                proof_base64: tampered_b64,
                public_inputs_base64: inputs_b64,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("ProofInvalid") || err_str.contains("proof")
            || err_str.contains("deserialization") || err_str.contains("verify error"),
        "expected rejection of tampered proof, got: {}",
        err_str
    );
}

#[test]
fn test_mismatched_vk_rejects_valid_proof() {
    use ark_bn254::{Bn254, Fr};
    use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
    use ark_serialize::CanonicalSerialize;
    use ark_snark::SNARK;
    use ark_std::rand::{SeedableRng, rngs::StdRng};

    // Generate a DIFFERENT VK (different seed → different trusted setup)
    #[derive(Clone)]
    struct SquareCircuit { x: Option<Fr> }

    impl ConstraintSynthesizer<Fr> for SquareCircuit {
        fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
            let x_val = self.x.unwrap_or(Fr::from(1u64));
            let y_val = x_val * x_val;
            let x_var = cs.new_witness_variable(|| Ok(x_val))?;
            let y_var = cs.new_input_variable(|| Ok(y_val))?;
            cs.enforce_constraint(
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + x_var,
                ark_relations::lc!() + y_var,
            )?;
            Ok(())
        }
    }

    let mut rng2 = StdRng::seed_from_u64(99); // Different seed!
    let circuit = SquareCircuit { x: None };
    let (_pk2, vk2): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng2).unwrap();
    let mut vk2_bytes: Vec<u8> = Vec::new();
    CanonicalSerialize::serialize_compressed(&vk2, &mut vk2_bytes).unwrap();
    let wrong_vk_b64 = base64_encode(&vk2_bytes);

    // Get the valid proof from the original setup (seed=42)
    let (_correct_vk, proof_b64, inputs_b64) = generate_test_proof_data();

    // Store the WRONG VK, then try to verify the valid proof
    let mut app = App::default();
    let admin = mk(&app, "admin");
    let contract = setup_contract_with_vk(&mut app, &admin, &wrong_vk_b64);

    let err = app
        .execute_contract(
            admin.clone(),
            contract.clone(),
            &ExecuteMsg::VerifyProof {
                proof_base64: proof_b64,
                public_inputs_base64: inputs_b64,
            },
            &[],
        )
        .unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(
        err_str.contains("ProofInvalid") || err_str.contains("proof verification failed"),
        "expected ProofInvalid with mismatched VK, got: {}",
        err_str
    );
}
