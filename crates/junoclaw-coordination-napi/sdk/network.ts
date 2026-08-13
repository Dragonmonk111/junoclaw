/**
 * CoordinationNetwork — high-level SDK for agent coordination.
 *
 * Usage:
 * ```typescript
 * const net = await CoordinationNetwork.join({
 *   peers: [peer1Pk, peer2Pk],
 *   identity: myPk,
 *   mockGate: true,
 * });
 *
 * const result = await net.send(message);
 * net.on('message', (msg, audit) => { ... });
 * await net.settle(batchId);
 * ```
 */

import { EventEmitter } from 'node:events'

import type {
  AgentMessage,
  Batch,
  BatchCertificate,
  CoordinationNetworkConfig,
  GateResult,
  MessageHandler,
  BatchHandler,
  SendResult,
} from './types.js'
import { GateVerdict } from './types.js'
import { createMessage, encodeMessage, decodeMessage, verifyMessageHash } from './message.js'
import { createBatch, hashBatch, hasBlockedMessage, filterBlocked, withGateResult } from './batch.js'
import { auditContent, auditBatch, type GateConfig, defaultGateConfig } from './gate.js'

export class CoordinationNetwork extends EventEmitter {
  readonly config: CoordinationNetworkConfig
  private gateConfig: GateConfig
  private pending: AgentMessage[] = []
  private batches: Map<bigint, BatchCertificate> = new Map()
  private height = 0n
  private lastHash: Uint8Array = new Uint8Array(32)
  private connected = false
  private messageHandlers: Map<string, MessageHandler> = new Map()
  private batchHandlers: Map<string, BatchHandler> = new Map()

  private constructor(config: CoordinationNetworkConfig) {
    super()
    this.config = config
    this.gateConfig = {
      ...defaultGateConfig,
      csiEndpoint: config.jLensEndpoint ?? defaultGateConfig.csiEndpoint,
      apiKey: config.jLensApiKey,
      mock: config.mockGate ?? false,
    }
  }

  /**
   * Join the coordination network.
   */
  static async join(config: CoordinationNetworkConfig): Promise<CoordinationNetwork> {
    const net = new CoordinationNetwork(config)
    net.connected = true
    return net
  }

  /**
   * Send a message through the coordination network.
   *
   * The message is audited by the J-Lens gate. If red-gated, it is blocked.
   * Otherwise it enters the pending queue and will be included in the next batch.
   */
  async send(
    from: Uint8Array,
    to: Uint8Array,
    content: Uint8Array | string,
  ): Promise<SendResult> {
    if (!this.connected) {
      throw new Error('Network not connected. Call CoordinationNetwork.join() first.')
    }

    const msg = createMessage(from, to, content)

    // Audit with gate
    const verdict = await auditContent(msg.content, this.gateConfig)
    if (verdict === GateVerdict.Red) {
      return { status: 'blocked', reason: 'J-Lens gate: red (deceptive content)' }
    }

    // Attach verdict and queue
    msg.jLensGate = verdict
    this.pending.push(msg)

    return { status: 'pending' }
  }

  /**
   * Finalize pending messages into a batch.
   *
   * Audits all messages, filters red-gated ones, creates a batch with
   * a gate result and simulated certificate.
   */
  async finalizeBatch(): Promise<BatchCertificate | null> {
    if (this.pending.length === 0) {
      return null
    }

    const messages = this.pending.splice(0)
    const timestamp = BigInt(Date.now())

    // Audit the batch
    const gateResult = await auditBatch(
      messages.map((m) => ({ content: m.content })),
      this.gateConfig,
    )

    // Filter out red-gated messages
    const filtered = messages.filter((m) => m.jLensGate !== GateVerdict.Red)

    if (filtered.length === 0) {
      return null
    }

    const batch = createBatch(filtered, this.lastHash, this.height, timestamp)
    const batchWithGate = withGateResult(batch, gateResult)
    const batchHash = hashBatch(batchWithGate)

    // Simulate a threshold certificate
    const cert = this.simulateCertificate(batchHash)

    const block: BatchCertificate = {
      batch: batchWithGate,
      certificate: cert,
      height: this.height,
      finalizedAt: timestamp,
    }

    this.batches.set(this.height, block)
    this.lastHash = batchHash
    this.height += 1n

    // Emit events
    this.emit('batch', block)
    for (const msg of filtered) {
      this.emit('message', msg, gateResult)
    }

    return block
  }

  /**
   * Settle a finalized batch on Juno via the coordination-settler contract.
   *
   * Requires settlerContract, junoRpc, and chainId to be configured.
   */
  async settle(batchId: bigint): Promise<string> {
    if (!this.config.settlerContract) {
      throw new Error('No settler contract configured')
    }
    if (!this.config.junoRpc) {
      throw new Error('No Juno RPC endpoint configured')
    }

    const block = this.batches.get(batchId)
    if (!block) {
      throw new Error(`Batch ${batchId} not found`)
    }

    // In production, this would submit a SubmitBatch tx to the coordination-settler contract.
    // For now, we simulate the settlement.
    const txHash = `settle_${batchId}_${Date.now()}`
    return txHash
  }

  /**
   * Query whether a batch was audited and get its attestation.
   */
  getAttestation(batchId: bigint): GateResult | undefined {
    const block = this.batches.get(batchId)
    return block?.batch.gateResult
  }

  /**
   * Get a finalized batch by height.
   */
  getBatch(height: bigint): BatchCertificate | undefined {
    return this.batches.get(height)
  }

  /**
   * Get the current height.
   */
  getCurrentHeight(): bigint {
    return this.height
  }

  /**
   * Check if the network is connected.
   */
  isConnected(): boolean {
    return this.connected
  }

  /**
   * Disconnect from the network.
   */
  disconnect(): void {
    this.connected = false
    this.removeAllListeners()
  }

  /**
   * Register a message handler.
   */
  onMessage(handler: MessageHandler): () => void {
    const id = `msg_${Date.now()}_${Math.random()}`
    this.messageHandlers.set(id, handler)
    this.on('message', handler)
    return () => {
      this.off('message', handler)
      this.messageHandlers.delete(id)
    }
  }

  /**
   * Register a batch handler.
   */
  onBatch(handler: BatchHandler): () => void {
    const id = `batch_${Date.now()}_${Math.random()}`
    this.batchHandlers.set(id, handler)
    this.on('batch', handler)
    return () => {
      this.off('batch', handler)
      this.batchHandlers.delete(id)
    }
  }

  /**
   * Simulate a threshold certificate (in production, this is a BLS aggregate).
   */
  private simulateCertificate(batchHash: Uint8Array): Uint8Array {
    const crypto = require('node:crypto')
    const h = crypto.createHash('sha256')
    h.update(Buffer.from(batchHash))
    h.update(Buffer.from(this.config.identity))
    // Simulate 2f+1 signatures
    for (let i = 0; i < 3; i++) {
      h.update(Buffer.from([i]))
    }
    return new Uint8Array(h.digest())
  }
}
