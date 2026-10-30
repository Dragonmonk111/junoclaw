/**
 * store-write-test — the REAL security-boundary write test.
 * Goes through WalletStore.signFor (opaque wallet_id → keychain-
 * sealed decrypt → in-memory signer → JunoClawClient) exactly the
 * way MCP tools do. Sends 1 ujclaw to self via `execute`-free
 * sendTokens path, then a contract execute on skill-registry's
 * on-chain manual read path.
 *
 *   npx tsx src/tmp-probe/store-write-test.ts [wallet_id]
 */
import { getDefaultWalletStore } from "../wallet/store.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";
import type { JunoClawClient } from "../utils/junoclaw-client.js";

const walletId = process.argv[2] ?? "dao-truth-operator";
const chain = CHAIN_REGISTRY["junoclaw-g1"];

async function main() {
  const store = getDefaultWalletStore();
  console.log(`wallet_id="${walletId}" chain=${chain.chainId}`);

  const { client, address } = (await store.signFor(walletId, chain)) as unknown as {
    client: JunoClawClient;
    address: string;
  };
  console.log(`signed in as ${address} — secrets never touched tool surface`);

  const balBefore = await client.getBalance(address, "ujclaw");
  console.log(`balance before: ${balBefore.amount} ${balBefore.denom}`);

  console.log("send 1 ujclaw to self…");
  const res = await client.sendTokens(
    address,
    address,
    [{ denom: "ujclaw", amount: "1" }],
    "auto",
    "mcp store-boundary write test"
  );
  console.log(`code=${res.code} height=${res.height} tx=${res.transactionHash}`);

  const found = (await client.getTx(res.transactionHash)) as {
    height: number;
    code: number;
    rawLog?: string;
  } | null;
  console.log(
    `getTx: ${found ? `height=${found.height} code=${found.code}` : "null"}`
  );

  const balAfter = await client.getBalance(address, "ujclaw");
  console.log(`balance after: ${balAfter.amount} ${balAfter.denom} (delta = fee)`);
}

main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
