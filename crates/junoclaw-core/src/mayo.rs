//! Post-quantum MAYO signature integration for JunoClaw.
//!
//! MAYO (Multivariate quAdratic hash-based signature sYstem) is a NIST PQC
//! Round 4 candidate. This module wraps the `sriracha-mayo` crate, providing
//! a clean API for key generation, signing, and verification compatible
//! with JunoClaw's agent identity and attestation flows.
//!
//! # Parameter Sets
//!
//! | Variant | NIST Level | Secret | Public Key | Signature | Use Case |
//! |---------|-----------|--------|------------|-----------|----------|
//! | MAYO-1  | 1         | 24 B   | 1,420 B    | 454 B     | Standard |
//! | MAYO-2  | 1         | 24 B   | 4,912 B    | **186 B** | On-chain |
//! | MAYO-3  | 3         | 32 B   | 2,986 B    | 681 B     | High sec |
//! | MAYO-5  | 5         | 40 B   | 5,554 B    | 964 B     | Max sec  |
//!
//! MAYO-2 is recommended for on-chain use due to its small signature size.

use anyhow::{anyhow, Result};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported MAYO parameter sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MayoVariant {
    /// NIST Level 1, 1420 B pk, 454 B sig
    Mayo1,
    /// NIST Level 1, 4912 B pk, 186 B sig — smallest signatures
    Mayo2,
    /// NIST Level 3, 2986 B pk, 681 B sig
    Mayo3,
    /// NIST Level 5, 5554 B pk, 964 B sig
    Mayo5,
}

impl fmt::Display for MayoVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MayoVariant::Mayo1 => write!(f, "mayo1"),
            MayoVariant::Mayo2 => write!(f, "mayo2"),
            MayoVariant::Mayo3 => write!(f, "mayo3"),
            MayoVariant::Mayo5 => write!(f, "mayo5"),
        }
    }
}

impl std::str::FromStr for MayoVariant {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "mayo1" | "mayo-1" => Ok(MayoVariant::Mayo1),
            "mayo2" | "mayo-2" => Ok(MayoVariant::Mayo2),
            "mayo3" | "mayo-3" => Ok(MayoVariant::Mayo3),
            "mayo5" | "mayo-5" => Ok(MayoVariant::Mayo5),
            _ => Err(anyhow!("unknown MAYO variant: {} (expected mayo1/2/3/5)", s)),
        }
    }
}

/// A MAYO keypair — serializable for agent identity storage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MayoKeypair {
    pub variant: MayoVariant,
    /// Secret seed (compact private key)
    #[serde(with = "hex_serde")]
    pub secret: Vec<u8>,
    /// Public key bytes
    #[serde(with = "hex_serde")]
    pub public: Vec<u8>,
}

/// A MAYO signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MayoSignature {
    pub variant: MayoVariant,
    #[serde(with = "hex_serde")]
    pub bytes: Vec<u8>,
}

