/**
 * JunoClawClient — transport adapter for the JunoClaw chain.
 *
 * The chain speaks standard Cosmos SDK gRPC on :9090 (no Tendermint
 * RPC), so this client re-implements the *used subset* of
 * CosmWasmClient/SigningCosmWasmClient over direct gRPC calls:
 *
 *   Queries: /cosmos.bank.*, /cosmos.auth.*, /cosmwasm.wasm.*,
 *            /layer.lightclient.v1.Query/LatestHeight
 *   Writes:  OfflineDirectSigner signs a standard SIGN_MODE_DIRECT
 *            SignDoc → TxRaw → cosmos.tx.v1beta1.Service/BroadcastTx
 *            (SYNC) → poll GetTx for the committed result.
 *   Gas:     "auto" fees go through cosmos.tx.v1beta1.Service/Simulate
 *            (real simulation on the node), then
 *            fee = ceil(gas_used * MULTIPLIER * gasPrice).
 *
 * Security: this client never sees a mnemonic. It holds an
 * OfflineDirectSigner produced inside WalletStore's existing
 * decrypt→sign→scrub path — the same boundary the Ffern audit
 * hardened. Nothing secret crosses the wire; the wire carries only
 * protobuf-encoded public data and signatures.
 */

import { toUtf8 } from "@cosmjs/encoding";
import { Decimal, Uint64 } from "@cosmjs/math";
import {
  EncodeObject,
  OfflineDirectSigner,
  Registry,
} from "@cosmjs/proto-signing";
import {
  defaultRegistryTypes,
  GasPrice,
  SignerData,
  StdFee,
} from "@cosmjs/stargate";
import { Any } from "cosmjs-types/google/protobuf/any.js";
import {
  QueryAccountRequest,
  QueryAccountResponse,
} from "cosmjs-types/cosmos/auth/v1beta1/query.js";
import { BaseAccount } from "cosmjs-types/cosmos/auth/v1beta1/auth.js";
import {
  QueryAllBalancesRequest,
  QueryAllBalancesResponse,
  QueryBalanceRequest,
  QueryBalanceResponse,
} from "cosmjs-types/cosmos/bank/v1beta1/query.js";
import { Coin } from "cosmjs-types/cosmos/base/v1beta1/coin.js";
import {
  BroadcastMode,
  BroadcastTxRequest,
  BroadcastTxResponse,
  GetTxRequest,
  GetTxResponse,
  SimulateRequest,
  SimulateResponse,
} from "cosmjs-types/cosmos/tx/v1beta1/service.js";
import { SignMode } from "cosmjs-types/cosmos/tx/signing/v1beta1/signing.js";
import {
  Tx,
  TxRaw,
  TxBody,
  AuthInfo,
  SignerInfo,
  ModeInfo,
  ModeInfo_Single,
  Fee,
} from "cosmjs-types/cosmos/tx/v1beta1/tx.js";
import {
  QuerySmartContractStateRequest,
  QuerySmartContractStateResponse,
  QueryContractInfoRequest,
  QueryContractInfoResponse,
  QueryCodeRequest,
  QueryCodeResponse,
} from "cosmjs-types/cosmwasm/wasm/v1/query.js";
import {
  MsgExecuteContract,
  MsgInstantiateContract,
  MsgMigrateContract,
  MsgStoreCode,
} from "cosmjs-types/cosmwasm/wasm/v1/tx.js";
import { MsgSend } from "cosmjs-types/cosmos/bank/v1beta1/tx.js";
import { sha256 } from "@cosmjs/crypto";

import { grpcUnary } from "./grpc-transport.js";

/**
 * Same encode path as SigningStargateClient: tool callers pass
 * `value` as a decoded plain object and the registry runs
 * fromPartial+encode → Any. CosmWasm message types are registered
 * explicitly (defaultRegistryTypes covers bank/gov/staking/ibc).
 */
