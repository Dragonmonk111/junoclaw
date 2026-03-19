use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use clap::{Parser, Subcommand};
use rand::RngCore;
use std::fs;
use std::path::PathBuf;
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};

/// bud-seal: Encrypted handover tool for JunoClaw genesis bud onboarding.
///
/// Uses X25519 key exchange + ChaCha20-Poly1305 AEAD.
/// Recipient generates a keypair, shares the public key.
/// Sender seals a file to the recipient's public key.
/// Only the recipient can open it.
#[derive(Parser)]
#[command(name = "bud-seal", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate an X25519 keypair for a bud recipient
    Keygen {
        /// Output directory for key files
        #[arg(short, long, default_value = ".")]
        out: PathBuf,
        /// Name prefix for key files (e.g. "dimi")
        #[arg(short, long)]
        name: String,
    },
    /// Seal (encrypt) a file to a recipient's public key
    Seal {
        /// Path to recipient's public key file (.pub)
        #[arg(short, long)]
        to: PathBuf,
        /// Path to plaintext file to encrypt
        #[arg(short, long)]
        file: PathBuf,
        /// Output path for sealed file (default: <file>.sealed)
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Open (decrypt) a sealed file with your private key
    Open {
        /// Path to your private key file (.key)
        #[arg(short, long)]
        key: PathBuf,
        /// Path to sealed file
        #[arg(short, long)]
        file: PathBuf,
        /// Output path for decrypted file (default: stdout)
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Keygen { out, name } => cmd_keygen(&out, &name),
        Commands::Seal { to, file, out } => cmd_seal(&to, &file, out.as_deref()),
        Commands::Open { key, file, out } => cmd_open(&key, &file, out.as_deref()),
    }
}

/// Generate X25519 keypair. Private key is a 32-byte static secret.
/// Public key is the corresponding X25519 point.
fn cmd_keygen(out_dir: &PathBuf, name: &str) {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);

    let priv_b64 = B64.encode(secret.to_bytes());
    let pub_b64 = B64.encode(public.as_bytes());

    let priv_path = out_dir.join(format!("{}.key", name));
    let pub_path = out_dir.join(format!("{}.pub", name));

    fs::write(&priv_path, &priv_b64).expect("Failed to write private key");
    fs::write(&pub_path, &pub_b64).expect("Failed to write public key");

    eprintln!("Keypair generated for '{}'", name);
    eprintln!("  Private key: {} (KEEP SECRET)", priv_path.display());
    eprintln!("  Public key:  {} (share with sender)", pub_path.display());
    eprintln!();
    eprintln!("Public key (base64): {}", pub_b64);
}

/// Seal a file using ephemeral X25519 + ChaCha20-Poly1305.
///
/// Format of .sealed file:
///   Line 1: "BUD-SEAL-V1"
///   Line 2: ephemeral public key (base64, 32 bytes)
///   Line 3: nonce (base64, 12 bytes)
///   Line 4: ciphertext (base64)
///
/// The shared secret is derived from:
///   ephemeral_secret * recipient_public_key
/// which only the recipient can also compute as:
///   recipient_secret * ephemeral_public_key
fn cmd_seal(recipient_pub_path: &PathBuf, plaintext_path: &PathBuf, out: Option<&std::path::Path>) {
    let pub_b64 = fs::read_to_string(recipient_pub_path).expect("Cannot read recipient public key");
    let pub_bytes: [u8; 32] = B64
        .decode(pub_b64.trim())
        .expect("Invalid base64 in public key")
        .try_into()
        .expect("Public key must be 32 bytes");
    let recipient_pub = PublicKey::from(pub_bytes);

    let plaintext = fs::read(plaintext_path).expect("Cannot read plaintext file");

    // Ephemeral keypair — used once, never stored
    let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
    let ephemeral_public = PublicKey::from(&ephemeral_secret);

    // X25519 shared secret
    let shared = ephemeral_secret.diffie_hellman(&recipient_pub);

    // Derive symmetric key from shared secret (raw 32 bytes = ChaCha20 key)
    let cipher = ChaCha20Poly1305::new_from_slice(shared.as_bytes())
        .expect("Shared secret must be 32 bytes");

    // Random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_ref())
        .expect("Encryption failed");

    // Write sealed file
    let sealed = format!(
        "BUD-SEAL-V1\n{}\n{}\n{}",
        B64.encode(ephemeral_public.as_bytes()),
        B64.encode(nonce_bytes),
        B64.encode(&ciphertext),
    );

    let out_path = match out {
        Some(p) => p.to_path_buf(),
        None => {
            let mut p = plaintext_path.clone();
            let ext = format!(
                "{}.sealed",
                p.extension().unwrap_or_default().to_str().unwrap_or("")
            );
            p.set_extension(ext);
            p
        }
    };

    fs::write(&out_path, sealed).expect("Failed to write sealed file");
    eprintln!("Sealed: {} -> {}", plaintext_path.display(), out_path.display());
    eprintln!("Only the holder of the matching private key can open this.");
}

/// Open a sealed file using the recipient's private key.
fn cmd_open(
    priv_key_path: &PathBuf,
    sealed_path: &PathBuf,
    out: Option<&std::path::Path>,
) {
    let priv_b64 = fs::read_to_string(priv_key_path).expect("Cannot read private key");
    let priv_bytes: [u8; 32] = B64
        .decode(priv_b64.trim())
        .expect("Invalid base64 in private key")
        .try_into()
        .expect("Private key must be 32 bytes");
    let secret = StaticSecret::from(priv_bytes);

    let sealed = fs::read_to_string(sealed_path).expect("Cannot read sealed file");
    let lines: Vec<&str> = sealed.lines().collect();

    if lines.len() < 4 || lines[0] != "BUD-SEAL-V1" {
        eprintln!("Error: not a valid BUD-SEAL-V1 file");
        std::process::exit(1);
    }

    let ephemeral_pub_bytes: [u8; 32] = B64
        .decode(lines[1])
        .expect("Invalid ephemeral public key")
        .try_into()
        .expect("Ephemeral public key must be 32 bytes");
    let ephemeral_pub = PublicKey::from(ephemeral_pub_bytes);

    let nonce_bytes: [u8; 12] = B64
        .decode(lines[2])
        .expect("Invalid nonce")
        .try_into()
        .expect("Nonce must be 12 bytes");
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = B64.decode(lines[3]).expect("Invalid ciphertext");

    // Reconstruct shared secret: recipient_secret * ephemeral_public
    let shared = secret.diffie_hellman(&ephemeral_pub);

    let cipher = ChaCha20Poly1305::new_from_slice(shared.as_bytes())
        .expect("Shared secret must be 32 bytes");

    let plaintext = cipher
        .decrypt(&nonce, ciphertext.as_ref())
        .expect("Decryption failed — wrong key or tampered file");

    match out {
        Some(p) => {
            fs::write(p, &plaintext).expect("Failed to write output");
            eprintln!("Decrypted: {} -> {}", sealed_path.display(), p.display());
        }
        None => {
            print!("{}", String::from_utf8_lossy(&plaintext));
        }
    }
}
