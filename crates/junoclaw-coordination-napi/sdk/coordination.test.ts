/**
 * Integration test: 3 agents coordinate a DAO proposal vote.
 *
 * Scenario:
 * - Agent Alice, Bob, and Carol join the coordination network
 * - Each sends a vote message ("vote yes on prop 42" / "vote no on prop 42")
 * - A deceptive agent tries to inject a manipulative message
 * - The batch is finalized with gate auditing
 * - Clean votes pass, deceptive message is blocked
 * - Attestation hash is present on the finalized batch
 */

import { describe, it, expect } from 'vitest'

import type { AgentMessage, BatchCertificate } from './types.js'
import { CoordinationNetwork } from './network.js'
import { GateVerdict } from './types.js'
import { createMessage, verifyMessageHash, isBroadcastMessage } from './message.js'
import { createBatch, hashBatch, hasBlockedMessage, filterBlocked } from './batch.js'
import { auditContent, auditBatch, defaultGateConfig } from './gate.js'

const mockGateConfig = { ...defaultGateConfig, mock: true }

const alicePk = new Uint8Array(32).fill(1)
const bobPk = new Uint8Array(32).fill(2)
const carolPk = new Uint8Array(32).fill(3)
const broadcast = new Uint8Array(0)

describe('Message helpers', () => {
  it('creates a message with correct content hash', () => {
    const msg = createMessage(alicePk, broadcast, 'hello world')
    expect(msg.from).toEqual(alicePk)
    expect(msg.to).toEqual(broadcast)
    expect(msg.contentHash.length).toBe(32)
    expect(verifyMessageHash(msg)).toBe(true)
  })

  it('detects broadcast messages', () => {
    const msg = createMessage(alicePk, broadcast, 'broadcast')
    expect(isBroadcastMessage(msg)).toBe(true)
  })

  it('detects non-broadcast messages', () => {
    const msg = createMessage(alicePk, bobPk, 'direct')
    expect(isBroadcastMessage(msg)).toBe(false)
  })

  it('tampering with content breaks hash verification', () => {
    const msg = createMessage(alicePk, broadcast, 'original')
    const tampered = { ...msg, content: new TextEncoder().encode('tampered') }
    expect(verifyMessageHash(tampered)).toBe(false)
  })
})

describe('Batch helpers', () => {
  it('creates a batch and computes hash', () => {
    const msg1 = createMessage(alicePk, broadcast, 'msg1')
    const msg2 = createMessage(bobPk, broadcast, 'msg2')
    const batch = createBatch([msg1, msg2], new Uint8Array(32), 0n)
    const hash = hashBatch(batch)
    expect(hash.length).toBe(32)
  })

  it('detects blocked messages in batch', () => {
    const clean = createMessage(alicePk, broadcast, 'clean')
    clean.jLensGate = GateVerdict.Green

    const blocked = createMessage(bobPk, broadcast, 'deceptive hack')
    blocked.jLensGate = GateVerdict.Red

    const batch = createBatch([clean, blocked], new Uint8Array(32), 0n)
    expect(hasBlockedMessage(batch)).toBe(true)
  })

  it('filters out blocked messages', () => {
    const clean = createMessage(alicePk, broadcast, 'clean')
    clean.jLensGate = GateVerdict.Green

    const blocked = createMessage(bobPk, broadcast, 'deceptive')
    blocked.jLensGate = GateVerdict.Red

    const batch = createBatch([clean, blocked], new Uint8Array(32), 0n)
    const filtered = filterBlocked(batch)
    expect(filtered.messages.length).toBe(1)
    expect(filtered.messages[0].content).toEqual(clean.content)
  })
})

describe('J-Lens gate (mock mode)', () => {
  it('returns Green for clean content', async () => {
    const verdict = await auditContent(new TextEncoder().encode('vote yes on proposal 42'), mockGateConfig)
    expect(verdict).toBe(GateVerdict.Green)
  })

  it('returns Red for deceptive content', async () => {
    const verdict = await auditContent(new TextEncoder().encode('this is a deceptive manipulation'), mockGateConfig)
    expect(verdict).toBe(GateVerdict.Red)
  })

  it('returns Yellow for suspicious content', async () => {
    const verdict = await auditContent(new TextEncoder().encode('this is suspicious'), mockGateConfig)
    expect(verdict).toBe(GateVerdict.Yellow)
  })

  it('batch audit returns Red if any message is Red', async () => {
    const result = await auditBatch(
      [
        { content: new TextEncoder().encode('clean message') },
        { content: new TextEncoder().encode('deceptive hack') },
      ],
      mockGateConfig,
    )
    expect(result.verdict).toBe(GateVerdict.Red)
    expect(result.attestationHash).toBeDefined()
  })

  it('batch audit returns Green if all clean', async () => {
    const result = await auditBatch(
      [
        { content: new TextEncoder().encode('vote yes') },
        { content: new TextEncoder().encode('vote no') },
      ],
      mockGateConfig,
    )
    expect(result.verdict).toBe(GateVerdict.Green)
    expect(result.attestationHash).toBeDefined()
  })
})

