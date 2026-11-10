//! JSONL spool + in-memory index.
//!
//! Every state change is appended to `attest-spool.jsonl` (+fsync) before
//! the in-memory index is updated; on boot the file is replayed. No
//! external DB needed for v1 — the file is the source of truth and can be
//! rotated/archived by ops.

use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::Context;
use parking_lot::RwLock;

use crate::types::{AttStatus, Attestation, BatchInfo, SpoolEntry};

#[derive(Default)]
pub struct UsageCounter {
    pub submitted: u64,
    pub anchored: u64,
    pub failed: u64,
}

pub struct Store {
    spool_path: PathBuf,
    spool: parking_lot::Mutex<File>,
    inner: RwLock<Inner>,
    next_batch: AtomicU64,
}

#[derive(Default)]
struct Inner {
    atts: HashMap<String, Attestation>,
    /// (customer, idempotency_key) -> attestation_id
    idem: HashMap<(String, String), String>,
    /// usage per customer
    usage: HashMap<String, UsageCounter>,
}

impl Store {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let mut inner = Inner::default();
        let mut next_batch: u64 = 1;

        if path.exists() {
            let f = File::open(path).context("open spool")?;
            for (lineno, line) in BufReader::new(f).lines().enumerate() {
                let line = line.context("read spool line")?;
                if line.trim().is_empty() {
                    continue;
                }
                let entry: SpoolEntry = serde_json::from_str(&line).with_context(|| {
                    format!("spool line {}: bad json", lineno + 1)
                })?;
                Self::replay(&mut inner, entry, &mut next_batch);
            }
        }

        let spool = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .context("open spool append")?;

