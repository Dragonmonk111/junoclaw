//! Project Aegis — ML-DSA-44 vs ML-DSA-65 decision model.
//!
//! Reproduces the size + consensus-bandwidth tables in
//! `docs/PROJECT_AEGIS_JUNO_FULL_PQC.md` §5.1, so the "ML-DSA-44 vs 65"
//! recommendation rests on numbers anyone can regenerate rather than a claim.
//!
//! The default build is dependency-free and runs fully offline — the size and
//! bandwidth figures are *spec constants* (FIPS 204 / RFC 8032 / SEC) plus
//! arithmetic, so no cryptographic library is needed to compute them.
//!
//! Build with `--features timing` to additionally measure real ML-DSA-44/65
//! keygen / sign / verify wall-clock time via the pure-Rust `fips204` crate
//! (that path requires a one-time crates.io fetch).
//!
//! Units are SI/decimal (1 KB = 1,000 B), matching how block storage and
//! bandwidth are conventionally quoted.

// ---- Spec-fixed primitive sizes (bytes) ------------------------------------

const ED25519_PK: usize = 32;
const ED25519_SIG: usize = 64;
const SECP256K1_PK: usize = 33; // compressed
const SECP256K1_SIG: usize = 64;

/// (name, NIST category label, public-key bytes, signature bytes)
struct Scheme {
    name: &'static str,
    cat: &'static str,
    pk: usize,
    sig: usize,
}

const ML_DSA_44: Scheme = Scheme { name: "ML-DSA-44", cat: "2", pk: 1312, sig: 2420 };
const ML_DSA_65: Scheme = Scheme { name: "ML-DSA-65", cat: "3", pk: 1952, sig: 3309 };

/// A hybrid pairs a classical signature with a PQC one (both must verify).
const fn hybrid_pk(classical_pk: usize, pqc_pk: usize) -> usize {
    classical_pk + pqc_pk
}
const fn hybrid_sig(classical_sig: usize, pqc_sig: usize) -> usize {
    classical_sig + pqc_sig
}

// ---- Consensus model parameters --------------------------------------------

const VALIDATOR_COUNTS: [usize; 3] = [50, 100, 150];
const BLOCK_SECONDS: f64 = 6.0;
const PROJECTION_VALIDATORS: usize = 100;

fn blocks_per_day() -> f64 {
    86_400.0 / BLOCK_SECONDS
}

// ---- Formatting (decimal / SI) ---------------------------------------------

fn human(bytes: f64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    const TB: f64 = 1_000_000_000_000.0;
    if bytes >= TB {
        format!("{:.2} TB", bytes / TB)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{:.0} B", bytes)
    }
}

