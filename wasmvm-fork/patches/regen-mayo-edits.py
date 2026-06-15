#!/usr/bin/env python3
"""Apply all MAYO wiring edits to a cosmwasm checkout that already has the
BN254 patches (00-09) applied. Anchors on the post-BN254 text so the diffs
git produces layer cleanly on top of the BN254 series.

Usage: python3 regen-mayo-edits.py /path/to/cosmwasm
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


# ── 10: packages/vm/Cargo.toml ──────────────────────────────────────────
patch(
    "packages/vm/Cargo.toml",
    'cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254" }\n',
    'cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254" }\n'
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo" }\n',
)

# ── 11a: packages/vm/src/imports.rs — use statement ─────────────────────
patch(
    "packages/vm/src/imports.rs",
    "use rand_core::OsRng;",
    "use cosmwasm_crypto_mayo::{gas::mayo_verify_cost, mayo_verify, MayoError};\n"
    "use rand_core::OsRng;",
)

# ── 11b: imports.rs — status codes + mayo_error_code, before do_abort ───
MAYO_VM = r'''
// ── MAYO host-function status codes ───────────────────────────────────────
//
// Returned in the u32 of `do_mayo_verify`'s `VmResult<u32>`. 0 = valid,
// 1 = invalid, >=2 = error (length mismatch, unknown variant, or internal).
const MAYO_OK: u32 = 0;
const MAYO_INVALID: u32 = 1;
const MAYO_ERR_INVALID_INPUT_LENGTH: u32 = 2;
const MAYO_ERR_UNKNOWN_VARIANT: u32 = 3;
const MAYO_ERR_INVALID_SIGNATURE: u32 = 4;
const MAYO_ERR_INTERNAL: u32 = 5;

#[inline]
fn mayo_error_code(err: &MayoError) -> u32 {
    match err {
        MayoError::InvalidInputLength { .. } => MAYO_ERR_INVALID_INPUT_LENGTH,
        MayoError::UnknownVariant(_) => MAYO_ERR_UNKNOWN_VARIANT,
        MayoError::InvalidSignature => MAYO_ERR_INVALID_SIGNATURE,
        MayoError::InternalError => MAYO_ERR_INTERNAL,
        _ => MAYO_ERR_INTERNAL,
    }
}

/// MAYO multi-variant signature verification (post-quantum precompile).
///
/// `variant` is one of 1 (MAYO-1), 2 (MAYO-2), 3 (MAYO-3), or 5 (MAYO-5).
/// Reads `pk`, `msg`, and `sig` from the three guest-supplied Regions.
/// Returns 0 for a valid signature, 1 for invalid, and non-zero error codes
/// for malformed inputs or internal failures. Gas is charged per variant
/// before any cryptographic work begins.
pub fn do_mayo_verify<A: BackendApi + 'static, S: Storage + 'static, Q: Querier + 'static>(
    mut env: FunctionEnvMut<Environment<A, S, Q>>,
    variant: u32,
    pk_ptr: u32,
    msg_ptr: u32,
    sig_ptr: u32,
) -> VmResult<u32> {
    let (data, mut store) = env.data_and_store_mut();

    // v2.2.2 read_region signature: (data, &mut store, ptr, max_len).
    let pk = read_region(data, &mut store, pk_ptr, 8192)?;
    let msg = read_region(data, &mut store, msg_ptr, 65536)?;
    let sig = read_region(data, &mut store, sig_ptr, 2048)?;

    let gas_info = GasInfo::with_cost(mayo_verify_cost(variant));
    process_gas_info(data, &mut store, gas_info)?;

    match mayo_verify(variant, &pk, &msg, &sig) {
        Ok(true) => Ok(MAYO_OK),
        Ok(false) => Ok(MAYO_INVALID),
        Err(e) => Ok(mayo_error_code(&e)),
    }
}

'''
patch(
    "packages/vm/src/imports.rs",
    "/// Aborts the contract and shows the given error message\n"
    "pub fn do_abort",
    MAYO_VM + "/// Aborts the contract and shows the given error message\n"
    "pub fn do_abort",
)

# ── 12: packages/vm/src/compatibility.rs ────────────────────────────────
patch(
    "packages/vm/src/compatibility.rs",
    '    "env.bn254_pairing_equality",\n    "env.secp256k1_verify",',
    '    "env.bn254_pairing_equality",\n    "env.mayo_verify",\n'
    '    "env.secp256k1_verify",',
)

# ── 13a: packages/vm/src/instance.rs — import list ──────────────────────
patch(
    "packages/vm/src/instance.rs",
    "    do_db_read, do_db_remove, do_db_write, do_debug, do_ed25519_batch_verify, do_ed25519_verify,\n"
    "    do_query_chain, do_secp256k1_recover_pubkey, do_secp256k1_verify,",
    "    do_db_read, do_db_remove, do_db_write, do_debug, do_ed25519_batch_verify, do_ed25519_verify,\n"
    "    do_mayo_verify, do_query_chain, do_secp256k1_recover_pubkey, do_secp256k1_verify,",
)

# ── 13b: packages/vm/src/instance.rs — host-fn registration ─────────────
patch(
    "packages/vm/src/instance.rs",
    '        env_imports.insert(\n'
    '            "bn254_pairing_equality",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_bn254_pairing_equality),\n'
    '        );\n',
    '        env_imports.insert(\n'
    '            "bn254_pairing_equality",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_bn254_pairing_equality),\n'
    '        );\n\n'
    '        // MAYO post-quantum signature verification host function.\n'
    '        // Multi-variant: 1=MAYO-1, 2=MAYO-2, 3=MAYO-3, 5=MAYO-5.\n'
    '        env_imports.insert(\n'
    '            "mayo_verify",\n'
    '            Function::new_typed_with_env(&mut store, &fe, do_mayo_verify),\n'
    '        );\n',
)

# ── 14a: packages/std/Cargo.toml — cosmwasm_2_3 feature ─────────────────
patch(
    "packages/std/Cargo.toml",
    'cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254"]',
    'cosmwasm_2_3 = ["cosmwasm_2_2", "dep:cosmwasm-crypto-bn254", "dep:cosmwasm-crypto-mayo"]',
)

# ── 14b: packages/std/Cargo.toml — optional dep ─────────────────────────
patch(
    "packages/std/Cargo.toml",
    'cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254", optional = true }\n',
    'cosmwasm-crypto-bn254 = { version = "0.1.0", path = "../crypto-bn254", optional = true }\n'
    'cosmwasm-crypto-mayo = { version = "0.1.0", path = "../crypto-mayo", optional = true }\n',
)

# ── 15a: packages/std/src/imports.rs — extern declaration ───────────────
patch(
    "packages/std/src/imports.rs",
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn bn254_pairing_equality(input_ptr: u32) -> u32;\n",
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn bn254_pairing_equality(input_ptr: u32) -> u32;\n\n"
    "    // MAYO post-quantum signature verification host function.\n"
    "    // Multi-variant: 1=MAYO-1, 2=MAYO-2, 3=MAYO-3, 5=MAYO-5.\n"
    "    // Returns 0 = valid, 1 = invalid, >=2 = error.\n"
    "    #[cfg(feature = \"cosmwasm_2_3\")]\n"
    "    fn mayo_verify(variant: u32, pk_ptr: u32, msg_ptr: u32, sig_ptr: u32) -> u32;\n",
)

# ── 15b: packages/std/src/imports.rs — Api method (before secp256k1) ────
MAYO_API = '''    #[cfg(feature = "cosmwasm_2_3")]
    fn mayo_verify(
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
            mayo_verify(
                variant,
                send_pk.as_ptr() as u32,
                send_msg.as_ptr() as u32,
                send_sig.as_ptr() as u32,
            )
        };
        match result {
            0 => Ok(true),
            1 => Ok(false),
            code => Err(mayo_error_from_code(code)),
        }
    }
'''
patch(
    "packages/std/src/imports.rs",
    "    fn secp256k1_verify(\n"
    "        &self,\n"
    "        message_hash: &[u8],",
    MAYO_API + "    fn secp256k1_verify(\n"
    "        &self,\n"
    "        message_hash: &[u8],",
)

# ── 15c: packages/std/src/imports.rs — error mapper (after bn254 one) ───
patch(
    "packages/std/src/imports.rs",
    '        c => VerificationError::generic_err(format!("bn254: unknown error code {c}")),\n'
    "    }\n}",
    '        c => VerificationError::generic_err(format!("bn254: unknown error code {c}")),\n'
    "    }\n}\n\n"
    '#[cfg(feature = "cosmwasm_2_3")]\n'
    "fn mayo_error_from_code(code: u32) -> VerificationError {\n"
    "    match code {\n"
    '        2 => VerificationError::generic_err("mayo: invalid input length"),\n'
    '        3 => VerificationError::generic_err("mayo: unknown variant"),\n'
    '        4 => VerificationError::generic_err("mayo: signature verification failed"),\n'
    '        5 => VerificationError::generic_err("mayo: internal error"),\n'
    '        c => VerificationError::generic_err(format!("mayo: unknown error code {c}")),\n'
    "    }\n}",
)

# ── 16: packages/std/src/traits.rs — Api trait default method ───────────
MAYO_TRAIT = '''    /// MAYO multi-variant post-quantum signature verification.
    ///
    /// `variant` is 1/2/3/5 for MAYO-1/2/3/5. Added in `cosmwasm_2_3`.
    #[cfg(feature = "cosmwasm_2_3")]
    #[allow(unused_variables)]
    fn mayo_verify(
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
    MAYO_TRAIT + "    fn debug(&self, message: &str);",
)

# ── 17: packages/std/src/testing/mock.rs — software fallback ────────────
MAYO_MOCK = '''    #[cfg(feature = "cosmwasm_2_3")]
    fn mayo_verify(
        &self,
        variant: u32,
        pk: &[u8],
        msg: &[u8],
        sig: &[u8],
    ) -> Result<bool, VerificationError> {
        cosmwasm_crypto_mayo::mayo_verify(variant, pk, msg, sig)
            .map_err(|e| VerificationError::generic_err(e.to_string()))
    }
'''
patch(
    "packages/std/src/testing/mock.rs",
    "    fn debug(&self, #[allow(unused)] message: &str) {",
    MAYO_MOCK + "    fn debug(&self, #[allow(unused)] message: &str) {",
)

print("All MAYO wiring edits applied.")
