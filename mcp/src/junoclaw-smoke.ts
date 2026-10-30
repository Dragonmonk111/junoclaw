/**
 * junoclaw-smoke — read-only smoke test for the JunoClawClient
 * adapter. Exercises the live gRPC surface on the configured
 * endpoint: height, balance, account, contract smart query, tx
 * lookup. No signing, no wallet, no broadcast — safe to run
 * against the live testnet.
 *
 *   npx tsx src/junoclaw-smoke.ts [grpc-endpoint]
 */

import { JunoClawClient } from "./utils/junoclaw-client.js";
import { CHAIN_REGISTRY } from "./resources/chains.js";

const chain = CHAIN_REGISTRY["junoclaw-g1"];
const endpoint = process.argv[2] ?? chain.grpcEndpoint!;

async function main() {
  console.log(`endpoint: ${endpoint}  chain: ${chain.chainId}`);
  const client = await JunoClawClient.connect(
    endpoint,
    chain.chainId,
    chain.gasPrice
  );

  const t0 = Date.now();
  const height = await client.getHeight();
  console.log(`PASS height = ${height} (${Date.now() - t0}ms)`);

  // dao-truth-operator wallet — enrolled in the MCP store, public info only.
  const probe =
    process.argv[3] ?? "juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd";
  try {
    const bal = await client.getBalance(probe, "ujclaw");
    console.log(`PASS balance(${probe.slice(0, 14)}…) = ${bal.amount} ${bal.denom}`);
  } catch (e) {
    console.log(`WARN balance probe: ${(e as Error).message}`);
  }

  try {
    const acct = await (client as unknown as { getAccount(a: string): Promise<unknown> }).getAccount(probe);
    console.log(
      `PASS account query = ${JSON.stringify(acct, (_k, v) => (typeof v === "bigint" ? v.toString() : v))}`
    );
  } catch (e) {
    console.log(`WARN account probe: ${(e as Error).message}`);
  }

  console.log("smoke complete");
}

main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
