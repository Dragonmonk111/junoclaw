#!/usr/bin/env python3
"""Apply all ML-DSA (FIPS 204) wiring edits to a cosmwasm checkout that already
has the BN254 (00-09) and MAYO (10-19) patches applied. Anchors on the
post-MAYO text so the diffs git produces layer cleanly on top of the MAYO
series.

Mirrors regen-mayo-edits.py exactly, one layer up. The host function is
`ml_dsa_verify(variant, pk_ptr, msg_ptr, sig_ptr) -> u32`, variant in
{44, 65, 87}.

Usage: python3 regen-mldsa-edits.py /path/to/cosmwasm
"""
import sys
import pathlib

root = pathlib.Path(sys.argv[1])


def patch(rel, old, new):
    p = root / rel
    s = p.read_text()
    if old not in s:
        raise SystemExit(f"ANCHOR NOT FOUND in {rel}:\n{old[:200]}")
    if s.count(old) != 1:
        raise SystemExit(f"ANCHOR NOT UNIQUE ({s.count(old)}x) in {rel}")
    p.write_text(s.replace(old, new, 1))
    print(f"  edited {rel}")


# ── 20: packages/vm/Cargo.toml ──────────────────────────────────────────
patch(
    "packages/vm/Cargo.toml",
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo" }\n',
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo" }\n'
    'cosmwasm-crypto-mldsa = { version = "0.1.0", path = "../crypto-mldsa" }\n',
)

# ── 21a: packages/vm/src/imports.rs — use statement ─────────────────────
patch(
    "packages/vm/src/imports.rs",
    "use cosmwasm_crypto_mayo::{gas::mayo_verify_cost, mayo_verify, MayoError};\n",
    "use cosmwasm_crypto_mayo::{gas::mayo_verify_cost, mayo_verify, MayoError};\n"
    "use cosmwasm_crypto_mldsa::{gas::ml_dsa_verify_cost, ml_dsa_verify, MlDsaError};\n",
)

# ── 21b: imports.rs — status codes + do_ml_dsa_verify, before do_abort ──
MLDSA_VM = r'''
// ── ML-DSA host-function status codes ─────────────────────────────────────
//
// Returned in the u32 of `do_ml_dsa_verify`'s `VmResult<u32>`. 0 = valid,
// 1 = invalid, >=2 = error (length mismatch, unknown variant, bad key).
const ML_DSA_OK: u32 = 0;
const ML_DSA_INVALID: u32 = 1;
const ML_DSA_ERR_INVALID_INPUT_LENGTH: u32 = 2;
const ML_DSA_ERR_UNKNOWN_VARIANT: u32 = 3;
const ML_DSA_ERR_INVALID_PUBLIC_KEY: u32 = 4;
const ML_DSA_ERR_INVALID_SIGNATURE: u32 = 5;

#[inline]
fn ml_dsa_error_code(err: &MlDsaError) -> u32 {
    match err {
        MlDsaError::InvalidInputLength { .. } => ML_DSA_ERR_INVALID_INPUT_LENGTH,
        MlDsaError::UnknownVariant(_) => ML_DSA_ERR_UNKNOWN_VARIANT,
        MlDsaError::InvalidPublicKey => ML_DSA_ERR_INVALID_PUBLIC_KEY,
        MlDsaError::InvalidSignature => ML_DSA_ERR_INVALID_SIGNATURE,
        _ => ML_DSA_ERR_INVALID_SIGNATURE,
    }
}

/// ML-DSA (FIPS 204) multi-variant signature verification (post-quantum precompile).
///
/// `variant` is one of 44 (ML-DSA-44), 65 (ML-DSA-65), or 87 (ML-DSA-87).
/// Reads `pk`, `msg`, and `sig` from the three guest-supplied Regions.
/// Returns 0 for a valid signature, 1 for invalid, and non-zero error codes
/// for malformed inputs. Gas is charged per variant before any cryptographic
/// work begins. Verification is integer-only and deterministic.
pub fn do_ml_dsa_verify<A: BackendApi + 'static, S: Storage + 'static, Q: Querier + 'static>(
    mut env: FunctionEnvMut<Environment<A, S, Q>>,
    variant: u32,
    pk_ptr: u32,
    msg_ptr: u32,
    sig_ptr: u32,
) -> VmResult<u32> {
    let (data, mut store) = env.data_and_store_mut();

    // v2.2.2 read_region signature: (data, &mut store, ptr, max_len).
    // Caps sized for ML-DSA-87 (pk 2592 B, sig 4627 B) with headroom.
    let pk = read_region(data, &mut store, pk_ptr, 8192)?;
    let msg = read_region(data, &mut store, msg_ptr, 65536)?;
    let sig = read_region(data, &mut store, sig_ptr, 8192)?;

    let gas_info = GasInfo::with_cost(ml_dsa_verify_cost(variant));
    process_gas_info(data, &mut store, gas_info)?;

    match ml_dsa_verify(variant, &pk, &msg, &sig) {
        Ok(true) => Ok(ML_DSA_OK),
        Ok(false) => Ok(ML_DSA_INVALID),
        Err(e) => Ok(ml_dsa_error_code(&e)),
    }
}

'''
patch(
    "packages/vm/src/imports.rs",
    "/// Aborts the contract and shows the given error message\n"
    "pub fn do_abort",
    MLDSA_VM + "/// Aborts the contract and shows the given error message\n"
    "pub fn do_abort",
)

