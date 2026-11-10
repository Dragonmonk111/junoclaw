//! Merkle tree + proofs matching `contracts/merkle-verifier` exactly.
//!
//! Convention (from `verify_merkle_proof` in the contract):
//! - leaves are caller-supplied 32-byte hashes (no extra leaf hashing)
//! - internal node = sha256(left ‖ right) — NOT sorted pairs
//! - proof element order is determined by the leaf index bit at each
//!   level: bit 0 → current is left, bit 1 → current is right
//! - odd node at a level is duplicated (standard RFC-6962-style carry)
//! - all hashes exchanged as lowercase hex

use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];

fn pair(a: &Hash, b: &Hash) -> Hash {
    let mut h = Sha256::new();
    h.update(a);
    h.update(b);
    h.finalize().into()
}

/// Build the tree; returns (root, levels) where levels[0] = leaves.
fn build(leaves: &[Hash]) -> (Hash, Vec<Vec<Hash>>) {
    assert!(!leaves.is_empty());
    let mut levels: Vec<Vec<Hash>> = vec![leaves.to_vec()];
    let mut cur = leaves.to_vec();
    while cur.len() > 1 {
        let mut next = Vec::with_capacity(cur.len().div_ceil(2));
        for chunk in cur.chunks(2) {
            let (l, r) = if chunk.len() == 2 {
                (chunk[0], chunk[1])
            } else {
                (chunk[0], chunk[0])
            };
            next.push(pair(&l, &r));
        }
        levels.push(next.clone());
        cur = next;
    }
    (cur[0], levels)
}

/// Inclusion proof for `leaves[idx]`: sibling hashes from leaf level up.
pub fn proof(leaves: &[Hash], idx: usize) -> Vec<Hash> {
    assert!(idx < leaves.len());
    let (_, levels) = build(leaves);
    let mut i = idx;
    let mut out = Vec::new();
    for level in &levels[..levels.len() - 1] {
        let sib = if i % 2 == 0 { i + 1 } else { i - 1 };
        let sib_hash = if sib < level.len() { level[sib] } else { level[i] };
        out.push(sib_hash);
        i /= 2;
    }
    out
}

/// Root of a leaf set — what gets anchored on-chain (hex).
pub fn root_hex(leaves: &[Hash]) -> String {
    hex::encode(build(leaves).0)
}

/// Recompute a root from a proof — mirrors the contract's verify loop.
/// Used in tests + available to callers validating receipts client-side.
pub fn compute_root_hex(leaf: &Hash, leaf_index: u32, proof: &[Hash]) -> String {
    let mut cur = *leaf;
    for (level, sib) in proof.iter().enumerate() {
        cur = if (leaf_index >> level) & 1 == 0 {
            pair(&cur, sib)
        } else {
            pair(sib, &cur)
        };
    }
    hex::encode(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(b: u8) -> Hash {
        let mut h = Sha256::new();
        h.update([b]);
        h.finalize().into()
    }

    #[test]
    fn proof_verifies_against_contract_algorithm() {
        for n in 1..=9usize {
            let leaves: Vec<Hash> = (0..n as u8).map(leaf).collect();
            let root = root_hex(&leaves);
            for (i, l) in leaves.iter().enumerate() {
                let p = proof(&leaves, i);
                assert_eq!(
                    compute_root_hex(l, i as u32, &p),
                    root,
                    "n={n} idx={i}"
                );
            }
        }
    }

    #[test]
    fn known_root_two_leaves() {
        // sanity: root of 2 leaves = sha256(l0‖l1)
        let leaves = vec![leaf(0), leaf(1)];
        let expected = pair(&leaves[0], &leaves[1]);
        assert_eq!(root_hex(&leaves), hex::encode(expected));
    }
}
