/**
 * skill-entry-probe — read-only: check whether `junoclaw-cosmos-mcp`
 * has an entry on the junoclaw-g1 skill-registry, and list all entries.
 *   npx tsx src/tmp-probe/skill-entry-probe.ts
 */
import { JunoClawClient } from "../utils/junoclaw-client.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";

const chain = CHAIN_REGISTRY["junoclaw-g1"];
const SR = chain.skillRegistry!;

async function main() {
  const client = await JunoClawClient.connect(
    chain.grpcEndpoint!,
    chain.chainId,
    chain.gasPrice
  );
  try {
    const r = await client.queryContractSmart(SR, {
      get_skill: { dapp_name: "junoclaw-cosmos-mcp" },
    });
    console.log("G1 entry:", JSON.stringify(r));
  } catch (e) {
    console.log("G1 get_skill:", (e as Error).message.slice(0, 300));
  }
  try {
    const l = await client.queryContractSmart(SR, { list_skills: {} });
    console.log("G1 list_skills:", JSON.stringify(l).slice(0, 2000));
  } catch (e) {
    console.log("G1 list_skills:", (e as Error).message.slice(0, 300));
  }
}
main();
