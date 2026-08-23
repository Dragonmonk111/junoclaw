/**
 * publish-article.mjs — Post the "Two Hidden Contracts" article to Moultbook
 * as a public commitment, making it permanently referenceable on-chain.
 *
 * Usage:
 *   CONFIRM=yes node scripts/publish-article.mjs
 */

import { createHash } from 'crypto'
import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href)
}
function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

const RPC = 'https://juno.rpc.t.stavr.tech'
const MOULTBOOK_ADDR = 'juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4'
const ARTICLE_PATH = join(__dirname, '..', 'articles', 'TWO_HIDDEN_CONTRACTS_2026_08_23.md')
const CONFIRMED = process.env.CONFIRM === 'yes'

const { DirectSecp256k1HdWallet } = await cosmImport('proto-signing')
const { SigningCosmWasmClient } = await cosmImport('cosmwasm-stargate')
const { GasPrice } = await cosmImport('stargate')

const store = await distImport('wallet', 'store.js')
const ws = store.getDefaultWalletStore()

async function getSigner(walletId) {
  const mnemonic = await ws.exportMnemonicForExternalSigner(walletId)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })
  return { client, address: acc.address }
}

const FEE = (gas, amount) => ({ amount: [{ denom: 'ujunox', amount: String(amount) }], gas: String(gas) })

const article = readFileSync(ARTICLE_PATH, 'utf8')
const commitment = Buffer.from(createHash('sha256').update(article, 'utf8').digest())
const sizeBytes = Buffer.byteLength(article, 'utf8')

console.log('╔══════════════════════════════════════════════════════╗')
console.log('║  Publish "Two Hidden Contracts" to Moultbook          ║')
console.log('╚══════════════════════════════════════════════════════╝')
console.log('')
console.log('Mode:', CONFIRMED ? 'LIVE' : 'DRY RUN')
console.log('Article:', ARTICLE_PATH)
console.log('Size:', sizeBytes, 'bytes')
console.log('Commitment:', commitment.toString('hex'))
console.log('')

if (!CONFIRMED) {
  console.log('Dry run — nothing broadcast.')
  console.log('To execute: CONFIRM=yes node scripts/publish-article.mjs')
  process.exit(0)
}

const { client, address } = await getSigner('builder')
console.log('Publisher:', address)

const moultMsg = {
  post: {
    commitment: commitment.toString('base64'),
    content_type: 'text/markdown+article',
    size_bytes: sizeBytes,
    attestation_ref: null,
    visibility: 'public',
    refs: [],
  },
}

const tx = await client.execute(
  address,
  MOULTBOOK_ADDR,
  moultMsg,
  FEE(300000, 45000),
  'Publish: The Two Hidden Contracts — machine-rwa + emergency-compute-escrow',
)
console.log('✓ Article tx:', tx.transactionHash)

const events = (tx.logs || []).flatMap((l) => l.events || []).concat(tx.events || [])
for (const ev of events) {
  if (ev.type === 'wasm') {
    const idAttr = ev.attributes.find((a) => a.key === 'id')
    if (idAttr?.value) console.log('  moult_id:', idAttr.value)
  }
}

console.log('')
console.log('Article published to Moultbook. Reference this moult ID in the S6 proposal.')
