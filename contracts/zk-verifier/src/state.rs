use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;
use cw_storage_plus::Item;

#[cw_serde]
pub struct Config {
    pub admin: Addr,
}

#[cw_serde]
pub struct LastVerification {
    pub verified: bool,
    pub block_height: u64,
}

pub const CONFIG: Item<Config> = Item::new("config");

/// Raw bytes of the arkworks VerifyingKey<Bn254> (CanonicalSerialize format).
/// Stored as opaque bytes to avoid serde issues with arkworks types.
pub const VK_BYTES: Item<Vec<u8>> = Item::new("vk_bytes");

pub const LAST_VERIFICATION: Item<LastVerification> = Item::new("last_verification");
