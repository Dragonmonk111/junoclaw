#!/usr/bin/env tsx
/**
 * Agentic Parliament Demo — 7 AI "Members of Parliament" on Juno testnet.
 *
 * Each MP has a policy stance. Proposals are evaluated against stances.
 * Votes are deterministic but on-chain — real wallets, real transactions.
 *
 * Run order:
 *   1. npx tsx src/parliament-demo.ts setup       — generate 7 MP wallets, fund, instantiate Parliament
 *   2. npx tsx src/parliament-demo.ts propose      — submit a policy proposal
 *   3. npx tsx src/parliament-demo.ts debate       — show each MP's reasoning
 *   4. npx tsx src/parliament-demo.ts vote         — all MPs cast votes on-chain
 *   5. npx tsx src/parliament-demo.ts tally        — show results
 *   6. npx tsx src/parliament-demo.ts execute      — execute passed proposal
 *   7. npx tsx src/parliament-demo.ts status       — show full parliament state
 */

import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config, validateConfig } from "./config.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const STATE_FILE = resolve(__dirname, "../parliament-state.json");

// ── Agent-company v3 code ID on uni-7 ──
const CODE_ID = 63;

// ── MP Definitions ──
interface MPProfile {
  name: string;
  role: string;
  stance: string;
  keywords_yes: string[];
  keywords_no: string[];
  bias: "yes" | "no" | "abstain";
  weight: number;
}

const MP_PROFILES: MPProfile[] = [
  {
    name: "The Builder",
    role: "Infrastructure Chair",
    stance: "Supports development, tooling, and protocol upgrades. Believes in building first, debating later.",
    keywords_yes: ["develop", "build", "upgrade", "infrastructure", "tooling", "deploy", "contract", "code", "fund developer"],
    keywords_no: ["cut", "reduce", "freeze", "halt"],
    bias: "yes",
    weight: 1429,
  },
  {
    name: "The Fiscal Hawk",
    role: "Treasury Oversight",
    stance: "Opposes unnecessary spending. Every token must be justified. Favors lean operations.",
    keywords_yes: ["audit", "reduce cost", "efficiency", "save", "burn", "cut spending"],
    keywords_no: ["spend", "fund", "allocate", "grant", "pay", "treasury", "airdrop"],
    bias: "no",
    weight: 1429,
  },
  {
    name: "The Populist",
    role: "Community Representative",
    stance: "Supports anything that directly benefits token holders — airdrops, staking rewards, community pools.",
    keywords_yes: ["community", "airdrop", "staking", "reward", "distribute", "holder", "governance", "vote"],
    keywords_no: ["centralize", "restrict", "elite", "multisig only"],
    bias: "yes",
    weight: 1429,
  },
  {
    name: "The Technocrat",
    role: "Verification Standards",
    stance: "Only supports proposals backed by on-chain proof or WAVS attestation. Evidence or abstain.",
    keywords_yes: ["verify", "attest", "proof", "wavs", "tee", "sgx", "audit", "test", "evidence"],
    keywords_no: ["trust", "promise", "soon", "roadmap", "maybe"],
    bias: "abstain",
    weight: 1429,
  },
  {
    name: "The Diplomat",
    role: "Cross-Chain Relations",
    stance: "Favors IBC connections, cross-chain integrations, and interoperability. Bridges over walls.",
    keywords_yes: ["ibc", "bridge", "cross-chain", "interop", "osmosis", "neutron", "cosmos", "atom", "relay"],
    keywords_no: ["isolate", "disconnect", "close channel"],
    bias: "yes",
    weight: 1429,
  },
  {
    name: "The Environmentalist",
    role: "Sustainability Advocate",
    stance: "Supports long-term sustainability, validator health, and network efficiency. Thinks in decades.",
    keywords_yes: ["sustain", "long-term", "validator", "health", "efficient", "green", "optimize", "stable"],
    keywords_no: ["short-term", "pump", "moon", "rush", "quick"],
    bias: "abstain",
    weight: 1429,
  },
  {
    name: "The Contrarian",
    role: "Devil's Advocate",
    stance: "Questions everything. If consensus is forming too fast, something is being missed. Default: NO.",
    keywords_yes: ["review", "reconsider", "delay", "investigate", "second opinion"],
    keywords_no: ["urgent", "fast-track", "no discussion", "pass now", "immediately"],
    bias: "no",
    weight: 1426, // slightly less to make total = 10000
  },
];

