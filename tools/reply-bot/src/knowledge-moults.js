/**
 * Mint flow for the knowledge-moults contract (A18c-5).
 *
 * Mirrors moultbook.js's signing/broadcast pattern, but targets the
 * knowledge-moults contract instead of Moultbook. Permissionless mint —
 * any funded address may call it; `agent` is a self-declared alias.
 */

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'

export const KNOWLEDGE_MOULTS_ADDR =
  process.env.KNOWLEDGE_MOULTS_ADDR || 'juno1plgknktvv09c0tzfceeswunknu4m9msh7xrffh3wkx5cmez4xvwqllehyd'

const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC = process.env.JUNO_REPLY_BOT_MNEMONIC
const DRY_RUN = process.env.MOULTBOOK_DRY_RUN === 'true'

/**
 * Build the ExecuteMsg::Mint payload. Validates the same constraints the
 * contract enforces (non-empty agent/motive/summary) so bad input fails
 * fast, client-side, before ever reaching the chain.
 */
export function buildMintMsg({ agent, motive, knowledgeSummary, sourceMoults = [], owner = null }) {
  if (!agent || !agent.trim()) throw new Error('agent is required')
  if (!motive || !motive.trim()) throw new Error('motive is required')
  if (!knowledgeSummary || !knowledgeSummary.trim()) throw new Error('knowledgeSummary is required')

  const refs = (sourceMoults || []).map((r) => (r.startsWith('moult:') ? r : `moult:${r}`))

  return {
    mint: {
      agent,
      motive,
      knowledge_summary: knowledgeSummary,
      source_moults: refs,
      owner: owner || null,
    },
  }
}

function findMintedId(result) {
  // Contract emits: action=mint, id=kmoult:<hex>, agent, owner, minter.
  const eventSources = [
    ...(result.logs || []).flatMap((log) => log.events || []),
    ...(result.events || []),
  ]
  for (const event of eventSources) {
    if (event.type === 'wasm') {
      const action = event.attributes.find((a) => a.key === 'action')
      const idAttr = event.attributes.find((a) => a.key === 'id')
      if (action?.value === 'mint' && idAttr?.value?.startsWith('kmoult:')) {
        return idAttr.value
      }
    }
  }
  return null
}

export async function mintKnowledgeMoult({ agent, motive, knowledgeSummary, sourceMoults, owner }) {
  const msg = buildMintMsg({ agent, motive, knowledgeSummary, sourceMoults, owner })

  let sender = null
  let wallet = null
  if (MNEMONIC) {
    wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [account] = await wallet.getAccounts()
    sender = account.address
  }

  if (DRY_RUN) {
    console.log(`[knowledge-moults] dry-run: would Mint on ${KNOWLEDGE_MOULTS_ADDR}${sender ? ` from ${sender}` : ''}`)
    console.log(`[knowledge-moults] dry-run: msg ${JSON.stringify(msg)}`)
    return { txHash: null, moultId: null, owner: owner || sender, dryRun: true }
  }

  if (!MNEMONIC) {
    throw new Error('Minting requires JUNO_REPLY_BOT_MNEMONIC to be set')
  }

  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, { gasPrice })
  const result = await client.execute(
    sender,
    KNOWLEDGE_MOULTS_ADDR,
    msg,
    'auto',
    `Knowledge Moult mint: ${motive}`,
  )
  const moultId = findMintedId(result)
  if (!moultId) {
    console.warn('[knowledge-moults] could not parse kmoult:id from tx result:', JSON.stringify(result.logs))
  }

  return { txHash: result.transactionHash, moultId, owner: owner || sender, dryRun: false }
}
