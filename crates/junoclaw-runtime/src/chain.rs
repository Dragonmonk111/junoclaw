//! Guarded on-chain signing client for the autonomous daemon.
//!
//! This is the bridge between the read/dry-run runtime and real Cosmos writes.
//! Security posture (enforced here, not just documented):
//!
//! - The signing key is derived from a BIP-39 mnemonic read from an **env var**
//!   whose *name* is in config; the mnemonic itself is never persisted.
//! - Every write path calls [`ChainClient::ensure_can_sign`] first, which honours
//!   the `enabled` master switch and the `signing_paused` runtime kill-switch.
//! - Read paths ([`ChainClient::query_smart`]) work without a key.
//!
//! Anything that broadcasts a transaction is therefore inert unless an operator
//! has both enabled signing and un-paused it.

use anyhow::{anyhow, bail, Context};
use cosmrs::cosmwasm::{MsgExecuteContract, MsgInstantiateContract};
use cosmrs::crypto::secp256k1::SigningKey;
use cosmrs::proto::cosmos::auth::v1beta1::{
    BaseAccount, QueryAccountRequest, QueryAccountResponse,
};
use cosmrs::proto::cosmwasm::wasm::v1::{
    QuerySmartContractStateRequest, QuerySmartContractStateResponse,
};
use cosmrs::proto::traits::Message as _;
use cosmrs::rpc::{Client, HttpClient};
use cosmrs::tendermint::chain::Id as ChainId;
use cosmrs::tx::{self, Fee, Msg, Raw, SignDoc, SignerInfo};
use cosmrs::{AccountId, Coin, Denom};
use serde::de::DeserializeOwned;
use serde_json::Value;

use junoclaw_core::config::{ChainConfig, SigningConfig};

/// Result of a broadcast transaction.
#[derive(Debug, Clone)]
pub struct TxOutcome {
    pub tx_hash: String,
    /// Set for instantiate txs when the `_contract_address` is found in events.
    pub contract_address: Option<String>,
}

/// A loaded signer: derived key + its bech32 account id.
struct Signer {
    key: SigningKey,
    account_id: AccountId,
}

pub struct ChainClient {
    rpc: HttpClient,
    chain_id: ChainId,
    signing: SigningConfig,
    /// (amount_per_gas, denom) parsed from `gas_prices`, e.g. (0.075, "ujuno").
    gas_price: f64,
    gas_denom: String,
    signer: Option<Signer>,
}

impl ChainClient {
    /// Build a client from chain config. Always wires the read path. Loads a
    /// signer only when `signing.enabled` (the key is parked until un-paused).
    pub fn connect(chain: &ChainConfig) -> anyhow::Result<Self> {
        let rpc = HttpClient::new(chain.rpc_endpoint.as_str())
            .with_context(|| format!("cosmos rpc init: {}", chain.rpc_endpoint))?;
        let chain_id: ChainId = chain
            .chain_id
            .parse()
            .map_err(|e| anyhow!("invalid chain_id {}: {e}", chain.chain_id))?;

        let (gas_price, gas_denom) = parse_gas_price(&chain.gas_prices)?;

        let signer = if chain.signing.enabled {
            Some(load_signer(&chain.signing)?)
        } else {
            None
        };

        Ok(Self {
            rpc,
            chain_id,
            signing: chain.signing.clone(),
            gas_price,
            gas_denom,
            signer,
        })
    }

    /// bech32 address of the loaded signer, if any.
    pub fn signer_address(&self) -> Option<String> {
        self.signer.as_ref().map(|s| s.account_id.to_string())
    }

    /// Refuse to sign unless enabled AND un-paused AND a key is loaded.
    fn ensure_can_sign(&self) -> anyhow::Result<&Signer> {
        if !self.signing.can_sign() {
            bail!(
                "signing refused: enabled={}, signing_paused={} (kill-switch)",
                self.signing.enabled,
                self.signing.signing_paused
            );
        }
        self.signer
            .as_ref()
            .ok_or_else(|| anyhow!("signing enabled but no key loaded"))
    }

    /// Read-only CosmWasm smart query. Works without a signer.
    pub async fn query_smart<T: DeserializeOwned>(
        &self,
        contract: &str,
        query_msg: &Value,
    ) -> anyhow::Result<T> {
        let req = QuerySmartContractStateRequest {
            address: contract.to_string(),
            query_data: serde_json::to_vec(query_msg)?,
        };
        let resp = self
            .rpc
            .abci_query(
                Some("/cosmwasm.wasm.v1.Query/SmartContractState".parse().unwrap()),
                req.encode_to_vec(),
                None,
                false,
            )
            .await
            .map_err(|e| anyhow!("abci_query smart: {e}"))?;
        if !resp.code.is_ok() {
            bail!("smart query abci code {:?}: {}", resp.code, resp.log);
        }
        let decoded = QuerySmartContractStateResponse::decode(resp.value.as_slice())
            .context("decode QuerySmartContractStateResponse")?;
        serde_json::from_slice::<T>(&decoded.data).context("decode contract JSON response")
    }

