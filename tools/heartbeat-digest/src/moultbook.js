/**
 * B3 Phase 2 — Moultbook posting helper.
 *
 * Signs and broadcasts a `Post` execute message to the DAO-owned Moultbook
 * contract. The digest markdown is the content; its SHA-256 is the commitment.
 *
 * The module is disabled by default. The watcher only calls it when
 * POST_TO_MOULTBOOK=true is set. Use MOULTBOOK_DRY_RUN=true to simulate a post
 * without broadcasting (no funds spent, no entry created).
 */

import { createHash } from 'crypto'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'

// ── Configuration ───────────────────────────────────────────────────────────

export const MOULTBOOK_ADDR =
  process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC_ENV = process.env.JUNO_AGENT_MNEMONIC_ENV || 'JUNO_AGENT_MNEMONIC'
const MNEMONIC = process.env[MNEMONIC_ENV] || process.env.JUNO_AGENT_MNEMONIC
const DRY_RUN = process.env.MOULTBOOK_DRY_RUN === 'true'

// ── Commitment ──────────────────────────────────────────────────────────────

export function commitmentFromMarkdown(markdown) {
  const raw = createHash('sha256').update(markdown, 'utf8').digest()
  return Buffer.from(raw).toString('base64')
}

export function sizeBytes(markdown) {
  return Buffer.byteLength(markdown, 'utf8')
}

// ── Message builder ─────────────────────────────────────────────────────────

export function buildPostMsg(markdown, previousMoultId = null) {
  const refs = previousMoultId ? [previousMoultId] : []
  return {
    post: {
      commitment: commitmentFromMarkdown(markdown),
      content_type: 'application/markdown+heartbeat',
      size_bytes: sizeBytes(markdown),
      attestation_ref: null,
      visibility: 'public',
      refs,
    },
  }
}

// ── Event parsing ────────────────────────────────────────────────────────────

function findMoultId(result) {
  // CosmJS returns parsed logs by default. Look for the wasm event emitted by
  // the contract: action=post, id=moult:..., author=...
  const logs = result.logs || []
  for (const log of logs) {
    for (const event of log.events || []) {
      if (event.type === 'wasm') {
        const action = event.attributes.find((a) => a.key === 'action')
        const idAttr = event.attributes.find((a) => a.key === 'id')
        if (action?.value === 'post' && idAttr?.value?.startsWith('moult:')) {
          return idAttr.value
        }
      }
    }
  }
  return null
}

// ── Post ─────────────────────────────────────────────────────────────────────

export async function postDigestToMoultbook(markdown, previousMoultId = null) {
  const msg = buildPostMsg(markdown, previousMoultId)

  let sender = null
  let wallet = null
  if (MNEMONIC) {
    wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    sender = account.address
  }

  if (DRY_RUN) {
    console.log(`[moultbook] dry-run: would Post to ${MOULTBOOK_ADDR}${sender ? ` from ${sender}` : ''}`)
    console.log(`[moultbook] dry-run: commitment ${msg.post.commitment.slice(0, 24)}...`)
    console.log(`[moultbook] dry-run: size ${msg.post.size_bytes} bytes`)
    console.log(`[moultbook] dry-run: refs ${JSON.stringify(msg.post.refs)}`)
    return {
      txHash: null,
      moultId: null,
      author: sender,
      dryRun: true,
    }
  }

  if (!MNEMONIC) {
    throw new Error(
      `Moultbook posting requires a mnemonic. Set ${MNEMONIC_ENV} or JUNO_AGENT_MNEMONIC.`,
    )
  }

  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, {
    gasPrice,
  })

  const result = await client.execute(sender, MOULTBOOK_ADDR, msg, 'auto', 'heartbeat digest')
  const moultId = findMoultId(result)
  if (!moultId) {
    console.warn('[moultbook] could not parse moult:id from tx result:', JSON.stringify(result.logs))
  }

  return {
    txHash: result.transactionHash,
    moultId,
    author: sender,
    dryRun: false,
  }
}
