use sha2::{Sha256, Digest};
use crate::entry::MemoryEntry;
use crate::merkle::MerkleTree;

/// Merkle inclusion proof for a single memory entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MerkleProof {
    /// Index of the leaf in the tree
    pub index: usize,
    /// Sibling hashes from leaf to root
    pub siblings: Vec<[u8; 32]>,
    /// The root hash at the time the proof was generated
    pub root: [u8; 32],
    /// The entry being proven
    pub entry_hash: [u8; 32],
}

/// Result of verifying a Merkle proof.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProofVerification {
    pub valid: bool,
    pub root: [u8; 32],
    pub computed_root: [u8; 32],
}

impl MerkleProof {
    /// Create a proof from a Merkle tree for the entry at `index`.
    pub fn from_tree(tree: &MerkleTree, index: usize) -> Option<Self> {
        if index >= tree.len() {
            return None;
        }
        let siblings = tree.proof(index);
        let entry_hash = tree.leaf(index)?;
        Some(Self {
            index,
            siblings,
            root: tree.root(),
            entry_hash,
        })
    }

    /// Verify this proof against a known root.
    pub fn verify(&self) -> ProofVerification {
        let computed = self.compute_root();
        ProofVerification {
            valid: computed == self.root,
            root: self.root,
            computed_root: computed,
        }
    }

    /// Compute the root from the proof data.
    fn compute_root(&self) -> [u8; 32] {
        let mut hash = self.entry_hash;
        let mut idx = self.index;

        for sibling in &self.siblings {
            if idx % 2 == 0 {
                hash = hash_pair(&hash, sibling);
            } else {
                hash = hash_pair(sibling, &hash);
            }
            idx /= 2;
        }

        hash
    }
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_roundtrip() {
        let mut tree = MerkleTree::new();
        let entries: Vec<MemoryEntry> = (0..8)
            .map(|_| MemoryEntry::new("robot-1", "standing", "walk", "moved"))
            .collect();
        for e in &entries {
            tree.append(e);
        }

        for i in 0..8 {
            let proof = MerkleProof::from_tree(&tree, i).unwrap();
            let verification = proof.verify();
            assert!(verification.valid, "Proof for index {} failed", i);
        }
    }

    #[test]
    fn test_tampered_proof_fails() {
        let mut tree = MerkleTree::new();
        let entries: Vec<MemoryEntry> = (0..4)
            .map(|_| MemoryEntry::new("robot-1", "standing", "walk", "moved"))
            .collect();
        for e in &entries {
            tree.append(e);
        }

        let mut proof = MerkleProof::from_tree(&tree, 1).unwrap();
        // Tamper with the entry hash
        proof.entry_hash[0] ^= 0xFF;
        let verification = proof.verify();
        assert!(!verification.valid, "Tampered proof should fail");
    }
}
