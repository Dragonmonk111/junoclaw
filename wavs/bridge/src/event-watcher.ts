/**
 * Event watcher — polls Juno blocks for WAVS trigger events from our contract.
 *
 * Watches for three event types emitted by agent-company:
 *   - wasm-outcome_create  → OutcomeVerify task
 *   - wasm-wavs_push       → DataVerify task
 *   - wasm-sortition_request → DrandRandomness task
 *
 * Uses CosmJS to search recent transactions by contract events.
 */

import { CosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { config } from "./config.js";

// The event types we watch for (must match contract.rs emit events)
const TRIGGER_EVENTS = [
  "wasm-outcome_create",
  "wasm-wavs_push",
  "wasm-sortition_request",
] as const;

export type TriggerEventType = (typeof TRIGGER_EVENTS)[number];

export interface TriggerEvent {
  eventType: TriggerEventType;
  proposalId: number;
  attributes: Record<string, string>;
  txHash: string;
  blockHeight: number;
}

/**
 * Scan a range of blocks for trigger events from our contract.
 * Uses the Tendermint tx_search RPC to find transactions with matching events.
 */
export async function scanForTriggerEvents(
  client: CosmWasmClient,
  fromHeight: number,
  toHeight: number
): Promise<TriggerEvent[]> {
  const events: TriggerEvent[] = [];
  const contractAddr = config.agentCompanyContract;

  for (const eventType of TRIGGER_EVENTS) {
    try {
      // Search for txs that emitted this event type from our contract
      const results = await (client as any).forceGetTmClient().txSearchAll({
        query: `${eventType}._contract_address='${contractAddr}' AND tx.height>=${fromHeight} AND tx.height<=${toHeight}`,
      });

      for (const tx of results.txs) {
        // Parse events from the tx result
        const parsedEvents = parseTxEvents(tx, eventType);
        events.push(...parsedEvents);
      }
    } catch (err: any) {
      // Some RPC endpoints don't support all search queries — skip gracefully
      if (!err.message?.includes("parse error")) {
        console.warn(
          `[watcher] Error scanning ${eventType}: ${err.message}`
        );
      }
    }
  }

  return events;
}

/**
 * Alternative: poll the contract directly for executed proposals that
 * haven't been attested yet. More reliable than tx_search on public RPCs.
 */
export async function findUnattestedProposals(
  client: CosmWasmClient
): Promise<TriggerEvent[]> {
  const contractAddr = config.agentCompanyContract;
  const events: TriggerEvent[] = [];

  try {
    // List recent proposals
    const proposals = await client.queryContractSmart(contractAddr, {
      list_proposals: { start_after: null, limit: 50 },
    });

    const proposalList: any[] = Array.isArray(proposals)
      ? proposals
      : (typeof proposals === "object" &&
          proposals !== null &&
          "proposals" in proposals &&
          Array.isArray((proposals as { proposals?: unknown }).proposals))
        ? (proposals as { proposals: any[] }).proposals
        : [];

    for (const p of proposalList) {
      // Only care about executed proposals
      if (!p.executed) continue;

      // Check if already attested
      try {
        const att = await client.queryContractSmart(contractAddr, {
          get_attestation: { proposal_id: p.id },
        });
        if (att) continue; // Already attested, skip
      } catch {
        // No attestation found — this is a candidate
      }

      // Determine event type from proposal kind
      const kind = p.kind;
      if (kind?.outcome_create) {
        events.push({
          eventType: "wasm-outcome_create",
          proposalId: p.id,
          attributes: {
            market_id: String(p.id),
            question: kind.outcome_create.question || "",
            resolution_criteria:
              kind.outcome_create.resolution_criteria || "",
            deadline_block: String(
              kind.outcome_create.deadline_block || 0
            ),
          },
          txHash: "queried",
          blockHeight: p.created_at_block || 0,
        });
      } else if (kind?.wavs_push) {
        events.push({
          eventType: "wasm-wavs_push",
          proposalId: p.id,
          attributes: {
            task_id: String(p.id),
            task_description: kind.wavs_push.task_description || "",
            data_sources: (kind.wavs_push.data_sources || []).join(","),
          },
          txHash: "queried",
          blockHeight: p.created_at_block || 0,
        });
      } else if (kind?.sortition_request) {
        events.push({
          eventType: "wasm-sortition_request",
          proposalId: p.id,
          attributes: {
            job_id: kind.sortition_request.job_id || `sort-${p.id}`,
            count: String(kind.sortition_request.count || 1),
          },
          txHash: "queried",
          blockHeight: p.created_at_block || 0,
        });
      }
    }
  } catch (err: any) {
    console.warn(`[watcher] Error querying proposals: ${err.message}`);
  }

  return events;
}

/**
 * Parse tx events from a Tendermint tx result.
 */
function parseTxEvents(tx: any, targetEventType: string): TriggerEvent[] {
  const events: TriggerEvent[] = [];

  if (!tx.result?.events) return events;

  for (const ev of tx.result.events) {
    if (ev.type !== targetEventType) continue;

    const attrs: Record<string, string> = {};
    let proposalId = 0;

    for (const attr of ev.attributes || []) {
      const key =
        typeof attr.key === "string"
          ? attr.key
          : Buffer.from(attr.key, "base64").toString();
      const value =
        typeof attr.value === "string"
          ? attr.value
          : Buffer.from(attr.value, "base64").toString();
      attrs[key] = value;

      if (key === "proposal_id" || key === "market_id") {
        proposalId = parseInt(value, 10) || 0;
      }
    }

    events.push({
      eventType: targetEventType as TriggerEventType,
      proposalId,
      attributes: attrs,
      txHash: Buffer.from(tx.hash).toString("hex").toUpperCase(),
      blockHeight: tx.height,
    });
  }

  return events;
}