# ── 22: packages/vm/src/compatibility.rs ────────────────────────────────
patch(
    "packages/vm/src/compatibility.rs",
    '    "env.mayo_verify",\n    "env.secp256k1_verify",',
    '    "env.mayo_verify",\n    "env.ml_dsa_verify",\n    "env.secp256k1_verify",',
)

# ── 23a: packages/vm/src/instance.rs — import list ──────────────────────
patch(
    "packages/vm/src/instance.rs",
    "    do_mayo_verify, do_query_chain, do_secp256k1_recover_pubkey, do_secp256k1_verify,",
    "    do_mayo_verify, do_ml_dsa_verify, do_query_chain, do_secp256k1_recover_pubkey, do_secp256k1_verify,",
)

# ── 23b: packages/vm/src/instance.rs — host-fn registration ─────────────
patch(
    "packages/vm/src/instance.rs",
    '        // MAYO post-quantum signature verification host function.\n'
    '        // Multi-variant: 1=MAYO-1, 2=MAYO-2, 3=MAYO-3, 5=MAYO-5.\n'
    '        env_imports.insert(\n'
    '            "mayo_verify",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_mayo_verify),\n'
    '        );\n',
    '        // MAYO post-quantum signature verification host function.\n'
    '        // Multi-variant: 1=MAYO-1, 2=MAYO-2, 3=MAYO-3, 5=MAYO-5.\n'
    '        env_imports.insert(\n'
    '            "mayo_verify",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_mayo_verify),\n'
    '        );\n\n'
    '        // ML-DSA (FIPS 204) post-quantum signature verification host function.\n'
    '        // Multi-variant: 44=ML-DSA-44, 65=ML-DSA-65, 87=ML-DSA-87.\n'
    '        env_imports.insert(\n'
    '            "ml_dsa_verify",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_ml_dsa_verify),\n'
    '        );\n',
)

# ── 24a: packages/std/Cargo.toml — cosmwasm_2_3 feature ─────────────────
patch(
    "packages/std/Cargo.toml",
    'cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254", "dep:cosmwasm-crypto-mayo"]',
    'cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254", "dep:cosmwasm-crypto-mayo", "dep:cosmwasm-crypto-mldsa"]',
)

# ── 24b: packages/std/Cargo.toml — optional dep ─────────────────────────
patch(
    "packages/std/Cargo.toml",
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo", optional = true }\n',
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo", optional = true }\n'
    'cosmwasm-crypto-mldsa = { version = "0.1.0", path = "../crypto-mldsa", optional = true }\n',
)

# ── 25a: packages/std/src/imports.rs — extern declaration ───────────────
patch(
    "packages/std/src/imports.rs",
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn mayo_verify(variant: u32, pk_ptr: u32, msg_ptr: u32, sig_ptr: u32) -> u32;\n",
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn mayo_verify(variant: u32, pk_ptr: u32, msg_ptr: u32, sig_ptr: u32) -> u32;\n\n"
    "    // ML-DSA (FIPS 204) post-quantum signature verification host function.\n"
    "    // Multi-variant: 44=ML-DSA-44, 65=ML-DSA-65, 87=ML-DSA-87.\n"
    "    // Returns 0 = valid, 1 = invalid, >=2 = error.\n"
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn ml_dsa_verify(variant: u32, pk_ptr: u32, msg_ptr: u32, sig_ptr: u32) -> u32;\n",
)

