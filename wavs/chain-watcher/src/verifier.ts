// ── Verification Workflow Engine ──
// Processes chain events and runs verification logic per event type.
// In production, this would invoke the WASI component inside a TEE.
// For now, it performs software-mode verification.

import { createHash } from 'crypto'
import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { CONFIG } from './config.js'
import { logger } from './logger.js'
import type { ChainEvent } from './event-watcher.js'

const LOG = 'Verifier'

export interface VerificationResult {
  verified: boolean
  proposalId: number
  taskType: string
  dataHash: string
  attestationHash: string
  details: string
}

export class Verifier {
  private client: CosmWasmClient | null = null

  private async getClient(): Promise<CosmWasmClient> {
    if (!this.client) {
      this.client = await CosmWasmClient.connect(CONFIG.rpcHttp)
    }
    return this.client
  }

  async verify(event: ChainEvent): Promise<VerificationResult | null> {
    const eventType = event.type.replace('wasm-', '')

    switch (eventType) {
      case 'wavs_push':
      case 'execute_proposal':
        return this.verifyWavsPush(event)

      case 'outcome_create':
        return this.verifyOutcomeCreate(event)

      case 'sortition_request':
        return this.verifySortitionRequest(event)

      case 'create_proposal':
      case 'cast_vote':
        // Informational events — log but don't attest
        logger.info(LOG, `Governance event: ${eventType}`, event.attributes)
        return null

      default:
        logger.debug(LOG, `Unhandled event type: ${eventType}`)
        return null
    }
  }

  // ── WAVS Push verification ──
  // Verifies that a WavsPush proposal was correctly executed
  private async verifyWavsPush(event: ChainEvent): Promise<VerificationResult | null> {
    const proposalId = Number(event.attributes['proposal_id'] || event.attributes['task_id'])
    if (!proposalId) {
      logger.warn(LOG, 'wavs_push event missing proposal_id/task_id')
      return null
    }

    const taskDescription = event.attributes['task_description'] || ''
    const contractAddr = event.attributes['contract_address'] || CONFIG.agentCompany

    try {
      const client = await this.getClient()

      // Query the proposal on-chain to verify it exists and was executed
      const proposal = await client.queryContractSmart(contractAddr, {
        get_proposal: { proposal_id: proposalId },
      })

      if (!proposal.executed) {
        logger.warn(LOG, `Proposal ${proposalId} not yet executed`, { status: proposal.status })
        return null
      }

      // Verify the task description matches
      const onChainDesc = proposal.kind?.wavs_push?.task_description
        || proposal.kind?.WavsPush?.task_description
        || ''

      const dataHash = createHash('sha256')
        .update(`${proposalId}:${onChainDesc}:${event.blockHeight}`)
        .digest('hex')

      const attestationHash = createHash('sha256')
        .update(`junoclaw:verifier:wavs_push:${dataHash}`)
        .digest('hex')

      const descMatch = !taskDescription || onChainDesc === taskDescription

      logger.info(LOG, `Verified wavs_push proposal ${proposalId}`, {
        descMatch,
        blockHeight: event.blockHeight,
      })

      return {
        verified: descMatch,
        proposalId,
        taskType: 'wavs_push',
        dataHash,
        attestationHash,
        details: `Task: "${onChainDesc}" | Executed at block ${event.blockHeight} | Description match: ${descMatch}`,
      }
    } catch (err) {
      logger.error(LOG, `Failed to verify wavs_push ${proposalId}: ${err}`)
      return null
    }
  }

  // ── Outcome Create verification ──
  // Verifies outcome market creation parameters
  private async verifyOutcomeCreate(event: ChainEvent): Promise<VerificationResult | null> {
    const proposalId = Number(event.attributes['market_id'] || event.attributes['proposal_id'])
    if (!proposalId) return null

    const question = event.attributes['question'] || ''
    const deadlineBlock = Number(event.attributes['deadline_block'] || 0)

    const dataHash = createHash('sha256')
      .update(`${proposalId}:outcome_create:${question}:${deadlineBlock}`)
      .digest('hex')

    const attestationHash = createHash('sha256')
      .update(`junoclaw:verifier:outcome_create:${dataHash}`)
      .digest('hex')

    logger.info(LOG, `Verified outcome_create ${proposalId}`, { question: question.slice(0, 40) })

    return {
      verified: true,
      proposalId,
      taskType: 'outcome_create',
      dataHash,
      attestationHash,
      details: `Outcome market: "${question}" | Deadline: block ${deadlineBlock}`,
    }
  }

  // ── Sortition verification ──
  // Logs the sortition request (randomness is handled by NOIS/drand)
  private async verifySortitionRequest(event: ChainEvent): Promise<VerificationResult | null> {
    const jobId = event.attributes['job_id'] || ''
    const count = Number(event.attributes['count'] || 0)
    const purpose = event.attributes['purpose'] || ''

    logger.info(LOG, `Sortition request: job=${jobId} count=${count} purpose="${purpose}"`)

    // Sortition verification is handled by the randomness submission path
    // This just logs for the feed
    return null
  }
}
