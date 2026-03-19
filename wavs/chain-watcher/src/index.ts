// ── JunoClaw Chain Watcher ──
// Persistent service that monitors Juno testnet events,
// runs verification workflows, submits attestations,
// and broadcasts results to the frontend via WebSocket.

import { CONFIG } from './config.js'
import { logger } from './logger.js'
import { EventWatcher } from './event-watcher.js'
import { Verifier } from './verifier.js'
import { Attestor } from './attestor.js'
import { FeedServer } from './feed-server.js'

const LOG = 'Main'

async function main() {
  logger.info(LOG, '═══════════════════════════════════════════')
  logger.info(LOG, '  JunoClaw Chain Watcher v0.1.0')
  logger.info(LOG, `  Chain: ${CONFIG.chainId}`)
  logger.info(LOG, `  Contract: ${CONFIG.agentCompany}`)
  logger.info(LOG, `  RPC: ${CONFIG.rpcHttp}`)
  logger.info(LOG, `  Feed port: ${CONFIG.feedPort}`)
  logger.info(LOG, '═══════════════════════════════════════════')

  // ── Initialize components ──
  const watcher = new EventWatcher()
  const verifier = new Verifier()
  const attestor = new Attestor()
  const feed = new FeedServer()

  // Init attestor (may fail if no mnemonic — that's OK, runs in read-only mode)
  const canAttest = await attestor.init()
  if (canAttest) {
    logger.info(LOG, `Attestation mode: ACTIVE (operator: ${attestor.getAddress()})`)
  } else {
    logger.info(LOG, 'Attestation mode: READ-ONLY (set OPERATOR_MNEMONIC to enable)')
  }

  // Start feed server
  feed.start()

  // ── Wire event pipeline ──
  // Event → Verify → Attest → Broadcast
  watcher.onEvent(async (event) => {
    // Broadcast raw event to feed
    feed.broadcastEvent(event)

    // Run verification
    const result = await verifier.verify(event)
    if (!result) return

    // Broadcast verification result
    feed.broadcastVerification(result)

    // Submit attestation if verified and operator wallet available
    if (result.verified && canAttest) {
      const txHash = await attestor.submit(result)
      if (txHash) {
        feed.broadcastAttestationTx(result.proposalId, txHash)
      }
    }
  })

  // Start watching
  await watcher.start()

  // ── Periodic maintenance ──
  setInterval(async () => {
    // Drain attestation queue
    await attestor.drainQueue()

    // Log status
    logger.debug(LOG, 'Status', {
      feedClients: feed.getClientCount(),
      attestationQueue: attestor.getQueueSize(),
    })
  }, 30_000)

  // ── Graceful shutdown ──
  const shutdown = () => {
    logger.info(LOG, 'Shutting down...')
    watcher.stop()
    feed.stop()
    process.exit(0)
  }

  process.on('SIGINT', shutdown)
  process.on('SIGTERM', shutdown)

  logger.info(LOG, 'Chain watcher running. Press Ctrl+C to stop.')
}

main().catch((err) => {
  logger.error(LOG, `Fatal error: ${err}`)
  process.exit(1)
})
