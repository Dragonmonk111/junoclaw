//! Batch compression — Commonware Bajillion-inspired root-of-roots.
//!
//! Instead of settling each robot decision batch individually on-chain,
//! we aggregate N batches into a single commitment using a Merkle tree
//! of batch hashes. This reduces on-chain settlement cost by ~99% at scale.
//!
//! Pattern from Commonware Bajillion:
//! - Root of roots: one Merkle root commits to N batch roots
//! - BLS signature aggregation: coordination nodes sign the root
//! - Certified commitment: ~100 bytes on-chain for thousands of decisions
//!
//! Our adaptation:
//! - Each batch has a `messages_hash` (already computed by coordination layer)
//! - We build a Merkle tree of all batch hashes in an epoch
//! - The root is settled on-chain via coordination-settler
//! - Individual batches can be challenged via Merkle proof if disputed

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single batch entry in the compression tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchEntry {
    pub batch_height: u64,
    pub messages_hash: String,
    pub verdict: String,
}

/// Compressed epoch — many batches committed as one root.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompressedEpoch {
    /// Epoch number
    pub epoch: u64,
    /// Merkle root of all batch hashes
    pub root: String,
    /// Number of batches compressed
    pub batch_count: u64,
    /// BLS aggregate signature (hex) — from coordination nodes
    pub aggregate_signature: Option<String>,
    /// Signer bitmap
    pub signers: Option<Vec<u8>>,
    /// All batch entries (kept off-chain, only root goes on-chain)
    pub batches: Vec<BatchEntry>,
}

impl CompressedEpoch {
    /// Build a compressed epoch from a list of batch entries.
    ///
    /// Computes the Merkle root of all batch hashes. In production,
    /// coordination nodes would BLS-sign this root.
    pub fn build(epoch: u64, batches: Vec<BatchEntry>) -> Self {
        let root = compute_merkle_root(&batches);
        let batch_count = batches.len() as u64;

        CompressedEpoch {
            epoch,
            root,
            batch_count,
            aggregate_signature: None,
            signers: None,
            batches,
        }
    }

    /// Size of the on-chain commitment (just the root + metadata).
    pub fn on_chain_size_bytes(&self) -> usize {
        // 32 bytes root + 8 bytes epoch + 8 bytes batch_count
        // + ~64 bytes BLS signature + ~32 bytes signer bitmap
        32 + 8 + 8 + 64 + 32
    }

    /// Compression ratio vs settling each batch individually.
    pub fn compression_ratio(&self) -> f64 {
        let individual_size = self.batch_count as f64 * 100.0; // ~100 bytes per batch
        let compressed_size = self.on_chain_size_bytes() as f64;
        if compressed_size > 0.0 {
            individual_size / compressed_size
        } else {
            0.0
        }
    }

    /// Generate a Merkle proof for a specific batch (for challenges).
    pub fn proof_for_batch(&self, batch_height: u64) -> Option<MerkleProof> {
        let index = self.batches.iter().position(|b| b.batch_height == batch_height)?;
        let leaves: Vec<[u8; 32]> = self.batches.iter().map(|b| hash_batch(b)).collect();
        let proof = build_merkle_proof(&leaves, index);
        Some(MerkleProof {
            batch_height,
            index: index as u64,
            leaf_hash: hex::encode(leaves[index]),
            proof: proof.into_iter().map(hex::encode).collect(),
        })
    }
}

/// Merkle proof for challenging a specific batch in a compressed epoch.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub batch_height: u64,
    pub index: u64,
    pub leaf_hash: String,
    pub proof: Vec<String>,
}

impl MerkleProof {
    /// Verify a Merkle proof against a known root.
    pub fn verify(&self, root: &str) -> bool {
        let mut current = match hex::decode(&self.leaf_hash) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            _ => return false,
        };

        let mut idx = self.index;
        for sibling_hex in &self.proof {
            let sibling = match hex::decode(sibling_hex) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    arr
                }
                _ => return false,
            };
            current = if idx % 2 == 0 {
                hash_pair(&current, &sibling)
            } else {
                hash_pair(&sibling, &current)
            };
            idx /= 2;
        }

        hex::encode(current) == root
    }
}

// ──────────────────────────────────────────────
// Merkle tree helpers
// ──────────────────────────────────────────────

fn hash_batch(batch: &BatchEntry) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(batch.batch_height.to_le_bytes());
    hasher.update(batch.messages_hash.as_bytes());
    hasher.update(batch.verdict.as_bytes());
    let result = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&result);
    arr
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(left);
    hasher.update(right);
    let result = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&result);
    arr
}

fn compute_merkle_root(batches: &[BatchEntry]) -> String {
    if batches.is_empty() {
        return hex::encode([0u8; 32]);
    }

    let mut leaves: Vec<[u8; 32]> = batches.iter().map(hash_batch).collect();

    while leaves.len() > 1 {
        let mut next = Vec::new();
        for chunk in leaves.chunks(2) {
            if chunk.len() == 2 {
                next.push(hash_pair(&chunk[0], &chunk[1]));
            } else {
                next.push(chunk[0]);
            }
        }
        leaves = next;
    }

    hex::encode(leaves[0])
}

fn build_merkle_proof(leaves: &[[u8; 32]], index: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    let mut current = leaves.to_vec();
    let mut idx = index;

    while current.len() > 1 {
        let sibling = if idx % 2 == 0 {
            idx + 1
        } else {
            idx - 1
        };

        if sibling < current.len() {
            proof.push(current[sibling]);
        }

        let mut next = Vec::new();
        for chunk in current.chunks(2) {
            if chunk.len() == 2 {
                next.push(hash_pair(&chunk[0], &chunk[1]));
            } else {
                next.push(chunk[0]);
            }
        }
        current = next;
        idx /= 2;
    }

    proof
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression() {
        let batches: Vec<BatchEntry> = (0..100)
            .map(|i| BatchEntry {
                batch_height: i,
                messages_hash: format!("hash_{}", i),
                verdict: "green".to_string(),
            })
            .collect();

        let epoch = CompressedEpoch::build(1, batches);
        assert_eq!(epoch.batch_count, 100);
        assert!(!epoch.root.is_empty());
        assert!(epoch.compression_ratio() > 10.0);
    }

    #[test]
    fn test_merkle_proof() {
        let batches: Vec<BatchEntry> = (0..8)
            .map(|i| BatchEntry {
                batch_height: i,
                messages_hash: format!("hash_{}", i),
                verdict: "green".to_string(),
            })
            .collect();

        let epoch = CompressedEpoch::build(1, batches);
        let proof = epoch.proof_for_batch(3).unwrap();
        assert!(proof.verify(&epoch.root));
    }

    #[test]
    fn test_empty_epoch() {
        let epoch = CompressedEpoch::build(0, Vec::new());
        assert_eq!(epoch.batch_count, 0);
    }
}
