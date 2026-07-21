//! Prints a deterministic Cosmos SIGN_MODE_DIRECT SignDoc (and the resulting
//! signature) as JSON, for cross-checking against an independent
//! implementation (see `scripts/crosscheck-signdoc.js`).
//!
//! This does NOT touch wasi bindings — it links only `crypto.rs`, so it runs
//! as a normal native binary: `cargo run --example print_signdoc_fixture`.

use junoclaw_sealed_signer::crypto::{
    juno_address_from_pubkey, secret_from_seed, sign_execute_contract_tx, signing_key_from_secret,
    CosmosExecuteTxRequest,
};

fn main() {
    let seed = [9u8; 32];
    let secret = secret_from_seed(&seed);
    let signing_key = signing_key_from_secret(&secret).expect("signing key");
    let address = juno_address_from_pubkey(signing_key.verifying_key()).expect("address");

    // Fixed test vector — must match scripts/crosscheck-signdoc.js exactly.
    let req = CosmosExecuteTxRequest {
        sender: &address,
        contract: &address,
        exec_msg_json: r#"{"post":{"text":"hello moultbook"}}"#,
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

    let signed = sign_execute_contract_tx(&signing_key, &req).expect("sign tx");

    let verifying_key = signing_key.verifying_key();
    let pubkey_compressed_hex = hex::encode(verifying_key.to_encoded_point(true).as_bytes());

    println!(
        "{}",
        serde_json::json!({
            "address": address,
            "pubkey_compressed_hex": pubkey_compressed_hex,
            "tx_bytes_hex": hex::encode(&signed.tx_bytes),
            "sign_doc_sha256_hex": signed.sign_doc_sha256_hex,
        })
    );
}
