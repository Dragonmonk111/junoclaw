/**
 * tmp-probe — READ-ONLY infrastructure verification for the
 * JunoClawClient gRPC adapter. No signing, no broadcast.
 *
 *   npx tsx src/tmp-probe/probe.ts
 */

import { JunoClawClient } from "../utils/junoclaw-client.js";
import { grpcUnary } from "../utils/grpc-transport.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";
import {
  QueryCodesRequest,
  QueryCodesResponse,
  QueryContractsByCodeRequest,
  QueryContractsByCodeResponse,
  QueryAllContractStateRequest,
  QueryAllContractStateResponse,
  QueryPinnedCodesRequest,
  QueryPinnedCodesResponse,
  QueryParamsRequest,
  QueryParamsResponse,
  QueryContractHistoryRequest,
  QueryContractHistoryResponse,
} from "cosmjs-types/cosmwasm/wasm/v1/query.js";
import {
  GetTxsEventRequest,
  GetTxsEventResponse,
  GetBlockWithTxsRequest,
  GetBlockWithTxsResponse,
  OrderBy,
} from "cosmjs-types/cosmos/tx/v1beta1/service.js";
import {
  GetLatestBlockRequest,
  GetLatestBlockResponse,
  GetNodeInfoRequest,
  GetNodeInfoResponse,
} from "cosmjs-types/cosmos/base/tendermint/v1beta1/query.js";
import { PageRequest } from "cosmjs-types/cosmos/base/query/v1beta1/pagination.js";

const chain = CHAIN_REGISTRY["junoclaw-g1"];
const ENDPOINT = process.argv[2] ?? chain.grpcEndpoint!;
const DENOM = "ujclaw";

const WALLETS: [string, string][] = [
  ["builder", "juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z"],
  ["dao-truth-operator", "juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd"],
  ["dao-verdict-helper", "juno1jpgd0vcveam90smx5rk6cuycv9lfemcpasf0fv"],
  ["relayer-soak", "juno1uzwj8lk4nqap5v5suv3a20zhmyghp2j3ac0tsh"],
  ["signing-smoke-uni7", "juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6"],
];

const js = (v: unknown) =>
  JSON.stringify(v, (_k, x) => (typeof x === "bigint" ? x.toString() : x));

function buf(u8: Uint8Array): Buffer {
  return Buffer.from(u8.buffer, u8.byteOffset, u8.byteLength);
}

const unimpl: { path: string; error: string }[] = [];

async function call<T>(
  path: string,
  req: Buffer,
  decode: (b: Buffer | Uint8Array) => T
): Promise<{ ok: true; data: T } | { ok: false; error: string }> {
  try {
    const resp = await grpcUnary(ENDPOINT, path, req);
    return { ok: true, data: decode(resp) };
  } catch (e) {
    const msg = (e as Error).message;
    if (/UNIMPLEMENTED/i.test(msg)) unimpl.push({ path, error: msg });
    return { ok: false, error: msg };
  }
}

