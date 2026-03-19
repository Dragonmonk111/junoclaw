// ── Juno Testnet Event Watcher ──
// Subscribes to Tendermint WebSocket for contract events.
// Falls back to polling if WS is unavailable.

import WebSocket from 'ws'
import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CONFIG } from './config.js'
import { logger } from './logger.js'

const LOG = 'EventWatcher'

export interface ChainEvent {
  type: string
  txHash: string
  blockHeight: number
  attributes: Record<string, string>
  timestamp: string
}

export type EventHandler = (event: ChainEvent) => void | Promise<void>

export class EventWatcher {
  private ws: WebSocket | null = null
  private handlers: EventHandler[] = []
  private lastHeight = 0
  private pollTimer: ReturnType<typeof setInterval> | null = null
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private running = false
  private client: CosmWasmClient | null = null

  onEvent(handler: EventHandler) {
    this.handlers.push(handler)
  }

  async start() {
    this.running = true
    logger.info(LOG, `Starting event watcher for ${CONFIG.agentCompany}`)
    logger.info(LOG, `Watched events: ${CONFIG.watchEvents.join(', ')}`)

    // Get a CosmWasm client for polling fallback
    try {
      this.client = await CosmWasmClient.connect(CONFIG.rpcHttp)
      const height = await this.client.getHeight()
      this.lastHeight = height
      logger.info(LOG, `Connected to RPC, current height: ${height}`)
    } catch (err) {
      logger.error(LOG, `Failed to connect to RPC: ${err}`)
    }

    // Try WebSocket first
    this.connectWs()

    // Start polling as fallback/supplement
    this.startPolling()
  }

  stop() {
    this.running = false
    if (this.ws) {
      this.ws.close()
      this.ws = null
    }
    if (this.pollTimer) {
      clearInterval(this.pollTimer)
      this.pollTimer = null
    }
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    logger.info(LOG, 'Stopped')
  }

  private connectWs() {
    if (!this.running) return

    try {
      logger.info(LOG, `Connecting to WebSocket: ${CONFIG.rpcWs}`)
      this.ws = new WebSocket(CONFIG.rpcWs)

      this.ws.on('open', () => {
        logger.info(LOG, 'WebSocket connected')
        // Subscribe to Tx events from our contract
        const subscribeMsg = {
          jsonrpc: '2.0',
          method: 'subscribe',
          id: 'junoclaw-watcher',
          params: {
            query: `tm.event='Tx' AND wasm._contract_address='${CONFIG.agentCompany}'`,
          },
        }
        this.ws!.send(JSON.stringify(subscribeMsg))
        logger.info(LOG, 'Subscribed to contract events')
      })

      this.ws.on('message', (data: WebSocket.Data) => {
        try {
          const msg = JSON.parse(data.toString())
          this.handleWsMessage(msg)
        } catch (err) {
          logger.error(LOG, `Failed to parse WS message: ${err}`)
        }
      })

      this.ws.on('close', () => {
        logger.warn(LOG, 'WebSocket disconnected')
        this.ws = null
        this.scheduleReconnect()
      })

      this.ws.on('error', (err) => {
        logger.error(LOG, `WebSocket error: ${err.message}`)
        if (this.ws) {
          this.ws.close()
          this.ws = null
        }
        this.scheduleReconnect()
      })
    } catch (err) {
      logger.error(LOG, `Failed to create WebSocket: ${err}`)
      this.scheduleReconnect()
    }
  }

  private scheduleReconnect() {
    if (!this.running || this.reconnectTimer) return
    logger.info(LOG, 'Reconnecting in 10s...')
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null
      this.connectWs()
    }, 10_000)
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private handleWsMessage(msg: any) {
    if (!msg.result?.data?.value?.TxResult) return

    const txResult = msg.result.data.value.TxResult
    const height = Number(txResult.height)
    const txHash = txResult.tx ? Buffer.from(txResult.tx, 'base64').toString('hex').slice(0, 64) : 'unknown'

    // Parse events from the result
    const events = txResult.result?.events ?? []
    for (const event of events) {
      const eventType = event.type as string
      if (!CONFIG.watchEvents.some(w => eventType.includes(w) || eventType === w)) continue

      const attributes: Record<string, string> = {}
      for (const attr of event.attributes ?? []) {
        const key = attr.key ? Buffer.from(attr.key, 'base64').toString() : ''
        const value = attr.value ? Buffer.from(attr.value, 'base64').toString() : ''
        if (key) attributes[key] = value
      }

      const chainEvent: ChainEvent = {
        type: eventType,
        txHash,
        blockHeight: height,
        attributes,
        timestamp: new Date().toISOString(),
      }

      logger.info(LOG, `Event: ${eventType} @ block ${height}`, { txHash: txHash.slice(0, 16), attributes })
      this.emit(chainEvent)
    }

    if (height > this.lastHeight) {
      this.lastHeight = height
    }
  }

  // ── Polling fallback ──

  private startPolling() {
    if (this.pollTimer) return
    this.pollTimer = setInterval(() => this.poll(), CONFIG.pollInterval)
    logger.info(LOG, `Polling every ${CONFIG.pollInterval}ms as fallback`)
  }

  private async poll() {
    if (!this.client) {
      try {
        this.client = await CosmWasmClient.connect(CONFIG.rpcHttp)
      } catch {
        return
      }
    }

    try {
      const currentHeight = await this.client.getHeight()
      if (currentHeight <= this.lastHeight) return

      // Search for recent TX events from our contract
      const searchUrl = `${CONFIG.restApi}/cosmos/tx/v1beta1/txs?events=wasm._contract_address%3D%27${CONFIG.agentCompany}%27&order_by=ORDER_BY_DESC&pagination.limit=5`

      const resp = await fetch(searchUrl)
      if (!resp.ok) {
        logger.debug(LOG, `Poll search failed: ${resp.status}`)
        return
      }

      const data = await resp.json() as {
        tx_responses?: Array<{
          txhash: string
          height: string
          logs?: Array<{
            events?: Array<{
              type: string
              attributes?: Array<{ key: string; value: string }>
            }>
          }>
        }>
      }

      for (const tx of data.tx_responses ?? []) {
        const height = Number(tx.height)
        if (height <= this.lastHeight) continue

        for (const log of tx.logs ?? []) {
          for (const event of log.events ?? []) {
            if (!CONFIG.watchEvents.some(w => event.type.includes(w) || event.type === w)) continue

            const attributes: Record<string, string> = {}
            for (const attr of event.attributes ?? []) {
              attributes[attr.key] = attr.value
            }

            const chainEvent: ChainEvent = {
              type: event.type,
              txHash: tx.txhash,
              blockHeight: height,
              attributes,
              timestamp: new Date().toISOString(),
            }

            logger.info(LOG, `Poll event: ${event.type} @ block ${height}`, { txHash: tx.txhash.slice(0, 16) })
            this.emit(chainEvent)
          }
        }
      }

      this.lastHeight = currentHeight
    } catch (err) {
      logger.debug(LOG, `Poll error: ${err}`)
    }
  }

  private emit(event: ChainEvent) {
    for (const handler of this.handlers) {
      try {
        const result = handler(event)
        if (result instanceof Promise) {
          result.catch(err => logger.error(LOG, `Handler error: ${err}`))
        }
      } catch (err) {
        logger.error(LOG, `Handler error: ${err}`)
      }
    }
  }
}
