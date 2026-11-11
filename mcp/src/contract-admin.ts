/**
 * contract-admin — generic CosmWasm admin driver for junoclaw-g1.
 *
 * Wraps the shared tx-builder (wallet store → junoclaw-grpc transport)
 * so any contract op can be driven without a bespoke script:
 *
 *   npx tsx src/contract-admin.ts --wallet attest-gw-test \
 *     --upload C:/path/to/foo.wasm
 *   npx tsx src/contract-admin.ts --wallet attest-gw-test \
 *     --instantiate 27 --msg '{"admin":"juno1..."}' --label my-contract
 *   npx tsx src/contract-admin.ts --wallet attest-gw-test \
 *     --exec juno1abc... --msg '{"store_vk":{"vk_base64":"..."}}'
 *   npx tsx src/contract-admin.ts --query juno1abc... \
 *     --qmsg '{"vk_status":{}}'
 *
 * NOTE: junoclaw-g1's execute-then-fail sequence quirk applies — a tx can
 * report failure yet land. Always confirm via --query after a suspicious
 * result before retrying.
 */
import { getDefaultWalletStore } from "./wallet/store.js";
import { queryContractState } from "./tools/chain-query.js";
import {
  executeContract,
  instantiateContract,
  uploadWasm,
} from "./tools/tx-builder.js";

const CHAIN_ID = "junoclaw-g1";
const DENOM = "ujclaw";

function arg(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : undefined;
}
function has(flag: string): boolean {
  return process.argv.includes(flag);
}
function fundsArg(): { denom: string; amount: string }[] | undefined {
  const f = arg("--funds");
  if (!f) return undefined;
  const [denom, amount] = f.split(":");
  return [{ denom, amount }];
}

async function walletId(): Promise<string> {
  const id = arg("--wallet") ?? "attest-gw-test";
  const w = (await getDefaultWalletStore().list()).find((e) => e.id === id);
  if (!w) throw new Error(`wallet ${id} not enrolled`);
  return id;
}

async function main() {
  const wid = await walletId();

  if (has("--upload")) {
    const path = arg("--upload")!;
    const up = await uploadWasm(CHAIN_ID, wid, path, "contract-admin store");
    console.log(`code_id=${up.codeId} tx=${up.txHash} height=${up.height}`);
    return;
  }

  if (has("--instantiate")) {
    const codeId = Number(arg("--instantiate"));
    const msg = JSON.parse(arg("--msg") ?? "{}");
    const inst = await instantiateContract(
      CHAIN_ID,
      wid,
      codeId,
      msg,
      arg("--label") ?? "contract",
      arg("--admin"),
      fundsArg(),
      "contract-admin instantiate"
    );
    console.log(`contract=${inst.contractAddress} tx=${inst.txHash} height=${inst.height}`);
    return;
  }

  if (has("--exec")) {
    const contract = arg("--exec")!;
    const msg = JSON.parse(arg("--msg")!);
    const r = await executeContract(
      CHAIN_ID,
      wid,
      contract,
      msg,
      fundsArg(),
      "contract-admin exec"
    );
    console.log(`tx=${r.txHash} height=${r.height}`);
    return;
  }

  if (has("--query")) {
    const contract = arg("--query")!;
    const qmsg = JSON.parse(arg("--qmsg") ?? "{}");
    console.log(
      JSON.stringify(await queryContractState(CHAIN_ID, contract, qmsg), null, 1)
    );
    return;
  }

  throw new Error(
    "usage: --upload <wasm> | --instantiate <codeId> --msg <> | " +
      "--exec <contract> --msg <> | --query <contract> --qmsg <>"
  );
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : err}`);
  process.exit(1);
});
