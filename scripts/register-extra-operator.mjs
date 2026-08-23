/**
 * register-extra-operator.mjs — Create and register a 5th operator so we have
 * enough to meet min_operators=3 for epoch finalization.
 * The Technocrat and Contrarian wallets are in parliament-state.json (gitignored)
 * and not in the WalletStore, so we need a new one.
 */

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
const TRUTH_MARKET_ADDR = 'juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p'
const CONFIRMED = process.env.CONFIRM === 'yes'

const { DirectSecp256k1HdWallet } = await cosmImport('proto-signing')
const { SigningCosmWasmClient } = await cosmImport('cosmwasm-stargate')
const { GasPrice } = await cosmImport('stargate')
const { WalletStore } = await distImport('wallet', 'store.js')
const { PassphraseKeyStore, defaultPassphraseSource } = await distImport('wallet', 'key-store.js')

const FEE = (gas, amount) => ({ amount: [{ denom: 'ujunox', amount: String(amount) }], gas: String(gas) })

// ─── Create wallet ───────────────────────────────────────────────────────────
const root = WalletStore.defaultRoot()
const backends = new Map()
backends.set('passphrase', new PassphraseKeyStore(root, defaultPassphraseSource()))
let preferredBackend = 'passphrase'
try {
  const { keychainKeyStore } = await distImport('wallet', 'keychain-store.js')
  const ks = await keychainKeyStore()
  backends.set('keychain', ks)
  preferredBackend = 'keychain'
} catch {}

const WALLET_ID = 'dao-verdict-helper'

// Check if already exists
let helperAddress = null
try {
  const store = new WalletStore(root, backends, preferredBackend)
  const mnemonic = await store.exportMnemonicForExternalSigner(WALLET_ID)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  helperAddress = acc.address
  console.log('Helper wallet already exists:', helperAddress)
} catch {}

if (!helperAddress) {
  if (!CONFIRMED) {
    console.log('DRY RUN: would create wallet', WALLET_ID)
    process.exit(0)
  }
  const store = new WalletStore(root, backends, preferredBackend)
  const entry = await store.generateAndAdd(WALLET_ID, {
    bech32Prefix: 'juno',
    backend: preferredBackend,
    wordCount: 24,
  })
  const verified = await store.verifyAddress(WALLET_ID)
  if (verified !== entry.address) throw new Error('Verification mismatch')
  helperAddress = entry.address
  console.log('Created helper wallet:', helperAddress)
}

// ─── Fund and register ───────────────────────────────────────────────────────
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

const { client: builderClient, address: builderAddr } = await getSigner('builder')

// Check balance
const bal = await builderClient.getBalance(helperAddress, 'ujunox')
if (BigInt(bal.amount) < 1000000n) {
  console.log('Funding helper with 1.5 JUNOX...')
  if (CONFIRMED) {
    await builderClient.sendTokens(
      builderAddr, helperAddress,
      [{ denom: 'ujunox', amount: '1500000' }],
      FEE(200000, 30000),
      'Fund dao-verdict-helper',
    )
    console.log('Funded')
  }
} else {
  console.log('Already funded:', bal.amount, 'ujunox')
}

// Check if already registered
const ops = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, { list_operators: {} })
const alreadyReg = (ops.operators || []).some(o => o.address === helperAddress)
if (alreadyReg) {
  console.log('Already registered as operator')
} else {
  console.log('Registering as operator...')
  if (CONFIRMED) {
    const { client: helperClient, address: helperAddr } = await getSigner(WALLET_ID)
    const regTx = await helperClient.execute(
      helperAddr, TRUTH_MARKET_ADDR,
      { register_operator: { fingerprint: 'dao-verdict-helper' } },
      FEE(400000, 50000),
      'Register helper operator',
      [{ denom: 'ujunox', amount: '1000000' }],
    )
    console.log('Registered, tx:', regTx.transactionHash)
  }
}

console.log('\nHelper operator ready:', helperAddress)
console.log('Use BATCH_HEIGHT=6 VERDICT_HELPER=' + helperAddress + ' to include in verdicts')