const msgRegistry = new Registry([
  ...defaultRegistryTypes,
  ["/cosmwasm.wasm.v1.MsgStoreCode", MsgStoreCode],
  ["/cosmwasm.wasm.v1.MsgInstantiateContract", MsgInstantiateContract],
  ["/cosmwasm.wasm.v1.MsgExecuteContract", MsgExecuteContract],
  ["/cosmwasm.wasm.v1.MsgMigrateContract", MsgMigrateContract],
]);

const GAS_MULTIPLIER = 1.4;
const TX_POLL_MS = 400;
const TX_POLL_ATTEMPTS = 30; // ~12s worst case at fast-finality speeds

function buf(u8: Uint8Array): Buffer {
  return Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength);
}

export interface JunoClawDeliverTx {
  transactionHash: string;
  height: number;
  code: number;
  rawLog?: string;
  gasWanted: number;
  gasUsed: number;
  events: readonly unknown[];
  msgResponses: readonly unknown[];
}

export class JunoClawClient {
  private constructor(
    private readonly endpoint: string,
    private readonly chainId: string,
    private readonly gasPrice: GasPrice,
    private readonly signer?: OfflineDirectSigner,
    private readonly signerAddress?: string
  ) {}

  static async connect(
    endpoint: string,
    chainId: string,
    gasPrice: string
  ): Promise<JunoClawClient> {
    return new JunoClawClient(
      endpoint,
      chainId,
      GasPrice.fromString(gasPrice)
    );
  }

  static async connectWithSigner(
    endpoint: string,
    signer: OfflineDirectSigner,
    chainId: string,
    gasPrice: string
  ): Promise<JunoClawClient> {
    const accounts = await signer.getAccounts();
    if (accounts.length === 0) throw new Error("signer has no accounts");
    return new JunoClawClient(
      endpoint,
      chainId,
      GasPrice.fromString(gasPrice),
      signer,
      accounts[0].address
    );
  }

  // ── queries ──────────────────────────────────────────────────────────

  async getHeight(): Promise<number> {
    // layer.lightclient.v1.Query/LatestHeight — hand-decoded:
    // response = { uint64 height = 1; ... } → tag 0x08 + varint.
    const resp = await grpcUnary(
      this.endpoint,
      "/layer.lightclient.v1.Query/LatestHeight",
      Buffer.alloc(0)
    );
    if (resp.length < 2 || resp[0] !== 0x08)
      throw new Error("LatestHeight: unexpected response shape");
    let height = 0n;
    let shift = 0n;
    let i = 1;
    while (i < resp.length) {
      const b = BigInt(resp[i++]);
      height |= (b & 0x7fn) << shift;
      if (!(b & 0x80n)) break;
      shift += 7n;
    }
    return Number(height);
  }

  async getBalance(address: string, searchDenom: string): Promise<Coin> {
    const req = QueryBalanceRequest.encode(
      QueryBalanceRequest.fromPartial({ address, denom: searchDenom })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmos.bank.v1beta1.Query/Balance",
      buf(req)
    );
    const decoded = QueryBalanceResponse.decode(resp);
    return (
      decoded.balance ?? { denom: searchDenom, amount: "0" }
    );
  }

  async getAllBalances(address: string): Promise<Coin[]> {
    const req = QueryAllBalancesRequest.encode(
      QueryAllBalancesRequest.fromPartial({ address })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmos.bank.v1beta1.Query/AllBalances",
      buf(req)
    );
    return QueryAllBalancesResponse.decode(resp).balances;
  }

  async queryContractSmart(address: string, queryMsg: unknown): Promise<unknown> {
    const req = QuerySmartContractStateRequest.encode(
      QuerySmartContractStateRequest.fromPartial({
        address,
        queryData: toUtf8(JSON.stringify(queryMsg)),
      })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmwasm.wasm.v1.Query/SmartContractState",
      buf(req)
    );
    const decoded = QuerySmartContractStateResponse.decode(resp);
    return JSON.parse(Buffer.from(decoded.data).toString("utf-8"));
  }

