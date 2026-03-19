#!/usr/bin/env tsx
/**
 * Genesis Bundled Proposal: Wire WAVS + Akash + Junoswap in a single
 * CodeUpgrade proposal, then submit the WeightChange to bud into 13.
 *
 * Run order:
 *   1. npx tsx src/genesis-proposal.ts code-upgrade   — wire infrastructure
 *   2. npx tsx src/genesis-proposal.ts vote-upgrade    — vote Yes on prop
 *   3. npx tsx src/genesis-proposal.ts exec-upgrade    — execute after deadline
 *   4. npx tsx src/genesis-proposal.ts bud             — WeightChange → 13 buds
 *   5. npx tsx src/genesis-proposal.ts vote-bud        — vote Yes on bud prop
 *   6. npx tsx src/genesis-proposal.ts exec-bud        — execute budding
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

const AGENT_COMPANY = "juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6";
const JUNOSWAP_FACTORY = "juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh";

async function getClient() {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );
  return { client, sender: account.address };
}

// ── Step 1: Submit the bundled CodeUpgrade proposal ──
async function submitCodeUpgrade() {
  validateConfig();
  const { client, sender } = await getClient();
  console.log(`\n[genesis] Sender (Genesis): ${sender}`);

  const msg = {
    create_proposal: {
      kind: {
        code_upgrade: {
          title: "JunoClaw Infrastructure Bundle: WAVS + Akash + Junoswap Revival",
          description: [
            "This proposal bundles three major integrations into a single governance action:",
            "",
            "1. JUNOSWAP REVIVAL: Wire the Junoswap v2 factory (code_id=61) into agent-company config.",
            "   Factory: juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh",
            "   Pairs deployed: JUNOX/USDC (juno1xn4mtv9...), JUNOX/STAKE (juno156t270z...)",
            "   All swaps are TEE-attested via WAVS verification pipeline.",
            "",
            "2. WAVS INTEGRATION: The WAVS operator (ghcr.io/lay3rlabs/wavs:1.5.1) watches",
            "   on-chain events, runs WASI components inside SGX/SEV enclaves, and submits",
            "   hardware-attested verification results. Proven on proposal 4 (TEE milestone).",
            "",
            "3. AKASH DEPLOYMENT: The WAVS operator stack is deployed on Akash Network for",
            "   decentralized, censorship-resistant compute. SDL ready, 63.77 AKT funded.",
            "   Eliminates single-point-of-failure for off-chain verification.",
            "",
            "Proposed by Genesis (Neo) as Vairagya Node validators.",
            "Requires 67% supermajority to pass.",
          ].join("\n"),
          actions: [
            {
              set_dex_factory: {
                factory_addr: JUNOSWAP_FACTORY,
              },
            },
          ],
        },
      },
    },
  };

  const result = await client.execute(sender, AGENT_COMPANY, msg, "auto",
    "Genesis CodeUpgrade: WAVS + Akash + Junoswap bundle");
  console.log(`[genesis] CodeUpgrade proposal TX: ${result.transactionHash}`);
  console.log(`[genesis] Gas: ${result.gasUsed}`);

  // Query to get proposal ID
  const proposals = await client.queryContractSmart(AGENT_COMPANY, {
    list_proposals: { limit: 1 },
  });
  const latest = proposals[proposals.length - 1];
  console.log(`[genesis] Proposal ID: ${latest.id}`);
  console.log(`[genesis] Status: ${JSON.stringify(latest.status)}`);
  console.log(`[genesis] Voting deadline block: ${latest.voting_deadline_block}`);
}

// ── Step 2: Vote Yes on the CodeUpgrade proposal ──
async function voteUpgrade() {
  validateConfig();
  const { client, sender } = await getClient();

  // Find the latest proposal
  const proposals = await client.queryContractSmart(AGENT_COMPANY, {
    list_proposals: { limit: 10 },
  });
  const codeUpgradeProp = proposals.find((p: any) =>
    p.kind?.code_upgrade && p.status === "open"
  ) || proposals[proposals.length - 1];

  console.log(`\n[genesis] Voting Yes on proposal ${codeUpgradeProp.id}...`);

  const msg = {
    cast_vote: {
      proposal_id: codeUpgradeProp.id,
      vote: "yes",
    },
  };

  const result = await client.execute(sender, AGENT_COMPANY, msg, "auto",
    `Genesis votes Yes on CodeUpgrade prop ${codeUpgradeProp.id}`);
  console.log(`[genesis] Vote TX: ${result.transactionHash}`);

  // Re-query status
  const updated = await client.queryContractSmart(AGENT_COMPANY, {
    get_proposal: { proposal_id: codeUpgradeProp.id },
  });
  console.log(`[genesis] Status after vote: ${JSON.stringify(updated.status)}`);
  console.log(`[genesis] Yes weight: ${updated.yes_weight} / ${updated.total_voted_weight}`);
}

// ── Step 3: Execute the CodeUpgrade proposal ──
async function execUpgrade() {
  validateConfig();
  const { client, sender } = await getClient();

  const proposals = await client.queryContractSmart(AGENT_COMPANY, {
    list_proposals: { limit: 10 },
  });
  const passed = proposals.find((p: any) =>
    p.kind?.code_upgrade && (p.status === "passed" || p.status === "Passed")
  );

  if (!passed) {
    console.error("[genesis] No passed CodeUpgrade proposal found");
    process.exit(1);
  }

  console.log(`\n[genesis] Executing proposal ${passed.id}...`);
  const msg = { execute_proposal: { proposal_id: passed.id } };
  const result = await client.execute(sender, AGENT_COMPANY, msg, "auto",
    `Execute CodeUpgrade prop ${passed.id}`);
  console.log(`[genesis] Execute TX: ${result.transactionHash}`);
  console.log(`[genesis] Gas: ${result.gasUsed}`);

  // Verify DEX factory is wired
  const cfg = await client.queryContractSmart(AGENT_COMPANY, { get_config: {} });
  console.log(`[genesis] DEX factory: ${cfg.dex_factory}`);
  console.log(`\n✅ Infrastructure wired. Ready for budding.`);
}

// ── Step 4: Submit the WeightChange (budding) proposal ──
async function submitBud() {
  validateConfig();
  const { client, sender } = await getClient();
  console.log(`\n[genesis] Genesis address: ${sender}`);

  // 13 buds × 769 = 9997, genesis keeps 3
  // For now, use placeholder addresses — replace with real bud addresses before execution
  const budPrefix = "juno1bud"; // placeholder — replace with real addresses
  const members = [
    { addr: sender, weight: 3, role: "human" }, // Genesis retains symbolic weight
  ];

  // Generate 13 placeholder bud entries
  // In production, these would be real Juno addresses of the 13 bud operators
  for (let i = 1; i <= 13; i++) {
    members.push({
      addr: `${budPrefix}${i.toString().padStart(2, "0")}placeholder`,
      weight: 769,
      role: i % 2 === 0 ? "agent" : "human",
    });
  }

  console.log(`[genesis] Members after budding:`);
  for (const m of members) {
    console.log(`  ${m.role.padEnd(6)} weight=${m.weight} ${m.addr.slice(0, 30)}...`);
  }
  console.log(`  Total weight: ${members.reduce((a, m) => a + m.weight, 0)}`);

  console.log(`\n⚠️  WARNING: Replace placeholder addresses with real bud addresses before executing!`);
  console.log(`⚠️  This script uses placeholder addresses for demonstration.`);
  console.log(`⚠️  Run with real addresses when ready to actually bud.\n`);

  // Don't actually submit with placeholder addresses
  console.log(`[genesis] To submit for real, update the bud addresses in this script and uncomment the execution block.`);

  /*
  const msg = {
    create_proposal: {
      kind: {
        weight_change: { members },
      },
    },
  };

  const result = await client.execute(sender, AGENT_COMPANY, msg, "auto",
    "Genesis budding: distribute weight to 13 buds");
  console.log(`[genesis] Bud proposal TX: ${result.transactionHash}`);
  */
}

// ── Step 5 & 6: Vote and execute budding (same pattern as upgrade) ──

const command = process.argv[2];
switch (command) {
  case "code-upgrade":
    submitCodeUpgrade().catch(console.error);
    break;
  case "vote-upgrade":
    voteUpgrade().catch(console.error);
    break;
  case "exec-upgrade":
    execUpgrade().catch(console.error);
    break;
  case "bud":
    submitBud().catch(console.error);
    break;
  default:
    console.log("Usage: npx tsx src/genesis-proposal.ts <code-upgrade|vote-upgrade|exec-upgrade|bud>");
    console.log("\nRun in order:");
    console.log("  1. code-upgrade  — Submit bundled WAVS+Akash+Junoswap proposal");
    console.log("  2. vote-upgrade  — Genesis votes Yes (100% → auto-pass)");
    console.log("  3. exec-upgrade  — Execute after voting deadline");
    console.log("  4. bud           — Submit WeightChange → 13 buds (placeholder addrs)");
}
