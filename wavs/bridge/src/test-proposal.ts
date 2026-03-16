#!/usr/bin/env tsx
/**
 * CP-4: Create, vote on, and execute an OutcomeCreate proposal on agent-company v2.
 * This emits a wasm-outcome_create event that the WAVS operator network picks up.
 *
 * Usage:  npx tsx src/test-proposal.ts
 */

import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

async function main() {
  validateConfig();

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const contract = config.agentCompanyContract;
  console.log(`[cp4] Signer: ${account.address}`);
  console.log(`[cp4] Contract: ${contract}`);

  // Get current block height for deadline
  const currentHeight = await client.getHeight();
  console.log(`[cp4] Current block height: ${currentHeight}`);

  // Step 1: Create OutcomeCreate proposal
  console.log(`\n[cp4] Step 1: Creating OutcomeCreate proposal...`);
  const createMsg = {
    create_proposal: {
      title: "WAVS E2E Test — Outcome Market",
      description: "Test proposal to verify WAVS trigger events fire correctly on-chain",
      kind: {
        outcome_create: {
          question: "Will JunoClaw WAVS attestation integration pass E2E test?",
          resolution_criteria: "Attestation successfully submitted and queried on uni-7",
          deadline_block: currentHeight + 500,
        },
      },
    },
  };

  const createResult = await client.execute(
    account.address,
    contract,
    createMsg,
    "auto",
    "CP-4: OutcomeCreate proposal for WAVS E2E test"
  );
  console.log(`[cp4] Create TX: ${createResult.transactionHash}`);
  console.log(`[cp4] Gas: ${createResult.gasUsed}`);

  // Extract proposal_id from events
  const proposalId = extractAttribute(createResult, "wasm", "proposal_id");
  console.log(`[cp4] Proposal ID: ${proposalId}`);

  // Step 2: Vote Yes
  console.log(`\n[cp4] Step 2: Voting YES on proposal ${proposalId}...`);
  const voteMsg = {
    cast_vote: {
      proposal_id: Number(proposalId),
      vote: "yes",
    },
  };

  const voteResult = await client.execute(
    account.address,
    contract,
    voteMsg,
    "auto",
    "CP-4: Vote YES"
  );
  console.log(`[cp4] Vote TX: ${voteResult.transactionHash}`);
  console.log(`[cp4] Gas: ${voteResult.gasUsed}`);

  // Query proposal to confirm it passed
  const proposal = await client.queryContractSmart(contract, {
    get_proposal: { proposal_id: Number(proposalId) },
  });
  console.log(`[cp4] Proposal status: ${proposal.status}`);
  console.log(`[cp4] Yes votes: ${proposal.yes_weight}, No votes: ${proposal.no_weight}`);

  // Step 3: Execute proposal (emits wasm-outcome_create event)
  // Need to wait for timelock — check if we can execute now or need to wait
  console.log(`\n[cp4] Step 3: Executing proposal ${proposalId}...`);
  const deadlineBlock = proposal.voting_deadline_block || proposal.min_deadline_block || proposal.deadline_block;
  console.log(`[cp4] Voting deadline block: ${deadlineBlock}`);

  const nowHeight = await client.getHeight();
  console.log(`[cp4] Current block: ${nowHeight}`);

  if (deadlineBlock && nowHeight < deadlineBlock) {
    const blocksToWait = deadlineBlock - nowHeight;
    const secondsToWait = blocksToWait * 6;
    console.log(`[cp4] ⏳ Need to wait ${blocksToWait} blocks (~${secondsToWait}s) for voting deadline...`);
    await waitForBlock(client, deadlineBlock);
    console.log(`[cp4] Deadline reached!`);
  }

  const executeMsg = {
    execute_proposal: {
      proposal_id: Number(proposalId),
    },
  };

  const execResult = await client.execute(
    account.address,
    contract,
    executeMsg,
    "auto",
    "CP-4: Execute OutcomeCreate — emit WAVS trigger"
  );
  console.log(`[cp4] Execute TX: ${execResult.transactionHash}`);
  console.log(`[cp4] Gas: ${execResult.gasUsed}`);

  // Check for wasm-outcome_create event attributes
  console.log(`\n[cp4] Checking events...`);
  const events = execResult.events || [];
  for (const event of events) {
    if (event.type.includes("outcome_create") || event.type === "wasm") {
      console.log(`[cp4] Event: ${event.type}`);
      for (const attr of event.attributes) {
        console.log(`[cp4]   ${attr.key} = ${attr.value}`);
      }
    }
  }

  // Verify proposal is now executed
  const finalProposal = await client.queryContractSmart(contract, {
    get_proposal: { proposal_id: Number(proposalId) },
  });
  console.log(`\n[cp4] Final status: ${finalProposal.status}`);

  console.log(`\n╔══════════════════════════════════════════════════════════════╗`);
  console.log(`║  ✅ CP-4 COMPLETE — Proposal executed, WAVS event emitted    ║`);
  console.log(`╠══════════════════════════════════════════════════════════════╣`);
  console.log(`║  Proposal ID:  ${proposalId}`);
  console.log(`║  Execute TX:   ${execResult.transactionHash}`);
  console.log(`║  Use this proposal_id for CP-5 attestation submission       ║`);
  console.log(`╚══════════════════════════════════════════════════════════════╝`);
}

function extractAttribute(
  result: any,
  eventType: string,
  key: string
): string {
  for (const event of result.events || []) {
    if (event.type === eventType || event.type.includes(eventType)) {
      for (const attr of event.attributes) {
        if (attr.key === key) return attr.value;
      }
    }
  }
  throw new Error(`Attribute ${key} not found in ${eventType} events`);
}

async function waitForBlock(
  client: SigningCosmWasmClient,
  targetBlock: number
): Promise<void> {
  while (true) {
    const current = await client.getHeight();
    if (current >= targetBlock) return;
    const remaining = targetBlock - current;
    if (remaining % 5 === 0) {
      console.log(`[cp4]   block ${current} / ${targetBlock} (${remaining} to go)`);
    }
    await new Promise((r) => setTimeout(r, 3000));
  }
}

main().catch((err) => {
  console.error("\n[cp4] Failed:", err.message || err);
  process.exit(1);
});
