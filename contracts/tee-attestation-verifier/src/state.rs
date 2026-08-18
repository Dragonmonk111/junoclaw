use cosmwasm_std::{ensure, Addr};
use cw_storage_plus::{Item, Map};
use serde::{Deserialize, Serialize};

pub const ADMIN: Item<Addr> = Item::new("admin");
pub const TRUSTED_MEASUREMENT: Item<String> = Item::new("trusted_measurement");

/// Last attestation result per robot
pub const ATTESTATIONS: Map<&str, AttestationRecord> = Map::new("attestations");

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AttestationRecord {
    pub verified: bool,
    pub attestation_type: String,
    pub measurement: String,
    pub report_data: String,
    pub verified_at: u64,
}
