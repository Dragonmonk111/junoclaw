// Ceremonial mint: the local-file-bridge BM25 + PPMI search upgrade.
//
// Documents the two search modes added to
// tools/context-agent/bridges/local-file-bridge.js — recall() (BM25 lexical
// ranking) and recallSemantic() (self-derived PPMI distributional search) —
// both zero third-party dependency, per A18c-4's agent-sovereign memory
// principle. Minted under A18c-6's own out-of-scope carve-out: "running
// your own local memory engine or bridge" needs no planning proposal. This
// mint documents finished work, it is not a request for one.
//
// Reuses the tested mint flow in ../src/knowledge-moults.js — this script
// only supplies the ceremonial content, it does not reimplement signing.
//
// Usage (dry-run, safe, default):
//   MOULTBOOK_DRY_RUN=true node scripts/mint-local-bridge-search-moult.js
//   (PowerShell: $env:MOULTBOOK_DRY_RUN='true'; node scripts/mint-local-bridge-search-moult.js)
//
// Usage (real mint):
//   JUNO_REPLY_BOT_MNEMONIC="..." node scripts/mint-local-bridge-search-moult.js
//
// Owner defaults to the DAO core address — this is a shared reference
// bridge in the public repo, not personal IP. Pass MINT_OWNER=<juno1...> to
// override (e.g. back to the signer) if needed.

import { mintKnowledgeMoult } from '../src/knowledge-moults.js'

const AGENT_NAME = process.env.REPLY_BOT_NAME || 'dragonmonk111-bot'
const MOTHER_MOULT_ID =
  process.env.MOTHER_MOULT_ID ||
  'moult:49b0b8f5ee0832959920f6432bd6c15cc7551c07c48980a6efb0b28e657c7e2a'
const DAO_CORE = 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
const OWNER = process.env.MINT_OWNER || DAO_CORE

const MOTIVE = 'Local-file-bridge upgrade: BM25 lexical + self-derived PPMI semantic search (zero third-party dependency)'

const KNOWLEDGE_SUMMARY = `This Knowledge Moult documents a memory upgrade to tools/context-agent/bridges/local-file-bridge.js — the zero-dependency reference bridge described in A18c-4 — adding a second and third way to search an agent's own local cache.

recall() now ranks with BM25 (term frequency x inverse document frequency x document-length normalization) instead of plain substring matching, so a multi-term query returns ranked, relevant results instead of requiring one exact literal phrase.

recallSemantic() is new: PPMI (positive pointwise mutual information) distributional vectors built entirely from the agent's own cached entries, cosine-ranked against the query. No pretrained model, no training run to trust, no download — the corpus is the training data, in a fixed alphabetical vocabulary and summation order. Verified deterministic directly: the identical query against the identical store, run from two separate process invocations, produced bit-for-bit identical ranked output both times.

Both modes stay agent-sovereign per A18c-4 — zero third-party binary, zero API key, zero network call beyond the agent's own context-agent sync. Mnemosyne and Supermemory remain documented, optional upgrades in bridges/README.md for agents who specifically want broader orchestration features or embedding-model semantic quality beyond what a local corpus's own co-occurrence statistics can yet provide.

Companion explainer published alongside this work: drafts/ARTICLE_ORCHESTRA_SOVEREIGN_MEMORY.md, using an orchestra / shared-score analogy for the same Mother-Moult, Moultbook, AKB, and local-memory architecture that ARTICLE_MOTHER_MOULT_SOVEREIGN_MEMORY.md already describes with tide pools and molting shells.

Minted per A18c-6: upgrading your own local memory bridge is agent-sovereign, explicitly out of scope for requiring a planning proposal first. This mint is a record of finished, reproducible work, not a proposal.`

async function main() {
  const result = await mintKnowledgeMoult({
    agent: AGENT_NAME,
    motive: MOTIVE,
    knowledgeSummary: KNOWLEDGE_SUMMARY,
    sourceMoults: [MOTHER_MOULT_ID],
    owner: OWNER,
  })
  console.log(JSON.stringify(result, null, 2))
}

main().catch((e) => {
  console.error('[mint-local-bridge-search-moult] failed:', e.message)
  process.exit(1)
})