fn main() {
    println!("Project Aegis — ML-DSA-44 vs ML-DSA-65 decision model");
    println!("(SI units: 1 KB = 1,000 B; block time = {} s)\n", BLOCK_SECONDS);

    // 1) Fixed sizes -------------------------------------------------------
    println!("== Fixed primitive sizes (spec constants) ==");
    println!(
        "{:<32} {:>6} {:>12} {:>12}",
        "Scheme", "cat", "pubkey (B)", "sig (B)"
    );
    println!(
        "{:<32} {:>6} {:>12} {:>12}",
        "Ed25519 (current consensus)", "-", ED25519_PK, ED25519_SIG
    );
    println!(
        "{:<32} {:>6} {:>12} {:>12}",
        "secp256k1 (current accounts)", "-", SECP256K1_PK, SECP256K1_SIG
    );
    for s in [&ML_DSA_44, &ML_DSA_65] {
        println!("{:<32} {:>6} {:>12} {:>12}", s.name, s.cat, s.pk, s.sig);
    }
    let h44_pk = hybrid_pk(ED25519_PK, ML_DSA_44.pk);
    let h44_sig = hybrid_sig(ED25519_SIG, ML_DSA_44.sig);
    let h65_pk = hybrid_pk(ED25519_PK, ML_DSA_65.pk);
    let h65_sig = hybrid_sig(ED25519_SIG, ML_DSA_65.sig);
    println!(
        "{:<32} {:>6} {:>12} {:>12}",
        "Hybrid Ed25519+ML-DSA-44", "2", h44_pk, h44_sig
    );
    println!(
        "{:<32} {:>6} {:>12} {:>12}",
        "Hybrid Ed25519+ML-DSA-65", "3", h65_pk, h65_sig
    );

    // 2) Per-block commit signature payload --------------------------------
    println!("\n== Commit signature payload per block (sig bytes only) ==");
    println!(
        "{:>11} {:>16} {:>14} {:>14} {:>18}",
        "validators", "Ed25519 base", "Hybrid-44", "Hybrid-65", "65 over 44"
    );
    for &n in &VALIDATOR_COUNTS {
        let base = (n * ED25519_SIG) as f64;
        let h44 = (n * h44_sig) as f64;
        let h65 = (n * h65_sig) as f64;
        println!(
            "{:>11} {:>16} {:>14} {:>14} {:>18}",
            n,
            human(base),
            human(h44),
            human(h65),
            format!("+{}", human(h65 - h44)),
        );
    }

    // 3) Block-data growth projection --------------------------------------
    let n = PROJECTION_VALIDATORS;
    let bpd = blocks_per_day();
    let bpy = bpd * 365.0;
    let base_blk = (n * ED25519_SIG) as f64;
    let h44_blk = (n * h44_sig) as f64;
    let h65_blk = (n * h65_sig) as f64;
    let delta_blk = h65_blk - h44_blk;

    println!(
        "\n== Block-data growth from commits (N={}, {} blocks/day) ==",
        n, bpd as u64
    );
    println!(
        "{:<28} {:>12} {:>12} {:>12}",
        "", "per block", "per day", "per year"
    );
    let row = |label: &str, per_block: f64| {
        println!(
            "{:<28} {:>12} {:>12} {:>12}",
            label,
            human(per_block),
            human(per_block * bpd),
            human(per_block * bpy),
        );
    };
    row("Ed25519 baseline", base_blk);
    row("Hybrid-44", h44_blk);
    row("Hybrid-65", h65_blk);
    row("Delta: 65 over 44", delta_blk);

    // 4) Recommendation summary --------------------------------------------
    let savings_pct = 100.0 * (1.0 - (h44_sig as f64 / h65_sig as f64));
    println!("\n== Reading ==");
    println!(
        "- Going PQC at consensus costs ~{} / year of block growth at N={} \
         regardless of level.",
        human(h44_blk * bpy),
        n
    );
    println!(
        "- Choosing ML-DSA-65 over ML-DSA-44 adds ~{} / year for one extra NIST \
         category.",
        human(delta_blk * bpy)
    );
    println!(
        "- ML-DSA-44 saves {:.1}% of signature bytes vs 65; recommended as the \
         consensus/account workhorse (pending verify-CPU + on-chain gas).",
        savings_pct
    );

    #[cfg(feature = "timing")]
    timing::run();

    #[cfg(not(feature = "timing"))]
    println!(
        "\n(timing skipped — rebuild with `cargo run --features timing` for real \
         ML-DSA keygen/sign/verify measurements.)"
    );
}

// ---- Optional real timing via pure-Rust FIPS 204 ---------------------------

#[cfg(feature = "timing")]
mod timing {
    use fips204::traits::{SerDes, Signer, Verifier};
    use std::time::Instant;

    const ITERS: u32 = 200;
    const MSG: &[u8] = b"project-aegis ml-dsa timing benchmark message";
    const CTX: &[u8] = b""; // empty context per FIPS 204

    pub fn run() {
        println!("\n== Real ML-DSA timing ({} iters, pure-Rust fips204) ==", ITERS);
        println!(
            "{:<12} {:>12} {:>12} {:>12} {:>10} {:>10}",
            "variant", "keygen (us)", "sign (us)", "verify (us)", "pk (B)", "sig (B)"
        );
        bench_44();
        bench_65();
        println!("(verify-time ratio is the consensus-relevant number; lower is better.)");
    }

    macro_rules! bench_variant {
        ($modname:path, $label:literal) => {{
            use $modname as v;
            // keygen
            let t = Instant::now();
            let mut last = None;
            for _ in 0..ITERS {
                last = Some(v::try_keygen().expect("keygen"));
            }
            let keygen_us = t.elapsed().as_micros() as f64 / ITERS as f64;
            let (pk, sk) = last.unwrap();

            // sign
            let t = Instant::now();
            let mut sig = None;
            for _ in 0..ITERS {
                sig = Some(sk.try_sign(MSG, CTX).expect("sign"));
            }
            let sign_us = t.elapsed().as_micros() as f64 / ITERS as f64;
            let sig = sig.unwrap();

            // verify
            let t = Instant::now();
            let mut ok = false;
            for _ in 0..ITERS {
                ok = pk.verify(MSG, &sig, CTX);
            }
            let verify_us = t.elapsed().as_micros() as f64 / ITERS as f64;
            assert!(ok, "signature must verify");

            println!(
                "{:<12} {:>12.1} {:>12.1} {:>12.1} {:>10} {:>10}",
                $label,
                keygen_us,
                sign_us,
                verify_us,
                pk.into_bytes().len(),
                sig.len()
            );
        }};
    }

    fn bench_44() {
        bench_variant!(fips204::ml_dsa_44, "ML-DSA-44");
    }
    fn bench_65() {
        bench_variant!(fips204::ml_dsa_65, "ML-DSA-65");
    }
}