        Ok(Self {
            spool_path: path.to_path_buf(),
            spool: parking_lot::Mutex::new(spool),
            inner: RwLock::new(inner),
            next_batch: AtomicU64::new(next_batch),
        })
    }

    fn replay(inner: &mut Inner, entry: SpoolEntry, next_batch: &mut u64) {
        match entry {
            SpoolEntry::Submit { att } => {
                inner
                    .idem
                    .insert((att.customer.clone(), att.idempotency_key.clone()),
                            att.attestation_id.clone());
                let c = inner.usage.entry(att.customer.clone()).or_default();
                c.submitted += 1;
                inner.atts.insert(att.attestation_id.clone(), att);
            }
            SpoolEntry::Batch {
                batch_height,
                robot_id,
                merkle_root,
                anchor_tx,
                anchored_at_height,
                items,
            } => {
                *next_batch = (*next_batch).max(batch_height + 1);
                for (id, leaf_index) in items {
                    let customer = match inner.atts.get_mut(&id) {
                        Some(att) => {
                            // proofs are recomputed lazily at receipt time
                            att.status = AttStatus::Anchored;
                            att.batch = Some(BatchInfo {
                                robot_id: robot_id.clone(),
                                batch_height,
                                merkle_root: merkle_root.clone(),
                                leaf_index,
                                proof: vec![],
                                anchor_tx: anchor_tx.clone(),
                                anchored_at_height,
                            });
                            att.customer.clone()
                        }
                        None => continue,
                    };
                    inner.usage.entry(customer).or_default().anchored += 1;
                }
            }
            SpoolEntry::BatchFailed { items, .. } => {
                for id in items {
                    let customer = match inner.atts.get_mut(&id) {
                        Some(att) => {
                            att.status = AttStatus::AnchorFailed;
                            att.customer.clone()
                        }
                        None => continue,
                    };
                    inner.usage.entry(customer).or_default().failed += 1;
                }
            }
            SpoolEntry::Purge { attestation_id } => {
                if let Some(att) = inner.atts.get_mut(&attestation_id) {
                    att.retained_payload = None;
                }
            }
        }
    }

    fn append(&self, entry: &SpoolEntry) -> anyhow::Result<()> {
        let mut f = self.spool.lock();
        let mut line = serde_json::to_vec(entry)?;
        line.push(b'\n');
        f.write_all(&line)?;
        f.sync_data()?;
        Ok(())
    }

    /// Idempotency check: returns (existing_id, stored_body_hash).
    pub fn idem_lookup(&self, customer: &str, key: &str) -> Option<(String, String)> {
        let inner = self.inner.read();
        inner
            .idem
            .get(&(customer.to_string(), key.to_string()))
            .and_then(|id| inner.atts.get(id))
            .map(|a| (a.attestation_id.clone(), a.body_hash.clone()))
    }

    pub fn submit(&self, att: Attestation) -> anyhow::Result<()> {
        self.append(&SpoolEntry::Submit { att: att.clone() })?;
        let mut inner = self.inner.write();
        inner.idem.insert(
            (att.customer.clone(), att.idempotency_key.clone()),
            att.attestation_id.clone(),
        );
        inner
            .usage
            .entry(att.customer.clone())
            .or_default()
            .submitted += 1;
        inner.atts.insert(att.attestation_id.clone(), att);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Attestation> {
        self.inner.read().atts.get(id).cloned()
    }

    /// Drain up to `max` queued attestations and mark them anchoring.
    /// Returns the records (with their leaf hashes decoded by caller).
    pub fn drain_queued(&self, max: usize) -> Vec<Attestation> {
        let mut inner = self.inner.write();
        let mut out = Vec::new();
        for att in inner.atts.values_mut() {
            if att.status == AttStatus::Queued && out.len() < max {
                att.status = AttStatus::Anchoring;
                out.push(att.clone());
            }
        }
        out
    }

    /// Mark an anchored batch: writes the batch entry, updates statuses.
    /// `proofs` keyed by attestation_id.
    pub fn commit_batch(
        &self,
        batch_height: u64,
        robot_id: &str,
        merkle_root: &str,
        anchor_tx: &str,
        anchored_at_height: u64,
        items: &[(String, u32, Vec<String>)], // (id, leaf_index, proof hex)
    ) -> anyhow::Result<()> {
        let entry = SpoolEntry::Batch {
            batch_height,
            robot_id: robot_id.to_string(),
            merkle_root: merkle_root.to_string(),
            anchor_tx: anchor_tx.to_string(),
            anchored_at_height,
            items: items
                .iter()
                .map(|(id, idx, _)| (id.clone(), *idx))
                .collect(),
        };
        self.append(&entry)?;
        let mut inner = self.inner.write();
        for (id, idx, proof) in items {
            let customer = match inner.atts.get_mut(id) {
                Some(att) => {
                    att.status = AttStatus::Anchored;
                    att.batch = Some(BatchInfo {
                        robot_id: robot_id.to_string(),
                        batch_height,
                        merkle_root: merkle_root.to_string(),
                        leaf_index: *idx,
                        proof: proof.clone(),
                        anchor_tx: anchor_tx.to_string(),
                        anchored_at_height,
                    });
                    att.customer.clone()
                }
                None => continue,
            };
            inner.usage.entry(customer).or_default().anchored += 1;
        }
        Ok(())
    }

    /// Park a batch's items as failed (anchor tx gave up after retries).
    pub fn fail_batch(
        &self,
        batch_height: u64,
        items: &[String],
        error: &str,
    ) -> anyhow::Result<()> {
        self.append(&SpoolEntry::BatchFailed {
            batch_height,
            items: items.to_vec(),
            error: error.to_string(),
        })?;
        let mut inner = self.inner.write();
        for id in items {
            let customer = match inner.atts.get_mut(id) {
                Some(att) => {
                    att.status = AttStatus::AnchorFailed;
                    att.customer.clone()
                }
                None => continue,
            };
            inner.usage.entry(customer).or_default().failed += 1;
        }
        Ok(())
    }

    /// Return Anchoring items back to Queued (e.g. retry after restart).
    pub fn requeue_anchoring(&self) -> usize {
        let mut inner = self.inner.write();
        let mut n = 0;
        for att in inner.atts.values_mut() {
            if att.status == AttStatus::Anchoring {
                att.status = AttStatus::Queued;
                n += 1;
            }
        }
        n
    }

    pub fn usage(&self, customer: &str) -> (UsageCounter, u64) {
        let inner = self.inner.read();
        let c = inner.usage.get(customer);
        let queued = inner
            .atts
            .values()
            .filter(|a| a.customer == customer && a.status == AttStatus::Queued)
            .count() as u64;
        (
            UsageCounter {
                submitted: c.map(|x| x.submitted).unwrap_or(0),
                anchored: c.map(|x| x.anchored).unwrap_or(0),
                failed: c.map(|x| x.failed).unwrap_or(0),
            },
            queued,
        )
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let inner = self.inner.read();
        let q = inner
            .atts
            .values()
            .filter(|a| a.status == AttStatus::Queued)
            .count();
        let anch = inner
            .atts
            .values()
            .filter(|a| a.status == AttStatus::Anchoring)
            .count();
        (inner.atts.len(), q, anch)
    }

    pub fn next_batch_height(&self) -> u64 {
        self.next_batch.fetch_add(1, Ordering::SeqCst)
    }

    pub fn purge_payload(&self, id: &str) -> anyhow::Result<bool> {
        self.append(&SpoolEntry::Purge {
            attestation_id: id.to_string(),
        })?;
        let mut inner = self.inner.write();
        Ok(match inner.atts.get_mut(id) {
            Some(a) => {
                a.retained_payload = None;
                true
            }
            None => false,
        })
    }

    pub fn spool_path(&self) -> &Path {
        &self.spool_path
    }
}
