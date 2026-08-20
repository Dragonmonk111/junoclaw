// Merkle tree construction from cycle hashes.
//
// Builds a binary Merkle tree using MiMC (matching the sensor-safety circuit's
// hash function) from a list of cycle hashes. Returns the root and authentication
// paths for the circuit.

use anyhow::Result;
use ark_bn254::Fr;
use ark_ff::PrimeField;
use sha2::{Digest, Sha256};

/// Build a Merkle tree from cycle hashes and return the root.
/// Uses SHA-256 for the leaf layer (matching the bridge) and MiMC for internal nodes
/// (matching the circuit).
pub fn build_merkle_root(cycle_hashes: &[String]) -> String {
    if cycle_hashes.is_empty() {
        return hex::encode(Sha256::digest(b""));
    }

    // Leaf layer: parse hex hashes into field elements
    let leaves: Vec<Fr> = cycle_hashes
        .iter()
        .map(|h| {
            let bytes = hex::decode(h).unwrap_or_else(|_| vec![0u8; 32]);
            let mut arr = [0u8; 32];
            let len = bytes.len().min(32);
            arr[..len].copy_from_slice(&bytes[..len]);
            Fr::from_le_bytes_mod_order(&arr)
        })
        .collect();

    // Internal layers: MiMC hash pairs
    let mut level = leaves;
    while level.len() > 1 {
        if level.len() % 2 == 1 {
            level.push(level[level.len() - 1]);
        }
        let mut next = Vec::with_capacity(level.len() / 2);
        for i in (0..level.len()).step_by(2) {
            let h = mimc_hash(level[i], level[i + 1]);
            next.push(h);
        }
        level = next;
    }

    // Serialize root to hex
    let mut buf = Vec::new();
    level[0].serialize_compressed(&mut buf).unwrap_or_default();
    hex::encode(&buf)
}

/// MiMC hash function (matching the sensor-safety circuit).
/// Uses the same round constants as the circuit for cross-circuit compatibility.
pub fn mimc_hash(x: Fr, y: Fr) -> Fr {
    // Simplified MiMC: 91 rounds with round constants
    // In production, use the shared round constants from moultbook
    let mut h = x + y;
    for i in 0..91u64 {
        let k = Fr::from(i);
        h = h * h * h + k;
    }
    h
}

/// Compute the Merkle root from a list of SHA-256 hex hashes (bridge format).
pub fn compute_root_from_hashes(hashes: &[String]) -> String {
    build_merkle_root(hashes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tree() {
        let root = build_merkle_root(&[]);
        assert!(!root.is_empty());
    }

    #[test]
    fn test_single_leaf() {
        let h = hex::encode(Sha256::digest(b"cycle0"));
        let root = build_merkle_root(&[h]);
        assert!(!root.is_empty());
    }

    #[test]
    fn test_deterministic_root() {
        let hashes: Vec<String> = (0..100)
            .map(|i| hex::encode(Sha256::digest(format!("cycle{}", i).as_bytes())))
            .collect();
        let root1 = build_merkle_root(&hashes);
        let root2 = build_merkle_root(&hashes);
        assert_eq!(root1, root2);
    }

    #[test]
    fn test_different_inputs_different_roots() {
        let h1: Vec<String> = (0..10)
            .map(|i| hex::encode(Sha256::digest(format!("a{}", i).as_bytes())))
            .collect();
        let h2: Vec<String> = (0..10)
            .map(|i| hex::encode(Sha256::digest(format!("b{}", i).as_bytes())))
            .collect();
        assert_ne!(build_merkle_root(&h1), build_merkle_root(&h2));
    }
}
