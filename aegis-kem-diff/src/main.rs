//! Project Aegis — Phase C / C2b: ML-KEM-768 differential conformance oracle.
//!
//! Cross-checks two *independent* Rust ML-KEM-768 implementations against each
//! other, byte-for-byte, exactly as
//! `docs/ADR-006-PQC-HYBRID-TRANSPORT.md` requires
//! (§"Crate / implementation selection", §"Security & determinism"):
//!
//!   * `fips203`        — integrity-chain (Eric Schorn). Same author and audit
//!                        posture as the `fips204` (ML-DSA) crate the rest of
//!                        Aegis already vendors (`cosmwasm-crypto-mldsa`).
//!   * `libcrux-ml-kem` — Cryspen. HACL*/F*-verified ML-KEM — the *formal*
//!                        cross-check oracle.
//!
//! The Go standard library `crypto/mlkem` is the **runtime** implementation
//! (see `../aegis-transport`). The three implementations together form the
//! three-way differential ADR-006 commits to; this crate is the Rust↔Rust leg
//! plus the ACVP wiring point (see `ACVP_WIRING.md`).
//!
//! What "differential" means here is the strong test: not just that each impl
//! round-trips with itself, but that an artifact produced by impl A
//! (encapsulation key / ciphertext) is consumed by impl B and yields the
//! *identical* 32-byte shared secret. A divergence in either impl's FIPS 203
//! interpretation surfaces immediately.
//!
//! Run:  `cargo run --release -- [iterations]`   (default 256)
//! Test: `cargo test`

use fips203::ml_kem_768::{self, CipherText, EncapsKey, KG};
use fips203::traits::{Decaps, Encaps, KeyGen, SerDes};
use libcrux_ml_kem::mlkem768::{self, MlKem768Ciphertext, MlKem768PublicKey};
use rand::RngCore;

// FIPS 203 fixed sizes for ML-KEM-768 (bytes). Asserted against both crates'
// own constants in `check_sizes` so any drift fails loudly.
const EK_LEN: usize = 1184; // encapsulation key
const CT_LEN: usize = 1088; // ciphertext
const DK_LEN: usize = 2400; // decapsulation key (kept local)
const SS_LEN: usize = 32; // shared secret

/// Fill an N-byte array from the OS CSPRNG (used only to seed each impl's
/// explicit randomness input — keygen seed `d||z` (64 B) and encaps `m` (32 B)).
fn rand_bytes<const N: usize>() -> [u8; N] {
    let mut b = [0u8; N];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b
}

/// `fips203` self round-trip: keygen → encaps → decaps must agree.
fn fips_roundtrip() -> Result<(), String> {
    let (ek, dk) = KG::try_keygen().map_err(|e| format!("fips keygen: {e}"))?;
    let (ssk_enc, ct) = ek.try_encaps().map_err(|e| format!("fips encaps: {e}"))?;
    let ssk_dec = dk.try_decaps(&ct).map_err(|e| format!("fips decaps: {e}"))?;
    if ssk_enc.into_bytes() != ssk_dec.into_bytes() {
        return Err("fips203 self round-trip shared-secret mismatch".into());
    }
    Ok(())
}

/// `libcrux-ml-kem` self round-trip: keygen → encaps → decaps must agree.
fn libcrux_roundtrip() -> Result<(), String> {
    let kp = mlkem768::generate_key_pair(rand_bytes::<64>());
    let (ct, ss_enc) = mlkem768::encapsulate(kp.public_key(), rand_bytes::<32>());
    let ss_dec = mlkem768::decapsulate(kp.private_key(), &ct);
    if ss_enc != ss_dec {
        return Err("libcrux self round-trip shared-secret mismatch".into());
    }
    Ok(())
}

/// Cross-direction A: `fips203` produces the keypair, `libcrux` encapsulates
/// against the fips203 encapsulation key, and `fips203` decapsulates the
/// libcrux ciphertext. Both must derive the same shared secret.
fn cross_fips_ek_libcrux_encaps() -> Result<(), String> {
    let (ek, dk) = KG::try_keygen().map_err(|e| format!("fips keygen: {e}"))?;
    let ek_bytes: [u8; EK_LEN] = ek.into_bytes();

    // libcrux encapsulates against fips203's serialized encapsulation key.
    let pk = MlKem768PublicKey::from(ek_bytes);
    let (ct, ss_libcrux) = mlkem768::encapsulate(&pk, rand_bytes::<32>());

    // fips203 decapsulates libcrux's serialized ciphertext.
    let ct_bytes: [u8; CT_LEN] = ct
        .as_ref()
        .try_into()
        .map_err(|_| "libcrux ciphertext length != CT_LEN".to_string())?;
    let ct_fips = CipherText::try_from_bytes(ct_bytes).map_err(|e| format!("fips ct parse: {e}"))?;
    let ss_fips = dk.try_decaps(&ct_fips).map_err(|e| format!("fips decaps: {e}"))?;

    if ss_fips.into_bytes() != ss_libcrux {
        return Err("cross A mismatch (fips ek / libcrux encaps / fips decaps)".into());
    }
    Ok(())
}