// ── Sample Proposals ──
const SAMPLE_PROPOSALS = [
  {
    title: "Fund Community Developer Pool — 5,000 JUNOX",
    description: [
      "Allocate 5,000 JUNOX from the Parliament treasury to a community developer pool.",
      "Developers can apply for grants to build tools, bots, and integrations on Juno.",
      "The pool will be managed by a 3-of-7 multisig of sitting MPs.",
      "All spending requires WAVS-attested receipts.",
    ].join("\n"),
  },
  {
    title: "Open IBC Channel to Osmosis for Junoswap Liquidity",
    description: [
      "Establish a new IBC channel between Juno and Osmosis to enable cross-chain liquidity.",
      "This would allow JUNO/OSMO pairs on Junoswap v2 with WAVS-verified swaps.",
      "Relay infrastructure funded from existing Akash deployment budget.",
      "Long-term goal: unified liquidity across Cosmos DEXes with attestation proofs.",
    ].join("\n"),
  },
  {
    title: "Mandate WAVS Attestation for All Contract Upgrades",
    description: [
      "Require that all future smart contract migrations produce a WAVS TEE attestation",
      "proving the new code matches the published source and passes all tests.",
      "This raises the bar for code upgrades from trust-based to proof-based.",
      "The WASI verification component would hash the compiled binary and compare",
      "against the published GitHub commit hash, producing a hardware-sealed receipt.",
    ].join("\n"),
  },
];

// ── Persistent State ──
interface ParliamentState {
  parliament_contract: string;
  mps: {
    name: string;
    address: string;
    mnemonic: string;
    profile_index: number;
  }[];
  current_proposal_id: number | null;
  current_proposal_index: number;
}

function loadState(): ParliamentState | null {
  if (!existsSync(STATE_FILE)) return null;
  return JSON.parse(readFileSync(STATE_FILE, "utf-8"));
}

function saveState(state: ParliamentState) {
  writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}

async function getClientFor(mnemonic: string) {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: "juno",
  });
  const [account] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );
  return { client, sender: account.address };
}

// ── Deterministic Vote Logic ──
function evaluateVote(
  profile: MPProfile,
  title: string,
  description: string
): { vote: "yes" | "no" | "abstain"; reasoning: string } {
  const text = `${title} ${description}`.toLowerCase();

  let yesScore = 0;
  let noScore = 0;

  for (const kw of profile.keywords_yes) {
    if (text.includes(kw.toLowerCase())) yesScore += 2;
  }
  for (const kw of profile.keywords_no) {
    if (text.includes(kw.toLowerCase())) noScore += 2;
  }

  // Apply bias
  if (profile.bias === "yes") yesScore += 1;
  if (profile.bias === "no") noScore += 1;

  let vote: "yes" | "no" | "abstain";
  let reasoning: string;

  if (yesScore > noScore) {
    vote = "yes";
    reasoning = `Aligns with my priorities (score: +${yesScore} / -${noScore}). Voting YES.`;
  } else if (noScore > yesScore) {
    vote = "no";
    reasoning = `Conflicts with my stance (score: +${yesScore} / -${noScore}). Voting NO.`;
  } else {
    vote = profile.bias === "abstain" ? "abstain" : profile.bias;
    reasoning = `Inconclusive (score: +${yesScore} / -${noScore}). Falling back to default: ${vote.toUpperCase()}.`;
  }

  return { vote, reasoning };
}

// ════════════════════════════════════════════════════════════════
// COMMANDS
// ════════════════════════════════════════════════════════════════

