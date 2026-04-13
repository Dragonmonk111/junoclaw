#!/usr/bin/env tsx
/**
 * Smoke test: exercise query tools against live uni-7 testnet.
 * No wallet needed — read-only.
 */

import {
  queryBalance,
  queryBlockHeight,
  queryContractInfo,
  queryContractState,
  queryTx,
  queryAllBalances,
} from "./tools/chain-query.js";
import { listTemplates, scaffoldProject } from "./tools/scaffold.js";
import { listChains } from "./resources/chains.js";

const CHAIN = "uni-7";
const NEO_WALLET = "juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m";
const AGENT_COMPANY = "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6";
const ZK_VERIFIER = "juno1ydxksvrfvn7s0qv08nlemj5pguyku0rwzjjmhsnt8m9gxpwc2rlse7ekem";
const ZK_VERIFY_TX = "F6D5774EE2073E2DD011399A7E96889BA026ED67C6A510D208FD5C575080F4DA";

async function test(name: string, fn: () => Promise<unknown>) {
  try {
    const result = await fn();
    console.log(`✓ ${name}`);
    console.log(`  ${JSON.stringify(result).slice(0, 120)}...\n`);
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    console.error(`✗ ${name}: ${msg}\n`);
  }
}

async function main() {
  console.log("═══ Cosmos MCP Smoke Test ═══\n");

  await test("list_chains", async () => {
    const chains = listChains();
    return { count: chains.length, ids: chains.map((c) => c.chainId) };
  });

  await test("query_block_height (uni-7)", () => queryBlockHeight(CHAIN));

  await test("query_balance (Neo wallet)", () => queryBalance(CHAIN, NEO_WALLET));

  await test("query_all_balances (Neo wallet)", () => queryAllBalances(CHAIN, NEO_WALLET));

  await test("query_contract_info (agent-company)", () => queryContractInfo(CHAIN, AGENT_COMPANY));

  await test("query_contract (agent-company config)", () =>
    queryContractState(CHAIN, AGENT_COMPANY, { get_config: {} })
  );

  await test("query_contract_info (zk-verifier)", () => queryContractInfo(CHAIN, ZK_VERIFIER));

  await test("query_contract (zk-verifier VK status)", () =>
    queryContractState(CHAIN, ZK_VERIFIER, { vk_status: {} })
  );

  await test("query_contract (zk-verifier last verify)", () =>
    queryContractState(CHAIN, ZK_VERIFIER, { last_verify: {} })
  );

  await test("query_tx (zk-verifier verify proof TX)", () => queryTx(CHAIN, ZK_VERIFY_TX));

  await test("list_templates", async () => {
    const templates = listTemplates();
    return { count: templates.length, ids: templates.map((t) => t.id) };
  });

  await test("scaffold_project (community_vote)", async () => {
    const result = scaffoldProject({
      templateId: "community_vote",
      projectName: "Test Village DAO",
      chainId: CHAIN,
      members: [
        { address: NEO_WALLET, weight: 5000 },
        { address: "juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2", weight: 5000 },
      ],
    });
    return { description: result.description, files: result.files.length };
  });

  console.log("═══ Done ═══");
}

main().catch(console.error);
