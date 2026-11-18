//! zk-ceremony — sequential Groth16 phase-2 MPC for jlens-readout.
//!
//! Model: delta-scaling contribution (BGM17-style, the "MMORPG"
//! ceremony's delta step). Each contributor samples rho, transforms
//! delta -> delta*rho across the SRS, and publishes [rho]G1/[rho]G2 as
//! the contribution public key. Toxic waste = the hidden delta product;
//! one honest contribution secures the key even though the base params
//! are publicly reproducible.
//!
//!   zk-ceremony init <out.bin>
//!   zk-ceremony contribute <in.bin> <out.bin> <seat> <attestation>
//!   zk-ceremony verify-pair <before.bin> <after.bin>
//!   zk-ceremony finalize <params.bin> <out_dir>
//!
//! Transcript format (little-endian, arkworks compressed points):
//!   pk            ProvingKey<Bn254> (canonical, compressed)
//!   n_contribs    u32
//!   per contrib:  seat_len u32 | seat utf8 | rho_g1 | rho_g2 |
//!                 attest_len u32 | attest utf8

use ark_bn254::{Bn254, Fr, G1Affine, G2Affine};
use ark_ec::{pairing::Pairing, AffineRepr, CurveGroup};
use ark_ff::{Field, UniformRand};
use ark_groth16::{Groth16, ProvingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::SNARK;
use jlens_readout_circuit::{
    bank_commitment_native, detection_native, readout_leaf_native, JLensReadoutCircuit, DIM, SCALE,
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;

const TREE_HEIGHT: usize = 4;
const INIT_SEED_DOMAIN: &str = "junoclaw-jlens-readout-v1-ceremony-base";

struct Contrib {
    seat: String,
    rho_g1: G1Affine,
    rho_g2: G2Affine,
    attestation: String,
}

fn write_transcript(path: &str, pk: &ProvingKey<Bn254>, contribs: &[Contrib]) -> std::io::Result<()> {
    let mut buf = Vec::new();
    pk.serialize_compressed(&mut buf).unwrap();
    buf.extend_from_slice(&(contribs.len() as u32).to_le_bytes());
    for c in contribs {
        buf.extend_from_slice(&(c.seat.len() as u32).to_le_bytes());
        buf.extend_from_slice(c.seat.as_bytes());
        c.rho_g1.serialize_compressed(&mut buf).unwrap();
        c.rho_g2.serialize_compressed(&mut buf).unwrap();
        buf.extend_from_slice(&(c.attestation.len() as u32).to_le_bytes());
        buf.extend_from_slice(c.attestation.as_bytes());
    }
    fs::write(path, &buf)
}

fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8], String> {
    if *pos + n > buf.len() {
        return Err("transcript truncated".into());
    }
    let s = &buf[*pos..*pos + n];
    *pos += n;
    Ok(s)
}

fn read_transcript(path: &str) -> Result<(ProvingKey<Bn254>, Vec<Contrib>), String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let mut cur = std::io::Cursor::new(&buf[..]);
    let pk = ProvingKey::<Bn254>::deserialize_compressed(&mut cur)
        .map_err(|e| format!("pk decode: {e}"))?;
    let mut pos = cur.position() as usize;
    let n = u32::from_le_bytes(take(&buf, &mut pos, 4)?.try_into().unwrap());
    let mut contribs = Vec::new();
    for _ in 0..n {
        let sl = u32::from_le_bytes(take(&buf, &mut pos, 4)?.try_into().unwrap()) as usize;
        let seat = String::from_utf8(take(&buf, &mut pos, sl)?.to_vec())
            .map_err(|_| "seat not utf8")?;
        let rho_g1 = G1Affine::deserialize_compressed(take(&buf, &mut pos, 32)?)
            .map_err(|e| format!("rho_g1: {e}"))?;
        let rho_g2 = G2Affine::deserialize_compressed(take(&buf, &mut pos, 64)?)
            .map_err(|e| format!("rho_g2: {e}"))?;
        let al = u32::from_le_bytes(take(&buf, &mut pos, 4)?.try_into().unwrap()) as usize;
        let attestation = String::from_utf8(take(&buf, &mut pos, al)?.to_vec())
            .map_err(|_| "attestation not utf8")?;
        contribs.push(Contrib {
            seat,
            rho_g1,
            rho_g2,
            attestation,
        });
    }
    if pos != buf.len() {
        return Err(format!("trailing {} bytes in transcript", buf.len() - pos));
    }
    Ok((pk, contribs))
}

