#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr, Coin, Uint128,
    };

    const ADMIN: &str = "cosmwasm1h34lmpywh4upnjdg90cjf4j70aee6z8qqfspugamjp42e4q28kqs8s7vcp";
    const NEW_ADMIN: &str = "cosmwasm1h34lmpywh4upnjdg90cjf4j70aee6z8qqfspugamjp42e4q28kqs8s7vcp";
    use crate::msg::*;
    use crate::state::*;
    use crate::contract::*;
    use crate::error::ContractError;

    #[test]
    fn instantiate_works() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: Some("transfer".to_string()),
            default_timeout_seconds: Some(3600),
            allowed_denoms: Some(vec!["ujclaw".to_string()]),
        };
        let res = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.port_id, "transfer");
        assert!(config.send_enabled);
        assert!(config.receive_enabled);
        assert_eq!(config.default_timeout_seconds, 3600);
        assert_eq!(config.admin, Addr::unchecked(ADMIN));
        assert_eq!(config.allowed_denoms, vec!["ujclaw".to_string()]);
    }

    #[test]
    fn instantiate_defaults() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        let res = instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();
        assert_eq!(res.messages.len(), 0);

        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.port_id, "transfer");
        assert_eq!(config.default_timeout_seconds, 3600);
        assert!(config.allowed_denoms.is_empty());
    }

    #[test]
    fn query_config_works() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: Some("transfer".to_string()),
            default_timeout_seconds: Some(1800),
            allowed_denoms: Some(vec!["ujclaw".to_string()]),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let config: ConfigResponse = cosmwasm_std::from_json(res).unwrap();
        assert_eq!(config.port_id, "transfer");
        assert_eq!(config.default_timeout_seconds, 1800);
        assert_eq!(config.allowed_denoms, vec!["ujclaw".to_string()]);
    }

    #[test]
    fn query_port_works() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: Some("wasm.jclaw-transfer".to_string()),
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        let res = query(deps.as_ref(), mock_env(), QueryMsg::Port {}).unwrap();
        let port: PortResponse = cosmwasm_std::from_json(res).unwrap();
        assert_eq!(port.port_id, "wasm.jclaw-transfer");
    }

    #[test]
    fn update_config_admin_only() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // Non-admin fails
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("notadmin", &[]),
            ExecuteMsg::UpdateConfig {
                send_enabled: Some(false),
                receive_enabled: None,
                default_timeout_seconds: None,
                allowed_denoms: None,
            },
        );
        assert!(res.is_err());

        // Admin succeeds
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info(ADMIN, &[]),
            ExecuteMsg::UpdateConfig {
                send_enabled: Some(false),
                receive_enabled: Some(false),
                default_timeout_seconds: Some(7200),
                allowed_denoms: Some(vec!["ujclaw".to_string(), "uosmo".to_string()]),
            },
        );
        assert!(res.is_ok());

        let config = CONFIG.load(&deps.storage).unwrap();
        assert!(!config.send_enabled);
        assert!(!config.receive_enabled);
        assert_eq!(config.default_timeout_seconds, 7200);
        assert_eq!(config.allowed_denoms, vec!["ujclaw".to_string(), "uosmo".to_string()]);
    }

    #[test]
    fn transfer_admin_works() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // Non-admin fails
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("notadmin", &[]),
            ExecuteMsg::TransferAdmin {
                new_admin: NEW_ADMIN.to_string(),
            },
        );
        assert!(res.is_err());

        // Admin succeeds
        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info(ADMIN, &[]),
            ExecuteMsg::TransferAdmin {
                new_admin: NEW_ADMIN.to_string(),
            },
        );
        assert!(res.is_ok());

        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.admin, Addr::unchecked(NEW_ADMIN));
    }

    #[test]
    fn denom_trace_ibc_denom() {
        let trace = DenomTrace {
            base_denom: "ujclaw".to_string(),
            path: vec!["transfer/channel-0".to_string()],
        };
        assert_eq!(trace.ibc_denom(), "transfer/channel-0/ujclaw");

        let trace2 = DenomTrace {
            base_denom: "ujclaw".to_string(),
            path: vec![],
        };
        assert_eq!(trace2.ibc_denom(), "ujclaw");
    }

    #[test]
    fn denom_trace_receive() {
        let trace = DenomTrace {
            base_denom: "ujclaw".to_string(),
            path: vec![],
        };
        let received = trace.receive("channel-0", "transfer");
        assert_eq!(received.path, vec!["transfer/channel-0"]);
        assert_eq!(received.ibc_denom(), "transfer/channel-0/ujclaw");
    }

    #[test]
    fn denom_trace_send_back() {
        let trace = DenomTrace {
            base_denom: "ujclaw".to_string(),
            path: vec!["transfer/channel-0".to_string()],
        };
        let sent = trace.send_back("channel-0", "transfer");
        assert!(sent.path.is_empty());
        assert_eq!(sent.ibc_denom(), "ujclaw");

        // Sending through wrong channel doesn't strip
        let sent2 = trace.send_back("channel-1", "transfer");
        assert_eq!(sent2.path, vec!["transfer/channel-0"]);
    }

    #[test]
    fn transfer_no_funds_rejected() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // First create a channel
        let channel_info = ChannelInfo {
            channel_id: "channel-0".to_string(),
            counterparty_channel_id: "channel-0".to_string(),
            counterparty_chain_id: "osmosis-1".to_string(),
            connection_id: "connection-0".to_string(),
            port_id: "transfer".to_string(),
            counterparty_port_id: "transfer".to_string(),
        };
        CHANNELS
            .save(deps.as_mut().storage, "channel-0", &channel_info)
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("sender", &[]),
            ExecuteMsg::Transfer {
                channel_id: "channel-0".to_string(),
                receiver: "osmo1recipient".to_string(),
                timeout_seconds: None,
                memo: None,
            },
        );
        assert!(res.is_err());
        match res.unwrap_err() {
            ContractError::NoFunds {} => {}
            _ => panic!("expected NoFunds error"),
        }
    }

    #[test]
    fn transfer_denom_not_allowed() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: Some(vec!["ujclaw".to_string()]),
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        let channel_info = ChannelInfo {
            channel_id: "channel-0".to_string(),
            counterparty_channel_id: "channel-0".to_string(),
            counterparty_chain_id: "osmosis-1".to_string(),
            connection_id: "connection-0".to_string(),
            port_id: "transfer".to_string(),
            counterparty_port_id: "transfer".to_string(),
        };
        CHANNELS
            .save(deps.as_mut().storage, "channel-0", &channel_info)
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("sender", &[Coin {
                denom: "uosmo".to_string(),
                amount: Uint128::new(1000),
            }]),
            ExecuteMsg::Transfer {
                channel_id: "channel-0".to_string(),
                receiver: "osmo1recipient".to_string(),
                timeout_seconds: None,
                memo: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn transfer_channel_not_found() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("sender", &[Coin {
                denom: "ujclaw".to_string(),
                amount: Uint128::new(1000),
            }]),
            ExecuteMsg::Transfer {
                channel_id: "nonexistent".to_string(),
                receiver: "osmo1recipient".to_string(),
                timeout_seconds: None,
                memo: None,
            },
        );
        assert!(res.is_err());
    }

    #[test]
    fn transfer_disabled_rejects() {
        let mut deps = mock_dependencies();
        let msg = InstantiateMsg {
            admin: ADMIN.to_string(),
            port_id: None,
            default_timeout_seconds: None,
            allowed_denoms: None,
        };
        instantiate(deps.as_mut(), mock_env(), mock_info("creator", &[]), msg).unwrap();

        // Disable sending
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info(ADMIN, &[]),
            ExecuteMsg::UpdateConfig {
                send_enabled: Some(false),
                receive_enabled: None,
                default_timeout_seconds: None,
                allowed_denoms: None,
            },
        )
        .unwrap();

        let channel_info = ChannelInfo {
            channel_id: "channel-0".to_string(),
            counterparty_channel_id: "channel-0".to_string(),
            counterparty_chain_id: "osmosis-1".to_string(),
            connection_id: "connection-0".to_string(),
            port_id: "transfer".to_string(),
            counterparty_port_id: "transfer".to_string(),
        };
        CHANNELS
            .save(deps.as_mut().storage, "channel-0", &channel_info)
            .unwrap();

        let res = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("sender", &[Coin {
                denom: "ujclaw".to_string(),
                amount: Uint128::new(1000),
            }]),
            ExecuteMsg::Transfer {
                channel_id: "channel-0".to_string(),
                receiver: "osmo1recipient".to_string(),
                timeout_seconds: None,
                memo: None,
            },
        );
        assert!(res.is_err());
    }
}
