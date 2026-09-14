use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("Unauthorized")]
    Unauthorized {},

    #[error("IBC transfer is disabled")]
    TransferDisabled {},

    #[error("IBC receive is disabled")]
    ReceiveDisabled {},

    #[error("Denom '{denom}' is not allowed for transfer")]
    DenomNotAllowed { denom: String },

    #[error("No funds attached to transfer")]
    NoFunds {},

    #[error("Multiple denoms in transfer; ICS-20 requires a single denom per packet")]
    MultipleDenoms {},

    #[error("Invalid packet data: {reason}")]
    InvalidPacket { reason: String },

    #[error("Invalid amount: {reason}")]
    InvalidAmount { reason: String },

    #[error("Channel not found: {channel_id}")]
    ChannelNotFound { channel_id: String },

    #[error("Denom trace not found: {ibc_denom}")]
    DenomTraceNotFound { ibc_denom: String },

    #[error("Insufficient escrow balance: need {needed}, have {available}")]
    InsufficientEscrow { needed: String, available: String },

    #[error("Invalid timeout: {reason}")]
    InvalidTimeout { reason: String },

    #[error("{0}")]
    Std(#[from] cosmwasm_std::StdError),
}