  async getContract(address: string): Promise<unknown> {
    const req = QueryContractInfoRequest.encode(
      QueryContractInfoRequest.fromPartial({ address })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmwasm.wasm.v1.Query/ContractInfo",
      buf(req)
    );
    const decoded = QueryContractInfoResponse.decode(resp);
    return { address: decoded.address, contractInfo: decoded.contractInfo };
  }

  async getCodeDetails(codeId: number): Promise<unknown> {
    const req = QueryCodeRequest.encode(
      QueryCodeRequest.fromPartial({ codeId: BigInt(codeId) })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmwasm.wasm.v1.Query/Code",
      buf(req)
    );
    const decoded = QueryCodeResponse.decode(resp);
    return decoded.codeInfo;
  }

  private async getAccount(
    address: string
  ): Promise<{ accountNumber: bigint; sequence: bigint }> {
    const req = QueryAccountRequest.encode(
      QueryAccountRequest.fromPartial({ address })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmos.auth.v1beta1.Query/Account",
      buf(req)
    );
    const decoded = QueryAccountResponse.decode(resp);
    if (!decoded.account) throw new Error(`account ${address} not found`);
    const base = BaseAccount.decode(decoded.account.value);
    return {
      accountNumber: base.accountNumber,
      sequence: base.sequence,
    };
  }

  async getTx(id: string): Promise<unknown | null> {
    const req = GetTxRequest.encode(
      GetTxRequest.fromPartial({ hash: id })
    ).finish();
    try {
      const resp = await grpcUnary(
        this.endpoint,
        "/cosmos.tx.v1beta1.Service/GetTx",
        buf(req)
      );
      const decoded = GetTxResponse.decode(resp);
      const r = decoded.txResponse;
      if (!r) return null;
      return {
        height: Number(r.height),
        txIndex: 0,
        hash: r.txhash,
        code: r.code,
        rawLog: r.rawLog,
        gasWanted: Number(r.gasWanted),
        gasUsed: Number(r.gasUsed),
        events: r.events,
        tx: decoded.tx ? Tx.encode(decoded.tx).finish() : new Uint8Array(),
      };
    } catch {
      return null;
    }
  }

  // ── writes ───────────────────────────────────────────────────────────

  private requireSigner() {
    if (!this.signer || !this.signerAddress)
      throw new Error("JunoClawClient was created without a signer");
    return { signer: this.signer, address: this.signerAddress };
  }

  private async buildTxRaw(
    msgs: EncodeObject[],
    fee: StdFee,
    memo: string,
    signerData: SignerData
  ): Promise<Uint8Array> {
    const { signer, address } = this.requireSigner();
    const accounts = await signer.getAccounts();
    const pubkey = Any.fromPartial({
      typeUrl: "/cosmos.crypto.secp256k1.PubKey",
      value: buf(
        // secp256k1.PubKey { key = 1 } — single bytes field
        Uint8Array.from([0x0a, accounts[0].pubkey.length, ...accounts[0].pubkey])
      ),
    });

    const body = TxBody.encode(
      TxBody.fromPartial({
        messages: msgs.map((m) => msgRegistry.encodeAsAny(m)),
        memo,
      })
    ).finish();

    const authInfo = AuthInfo.encode(
      AuthInfo.fromPartial({
        signerInfos: [
          SignerInfo.fromPartial({
            publicKey: pubkey,
            modeInfo: ModeInfo.fromPartial({
              single: ModeInfo_Single.fromPartial({
                mode: SignMode.SIGN_MODE_DIRECT,
              }),
            }),
            sequence: BigInt(signerData.sequence),
          }),
        ],
        fee: Fee.fromPartial({
          amount: fee.amount.map((c) => Coin.fromPartial(c)),
          gasLimit: BigInt(fee.gas),
        }),
      })
    ).finish();

    const signDoc = {
      bodyBytes: body,
      authInfoBytes: authInfo,
      chainId: signerData.chainId,
      accountNumber: BigInt(signerData.accountNumber),
    };
    const { signature } = await signer.signDirect(address, signDoc);

    return TxRaw.encode(
      TxRaw.fromPartial({
        bodyBytes: body,
        authInfoBytes: authInfo,
        signatures: [Buffer.from(signature.signature, "base64")],
      })
    ).finish();
  }