async function main() {
  console.log(`=== junoclaw probe — endpoint ${ENDPOINT} ===\n`);
  const client = await JunoClawClient.connect(ENDPOINT, chain.chainId, chain.gasPrice);
  const height = await client.getHeight();
  console.log(`height = ${height}\n`);

  // ── 1. BALANCE SWEEP ────────────────────────────────────────────────
  console.log("--- BALANCE SWEEP ---");
  for (const [name, addr] of WALLETS) {
    let bal = "<err>",
      all = "<err>",
      acct = "<err>";
    try {
      const b = await client.getBalance(addr, DENOM);
      bal = `${b.amount} ${b.denom}`;
    } catch (e) {
      bal = `ERR ${(e as Error).message}`;
    }
    try {
      const coins = await client.getAllBalances(addr);
      all = coins.length ? coins.map((c) => `${c.amount}${c.denom}`).join(",") : "(none)";
    } catch (e) {
      all = `ERR ${(e as Error).message}`;
    }
    try {
      const a = await (client as unknown as {
        getAccount(a: string): Promise<{ accountNumber: bigint; sequence: bigint }>;
      }).getAccount(addr);
      acct = `accNum=${a.accountNumber} seq=${a.sequence}`;
    } catch (e) {
      acct = `ERR ${(e as Error).message}`;
    }
    console.log(`${name.padEnd(20)} ${addr}`);
    console.log(`  balance=${bal} | all=${all} | ${acct}`);
  }

  // ── 2. CONTRACT DISCOVERY ───────────────────────────────────────────
  console.log("\n--- CONTRACT DISCOVERY ---");

  // 2a. /cosmwasm.wasm.v1.Query/Codes
  const codeIds: bigint[] = [];
  const codeLabels = new Map<string, string>();
  {
    let key: Uint8Array | undefined;
    let page = 0;
    let codesErr = "";
    do {
      const req = QueryCodesRequest.encode(
        QueryCodesRequest.fromPartial({
          pagination: PageRequest.fromPartial({ key, limit: 100n }),
        })
      ).finish();
      const r = await call(
        "/cosmwasm.wasm.v1.Query/Codes",
        buf(req),
        (b) => QueryCodesResponse.decode(b)
      );
      if (!r.ok) {
        codesErr = r.error;
        break;
      }
      for (const ci of r.data.codeInfos) {
        codeIds.push(ci.codeId);
        codeLabels.set(ci.codeId.toString(), `${ci.creator}`);
      }
      key = r.data.pagination?.nextKey?.length ? r.data.pagination.nextKey : undefined;
      page++;
    } while (key && page < 20);
    if (codesErr) {
      console.log(`Query/Codes FAILED: ${codesErr}`);
    } else {
      console.log(`Query/Codes OK — ${codeIds.length} code(s): [${codeIds.join(", ")}]`);
      for (const ci of codeIds) {
        try {
          const det = (await client.getCodeDetails(Number(ci))) as {
            codeId?: bigint;
            creator?: string;
            dataHash?: Uint8Array;
            instantiatePermission?: unknown;
          } | undefined;
          console.log(
            `  codeId=${ci} creator=${det?.creator ?? "?"} dataHash=${
              det?.dataHash ? Buffer.from(det.dataHash).toString("hex") : "?"
            }`
          );
        } catch (e) {
          console.log(`  codeId=${ci} getCodeDetails ERR: ${(e as Error).message}`);
        }
      }
    }
  }

  // 2b. for each codeId: /cosmwasm.wasm.v1.Query/ContractsByCode
  const contracts: { address: string; codeId: bigint }[] = [];
  for (const codeId of codeIds) {
    let key: Uint8Array | undefined;
    let page = 0;
    do {
      const req = QueryContractsByCodeRequest.encode(
        QueryContractsByCodeRequest.fromPartial({
          codeId,
          pagination: PageRequest.fromPartial({ key, limit: 100n }),
        })
      ).finish();
      const r = await call(
        "/cosmwasm.wasm.v1.Query/ContractsByCode",
        buf(req),
        (b) => QueryContractsByCodeResponse.decode(b)
      );
      if (!r.ok) {
        console.log(`ContractsByCode(codeId=${codeId}) FAILED: ${r.error}`);
        break;
      }
      for (const a of r.data.contracts) contracts.push({ address: a, codeId });
      key = r.data.pagination?.nextKey?.length ? r.data.pagination.nextKey : undefined;
      page++;
    } while (key && page < 20);
  }
  console.log(`discovered ${contracts.length} contract(s)`);

  // fallback: QueryContractsByCreator for each wallet (if Codes/ContractsByCode empty)
  if (contracts.length === 0) {
    const { QueryContractsByCreatorRequest, QueryContractsByCreatorResponse } =
      await import("cosmjs-types/cosmwasm/wasm/v1/query.js");
    for (const [name, addr] of WALLETS) {
      const enc = QueryContractsByCreatorRequest.encode(
        QueryContractsByCreatorRequest.fromPartial({
          creatorAddress: addr,
          pagination: PageRequest.fromPartial({ limit: 100n }),
        })
      ).finish();
      const r = await call(
        "/cosmwasm.wasm.v1.Query/ContractsByCreator",
        buf(enc),
        (b) => QueryContractsByCreatorResponse.decode(b)
      );
      if (r.ok) {
        for (const a of r.data.contractAddresses)
          contracts.push({ address: a, codeId: -1n });
        console.log(`ContractsByCreator(${name}): ${r.data.contractAddresses.length} contract(s)`);
      } else {
        console.log(`ContractsByCreator(${name}) FAILED: ${r.error}`);
      }
    }
  }

  // 2c. getContract + smart queries for each contract
  const QUERY_CANDIDATES: Record<string, unknown>[] = [
    { config: {} },
    { info: {} },
    { get_config: {} },
    { state: {} },
    { contract_info: {} },
    { params: {} },
    { get_state: {} },
  ];
  for (const c of contracts) {
    console.log(`\ncontract ${c.address} (codeId=${c.codeId})`);
    try {
      const info = (await client.getContract(c.address)) as {
        address: string;
        contractInfo?: {
          codeId?: bigint;
          creator?: string;
          admin?: string;
          label?: string;
        };
      };
      const ci = info.contractInfo;
      console.log(
        `  label="${ci?.label ?? "?"}" codeId=${ci?.codeId ?? "?"} creator=${ci?.creator ?? "?"} admin=${ci?.admin ?? "?"}`
      );
    } catch (e) {
      console.log(`  getContract ERR: ${(e as Error).message}`);
    }
    for (const q of QUERY_CANDIDATES) {
      try {
        const res = await client.queryContractSmart(c.address, q);
        console.log(`  smart ${JSON.stringify(q)} => ${js(res)?.slice(0, 500)}`);
        break; // one successful smart query per contract is enough
      } catch (e) {
        const msg = (e as Error).message;
        console.log(`  smart ${JSON.stringify(q)} ERR: ${msg.slice(0, 160)}`);
      }
    }
    // extra coverage: AllContractState + ContractHistory (first contract only is enough, but keep it cheap — only contract index 0)
  }

  // 2d. extra wasm query surface on first contract (coverage check)
  if (contracts.length > 0) {
    const a = contracts[0].address;
    const r1 = await call(
      "/cosmwasm.wasm.v1.Query/AllContractState",
      buf(
        QueryAllContractStateRequest.encode(
          QueryAllContractStateRequest.fromPartial({
            address: a,
            pagination: PageRequest.fromPartial({ limit: 5n }),
          })
        ).finish()
      ),
      (b) => QueryAllContractStateResponse.decode(b)
    );
    console.log(
      r1.ok
        ? `AllContractState(${a.slice(0, 18)}…): ${r1.data.models.length} model(s) shown`
        : `AllContractState FAILED: ${r1.error}`
    );
    const r2 = await call(
      "/cosmwasm.wasm.v1.Query/ContractHistory",
      buf(
        QueryContractHistoryRequest.encode(
          QueryContractHistoryRequest.fromPartial({
            address: a,
            pagination: PageRequest.fromPartial({ limit: 10n }),
          })
        ).finish()
      ),
      (b) => QueryContractHistoryResponse.decode(b)
    );
    console.log(
      r2.ok
        ? `ContractHistory: ${js(r2.data.entries)?.slice(0, 400)}`
        : `ContractHistory FAILED: ${r2.error}`
    );
  }
  {
    const rp = await call(
      "/cosmwasm.wasm.v1.Query/PinnedCodes",
      buf(
        QueryPinnedCodesRequest.encode(
          QueryPinnedCodesRequest.fromPartial({
            pagination: PageRequest.fromPartial({ limit: 50n }),
          })
        ).finish()
      ),
      (b) => QueryPinnedCodesResponse.decode(b)
    );
    console.log(rp.ok ? `PinnedCodes: [${rp.data.codeIds.join(",")}]` : `PinnedCodes FAILED: ${rp.error}`);
    const rq = await call(
      "/cosmwasm.wasm.v1.Query/Params",
      buf(QueryParamsRequest.encode(QueryParamsRequest.fromPartial({})).finish()),
      (b) => QueryParamsResponse.decode(b)
    );
    console.log(rq.ok ? `wasm Params: ${js(rq.data.params)?.slice(0, 300)}` : `wasm Params FAILED: ${rq.error}`);
  }

  // ── 3. GETTX ────────────────────────────────────────────────────────
  console.log("\n--- GETTX ---");
  const txHashes: string[] = [];
  const eventQueries = [
    ["tx.height>0"],
    ["message.action='/cosmwasm.wasm.v1.MsgInstantiateContract'"],
    ["message.action='/cosmwasm.wasm.v1.MsgExecuteContract'"],
    ["message.action='/cosmos.bank.v1beta1.MsgSend'"],
    ["wasm._contract_address='*'"],
    ["instantiate._contract_address='*'"],
  ];
  for (const ev of eventQueries) {
    const req = GetTxsEventRequest.encode(
      GetTxsEventRequest.fromPartial({
        events: ev,
        orderBy: OrderBy.ORDER_BY_DESC,
        limit: 5n,
      })
    ).finish();
    const r = await call("/cosmos.tx.v1beta1.Service/GetTxsByEvent", buf(req), (b) =>
      GetTxsEventResponse.decode(b)
    );
    if (!r.ok) {
      console.log(`GetTxsByEvent ${js(ev)} FAILED: ${r.error}`);
      continue;
    }
    const hashes = r.data.txResponses.map((t) => t.txhash);
    console.log(`GetTxsByEvent ${js(ev)} => ${hashes.length} tx(s): ${js(hashes.slice(0, 3))}`);
    for (const h of hashes) if (!txHashes.includes(h)) txHashes.push(h);
    if (txHashes.length) break;
  }

  // fallback: scan recent blocks via GetBlockWithTxs
  if (txHashes.length === 0) {
    console.log("GetTxsByEvent gave no hashes — trying GetBlockWithTxs scan");
    for (let h = height; h > Math.max(0, height - 200) && txHashes.length === 0; h--) {
      const req = GetBlockWithTxsRequest.encode(
        GetBlockWithTxsRequest.fromPartial({ height: BigInt(h) })
      ).finish();
      const r = await call("/cosmos.tx.v1beta1.Service/GetBlockWithTxs", buf(req), (b) =>
        GetBlockWithTxsResponse.decode(b)
      );
      if (!r.ok) {
        console.log(`GetBlockWithTxs(h=${h}) FAILED: ${r.error}`);
        break;
      }
      if (r.data.txs.length > 0) {
        // hash = sha256(raw tx bytes)
        const { sha256 } = await import("@cosmjs/crypto");
        const { Tx } = await import("cosmjs-types/cosmos/tx/v1beta1/tx.js");
        for (const tx of r.data.txs) {
          const raw = Tx.encode(tx).finish();
          txHashes.push(Buffer.from(sha256(raw)).toString("hex").toUpperCase());
        }
        console.log(`block ${h}: ${r.data.txs.length} tx(s) -> ${js(txHashes)}`);
      }
    }
  }

  // also sanity-check tendermint-ish service surface
  {
    const r = await call(
      "/cosmos.base.tendermint.v1beta1.Service/GetLatestBlock",
      buf(GetLatestBlockRequest.encode(GetLatestBlockRequest.fromPartial({})).finish()),
      (b) => GetLatestBlockResponse.decode(b)
    );
    console.log(
      r.ok
        ? `GetLatestBlock OK — block height=${r.data.block?.header?.height ?? r.data.sdkBlock?.header?.height}`
        : `GetLatestBlock FAILED: ${r.error}`
    );
    const rn = await call(
      "/cosmos.base.tendermint.v1beta1.Service/GetNodeInfo",
      buf(GetNodeInfoRequest.encode(GetNodeInfoRequest.fromPartial({})).finish()),
      (b) => GetNodeInfoResponse.decode(b)
    );
    console.log(
      rn.ok
        ? `GetNodeInfo OK — network=${rn.data.defaultNodeInfo?.network} version=${rn.data.applicationVersion?.version}`
        : `GetNodeInfo FAILED: ${rn.error}`
    );
  }

  if (txHashes.length > 0) {
    const h = txHashes[0];
    const res = (await client.getTx(h)) as {
      height: number;
      hash: string;
      code: number;
      rawLog?: string;
      gasWanted: number;
      gasUsed: number;
    } | null;
    if (res) {
      console.log(
        `client.getTx(${h}) => height=${res.height} code=${res.code} gas=${res.gasUsed}/${res.gasWanted} rawLog=${(res.rawLog ?? "").slice(0, 200)}`
      );
    } else {
      console.log(`client.getTx(${h}) => null (GetTx lookup failed)`);
    }
  } else {
    console.log("No committed tx hash discoverable via any method.");
  }

  // ── 4. UNIMPLEMENTED SUMMARY ────────────────────────────────────────
  console.log("\n--- UNIMPLEMENTED PATHS ---");
  if (unimpl.length === 0) console.log("(none)");
  for (const u of unimpl) console.log(`${u.path} :: ${u.error}`);
  console.log("\nprobe complete");
}

main().catch((e) => {
  console.error(`FATAL: ${(e as Error).message}`);
  process.exit(1);
});
