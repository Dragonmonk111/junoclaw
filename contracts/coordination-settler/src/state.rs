use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

use crate::msg::InstantiateMsg;

/// Contract configuration — set at instantiation, updatable by admin.
#[cosmwasm_schema::cw_serde]
pub struct Config {
    pub admin: Addr,
    pub threshold: u32,
    pub latest_height: Option<u64>,
}

/// A settled batch — stored on-chain as final settlement evidence.
#[cosmwasm_schema::cw_serde]
pub struct SettledBatch {
    pub commonware_height: u64,
    pub messages_hash: [u8; 32],
    pub certificate: Vec<u8>,
    pub timestamp: u64,
    pub submitter: Addr,
}

/// Storage keys
pub const CONFIG: Item<Config> = Item::new("config");

/// Validator public keys (BLS12-381 compressed, 48 bytes each)
pub const VALIDATORS: Item<Vec<Vec<u8>>> = Item::new("validators");

/// Settled batches indexed by Commonware height
pub const BATCHES: Map<u64, SettledBatch> = Map::new("batches");

/// Authorized relayers
pub const RELAYERS: Map<&Addr, bool> = Map::new("relayers");

impl From<InstantiateMsg> for Config {
    fn from(msg: InstantiateMsg) -> Self {
        Self {
            admin: Addr::unchecked(&msg.admin),
            threshold: msg.threshold,
            latest_height: None,
        }
    }
}