// ── SETUP: Generate wallets, fund, instantiate Parliament ──
async function setup() {
  validateConfig();
  const { client: neoClient, sender: neoAddr } = await getClientFor(config.mnemonic);
  console.log(`\n⚙️  Setting up Agentic Parliament on ${config.chainId}`);
  console.log(`   Funder (Neo): ${neoAddr}`);

  const neoBalance = await neoClient.getBalance(neoAddr, config.denom);
  console.log(`   Balance: ${neoBalance.amount} ${neoBalance.denom}\n`);

  // Step 1: Generate 7 MP wallets
  console.log("━━━ Step 1: Generating MP wallets ━━━\n");
  const mps: ParliamentState["mps"] = [];
  const members: any[] = [];

  for (let i = 0; i < MP_PROFILES.length; i++) {
    const profile = MP_PROFILES[i];
    const wallet = await DirectSecp256k1HdWallet.generate(24, { prefix: "juno" });
    const [account] = await wallet.getAccounts();
    console.log(`  MP ${i + 1}: ${profile.name.padEnd(22)} → ${account.address}`);

    mps.push({
      name: profile.name,
      address: account.address,
      mnemonic: wallet.mnemonic,
      profile_index: i,
    });

    members.push({
      addr: account.address,
      role: "human",
      weight: profile.weight,
      alias: profile.name,
    });
  }

  const totalWeight = MP_PROFILES.reduce((a, p) => a + p.weight, 0);
  console.log(`\n  Total weight: ${totalWeight}`);

  // Step 2: Fund each MP with gas money (500 JUNOX each = 3,500 total)
  console.log("\n━━━ Step 2: Funding MP wallets ━━━\n");
  const fundAmount = "500000000"; // 500 JUNOX (6 decimals)

  for (const mp of mps) {
    const result = await neoClient.sendTokens(
      neoAddr,
      mp.address,
      [{ denom: config.denom, amount: fundAmount }],
      "auto",
      `Fund parliament MP: ${mp.name}`
    );
    console.log(`  Funded ${mp.name.padEnd(22)} → TX: ${result.transactionHash.slice(0, 16)}...`);
  }

  // Step 3: Instantiate Parliament contract
  console.log("\n━━━ Step 3: Instantiating Parliament DAO ━━━\n");

  const instantiateMsg = {
    name: "JunoClaw Agentic Parliament",
    admin: null,
    governance: null,
    escrow_contract: "juno1dh43lswg5ekv7q2p44s6hgays47k5mz67742vdwpd025p8q05kgs0azwrv",
    agent_registry: "juno1qulyspwzjzsz7rq65v6ptzt278f9ta9uh0upxu6xa08gf4v5gzaqm676j7",
    task_ledger: null,
    nois_proxy: null,
    members,
    proposal_timelock_blocks: 5,
    denom: config.denom,
    voting_period_blocks: 200,
    quorum_percent: 100,
    adaptive_threshold_blocks: 0,
    adaptive_min_blocks: 200,
    verification: null,
  };

  const instantiateResult = await neoClient.instantiate(
    neoAddr,
    CODE_ID,
    instantiateMsg,
    "JunoClaw Agentic Parliament",
    "auto",
    {
      admin: neoAddr,
      memo: "7-member AI parliament demo on uni-7",
    }
  );

  console.log(`  Contract: ${instantiateResult.contractAddress}`);
  console.log(`  TX: ${instantiateResult.transactionHash}`);

  // Verify
  const cfg = await neoClient.queryContractSmart(
    instantiateResult.contractAddress,
    { get_config: {} }
  );
  console.log(`  Name: ${cfg.name}`);
  console.log(`  Members: ${members.length}`);
  console.log(`  Quorum: ${cfg.quorum_percent}%`);
  console.log(`  Voting period: ${cfg.voting_period_blocks} blocks`);

  // Save state
  const state: ParliamentState = {
    parliament_contract: instantiateResult.contractAddress,
    mps,
    current_proposal_id: null,
    current_proposal_index: 0,
  };
  saveState(state);

  console.log(`\n╔══════════════════════════════════════════════════════════════╗`);
  console.log(`║  ✅ PARLIAMENT ESTABLISHED                                   ║`);
  console.log(`╠══════════════════════════════════════════════════════════════╣`);
  console.log(`║  Contract: ${instantiateResult.contractAddress}`);
  console.log(`║  Members:  7 MPs, 100% quorum (all must vote)              ║`);
  console.log(`║  Chain:    ${config.chainId.padEnd(49)}║`);
  console.log(`╠══════════════════════════════════════════════════════════════╣`);
  for (let i = 0; i < MP_PROFILES.length; i++) {
    const p = MP_PROFILES[i];
    const seat = `  Seat ${i + 1}: ${p.name.padEnd(22)} (${p.role})`;
    console.log(`║${seat.padEnd(62)}║`);
  }
  console.log(`╚══════════════════════════════════════════════════════════════╝`);
  console.log(`\nState saved to: ${STATE_FILE}`);
  console.log(`\nNext: npx tsx src/parliament-demo.ts propose`);
}