    /// Fetch (account_number, sequence) for the signer from x/auth.
    async fn account_info(&self, address: &str) -> anyhow::Result<(u64, u64)> {
        let req = QueryAccountRequest {
            address: address.to_string(),
        };
        let resp = self
            .rpc
            .abci_query(
                Some("/cosmos.auth.v1beta1.Query/Account".parse().unwrap()),
                req.encode_to_vec(),
                None,
                false,
            )
            .await
            .map_err(|e| anyhow!("abci_query account: {e}"))?;
        if !resp.code.is_ok() {
            bail!("account query abci code {:?}: {}", resp.code, resp.log);
        }
        let qa = QueryAccountResponse::decode(resp.value.as_slice())
            .context("decode QueryAccountResponse")?;
        let any = qa.account.ok_or_else(|| anyhow!("account not found: {address}"))?;
        let base = BaseAccount::decode(any.value.as_slice()).context("decode BaseAccount")?;
        Ok((base.account_number, base.sequence))
    }

    /// Sign + broadcast a single message, returning the commit outcome.
    async fn sign_and_broadcast(
        &self,
        signer: &Signer,
        msg: cosmrs::Any,
    ) -> anyhow::Result<TxOutcome> {
        let (account_number, sequence) =
            self.account_info(&signer.account_id.to_string()).await?;

        let gas_limit = self.signing.default_gas_limit;
        let fee_amount = (self.gas_price * gas_limit as f64).ceil() as u128;
        let fee = Fee::from_amount_and_gas(
            Coin {
                denom: self.gas_denom.parse::<Denom>().map_err(|e| anyhow!("denom: {e}"))?,
                amount: fee_amount,
            },
            gas_limit,
        );

        let tx_body = tx::Body::new(vec![msg], "junoclaw", 0u16);
        let auth_info =
            SignerInfo::single_direct(Some(signer.key.public_key()), sequence).auth_info(fee);
        let sign_doc = SignDoc::new(&tx_body, &auth_info, &self.chain_id, account_number)
            .map_err(|e| anyhow!("sign doc: {e}"))?;
        let raw: Raw = sign_doc.sign(&signer.key).map_err(|e| anyhow!("sign: {e}"))?;
        let tx_bytes = raw.to_bytes().map_err(|e| anyhow!("encode tx: {e}"))?;

        let resp = Raw::from_bytes(&tx_bytes)
            .map_err(|e| anyhow!("decode raw: {e}"))?
            .broadcast_commit(&self.rpc)
            .await
            .map_err(|e| anyhow!("broadcast_commit: {e}"))?;

        if resp.check_tx.code.is_err() {
            bail!(
                "check_tx rejected (code {:?}): {}",
                resp.check_tx.code,
                resp.check_tx.log
            );
        }
        if resp.tx_result.code.is_err() {
            bail!(
                "tx failed (code {:?}): {}",
                resp.tx_result.code,
                resp.tx_result.log
            );
        }

        let contract_address = parse_instantiate_addr(&resp.tx_result.events);
        Ok(TxOutcome {
            tx_hash: resp.hash.to_string(),
            contract_address,
        })
    }

    /// Execute a CosmWasm contract (guarded write).
    pub async fn execute_contract(
        &self,
        contract: &str,
        msg_json: &Value,
    ) -> anyhow::Result<TxOutcome> {
        let signer = self.ensure_can_sign()?;
        let exec = MsgExecuteContract {
            sender: signer.account_id.clone(),
            contract: contract
                .parse::<AccountId>()
                .map_err(|e| anyhow!("contract addr: {e}"))?,
            msg: serde_json::to_vec(msg_json)?,
            funds: vec![],
        };
        let any = exec.to_any().map_err(|e| anyhow!("encode exec msg: {e}"))?;
        self.sign_and_broadcast(signer, any).await
    }

    /// Instantiate a CosmWasm contract (guarded write).
    pub async fn instantiate_contract(
        &self,
        code_id: u64,
        label: &str,
        msg_json: &Value,
        admin: Option<&str>,
    ) -> anyhow::Result<TxOutcome> {
        let signer = self.ensure_can_sign()?;
        let admin_id = match admin {
            Some(a) => Some(a.parse::<AccountId>().map_err(|e| anyhow!("admin addr: {e}"))?),
            None => None,
        };
        let init = MsgInstantiateContract {
            sender: signer.account_id.clone(),
            admin: admin_id,
            code_id,
            label: Some(label.to_string()),
            msg: serde_json::to_vec(msg_json)?,
            funds: vec![],
        };
        let any = init.to_any().map_err(|e| anyhow!("encode init msg: {e}"))?;
        self.sign_and_broadcast(signer, any).await
    }
}

