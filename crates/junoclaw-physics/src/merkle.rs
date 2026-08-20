//! Merkle tree construction from reflex cycle hashes.
//!
//! Builds a binary Merkle tree using SHA-256 from a list of cycle hashes.
//! Returns the root (for on-chain anchoring) and authentication paths
//! (for individual cycle verification).

use sha2::{Digest, Sha256};

/// Compute the Merkle root from a list of hex-encoded cycle hashes.
///
/// Uses SHA-256 for all layers (matching the bridge and on-chain verifier).
/// If the list is empty, returns SHA-256 of empty bytes.
/// If the list has one element, returns that element.
/// Pads odd levels by duplicating the last node.
pub fn compute_merkle_root(cycle_hashes: &[String]) -> String {
    if cycle_hashes.is_empty() {
        return hex::encode(Sha256::digest(b""));
    }

    if cycle_hashes.len() == 1 {
        return cycle_hashes[0].clone();
    }

    // Parse hex hashes to bytes
    let mut level: Vec<Vec<u8>> = cycle_hashes
        .iter()
        .map(|h| hex::decode(h).unwrap_or_else(|_| vec![0u8; 32]))
        .collect();

    // Build tree
    while level.len() > 1 {
        // Pad odd level
        if level.len() % 2 == 1 {
            level.push(level[level.len() - 1].clone());
        }

        let mut next = Vec::with_capacity(level.len() / 2);
        for i in (0..level.len()).step_by(2) {
            let mut hasher = Sha256::new();
            hasher.update(&level[i]);
            hasher.update(&level[i + 1]);
            next.push(hasher.finalize().to_vec());
        }
        level = next;
    }

    hex::encode(&level[0])
}

/// Compute a Merkle authentication proof for a leaf at the given index.
///
/// Returns a list of sibling hashes from the leaf level up to the root.
/// The verifier can reconstruct the root by hashing the leaf with each
/// sibling in sequence (left or right depending on index parity).
pub fn compute_merkle_proof(cycle_hashes: &[String], leaf_index: usize) -> Vec<String> {
    if cycle_hashes.is_empty() || leaf_index >= cycle_hashes.len() {
        return Vec::new();
    }

    if cycle_hashes.len() == 1 {
        return Vec::new(); // No proof needed for single leaf
    }

    let mut level: Vec<Vec<u8>> = cycle_hashes
        .iter()
        .map(|h| hex::decode(h).unwrap_or_else(|_| vec![0u8; 32]))
        .collect();

    let mut proof = Vec::new();
    let mut idx = leaf_index;

    while level.len() > 1 {
        // Pad odd level
        if level.len() % 2 == 1 {
            level.push(level[level.len() - 1].clone());
        }

        // Get sibling
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        proof.push(hex::encode(&level[sibling_idx]));

        // Hash pairs
        let mut next = Vec::with_capacity(level.len() / 2);
        for i in (0..level.len()).step_by(2) {
            let mut hasher = Sha256::new();
            hasher.update(&level[i]);
            hasher.update(&level[i + 1]);
            next.push(hasher.finalize().to_vec());
        }
        level = next;
        idx /= 2;
    }

    proof
}

/// Verify a Merkle proof against a known root.
///
/// Reconstructs the root by hashing the leaf with each sibling in the proof,
/// alternating left/right based on the index parity at each level.
pub fn verify_merkle_proof(leaf_hash: &str, leaf_index: usize, proof: &[String]) -> String {
    let mut current = hex::decode(leaf_hash).unwrap_or_else(|_| vec![0u8; 32]);
    let mut idx = leaf_index;

    for sibling_hex in proof {
        let sibling = hex::decode(sibling_hex).unwrap_or_else(|_| vec![0u8; 32]);
        let mut hasher = Sha256::new();
        if idx % 2 == 0 {
            hasher.update(&current);
            hasher.update(&sibling);
        } else {
            hasher.update(&sibling);
            hasher.update(&current);
        }
        current = hasher.finalize().to_vec();
        idx /= 2;
    }

    hex::encode(&current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hashes(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| {
                let mut hasher = Sha256::new();
                hasher.update(i.to_le_bytes());
                hex::encode(hasher.finalize())
            })
            .collect()
    }

    #[test]
    fn test_empty_merkle_root() {
        let root = compute_merkle_root(&[]);
        assert_eq!(root.len(), 64);
    }

    #[test]
    fn test_single_leaf_root() {
        let hashes = make_hashes(1);
        let root = compute_merkle_root(&hashes);
        assert_eq!(root, hashes[0]);
    }

    #[test]
    fn test_two_leaf_root() {
        let hashes = make_hashes(2);
        let root = compute_merkle_root(&hashes);

        // Manual computation
        let mut hasher = Sha256::new();
        hasher.update(hex::decode(&hashes[0]).unwrap());
        hasher.update(hex::decode(&hashes[1]).unwrap());
        let expected = hex::encode(hasher.finalize());

        assert_eq!(root, expected);
    }

    #[test]
    fn test_odd_number_leaves() {
        let hashes = make_hashes(3);
        let root = compute_merkle_root(&hashes);

        // Should pad: [h0, h1, h2, h2]
        let h = |a: &[u8], b: &[u8]| {
            let mut hasher = Sha256::new();
            hasher.update(a);
            hasher.update(b);
            hasher.finalize().to_vec()
        };

        let l0 = hex::decode(&hashes[0]).unwrap();
        let l1 = hex::decode(&hashes[1]).unwrap();
        let l2 = hex::decode(&hashes[2]).unwrap();

        let h01 = h(&l0, &l1);
        let h22 = h(&l2, &l2);
        let root_expected = hex::encode(h(&h01, &h22));

        assert_eq!(root, root_expected);
    }

    #[test]
    fn test_proof_verification() {
        let hashes = make_hashes(8);
        let root = compute_merkle_root(&hashes);

        for i in 0..8 {
            let proof = compute_merkle_proof(&hashes, i);
            let computed_root = verify_merkle_proof(&hashes[i], i, &proof);
            assert_eq!(computed_root, root, "proof for leaf {} should verify", i);
        }
    }

    #[test]
    fn test_proof_verification_odd() {
        let hashes = make_hashes(5);
        let root = compute_merkle_root(&hashes);

        for i in 0..5 {
            let proof = compute_merkle_proof(&hashes, i);
            let computed_root = verify_merkle_proof(&hashes[i], i, &proof);
            assert_eq!(computed_root, root, "proof for leaf {} should verify (odd)", i);
        }
    }

    #[test]
    fn test_proof_single_leaf() {
        let hashes = make_hashes(1);
        let proof = compute_merkle_proof(&hashes, 0);
        assert!(proof.is_empty(), "single leaf should have empty proof");
    }

    #[test]
    fn test_large_tree() {
        let hashes = make_hashes(1000);
        let root = compute_merkle_root(&hashes);

        // Verify a few random leaves
        for i in [0, 42, 500, 999] {
            let proof = compute_merkle_proof(&hashes, i);
            let computed = verify_merkle_proof(&hashes[i], i, &proof);
            assert_eq!(computed, root, "proof for leaf {} should verify in large tree", i);
        }
    }

    #[test]
    fn test_deterministic_root() {
        let hashes = make_hashes(10);
        let root1 = compute_merkle_root(&hashes);
        let root2 = compute_merkle_root(&hashes);
        assert_eq!(root1, root2, "root should be deterministic");
    }
}
