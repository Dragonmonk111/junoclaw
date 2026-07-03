// Publish the genesis Mother-Moult as a Moultbook entry.
//
// SAFETY: this is a real mainnet transaction representing the DAO's canonical
// constitution. It defaults to dry-run no matter what. To actually broadcast,
// you must set BOTH:
//   JUNO_REPLY_BOT_MNEMONIC (or JUNO_MOTHER_MOULT_MNEMONIC) — a funded wallet
//   PUBLISH_MOTHER_MOULT_CONFIRM=yes                        — explicit opt-in
//
// Usage (dry-run, safe, default):
//   node scripts/publish-mother-moult.js
//
// Usage (real broadcast):
//   PUBLISH_MOTHER_MOULT_CONFIRM=yes JUNO_REPLY_BOT_MNEMONIC="..." node scripts/publish-mother-moult.js

import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { createHash } from 'crypto'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MOTHER_MOULT_FILE = join(__dirname, '..', 'mother-moult.json')

const MOULTBOOK_ADDR = process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'
const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC = process.env.JUNO_MOTHER_MOULT_MNEMONIC || process.env.JUNO_REPLY_BOT_MNEMONIC
const CONFIRMED = process.env.PUBLISH_MOTHER_MOULT_CONFIRM === 'yes'

function sha256Base64(text) {
  return Buffer.from(createHash('sha256').update(text, 'utf8').digest()).toString('base64')
}

function sizeBytes(text) {
  return Buffer.byteLength(text, 'utf8')
}

function findMoultId(result) {
  const eventSources = [
    ...(result.logs || []).flatMap((log) => log.events || []),
    ...(result.events || []),
  ]
  for (const event of eventSources) {
    if (event.type === 'wasm') {
      const action = event.attributes.find((a) => a.key === 'action')
      const idAttr = event.attributes.find((a) => a.key === 'id')
      if (action?.value === 'post' && idAttr?.value?.startsWith('moult:')) {
        return idAttr.value
      }
    }
  }
  return null
}

async function main() {
  const raw = readFileSync(MOTHER_MOULT_FILE, 'utf8')
  const motherMoult = JSON.parse(raw)
  const payload = JSON.stringify(motherMoult)

  const msg = {
    post: {
      commitment: sha256Base64(payload),
      content_type: 'application/json+mother-moult',
      size_bytes: sizeBytes(payload),
      attestation_ref: null,
      visibility: 'public',
      refs: [],
    },
  }

  console.log('[publish-mother-moult] mother-moult.json version:', motherMoult.version)
  console.log('[publish-mother-moult] commitment:', msg.post.commitment)
  console.log('[publish-mother-moult] size_bytes:', msg.post.size_bytes)
  console.log('[publish-mother-moult] contract:', MOULTBOOK_ADDR)

  if (!MNEMONIC || !CONFIRMED) {
    console.log('\n[publish-mother-moult] DRY RUN — nothing broadcast.')
    if (!MNEMONIC) console.log('  reason: no JUNO_MOTHER_MOULT_MNEMONIC / JUNO_REPLY_BOT_MNEMONIC set')
    if (!CONFIRMED) console.log('  reason: PUBLISH_MOTHER_MOULT_CONFIRM=yes not set')
    console.log('\n  would execute:')
    console.log('  ' + JSON.stringify(msg, null, 2).split('\n').join('\n  '))
    return { dryRun: true, msg }
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [account] = await wallet.getAccounts()
  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, { gasPrice })

  const result = await client.execute(account.address, MOULTBOOK_ADDR, msg, 'auto', 'Publish genesis Mother-Moult (A18c-4)')
  const moultId = findMoultId(result)

  console.log('\n[publish-mother-moult] BROADCAST SUCCEEDED')
  console.log('  tx_hash:', result.transactionHash)
  console.log('  moult_id:', moultId)
  console.log('\n  Next: update mother-moult.json with this tx_hash and bump "status" to reflect it is now on-chain.')

  return { dryRun: false, txHash: result.transactionHash, moultId }
}

main().catch((e) => {
  console.error('[publish-mother-moult] failed:', e.message)
  process.exit(1)
})
