#!/usr/bin/env node
/**
 * Cosmos MCP Server — AI-native interface to any Cosmos chain
 *
 * Built by JunoClaw. Open source. Chain-agnostic.
 *
 * The journey:
 *   VairagyaNode → validates blocks (service to the network)
 *   JunoClaw     → governs DAOs (service to communities)
 *   Cosmos MCP   → enables builders (service to the ecosystem)
 *
 * Each step is the same act of service at a higher layer.
 * The validator doesn't choose transactions. The MCP doesn't choose chains.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

import { CHAIN_REGISTRY, listChains, listTestnets, getChain } from "./resources/chains.js";
import {
  queryBalance,
  queryAllBalances,
  queryContractState,
  queryContractInfo,
  queryTx,
  queryBlockHeight,
  queryCodeInfo,
} from "./tools/chain-query.js";
import {
  sendTokens,
  executeContract,
  uploadWasm,
  instantiateContract,
  migrateContract,
} from "./tools/tx-builder.js";
import { scaffoldProject, listTemplates, DAO_TEMPLATES } from "./tools/scaffold.js";

const server = new McpServer({
  name: "cosmos-mcp",
  version: "0.1.0",
});

// ════════════════════════════════════════════════════
//  RESOURCES — static knowledge, freely readable
// ════════════════════════════════════════════════════

server.resource(
  "chain-registry",
  "cosmos://chains",
  async (uri) => ({
    contents: [
      {
        uri: uri.href,
        mimeType: "application/json",
        text: JSON.stringify(listChains(), null, 2),
      },
    ],
  })
);

// Individual chain resources
for (const [chainId, chain] of Object.entries(CHAIN_REGISTRY)) {
  server.resource(
    `chain-${chainId}`,
    `cosmos://chains/${chainId}`,
    async (uri) => ({
      contents: [
        {
          uri: uri.href,
          mimeType: "application/json",
          text: JSON.stringify(chain, null, 2),
        },
      ],
    })
  );
}

server.resource(
  "dao-templates",
  "cosmos://templates",
  async (uri) => ({
    contents: [
      {
        uri: uri.href,
        mimeType: "application/json",
        text: JSON.stringify(DAO_TEMPLATES, null, 2),
      },
    ],
  })
);

// ════════════════════════════════════════════════════
//  QUERY TOOLS — read-only, no wallet needed
// ════════════════════════════════════════════════════

server.tool(
  "query_balance",
  "Get token balance for an address on any Cosmos chain",
  {
    chain_id: z.string().describe("Chain ID (e.g. 'uni-7', 'juno-1', 'osmosis-1')"),
    address: z.string().describe("Bech32 address"),
    denom: z.string().optional().describe("Token denom (defaults to chain native)"),
  },
  async ({ chain_id, address, denom }) => {
    const result = await queryBalance(chain_id, address, denom);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_all_balances",
  "Get all token balances for an address",
  {
    chain_id: z.string().describe("Chain ID"),
    address: z.string().describe("Bech32 address"),
  },
  async ({ chain_id, address }) => {
    const result = await queryAllBalances(chain_id, address);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_contract",
  "Query a CosmWasm smart contract (read-only)",
  {
    chain_id: z.string().describe("Chain ID"),
    contract_address: z.string().describe("Contract bech32 address"),
    query_msg: z.string().describe("JSON query message (e.g. '{\"config\":{}}')"),
  },
  async ({ chain_id, contract_address, query_msg }) => {
    const msg = JSON.parse(query_msg);
    const result = await queryContractState(chain_id, contract_address, msg);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_contract_info",
  "Get contract metadata (code ID, creator, admin, label)",
  {
    chain_id: z.string().describe("Chain ID"),
    contract_address: z.string().describe("Contract bech32 address"),
  },
  async ({ chain_id, contract_address }) => {
    const result = await queryContractInfo(chain_id, contract_address);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_tx",
  "Look up a transaction by hash",
  {
    chain_id: z.string().describe("Chain ID"),
    tx_hash: z.string().describe("Transaction hash"),
  },
  async ({ chain_id, tx_hash }) => {
    const result = await queryTx(chain_id, tx_hash);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_block_height",
  "Get current block height of a chain",
  {
    chain_id: z.string().describe("Chain ID"),
  },
  async ({ chain_id }) => {
    const result = await queryBlockHeight(chain_id);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "query_code_info",
  "Get info about an uploaded WASM code ID",
  {
    chain_id: z.string().describe("Chain ID"),
    code_id: z.number().describe("Code ID"),
  },
  async ({ chain_id, code_id }) => {
    const result = await queryCodeInfo(chain_id, code_id);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "list_chains",
  "List all supported Cosmos chains in the registry",
  {},
  async () => {
    const chains = listChains().map((c) => ({
      chainId: c.chainId,
      name: c.chainName,
      denom: c.denom,
      isTestnet: c.isTestnet,
    }));
    return { content: [{ type: "text" as const, text: JSON.stringify(chains, null, 2) }] };
  }
);

// ════════════════════════════════════════════════════
//  TRANSACTION TOOLS — write operations, need mnemonic
// ════════════════════════════════════════════════════

server.tool(
  "send_tokens",
  "Send tokens to an address. Requires mnemonic.",
  {
    chain_id: z.string().describe("Chain ID"),
    mnemonic: z.string().describe("Sender wallet mnemonic (never stored)"),
    recipient: z.string().describe("Recipient bech32 address"),
    amount: z.string().describe("Amount in base denom (e.g. '1000000' for 1 JUNO)"),
    denom: z.string().optional().describe("Token denom (defaults to chain native)"),
    memo: z.string().optional().describe("TX memo"),
  },
  async ({ chain_id, mnemonic, recipient, amount, denom, memo }) => {
    const result = await sendTokens(chain_id, mnemonic, recipient, amount, denom, memo);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "execute_contract",
  "Execute a message on a CosmWasm contract. Requires mnemonic.",
  {
    chain_id: z.string().describe("Chain ID"),
    mnemonic: z.string().describe("Sender wallet mnemonic"),
    contract_address: z.string().describe("Contract bech32 address"),
    msg: z.string().describe("JSON execute message"),
    funds: z.string().optional().describe("JSON array of coins to send with msg"),
    memo: z.string().optional().describe("TX memo"),
  },
  async ({ chain_id, mnemonic, contract_address, msg, funds, memo }) => {
    const parsedMsg = JSON.parse(msg);
    const parsedFunds = funds ? JSON.parse(funds) : undefined;
    const result = await executeContract(chain_id, mnemonic, contract_address, parsedMsg, parsedFunds, memo);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "upload_wasm",
  "Upload a WASM binary to a Cosmos chain. Requires mnemonic.",
  {
    chain_id: z.string().describe("Chain ID"),
    mnemonic: z.string().describe("Uploader wallet mnemonic"),
    wasm_path: z.string().describe("Absolute path to .wasm file"),
    memo: z.string().optional().describe("TX memo"),
  },
  async ({ chain_id, mnemonic, wasm_path, memo }) => {
    const result = await uploadWasm(chain_id, mnemonic, wasm_path, memo);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "instantiate_contract",
  "Instantiate a contract from an uploaded code ID. Requires mnemonic.",
  {
    chain_id: z.string().describe("Chain ID"),
    mnemonic: z.string().describe("Instantiator wallet mnemonic"),
    code_id: z.number().describe("Code ID of uploaded WASM"),
    msg: z.string().describe("JSON instantiate message"),
    label: z.string().describe("Human-readable contract label"),
    admin: z.string().optional().describe("Admin address (defaults to sender)"),
    funds: z.string().optional().describe("JSON array of coins to send"),
    memo: z.string().optional().describe("TX memo"),
  },
  async ({ chain_id, mnemonic, code_id, msg, label, admin, funds, memo }) => {
    const parsedMsg = JSON.parse(msg);
    const parsedFunds = funds ? JSON.parse(funds) : undefined;
    const result = await instantiateContract(chain_id, mnemonic, code_id, parsedMsg, label, admin, parsedFunds, memo);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "migrate_contract",
  "Migrate a contract to a new code ID. Requires admin mnemonic.",
  {
    chain_id: z.string().describe("Chain ID"),
    mnemonic: z.string().describe("Admin wallet mnemonic"),
    contract_address: z.string().describe("Contract to migrate"),
    new_code_id: z.number().describe("New code ID"),
    migrate_msg: z.string().describe("JSON migrate message"),
    memo: z.string().optional().describe("TX memo"),
  },
  async ({ chain_id, mnemonic, contract_address, new_code_id, migrate_msg, memo }) => {
    const parsedMsg = JSON.parse(migrate_msg);
    const result = await migrateContract(chain_id, mnemonic, contract_address, new_code_id, parsedMsg, memo);
    return { content: [{ type: "text" as const, text: JSON.stringify(result, null, 2) }] };
  }
);

// ════════════════════════════════════════════════════
//  SCAFFOLD TOOLS — project generation (juno.new)
// ════════════════════════════════════════════════════

server.tool(
  "list_templates",
  "List available DAO templates for scaffolding",
  {},
  async () => {
    const templates = listTemplates().map((t) => ({
      id: t.id,
      name: t.name,
      description: t.description,
      verification: t.verification,
      features: t.features,
    }));
    return { content: [{ type: "text" as const, text: JSON.stringify(templates, null, 2) }] };
  }
);

server.tool(
  "scaffold_project",
  "Generate a CosmWasm DAO project from a template (juno.new). Returns file contents to write.",
  {
    template_id: z.string().describe("Template ID (use list_templates to see options)"),
    project_name: z.string().describe("Human-readable project name"),
    chain_id: z.string().describe("Target chain ID"),
    members: z.string().describe('JSON array of {address, weight} objects'),
    voting_period: z.number().optional().describe("Voting period in blocks"),
    quorum: z.number().optional().describe("Quorum percentage (1-100)"),
  },
  async ({ template_id, project_name, chain_id, members, voting_period, quorum }) => {
    const parsedMembers = JSON.parse(members);
    const result = scaffoldProject({
      templateId: template_id,
      projectName: project_name,
      chainId: chain_id,
      members: parsedMembers,
      votingPeriod: voting_period,
      quorum,
    });

    const output = {
      description: result.description,
      template: result.template.name,
      deployCommand: result.deployCommand,
      files: result.files.map((f) => ({ path: f.path, lines: f.content.split("\n").length })),
      fileContents: result.files,
    };

    return { content: [{ type: "text" as const, text: JSON.stringify(output, null, 2) }] };
  }
);

// ════════════════════════════════════════════════════
//  PROMPTS — pre-built workflows
// ════════════════════════════════════════════════════

server.prompt(
  "deploy-dao",
  "Step-by-step guide to deploy a DAO on a Cosmos chain",
  {
    chain_id: z.string().describe("Target chain ID").default("uni-7"),
    template_id: z.string().describe("DAO template ID").default("community_vote"),
  },
  ({ chain_id, template_id }) => ({
    messages: [
      {
        role: "user" as const,
        content: {
          type: "text" as const,
          text: `Deploy a ${template_id} DAO on ${chain_id}. Steps:
1. Use list_templates to confirm the template exists
2. Use scaffold_project to generate the contract code
3. Write the generated files to disk
4. Build with: cargo build --target wasm32-unknown-unknown --release
5. Optimize with: wasm-opt -Oz --strip-debug --strip-producers
6. Use upload_wasm to upload the optimized binary
7. Use instantiate_contract with the member list
8. Verify with query_contract using {config:{}}`,
        },
      },
    ],
  })
);

server.prompt(
  "check-contract",
  "Inspect a deployed CosmWasm contract",
  {
    chain_id: z.string().describe("Chain ID").default("uni-7"),
    contract_address: z.string().describe("Contract address"),
  },
  ({ chain_id, contract_address }) => ({
    messages: [
      {
        role: "user" as const,
        content: {
          type: "text" as const,
          text: `Inspect contract ${contract_address} on ${chain_id}:
1. Use query_contract_info to get code ID, creator, admin
2. Use query_contract with {"config":{}} to read configuration
3. Use query_contract with {"list_proposals":{"limit":5}} to see recent proposals
4. Report the contract's purpose, membership, and activity level`,
        },
      },
    ],
  })
);

// ════════════════════════════════════════════════════
//  START
// ════════════════════════════════════════════════════

async function main() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
  console.error("🌌 Cosmos MCP server running — serving any chain, holding no keys");
}

main().catch((err) => {
  console.error("Fatal:", err);
  process.exit(1);
});
