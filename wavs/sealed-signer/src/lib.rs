//! JunoClaw WAVS Sealed Signer
//!
//! M1.5: entropy from `wasi:random/random`, persistence via `wasi:keyvalue`,
//! and the sealing passphrase supplied through `WAVS_ENV_SIGNER_PASSPHRASE`.
//! The raw private key never leaves the component except as an AES-GCM blob.

pub mod crypto;

#[allow(warnings)]
mod bindings;

use k256::ecdsa::SigningKey;
use std::sync::Mutex;

use crate::crypto::*;

/// In-memory cache of the decrypted signing key for the current component
/// lifetime. In a real TEE this key would be unsealed from the enclave
/// storage on each invocation; here it lives only in component RAM.
static SIGNING_KEY: Mutex<Option<(SigningKey, Vec<u8>)>> = Mutex::new(None);

const PASSPHRASE_ENV: &str = "WAVS_ENV_SIGNER_PASSPHRASE";

fn read_passphrase() -> Result<String, String> {
    std::env::var(PASSPHRASE_ENV)
        .map_err(|_| format!("{} env var not set", PASSPHRASE_ENV))
}

fn cached_signing_key(sealed_blob: &[u8], passphrase: &str) -> Result<SigningKey, String> {
    let mut guard = SIGNING_KEY.lock().map_err(|e| e.to_string())?;
    if let Some((key, cached_blob)) = guard.as_ref() {
        if cached_blob == sealed_blob {
            return Ok(key.clone());
        }
    }
    let secret = decrypt_key(sealed_blob, passphrase).map_err(|e| e.to_string())?;
    let key = SigningKey::from_bytes(&secret.into()).map_err(|e| e.to_string())?;
    *guard = Some((key.clone(), sealed_blob.to_vec()));
    Ok(key)
}

struct SealedSignerComponent;

impl bindings::exports::junoclaw::sealed_signer::signer::Guest for SealedSignerComponent {
    fn generate_key() -> Result<bindings::exports::junoclaw::sealed_signer::signer::KeyInfo, String> {
        let passphrase = read_passphrase()?;

        let raw = bindings::wasi::random::random::get_random_bytes(32);
        let mut secret = [0u8; 32];
        if raw.len() != 32 {
            return Err(format!("expected 32 random bytes, got {}", raw.len()));
        }
        secret.copy_from_slice(&raw);

        let signing_key = signing_key_from_secret(&secret).map_err(|e| e.to_string())?;
        let verifying_key = signing_key.verifying_key();
        let address = juno_address_from_pubkey(verifying_key).map_err(|e| e.to_string())?;
        let pubkey_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());

        let sealed_blob = encrypt_key(&secret, &passphrase).map_err(|e| e.to_string())?;

        // Cache the key for the rest of this component lifetime.
        if let Ok(mut guard) = SIGNING_KEY.lock() {
            *guard = Some((signing_key.clone(), sealed_blob.clone()));
        }

        Ok(bindings::exports::junoclaw::sealed_signer::signer::KeyInfo {
            address,
            pubkey: pubkey_hex,
            sealed_blob,
        })
    }

    fn sign(
        message: Vec<u8>,
        sealed_blob: Vec<u8>,
    ) -> Result<bindings::exports::junoclaw::sealed_signer::signer::SignInfo, String> {
        let passphrase = read_passphrase()?;
        let key = cached_signing_key(&sealed_blob, &passphrase)?;
        (|| -> anyhow::Result<_> {
            let sig = sign_message(&key, &message)?;
            let sig_hex = hex::encode(&sig);
            let verifying_key = key.verifying_key();
            let address = juno_address_from_pubkey(verifying_key)?;
            let pubkey_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());
            Ok(bindings::exports::junoclaw::sealed_signer::signer::SignInfo {
                address,
                pubkey: pubkey_hex,
                signature: sig_hex,
            })
        })().map_err(|e| e.to_string())
    }

    fn sign_cosmos_execute_tx(
        sealed_blob: Vec<u8>,
        req: bindings::exports::junoclaw::sealed_signer::signer::CosmosExecuteTxRequest,
    ) -> Result<bindings::exports::junoclaw::sealed_signer::signer::SignedCosmosTx, String> {
        let passphrase = read_passphrase()?;
        let key = cached_signing_key(&sealed_blob, &passphrase)?;
        (|| -> anyhow::Result<_> {
            let tx_req = CosmosExecuteTxRequest {
                sender: &req.sender,
                contract: &req.contract,
                exec_msg_json: &req.exec_msg_json,
                funds_denom: &req.funds_denom,
                funds_amount: req.funds_amount as u128,
                gas_limit: req.gas_limit,
                fee_denom: &req.fee_denom,
                fee_amount: req.fee_amount as u128,
                memo: &req.memo,
                chain_id: &req.chain_id,
                account_number: req.account_number,
                sequence: req.sequence,
            };
            let signed = sign_execute_contract_tx(&key, &tx_req)?;
            let verifying_key = key.verifying_key();
            let address = juno_address_from_pubkey(verifying_key)?;
            let pubkey_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());
            Ok(bindings::exports::junoclaw::sealed_signer::signer::SignedCosmosTx {
                address,
                pubkey: pubkey_hex,
                tx_bytes: signed.tx_bytes,
                sign_doc_sha256_hex: signed.sign_doc_sha256_hex,
            })
        })().map_err(|e| e.to_string())
    }
}

bindings::export!(SealedSignerComponent with_types_in bindings);
