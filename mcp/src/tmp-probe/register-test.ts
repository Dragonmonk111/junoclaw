/**
 * register-test — capstone economy write: RegisterAgent on the
 * live agent-registry contract via the MCP store boundary, then
 * verify with GetAgentsByOwner.
 *
 *   npx tsx src/tmp-probe/register-test.ts [wallet_id]
 */
import { getDefaultWalletStore } from "../wallet/store.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";
import type { JunoClawClient } from "../utils/junoclaw-client.js";

const walletId = process.argv[2] ?? "dao-truth-operator";
const chain = CHAIN_REGISTRY["junoclaw-g1"];
const REGISTRY = "juno1n7fx4duuy59dvscmwrvhqss74heaectwjyac8gn5w2w637l2dxmqy9jgwl";

async function main() {
  const store = getDefaultWalletStore();
  const { client, address } = (await store.signFor(walletId, chain)) as unknown as {
    client: JunoClawClient;
    address: string;
  };
  console.log(`registering agent for ${address}`);

  const res = await client.execute(
    address,
    REGISTRY,
    {
      register_agent: {
        name: "dao-truth-operator-mcp",
        description:
          "MCP adapter verification agent — registered via sovereign junoclaw-grpc transport",
        capabilities_hash:
          "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        model: "swe-2",
      },
    },
    "auto",
    "agent-registry registration via mcp adapter",
    [{ denom: "ujclaw", amount: "1000000" }]
  );
  console.log(`execute: code=${res.code} height=${res.height}`);
  console.log(`tx=${res.transactionHash}`);
  if (res.code !== 0) {
    console.log(`rawLog=${res.rawLog}`);
    process.exit(1);
  }

  const agents = await client.queryContractSmart(REGISTRY, {
    get_agents_by_owner: { owner: address },
  });
  console.log(`agents by owner: ${JSON.stringify(agents, null, 1)}`);
}

main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
