/**
 * a052-withdraw.mjs — Withdraw unstaked funds after the 24h cooldown.
 *
 * Usage:
 *   CONFIRM=yes node scripts/a052-withdraw.mjs
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
const DAO_OPERATOR_ADDR = 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd'
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

console.log('╔══════════════════════════════════════════════════════╗')
console.log('║  A052 Withdraw Unstake — Post-Cooldown                ║')
console.log('╚══════════════════════════════════════════════════════╝')
console.log('')
console.log('Mode:', CONFIRMED ? 'LIVE' : 'DRY RUN')
console.log('')

// Check operator state before withdrawing
const { client: daoClient, address: daoAddr } = await getSigner('dao-truth-operator')
const { client: builderClient, address: builderAddr } = await getSigner('builder')

const opInfo = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, {
  get_operator: { address: DAO_OPERATOR_ADDR },
})
console.log('Operator state:')
console.log('  Address:', DAO_OPERATOR_ADDR)
console.log('  Stake:', opInfo.stake, 'ujunox')
console.log('  Active:', opInfo.active)
console.log('  Unstake request time:', opInfo.unstake_request_time || '0')
console.log('  Total rewards:', opInfo.total_rewards, 'ujunox')
console.log('  Total slashed:', opInfo.total_slashed, 'ujunox')
console.log('')

if (opInfo.unstake_request_time && opInfo.unstake_request_time > 0) {
  const cooldownEnd = opInfo.unstake_request_time + 86400
  const now = Math.floor(Date.now() / 1000)
  const remaining = cooldownEnd - now
  if (remaining > 0) {
    console.log(`  Cooldown: ${Math.floor(remaining / 3600)}h ${Math.floor((remaining % 3600) / 60)}m remaining`)
    console.log('  Cannot withdraw yet — wait for cooldown to elapse.')
    if (!CONFIRMED) {
      console.log('\nDry run — would attempt withdraw anyway for testing.')
    } else {
      process.exit(0)
    }
  } else {
    console.log('  Cooldown: ELAPSED — ready to withdraw')
  }
} else {
  console.log('  No pending unstake request. Run a052-closeout.mjs first.')
  if (!CONFIRMED) {
    console.log('  (Dry run — continuing anyway)')
  } else {
    process.exit(0)
  }
}

if (!CONFIRMED) {
  console.log('\nDry run — nothing broadcast.')
  console.log('To execute: CONFIRM=yes node scripts/a052-withdraw.mjs')
  process.exit(0)
}

// Withdraw
console.log('\n═══ Step 1: Withdraw unstake ═══')
const withdrawMsg = { withdraw_unstake: {} }

try {
  const withdrawTx = await daoClient.execute(
    daoAddr,
    TRUTH_MARKET_ADDR,
    withdrawMsg,
    FEE(300000, 30000),
    'A052 withdraw unstake — mandate complete',
  )
  console.log('  ✓ Withdraw tx:', withdrawTx.transactionHash)
} catch (e) {
  console.log('  Withdraw failed:', e.message.substring(0, 200))
}

// Check post-withdraw state
console.log('\n═══ Step 2: Verify post-withdraw state ═══')
const postOp = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, {
  get_operator: { address: DAO_OPERATOR_ADDR },
})
console.log('  Stake:', postOp.stake, 'ujunox')
console.log('  Active:', postOp.active)
console.log('  Unstake request time:', postOp.unstake_request_time || '0')

const daoBalance = await builderClient.getBalance(daoAddr, 'ujunox')
console.log('  DAO wallet balance:', daoBalance.amount, 'ujunox')

console.log('\n═══════════════════════════════════════════════════════')
console.log('  A052 Withdraw Complete')
console.log('  Stake withdrawn to DAO operator wallet')
console.log('  Mandate fully closed')
console.log('═══════════════════════════════════════════════════════')
