/**
 * audit-post.mjs — Post Akash deployment audit events to Moultbook.
 *
 * Each Akash transaction (create deployment, create lease, close deployment)
 * is recorded as a Moultbook `Post` entry with:
 *   - content_type: "application/json+jlens-akash-audit"
 *   - commitment: SHA-256 of the audit JSON
 *   - refs: [tx_hash]
 *
 * This creates an immutable, on-chain audit trail of every autonomous
 * Akash transaction signed by the JunoClaw agent, visible to all DAO members.
 *
 * Used by auto-deploy.mjs.
 */

import { createHash } from "crypto";
import { join, dirname } from "path";
import { fileURLToPath, pathToFileURL } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MCP_DIST = join(__dirname, "..", "..", "mcp", "dist");

// Windows ESM dynamic import() requires file:// URLs, not raw paths.
function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href);
}

/**
 * Post an audit entry to Moultbook.
 *
 * @param {object} opts
 * @param {string} opts.moultbookAddr — Moultbook contract address on Juno
 * @param {string} opts.junoWalletId — WalletStore ID for Juno signing wallet
 * @param {Buffer} opts.commitment — 32-byte SHA-256 commitment
 * @param {string} opts.contentType — MIME type for the entry
 * @param {string[]} opts.refs — References (e.g. tx hashes)
 * @returns {Promise<string>} — tx hash of the Moultbook post
 */
export async function postToMoultbook(opts) {
  const { moultbookAddr, junoWalletId, commitment, contentType, refs } = opts;

  if (!moultbookAddr || !junoWalletId) {
    console.warn("[audit] Moultbook address or Juno wallet ID not set — skipping audit post");
    return null;
  }

  if (commitment.length !== 32) {
    throw new Error(`Commitment must be 32 bytes, got ${commitment.length}`);
  }

  // Load WalletStore and sign for Juno
  const store = await distImport("wallet", "store.js");
  const { CHAIN_REGISTRY } = await distImport("resources", "chains.js");

  const junoChain = CHAIN_REGISTRY["juno-1"];
  if (!junoChain) {
    throw new Error("juno-1 chain not found in registry");
  }

  const ws = store.getDefaultWalletStore();
  const { client, address } = await ws.signFor(junoWalletId, junoChain);

  // Execute Moultbook Post message
  const msg = {
    post: {
      commitment: Array.from(commitment),
      content_type: contentType,
      size_bytes: 0,
      attestation_ref: null,
      visibility: { public: {} },
      refs: refs || [],
    },
  };

  const result = await client.execute(address, moultbookAddr, msg, "auto");
  console.log(`[audit] Moultbook post tx: ${result.transactionHash}`);
  return result.transactionHash;
}

/**
 * Build a standard audit commitment from event data.
 *
 * @param {object} eventData — Arbitrary JSON-serializable audit data
 * @returns {Buffer} — 32-byte SHA-256 hash
 */
export function buildAuditCommitment(eventData) {
  const json = JSON.stringify(eventData);
  return createHash("sha256").update(json).digest();
}
