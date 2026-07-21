import { createHash } from 'crypto'
import { writeFileSync, mkdirSync } from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'
import { SigningCosmWasmClient, CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice, StargateClient } from '@cosmjs/stargate'

const __dirname = dirname(fileURLToPath(import.meta.url))
// Where AKB export payload text is mirrored so context-agent's resolvers can
// recover it later (Moultbook only stores the commitment hash, see ADR-002 /
// akb-spec.md's "whoever originates an export is responsible for keeping the
// plaintext resolvable"). Same pattern tools/heartbeat-digest already uses
// via its GitHub mirror, generalized here to every AKB export this bot posts.
const EXPORTS_DIR = join(__dirname, '..', 'exports')

/**
 * Persist the exact payload text that was hashed into an export's on-chain
 * commitment, keyed by moult id, so a content resolver can fetch it later
 * (e.g. via the GitHub raw mirror of this repo) and verify it still matches.
 * Best-effort: a failure here must never fail the broadcast that already
 * succeeded on-chain.
 */
export function mirrorExportToFile(moultId, text) {
  try {
    mkdirSync(EXPORTS_DIR, { recursive: true })
    const idHex = moultId.startsWith('moult:') ? moultId.slice('moult:'.length) : moultId
    writeFileSync(join(EXPORTS_DIR, `${idHex}.json`), JSON.stringify({ moult_id: moultId, text }, null, 2))
  } catch (e) {
    console.warn('[reply-bot] could not mirror export to file:', e.message)
  }
}

export const MOULTBOOK_ADDR =
  process.env.MOULTBOOK_ADDR || 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC = process.env.JUNO_REPLY_BOT_MNEMONIC
const DRY_RUN = process.env.MOULTBOOK_DRY_RUN === 'true'