// ── PROPOSE: Submit the next sample proposal ──
async function propose() {
  const state = loadState();
  if (!state) {
    console.error("No parliament state found. Run 'setup' first.");
    process.exit(1);
  }

  const proposalIndex = state.current_proposal_index % SAMPLE_PROPOSALS.length;
  const proposal = SAMPLE_PROPOSALS[proposalIndex];

  // Speaker (MP 0 = The Builder) submits
  const speaker = state.mps[0];
  const profile = MP_PROFILES[speaker.profile_index];
  const { client, sender } = await getClientFor(speaker.mnemonic);

  console.log(`\n📜 Proposal submitted by ${speaker.name} (${profile.role})`);
  console.log(`   "${proposal.title}"\n`);

  const msg = {
    create_proposal: {
      kind: {
        free_text: {
          title: proposal.title,
          description: proposal.description,
        },
      },
    },
  };

  const result = await client.execute(
    sender,
    state.parliament_contract,
    msg,
    "auto",
    `Parliament proposal: ${proposal.title}`
  );
  console.log(`   TX: ${result.transactionHash}`);

  // Get proposal ID — find the highest-ID proposal (newest)
  const proposals = await client.queryContractSmart(state.parliament_contract, {
    list_proposals: { limit: 50 },
  });
  // Sort by ID descending, pick the first open one (or highest overall)
  const sorted = [...proposals].sort((a: any, b: any) => b.id - a.id);
  const latest = sorted.find((p: any) => p.status === "open") || sorted[0];
  console.log(`   Proposal ID: ${latest.id}`);
  console.log(`   Status: ${JSON.stringify(latest.status)}`);
  console.log(`   Voting deadline block: ${latest.voting_deadline_block}`);

  state.current_proposal_id = latest.id;
  state.current_proposal_index = proposalIndex + 1;
  saveState(state);

  console.log(`\n   ─── Proposal Text ───`);
  console.log(`   ${proposal.description.split("\n").join("\n   ")}`);
  console.log(`\nNext: npx tsx src/parliament-demo.ts debate`);
}

// ── DEBATE: Show each MP's reasoning ──
async function debate() {
  const state = loadState();
  if (!state || state.current_proposal_id === null) {
    console.error("No active proposal. Run 'propose' first.");
    process.exit(1);
  }

  const proposalIndex = (state.current_proposal_index - 1) % SAMPLE_PROPOSALS.length;
  const proposal = SAMPLE_PROPOSALS[proposalIndex];

  console.log(`\n🏛️  PARLIAMENTARY DEBATE`);
  console.log(`   Proposal: "${proposal.title}"`);
  console.log(`   ${"─".repeat(60)}\n`);

  for (let i = 0; i < state.mps.length; i++) {
    const mp = state.mps[i];
    const profile = MP_PROFILES[mp.profile_index];
    const { vote, reasoning } = evaluateVote(profile, proposal.title, proposal.description);

    const voteColor =
      vote === "yes" ? "✅" : vote === "no" ? "❌" : "⚪";

    console.log(`   ${voteColor} ${profile.name} (${profile.role})`);
    console.log(`      Stance: "${profile.stance}"`);
    console.log(`      Evaluation: ${reasoning}`);
    console.log(`      Intended vote: ${vote.toUpperCase()}`);
    console.log();
  }

  // Tally preview
  let yes = 0, no = 0, abstain = 0;
  for (const mp of state.mps) {
    const profile = MP_PROFILES[mp.profile_index];
    const { vote } = evaluateVote(profile, proposal.title, proposal.description);
    if (vote === "yes") yes += profile.weight;
    else if (vote === "no") no += profile.weight;
    else abstain += profile.weight;
  }
  const total = yes + no + abstain;
  console.log(`   ─── Pre-Vote Forecast ───`);
  console.log(`   YES:     ${yes} weight (${((yes / total) * 100).toFixed(1)}%)`);
  console.log(`   NO:      ${no} weight (${((no / total) * 100).toFixed(1)}%)`);
  console.log(`   ABSTAIN: ${abstain} weight (${((abstain / total) * 100).toFixed(1)}%)`);
  console.log(`   Quorum needed: 51%`);
  console.log(`   Forecast: ${yes > no ? "LIKELY PASS ✅" : no > yes ? "LIKELY FAIL ❌" : "TOO CLOSE TO CALL ⚖️"}`);
  console.log(`\nNext: npx tsx src/parliament-demo.ts vote`);
}

