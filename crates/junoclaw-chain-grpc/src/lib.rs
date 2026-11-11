//! Shared gRPC chain client for JunoClaw g1.
//!
//! g1 speaks standard Cosmos SDK gRPC on :9090 — no Tendermint RPC, so
//! cosmrs's HttpClient is unusable here. We keep cosmrs for key handling
//! and tx encoding (SignDoc → TxRaw bytes) and move the transport to
//! tonic unary calls against the SDK services:
//!
//!   /cosmwasm.wasm.v1.Query/SmartContractState   (queries)
//!   /cosmos.auth.v1beta1.Query/Account           (sequence)
//!   /cosmos.bank.v1beta1.Query/*                 (balances)
//!   /cosmos.tx.v1beta1.Service/BroadcastTx       (submit, SYNC mode)
//!   /cosmos.tx.v1beta1.Service/GetTx             (confirm)
//!   /layer.lightclient.v1.Query/LatestHeight     (custom height oracle)
//!   /layer.lightclient.v1.Query/Block            (block + cert)
//!
//! Signing follows the mcp boundary rules: the mnemonic lives ONLY in a
//! named env var, never in config/files/logs.

use anyhow::{anyhow, bail, Context};
use cosmrs::{
    crypto::secp256k1::SigningKey,
    proto::{
        cosmos::{
            auth::v1beta1::{BaseAccount, QueryAccountRequest, QueryAccountResponse},
            bank::v1beta1::{
                QueryAllBalancesRequest, QueryAllBalancesResponse, QueryBalanceRequest,
                QueryBalanceResponse,
            },
            tx::v1beta1::{
                BroadcastMode, BroadcastTxRequest, BroadcastTxResponse, GetTxRequest,
                GetTxResponse,
            },
        },
        cosmwasm::wasm::v1::{QuerySmartContractStateRequest, QuerySmartContractStateResponse},
        prost::Message,
    },
    tendermint::chain::Id as ChainId,
    tx::{self, Fee, Msg, Raw, SignDoc, SignerInfo},
    AccountId, Any, Coin, Denom,
};
use serde::de::DeserializeOwned;
use tonic::{
    client::Grpc,
    codegen::http::uri::PathAndQuery,
    transport::{Channel, Endpoint},
};

/// Client configuration — callers build this from their own config types.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub grpc_endpoint: String,
    pub chain_id: String,
    pub gas_price: String,
    pub gas_limit: u64,
    /// Name of the env var holding the BIP-39 mnemonic. Optional — read-only
    /// clients can pass None; writes then fail with a clear error.
    pub mnemonic_env: Option<String>,
    pub account_prefix: String,
    pub hd_path: String,
}

/// Outcome of a confirmed broadcast.
#[derive(Debug)]
pub struct TxOutcome {
    pub tx_hash: String,
    pub height: u64,
}

pub struct GrpcClient {
    grpc: Grpc<Channel>,
    chain_id: ChainId,
    gas_price: f64,
    gas_denom: String,
    gas_limit: u64,
    signer: Option<Signer>,
    /// Added to the fetched account sequence. g1's ante check can run
    /// ahead of the committed account sequence when txs are still
    /// mempool-pending — callers set this to skip ahead when needed.
    pub seq_offset: u64,
}

struct Signer {
    key: SigningKey,
    account_id: AccountId,
}

