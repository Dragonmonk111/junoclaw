use sha2::{Sha256, Digest};
use crate::entry::MemoryEntry;

/// Append-only Merkle tree of memory entries.
///
/// Each leaf is the SHA-256 hash of a `MemoryEntry`. Internal nodes
/// are SHA-256 of concatenated children. The root is committed on-chain,
/// making the entire memory history tamper-evident.
///
/// The tree uses a simple binary layout: leaves are padded to the next
/// power of 2, and internal nodes are computed bottom-up.
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// All leaf hashes (one per entry)
    leaves: Vec<[u8; 32]>,
    /// All node hashes (level by level, bottom to top)
    /// nodes[0] = leaf level, nodes[1] = first internal level, etc.
    nodes: Vec<Vec<[u8; 32]>>,
    /// Current root hash
    root: [u8; 32],
}

impl MerkleTree {
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            nodes: Vec::new(),
            root: [0u8; 32],
        }
    }

    /// Append a memory entry and recompute the root.
    pub fn append(&mut self, entry: &MemoryEntry) -> [u8; 32] {
        let leaf = entry.hash();
        self.leaves.push(leaf);
        self.recompute();
        self.root
    }

    /// Get the current root hash.
    pub fn root(&self) -> [u8; 32] {
        self.root
    }

    /// Get a leaf hash by index.
    pub fn leaf(&self, index: usize) -> Option<[u8; 32]> {
        self.leaves.get(index).copied()
    }

    /// Get the number of leaves.
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Check if the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// Generate a Merkle inclusion proof for the leaf at `index`.
    /// Returns the sibling hashes from leaf to root.
    pub fn proof(&self, index: usize) -> Vec<[u8; 32]> {
        if index >= self.leaves.len() {
            return Vec::new();
        }

        let mut proof = Vec::new();
        let mut idx = index;

        for level in 0..self.nodes.len().saturating_sub(1) {
            let level_nodes = &self.nodes[level];
            let sibling = if idx % 2 == 0 { idx + 1 } else { idx - 1 };

            if sibling < level_nodes.len() {
                proof.push(level_nodes[sibling]);
            } else {
                // Pad with zero hash if sibling doesn't exist
                proof.push([0u8; 32]);
            }
            idx /= 2;
        }

        proof
    }

    /// Recompute all node levels and root from leaves.
    fn recompute(&mut self) {
        if self.leaves.is_empty() {
            self.root = [0u8; 32];
            return;
        }

        self.nodes.clear();
        self.nodes.push(self.leaves.clone());

        let mut current = self.leaves.clone();

        while current.len() > 1 {
            let mut next = Vec::new();
            for i in (0..current.len()).step_by(2) {
                let left = current[i];
                let right = if i + 1 < current.len() {
                    current[i + 1]
                } else {
                    [0u8; 32]
                };
                next.push(hash_pair(&left, &right));
            }
            self.nodes.push(next.clone());
            current = next;
        }

        self.root = current[0];
    }
}

/// Hash two child nodes together.
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
    fn test_empty_tree() {
        let tree = MerkleTree::new();
        assert!(tree.is_empty());
        assert_eq!(tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_single_entry() {
        let mut tree = MerkleTree::new();
        let entry = MemoryEntry::new("robot-1", "standing", "walk", "moved");
        let root = tree.append(&entry);
        assert_eq!(tree.len(), 1);
        assert_eq!(root, entry.hash());
    }

    #[test]
    fn test_multiple_entries() {
        let mut tree = MerkleTree::new();
        for i in 0..4 {
            let entry = MemoryEntry::new("robot-1", "standing", "walk", "moved");
            tree.append(&entry);
        }
        assert_eq!(tree.len(), 4);
        // Root should be non-zero
        assert_ne!(tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_proof_verification() {
        let mut tree = MerkleTree::new();
        let entries: Vec<MemoryEntry> = (0..4)
            .map(|i| MemoryEntry::new("robot-1", "standing", "walk", "moved"))
            .collect();
        for e in &entries {
            tree.append(e);
        }

        let proof = tree.proof(2);
        assert!(!proof.is_empty());

        // Verify proof manually
        let leaf = entries[2].hash();
        let computed_root = verify_proof(leaf, &proof, 2);
        assert_eq!(computed_root, tree.root());
    }

    fn verify_proof(leaf: [u8; 32], proof: &[[u8; 32]], index: usize) -> [u8; 32] {
        let mut hash = leaf;
        let mut idx = index;
        for sibling in proof {
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
