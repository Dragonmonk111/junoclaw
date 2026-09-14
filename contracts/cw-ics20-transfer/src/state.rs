use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};

/// ICS-20 packet data as defined in ICS-20 spec.
/// https://github.com/cosmos/ibc/tree/main/spec/app/ics-020-fungible-token-transfer
#[cw_serde]
pub struct Ics20Packet {
    pub denom: String,
    pub amount: String,
    pub sender: String,
    pub receiver: String,
    pub memo: Option<String>,
}

/// ICS-20 acknowledgement.
/// Success: `{"result":"AQ=="}` (base64 of 0x01)
/// Error: `{"error":"message"}`
#[cw_serde]
pub enum Ics20Ack {
    Result(String),
    Error(String),
}

/// Channel info for an established IBC channel.
#[cw_serde]
pub struct ChannelInfo {
    /// The channel id on this chain (e.g. "channel-0")
    pub channel_id: String,
    /// The counterparty channel id
    pub counterparty_channel_id: String,
    /// The counterparty chain id (connection's client chain)
    pub counterparty_chain_id: String,
    /// The connection id used by this channel
    pub connection_id: String,
    /// The port id on this side
    pub port_id: String,
    /// The port id on the counterparty side
    pub counterparty_port_id: String,
}

/// Denom trace for an IBC-sourced token.
/// Tracks the full path so we can construct the correct IBC denom.
#[cw_serde]
pub struct DenomTrace {
    /// The original denom before any IBC transfers (e.g. "ujclaw")
    pub base_denom: String,
    /// The path of channels this token has traversed, newest first.
    /// E.g. ["transfer/channel-0"] means it came through channel-0.
    pub path: Vec<String>,
}

impl DenomTrace {
    /// Construct the full IBC denom string.
    /// Format: {path}/{base_denom} where path is channel hops joined by "/".
    /// E.g. "transfer/channel-0/ujclaw"
    pub fn ibc_denom(&self) -> String {
        if self.path.is_empty() {
            return self.base_denom.clone();
        }
        let path = self.path.join("/");
        format!("{}/{}", path, self.base_denom)
    }

    /// When receiving a token from a counterparty, prepend the channel hop.
    pub fn receive(&self, channel_id: &str, port_id: &str) -> DenomTrace {
        let mut new_path = vec![format!("{}/{}", port_id, channel_id)];
        new_path.extend(self.path.iter().cloned());
        DenomTrace {
            base_denom: self.base_denom.clone(),
            path: new_path,
        }
    }

    /// When sending back, strip the first hop if it matches.
    pub fn send_back(&self, channel_id: &str, port_id: &str) -> DenomTrace {
        let hop = format!("{}/{}", port_id, channel_id);
        if self.path.first().map(|h| h == &hop).unwrap_or(false) {
            DenomTrace {
                base_denom: self.base_denom.clone(),
                path: self.path[1..].to_vec(),
            }
        } else {
            DenomTrace {
                base_denom: self.base_denom.clone(),
                path: self.path.clone(),
            }
        }
    }
}

/// Escrow balance tracking per channel + denom.
#[cw_serde]
pub struct EscrowBalance {
    pub channel_id: String,
    pub denom: String,
    pub amount: Uint128,
}

/// Transfer config
#[cw_serde]
pub struct TransferConfig {
    /// The IBC port ID this contract binds. Default: "transfer"
    pub port_id: String,
    /// Whether the contract is allowed to send transfers
    pub send_enabled: bool,
    /// Whether the contract is allowed to receive transfers
    pub receive_enabled: bool,
    /// Default timeout in seconds for outgoing packets
    pub default_timeout_seconds: u64,
    /// Admin who can update config
    pub admin: Addr,
    /// Allowed denoms for transfer (empty = allow all)
    pub allowed_denoms: Vec<String>,
}
