//! Sealed signer support co-located inside the verifier workflow.
//!
//! M1.5 uses on-enclave `wasi:random/random`, persists the sealed key with
//! `wasi:keyvalue/store`, and reads the passphrase from `WAVS_ENV_SIGNER_PASSPHRASE`.

use std::sync::Mutex;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};

use crate::bindings::wasi::keyvalue::store::open;
use crate::bindings::wasi::random::random::get_random_bytes;

pub const PBKDF2_ROUNDS: u32 = 100_000;
pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

const PASSPHRASE_ENV: &str = "WAVS_ENV_SIGNER_PASSPHRASE";
const SEALED_KEY_BUCKET: &str = "sealed-signer";
const SEALED_KEY_KV_KEY: &str = "sealed-key";

/// In-memory cache of the decrypted signing key + sealed blob for the current
/// component lifetime.
static SIGNING_KEY: Mutex<Option<(SigningKey, Vec<u8>)>> = Mutex::new(None);

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KeyInfo {
    pub address: String,
    pub pubkey: String,
    pub sealed_blob: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SignInfo {
    pub address: String,
    pub pubkey: String,
    pub signature: String,
}

fn read_passphrase() -> anyhow::Result<String> {
    std::env::var(PASSPHRASE_ENV)
        .map_err(|_| anyhow::anyhow!("{} env var not set", PASSPHRASE_ENV))
}

fn juno_address_from_pubkey(pubkey: &VerifyingKey) -> anyhow::Result<String> {
    let compressed = pubkey.to_encoded_point(true).to_bytes();
    let sha = Sha256::digest(&compressed);
    let mut ripemd = ripemd::Ripemd160::new();
    ripemd.update(sha);
    let addr_bytes = ripemd.finalize();
    let hrp = bech32::Hrp::parse("juno")?;
    Ok(bech32::encode::<bech32::Bech32>(hrp, &addr_bytes)?)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

fn derive_salt_and_nonce(secret: &[u8; 32], passphrase: &str) -> ([u8; SALT_LEN], [u8; NONCE_LEN]) {
    let mut hasher = Sha256::new();
    hasher.update(secret);
    hasher.update(passphrase.as_bytes());
    let salt: [u8; SALT_LEN] = hasher.finalize().into();

    let mut hasher = Sha256::new();
    hasher.update(salt);
    hasher.update(b"nonce");
    let nonce_bytes: [u8; NONCE_LEN] = hasher.finalize()[..NONCE_LEN]
        .try_into()
        .expect("nonce length");

    (salt, nonce_bytes)
}

fn encrypt_key(secret: &[u8; 32], passphrase: &str) -> anyhow::Result<Vec<u8>> {
    let (salt, nonce_bytes) = derive_salt_and_nonce(secret, passphrase);

    let key = derive_key(passphrase, &salt);
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher.encrypt(nonce, secret.as_slice())?;

    let mut sealed = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&salt);
    sealed.extend_from_slice(&nonce_bytes);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

fn decrypt_key(sealed_blob: &[u8], passphrase: &str) -> anyhow::Result<[u8; 32]> {
    if sealed_blob.len() < SALT_LEN + NONCE_LEN + TAG_LEN {
        anyhow::bail!("sealed blob too short");
    }
    let salt = &sealed_blob[..SALT_LEN];
    let nonce_bytes = &sealed_blob[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &sealed_blob[SALT_LEN + NONCE_LEN..];

    let key = derive_key(passphrase, salt);
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext)?;

    if plaintext.len() != 32 {
        anyhow::bail!("decrypted key is not 32 bytes");
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&plaintext);
    Ok(secret)
}

fn signing_key_from_secret(secret: &[u8; 32]) -> anyhow::Result<SigningKey> {
    Ok(SigningKey::from_bytes(&(*secret).into())?)
}

fn sign_message(signing_key: &SigningKey, message: &[u8]) -> anyhow::Result<Vec<u8>> {
    let signature: Signature = signing_key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    Ok(signature.to_bytes().to_vec())
}

fn kv_load() -> anyhow::Result<Option<Vec<u8>>> {
    let bucket = open(SEALED_KEY_BUCKET)?;
    Ok(bucket.get(SEALED_KEY_KV_KEY)?)
}

fn kv_save(value: &[u8]) -> anyhow::Result<()> {
    let bucket = open(SEALED_KEY_BUCKET)?;
    Ok(bucket.set(SEALED_KEY_KV_KEY, value)?)
}

fn cached_signing_key(sealed_blob: &[u8], passphrase: &str) -> anyhow::Result<SigningKey> {
    let mut guard = SIGNING_KEY.lock().map_err(|e| anyhow::anyhow!("{}", e))?;
    if let Some((key, cached_blob)) = guard.as_ref() {
        if cached_blob == sealed_blob {
            return Ok(key.clone());
        }
    }
    let secret = decrypt_key(sealed_blob, passphrase)?;
    let key = signing_key_from_secret(&secret)?;
    *guard = Some((key.clone(), sealed_blob.to_vec()));
    Ok(key)
}

/// Generate a fresh signing key from on-enclave randomness, seal it, persist it
/// in the WAVS keyvalue store, and return the public key material.
pub fn generate_key() -> anyhow::Result<KeyInfo> {
    let passphrase = read_passphrase()?;

    let raw = get_random_bytes(32);
    if raw.len() != 32 {
        anyhow::bail!("expected 32 random bytes, got {}", raw.len());
    }
    let mut secret = [0u8; 32];
    secret.copy_from_slice(&raw);

    let signing_key = signing_key_from_secret(&secret)?;
    let verifying_key = signing_key.verifying_key();
    let address = juno_address_from_pubkey(verifying_key)?;
    let pubkey_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());

    let sealed_blob = encrypt_key(&secret, &passphrase)?;
    kv_save(&sealed_blob)?;

    if let Ok(mut guard) = SIGNING_KEY.lock() {
        *guard = Some((signing_key.clone(), sealed_blob.clone()));
    }

    Ok(KeyInfo {
        address,
        pubkey: pubkey_hex,
        sealed_blob,
    })
}

/// Load the persisted sealed key (or use the in-memory cache), sign `message`,
/// and return the signature plus public key material.
///
/// If no sealed key exists yet, this generates a fresh one from on-enclave
/// randomness and persists it before signing.
pub fn sign(message: &[u8]) -> anyhow::Result<SignInfo> {
    let passphrase = read_passphrase()?;

    let sealed_blob = match kv_load()? {
        Some(blob) => blob,
        None => {
            let key_info = generate_key()?;
            key_info.sealed_blob
        }
    };

    let key = cached_signing_key(&sealed_blob, &passphrase)?;
    let sig = sign_message(&key, message)?;
    let sig_hex = hex::encode(&sig);
    let verifying_key = key.verifying_key();
    let address = juno_address_from_pubkey(verifying_key)?;
    let pubkey_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());

    Ok(SignInfo {
        address,
        pubkey: pubkey_hex,
        signature: sig_hex,
    })
}

