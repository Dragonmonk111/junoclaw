/**
 * setup-relayer-wallet.mjs — Create and fund a relayer wallet for the soak test.
 * Exports the mnemonic to a temp file that the soak script can read.
 */

import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'
import { writeFileSync, mkdirSync } from 'fs'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href)
}
function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

const RPC = 'https://juno.rpc.t.stavr.tech'
const WALLET_ID = 'relayer-soak'
const FUND_AMOUNT = '5000000' // 5 JUNOX

const { DirectSecp256k1HdWallet } = await cosmImport('proto-signing')
const { SigningCosmWasmClient } = await cosmImport('cosmwasm-stargate')
const { GasPrice } = await cosmImport('stargate')

const store = await distImport('wallet', 'store.js')
const ws = store.getDefaultWalletStore()

const FEE = (gas, amount) => ({ amount: [{ denom: 'ujunox', amount: String(amount) }], gas: String(gas) })

// Check if relayer wallet exists
let relayerAddr = null
try {
  const mnemonic = await ws.exportMnemonicForExternalSigner(WALLET_ID)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  relayerAddr = acc.address
  console.log('Relayer wallet exists:', relayerAddr)
} catch {
  console.log('Creating relayer wallet...')
  const entry = await ws.generateAndAdd(WALLET_ID, {
    bech32Prefix: 'juno',
    backend: 'keychain',
    wordCount: 24,
  })
  relayerAddr = entry.address
  console.log('Created relayer wallet:', relayerAddr)
}

// Fund it from builder
async function getSigner(walletId) {
  const mnemonic = await ws.exportMnemonicForExternalSigner(walletId)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })
  return { client, address: acc.address }
}

const { client: builderClient, address: builderAddr } = await getSigner('builder')

const bal = await builderClient.getBalance(relayerAddr, 'ujunox')
if (BigInt(bal.amount) < 1000000n) {
  console.log(`Funding relayer with ${FUND_AMOUNT} ujunox...`)
  const tx = await builderClient.sendTokens(
    builderAddr, relayerAddr,
    [{ denom: 'ujunox', amount: FUND_AMOUNT }],
    FEE(200000, 30000),
    'Fund relayer-soak wallet for 6-layer soak test',
  )
  console.log('Funded, tx:', tx.transactionHash)
} else {
  console.log('Already funded:', bal.amount, 'ujunox')
}

// Export mnemonic to a gitignored file for the soak script
const mnemonic = await ws.exportMnemonicForExternalSigner(WALLET_ID)
const soakDir = join(__dirname, '..', 'soak-logs')
mkdirSync(soakDir, { recursive: true })
const keyFile = join(soakDir, '.relayer-key')
writeFileSync(keyFile, mnemonic, { mode: 0o600 })
console.log('Relayer mnemonic written to:', keyFile)
console.log('Relayer address:', relayerAddr)
