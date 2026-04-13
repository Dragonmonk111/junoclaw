//! Generate Groth16 proof data (VK + proof + public inputs) as base64 strings.
//! Output can be used to test the zk-verifier contract on-chain.
//!
//! Usage: cargo +stable run -p zk-verifier --example generate_proof

use ark_bn254::{Bn254, Fr};
use ark_groth16::{Groth16, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_serialize::CanonicalSerialize;
use ark_snark::SNARK;
use ark_std::rand::{rngs::StdRng, SeedableRng};

/// Minimal circuit: prove knowledge of x such that x * x = y
#[derive(Clone)]
struct SquareCircuit {
    x: Option<Fr>,
}

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

fn main() {
    // Write JSON to file for easy consumption by TypeScript
    let out_path = std::env::var("PROOF_OUTPUT").unwrap_or_else(|_| {
        let dir = std::env::temp_dir();
        dir.join("groth16_proof.json").to_string_lossy().into_owned()
    });
    let mut rng = StdRng::seed_from_u64(42);

    println!("Generating trusted setup for SquareCircuit...");
    let circuit = SquareCircuit { x: None };
    let (pk, vk): (ProvingKey<Bn254>, VerifyingKey<Bn254>) =
        Groth16::<Bn254>::circuit_specific_setup(circuit, &mut rng).unwrap();

    println!("Generating proof for x=3 (y=9)...");
    let circuit = SquareCircuit {
        x: Some(Fr::from(3u64)),
    };
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).unwrap();

    // Serialize
    let mut vk_bytes = Vec::new();
    CanonicalSerialize::serialize_compressed(&vk, &mut vk_bytes).unwrap();

    let mut proof_bytes = Vec::new();
    CanonicalSerialize::serialize_compressed(&proof, &mut proof_bytes).unwrap();

    let public_input = Fr::from(9u64);
    let mut input_bytes = Vec::new();
    CanonicalSerialize::serialize_compressed(&public_input, &mut input_bytes).unwrap();

    // Verify locally first
    let pvk = ark_groth16::prepare_verifying_key(&vk);
    let valid = Groth16::<Bn254>::verify_proof(&pvk, &proof, &[public_input]).unwrap();
    println!("Local verification: {}", if valid { "VALID ✓" } else { "INVALID ✗" });

    println!("\n════════════════════════════════════════");
    println!("  BASE64 ENCODED DATA FOR ON-CHAIN USE");
    println!("════════════════════════════════════════");
    println!("\nVK ({} bytes):", vk_bytes.len());
    println!("{}", base64_encode(&vk_bytes));
    println!("\nProof ({} bytes):", proof_bytes.len());
    println!("{}", base64_encode(&proof_bytes));
    println!("\nPublic inputs ({} bytes):", input_bytes.len());
    println!("{}", base64_encode(&input_bytes));

    // Also output as JSON for easy copy-paste into TypeScript
    println!("\n════════════════════════════════════════");
    println!("  JSON FOR TYPESCRIPT");
    println!("════════════════════════════════════════");
    println!("{{");
    println!("  \"vk_base64\": \"{}\",", base64_encode(&vk_bytes));
    println!("  \"proof_base64\": \"{}\",", base64_encode(&proof_bytes));
    println!("  \"public_inputs_base64\": \"{}\"", base64_encode(&input_bytes));
    println!("}}");

    // Write to file
    let json = format!(
        "{{\n  \"vk_base64\": \"{}\",\n  \"proof_base64\": \"{}\",\n  \"public_inputs_base64\": \"{}\"\n}}",
        base64_encode(&vk_bytes),
        base64_encode(&proof_bytes),
        base64_encode(&input_bytes)
    );
    std::fs::write(&out_path, &json).expect("Failed to write proof JSON");
    println!("\nSaved to: {}", out_path);
}
