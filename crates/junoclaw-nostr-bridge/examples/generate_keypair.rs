//! One-shot Nostr keypair generator.
//!
//! Used to mint identities that need to be stable across restarts:
//! - `RELAY_OWNER_PUBKEY` / `BUZZ_RELAY_PRIVATE_KEY` for a self-hosted Buzz relay
//! - `JUNOCLAW_NOSTR_PRIVKEY` for this bridge's own publishing identity
//! - per-agent identities for agents joining a Buzz relay
//!
//! Prints to stdout only — never writes key material to disk. Pipe the
//! private key straight into a secret manager; do not paste it into a
//! file that might get committed.
//!
//! Usage: cargo run -p junoclaw-nostr-bridge --example generate_keypair

use nostr_sdk::Keys;

fn main() {
    let keys = Keys::generate();
    let pubkey_hex = keys.public_key().to_string();
    let privkey_hex = keys
        .secret_key()
        .expect("Keys::generate() always produces a secret key")
        .to_secret_hex();

    println!("=== New Nostr Keypair ===");
    println!();
    println!("public_key (hex, 64 chars) : {pubkey_hex}");
    println!("private_key (hex, 64 chars): {privkey_hex}");
    println!();
    println!("The public key is safe to publish (e.g. as RELAY_OWNER_PUBKEY,");
    println!("or in a governance record identifying this DAO's relay).");
    println!();
    println!("The private key controls this identity. Store it in a secret");
    println!("manager or DAO-controlled vault — never commit it to git, never");
    println!("paste it into a chat log, never leave it in shell history.");
}