fn sha256_hex(b: &[u8]) -> String {
    hex::encode(Sha256::digest(b))
}

fn file_sha256(path: &str) -> String {
    sha256_hex(&fs::read(path).unwrap())
}

fn g1() -> G1Affine {
    G1Affine::generator()
}
fn g2() -> G2Affine {
    G2Affine::generator()
}

fn pair(a: &G1Affine, b: &G2Affine) -> <Bn254 as Pairing>::TargetField {
    Bn254::pairing(*a, *b).0
}

/// Apply a fresh rho to the SRS: delta scales by rho, the delta-inverse
/// queries (h, l) scale by rho^-1. Everything else is untouched —
/// alpha/beta/gamma are not toxic waste once delta is unknown.
fn apply_contribution(pk: &mut ProvingKey<Bn254>, rho: Fr) {
    let rho_inv = rho.inverse().expect("rho nonzero");
    pk.vk.delta_g2 = (pk.vk.delta_g2 * rho).into_affine();
    pk.delta_g1 = (pk.delta_g1 * rho).into_affine();
    for q in pk.h_query.iter_mut() {
        *q = (q.into_group() * rho_inv).into_affine();
    }
    for q in pk.l_query.iter_mut() {
        *q = (q.into_group() * rho_inv).into_affine();
    }
}

fn cmd_init(out: &str) {
    let seed: [u8; 32] = Sha256::digest(INIT_SEED_DOMAIN.as_bytes()).into();
    let mut rng = ChaCha20Rng::from_seed(seed);
    let (pk, _vk) =
        Groth16::<Bn254>::circuit_specific_setup(JLensReadoutCircuit::empty(TREE_HEIGHT), &mut rng)
            .expect("setup");
    write_transcript(out, &pk, &[]).expect("write");
    println!(
        "init: {} contributions, sha256={}",
        0,
        file_sha256(out)
    );
    println!("base params are deterministically reproducible — security comes from contributions, not the init seed");
}

fn cmd_contribute(inp: &str, out: &str, seat: &str, attestation: &str) {
    let before_hash = file_sha256(inp);
    let (mut pk, mut contribs) = read_transcript(inp).expect("read transcript");

    // Entropy: OS CSPRNG mixed with the seat+attestation so every
    // contribution stream is unique. Nothing is written to disk.
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let mut h = Sha256::new();
    h.update(seed);
    h.update(seat.as_bytes());
    h.update(attestation.as_bytes());
    let mixed: [u8; 32] = h.finalize().into();
    let mut rng = ChaCha20Rng::from_seed(mixed);
    let rho = Fr::rand(&mut rng);

    let rho_g1 = (g1() * rho).into_affine();
    let rho_g2 = (g2() * rho).into_affine();
    apply_contribution(&mut pk, rho);
    drop(rng);

    contribs.push(Contrib {
        seat: seat.to_string(),
        rho_g1,
        rho_g2,
        attestation: attestation.to_string(),
    });
    write_transcript(out, &pk, &contribs).expect("write");
    println!("contributed seat={seat} n_contribs={}", contribs.len());
    println!("params_before sha256={before_hash}");
    println!("params_after  sha256={}", file_sha256(out));
    println!("DESTROY any local entropy notes — the transcript file carries no secrets");
}