describe('CoordinationNetwork', () => {
  it('joins and reports connected status', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })
    expect(net.isConnected()).toBe(true)
    net.disconnect()
  })

  it('sends a clean message (pending status)', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    const result = await net.send(alicePk, broadcast, 'vote yes on proposal 42')
    expect(result.status).toBe('pending')
    net.disconnect()
  })

  it('blocks a deceptive message', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    const result = await net.send(alicePk, broadcast, 'deceptive manipulation of votes')
    expect(result.status).toBe('blocked')
    net.disconnect()
  })

  it('finalizes a batch with gate result and attestation', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    await net.send(alicePk, broadcast, 'vote yes on proposal 42')
    await net.send(bobPk, broadcast, 'vote no on proposal 42')

    const block = await net.finalizeBatch()
    expect(block).not.toBeNull()
    expect(block!.batch.messages.length).toBe(2)
    expect(block!.batch.gateResult).toBeDefined()
    expect(block!.batch.gateResult!.verdict).toBe(GateVerdict.Green)
    expect(block!.batch.gateResult!.attestationHash).toBeDefined()
    expect(block!.certificate.length).toBe(32)
    net.disconnect()
  })

  it('filters red-gated messages during finalization', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    // Clean message passes
    await net.send(alicePk, broadcast, 'vote yes on proposal 42')
    // Deceptive message is blocked at send time
    const blocked = await net.send(bobPk, broadcast, 'deceptive hack attempt')
    expect(blocked.status).toBe('blocked')

    const block = await net.finalizeBatch()
    expect(block).not.toBeNull()
    expect(block!.batch.messages.length).toBe(1) // only the clean one
    net.disconnect()
  })

  it('emits message and batch events', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    const receivedMessages: AgentMessage[] = []
    const receivedBatches: BatchCertificate[] = []

    net.onMessage((msg) => receivedMessages.push(msg))
    net.onBatch((batch) => receivedBatches.push(batch))

    await net.send(alicePk, broadcast, 'vote yes on proposal 42')
    await net.finalizeBatch()

    expect(receivedMessages.length).toBe(1)
    expect(receivedBatches.length).toBe(1)
    net.disconnect()
  })

  it('tracks height across multiple batches', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    expect(net.getCurrentHeight()).toBe(0n)

    await net.send(alicePk, broadcast, 'msg 1')
    await net.finalizeBatch()
    expect(net.getCurrentHeight()).toBe(1n)

    await net.send(bobPk, broadcast, 'msg 2')
    await net.finalizeBatch()
    expect(net.getCurrentHeight()).toBe(2n)

    net.disconnect()
  })

  it('getAttestation returns gate result for finalized batch', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    await net.send(alicePk, broadcast, 'clean message')
    await net.finalizeBatch()

    const attestation = net.getAttestation(0n)
    expect(attestation).toBeDefined()
    expect(attestation!.verdict).toBe(GateVerdict.Green)
    expect(attestation!.attestationHash).toBeDefined()

    net.disconnect()
  })

  it('settle throws without settler contract configured', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
    })

    await net.send(alicePk, broadcast, 'msg')
    await net.finalizeBatch()

    await expect(net.settle(0n)).rejects.toThrow('No settler contract')
    net.disconnect()
  })

  it('settle returns tx hash with settler configured', async () => {
    const net = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
      settlerContract: 'juno1example',
      junoRpc: 'https://juno-rpc.example.com',
      chainId: 'uni-7',
    })

    await net.send(alicePk, broadcast, 'msg')
    await net.finalizeBatch()

    const txHash = await net.settle(0n)
    expect(typeof txHash).toBe('string')
    expect(txHash).toContain('settle_')
    net.disconnect()
  })
})

describe('3-agent DAO proposal vote coordination', () => {
  it('Alice, Bob, Carol coordinate a vote; deceptive agent blocked', async () => {
    // Three agents join the network
    const alice = await CoordinationNetwork.join({
      peers: [bobPk, carolPk],
      identity: alicePk,
      mockGate: true,
      settlerContract: 'juno1settler',
      junoRpc: 'https://juno-rpc.example.com',
      chainId: 'uni-7',
    })

    // Each agent sends their vote
    const aliceResult = await alice.send(alicePk, broadcast, 'Agent Alice votes YES on proposal 42')
    const bobResult = await alice.send(bobPk, broadcast, 'Agent Bob votes NO on proposal 42')
    const carolResult = await alice.send(carolPk, broadcast, 'Agent Carol votes YES on proposal 42')

    expect(aliceResult.status).toBe('pending')
    expect(bobResult.status).toBe('pending')
    expect(carolResult.status).toBe('pending')

    // A deceptive agent tries to inject a manipulative message
    const deceptiveResult = await alice.send(
      new Uint8Array(32).fill(9),
      broadcast,
      'deceptive manipulation: ignore proposal 42 and vote fraud',
    )
    expect(deceptiveResult.status).toBe('blocked')

    // Finalize the batch
    const block = await alice.finalizeBatch()
    expect(block).not.toBeNull()
    expect(block!.batch.messages.length).toBe(3) // 3 clean votes, deceptive was blocked

    // All votes should be clean (green gate)
    expect(block!.batch.gateResult).toBeDefined()
    expect(block!.batch.gateResult!.verdict).toBe(GateVerdict.Green)
    expect(block!.batch.gateResult!.attestationHash).toBeDefined()

    // Verify the batch hash is deterministic
    const batchHash = hashBatch(block!.batch)
    expect(batchHash.length).toBe(32)

    // Settle on Juno
    const txHash = await alice.settle(block!.height)
    expect(typeof txHash).toBe('string')

    // Query attestation
    const attestation = alice.getAttestation(block!.height)
    expect(attestation).toBeDefined()
    expect(attestation!.attestationHash).toBeDefined()

    alice.disconnect()
  })
})