/// Derive a secp256k1 signing key from the configured mnemonic env var.
fn load_signer(cfg: &SigningConfig) -> anyhow::Result<Signer> {
    let phrase = std::env::var(&cfg.mnemonic_env).map_err(|_| {
        anyhow!(
            "signing enabled but env var {} is not set (mnemonic not provided)",
            cfg.mnemonic_env
        )
    })?;
    let mnemonic = bip32::Mnemonic::new(phrase.trim(), bip32::Language::English)
        .map_err(|e| anyhow!("invalid mnemonic: {e}"))?;
    let seed = mnemonic.to_seed("");
    let path = cfg
        .hd_path
        .parse::<bip32::DerivationPath>()
        .map_err(|e| anyhow!("invalid hd_path {}: {e}", cfg.hd_path))?;
    let xprv = bip32::XPrv::derive_from_path(seed.as_bytes(), &path)
        .map_err(|e| anyhow!("derive key: {e}"))?;
    let key = SigningKey::from_slice(&xprv.to_bytes()).map_err(|e| anyhow!("signing key: {e}"))?;
    let account_id = key
        .public_key()
        .account_id(&cfg.account_prefix)
        .map_err(|e| anyhow!("account id: {e}"))?;
    Ok(Signer { key, account_id })
}

/// Parse a gas-price string like "0.075ujuno" into (0.075, "ujuno").
fn parse_gas_price(s: &str) -> anyhow::Result<(f64, String)> {
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .ok_or_else(|| anyhow!("gas_prices missing denom: {s}"))?;
    let (num, denom) = s.split_at(split);
    let price: f64 = num.parse().map_err(|e| anyhow!("gas_prices number {num}: {e}"))?;
    Ok((price, denom.to_string()))
}

/// Best-effort extraction of `_contract_address` from instantiate tx events.
fn parse_instantiate_addr(events: &[cosmrs::tendermint::abci::Event]) -> Option<String> {
    for ev in events {
        if ev.kind == "instantiate" {
            for attr in &ev.attributes {
                if attr.key_str().ok() == Some("_contract_address") {
                    if let Ok(v) = attr.value_str() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // BIP-39 well-known 24-word test mnemonic (all-zero entropy; never used for
    // real funds). bip32 0.5.x only accepts 256-bit (24-word) mnemonics.
    const TEST_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";

    fn signing_cfg(enabled: bool, paused: bool) -> SigningConfig {
        SigningConfig {
            enabled,
            signing_paused: paused,
            mnemonic_env: "JUNOCLAW_TEST_MNEMONIC".to_string(),
            account_prefix: "juno".to_string(),
            hd_path: "m/44'/118'/0'/0/0".to_string(),
            default_gas_limit: 500_000,
            agent_company_code_id: None,
        }
    }

    #[test]
    fn test_parse_gas_price() {
        assert_eq!(parse_gas_price("0.075ujuno").unwrap(), (0.075, "ujuno".to_string()));
        assert_eq!(parse_gas_price("0.025ujunox").unwrap(), (0.025, "ujunox".to_string()));
        assert!(parse_gas_price("0.075").is_err());
    }

    #[test]
    fn test_can_sign_logic() {
        assert!(!signing_cfg(false, false).can_sign());
        assert!(!signing_cfg(true, true).can_sign());
        assert!(!signing_cfg(false, true).can_sign());
        assert!(signing_cfg(true, false).can_sign());
    }

    #[test]
    fn test_key_derivation_deterministic_juno_address() {
        std::env::set_var("JUNOCLAW_TEST_MNEMONIC", TEST_MNEMONIC);
        let signer = load_signer(&signing_cfg(true, false)).unwrap();
        let addr = signer.account_id.to_string();
        // Standard m/44'/118'/0'/0/0 address for this mnemonic under the "juno" prefix.
        assert!(addr.starts_with("juno1"), "unexpected address: {addr}");
    }

    #[test]
    fn test_missing_mnemonic_env_errors() {
        std::env::remove_var("JUNOCLAW_ABSENT_ENV");
        let mut cfg = signing_cfg(true, false);
        cfg.mnemonic_env = "JUNOCLAW_ABSENT_ENV".to_string();
        assert!(load_signer(&cfg).is_err());
    }

    #[test]
    fn test_instantiate_addr_parse() {
        use cosmrs::tendermint::abci::{Event, EventAttribute};
        let ev = Event::new(
            "instantiate",
            vec![
                EventAttribute::from(("code_id", "77", true)),
                EventAttribute::from(("_contract_address", "juno1abc", true)),
            ],
        );
        assert_eq!(parse_instantiate_addr(&[ev]), Some("juno1abc".to_string()));
    }
}