// ── VOTE: Each MP casts their vote on-chain ──
async function vote() {
  const state = loadState();
  if (!state || state.current_proposal_id === null) {
    console.error("No active proposal. Run 'propose' first.");
    process.exit(1);
  }

  const proposalIndex = (state.current_proposal_index - 1) % SAMPLE_PROPOSALS.length;
  const proposal = SAMPLE_PROPOSALS[proposalIndex];

  console.log(`\n🗳️  VOTING ON: "${proposal.title}"`);
  console.log(`   Proposal ID: ${state.current_proposal_id}\n`);

  for (let i = 0; i < state.mps.length; i++) {
    const mp = state.mps[i];
    const profile = MP_PROFILES[mp.profile_index];
    const { vote: voteChoice, reasoning } = evaluateVote(
      profile,
      proposal.title,
      proposal.description
    );

    const { client, sender } = await getClientFor(mp.mnemonic);

    const msg = {
      cast_vote: {
        proposal_id: state.current_proposal_id,
        vote: voteChoice,
      },
    };

    try {
      const result = await client.execute(
        sender,
        state.parliament_contract,
        msg,
        "auto",
        `${mp.name} votes ${voteChoice} on proposal ${state.current_proposal_id}`
      );

      const icon = voteChoice === "yes" ? "✅" : voteChoice === "no" ? "❌" : "⚪";
      console.log(
        `   ${icon} ${profile.name.padEnd(22)} → ${voteChoice.toUpperCase().padEnd(7)} TX: ${result.transactionHash.slice(0, 16)}...`
      );
    } catch (err: any) {
      console.log(
        `   ⚠️  ${profile.name.padEnd(22)} → FAILED: ${err.message?.slice(0, 60)}`
      );
    }
  }

  // Query final state
  const { client } = await getClientFor(state.mps[0].mnemonic);
  const prop = await client.queryContractSmart(state.parliament_contract, {
    get_proposal: { proposal_id: state.current_proposal_id },
  });

  console.log(`\n   ─── On-Chain Result ───`);
  console.log(`   Status: ${JSON.stringify(prop.status)}`);
  console.log(`   Yes weight: ${prop.yes_weight}`);
  console.log(`   No weight: ${prop.no_weight}`);
  console.log(`   Abstain weight: ${prop.abstain_weight || 0}`);
  console.log(`   Total voted: ${prop.total_voted_weight}`);

  const passed = prop.status === "passed" || prop.status === "Passed";
  console.log(`\n   ${passed ? "✅ PROPOSAL PASSED" : "❌ PROPOSAL DID NOT PASS"}`);

  if (passed) {
    console.log(`\nNext: npx tsx src/parliament-demo.ts execute`);
  } else {
    console.log(`\nNext: npx tsx src/parliament-demo.ts propose  (submit next proposal)`);
  }
}

// ── EXECUTE: Execute a passed proposal ──
async function execute() {
  const state = loadState();
  if (!state || state.current_proposal_id === null) {
    console.error("No active proposal. Run 'propose' first.");
    process.exit(1);
  }

  const speaker = state.mps[0];
  const { client, sender } = await getClientFor(speaker.mnemonic);

  console.log(`\n⚡ Executing proposal ${state.current_proposal_id}...`);

  const msg = { execute_proposal: { proposal_id: state.current_proposal_id } };
  try {
    const result = await client.execute(
      sender,
      state.parliament_contract,
      msg,
      "auto",
      `Execute parliament proposal ${state.current_proposal_id}`
    );
    console.log(`   TX: ${result.transactionHash}`);
    console.log(`   ✅ Proposal executed successfully`);
  } catch (err: any) {
    console.log(`   ⚠️  ${err.message?.slice(0, 100)}`);
  }

  state.current_proposal_id = null;
  saveState(state);

  console.log(`\nNext: npx tsx src/parliament-demo.ts propose  (submit next proposal)`);
}

