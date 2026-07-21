//! Pure Rust crypto primitives for the JunoClaw sealed signer.
//!
//! This module has no WASI/component dependencies so it can be unit-tested on
//! the host and reused by the WAVS component wrapper.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use k256::ecdsa::{signature::Signer, Signature, SigningKey, VerifyingKey};
use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};

pub const PBKDF2_ROUNDS: u32 = 100_000;
pub const SALT_LEN: usize = 32;
pub const NONCE_LEN: usize = 12;
pub const TAG_LEN: usize = 16;

/// Derive a 32-byte secret from a deterministic seed.
/// Kept for host unit tests only; the component uses `wasi:random/random`
/// at runtime.
#[cfg(any(test, not(target_arch = "wasm32")))]
pub fn secret_from_seed(seed: &[u8]) -> [u8; 32] {
    Sha256::digest(seed).into()
}

pub fn juno_address_from_pubkey(pubkey: &VerifyingKey) -> anyhow::Result<String> {
    let compressed = pubkey.to_encoded_point(true).to_bytes();
    let sha = Sha256::digest(&compressed);
    let mut ripemd = ripemd::Ripemd160::new();
    ripemd.update(sha);
    let addr_bytes = ripemd.finalize();
    let hrp = bech32::Hrp::parse("juno")?;
    Ok(bech32::encode::<bech32::Bech32>(hrp, &addr_bytes)?)
}

pub fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, PBKDF2_ROUNDS, &mut key);
    key
}

