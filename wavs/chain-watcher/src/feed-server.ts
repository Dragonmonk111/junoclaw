// ── WebSocket Feed Server ──
// Broadcasts chain events and verification results to connected frontends.
// The frontend's UpdatesPanel can connect to this feed.

import { WebSocketServer, WebSocket } from 'ws'
import { CONFIG } from './config.js'
import { logger } from './logger.js'
import type { ChainEvent } from './event-watcher.js'
import type { VerificationResult } from './verifier.js'

const LOG = 'FeedServer'

export interface FeedMessage {
  type: 'chain_event' | 'verification' | 'attestation_tx' | 'status'
  timestamp: string
  data: Record<string, unknown>
}

export class FeedServer {
  private wss: WebSocketServer | null = null
  private clients: Set<WebSocket> = new Set()
  private recentMessages: FeedMessage[] = []
  private maxRecent = 100

  start() {
    this.wss = new WebSocketServer({ port: CONFIG.feedPort })

    this.wss.on('connection', (ws) => {
      this.clients.add(ws)
      logger.info(LOG, `Client connected (${this.clients.size} total)`)

      // Send recent history to new client
      for (const msg of this.recentMessages) {
        ws.send(JSON.stringify(msg))
      }

      // Send current status
      this.sendTo(ws, {
        type: 'status',
        timestamp: new Date().toISOString(),
        data: {
          watcherStatus: 'running',
          connectedClients: this.clients.size,
          recentEventCount: this.recentMessages.length,
          contractAddress: CONFIG.agentCompany,
          chainId: CONFIG.chainId,
        },
      })

      ws.on('close', () => {
        this.clients.delete(ws)
        logger.info(LOG, `Client disconnected (${this.clients.size} total)`)
      })

      ws.on('error', () => {
        this.clients.delete(ws)
      })
    })

    logger.info(LOG, `Feed server listening on port ${CONFIG.feedPort}`)
  }

  stop() {
    if (this.wss) {
      for (const client of this.clients) {
        client.close()
      }
      this.wss.close()
      this.wss = null
      this.clients.clear()
    }
    logger.info(LOG, 'Feed server stopped')
  }

  broadcastEvent(event: ChainEvent) {
    const msg: FeedMessage = {
      type: 'chain_event',
      timestamp: event.timestamp,
      data: {
        eventType: event.type,
        txHash: event.txHash,
        blockHeight: event.blockHeight,
        attributes: event.attributes,
      },
    }
    this.broadcast(msg)
  }

  broadcastVerification(result: VerificationResult) {
    const msg: FeedMessage = {
      type: 'verification',
      timestamp: new Date().toISOString(),
      data: {
        verified: result.verified,
        proposalId: result.proposalId,
        taskType: result.taskType,
        dataHash: result.dataHash,
        attestationHash: result.attestationHash,
        details: result.details,
      },
    }
    this.broadcast(msg)
  }

  broadcastAttestationTx(proposalId: number, txHash: string) {
    const msg: FeedMessage = {
      type: 'attestation_tx',
      timestamp: new Date().toISOString(),
      data: { proposalId, txHash },
    }
    this.broadcast(msg)
  }

  private broadcast(msg: FeedMessage) {
    this.recentMessages.push(msg)
    if (this.recentMessages.length > this.maxRecent) {
      this.recentMessages.shift()
    }

    const payload = JSON.stringify(msg)
    for (const client of this.clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(payload)
      }
    }
  }

  private sendTo(ws: WebSocket, msg: FeedMessage) {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }

  getClientCount(): number {
    return this.clients.size
  }
}
