//! Known-answer tests (KATs) for all four MAYO parameter sets.
//!
//! Vectors in `tests/vectors/mayo{1,2,3,5}.txt` were produced by the
//! reference C implementation (sriracha-mayo, ChaCha20Rng seed=[42;32])
//! via the crate's `print_vectors` tests:
//!
//!   cargo test --features test-c -- print_vectors --ignored --nocapture
//!
//! (requires cmake + a C toolchain; see scripts/cross-check.sh). Each file
//! carries `msg` (UTF-8), `pk`, `sig`, and `pk_sha256` so the fixture is
//! self-authenticating against corruption.
//!
//! The contract under test: a reference-impl signature MUST verify, and
//! ANY single-byte corruption of sig/pk/msg MUST be rejected — the
//! verifier must never accept garbage, even bit-adjacent garbage.

use hex::decode;
use junoclaw_mayo_verify::{verify, Mayo1, Mayo2, Mayo3, Mayo5, ParameterSet};
use sha2::{Digest, Sha256};

struct Kat {
    msg: Vec<u8>,
    pk: Vec<u8>,
    sig: Vec<u8>,
    pk_sha256: Vec<u8>,
}

fn parse_kat(data: &str) -> Kat {
    let mut kat = Kat {
        msg: Vec::new(),
        pk: Vec::new(),
        sig: Vec::new(),
        pk_sha256: Vec::new(),
    };
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once(':').expect("KAT line must be `key: value`");
        let v = v.trim();
        match k.trim() {
            "msg" => kat.msg = v.as_bytes().to_vec(),
            "msg_hex" => kat.msg = decode(v).expect("msg_hex must be hex"),
            "pk" => kat.pk = decode(v).expect("pk must be hex"),
            "sig" => kat.sig = decode(v).expect("sig must be hex"),
            "pk_sha256" => kat.pk_sha256 = decode(v).expect("pk_sha256 must be hex"),
            other => panic!("unknown KAT key: {other}"),
        }
    }
    kat
}

fn run_kat<P: ParameterSet>(data: &str) {
    let kat = parse_kat(data);

    // Fixture shape: lengths match the parameter set exactly.
    assert_eq!(kat.pk.len(), P::PK_BYTES, "{}: KAT pk length", P::NAME);
    assert_eq!(kat.sig.len(), P::SIG_BYTES, "{}: KAT sig length", P::NAME);

    // Fixture integrity: pk_sha256 pins the public key bytes.
    assert_eq!(
        Sha256::digest(&kat.pk).as_slice(),
        kat.pk_sha256.as_slice(),
        "{}: KAT pk_sha256 mismatch — corrupted fixture",
        P::NAME
    );

    // Known answer: reference-impl signature verifies.
    assert!(
        verify::<P>(&kat.msg, &kat.sig, &kat.pk).expect("right-length input must not error"),
        "{}: valid reference signature rejected",
        P::NAME
    );

    // Negative: flip the first sig byte.
    let mut bad_sig = kat.sig.clone();
    bad_sig[0] ^= 0x01;
    assert!(
        !verify::<P>(&kat.msg, &bad_sig, &kat.pk).unwrap(),
        "{}: corrupted sig accepted (first byte)",
        P::NAME
    );
    // Negative: flip the last sig byte (salt-adjacent region).
    let mut bad_sig = kat.sig.clone();
    let last = bad_sig.len() - 1;
    bad_sig[last] ^= 0x80;
    assert!(
        !verify::<P>(&kat.msg, &bad_sig, &kat.pk).unwrap(),
        "{}: corrupted sig accepted (last byte)",
        P::NAME
    );

    // Negative: flip a pk byte.
    let mut bad_pk = kat.pk.clone();
    bad_pk[P::PK_BYTES / 2] ^= 0x01;
    assert!(
        !verify::<P>(&kat.msg, &kat.sig, &bad_pk).unwrap(),
        "{}: corrupted pk accepted",
        P::NAME
    );

    // Negative: wrong message.
    let mut bad_msg = kat.msg.clone();
    bad_msg.push(b'!');
    assert!(
        !verify::<P>(&bad_msg, &kat.sig, &kat.pk).unwrap(),
        "{}: wrong message accepted",
        P::NAME
    );

    // Cross-variant hygiene: a MAYO-N signature must not verify under a
    // different variant's key (checked implicitly — lengths differ per
    // variant, so this is covered by the length contract in fuzz_inputs).
}

#[test]
fn kat_mayo1() {
    run_kat::<Mayo1>(include_str!("vectors/mayo1.txt"));
}

#[test]
fn kat_mayo2() {
    run_kat::<Mayo2>(include_str!("vectors/mayo2.txt"));
}

#[test]
fn kat_mayo3() {
    run_kat::<Mayo3>(include_str!("vectors/mayo3.txt"));
}

#[test]
fn kat_mayo5() {
    run_kat::<Mayo5>(include_str!("vectors/mayo5.txt"));
}
