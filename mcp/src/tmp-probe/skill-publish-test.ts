/**
 * skill-publish-test — claim `junoclaw-cosmos-mcp` on the junoclaw-g1
 * skill-registry with the same URI+hash registered on juno-1, from the
 * same publisher key (`builder`) so the publisher identity is
 * consistent across chains. Verifies with get_skill afterwards.
 *
 *   npx tsx src/tmp-probe/skill-publish-test.ts [wallet_id] [skill_hash]
 */
import { getDefaultWalletStore } from "../wallet/store.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";
import type { JunoClawClient } from "../utils/junoclaw-client.js";

const walletId = process.argv[2] ?? "builder";
const skillHash =
  process.argv[3] ??
  "2b46bf32b454ba73909be829126a341266cd1f2b055a3d00b5373eb9b4f16043";
const chain = CHAIN_REGISTRY["junoclaw-g1"];
const SR = chain.skillRegistry!;

async function main() {
  const store = getDefaultWalletStore();
  const { client, address } = (await store.signFor(walletId, chain)) as unknown as {
    client: JunoClawClient;
    address: string;
  };
  console.log(`publishing as ${address} (publisher must match juno-1 entry)`);

  const res = await client.execute(
    address,
    SR,
    {
      publish_skill: {
        dapp_name: "junoclaw-cosmos-mcp",
        chain_id: "junoclaw-g1",
        skill_uri:
          "https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/mcp/SKILL.md",
        skill_hash: skillHash,
      },
    },
    "auto",
    "claim junoclaw-cosmos-mcp on junoclaw-g1 skill-registry",
    [{ denom: "ujclaw", amount: "1000000" }]
  );
  console.log(`execute: code=${res.code} height=${res.height}`);
  console.log(`tx=${res.transactionHash}`);
  if (res.code !== 0) {
    console.log(`rawLog=${res.rawLog}`);
    process.exit(1);
  }

  const entry = await client.queryContractSmart(SR, {
    get_skill: { dapp_name: "junoclaw-cosmos-mcp" },
  });
  console.log(`entry: ${JSON.stringify(entry, null, 1)}`);
}
main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
