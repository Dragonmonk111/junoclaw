import { createHash } from 'crypto'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'

export const MOULTBOOK_ADDR =
  process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC = process.env.JUNO_REPLY_BOT_MNEMONIC
const DRY_RUN = process.env.MOULTBOOK_DRY_RUN === 'true'

export function sha256Base64(text) {
  return Buffer.from(createHash('sha256').update(text, 'utf8').digest()).toString('base64')
}

export function sizeBytes(text) {
  return Buffer.byteLength(text, 'utf8')
}

export function buildReplyPost(body, replyToMoultId, agentName = 'dragonmonk111-bot') {
  const text = typeof body === 'string' ? body : JSON.stringify(body, null, 2)
  const reply = {
    reply_to: replyToMoultId,
    agent: agentName,
    version: 'a18c-1',
    text,
  }
  const payload = JSON.stringify(reply, null, 2)
  return {
    post: {
      commitment: sha256Base64(payload),
      content_type: 'application/json+agent-reply',
      size_bytes: sizeBytes(payload),
      attestation_ref: null,
      visibility: 'public',
      refs: [replyToMoultId],
    },
  }
}

/**
 * Validate an AKB v1.0 export envelope and build a Moultbook post message.
 * The posted payload is the envelope itself; content_type is taken from the
 * envelope's content.mime_type (e.g. application/json+agent-insight or
 * application/json+redmark). This keeps the Moultbook entry self-describing
 * and AKB-compatible.
 */
export function buildAkbExportPost(envelope) {
  if (!envelope || typeof envelope !== 'object') throw new Error('envelope must be an object')
  if (envelope.direction !== 'export') throw new Error('envelope.direction must be "export"')
  if (!envelope.mother_moult_id) throw new Error('envelope.mother_moult_id is required')
  if (!envelope.author?.wallet) throw new Error('envelope.author.wallet is required')
  if (!envelope.author?.type) throw new Error('envelope.author.type is required')
  if (!envelope.content?.mime_type) throw new Error('envelope.content.mime_type is required')
  if (!envelope.content?.text) throw new Error('envelope.content.text is required')

  const payload = JSON.stringify(envelope, null, 2)
  const refs = (envelope.refs || []).map((r) => (r.startsWith('moult:') ? r : `moult:${r}`))
  return {
    post: {
      commitment: sha256Base64(payload),
      content_type: envelope.content.mime_type,
      size_bytes: sizeBytes(payload),
      attestation_ref: null,
      visibility: 'public',
      refs,
    },
  }
}

let _cachedSignerAddress
/**
 * Address of the wallet this bot signs with (derived from JUNO_REPLY_BOT_MNEMONIC),
 * or REPLY_BOT_WALLET, or null in pure dry-run. Cached after first derivation.
 * The server uses this to prefill an export envelope's author so the UI doesn't
 * have to know the bot's address.
 */
export async function getSignerAddress() {
  if (_cachedSignerAddress !== undefined) return _cachedSignerAddress
  if (MNEMONIC) {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    _cachedSignerAddress = account.address
  } else {
    _cachedSignerAddress = process.env.REPLY_BOT_WALLET || null
  }
  return _cachedSignerAddress
}

export async function postAkbExportToMoultbook(envelope) {
  let sender = null
  let wallet = null

  if (MNEMONIC) {
    wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    sender = account.address
  }

  // Provenance integrity: the on-chain author is always info.sender (the signing
  // wallet). Stamp the envelope's author.wallet to match so a consumer can never
  // be shown an envelope claiming a different author than the one that signed it.
  const stamped = sender
    ? { ...envelope, author: { ...(envelope.author || {}), wallet: sender, type: envelope.author?.type || 'agent' } }
    : envelope

  const msg = buildAkbExportPost(stamped)

  if (DRY_RUN) {
    console.log(`[reply-bot] dry-run: would Post to ${MOULTBOOK_ADDR}${sender ? ` from ${sender}` : ''}`)
    console.log(`[reply-bot] dry-run: content_type ${msg.post.content_type}`)
    console.log(`[reply-bot] dry-run: commitment ${msg.post.commitment.slice(0, 24)}...`)
    console.log(`[reply-bot] dry-run: refs ${JSON.stringify(msg.post.refs)}`)
    return { txHash: null, moultId: null, author: sender, dryRun: true }
  }

  if (!MNEMONIC) {
    throw new Error('Moultbook posting requires JUNO_REPLY_BOT_MNEMONIC to be set')
  }

  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, { gasPrice })
  const result = await client.execute(sender, MOULTBOOK_ADDR, msg, 'auto', `AKB export from ${stamped.author?.alias || sender}`)
  const moultId = findMoultId(result)
  if (!moultId) {
    console.warn('[reply-bot] could not parse moult:id from tx result:', JSON.stringify(result.logs))
  }

  return { txHash: result.transactionHash, moultId, author: sender, dryRun: false }
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

export async function postReplyToMoultbook(replyBody, replyToMoultId, agentName = 'dragonmonk111-bot') {
  if (!replyToMoultId) throw new Error('replyToMoultId is required')

  const msg = buildReplyPost(replyBody, replyToMoultId, agentName)
  let sender = null
  let wallet = null

  if (MNEMONIC) {
    wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    sender = account.address
  }

  if (DRY_RUN) {
    console.log(`[reply-bot] dry-run: would Post to ${MOULTBOOK_ADDR}${sender ? ` from ${sender}` : ''}`)
    console.log(`[reply-bot] dry-run: reply_to ${replyToMoultId}`)
    console.log(`[reply-bot] dry-run: commitment ${msg.post.commitment.slice(0, 24)}...`)
    console.log(`[reply-bot] dry-run: refs ${JSON.stringify(msg.post.refs)}`)
    return { txHash: null, moultId: null, author: sender, dryRun: true }
  }

  if (!MNEMONIC) {
    throw new Error('Moultbook posting requires JUNO_REPLY_BOT_MNEMONIC to be set')
  }

  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, { gasPrice })
  const result = await client.execute(sender, MOULTBOOK_ADDR, msg, 'auto', `A18c reply from ${agentName}`)
  const moultId = findMoultId(result)
  if (!moultId) {
    console.warn('[reply-bot] could not parse moult:id from tx result:', JSON.stringify(result.logs))
  }

  return { txHash: result.transactionHash, moultId, author: sender, dryRun: false }
}
