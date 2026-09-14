use cosmwasm_std::{
    entry_point, from_json, to_json_binary, BankMsg, Binary, Coin, CosmosMsg, Deps,
    DepsMut, Env, Ibc3ChannelOpenResponse, IbcBasicResponse, IbcChannelCloseMsg,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcMsg, IbcOrder, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, IbcTimeout,
    MessageInfo, Order, Response, StdAck, StdResult, Timestamp, Uint128,
};
use cw2::set_contract_version;
use cw_storage_plus::{Item, Map};

use crate::error::ContractError;
use crate::msg::*;
use crate::state::*;

const CONTRACT_NAME: &str = "crates.io:cw-ics20-transfer";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// ICS-20 version string for channel handshake
const ICS20_VERSION: &str = "ics20-1";
/// ICS-20 ordering — UNORDERED
const ICS20_ORDERING: IbcOrder = IbcOrder::Unordered;

/// Transfer config
pub const CONFIG: Item<TransferConfig> = Item::new("transfer_config");

/// Channel info: channel_id → ChannelInfo
pub const CHANNELS: Map<&str, ChannelInfo> = Map::new("channels");

/// Escrow balances: (channel_id, denom) → amount
pub const ESCROW: Map<(&str, &str), Uint128> = Map::new("escrow");

/// Denom traces: ibc_denom → DenomTrace
pub const DENOM_TRACES: Map<&str, DenomTrace> = Map::new("denom_traces");

/// Next packet sequence per channel
pub const NEXT_SEQ: Map<&str, u64> = Map::new("next_seq");

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let admin = deps.api.addr_validate(&msg.admin)?;
    let port_id = msg.port_id.unwrap_or_else(|| "transfer".to_string());
    let default_timeout = msg.default_timeout_seconds.unwrap_or(3600); // 1 hour default

    let config = TransferConfig {
        port_id: port_id.clone(),
        send_enabled: true,
        receive_enabled: true,
        default_timeout_seconds: default_timeout,
        admin,
        allowed_denoms: msg.allowed_denoms.unwrap_or_default(),
    };

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("port_id", port_id)
        .add_attribute("default_timeout_seconds", default_timeout.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            channel_id,
            receiver,
            timeout_seconds,
            memo,
        } => execute_transfer(deps, env, info, channel_id, receiver, timeout_seconds, memo),
        ExecuteMsg::UpdateConfig {
            send_enabled,
            receive_enabled,
            default_timeout_seconds,
            allowed_denoms,
        } => execute_update_config(
            deps,
            info,
            send_enabled,
            receive_enabled,
            default_timeout_seconds,
            allowed_denoms,
        ),
        ExecuteMsg::TransferAdmin { new_admin } => execute_transfer_admin(deps, info, new_admin),
    }
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    channel_id: String,
    receiver: String,
    timeout_seconds: Option<u64>,
    memo: Option<String>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if !config.send_enabled {
        return Err(ContractError::TransferDisabled {});
    }

    let _channel = CHANNELS
        .may_load(deps.storage, &channel_id)?
        .ok_or(ContractError::ChannelNotFound {
            channel_id: channel_id.clone(),
        })?;

    // Must have exactly one coin attached
    if info.funds.len() != 1 {
        if info.funds.is_empty() {
            return Err(ContractError::NoFunds {});
        }
        return Err(ContractError::MultipleDenoms {});
    }

    let coin = &info.funds[0];

    // Check allowed denoms if configured
    if !config.allowed_denoms.is_empty()
        && !config.allowed_denoms.contains(&coin.denom)
    {
        return Err(ContractError::DenomNotAllowed {
            denom: coin.denom.clone(),
        });
    }

    // Escrow the tokens (they're already sent to the contract via info.funds)
    // Update escrow balance
    let current = ESCROW
        .may_load(deps.storage, (&channel_id, &coin.denom))?
        .unwrap_or(Uint128::zero());
    let new_balance = current + coin.amount;
    ESCROW.save(deps.storage, (&channel_id, &coin.denom), &new_balance)?;

    // Build the ICS-20 packet
    let packet = Ics20Packet {
        denom: coin.denom.clone(),
        amount: coin.amount.to_string(),
        sender: info.sender.to_string(),
        receiver: receiver.clone(),
        memo: memo.clone(),
    };

    let packet_data = to_json_binary(&packet)?;

    // Calculate timeout
    let timeout_secs = timeout_seconds.unwrap_or(config.default_timeout_seconds);
    let timeout_timestamp = env
        .block
        .time
        .plus_seconds(timeout_secs)
        .nanos();

    // Get next sequence
    let seq = NEXT_SEQ
        .may_load(deps.storage, &channel_id)?
        .unwrap_or(1);
    NEXT_SEQ.save(deps.storage, &channel_id, &(seq + 1))?;

    let ibc_msg = CosmosMsg::Ibc(IbcMsg::SendPacket {
        channel_id: channel_id.clone(),
        data: packet_data,
        timeout: IbcTimeout::with_timestamp(Timestamp::from_nanos(timeout_timestamp)),
    });

    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_attribute("channel_id", channel_id)
        .add_attribute("denom", &coin.denom)
        .add_attribute("amount", coin.amount)
        .add_attribute("receiver", &receiver)
        .add_attribute("sequence", seq.to_string())
        .add_message(ibc_msg))
}

fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    send_enabled: Option<bool>,
    receive_enabled: Option<bool>,
    default_timeout_seconds: Option<u64>,
    allowed_denoms: Option<Vec<String>>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    if let Some(v) = send_enabled {
        config.send_enabled = v;
    }
    if let Some(v) = receive_enabled {
        config.receive_enabled = v;
    }
    if let Some(v) = default_timeout_seconds {
        config.default_timeout_seconds = v;
    }
    if let Some(v) = allowed_denoms {
        config.allowed_denoms = v;
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attribute("action", "update_config"))
}

fn execute_transfer_admin(
    deps: DepsMut,
    info: MessageInfo,
    new_admin: String,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    let new_admin = deps.api.addr_validate(&new_admin)?;
    config.admin = new_admin.clone();
    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_admin")
        .add_attribute("new_admin", new_admin.to_string()))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::Channel { channel_id } => to_json_binary(&query_channel(deps, &channel_id)?),
        QueryMsg::Channels {} => to_json_binary(&query_channels(deps)?),
        QueryMsg::Escrow { channel_id, denom } => {
            to_json_binary(&query_escrow(deps, &channel_id, &denom)?)
        }
        QueryMsg::DenomTrace { ibc_denom } => {
            to_json_binary(&query_denom_trace(deps, &ibc_denom)?)
        }
        QueryMsg::Port {} => to_json_binary(&query_port(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        port_id: config.port_id,
        send_enabled: config.send_enabled,
        receive_enabled: config.receive_enabled,
        default_timeout_seconds: config.default_timeout_seconds,
        admin: config.admin,
        allowed_denoms: config.allowed_denoms,
    })
}

fn query_channel(deps: Deps, channel_id: &str) -> StdResult<ChannelResponse> {
    let channel = CHANNELS.load(deps.storage, channel_id)?;
    Ok(ChannelResponse {
        channel_id: channel.channel_id,
        counterparty_channel_id: channel.counterparty_channel_id,
        counterparty_chain_id: channel.counterparty_chain_id,
        connection_id: channel.connection_id,
        port_id: channel.port_id,
        counterparty_port_id: channel.counterparty_port_id,
    })
}

fn query_channels(deps: Deps) -> StdResult<ChannelsResponse> {
    let channels: Vec<ChannelResponse> = CHANNELS
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, ch) = item.unwrap();
            ChannelResponse {
                channel_id: ch.channel_id,
                counterparty_channel_id: ch.counterparty_channel_id,
                counterparty_chain_id: ch.counterparty_chain_id,
                connection_id: ch.connection_id,
                port_id: ch.port_id,
                counterparty_port_id: ch.counterparty_port_id,
            }
        })
        .collect();
    Ok(ChannelsResponse { channels })
}

fn query_escrow(deps: Deps, channel_id: &str, denom: &str) -> StdResult<EscrowResponse> {
    let amount = ESCROW
        .may_load(deps.storage, (channel_id, denom))?
        .unwrap_or(Uint128::zero());
    Ok(EscrowResponse {
        channel_id: channel_id.to_string(),
        denom: denom.to_string(),
        amount,
    })
}

fn query_denom_trace(deps: Deps, ibc_denom: &str) -> StdResult<DenomTraceResponse> {
    let trace = DENOM_TRACES.load(deps.storage, ibc_denom)?;
    Ok(DenomTraceResponse {
        base_denom: trace.base_denom,
        path: trace.path,
    })
}

