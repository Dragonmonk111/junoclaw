import { fetchProposals, fetchProposal } from './dao.js'
import { buildAkbEnvelope } from './builder.js'
import { postProposalEnvelope } from './poster.js'
import { loadState, saveState, isPosted, markPosted } from './state.js'
import { POLL_INTERVAL_MS } from './config.js'

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function parseArgs() {
  const args = process.argv.slice(2)
  let once = false
  let proposalId = null
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--once') once = true
    if (args[i] === '--daemon') once = false
    if ((args[i] === '--proposal-id' || args[i] === '-id') && args[i + 1]) {
      proposalId = Number(args[++i])
    }
    if (args[i]?.startsWith('--proposal-id=')) {
      proposalId = Number(args[i].split('=')[1])
    }
  }
  return { once, proposalId }
}

async function processSingleProposal(id, state) {
  if (isPosted(state, id)) {
    console.log(`[indexer] A${id} already posted; skipping`)
    return state
  }

  console.log(`[indexer] fetching proposal A${id}`)
  const body = await fetchProposal(id)
  const envelope = buildAkbEnvelope({ id, body })

  console.log(`[indexer] posting AKB export for A${id}`)
  const result = await postProposalEnvelope(envelope)

  if (result.dryRun) {
    console.log(`[indexer] dry-run result for A${id}:`, JSON.stringify(result, null, 2))
    return state
  }

  console.log(`[indexer] posted A${id} -> ${result.moultId} tx ${result.txHash}`)
  return markPosted(state, {
    proposal_id: id,
    moult_id: result.moultId,
    tx_hash: result.txHash,
    timestamp: new Date().toISOString(),
  })
}

async function processAllProposals(state) {
  const proposals = await fetchProposals()
  console.log(`[indexer] found ${proposals.length} proposals`)

  for (const p of proposals) {
    const id = p.id
    if (id === undefined || id === null) continue
    state = await processSingleProposal(id, state)
  }

  return state
}

async function main() {
  const { once, proposalId } = parseArgs()
  let state = loadState()

  if (Number.isFinite(proposalId)) {
    state = await processSingleProposal(proposalId, state)
    saveState(state)
    return
  }

  if (once) {
    state = await processAllProposals(state)
    saveState(state)
    return
  }

  console.log(`[indexer] starting daemon (interval ${POLL_INTERVAL_MS}ms)`)
  while (true) {
    state = await processAllProposals(state)
    saveState(state)
    console.log(`[indexer] sleeping ${POLL_INTERVAL_MS}ms`)
    await sleep(POLL_INTERVAL_MS)
  }
}

main().catch((err) => {
  console.error('[indexer] fatal:', err.message)
  process.exit(1)
})