  private async simulate(rawTx: Uint8Array): Promise<bigint> {
    const req = SimulateRequest.encode(
      SimulateRequest.fromPartial({ txBytes: rawTx })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmos.tx.v1beta1.Service/Simulate",
      buf(req)
    );
    const decoded = SimulateResponse.decode(resp);
    if (!decoded.gasInfo) throw new Error("simulate: no gas_info returned");
    return decoded.gasInfo.gasUsed;
  }

  private async autoFee(
    msgs: EncodeObject[],
    memo: string,
    signerData: SignerData
  ): Promise<StdFee> {
    // Simulate the fully-signed tx with a generous gas ceiling; the
    // node's simulate path executes the msgs against committed state.
    const probe: StdFee = {
      amount: [
        {
          denom: this.gasPrice.denom,
          amount: Decimal.fromUserInput("1", 6).ceil().toString(), // 1 ujclaw
        },
      ],
      gas: "3000000",
    };
    const probeTx = await this.buildTxRaw(msgs, probe, memo, signerData);
    const gasUsed = await this.simulate(probeTx);
    const gas = BigInt(Math.ceil(Number(gasUsed) * GAS_MULTIPLIER));
    const amount = this.gasPrice.amount
      .multiply(Uint64.fromString(gas.toString()))
      .ceil();
    return {
      amount: [{ denom: this.gasPrice.denom, amount: amount.toString() }],
      gas: gas.toString(),
    };
  }

  async signAndBroadcast(
    signerAddress: string,
    msgs: EncodeObject[],
    fee: StdFee | "auto",
    memo = ""
  ): Promise<JunoClawDeliverTx> {
    const { address } = this.requireSigner();
    if (signerAddress !== address)
      throw new Error("signerAddress does not match the signer account");

    const account = await this.getAccount(address);
    const signerData: SignerData = {
      accountNumber: Number(account.accountNumber),
      sequence: Number(account.sequence),
      chainId: this.chainId,
    };

    const realFee =
      fee === "auto" ? await this.autoFee(msgs, memo, signerData) : fee;
    const rawTx = await this.buildTxRaw(msgs, realFee, memo, signerData);

    const req = BroadcastTxRequest.encode(
      BroadcastTxRequest.fromPartial({
        txBytes: rawTx,
        mode: BroadcastMode.BROADCAST_MODE_SYNC,
      })
    ).finish();
    const resp = await grpcUnary(
      this.endpoint,
      "/cosmos.tx.v1beta1.Service/BroadcastTx",
      buf(req)
    );
    const decoded = BroadcastTxResponse.decode(resp);
    const check = decoded.txResponse;
    if (check && check.code !== 0) {
      throw new Error(
        `check_tx rejected (code ${check.code}, ${check.codespace}): ${check.rawLog}`
      );
    }
    const txhash = (check?.txhash ||
      Buffer.from(sha256(rawTx)).toString("hex").toUpperCase()) as string;

    // SYNC means check_tx passed — the tx is in the mempool. Poll GetTx
    // until it commits (fast finality: normally one poll).
    for (let i = 0; i < TX_POLL_ATTEMPTS; i++) {
      await new Promise((r) => setTimeout(r, TX_POLL_MS));
      const found = (await this.getTx(txhash)) as
        | {
            height: number;
            code: number;
            rawLog?: string;
            gasWanted: number;
            gasUsed: number;
            events: readonly unknown[];
          }
        | null;
      if (found) {
        return {
          transactionHash: txhash,
          height: found.height,
          code: found.code,
          rawLog: found.rawLog,
          gasWanted: found.gasWanted,
          gasUsed: found.gasUsed,
          events: found.events,
          msgResponses: [],
        };
      }
    }
    throw new Error(
      `tx ${txhash} accepted into mempool but not committed within ~${(TX_POLL_MS * TX_POLL_ATTEMPTS) / 1000}s`
    );
  }