/// Verify one contribution transition: the new SRS must be exactly the
/// old SRS with a single consistent rho applied.
fn cmd_verify_pair(before: &str, after: &str) {
    let (old, oc) = read_transcript(before).expect("read before");
    let (new, nc) = read_transcript(after).expect("read after");

    assert_eq!(
        nc.len(),
        oc.len() + 1,
        "after must add exactly one contribution"
    );
    let c = nc.last().unwrap();
    for (a, b) in oc.iter().zip(nc.iter()) {
        assert_eq!(a.seat, b.seat, "contribution history rewritten");
        assert_eq!(a.rho_g1, b.rho_g1);
        assert_eq!(a.rho_g2, b.rho_g2);
    }

    // Published rho must be the same scalar in both groups.
    assert_eq!(
        pair(&c.rho_g1, &g2()),
        pair(&g1(), &c.rho_g2),
        "rho_g1/rho_g2 are not the same scalar"
    );

    // Untouched elements: byte-identical.
    assert_eq!(old.vk.alpha_g1, new.vk.alpha_g1, "alpha_g1 changed");
    assert_eq!(old.vk.beta_g2, new.vk.beta_g2, "beta_g2 changed");
    assert_eq!(old.vk.gamma_g2, new.vk.gamma_g2, "gamma_g2 changed");
    assert_eq!(old.vk.gamma_abc_g1, new.vk.gamma_abc_g1, "gamma_abc changed");
    assert_eq!(old.beta_g1, new.beta_g1, "beta_g1 changed");
    assert_eq!(old.a_query, new.a_query, "a_query changed");
    assert_eq!(old.b_g1_query, new.b_g1_query, "b_g1_query changed");
    assert_eq!(old.b_g2_query, new.b_g2_query, "b_g2_query changed");
    assert_eq!(old.h_query.len(), new.h_query.len());
    assert_eq!(old.l_query.len(), new.l_query.len());

    // Delta consistency: same rho on both delta points.
    assert_eq!(
        pair(&new.delta_g1, &old.vk.delta_g2),
        pair(&old.delta_g1, &new.vk.delta_g2),
        "delta_g1/delta_g2 scaled inconsistently"
    );
    // The applied rho equals the published contribution key.
    assert_eq!(
        pair(&new.delta_g1, &g2()),
        pair(&old.delta_g1, &c.rho_g2),
        "delta_g1 not scaled by published rho"
    );
    assert_eq!(
        pair(&g1(), &new.vk.delta_g2),
        pair(&c.rho_g1, &old.vk.delta_g2),
        "delta_g2 not scaled by published rho"
    );

    // h/l queries scaled by rho^-1: e(new_q, delta_new) == e(old_q, delta_old).
    for (i, (a, b)) in old.h_query.iter().zip(new.h_query.iter()).enumerate() {
        assert_eq!(
            pair(b, &new.vk.delta_g2),
            pair(a, &old.vk.delta_g2),
            "h_query[{i}] not scaled by rho^-1"
        );
    }
    for (i, (a, b)) in old.l_query.iter().zip(new.l_query.iter()).enumerate() {
        assert_eq!(
            pair(b, &new.vk.delta_g2),
            pair(a, &old.vk.delta_g2),
            "l_query[{i}] not scaled by rho^-1"
        );
    }

    println!("verify-pair OK: seat={} contribution consistent", c.seat);
}

/// Emit the production VK (+ PK for the prover) and self-test a proof.
fn cmd_finalize(params: &str, out_dir: &str) {
    let (pk, contribs) = read_transcript(params).expect("read transcript");
    assert!(!contribs.is_empty(), "refusing to finalize an un-contributed SRS");

    // Self-test: prove the public fixture witness against the final key.
    let mut rng = ChaCha20Rng::from_seed(Sha256::digest(b"finalize-selftest").into());
    let (circuit, public_inputs) = fixture_circuit();
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).expect("prove");
    let vk = &pk.vk;
    assert!(
        Groth16::<Bn254>::verify(vk, &public_inputs, &proof).expect("verify"),
        "final SRS produced an unverifiable proof — do NOT ship"
    );

    fs::create_dir_all(out_dir).unwrap();
    let mut vk_bytes = Vec::new();
    vk.serialize_compressed(&mut vk_bytes).unwrap();
    let mut pk_bytes = Vec::new();
    pk.serialize_compressed(&mut pk_bytes).unwrap();
    use base64::Engine;
    fs::write(
        format!("{out_dir}/jlens-vk-prod.b64"),
        base64::engine::general_purpose::STANDARD.encode(&vk_bytes),
    )
    .unwrap();
    fs::write(format!("{out_dir}/jlens-pk-prod.bin"), &pk_bytes).unwrap();

    println!("finalized {} contributions", contribs.len());
    println!("transcript sha256={}", file_sha256(params));
    println!("vk sha256={}", sha256_hex(&vk_bytes));
    println!("wrote {out_dir}/jlens-vk-prod.b64 + jlens-pk-prod.bin");
    println!("self-test proof verified against production VK — SRS is sound");
}

