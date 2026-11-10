//! gRPC chain client for JunoClaw g1.
//!
//! g1 speaks standard Cosmos SDK gRPC on :9090 — no Tendermint RPC, so
//! cosmrs's HttpClient is unusable here. We keep cosmrs for key handling
//! and tx encoding (SignDoc → TxRaw bytes) and move the transport to
//! tonic unary calls against the SDK services:
//!
//!   /cosmwasm.wasm.v1.Query/SmartContractState   (queries)
//!   /cosmos.auth.v1beta1.Query/Account           (sequence)
//!   /cosmos.tx.v1beta1.Service/BroadcastTx       (submit, SYNC mode)
//!   /cosmos.tx.v1beta1.Service/GetTx             (confirm)
//!   /layer.lightclient.v1.Query/LatestHeight     (custom height oracle)
//!
//! Signing follows the mcp boundary rules: the mnemonic lives ONLY in a
//! named env var, never in config/files/logs.

use anyhow::{anyhow, bail, Context};
use cosmrs::{
    crypto::secp256k1::SigningKey,
    proto::{
        cosmos::{
            auth::v1beta1::{BaseAccount, QueryAccountRequest, QueryAccountResponse},
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
    AccountId, Coin, Denom,
};
use serde::de::DeserializeOwned;
use tonic::{
    client::Grpc,
    codegen::http::uri::PathAndQuery,
    transport::{Channel, Endpoint},
};

use crate::config::Config;

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
}

struct Signer {
    key: SigningKey,
    account_id: AccountId,
}

impl GrpcClient {
    pub async fn connect(cfg: &Config) -> anyhow::Result<Self> {
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
        let signer = load_signer(cfg).ok();
        if signer.is_none() {
            tracing::warn!(
                env = %cfg.mnemonic_env,
                "no signer loaded — anchors will fail until the mnemonic env var is set"
            );
        }
        Ok(Self {
            grpc: Grpc::new(channel),
            chain_id,
            gas_price,
            gas_denom,
            gas_limit: cfg.gas_limit,
            signer,
        })
    }

    pub fn signer_address(&self) -> Option<String> {
        self.signer.as_ref().map(|s| s.account_id.to_string())
    }

    /// Generic unary call against an SDK service path.
    async fn unary<Req: Message + Clone + 'static, Resp: Message + Default + 'static>(
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

    /// Latest finalized height via the chain's lightclient query.
    /// Custom proto we don't codegen: `{ uint64 height = 1 }` shimmed below.
    pub async fn latest_height(&mut self) -> anyhow::Result<u64> {
        let resp: LatestHeightResponse = self
            .unary("/layer.lightclient.v1.Query/LatestHeight", EmptyMsg {})
            .await?;
        Ok(resp.height)
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

    async fn account_info(&mut self, address: &str) -> anyhow::Result<(u64, u64)> {
        let resp: QueryAccountResponse = self
            .unary(
                "/cosmos.auth.v1beta1.Query/Account",
                QueryAccountRequest {
                    address: address.to_string(),
                },
            )
            .await?;
        let any = resp
            .account
            .ok_or_else(|| anyhow!("account not found: {address}"))?;
        let base =
            BaseAccount::decode(any.value.as_slice()).context("decode BaseAccount")?;
        Ok((base.account_number, base.sequence))
    }

    /// Execute a CosmWasm contract (signer must be loaded) and wait for
    /// inclusion. Returns the tx outcome.
    pub async fn execute_contract(
        &mut self,
        contract: &str,
        msg_json: &serde_json::Value,
    ) -> anyhow::Result<TxOutcome> {
        let account_id = self
            .signer
            .as_ref()
            .ok_or_else(|| anyhow!("no signer loaded (mnemonic env unset)"))?
            .account_id
            .clone();

        let exec = cosmrs::cosmwasm::MsgExecuteContract {
            sender: account_id.clone(),
            contract: contract
                .parse::<AccountId>()
                .map_err(|e| anyhow!("contract addr: {e}"))?,
            msg: serde_json::to_vec(msg_json)?,
            funds: vec![],
        };
        let any = exec.to_any().map_err(|e| anyhow!("encode msg: {e}"))?;

        let (account_number, sequence) = self.account_info(&account_id.to_string()).await?;

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
        let tx_body = tx::Body::new(vec![any], "junoclaw-attest", 0u16);

        // Sign under an immutable borrow of the signer (SigningKey isn't Clone).
        let tx_bytes = {
            let key = &self
                .signer
                .as_ref()
                .ok_or_else(|| anyhow!("no signer loaded"))?
                .key;
            let auth_info =
                SignerInfo::single_direct(Some(key.public_key()), sequence).auth_info(fee);
            let sign_doc =
                SignDoc::new(&tx_body, &auth_info, &self.chain_id, account_number)
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
            match self
                .unary::<_, GetTxResponse>(
                    "/cosmos.tx.v1beta1.Service/GetTx",
                    GetTxRequest {
                        hash: tx_hash.clone(),
                    },
                )
                .await
            {
                Ok(resp) => {
                    if let Some(tr) = resp.tx_response {
                        if tr.code != 0 {
                            bail!(
                                "tx failed on-chain (code {}): {}",
                                tr.code,
                                tr.raw_log
                            );
                        }
                        return Ok(TxOutcome {
                            tx_hash,
                            height: tr.height as u64,
                        });
                    }
                }
                Err(_) => continue, // not indexed yet
            }
        }
        // Mempool-accepted but unconfirmed in ~12s — caller treats as pending.
        tracing::warn!(tx = %tx_hash, "tx broadcast but not confirmed within poll window");
        Ok(TxOutcome { tx_hash, height: 0 })
    }
}

fn parse_gas_price(s: &str) -> anyhow::Result<(f64, String)> {
    let split = s
        .find(|c: char| c.is_ascii_alphabetic())
        .ok_or_else(|| anyhow!("gas_price missing denom: {s}"))?;
    let (num, denom) = s.split_at(split);
    Ok((num.parse()?, denom.to_string()))
}

fn load_signer(cfg: &Config) -> anyhow::Result<Signer> {
    let phrase = std::env::var(&cfg.mnemonic_env)
        .map_err(|_| anyhow!("env var {} not set", cfg.mnemonic_env))?;
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

// ── prost shims for the custom height query ───────────────────────────

#[derive(Clone, PartialEq, Message)]
pub struct EmptyMsg {}

#[derive(Clone, PartialEq, Message)]
pub struct LatestHeightResponse {
    #[prost(uint64, tag = "1")]
    pub height: u64,
}