// ──────────────────────────────────────────────
// Cosmos SDK SIGN_MODE_DIRECT signing
// ──────────────────────────────────────────────

/// Structured request to sign a single `cosmwasm.wasm.v1.MsgExecuteContract`
/// as a Cosmos SDK `SIGN_MODE_DIRECT` transaction. The enclave builds the
/// canonical `TxBody`/`AuthInfo`/`SignDoc` bytes itself from these fields — it
/// never trusts caller-supplied raw protobuf bytes, so a malicious caller
/// cannot get the enclave to sign anything other than the described contract
/// call.
#[derive(Debug, Clone)]
pub struct CosmosExecuteTxRequest {
    pub sender: String,
    pub contract: String,
    pub exec_msg_json: String,
    pub funds_denom: String,
    pub funds_amount: u128,
    pub gas_limit: u64,
    pub fee_denom: String,
    pub fee_amount: u128,
    pub memo: String,
    pub chain_id: String,
    pub account_number: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone)]
pub struct SignedCosmosTx {
    /// Juno address derived from the signing key that produced the signature.
    pub address: String,
    /// Protobuf-encoded `TxRaw`, ready to broadcast as-is.
    pub tx_bytes: Vec<u8>,
    /// SHA-256 hex of the exact `SignDoc` bytes that were signed, for
    /// out-of-band auditing.
    pub sign_doc_sha256_hex: String,
}