# ── 25b: packages/std/src/imports.rs — Api method (before secp256k1) ────
MLDSA_API = '''    #[cfg(feature = "cosmwasm_2_3")]
    fn ml_dsa_verify(
        &self,
        variant: u32,
        pk: &[u8],
        msg: &[u8],
        sig: &[u8],
    ) -> Result<bool, VerificationError> {
        let send_pk = Region::from_slice(pk);
        let send_msg = Region::from_slice(msg);
        let send_sig = Region::from_slice(sig);
        let result = unsafe {
            ml_dsa_verify(
                variant,
                send_pk.as_ptr() as u32,
                send_msg.as_ptr() as u32,
                send_sig.as_ptr() as u32,
            )
        };
        match result {
            0 => Ok(true),
            1 => Ok(false),
            code => Err(ml_dsa_error_from_code(code)),
        }
    }
'''
patch(
    "packages/std/src/imports.rs",
    "    fn secp256k1_verify(\n"
    "        &self,\n"
    "        message_hash: &[u8],",
    MLDSA_API + "    fn secp256k1_verify(\n"
    "        &self,\n"
    "        message_hash: &[u8],",
)

# ── 25c: packages/std/src/imports.rs — error mapper (after mayo one) ────
patch(
    "packages/std/src/imports.rs",
    '        c => VerificationError::generic_err(format!("mayo: unknown error code {c}")),\n'
    "    }\n}",
    '        c => VerificationError::generic_err(format!("mayo: unknown error code {c}")),\n'
    "    }\n}\n\n"
    '#[cfg(feature = "cosmwasm_2_3")]\n'
    "fn ml_dsa_error_from_code(code: u32) -> VerificationError {\n"
    "    match code {\n"
    '        2 => VerificationError::generic_err("ml-dsa: invalid input length"),\n'
    '        3 => VerificationError::generic_err("ml-dsa: unknown variant"),\n'
    '        4 => VerificationError::generic_err("ml-dsa: invalid public key"),\n'
    '        5 => VerificationError::generic_err("ml-dsa: signature verification failed"),\n'
    '        c => VerificationError::generic_err(format!("ml-dsa: unknown error code {c}")),\n'
    "    }\n}",
)

# ── 26: packages/std/src/traits.rs — Api trait default method ───────────
MLDSA_TRAIT = '''    /// ML-DSA (FIPS 204) multi-variant post-quantum signature verification.
    ///
    /// `variant` is 44/65/87 for ML-DSA-44/65/87. Added in `cosmwasm_2_3`.
    #[cfg(feature = "cosmwasm_2_3")]
    #[allow(unused_variables)]
    fn ml_dsa_verify(
        &self,
        variant: u32,
        pk: &[u8],
        msg: &[u8],
        sig: &[u8],
    ) -> Result<bool, VerificationError> {
        unimplemented!()
    }
'''
patch(
    "packages/std/src/traits.rs",
    "    fn debug(&self, message: &str);",
    MLDSA_TRAIT + "    fn debug(&self, message: &str);",
)

# ── 27: packages/std/src/testing/mock.rs — software fallback ────────────
MLDSA_MOCK = '''    #[cfg(feature = "cosmwasm_2_3")]
    fn ml_dsa_verify(
        &self,
        variant: u32,
        pk: &[u8],
        msg: &[u8],
        sig: &[u8],
    ) -> Result<bool, VerificationError> {
        cosmwasm_crypto_mldsa::ml_dsa_verify(variant, pk, msg, sig)
            .map_err(|e| VerificationError::generic_err(e.to_string()))
    }
'''
patch(
    "packages/std/src/testing/mock.rs",
    "    fn debug(&self, #[allow(unused)] message: &str) {",
    MLDSA_MOCK + "    fn debug(&self, #[allow(unused)] message: &str) {",
)

print("All ML-DSA wiring edits applied.")
