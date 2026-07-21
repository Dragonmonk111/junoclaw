/**
 * PendingTxStore — second-approval staging for fund-moving transactions.
 *
 * Rattadan's advice (2026-07-21): an AI-driven signer should not be able to
 * move funds in a single tool call. This module implements the lever:
 * fund-moving tools (see `index.ts` — `FUNDS_MOVING_TOOLS`) stage their
 * intent here instead of broadcasting immediately, return a human-readable
 * preview + a one-time `confirmation_id`, and only actually sign+broadcast
 * when a second, separate `confirm_transaction` tool call redeems that id.
 *
 * Design goals (matches the project's existing security posture — see
 * `SigningPausedError` and the `JUNOCLAW_ALLOWED_MSG_TYPES` gate in
 * `tools/tx-builder.ts`):
 *   - Fail SAFE by default: confirmation is REQUIRED unless the operator
 *     explicitly opts out via `JUNOCLAW_REQUIRE_TX_CONFIRMATION=0`. This is
 *     the opposite default of the msg-type allowlist (which fails closed
 *     toward "disabled") because here the safer default is "add friction",
 *     not "disable the capability".
 *   - Single-use: redeeming a confirmation_id consumes it immediately,
 *     before the underlying broadcast even starts, so a replayed
 *     confirm_transaction call cannot double-spend.
 *   - Short TTL (default 5 minutes): an abandoned pending intent cannot be
 *     resurrected by a later, unrelated conversation turn.
 *   - In-memory only, per-process: a pending intent never touches disk and
 *     is not part of any wallet's encrypted state. Restarting the MCP
 *     process discards all pending intents — there is nothing durable to
 *     leak or to accidentally replay across a restart.
 *   - The *execution closure* — not just a serialized copy of the
 *     parameters — is what gets staged, so `confirm_transaction` cannot be
 *     used to substitute different parameters than what the human saw in
 *     the preview.
 */

const DEFAULT_TTL_MS = 5 * 60 * 1000; // 5 minutes

export interface PendingTx {
  confirmationId: string;
  toolName: string;
  summary: Record<string, unknown>;
  createdAt: number;
  expiresAt: number;
  execute: () => Promise<unknown>;
}

export interface StagedTxPreview {
  confirmation_id: string;
  status: "pending_confirmation";
  tool: string;
  summary: Record<string, unknown>;
  expires_at: string;
  instructions: string;
}

class PendingTxStore {
  private pending = new Map<string, PendingTx>();

  private sweep(): void {
    const now = Date.now();
    for (const [id, tx] of this.pending) {
      if (tx.expiresAt <= now) this.pending.delete(id);
    }
  }

  stage(
    toolName: string,
    summary: Record<string, unknown>,
    execute: () => Promise<unknown>,
    ttlMs: number = DEFAULT_TTL_MS
  ): StagedTxPreview {
    this.sweep();
    const confirmationId = `pending_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
    const now = Date.now();
    const expiresAt = now + ttlMs;
    this.pending.set(confirmationId, {
      confirmationId,
      toolName,
      summary,
      createdAt: now,
      expiresAt,
      execute,
    });
    return {
      confirmation_id: confirmationId,
      status: "pending_confirmation",
      tool: toolName,
      summary,
      expires_at: new Date(expiresAt).toISOString(),
      instructions:
        "This transaction has NOT been signed or broadcast. Review the summary above with " +
        "the user. To proceed, call `confirm_transaction` with this confirmation_id within " +
        `${Math.round(ttlMs / 60000)} minute(s). Calling the original tool again will stage a new, ` +
        "separate pending transaction rather than confirming this one.",
    };
  }

  async confirm(confirmationId: string): Promise<{ tool: string; summary: Record<string, unknown>; result: unknown }> {
    this.sweep();
    const tx = this.pending.get(confirmationId);
    if (!tx) {
      throw new Error(
        `No pending transaction found for confirmation_id "${confirmationId}". ` +
          "It may have already been confirmed, expired (5 minute TTL), or never existed."
      );
    }
    // Single-use: remove before executing so a concurrent or replayed
    // confirm cannot redeem the same intent twice.
    this.pending.delete(confirmationId);
    const result = await tx.execute();
    return { tool: tx.toolName, summary: tx.summary, result };
  }

  /** Test/ops helper — not used by any tool handler. */
  peek(confirmationId: string): PendingTx | undefined {
    this.sweep();
    return this.pending.get(confirmationId);
  }

  size(): number {
    this.sweep();
    return this.pending.size;
  }
}

let store: PendingTxStore | undefined;

export function getPendingTxStore(): PendingTxStore {
  if (!store) store = new PendingTxStore();
  return store;
}

/**
 * Fail-safe default: confirmation is REQUIRED unless the operator
 * explicitly opts out. Any value other than "0" or "false" (case
 * insensitive) is treated as "still required" — matching the fail-closed
 * parsing style used for `JUNOCLAW_SIGNING_PAUSED` elsewhere in this file
 * tree, just inverted (there, unset means OFF; here, unset means ON).
 */
export function isTxConfirmationRequired(): boolean {
  const raw = process.env.JUNOCLAW_REQUIRE_TX_CONFIRMATION;
  if (raw === undefined) return true;
  const normalized = raw.trim().toLowerCase();
  return !(normalized === "0" || normalized === "false");
}