fn query_port(deps: Deps) -> StdResult<PortResponse> {
    let config = CONFIG.load(deps.storage)?;
    Ok(PortResponse {
        port_id: config.port_id,
    })
}

// ── IBC Entry Points ──

#[entry_point]
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> Result<Option<Ibc3ChannelOpenResponse>, ContractError> {
    let channel = msg.channel();

    if channel.order != ICS20_ORDERING {
        return Err(ContractError::InvalidPacket {
            reason: "channel must be UNORDERED".to_string(),
        });
    }

    match &msg {
        IbcChannelOpenMsg::OpenInit { .. } => {
            Ok(Some(Ibc3ChannelOpenResponse { version: ICS20_VERSION.to_string() }))
        }
        IbcChannelOpenMsg::OpenTry {
            counterparty_version,
            ..
        } => {
            if counterparty_version.as_str() != ICS20_VERSION {
                return Err(ContractError::InvalidPacket {
                    reason: format!(
                        "counterparty version must be {}, got {}",
                        ICS20_VERSION, counterparty_version
                    ),
                });
            }
            Ok(Some(Ibc3ChannelOpenResponse { version: ICS20_VERSION.to_string() }))
        }
    }
}

#[entry_point]
pub fn ibc_channel_connect(
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    let channel = msg.channel();

    let channel_info = ChannelInfo {
        channel_id: channel.endpoint.channel_id.clone(),
        counterparty_channel_id: channel.counterparty_endpoint.channel_id.clone(),
        counterparty_chain_id: channel.counterparty_endpoint.port_id.clone(), // best available
        connection_id: channel.connection_id.clone(),
        port_id: channel.endpoint.port_id.clone(),
        counterparty_port_id: channel.counterparty_endpoint.port_id.clone(),
    };

    CHANNELS.save(deps.storage, &channel.endpoint.channel_id, &channel_info)?;
    NEXT_SEQ.save(deps.storage, &channel.endpoint.channel_id, &1u64)?;

    Ok(IbcBasicResponse::new()
        .add_attribute("action", "ibc_channel_connect")
        .add_attribute("channel_id", &channel.endpoint.channel_id)
        .add_attribute("counterparty_channel_id", &channel.counterparty_endpoint.channel_id))
}

#[entry_point]
pub fn ibc_channel_close(
    deps: DepsMut,
    _env: Env,
    msg: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    let channel = msg.channel();
    CHANNELS.remove(deps.storage, &channel.endpoint.channel_id);
    NEXT_SEQ.remove(deps.storage, &channel.endpoint.channel_id);

    Ok(IbcBasicResponse::new()
        .add_attribute("action", "ibc_channel_close")
        .add_attribute("channel_id", &channel.endpoint.channel_id))
}

#[entry_point]
pub fn ibc_packet_receive(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if !config.receive_enabled {
        return Ok(IbcReceiveResponse::new(StdAck::error("receive disabled".to_string())));
    }

    let packet = msg.packet;
    let channel_id = packet.dest.channel_id.clone();

    let packet_data: Ics20Packet = match from_json(&packet.data) {
        Ok(d) => d,
        Err(e) => {
            return Ok(IbcReceiveResponse::new(StdAck::error(format!("invalid packet data: {e}"))));
        }
    };

    let amount: Uint128 = match packet_data.amount.parse() {
        Ok(a) => a,
        Err(e) => {
            return Ok(IbcReceiveResponse::new(StdAck::error(format!("invalid amount: {e}"))));
        }
    };

    let incoming_denom = packet_data.denom.clone();
    let our_hop = format!("{}/{}", config.port_id, channel_id);
    let is_returning = incoming_denom.starts_with(&format!("{}/", our_hop));

    let (local_denom, trace) = if is_returning {
        let remaining = &incoming_denom[our_hop.len() + 1..];
        let trace = DenomTrace { base_denom: remaining.to_string(), path: vec![] };
        (remaining.to_string(), trace)
    } else {
        let trace = DenomTrace { base_denom: incoming_denom.clone(), path: vec![our_hop.clone()] };
        let full_denom = trace.ibc_denom();
        (full_denom, trace)
    };

    if is_returning {
        let escrowed = ESCROW
            .may_load(deps.storage, (&channel_id, &local_denom))?
            .unwrap_or(Uint128::zero());

        if escrowed < amount {
            return Ok(IbcReceiveResponse::new(StdAck::error(format!(
                "insufficient escrow: have {}, need {}", escrowed, amount
            ))));
        }

        ESCROW.save(deps.storage, (&channel_id, &local_denom), &(escrowed - amount))?;

        match deps.api.addr_validate(&packet_data.receiver) {
            Ok(addr) => {
                let bank_msg = CosmosMsg::Bank(BankMsg::Send {
                    to_address: addr.to_string(),
                    amount: vec![Coin { denom: local_denom.clone(), amount }],
                });
                Ok(IbcReceiveResponse::new(StdAck::success(b"\x01"))
                    .add_message(bank_msg)
                    .add_attribute("action", "receive_unescrow")
                    .add_attribute("denom", &local_denom)
                    .add_attribute("amount", amount))
            }
            Err(e) => Ok(IbcReceiveResponse::new(StdAck::error(format!("invalid receiver: {e}")))),
        }
    } else {
        DENOM_TRACES.save(deps.storage, &local_denom, &trace)?;

        match deps.api.addr_validate(&packet_data.receiver) {
            Ok(addr) => {
                let bank_msg = CosmosMsg::Bank(BankMsg::Send {
                    to_address: addr.to_string(),
                    amount: vec![Coin { denom: local_denom.clone(), amount }],
                });
                Ok(IbcReceiveResponse::new(StdAck::success(b"\x01"))
                    .add_message(bank_msg)
                    .add_attribute("action", "receive_mint")
                    .add_attribute("denom", &local_denom)
                    .add_attribute("amount", amount))
            }
            Err(e) => Ok(IbcReceiveResponse::new(StdAck::error(format!("invalid receiver: {e}")))),
        }
    }
}