pub fn sign_execute_contract_tx(
    signing_key: &SigningKey,
    req: &CosmosExecuteTxRequest,
) -> anyhow::Result<SignedCosmosTx> {
    use cosmrs::cosmwasm::MsgExecuteContract;
    use cosmrs::crypto::PublicKey;
    use cosmrs::proto::cosmos::tx::v1beta1::TxRaw;
    use cosmrs::tx::{Body, Fee, Msg, Raw, SignDoc, SignerInfo};
    use cosmrs::{AccountId, Coin};

    let sender: AccountId = req
        .sender
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid sender address: {e}"))?;
    let contract: AccountId = req
        .contract
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid contract address: {e}"))?;

    let funds = if req.funds_amount == 0 {
        vec![]
    } else {
        vec![Coin::new(req.funds_amount, &req.funds_denom)
            .map_err(|e| anyhow::anyhow!("invalid funds coin: {e}"))?]
    };

    let exec_msg = MsgExecuteContract {
        sender,
        contract,
        msg: req.exec_msg_json.as_bytes().to_vec(),
        funds,
    };

    let exec_msg_any = exec_msg
        .to_any()
        .map_err(|e| anyhow::anyhow!("failed to encode MsgExecuteContract: {e}"))?;
    let body = Body::new(vec![exec_msg_any], req.memo.clone(), 0u32);

    let fee_coin = Coin::new(req.fee_amount, &req.fee_denom)
        .map_err(|e| anyhow::anyhow!("invalid fee coin: {e}"))?;
    let fee = Fee::from_amount_and_gas(fee_coin, req.gas_limit);

    let public_key = PublicKey::from(signing_key.verifying_key());
    let signer_info = SignerInfo::single_direct(Some(public_key), req.sequence);
    let auth_info = signer_info.auth_info(fee);

    let chain_id: tendermint::chain::Id = req
        .chain_id
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid chain id: {e}"))?;

    let sign_doc = SignDoc::new(&body, &auth_info, &chain_id, req.account_number)
        .map_err(|e| anyhow::anyhow!("failed to build sign doc: {e}"))?;

    let body_bytes = sign_doc.body_bytes.clone();
    let auth_info_bytes = sign_doc.auth_info_bytes.clone();
    let sign_doc_bytes = sign_doc
        .into_bytes()
        .map_err(|e| anyhow::anyhow!("failed to encode sign doc: {e}"))?;

    let sign_doc_sha256_hex = hex::encode(Sha256::digest(&sign_doc_bytes));
    let signature = sign_message(signing_key, &sign_doc_bytes)?;

    let raw: Raw = TxRaw {
        body_bytes,
        auth_info_bytes,
        signatures: vec![signature],
    }
    .into();

    let tx_bytes = raw
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("failed to encode signed tx: {e}"))?;

    let address = juno_address_from_pubkey(signing_key.verifying_key())?;

    Ok(SignedCosmosTx {
        address,
        tx_bytes,
        sign_doc_sha256_hex,
    })
}

/// Load the persisted sealed signer key (or generate one on first use) and
/// sign a `MsgExecuteContract` as a Cosmos SDK `TxRaw`.
pub fn sign_cosmos_execute_tx(req: CosmosExecuteTxRequest) -> anyhow::Result<SignedCosmosTx> {
    let passphrase = read_passphrase()?;
    let sealed_blob = match kv_load()? {
        Some(blob) => blob,
        None => generate_key()?.sealed_blob,
    };
    let key = cached_signing_key(&sealed_blob, &passphrase)?;
    sign_execute_contract_tx(&key, &req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::signature::Verifier;

    #[test]
    fn encryption_roundtrip() {
        let secret = [1u8; 32];
        let sealed = encrypt_key(&secret, "test-passphrase").unwrap();
        let decrypted = decrypt_key(&sealed, "test-passphrase").unwrap();
        assert_eq!(secret.to_vec(), decrypted.to_vec());
    }

    #[test]
    fn wrong_passphrase_fails() {
        let secret = [2u8; 32];
        let sealed = encrypt_key(&secret, "right").unwrap();
        assert!(decrypt_key(&sealed, "wrong").is_err());
    }

    #[test]
    fn sign_and_verify() {
        let secret = [7u8; 32];
        let signing_key = signing_key_from_secret(&secret).unwrap();
        let message = b"hello moultbook";
        let sig = sign_message(&signing_key, message).unwrap();
        let verifying_key = signing_key.verifying_key();
        let address = juno_address_from_pubkey(verifying_key).unwrap();
        assert!(address.starts_with("juno1"));
        let signature = Signature::try_from(sig.as_slice()).unwrap();
        assert!(verifying_key.verify(message, &signature).is_ok());
    }
}
