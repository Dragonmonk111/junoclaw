/**
 * Smoke test for the second-approval transaction staging lever
 * (Rattadan's advice, 2026-07-21). See `pending-tx.ts` for design notes.
 *
 * Run:
 *   npm run pending-tx-test    (from mcp/)
 *   tsx src/wallet/pending-tx-test.ts
 *
 * Coverage:
 *   - isTxConfirmationRequired() defaults true when env unset
 *   - isTxConfirmationRequired() honors "0" / "false" opt-out
 *   - stage() returns a preview without executing the closure
 *   - confirm() executes exactly once and returns the result
 *   - confirm() on an unknown id throws
 *   - confirm() is single-use: second confirm on same id throws
 *   - expired entries are rejected (TTL enforcement)
 *   - two stage() calls for the same tool get distinct ids and
 *     independent execution
 */

import { getPendingTxStore, isTxConfirmationRequired } from "./pending-tx.js";

let passed = 0;
let failed = 0;
const failures: string[] = [];

function test(name: string, fn: () => Promise<void>): Promise<void> {
  return fn().then(
    () => {
      console.log(`  PASS  ${name}`);
      passed++;
    },
    (e: Error) => {
      console.log(`  FAIL  ${name}\n        ${e.message}`);
      failed++;
      failures.push(name);
    }
  );
}

async function expectThrow(fn: () => Promise<unknown>, description: string): Promise<void> {
  let threw = false;
  try {
    await fn();
  } catch {
    threw = true;
  }
  if (!threw) throw new Error(`expected throw for "${description}", but none occurred`);
}

console.log("\n━━━ second-approval tx staging (pending-tx.ts) ━━━\n");

async function run() {
  const savedEnv = process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION;

  await test("isTxConfirmationRequired() defaults true when env unset", async () => {
    delete process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION;
    if (isTxConfirmationRequired() !== true) {
      throw new Error("expected true by default (fail-safe)");
    }
  });

  await test('isTxConfirmationRequired() returns false for "0"', async () => {
    process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION = "0";
    if (isTxConfirmationRequired() !== false) throw new Error("expected false for '0'");
  });

  await test('isTxConfirmationRequired() returns false for "false" (case-insensitive)', async () => {
    process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION = "FALSE";
    if (isTxConfirmationRequired() !== false) throw new Error("expected false for 'FALSE'");
  });

  await test("isTxConfirmationRequired() treats other values as still required", async () => {
    process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION = "yes-please";
    if (isTxConfirmationRequired() !== true) throw new Error("expected true for unrecognized value");
  });

  if (savedEnv === undefined) delete process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION;
  else process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION = savedEnv;

  await test("stage() returns a preview without invoking the execute closure", async () => {
    const store = getPendingTxStore();
    let invoked = false;
    const preview = store.stage("send_tokens", { recipient: "juno1abc" }, async () => {
      invoked = true;
      return { txhash: "DEADBEEF" };
    });
    if (invoked) throw new Error("execute closure ran during stage(), expected deferred");
    if (preview.status !== "pending_confirmation") throw new Error(`bad status: ${preview.status}`);
    if (preview.tool !== "send_tokens") throw new Error(`bad tool: ${preview.tool}`);
    if (!preview.confirmation_id.startsWith("pending_")) {
      throw new Error(`bad confirmation_id shape: ${preview.confirmation_id}`);
    }
  });

  await test("confirm() executes the staged closure exactly once and returns its result", async () => {
    const store = getPendingTxStore();
    let calls = 0;
    const preview = store.stage("delegate_tokens", { validator_address: "junovaloper1x" }, async () => {
      calls++;
      return { txhash: "ABC123" };
    });
    const result = await store.confirm(preview.confirmation_id);
    if (calls !== 1) throw new Error(`expected 1 call, got ${calls}`);
    if ((result.result as { txhash: string }).txhash !== "ABC123") {
      throw new Error(`unexpected result: ${JSON.stringify(result)}`);
    }
    if (result.tool !== "delegate_tokens") throw new Error(`bad tool echo: ${result.tool}`);
  });

  await test("confirm() on an unknown confirmation_id throws", async () => {
    const store = getPendingTxStore();
    await expectThrow(() => store.confirm("pending_does_not_exist"), "unknown confirmation_id");
  });

  await test("confirm() is single-use: a second confirm on the same id throws", async () => {
    const store = getPendingTxStore();
    const preview = store.stage("withdraw_rewards", {}, async () => ({ txhash: "ONCE" }));
    await store.confirm(preview.confirmation_id);
    await expectThrow(() => store.confirm(preview.confirmation_id), "replayed confirmation_id");
  });

  await test("expired entries (TTL) are rejected by confirm()", async () => {
    const store = getPendingTxStore();
    const preview = store.stage("ibc_transfer", {}, async () => ({ txhash: "EXPIRED" }), 1 /* ms */);
    await new Promise((r) => setTimeout(r, 20));
    await expectThrow(() => store.confirm(preview.confirmation_id), "expired confirmation_id");
  });

  await test("two stage() calls for the same tool get distinct ids and independent execution", async () => {
    const store = getPendingTxStore();
    const counts = { a: 0, b: 0 };
    const p1 = store.stage("send_tokens", { recipient: "juno1a" }, async () => { counts.a++; return { txhash: "A" }; });
    const p2 = store.stage("send_tokens", { recipient: "juno1b" }, async () => { counts.b++; return { txhash: "B" }; });
    if (p1.confirmation_id === p2.confirmation_id) throw new Error("collision in confirmation_id");
    await store.confirm(p1.confirmation_id);
    if (counts.a !== 1 || counts.b !== 0) {
      throw new Error(`confirming p1 should only run closure A: a=${counts.a} b=${counts.b}`);
    }
    await store.confirm(p2.confirmation_id);
    const [finalA, finalB]: number[] = [counts.a, counts.b];
    if (finalA !== 1 || finalB !== 1) {
      throw new Error(`confirming p2 should only run closure B: a=${finalA} b=${finalB}`);
    }
  });

  console.log(`\n${passed} passed, ${failed} failed\n`);

  if (failed > 0) {
    console.log("Failed tests:");
    for (const name of failures) console.log(`  - ${name}`);
    process.exit(1);
  }

  process.exit(0);
}

run().catch((e) => {
  console.error("test runner crashed:", e);
  process.exit(1);
});
