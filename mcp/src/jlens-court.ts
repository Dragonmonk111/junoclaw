#!/usr/bin/env tsx
/**
 * J-Lens thought-court driver — junoclaw-g1.
 *
 * Deploys and drives the optimistic-verification court: bonded
 * attestation anchors, bonded challenges, committee verdicts,
 * slashing. Verifier seats are stake-bound; the DAO is admin.
 *
 * Usage:
 *   cd mcp && npx tsx src/jlens-court.ts --wasm <thought_court.wasm>
 *     [--code-id N | --contract <addr>]
 *     (--deploy | --anchor <id> | --challenge <id> |
 *      --verdict <id> --verdict-kind clean|fraud --evidence-hash <hex> |
 *      --finalize <id> | --resolve-timeout <id> |
 *      --force-resolve <id> --verdict-kind clean|fraud (admin) |
 *      --register-verifier [--wallet <id>] |
 *      --send <juno1...> --amount <ujclaw> [--wallet <id>] |
 *      --query <json> | --query-attestation <id> | --query-dispute <id>)
 *
 * Bonds are paid in ujclaw: --bond overrides the sent amount.
 */

import { readFileSync } from "fs";
import { resolve } from "path";

import { queryContractState } from "./tools/chain-query.js";
import { executeContract, instantiateContract, sendTokens, uploadWasm } from "./tools/tx-builder.js";
import { getDefaultWalletStore } from "./wallet/store.js";

const CHAIN_ID = "junoclaw-g1";
const DENOM = "ujclaw";

function arg(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

function has(flag: string): boolean {
  return process.argv.includes(flag);
}

async function walletId(): Promise<string> {
  const id = arg("--wallet") ?? "jlens-ops";
  const w = (await getDefaultWalletStore().list()).find((e) => e.id === id);
  if (!w) throw new Error(`wallet ${id} not enrolled`);
  return id;
}

function funds(amount: string | undefined, fallback: string) {
  return [{ denom: DENOM, amount: amount ?? fallback }];
}

async function main() {
  const wid = await walletId();

  // ── deploy path ──────────────────────────────────────────────────
  if (has("--deploy")) {
    let codeId = arg("--code-id") ? Number(arg("--code-id")) : undefined;
    if (!codeId) {
      const up = await uploadWasm(
        CHAIN_ID,
        wid,
        resolve(arg("--wasm") ?? ""),
        "jlens thought-court store"
      );
      codeId = up.codeId;
      console.log(`stored code_id=${codeId} tx=${up.txHash}`);
    }
    const inst = await instantiateContract(
      CHAIN_ID,
      wid,
      codeId,
      {
        // DAO address takes over via UpdateConfig{admin} post-governance.
        admin: undefined,
        challenge_window_secs: 86_400,
        verdict_window_secs: 43_200,
        committee_size: 3,
        quorum_bp: 6_700,
        challenger_reward_bp: 5_000,
        operator_reward_bp: 5_000,
        verifier_slash_bp: 2_000,
      },
      "jlens-thought-court",
      undefined,
      undefined,
      "jlens thought-court instantiate"
    );
    console.log(`instantiated ${inst.contractAddress} tx=${inst.txHash}`);
    return;
  }

  // ── bank send (funding verifier seats; no contract needed) ────────
  if (arg("--send")) {
    const amt = arg("--amount");
    if (!amt) throw new Error("--amount <ujclaw> required with --send");
    const r = await sendTokens(CHAIN_ID, wid, arg("--send")!, amt, DENOM, "jlens court fund");
    console.log(`tx=${r.txHash} height=${r.height}`);
    return;
  }

  const contract = arg("--contract");
  if (!contract) throw new Error("--contract required (or --deploy)");

  // ── queries (no wallet spend) ────────────────────────────────────
  if (arg("--query")) {
    console.log(JSON.stringify(await queryContractState(CHAIN_ID, contract, JSON.parse(arg("--query")!)), null, 1));
    return;
  }
  if (arg("--query-attestation")) {
    console.log(JSON.stringify(await queryContractState(CHAIN_ID, contract, {
      get_attestation: { attestation_id: arg("--query-attestation") },
    }), null, 1));
    return;
  }
  if (arg("--query-dispute")) {
    console.log(JSON.stringify(await queryContractState(CHAIN_ID, contract, {
      get_dispute: { attestation_id: arg("--query-dispute") },
    }), null, 1));
    return;
  }

  // ── executions ───────────────────────────────────────────────────
  const exec = async (msg: Record<string, unknown>, f?: { denom: string; amount: string }[], memo = "jlens court") => {
    const r = await executeContract(CHAIN_ID, wid, contract, msg, f, memo);
    console.log(`tx=${r.txHash} height=${r.height}`);
    return r;
  };

  if (arg("--anchor")) {
    const root = arg("--activation-root");
    if (!root) throw new Error("--activation-root required");
    await exec(
      { anchor_attestation: { id: arg("--anchor"), activation_root: root } },
      funds(arg("--bond"), "1000000"),
      "jlens anchor attestation"
    );
    return;
  }

  if (arg("--challenge")) {
    await exec(
      { open_challenge: { attestation_id: arg("--challenge") } },
      funds(arg("--bond"), "500000"),
      "jlens open challenge"
    );
    return;
  }

  if (arg("--verdict")) {
    const kind = arg("--verdict-kind");
    const ev = arg("--evidence-hash");
    if (!kind || !ev) throw new Error("--verdict-kind clean|fraud and --evidence-hash required");
    await exec(
      {
        submit_verdict: {
          attestation_id: arg("--verdict"),
          verdict: kind === "fraud" ? "fraud" : "clean",
          evidence_hash: ev,
        },
      },
      undefined,
      "jlens submit verdict"
    );
    return;
  }

  if (arg("--finalize")) {
    await exec({ finalize_attestation: { attestation_id: arg("--finalize") } });
    return;
  }

  if (arg("--resolve-timeout")) {
    await exec({ resolve_timeout: { attestation_id: arg("--resolve-timeout") } });
    return;
  }

  if (arg("--force-resolve")) {
    const kind = arg("--verdict-kind");
    if (!kind) throw new Error("--verdict-kind clean|fraud required with --force-resolve");
    await exec(
      {
        force_resolve: {
          attestation_id: arg("--force-resolve"),
          verdict: kind === "fraud" ? "fraud" : "clean",
        },
      },
      undefined,
      "jlens force resolve"
    );
    return;
  }

  if (has("--register-verifier")) {
    await exec({ register_verifier: {} }, funds(arg("--bond"), "2000000"), "jlens verifier seat");
    return;
  }

  if (has("--remove-verifier")) {
    await exec({ remove_verifier: {} });
    return;
  }

  if (has("--update-config")) {
    const u: Record<string, unknown> = {};
    for (const k of [
      "challenge_window_secs",
      "verdict_window_secs",
      "committee_size",
      "quorum_bp",
      "challenger_reward_bp",
      "operator_reward_bp",
      "verifier_slash_bp",
    ]) {
      const v = arg(`--${k}`);
      if (v !== undefined) u[k] = Number(v);
    }
    if (arg("--admin")) u.admin = arg("--admin");
    for (const k of ["min_operator_bond", "min_challenger_bond", "min_verifier_bond"]) {
      const v = arg(`--${k}`);
      if (v !== undefined) u[k] = v; // Uint128 serializes as string
    }
    await exec({ update_config: u }, undefined, "jlens court update-config");
    return;
  }

  throw new Error("no action given — see usage in header");
}

main().catch((err: unknown) => {
  console.error(`error: ${err instanceof Error ? err.message : err}`);
  process.exit(1);
});