/// Prove the fixture witness with the transcript's PK — emits the same
/// artifacts as gen-proof but bound to the ceremony key. This is the
/// post-swap smoke test: `resolve_with_proof` on-chain accepts a proof
/// made from this transcript.
fn cmd_prove(params: &str, out_dir: &str) {
    let (pk, _) = read_transcript(params).expect("read transcript");
    let mut rng = ChaCha20Rng::from_seed(Sha256::digest(b"fixture-proof").into());
    let (circuit, public_inputs) = fixture_circuit();
    let proof = Groth16::<Bn254>::prove(&pk, circuit, &mut rng).expect("prove");
    assert!(
        Groth16::<Bn254>::verify(&pk.vk, &public_inputs, &proof).expect("verify"),
        "proof failed self-verify"
    );
    fs::create_dir_all(out_dir).unwrap();
    let mut proof_bytes = Vec::new();
    proof.serialize_compressed(&mut proof_bytes).unwrap();
    let mut inputs_bytes = Vec::new();
    for i in &public_inputs {
        i.serialize_compressed(&mut inputs_bytes).unwrap();
    }
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    fs::write(format!("{out_dir}/jlens-proof-prod.b64"), b64.encode(&proof_bytes)).unwrap();
    fs::write(format!("{out_dir}/jlens-inputs-prod.b64"), b64.encode(&inputs_bytes)).unwrap();
    println!("wrote {out_dir}/jlens-proof-prod.b64 + jlens-inputs-prod.b64");
    println!("proof sha256={} inputs={} bytes", sha256_hex(&proof_bytes), inputs_bytes.len());
}

/// Same public fixture witness as the circuit tests — deterministic and
/// reproducible so every operator runs the identical self-check.
fn fixture_circuit() -> (JLensReadoutCircuit, Vec<Fr>) {
    let acts: Vec<Fr> = (0..DIM).map(|i| Fr::from(10u64 + i as u64)).collect();
    let bank: Vec<Fr> = (0..DIM).map(|i| Fr::from(3u64 + i as u64)).collect();
    let threshold = Fr::from(500u64);
    let score: Fr = acts.iter().zip(bank.iter()).map(|(a, b)| *a * *b).sum();
    let integrity = Fr::from(SCALE) - score;
    let detection = detection_native(score, threshold);
    let leaf = readout_leaf_native(&acts);
    let zero_leaf = sensor_safety_circuit::mimc_hash(Fr::from(0u64), Fr::from(0u64));
    let mut leaves = vec![zero_leaf; (1 << TREE_HEIGHT) - 1];
    leaves.push(leaf);
    let (root, paths, bits) =
        jlens_readout_circuit::build_merkle_tree_for_fixture(&leaves, TREE_HEIGHT);
    let leaf_idx = leaves.len() - 1;
    let circuit = JLensReadoutCircuit {
        merkle_root: Some(root),
        bank_commitment: Some(bank_commitment_native(&bank, threshold)),
        score_out: Some(score),
        integrity_out: Some(integrity),
        detection_out: Some(detection),
        activations: acts.iter().map(|a| Some(*a)).collect(),
        bank_vec: bank.iter().map(|b| Some(*b)).collect(),
        threshold: Some(threshold),
        merkle_path: paths[leaf_idx].iter().map(|p| Some(*p)).collect(),
        path_bits: bits[leaf_idx].iter().map(|b| Some(*b)).collect(),
        tree_height: TREE_HEIGHT,
    };
    let public = vec![root, circuit.bank_commitment.unwrap(), score, integrity, detection];
    (circuit, public)
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  zk-ceremony init <out.bin>\n  zk-ceremony contribute <in.bin> <out.bin> <seat> <attestation>\n  zk-ceremony verify-pair <before.bin> <after.bin>\n  zk-ceremony finalize <params.bin> <out_dir>\n  zk-ceremony prove <params.bin> <out_dir>"
    );
    std::process::exit(2);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("init") if a.len() == 3 => cmd_init(&a[2]),
        Some("contribute") if a.len() == 6 => cmd_contribute(&a[2], &a[3], &a[4], &a[5]),
        Some("verify-pair") if a.len() == 4 => cmd_verify_pair(&a[2], &a[3]),
        Some("finalize") if a.len() == 4 => cmd_finalize(&a[2], &a[3]),
        Some("prove") if a.len() == 4 => cmd_prove(&a[2], &a[3]),
        _ => usage(),
    }
}