impl GrpcClient {
    pub async fn connect(cfg: &ClientConfig) -> anyhow::Result<Self> {
        let url = format!("http://{}", cfg.grpc_endpoint);
        let channel = Endpoint::from_shared(url.clone())
            .with_context(|| format!("bad grpc endpoint {url}"))?
            .connect()
            .await
            .with_context(|| format!("grpc connect {url}"))?;
        let chain_id: ChainId = cfg
            .chain_id
            .parse()
            .map_err(|e| anyhow!("bad chain_id {}: {e}", cfg.chain_id))?;
        let (gas_price, gas_denom) = parse_gas_price(&cfg.gas_price)?;
        let signer = cfg
            .mnemonic_env
            .as_deref()
            .and_then(|env| load_signer(env, cfg).ok());
        if signer.is_none() && cfg.mnemonic_env.is_some() {
            tracing::warn!("no signer loaded — writes will fail until the mnemonic env var is set");
        }
        Ok(Self {
            grpc: Grpc::new(channel),
            chain_id,
            gas_price,
            gas_denom,
            gas_limit: cfg.gas_limit,
            signer,
            seq_offset: 0,
        })
    }

    pub fn signer_address(&self) -> Option<String> {
        self.signer.as_ref().map(|s| s.account_id.to_string())
    }

    /// Generic unary call against an SDK service path.
    pub async fn unary<Req: Message + Clone + 'static, Resp: Message + Default + 'static>(
        &mut self,
        path: &'static str,
        req: Req,
    ) -> anyhow::Result<Resp> {
        self.grpc
            .ready()
            .await
            .map_err(|e| anyhow!("grpc not ready: {e}"))?;
        let resp = self
            .grpc
            .unary(
                tonic::Request::new(req),
                PathAndQuery::from_static(path),
                tonic::codec::ProstCodec::default(),
            )
            .await
            .map_err(|e| anyhow!("grpc {path}: {e}"))?;
        Ok(resp.into_inner())
    }

    /// Latest finalized height + timestamp via the chain's lightclient query.
    pub async fn latest(&mut self) -> anyhow::Result<LatestHeightResponse> {
        self.unary("/layer.lightclient.v1.Query/LatestHeight", EmptyMsg {})
            .await
    }

    /// Latest finalized height only.
    pub async fn latest_height(&mut self) -> anyhow::Result<u64> {
        Ok(self.latest().await?.height)
    }

    /// Block at a height (0 = latest): proposal + threshold cert + payload.
    pub async fn block(&mut self, height: u64) -> anyhow::Result<LayerBlock> {
        self.unary(
            "/layer.lightclient.v1.Query/Block",
            LayerBlockRequest { height },
        )
        .await
    }

    /// State membership proof for a raw storage key.
    pub async fn proof(&mut self, key: &[u8]) -> anyhow::Result<LayerProofResponse> {
        self.unary(
            "/layer.lightclient.v1.Query/Proof",
            LayerProofRequest {
                key: key.to_vec(),
            },
        )
        .await
    }

    /// CosmWasm smart query → JSON value.
    pub async fn query_smart(
        &mut self,
        contract: &str,
        msg: &serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let resp: QuerySmartContractStateResponse = self
            .unary(
                "/cosmwasm.wasm.v1.Query/SmartContractState",
                QuerySmartContractStateRequest {
                    address: contract.to_string(),
                    query_data: serde_json::to_vec(msg)?,
                },
            )
            .await?;
        Ok(serde_json::from_slice(&resp.data).context("decode smart query response")?)
    }

    /// Typed smart query for callers that know the response shape.
    pub async fn query_smart_typed<T: DeserializeOwned>(
        &mut self,
        contract: &str,
        msg: &serde_json::Value,
    ) -> anyhow::Result<T> {
        Ok(serde_json::from_value(
            self.query_smart(contract, msg).await?,
        )?)
    }