const AGENT_COMPANY_ADDR = process.env.AGENT_COMPANY_ADDR
const SEALED_SIGNER_ADDRESS = process.env.SEALED_SIGNER_ADDRESS

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
  } else {
    // msg.post.commitment was computed by buildAkbExportPost as
    // sha256Base64(JSON.stringify(stamped, null, 2)) — recompute the same
    // payload text here (pure, deterministic) so a resolver can later
    // recover and verify it, exactly like the heartbeat digest's GitHub mirror.
    mirrorExportToFile(moultId, JSON.stringify(stamped, null, 2))
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

export function parseSignRequestId(result) {
  const eventSources = [
    ...(result.logs || []).flatMap((log) => log.events || []),
    ...(result.events || []),
  ]
  for (const event of eventSources) {
    if (event.type === 'wasm-sign_request' || event.type === 'sign_request') {
      const idAttr = event.attributes.find((a) => a.key === 'id')
      if (idAttr?.value && /^\d+$/.test(idAttr.value)) {
        return Number(idAttr.value)
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

// ── Sealed-signer relayer helpers (M2) ──
// These functions replace the plaintext-mnemonic signing path with a TEE-backed
// signer. The relayer wallet only needs to be configured as the `relayer` role
// in agent-company; it never sees the enclave signing key.

export async function getSealedSignerAccountInfo() {
  if (!SEALED_SIGNER_ADDRESS) throw new Error('SEALED_SIGNER_ADDRESS not set')
  const client = await StargateClient.connect(RPC_ENDPOINT)
  try {
    const account = await client.getAccount(SEALED_SIGNER_ADDRESS)
    if (!account) throw new Error(`Sealed signer account not found: ${SEALED_SIGNER_ADDRESS}`)
    return {
      address: SEALED_SIGNER_ADDRESS,
      accountNumber: account.accountNumber,
      sequence: account.sequence,
    }
  } finally {
    await client.disconnect()
  }
}

export function buildRequestSignedTx(accountInfo, execMsg, opts = {}) {
  return {
    sender: accountInfo.address,
    contract: MOULTBOOK_ADDR,
    exec_msg_json: JSON.stringify(execMsg),
    funds_denom: opts.fundsDenom || '',
    funds_amount: opts.fundsAmount || '0',
    gas_limit: opts.gasLimit || 200_000,
    fee_denom: opts.feeDenom || 'ujuno',
    fee_amount: opts.feeAmount || '5000',
    memo: opts.memo || 'sealed signer Moultbook post',
    chain_id: opts.chainId || 'uni-7',
    account_number: accountInfo.accountNumber,
    sequence: accountInfo.sequence,
  }
}

export async function requestSignedTx(client, sender, request) {
  if (!AGENT_COMPANY_ADDR) throw new Error('AGENT_COMPANY_ADDR not set')
  return client.execute(
    sender,
    AGENT_COMPANY_ADDR,
    { request_signed_tx: request },
    'auto',
    `sealed signer request ${request.id}`
  )
}

export async function pollSignRequest(id, opts = {}) {
  if (!AGENT_COMPANY_ADDR) throw new Error('AGENT_COMPANY_ADDR not set')
  const { maxAttempts = 30, delayMs = 2000 } = opts
  const client = await CosmWasmClient.connect(RPC_ENDPOINT)
  try {
    for (let i = 0; i < maxAttempts; i++) {
      const req = await client.queryContractSmart(AGENT_COMPANY_ADDR, {
        get_sign_request: { id },
      })
      if (req && req.status && 'signed' in req.status && req.tx_bytes) {
        return req
      }
      await new Promise((resolve) => setTimeout(resolve, delayMs))
    }
    throw new Error(`Timed out waiting for sign request ${id} to be signed`)
  } finally {
    await client.disconnect()
  }
}

export async function broadcastTxBytes(txBytesBase64) {
  const client = await StargateClient.connect(RPC_ENDPOINT)
  try {
    const txBytes = Buffer.from(txBytesBase64, 'base64')
    return await client.broadcastTx(txBytes)
  } finally {
    await client.disconnect()
  }
}

export async function ackBroadcastTx(client, sender, id) {
  if (!AGENT_COMPANY_ADDR) throw new Error('AGENT_COMPANY_ADDR not set')
  return client.execute(
    sender,
    AGENT_COMPANY_ADDR,
    { ack_broadcast_tx: { id } },
    'auto',
    `ack broadcast ${id}`
  )
}

// ── Off-chain invoke API path (M2.1 prototype) ──
// These functions use the WAVS invoke HTTP endpoint to call the sealed
// signer component directly, bypassing the on-chain round-trip entirely.
// The flow collapses from 7 steps (request → poll → broadcast → ack) to 3:
//   1. Fetch account info (sequence)
//   2. POST /invoke/sealed-signer → get tx_bytes + attestation
//   3. Broadcast tx_bytes on-chain
//
// The invoke server must be running (wavs/bridge/src/invoke-server.ts).
// Configure with:
//   WAVS_INVOKE_URL      — invoke server base URL (e.g. http://127.0.0.1:PORT)
//   WAVS_INVOKE_TOKEN    — bearer token (must match server)
//   WAVS_INVOKE_SEALED_BLOB — hex-encoded sealed blob (must match server)

const INVOKE_URL = process.env.WAVS_INVOKE_URL || ''
const INVOKE_TOKEN = process.env.WAVS_INVOKE_TOKEN || ''
const INVOKE_SEALED_BLOB = process.env.WAVS_INVOKE_SEALED_BLOB || ''

/**
 * Call the WAVS invoke API to sign a Cosmos execute tx inside the TEE.
 * Returns { tx_bytes, sign_doc_sha256_hex, address, pubkey } synchronously.
 *
 * This replaces the entire RequestSignedTx → StoreSignedTx → poll → broadcast
 * round-trip with a single HTTP call.
 */
export async function invokeSealedSigner(accountInfo, execMsg, opts = {}) {
  if (!INVOKE_URL) throw new Error('WAVS_INVOKE_URL not set')
  if (!INVOKE_TOKEN) throw new Error('WAVS_INVOKE_TOKEN not set')

  const requestBody = {
    trigger: 'sign_request',
    input: {
      sender: accountInfo.address,
      contract: MOULTBOOK_ADDR,
      exec_msg_json: JSON.stringify(execMsg),
      funds_denom: opts.fundsDenom || '',
      funds_amount: opts.fundsAmount || '0',
      gas_limit: opts.gasLimit || 200_000,
      fee_denom: opts.feeDenom || 'ujuno',
      fee_amount: opts.feeAmount || '5000',
      memo: opts.memo || 'sealed signer Moultbook post (invoke)',
      chain_id: opts.chainId || 'uni-7',
      account_number: accountInfo.accountNumber,
      sequence: accountInfo.sequence,
    },
  }

  const resp = await fetch(`${INVOKE_URL}/invoke/sealed-signer`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${INVOKE_TOKEN}`,
    },
    body: JSON.stringify(requestBody),
  })

  if (!resp.ok) {
    const text = await resp.text().catch(() => '<no body>')
    throw new Error(`invoke failed (${resp.status}): ${text}`)
  }

  const result = await resp.json()
  if (!result.output?.tx_bytes) {
    throw new Error(`invoke returned no tx_bytes: ${JSON.stringify(result)}`)
  }

  return {
    txBytes: result.output.tx_bytes,
    signDocSha256Hex: result.output.sign_doc_sha256_hex,
    address: result.output.address,
    pubkey: result.output.pubkey,
    attestationHash: result.attestation?.attestation_hash || result.output.sign_doc_sha256_hex,
  }
}

/**
 * Post to Moultbook using the off-chain invoke API path.
 *
 * This is the simplified sealed signer flow:
 *   1. Get sealed signer account info (account number + sequence)
 *   2. Invoke the sealed signer via HTTP → get signed tx_bytes
 *   3. Broadcast the signed tx_bytes on-chain
 *
 * No RequestSignedTx, no StoreSignedTx, no AckBroadcastTx, no polling.
 * The relayer wallet is not needed — the tx is already signed by the enclave.
 */
export async function postViaSealedSignerInvoke(execMsg, opts = {}) {
  // 1. Get account info for the sealed signer address
  const accountInfo = await getSealedSignerAccountInfo()

  // 2. Invoke the sealed signer component via HTTP
  const signed = await invokeSealedSigner(accountInfo, execMsg, opts)
  console.log(`[reply-bot] invoke signed tx for ${signed.address} (signDoc=${signed.signDocSha256Hex.slice(0, 16)}...)`)

  // 3. Broadcast the signed transaction
  const broadcastResult = await broadcastTxBytes(signed.txBytes)
  console.log(`[reply-bot] broadcast tx: ${broadcastResult.transactionHash}`)

  return {
    txHash: broadcastResult.transactionHash,
    signDocSha256Hex: signed.signDocSha256Hex,
    attestationHash: signed.attestationHash,
    address: signed.address,
    invokeMode: true,
  }
}