/// Cross-direction B: `libcrux` produces the keypair, `fips203` encapsulates
/// against the libcrux encapsulation key, and `libcrux` decapsulates the
/// fips203 ciphertext. Both must derive the same shared secret.
fn cross_libcrux_ek_fips_encaps() -> Result<(), String> {
    let kp = mlkem768::generate_key_pair(rand_bytes::<64>());
    let ek_bytes: [u8; EK_LEN] = *kp.pk();

    // fips203 encapsulates against libcrux's serialized encapsulation key.
    let ek_fips = EncapsKey::try_from_bytes(ek_bytes).map_err(|e| format!("fips ek parse: {e}"))?;
    let (ss_fips, ct_fips) = ek_fips.try_encaps().map_err(|e| format!("fips encaps: {e}"))?;

    // libcrux decapsulates fips203's serialized ciphertext.
    let ct_bytes: [u8; CT_LEN] = ct_fips.into_bytes();
    let ct_libcrux = MlKem768Ciphertext::from(ct_bytes);
    let ss_libcrux = mlkem768::decapsulate(kp.private_key(), &ct_libcrux);

    if ss_fips.into_bytes() != ss_libcrux {
        return Err("cross B mismatch (libcrux ek / fips encaps / libcrux decaps)".into());
    }
    Ok(())
}

/// Assert the FIPS 203 fixed sizes agree across the spec, both crates, and the
/// artifacts they actually emit. A drift in any of these is a hard failure.
fn check_sizes() -> Result<(), String> {
    if ml_kem_768::EK_LEN != EK_LEN || ml_kem_768::CT_LEN != CT_LEN || ml_kem_768::DK_LEN != DK_LEN {
        return Err(format!(
            "fips203 size constants drifted: ek={} ct={} dk={}",
            ml_kem_768::EK_LEN,
            ml_kem_768::CT_LEN,
            ml_kem_768::DK_LEN
        ));
    }
    let kp = mlkem768::generate_key_pair(rand_bytes::<64>());
    if kp.pk().len() != EK_LEN || kp.sk().len() != DK_LEN {
        return Err(format!(
            "libcrux key sizes drifted: ek={} dk={}",
            kp.pk().len(),
            kp.sk().len()
        ));
    }
    let (ct, ss) = mlkem768::encapsulate(kp.public_key(), rand_bytes::<32>());
    if ct.as_ref().len() != CT_LEN || ss.len() != SS_LEN {
        return Err(format!(
            "libcrux ct/ss sizes drifted: ct={} ss={}",
            ct.as_ref().len(),
            ss.len()
        ));
    }
    Ok(())
}

/// One full differential round: every self and cross check.
fn one_round() -> Result<(), String> {
    fips_roundtrip()?;
    libcrux_roundtrip()?;
    cross_fips_ek_libcrux_encaps()?;
    cross_libcrux_ek_fips_encaps()?;
    Ok(())
}

fn main() {
    let iters: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);

    println!("Project Aegis — Phase C/C2b: ML-KEM-768 differential conformance oracle");
    println!("  primary : fips203 (integrity-chain)");
    println!("  oracle  : libcrux-ml-kem (Cryspen, HACL*/F*-verified)");
    println!("  runtime : Go stdlib crypto/mlkem (see ../aegis-transport)");
    println!();

    if let Err(e) = check_sizes() {
        eprintln!("FAIL (sizes): {e}");
        std::process::exit(1);
    }
    println!("sizes OK: ek={EK_LEN} ct={CT_LEN} dk={DK_LEN} ss={SS_LEN} (FIPS 203)");

    for i in 0..iters {
        if let Err(e) = one_round() {
            eprintln!("FAIL at iteration {i}: {e}");
            std::process::exit(1);
        }
    }

    println!(
        "PASS: {iters} iterations × {{self×2, cross×2}} — fips203 ↔ libcrux-ml-kem agree byte-for-byte"
    );
    println!("(NIST ACVP KAT wiring: see ACVP_WIRING.md)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_fips203() {
        check_sizes().unwrap();
    }

    #[test]
    fn fips203_self_roundtrip() {
        for _ in 0..32 {
            fips_roundtrip().unwrap();
        }
    }

    #[test]
    fn libcrux_self_roundtrip() {
        for _ in 0..32 {
            libcrux_roundtrip().unwrap();
        }
    }

    #[test]
    fn cross_fips_ek_libcrux_encaps_agree() {
        for _ in 0..32 {
            cross_fips_ek_libcrux_encaps().unwrap();
        }
    }

    #[test]
    fn cross_libcrux_ek_fips_encaps_agree() {
        for _ in 0..32 {
            cross_libcrux_ek_fips_encaps().unwrap();
        }
    }

    /// A tampered ciphertext must NOT yield the encapsulator's shared secret.
    /// FIPS 203 implicit rejection makes decaps return a pseudo-random secret
    /// (not an error), so the two sides simply fail to agree.
    #[test]
    fn tampered_ciphertext_breaks_agreement() {
        let (ek, dk) = KG::try_keygen().unwrap();
        let (ss_enc, ct) = ek.try_encaps().unwrap();
        let mut ct_bytes: [u8; CT_LEN] = ct.into_bytes();
        ct_bytes[0] ^= 0xFF;
        let ct_bad = CipherText::try_from_bytes(ct_bytes).unwrap();
        let ss_dec = dk.try_decaps(&ct_bad).unwrap();
        assert_ne!(
            ss_enc.into_bytes(),
            ss_dec.into_bytes(),
            "tampered ciphertext still agreed"
        );
    }
}