    /// Account (number, sequence) or None if the account doesn't exist.
    pub async fn account(&mut self, address: &str) -> anyhow::Result<Option<(u64, u64)>> {
        match self
            .unary::<_, QueryAccountResponse>(
                "/cosmos.auth.v1beta1.Query/Account",
                QueryAccountRequest {
                    address: address.to_string(),
                },
            )
            .await
        {
            Ok(resp) => {
                let any = resp
                    .account
                    .ok_or_else(|| anyhow!("empty account response"))?;
                let base = BaseAccount::decode(any.value.as_slice())
                    .context("decode BaseAccount")?;
                Ok(Some((base.account_number, base.sequence)))
            }
            Err(e) => {
                if e.to_string().contains("not found") || e.to_string().contains("NotFound") {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// All balances for an address.
    pub async fn all_balances(&mut self, address: &str) -> anyhow::Result<Vec<(String, String)>> {
        let resp: QueryAllBalancesResponse = self
            .unary(
                "/cosmos.bank.v1beta1.Query/AllBalances",
                QueryAllBalancesRequest {
                    address: address.to_string(),
                    pagination: None,
                    resolve_denom: false,
                },
            )
            .await?;
        Ok(resp
            .balances
            .into_iter()
            .map(|c| (c.denom, c.amount))
            .collect())
    }

    /// Single-denom balance.
    pub async fn balance(&mut self, address: &str, denom: &str) -> anyhow::Result<String> {
        let resp: QueryBalanceResponse = self
            .unary(
                "/cosmos.bank.v1beta1.Query/Balance",
                QueryBalanceRequest {
                    address: address.to_string(),
                    denom: denom.to_string(),
                },
            )
            .await?;
        Ok(resp.balance.map(|c| c.amount).unwrap_or_else(|| "0".into()))
    }

    /// Fetch a tx by hash.
    pub async fn get_tx(&mut self, hash: &str) -> anyhow::Result<GetTxResponse> {
        self.unary(
            "/cosmos.tx.v1beta1.Service/GetTx",
            GetTxRequest {
                hash: hash.to_string(),
            },
        )
        .await
    }

    /// Execute a CosmWasm contract (signer must be loaded) and wait for
    /// inclusion. Returns the tx outcome.
    pub async fn execute_contract(
        &mut self,
        contract: &str,
        msg_json: &serde_json::Value,
    ) -> anyhow::Result<TxOutcome> {
        self.execute_contract_funded(contract, msg_json, vec![])
            .await
    }

    /// `execute_contract` with native funds attached (Fund{}-style calls).
    pub async fn execute_contract_funded(
        &mut self,
        contract: &str,
        msg_json: &serde_json::Value,
        funds: Vec<(String, u128)>,
    ) -> anyhow::Result<TxOutcome> {
        let account_id = self.require_signer()?;
        let exec = cosmrs::cosmwasm::MsgExecuteContract {
            sender: account_id,
            contract: contract
                .parse::<AccountId>()
                .map_err(|e| anyhow!("contract addr: {e}"))?,
            msg: serde_json::to_vec(msg_json)?,
            funds: funds
                .into_iter()
                .map(|(denom, amount)| Coin {
                    denom: denom.parse::<Denom>().unwrap(),
                    amount,
                })
                .collect(),
        };
        let any = exec.to_any().map_err(|e| anyhow!("encode msg: {e}"))?;
        self.sign_and_broadcast(vec![any]).await
    }

    /// Store wasm bytecode; returns the assigned code_id.
    pub async fn store_code(&mut self, wasm_bytes: &[u8]) -> anyhow::Result<(u64, TxOutcome)> {
        let account_id = self.require_signer()?;
        let store = cosmrs::cosmwasm::MsgStoreCode {
            sender: account_id,
            wasm_byte_code: wasm_bytes.to_vec(),
            instantiate_permission: None,
        };
        let any = store.to_any().map_err(|e| anyhow!("encode msg: {e}"))?;
        let (out, tr) = self.sign_and_broadcast_tr(vec![any]).await?;
        let code_id = tx_event_attr(&tr, "store_code", "code_id")
            .or_else(|| tx_event_attr(&tr, "wasm-store_code", "code_id"))
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| anyhow!("store_code event missing code_id"))?;
        Ok((code_id, out))
    }

    /// Instantiate a stored code_id. `admin` = migration admin
    /// (defaults to sender when None).
    pub async fn instantiate_contract(
        &mut self,
        code_id: u64,
        msg_json: &serde_json::Value,
        label: &str,
        admin: Option<&str>,
        funds: Vec<(String, u128)>,
    ) -> anyhow::Result<(String, TxOutcome)> {
        let account_id = self.require_signer()?;
        let inst = cosmrs::cosmwasm::MsgInstantiateContract {
            sender: account_id,
            admin: admin.map(|a| a.parse::<AccountId>().unwrap()),
            code_id,
            label: Some(label.to_string()),
            msg: serde_json::to_vec(msg_json)?,
            funds: funds
                .into_iter()
                .map(|(denom, amount)| Coin {
                    denom: denom.parse::<Denom>().unwrap(),
                    amount,
                })
                .collect(),
        };
        let any = inst.to_any().map_err(|e| anyhow!("encode msg: {e}"))?;
        let (out, tr) = self.sign_and_broadcast_tr(vec![any]).await?;
        let addr = tx_event_attr(&tr, "instantiate", "_contract_address")
            .or_else(|| tx_event_attr(&tr, "wasm-instantiate", "_contract_address"))
            .ok_or_else(|| anyhow!("instantiate event missing _contract_address"))?;
        Ok((addr, out))
    }

    fn require_signer(&self) -> anyhow::Result<AccountId> {
        Ok(self
            .signer
            .as_ref()
            .ok_or_else(|| anyhow!("no signer loaded (mnemonic env unset)"))?
            .account_id
            .clone())
    }

    /// Sign and broadcast a tx body of pre-encoded messages, then poll
    /// GetTx until inclusion. Static gas — g1 failed txs consume
    /// sequence, so callers should re-query after failures rather than
    /// offset blindly.
    async fn sign_and_broadcast(&mut self, msgs: Vec<Any>) -> anyhow::Result<TxOutcome> {
        Ok(self.sign_and_broadcast_tr(msgs).await?.0)
    }

    /// Same as `sign_and_broadcast` but also returns the indexed
    /// TxResponse for event parsing. None when unconfirmed.
    async fn sign_and_broadcast_tr(
        &mut self,
        msgs: Vec<Any>,
    ) -> anyhow::Result<(TxOutcome, Option<cosmrs::proto::cosmos::base::abci::v1beta1::TxResponse>)> {
        let account_id = self.require_signer()?;
        let (account_number, sequence) = self
            .account(&account_id.to_string())
            .await?
            .ok_or_else(|| anyhow!("signer account not found on-chain"))?;
        let sequence = sequence + self.seq_offset;

        let fee_amount = (self.gas_price * self.gas_limit as f64).ceil() as u128;
        let fee = Fee::from_amount_and_gas(
            Coin {
                denom: self
                    .gas_denom
                    .parse::<Denom>()
                    .map_err(|e| anyhow!("denom: {e}"))?,
                amount: fee_amount,
            },
            self.gas_limit,
        );
        let tx_body = tx::Body::new(msgs, "junoclaw", 0u16);

        // Sign under an immutable borrow of the signer (SigningKey isn't Clone).
        let tx_bytes = {
            let key = &self
                .signer
                .as_ref()
                .ok_or_else(|| anyhow!("no signer loaded"))?
                .key;
            let auth_info =
                SignerInfo::single_direct(Some(key.public_key()), sequence).auth_info(fee);
            let sign_doc = SignDoc::new(&tx_body, &auth_info, &self.chain_id, account_number)
                .map_err(|e| anyhow!("sign doc: {e}"))?;
            let raw: Raw = sign_doc.sign(key).map_err(|e| anyhow!("sign: {e}"))?;
            raw.to_bytes().map_err(|e| anyhow!("encode tx: {e}"))?
        };

        let resp: BroadcastTxResponse = self
            .unary(
                "/cosmos.tx.v1beta1.Service/BroadcastTx",
                BroadcastTxRequest {
                    tx_bytes,
                    mode: BroadcastMode::Sync as i32,
                },
            )
            .await?;
        let tx_resp = resp
            .tx_response
            .ok_or_else(|| anyhow!("broadcast: empty tx_response"))?;
        if tx_resp.code != 0 {
            bail!(
                "broadcast rejected (code {} {}): {}",
                tx_resp.code,
                tx_resp.codespace,
                tx_resp.raw_log
            );
        }
        let tx_hash = tx_resp.txhash.clone();

        // Confirm inclusion — g1 finalizes fast; poll GetTx briefly.
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            match self.get_tx(&tx_hash).await {
                Ok(resp) => {
                    if let Some(tr) = resp.tx_response {
                        if tr.code != 0 {
                            bail!(
                                "tx failed on-chain (code {}): {} [txhash={}]",
                                tr.code,
                                tr.raw_log,
                                tr.txhash
                            );
                        }
                        let height = tr.height as u64;
                        return Ok((TxOutcome { tx_hash, height }, Some(tr)));
                    }
                }
                Err(_) => continue, // not indexed yet
            }
        }
        // Mempool-accepted but unconfirmed in ~12s — caller treats as pending.
        tracing::warn!(tx = %tx_hash, "tx broadcast but not confirmed within poll window");
        Ok((TxOutcome { tx_hash, height: 0 }, None))
    }
}

fn parse_gas_price(s: &str) -> anyhow::Result<(f64, String)> {
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .ok_or_else(|| anyhow!("gas_price missing denom: {s}"))?;
    let (num, denom) = s.split_at(split);
    Ok((num.parse()?, denom.to_string()))
}

fn load_signer(mnemonic_env: &str, cfg: &ClientConfig) -> anyhow::Result<Signer> {
    let phrase = std::env::var(mnemonic_env)
        .map_err(|_| anyhow!("env var {} not set", mnemonic_env))?;
    let mnemonic = bip32::Mnemonic::new(phrase.trim(), bip32::Language::English)
        .map_err(|e| anyhow!("invalid mnemonic: {e}"))?;
    let seed = mnemonic.to_seed("");
    let path = cfg
        .hd_path
        .parse::<bip32::DerivationPath>()
        .map_err(|e| anyhow!("bad hd_path: {e}"))?;
    let xprv = bip32::XPrv::derive_from_path(seed.as_bytes(), &path)
        .map_err(|e| anyhow!("derive: {e}"))?;
    let key =
        SigningKey::from_slice(&xprv.to_bytes()).map_err(|e| anyhow!("signing key: {e}"))?;
    let account_id = key
        .public_key()
        .account_id(&cfg.account_prefix)
        .map_err(|e| anyhow!("account id: {e}"))?;
    Ok(Signer { key, account_id })
}

// ── prost shims for the node's custom layer.* queries ─────────────────
// Shapes mirror proto/layer/lightclient/v1/query.proto in the node repo —
// keep fields aligned or decoding breaks.

#[derive(Clone, PartialEq, Message)]
pub struct EmptyMsg {}

/// layer.lightclient.v1.QueryLatestHeightResponse
#[derive(Clone, PartialEq, Message)]
pub struct LatestHeightResponse {
    #[prost(uint64, tag = "1")]
    pub height: u64,
    /// Block timestamp in nanoseconds since UNIX epoch (0 if unknown).
    #[prost(uint64, tag = "2")]
    pub timestamp_nanos: u64,
}

/// layer.lightclient.v1.QueryBlockRequest
#[derive(Clone, PartialEq, Message)]
pub struct LayerBlockRequest {
    #[prost(uint64, tag = "1")]
    pub height: u64,
}

/// layer.lightclient.v1.QueryBlockResponse — proposal + threshold cert +
/// payload, the pieces an 08-wasm ClientMessage or offline verifier needs.
#[derive(Clone, PartialEq, Message)]
pub struct LayerBlockResponse {
    #[prost(uint64, tag = "1")]
    pub height: u64,
    #[prost(uint64, tag = "2")]
    pub timestamp_nanos: u64,
    /// commonware-codec Proposal { round, parent, payload }.
    #[prost(bytes = "bytes", tag = "3")]
    pub proposal_bytes: Vec<u8>,
    /// Aggregated BLS12-381 threshold signature (48-byte compressed G1).
    #[prost(bytes = "bytes", tag = "4")]
    pub certificate_bytes: Vec<u8>,
    /// bincode BlockPayload — sha256 = digest inside proposal_bytes.
    #[prost(bytes = "bytes", tag = "5")]
    pub payload_bytes: Vec<u8>,
}

/// layer.lightclient.v1.QueryProofRequest
#[derive(Clone, PartialEq, Message)]
pub struct LayerProofRequest {
    #[prost(bytes = "bytes", tag = "1")]
    pub key: Vec<u8>,
}

/// layer.lightclient.v1.QueryProofResponse — membership proof against the
/// state_root carried by the block at state_height + 1.
#[derive(Clone, PartialEq, Message)]
pub struct LayerProofResponse {
    #[prost(uint64, tag = "1")]
    pub state_height: u64,
    #[prost(bytes = "bytes", tag = "2")]
    pub key: Vec<u8>,
    #[prost(bytes = "bytes", tag = "3")]
    pub value: Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub leaf_index: u64,
    #[prost(bytes = "bytes", repeated, tag = "5")]
    pub siblings: Vec<prost::bytes::Bytes>,
}

/// Alias used by the REST layer.
pub type LayerBlock = LayerBlockResponse;

/// Canonical bech32 for contract-embedded addresses.
///
/// During the dual-prefix window users may present `junoclaw1…`
/// addresses, but CosmWasm contracts store `Addr` as the input string
/// verbatim — a `junoclaw1` value stored in state would fail every
/// `juno1` `info.sender` comparison. Anything destined for a contract
/// message arg MUST be canonicalized to the `juno` hrp first; both
/// forms decode to identical bytes chain-side.
pub fn canonical_bech32(addr: &str) -> anyhow::Result<String> {
    let id: AccountId = addr
        .trim()
        .parse()
        .map_err(|e| anyhow!("invalid bech32 address {addr:?}: {e}"))?;
    match id.prefix() {
        "juno" | "junoclaw" => AccountId::new("juno", &id.to_bytes())
            .map(|a| a.to_string())
            .map_err(|e| anyhow!("re-encode bech32: {e}")),
        other => bail!("unsupported bech32 prefix {other:?} (expected juno or junoclaw)"),
    }
}

/// Pull `attr` out of a tx's `event_type` event. SDK events carry
/// key/value as strings (EventAttribute).
fn tx_event_attr(
    tr: &Option<cosmrs::proto::cosmos::base::abci::v1beta1::TxResponse>,
    event_type: &str,
    attr: &str,
) -> Option<String> {
    tr.as_ref()?.events.iter().find_map(|e| {
        if e.r#type != event_type {
            return None;
        }
        e.attributes
            .iter()
            .find(|a| a.key == attr)
            .map(|a| a.value.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_bech32_normalizes_junoclaw_to_juno() {
        let junoclaw_addr = AccountId::new("junoclaw", &[42u8; 20])
            .unwrap()
            .to_string();
        assert!(junoclaw_addr.starts_with("junoclaw1"));
        let canonical = canonical_bech32(&junoclaw_addr).unwrap();
        assert!(canonical.starts_with("juno1"));
        // same canonical bytes either way
        let parse = |s: &str| s.parse::<AccountId>().unwrap().to_bytes();
        assert_eq!(parse(&canonical), parse(&junoclaw_addr));
        // juno input passes through unchanged
        assert_eq!(canonical_bech32(&canonical).unwrap(), canonical);
        // foreign prefixes rejected
        assert!(canonical_bech32(&AccountId::new("osmo", &[1u8; 20]).unwrap().to_string()).is_err());
    }
}