pub fn derive_salt_and_nonce(secret: &[u8; 32], passphrase: &str) -> ([u8; SALT_LEN], [u8; NONCE_LEN]) {
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

pub fn encrypt_key(secret: &[u8; 32], passphrase: &str) -> anyhow::Result<Vec<u8>> {
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

pub fn decrypt_key(sealed_blob: &[u8], passphrase: &str) -> anyhow::Result<[u8; 32]> {
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

pub fn signing_key_from_secret(secret: &[u8; 32]) -> anyhow::Result<SigningKey> {
    Ok(SigningKey::from_bytes(&(*secret).into())?)
}

pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> anyhow::Result<Vec<u8>> {
    let signature: Signature = signing_key.sign(message);
    let signature = signature.normalize_s().unwrap_or(signature);
    Ok(signature.to_bytes().to_vec())
}

/// Structured request to sign a single `cosmwasm.wasm.v1.MsgExecuteContract`
/// as a Cosmos SDK `SIGN_MODE_DIRECT` transaction. Deliberately scoped to
/// this one message type: it's the only message JunoClaw's agent tooling
/// (Moultbook posting) actually needs to broadcast today.
///
/// The enclave builds the canonical `TxBody`/`AuthInfo`/`SignDoc` bytes
/// itself from these structured fields — it never trusts caller-supplied
/// raw protobuf bytes as the thing being signed, so a malicious caller
/// cannot get the enclave to sign something other than the described
/// contract call.
pub struct CosmosExecuteTxRequest<'a> {
    pub sender: &'a str,
    pub contract: &'a str,
    pub exec_msg_json: &'a str,
    pub funds_denom: &'a str,
    pub funds_amount: u128,
    pub gas_limit: u64,
    pub fee_denom: &'a str,
    pub fee_amount: u128,
    pub memo: &'a str,
    pub chain_id: &'a str,
    pub account_number: u64,
    pub sequence: u64,
}

pub struct SignedCosmosTx {
    /// Protobuf-encoded `TxRaw`, ready to broadcast as-is.
    pub tx_bytes: Vec<u8>,
    /// SHA-256 hex of the exact `SignDoc` bytes that were signed, for
    /// out-of-band auditing (does not require decoding the tx to verify
    /// which bytes were actually signed).
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
        vec![Coin::new(req.funds_amount, req.funds_denom)
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
    let body = Body::new(vec![exec_msg_any], req.memo, 0u32);

    let fee_coin = Coin::new(req.fee_amount, req.fee_denom)
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

    Ok(SignedCosmosTx {
        tx_bytes,
        sign_doc_sha256_hex,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::ecdsa::signature::Verifier;

    #[test]
    fn known_seed_produces_expected_address() {
        // Deterministic seed.
        let seed = [42u8; 32];
        let secret = secret_from_seed(&seed);
        let signing_key = signing_key_from_secret(&secret).unwrap();
        let address = juno_address_from_pubkey(signing_key.verifying_key()).unwrap();
        assert!(address.starts_with("juno1"));
        assert_eq!(address.len(), 43);
    }

    #[test]
    fn encryption_roundtrip() {
        let seed = [1u8; 32];
        let secret = secret_from_seed(&seed);
        let passphrase = "test-passphrase";
        let sealed = encrypt_key(&secret, passphrase).unwrap();
        let decrypted = decrypt_key(&sealed, passphrase).unwrap();
        assert_eq!(secret.to_vec(), decrypted.to_vec());
    }

    #[test]
    fn wrong_passphrase_fails_decryption() {
        let seed = [2u8; 32];
        let secret = secret_from_seed(&seed);
        let sealed = encrypt_key(&secret, "right").unwrap();
        assert!(decrypt_key(&sealed, "wrong").is_err());
    }

    #[test]
    fn sign_and_verify_cosmos_execute_tx() {
        use cosmrs::tx::Raw;

        let seed = [9u8; 32];
        let secret = secret_from_seed(&seed);
        let signing_key = signing_key_from_secret(&secret).unwrap();

        let req = CosmosExecuteTxRequest {
            sender: "juno1sender0000000000000000000000000000000",
            contract: "juno1contract00000000000000000000000000000",
            exec_msg_json: r#"{"post":{"text":"hello"}}"#,
            funds_denom: "ujuno",
            funds_amount: 0,
            gas_limit: 200_000,
            fee_denom: "ujuno",
            fee_amount: 5000,
            memo: "AKB export from junoclaw",
            chain_id: "uni-7",
            account_number: 42,
            sequence: 7,
        };

        // Bech32 addresses above are not valid (wrong checksum/length), so
        // this is exercised via the address-validity error path elsewhere;
        // use a real-looking address derived from a signing key here instead.
        let sender_address = juno_address_from_pubkey(signing_key.verifying_key()).unwrap();
        let req = CosmosExecuteTxRequest {
            sender: &sender_address,
            contract: &sender_address,
            ..req
        };

        let signed = sign_execute_contract_tx(&signing_key, &req).unwrap();
        assert!(!signed.tx_bytes.is_empty());
        assert_eq!(signed.sign_doc_sha256_hex.len(), 64);

        // Decode the signed TxRaw back and verify the signature against the
        // reconstructed SignDoc bytes, matching what a chain full node does.
        let raw = Raw::from_bytes(&signed.tx_bytes).unwrap();
        let proto: cosmrs::proto::cosmos::tx::v1beta1::TxRaw = raw.into();
        assert_eq!(proto.signatures.len(), 1);

        let sign_doc = cosmrs::proto::cosmos::tx::v1beta1::SignDoc {
            body_bytes: proto.body_bytes.clone(),
            auth_info_bytes: proto.auth_info_bytes.clone(),
            chain_id: req.chain_id.to_string(),
            account_number: req.account_number,
        };
        use cosmrs::proto::traits::MessageExt;
        let sign_doc_bytes = sign_doc.to_bytes().unwrap();
        let recomputed_hash = hex::encode(Sha256::digest(&sign_doc_bytes));
        assert_eq!(recomputed_hash, signed.sign_doc_sha256_hex);

        let signature = Signature::try_from(proto.signatures[0].as_slice()).unwrap();
        assert!(signing_key
            .verifying_key()
            .verify(&sign_doc_bytes, &signature)
            .is_ok());

        // Decode the TxBody and check the wire-level type_url and embedded
        // JSON exactly match what a Cosmos full node expects for
        // MsgExecuteContract, not just that signing round-trips locally.
        use cosmrs::proto::cosmwasm::wasm::v1::MsgExecuteContract as ProtoMsgExecuteContract;
        use prost::Message;
        let tx_body = cosmrs::proto::cosmos::tx::v1beta1::TxBody::decode(proto.body_bytes.as_slice())
            .unwrap();
        assert_eq!(tx_body.memo, req.memo);
        assert_eq!(tx_body.messages.len(), 1);
        assert_eq!(tx_body.messages[0].type_url, "/cosmwasm.wasm.v1.MsgExecuteContract");
        let decoded_exec_msg =
            ProtoMsgExecuteContract::decode(tx_body.messages[0].value.as_slice()).unwrap();
        assert_eq!(decoded_exec_msg.sender, sender_address);
        assert_eq!(decoded_exec_msg.contract, sender_address);
        assert_eq!(decoded_exec_msg.msg, req.exec_msg_json.as_bytes());
    }

    #[test]
    fn sign_and_verify_message() {
        let seed = [7u8; 32];
        let secret = secret_from_seed(&seed);
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