  async execute(
    senderAddress: string,
    contractAddress: string,
    msg: Record<string, unknown>,
    fee: StdFee | "auto",
    memo = "",
    funds?: readonly Coin[]
  ): Promise<JunoClawDeliverTx> {
    const execMsg: EncodeObject = {
      typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
      value: MsgExecuteContract.fromPartial({
        sender: senderAddress,
        contract: contractAddress,
        msg: toUtf8(JSON.stringify(msg)),
        funds: (funds ?? []).map((c) => Coin.fromPartial(c)),
      }),
    };
    return this.signAndBroadcast(senderAddress, [execMsg], fee, memo);
  }

  async sendTokens(
    senderAddress: string,
    recipientAddress: string,
    transferAmount: readonly Coin[],
    fee: StdFee | "auto",
    memo = ""
  ): Promise<JunoClawDeliverTx> {
    const sendMsg: EncodeObject = {
      typeUrl: "/cosmos.bank.v1beta1.MsgSend",
      value: MsgSend.fromPartial({
        fromAddress: senderAddress,
        toAddress: recipientAddress,
        amount: transferAmount.map((c) => Coin.fromPartial(c)),
      }),
    };
    return this.signAndBroadcast(senderAddress, [sendMsg], fee, memo);
  }

  async upload(
    senderAddress: string,
    wasmCode: Uint8Array,
    fee: StdFee | "auto",
    memo = ""
  ): Promise<{ codeId: number } & JunoClawDeliverTx> {
    const uploadMsg: EncodeObject = {
      typeUrl: "/cosmwasm.wasm.v1.MsgStoreCode",
      value: MsgStoreCode.fromPartial({
        sender: senderAddress,
        wasmByteCode: wasmCode,
      }),
    };
    const res = await this.signAndBroadcast(senderAddress, [uploadMsg], fee, memo);
    // code_id is in the events; parse defensively
    const eventStr = JSON.stringify(res.events);
    const m = eventStr.match(/"key":"code_id","value":"(\d+)"/) ??
      eventStr.match(/code_id["\s:]+(\d+)/);
    return { ...res, codeId: m ? Number(m[1]) : 0 };
  }

  async instantiate(
    senderAddress: string,
    adminAddress: string | undefined,
    codeId: number,
    msg: Record<string, unknown>,
    label: string,
    fee: StdFee | "auto",
    options?: { funds?: readonly Coin[] }
  ): Promise<{ contractAddress: string } & JunoClawDeliverTx> {
    const initMsg: EncodeObject = {
      typeUrl: "/cosmwasm.wasm.v1.MsgInstantiateContract",
      value: MsgInstantiateContract.fromPartial({
        sender: senderAddress,
        admin: adminAddress ?? "",
        codeId: BigInt(codeId),
        label,
        msg: toUtf8(JSON.stringify(msg)),
        funds: (options?.funds ?? []).map((c) => Coin.fromPartial(c)),
      }),
    };
    const res = await this.signAndBroadcast(senderAddress, [initMsg], fee, options && "memo" in options ? (options as { memo?: string }).memo ?? "" : "");
    const eventStr = JSON.stringify(res.events);
    const m = eventStr.match(/_contract_address["\s,:]+"(juno1[a-z0-9]+)"/) ??
      eventStr.match(/"(juno1[a-z0-9]{38,})"/);
    return { ...res, contractAddress: m ? m[1] : "" };
  }

  async migrate(
    senderAddress: string,
    contractAddress: string,
    codeId: number,
    msg: Record<string, unknown>,
    fee: StdFee | "auto",
    memo = ""
  ): Promise<JunoClawDeliverTx> {
    const migrateMsg: EncodeObject = {
      typeUrl: "/cosmwasm.wasm.v1.MsgMigrateContract",
      value: MsgMigrateContract.fromPartial({
        sender: senderAddress,
        contract: contractAddress,
        codeId: BigInt(codeId),
        msg: toUtf8(JSON.stringify(msg)),
      }),
    };
    return this.signAndBroadcast(senderAddress, [migrateMsg], fee, memo);
  }
}
