use std::collections::HashMap;
use parking_lot::RwLock;
use crate::entry::MemoryEntry;
use crate::merkle::MerkleTree;
use crate::proof::MerkleProof;

/// In-memory index for O(log n) lookup of robot memory entries.
///
/// The index maintains:
/// - A Merkle tree of all entries (for on-chain root commitment)
/// - A hash map from entry ID → entry (for O(1) lookup)
/// - A hash map from robot_id → list of entry IDs (for per-robot queries)
/// - A hash map from state description → list of entry IDs (for similarity search)
///
/// The Merkle root is committed to the blockchain after each batch,
/// making the memory tamper-evident and shareable across robot owners.
pub struct MemoryIndex {
    /// All entries in insertion order
    entries: RwLock<Vec<MemoryEntry>>,
    /// Merkle tree of all entry hashes
    tree: RwLock<MerkleTree>,
    /// Entry ID → index in entries vector
    by_id: RwLock<HashMap<String, usize>>,
    /// Robot ID → list of entry indices
    by_robot: RwLock<HashMap<String, Vec<usize>>>,
    /// State description → list of entry indices (for "similar state" queries)
    by_state: RwLock<HashMap<String, Vec<usize>>>,
}

impl MemoryIndex {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(Vec::new()),
            tree: RwLock::new(MerkleTree::new()),
            by_id: RwLock::new(HashMap::new()),
            by_robot: RwLock::new(HashMap::new()),
            by_state: RwLock::new(HashMap::new()),
        }
    }

    /// Insert a new memory entry. Returns the Merkle proof for the entry.
    pub fn insert(&self, entry: MemoryEntry) -> MerkleProof {
        let mut entries = self.entries.write();
        let mut tree = self.tree.write();
        let mut by_id = self.by_id.write();
        let mut by_robot = self.by_robot.write();
        let mut by_state = self.by_state.write();

        let index = entries.len();
        let robot_id = entry.robot_id.clone();
        let state_desc = entry.state.description.clone();
        let entry_id = entry.id.clone();

        tree.append(&entry);
        entries.push(entry);

        by_id.insert(entry_id, index);
        by_robot.entry(robot_id).or_default().push(index);
        by_state.entry(state_desc).or_default().push(index);

        MerkleProof::from_tree(&tree, index).unwrap_or(MerkleProof {
            index,
            siblings: Vec::new(),
            root: tree.root(),
            entry_hash: [0u8; 32],
        })
    }

    /// Get the current Merkle root (for on-chain commitment).
    pub fn root(&self) -> [u8; 32] {
        self.tree.read().root()
    }

    /// Get the total number of entries.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Check if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.read().is_empty()
    }

    /// Look up an entry by ID. O(1).
    pub fn get(&self, id: &str) -> Option<MemoryEntry> {
        let by_id = self.by_id.read();
        let entries = self.entries.read();
        by_id.get(id).map(|&i| entries[i].clone())
    }

    /// Get all entries for a specific robot. O(1) lookup + O(k) clone.
    pub fn get_by_robot(&self, robot_id: &str) -> Vec<MemoryEntry> {
        let by_robot = self.by_robot.read();
        let entries = self.entries.read();
        by_robot
            .get(robot_id)
            .map(|indices| indices.iter().map(|&i| entries[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Find all entries with a similar state description. O(1) lookup + O(k) clone.
    /// This is the "has any robot ever been in a state like this?" query.
    pub fn find_similar(&self, state_desc: &str) -> Vec<MemoryEntry> {
        let by_state = self.by_state.read();
        let entries = self.entries.read();
        by_state
            .get(state_desc)
            .map(|indices| indices.iter().map(|&i| entries[i].clone()).collect())
            .unwrap_or_default()
    }

    /// Generate a Merkle proof for the entry at `index`.
    pub fn proof(&self, index: usize) -> Option<MerkleProof> {
        let tree = self.tree.read();
        MerkleProof::from_tree(&tree, index)
    }

    /// Get the last N entries (most recent memory).
    pub fn recent(&self, n: usize) -> Vec<MemoryEntry> {
        let entries = self.entries.read();
        let start = entries.len().saturating_sub(n);
        entries[start..].to_vec()
    }

    /// Get all entries for a robot, filtered by state description.
    pub fn query_robot_state(&self, robot_id: &str, state_desc: &str) -> Vec<MemoryEntry> {
        self.get_by_robot(robot_id)
            .into_iter()
            .filter(|e| e.state.description == state_desc)
            .collect()
    }
}

impl Default for MemoryIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_lookup() {
        let index = MemoryIndex::new();
        let entry = MemoryEntry::new("robot-1", "standing", "walk", "moved 0.3m");
        let entry_id = entry.id.clone();
        let proof = index.insert(entry);

        assert!(proof.verify().valid);
        assert_eq!(index.len(), 1);
        assert!(index.get(&entry_id).is_some());
    }

    #[test]
    fn test_robot_query() {
        let index = MemoryIndex::new();

        for _ in 0..3 {
            index.insert(MemoryEntry::new("robot-1", "standing", "walk", "moved"));
        }
        for _ in 0..2 {
            index.insert(MemoryEntry::new("robot-2", "standing", "walk", "moved"));
        }

        assert_eq!(index.get_by_robot("robot-1").len(), 3);
        assert_eq!(index.get_by_robot("robot-2").len(), 2);
        assert_eq!(index.get_by_robot("robot-3").len(), 0);
    }

    #[test]
    fn test_similar_state_query() {
        let index = MemoryIndex::new();

        index.insert(MemoryEntry::new("robot-1", "standing", "walk", "moved"));
        index.insert(MemoryEntry::new("robot-2", "fallen", "recover", "stood up"));
        index.insert(MemoryEntry::new("robot-3", "standing", "turn", "turned 90°"));

        let similar = index.find_similar("standing");
        assert_eq!(similar.len(), 2);
        assert_eq!(similar[0].robot_id, "robot-1");
        assert_eq!(similar[1].robot_id, "robot-3");
    }

    #[test]
    fn test_root_changes_on_insert() {
        let index = MemoryIndex::new();
        let root0 = index.root();

        index.insert(MemoryEntry::new("robot-1", "standing", "walk", "moved"));
        let root1 = index.root();

        assert_ne!(root0, root1);

        index.insert(MemoryEntry::new("robot-1", "walking", "halt", "stopped"));
        let root2 = index.root();

        assert_ne!(root1, root2);
    }

    #[test]
    fn test_recent() {
        let index = MemoryIndex::new();
        for i in 0..10 {
            index.insert(MemoryEntry::new("robot-1", "standing", "walk", "moved"));
        }
        let recent = index.recent(3);
        assert_eq!(recent.len(), 3);
    }
}
