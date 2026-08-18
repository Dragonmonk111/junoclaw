use sensor_safety_circuit::{
    build_merkle_tree, envelope_commitment, mimc_hash, sensor_leaf,
    SensorSafetyCircuit,
};
use ark_bn254::{Bn254, Fr};
use ark_ff::PrimeField;
use ark_groth16::Groth16;
use ark_serialize::{CanonicalSerialize, CanonicalDeserialize};
use ark_snark::SNARK;
use ark_std::rand::rngs::StdRng;
use ark_std::rand::SeedableRng;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: gen-safety-proof <command> [args]");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  setup <tree_height> <output_dir>");
        eprintln!("  prove <proving_key> <speed> <force> <distance> <tilt> <accel> <max_speed> <max_force> <min_dist> <max_tilt> <max_accel> <cycle_index> <merkle_root> <output_file>");
        eprintln!("  verify <verifying_key> <envelope_commitment> <merkle_root> <cycle_index> <proof_file>");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "setup" => {
            let tree_height: usize = args[2].parse().expect("invalid tree_height");
            let output_dir = &args[3];
            fs::create_dir_all(output_dir).expect("failed to create output dir");

            let rng = &mut StdRng::seed_from_u64(42);
            let empty_circuit = SensorSafetyCircuit::empty(tree_height);
            let (pk, vk) = Groth16::<Bn254>::circuit_specific_setup(empty_circuit, rng)
                .expect("setup failed");

            let pk_path = format!("{}/safety_proving_key.bin", output_dir);
            let vk_path = format!("{}/safety_verifying_key.bin", output_dir);

            let mut pk_buf = Vec::new();
            pk.serialize_uncompressed(&mut pk_buf).unwrap();
            fs::write(&pk_path, &pk_buf).expect("failed to write pk");

            let mut vk_buf = Vec::new();
            vk.serialize_uncompressed(&mut vk_buf).unwrap();
            fs::write(&vk_path, &vk_buf).expect("failed to write vk");

            println!("Setup complete: pk={}, vk={}", pk_path, vk_path);
        }

        "prove" => {
            let speed = Fr::from(args[3].parse::<u64>().unwrap());
            let force = Fr::from(args[4].parse::<u64>().unwrap());
            let distance = Fr::from(args[5].parse::<u64>().unwrap());
            let tilt = Fr::from(args[6].parse::<u64>().unwrap());
            let accel = Fr::from(args[7].parse::<u64>().unwrap());

            let max_speed = Fr::from(args[8].parse::<u64>().unwrap());
            let max_force = Fr::from(args[9].parse::<u64>().unwrap());
            let min_dist = Fr::from(args[10].parse::<u64>().unwrap());
            let max_tilt = Fr::from(args[11].parse::<u64>().unwrap());
            let max_accel = Fr::from(args[12].parse::<u64>().unwrap());

            let cycle_index = Fr::from(args[13].parse::<u64>().unwrap());

            let leaf = sensor_leaf(speed, force, distance, tilt, accel);
            let zero = Fr::from(0u64);
            let zero_leaf = mimc_hash(zero, zero);
            let tree_height = 1;
            let (merkle_root, paths, bits) = build_merkle_tree(&[leaf, zero_leaf], tree_height);

            let env_commit = envelope_commitment(max_speed, max_force, min_dist, max_tilt, max_accel);

            let pk_path = &args[2];
            let pk_bytes = fs::read(pk_path).expect("failed to read pk");
            let pk = ark_groth16::ProvingKey::<Bn254>::deserialize_uncompressed(&pk_bytes[..])
                .expect("failed to deserialize pk");

            let circuit = SensorSafetyCircuit::new(
                env_commit,
                merkle_root,
                cycle_index,
                speed, force, distance, tilt, accel,
                max_speed, max_force, min_dist, max_tilt, max_accel,
                paths[0].clone(),
                bits[0].clone(),
                tree_height,
            );

            let rng = &mut StdRng::seed_from_u64(123);
            let proof = Groth16::<Bn254>::prove(&pk, circuit, rng).expect("prove failed");

            let output_file = &args[15];
            let mut proof_buf = Vec::new();
            proof.serialize_uncompressed(&mut proof_buf).unwrap();
            fs::write(output_file, &proof_buf).expect("failed to write proof");

            println!("Proof generated: {}", output_file);
            println!("Public inputs: envelope_commitment={}, merkle_root={}, cycle_index={}",
                env_commit, merkle_root, cycle_index);
        }

        "verify" => {
            let vk_path = &args[2];
            let vk_bytes = fs::read(vk_path).expect("failed to read vk");
            let vk = ark_groth16::VerifyingKey::<Bn254>::deserialize_uncompressed(&vk_bytes[..])
                .expect("failed to deserialize vk");

            let env_commit = parse_fr(&args[3]);
            let merkle_root = parse_fr(&args[4]);
            let cycle_index = parse_fr(&args[5]);

            let proof_path = &args[6];
            let proof_bytes = fs::read(proof_path).expect("failed to read proof");
            let proof = ark_groth16::Proof::<Bn254>::deserialize_uncompressed(&proof_bytes[..])
                .expect("failed to deserialize proof");

            let public_inputs = vec![env_commit, merkle_root, cycle_index];
            let valid = Groth16::<Bn254>::verify(&vk, &public_inputs, &proof)
                .expect("verify failed");

            if valid {
                println!("PROOF VALID");
            } else {
                println!("PROOF INVALID");
                std::process::exit(1);
            }
        }

        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}

fn parse_fr(s: &str) -> Fr {
    use std::str::FromStr;
    Fr::from_str(s).unwrap_or_else(|_| {
        panic!("failed to parse field element: {}", s);
    })
}