// ── TALLY: Show current vote state ──
async function tally() {
  const state = loadState();
  if (!state || state.current_proposal_id === null) {
    console.error("No active proposal.");
    process.exit(1);
  }

  const { client } = await getClientFor(state.mps[0].mnemonic);
  const prop = await client.queryContractSmart(state.parliament_contract, {
    get_proposal: { proposal_id: state.current_proposal_id },
  });

  console.log(`\n📊 TALLY — Proposal #${state.current_proposal_id}`);
  console.log(`   Title: ${prop.kind?.text?.title || "unknown"}`);
  console.log(`   Status: ${JSON.stringify(prop.status)}`);
  console.log(`   Yes: ${prop.yes_weight} | No: ${prop.no_weight} | Abstain: ${prop.abstain_weight || 0}`);
  console.log(`   Total voted: ${prop.total_voted_weight}`);
}

// ── STATUS: Full parliament overview ──
async function status() {
  const state = loadState();
  if (!state) {
    console.error("No parliament state found. Run 'setup' first.");
    process.exit(1);
  }

  const { client } = await getClientFor(state.mps[0].mnemonic);
  const cfg = await client.queryContractSmart(state.parliament_contract, {
    get_config: {},
  });

  console.log(`\n🏛️  PARLIAMENT STATUS`);
  console.log(`   Contract: ${state.parliament_contract}`);
  console.log(`   Chain: ${config.chainId}`);
  console.log(`   Name: ${cfg.name}`);
  console.log(`   Quorum: ${cfg.quorum_percent}%`);
  console.log(`   Voting period: ${cfg.voting_period_blocks} blocks\n`);

  console.log(`   ─── Seats ───`);
  for (let i = 0; i < state.mps.length; i++) {
    const mp = state.mps[i];
    const profile = MP_PROFILES[mp.profile_index];
    const balance = await client.getBalance(mp.address, config.denom);
    const junox = (parseInt(balance.amount) / 1_000_000).toFixed(1);
    console.log(
      `   Seat ${i + 1}: ${profile.name.padEnd(22)} w=${profile.weight}  ${junox} JUNOX  ${mp.address.slice(0, 20)}...`
    );
  }

  // List proposals
  try {
    const proposals = await client.queryContractSmart(state.parliament_contract, {
      list_proposals: { limit: 20 },
    });
    if (proposals.length > 0) {
      console.log(`\n   ─── Proposals ───`);
      for (const p of proposals) {
        const title = p.kind?.text?.title || p.kind?.code_upgrade?.title || "unknown";
        console.log(`   #${p.id}: ${title} — ${JSON.stringify(p.status)}`);
      }
    }
  } catch {
    console.log(`\n   No proposals yet.`);
  }

  console.log();
}

// ════════════════════════════════════════════════════════════════
// CLI
// ════════════════════════════════════════════════════════════════

const command = process.argv[2];
switch (command) {
  case "setup":
    setup().catch(console.error);
    break;
  case "propose":
    propose().catch(console.error);
    break;
  case "debate":
    debate().catch(console.error);
    break;
  case "vote":
    vote().catch(console.error);
    break;
  case "tally":
    tally().catch(console.error);
    break;
  case "execute":
    execute().catch(console.error);
    break;
  case "status":
    status().catch(console.error);
    break;
  default:
    console.log(`
🏛️  JunoClaw Agentic Parliament — Demo

Usage: npx tsx src/parliament-demo.ts <command>

Commands:
  setup     Generate 7 MP wallets, fund them, instantiate Parliament on ${config.chainId}
  propose   Submit the next policy proposal (from sample set)
  debate    Show each MP's reasoning before the vote
  vote      All 7 MPs cast their votes on-chain
  tally     Show current vote count
  execute   Execute a passed proposal
  status    Show full parliament state

The 7 Members of Parliament:
  1. The Builder          — Infrastructure Chair (default: YES)
  2. The Fiscal Hawk      — Treasury Oversight (default: NO)
  3. The Populist         — Community Representative (default: YES)
  4. The Technocrat       — Verification Standards (default: ABSTAIN)
  5. The Diplomat         — Cross-Chain Relations (default: YES)
  6. The Environmentalist — Sustainability Advocate (default: ABSTAIN)
  7. The Contrarian       — Devil's Advocate (default: NO)

Each MP evaluates proposals against their policy stance and votes deterministically.
All votes are real on-chain transactions on Juno testnet.
    `);
}
