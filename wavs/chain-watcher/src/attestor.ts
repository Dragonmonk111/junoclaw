// ── Attestation Submitter ──
// Submits verified attestation results back to the agent-company contract.
// Requires an operator wallet with funds for gas.

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'
import { CONFIG } from './config.js'
import { logger } from './logger.js'
import type { VerificationResult } from './verifier.js'

const LOG = 'Attestor'

export class Attestor {
  private client: SigningCosmWasmClient | null = null
  private address: string | null = null
  private queue: VerificationResult[] = []
  private processing = false

  async init(): Promise<boolean> {
    if (!CONFIG.operatorMnemonic) {
      logger.warn(LOG, 'No OPERATOR_MNEMONIC set — attestation submission disabled (read-only mode)')
      return false
    }

    try {
      const wallet = await DirectSecp256k1HdWallet.fromMnemonic(CONFIG.operatorMnemonic, {
        prefix: 'juno',
      })
      const [account] = await wallet.getAccounts()
      this.address = account.address

      this.client = await SigningCosmWasmClient.connectWithSigner(
        CONFIG.rpcHttp,
        wallet,
        { gasPrice: GasPrice.fromString(CONFIG.gasPrice) },
      )

      logger.info(LOG, `Operator wallet: ${this.address}`)
      return true
    } catch (err) {
      logger.error(LOG, `Failed to init operator wallet: ${err}`)
      return false
    }
  }

  async submit(result: VerificationResult): Promise<string | null> {
    if (!this.client || !this.address) {
      logger.warn(LOG, `Cannot submit attestation — no operator wallet. Queuing for later.`)
      this.queue.push(result)
      return null
    }

    try {
      const msg = {
        submit_attestation: {
          proposal_id: result.proposalId,
          task_type: result.taskType,
          data_hash: result.dataHash,
          attestation_hash: result.attestationHash,
        },
      }

      logger.info(LOG, `Submitting attestation for proposal ${result.proposalId}`, {
        taskType: result.taskType,
        dataHash: result.dataHash.slice(0, 16),
      })

      const tx = await this.client.execute(
        this.address,
        CONFIG.agentCompany,
        msg,
        'auto',
      )

      logger.info(LOG, `Attestation TX confirmed: ${tx.transactionHash}`, {
        proposalId: result.proposalId,
        gasUsed: tx.gasUsed,
      })

      return tx.transactionHash
    } catch (err) {
      const errMsg = err instanceof Error ? err.message : String(err)

      // Don't retry if attestation already exists
      if (errMsg.includes('attestation already submitted')) {
        logger.info(LOG, `Attestation already exists for proposal ${result.proposalId} — skipping`)
        return null
      }

      logger.error(LOG, `Failed to submit attestation: ${errMsg}`, {
        proposalId: result.proposalId,
      })
      this.queue.push(result)
      return null
    }
  }

  async drainQueue(): Promise<void> {
    if (this.processing || this.queue.length === 0) return
    this.processing = true

    logger.info(LOG, `Draining attestation queue (${this.queue.length} pending)`)

    const items = [...this.queue]
    this.queue = []

    for (const item of items) {
      await this.submit(item)
      // Small delay between TXs to avoid sequence errors
      await new Promise(resolve => setTimeout(resolve, 1000))
    }

    this.processing = false
  }

  getQueueSize(): number {
    return this.queue.length
  }

  getAddress(): string | null {
    return this.address
  }
}
