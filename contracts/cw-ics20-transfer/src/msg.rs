use cosmwasm_schema::cw_serde;
use cosmwasm_std::Addr;

#[cw_serde]
pub struct InstantiateMsg {
    /// Admin who can update config
    pub admin: String,
    /// IBC port ID to bind. Default: "transfer"
    pub port_id: Option<String>,
    /// Default timeout in seconds for outgoing packets
    pub default_timeout_seconds: Option<u64>,
    /// Allowed denoms for transfer (empty = allow all)
    pub allowed_denoms: Option<Vec<String>>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Transfer tokens to a counterparty chain via IBC.
    /// The sender must attach the coins as funds.
    Transfer {
        /// The channel id on this chain to send through
        channel_id: String,
        /// The recipient address on the counterparty chain
        receiver: String,
        /// Optional timeout in seconds from now (0 = use default)
        timeout_seconds: Option<u64>,
        /// Optional memo (ICS-20 memo field)
        memo: Option<String>,
    },

    /// Update transfer config (admin only)
    UpdateConfig {
        send_enabled: Option<bool>,
        receive_enabled: Option<bool>,
        default_timeout_seconds: Option<u64>,
        allowed_denoms: Option<Vec<String>>,
    },

    /// Transfer admin
    TransferAdmin { new_admin: String },
}

#[cw_serde]
pub enum QueryMsg {
    /// Get the transfer config
    Config {},
    /// Get channel info by channel id
    Channel { channel_id: String },
    /// List all channels
    Channels {},
    /// Get escrow balance for a channel + denom
    Escrow { channel_id: String, denom: String },
    /// Get denom trace for an IBC denom
    DenomTrace { ibc_denom: String },
    /// Get the IBC port ID this contract is bound to
    Port {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub port_id: String,
    pub send_enabled: bool,
    pub receive_enabled: bool,
    pub default_timeout_seconds: u64,
    pub admin: Addr,
    pub allowed_denoms: Vec<String>,
}

#[cw_serde]
pub struct ChannelResponse {
    pub channel_id: String,
    pub counterparty_channel_id: String,
    pub counterparty_chain_id: String,
    pub connection_id: String,
    pub port_id: String,
    pub counterparty_port_id: String,
}

#[cw_serde]
pub struct ChannelsResponse {
    pub channels: Vec<ChannelResponse>,
}

#[cw_serde]
pub struct EscrowResponse {
    pub channel_id: String,
    pub denom: String,
    pub amount: cosmwasm_std::Uint128,
}

#[cw_serde]
pub struct DenomTraceResponse {
    pub base_denom: String,
    pub path: Vec<String>,
}

#[cw_serde]
pub struct PortResponse {
    pub port_id: String,
}