#[entry_point]
pub fn ibc_packet_ack(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    let ack_data: Ics20Ack = match from_json(&msg.acknowledgement.data) {
        Ok(a) => a,
        Err(_) => {
            return Ok(IbcBasicResponse::new()
                .add_attribute("action", "ibc_packet_ack")
                .add_attribute("status", "unparseable"));
        }
    };

    match ack_data {
        Ics20Ack::Result(_) => {
            // Success — tokens are now on the counterparty chain, escrow stays
            Ok(IbcBasicResponse::new()
                .add_attribute("action", "ibc_packet_ack")
                .add_attribute("status", "success"))
        }
        Ics20Ack::Error(err) => {
            // Failed — refund the escrowed tokens to the original sender
            let original_packet: Ics20Packet = from_json(&msg.original_packet.data)?;
            let channel_id = msg.original_packet.src.channel_id.clone();
            let denom = original_packet.denom.clone();
            let amount: Uint128 = original_packet.amount.parse().unwrap_or(Uint128::zero());

            // Reduce escrow
            let current = ESCROW
                .may_load(deps.storage, (&channel_id, &denom))?
                .unwrap_or(Uint128::zero());
            if current >= amount {
                ESCROW.save(deps.storage, (&channel_id, &denom), &(current - amount))?;
            }

            let refund_msg = CosmosMsg::Bank(BankMsg::Send {
                to_address: original_packet.sender,
                amount: vec![Coin { denom: denom.clone(), amount }],
            });

            Ok(IbcBasicResponse::new()
                .add_attribute("action", "ibc_packet_ack_refund")
                .add_attribute("error", err)
                .add_attribute("denom", denom)
                .add_attribute("amount", amount)
                .add_message(refund_msg))
        }
    }
}

#[entry_point]
pub fn ibc_packet_timeout(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    // Timeout — refund the escrowed tokens to the original sender
    let original_packet: Ics20Packet = from_json(&msg.packet.data)?;
    let channel_id = msg.packet.src.channel_id.clone();
    let denom = original_packet.denom.clone();
    let amount: Uint128 = original_packet.amount.parse().unwrap_or(Uint128::zero());

    // Reduce escrow
    let current = ESCROW
        .may_load(deps.storage, (&channel_id, &denom))?
        .unwrap_or(Uint128::zero());
    if current >= amount {
        ESCROW.save(deps.storage, (&channel_id, &denom), &(current - amount))?;
    }

    let refund_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: original_packet.sender,
        amount: vec![Coin { denom: denom.clone(), amount }],
    });

    Ok(IbcBasicResponse::new()
        .add_attribute("action", "ibc_packet_timeout_refund")
        .add_attribute("denom", denom)
        .add_attribute("amount", amount)
        .add_message(refund_msg))
}