/// Generate a new MAYO keypair for the given variant.
pub fn generate_keypair(variant: MayoVariant) -> Result<MayoKeypair> {
    let mut rng = OsRng;
    match variant {
        MayoVariant::Mayo1 => {
            let sk = sriracha_mayo::mayo1::SecretKey::random(&mut rng)?;
            let pk = sk.public_key()?;
            Ok(MayoKeypair {
                variant,
                secret: sk.to_bytes().to_vec(),
                public: pk.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo2 => {
            let sk = sriracha_mayo::mayo2::SecretKey::random(&mut rng)?;
            let pk = sk.public_key()?;
            Ok(MayoKeypair {
                variant,
                secret: sk.to_bytes().to_vec(),
                public: pk.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo3 => {
            let sk = sriracha_mayo::mayo3::SecretKey::random(&mut rng)?;
            let pk = sk.public_key()?;
            Ok(MayoKeypair {
                variant,
                secret: sk.to_bytes().to_vec(),
                public: pk.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo5 => {
            let sk = sriracha_mayo::mayo5::SecretKey::random(&mut rng)?;
            let pk = sk.public_key()?;
            Ok(MayoKeypair {
                variant,
                secret: sk.to_bytes().to_vec(),
                public: pk.to_bytes().to_vec(),
            })
        }
    }
}

/// Sign a message using a MAYO keypair.
/// Uses `namespace || msg` domain separation (Commonware convention).
pub fn sign(keypair: &MayoKeypair, namespace: &[u8], msg: &[u8]) -> Result<MayoSignature> {
    // Domain separation: namespace || 0x00 || msg
    let mut payload = Vec::with_capacity(namespace.len() + 1 + msg.len());
    payload.extend_from_slice(namespace);
    payload.push(0);
    payload.extend_from_slice(msg);

    match keypair.variant {
        MayoVariant::Mayo1 => {
            let sk = sriracha_mayo::mayo1::SecretKey::from_bytes(&keypair.secret)?;
            let sig = sk.sign(&payload)?;
            Ok(MayoSignature {
                variant: keypair.variant,
                bytes: sig.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo2 => {
            let sk = sriracha_mayo::mayo2::SecretKey::from_bytes(&keypair.secret)?;
            let sig = sk.sign(&payload)?;
            Ok(MayoSignature {
                variant: keypair.variant,
                bytes: sig.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo3 => {
            let sk = sriracha_mayo::mayo3::SecretKey::from_bytes(&keypair.secret)?;
            let sig = sk.sign(&payload)?;
            Ok(MayoSignature {
                variant: keypair.variant,
                bytes: sig.to_bytes().to_vec(),
            })
        }
        MayoVariant::Mayo5 => {
            let sk = sriracha_mayo::mayo5::SecretKey::from_bytes(&keypair.secret)?;
            let sig = sk.sign(&payload)?;
            Ok(MayoSignature {
                variant: keypair.variant,
                bytes: sig.to_bytes().to_vec(),
            })
        }
    }
}

/// Verify a MAYO signature.
pub fn verify(
    variant: MayoVariant,
    public_key: &[u8],
    namespace: &[u8],
    msg: &[u8],
    signature: &[u8],
) -> Result<bool> {
    let mut payload = Vec::with_capacity(namespace.len() + 1 + msg.len());
    payload.extend_from_slice(namespace);
    payload.push(0);
    payload.extend_from_slice(msg);

    match variant {
        MayoVariant::Mayo1 => {
            let pk = sriracha_mayo::mayo1::PublicKey::from_bytes(public_key)?;
            let sig = sriracha_mayo::mayo1::Signature::from_bytes(signature)?;
            Ok(pk.verify(&payload, &sig))
        }
        MayoVariant::Mayo2 => {
            let pk = sriracha_mayo::mayo2::PublicKey::from_bytes(public_key)?;
            let sig = sriracha_mayo::mayo2::Signature::from_bytes(signature)?;
            Ok(pk.verify(&payload, &sig))
        }
        MayoVariant::Mayo3 => {
            let pk = sriracha_mayo::mayo3::PublicKey::from_bytes(public_key)?;
            let sig = sriracha_mayo::mayo3::Signature::from_bytes(signature)?;
            Ok(pk.verify(&payload, &sig))
        }
        MayoVariant::Mayo5 => {
            let pk = sriracha_mayo::mayo5::PublicKey::from_bytes(public_key)?;
            let sig = sriracha_mayo::mayo5::Signature::from_bytes(signature)?;
            Ok(pk.verify(&payload, &sig))
        }
    }
}

/// Hex serialization helper module.
mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mayo2_roundtrip() {
        let kp = generate_keypair(MayoVariant::Mayo2).unwrap();
        let namespace = b"junoclaw.test";
        let msg = b"hello, mayo!";

        let sig = sign(&kp, namespace, msg).unwrap();
        assert!(verify(MayoVariant::Mayo2, &kp.public, namespace, msg, &sig.bytes).unwrap());

        // Wrong msg should fail
        assert!(!verify(MayoVariant::Mayo2, &kp.public, namespace, b"wrong", &sig.bytes).unwrap());
    }

    #[test]
    fn test_all_variants_keygen() {
        for variant in [MayoVariant::Mayo1, MayoVariant::Mayo2, MayoVariant::Mayo3, MayoVariant::Mayo5] {
            let kp = generate_keypair(variant).unwrap();
            assert!(!kp.secret.is_empty());
            assert!(!kp.public.is_empty());
        }
    }
}
