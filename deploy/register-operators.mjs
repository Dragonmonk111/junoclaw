import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL = 'https://juno.rpc.t.stavr.tech'
const DENOM   = 'ujunox'
const STAKE   = '1000000' // 1 JUNOX = min_stake

const state = JSON.parse(readFileSync(join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json'), 'utf8'))
const deployed = JSON.parse(readFileSync(join(__dir, 'deployed.json'), 'utf8'))
const contractAddr = deployed['truth-market']?.address

if (!contractAddr) {
  console.error('truth-market address not found in deployed.json')
  process.exit(1)
}

// Register 2 more operators: The Technocrat (gpu miner) and The Contrarian (cloud miner)
const operatorsToRegister = [
  { name: 'The Technocrat', model: 'qwen-3b', hardware: 'jetson-orin', identityType: 'gpu' },
  { name: 'The Contrarian', model: 'mistral-7b', hardware: 'cloud', identityType: 'gpu' },
]

console.log(`\n  Registering 2 more operators on truth-market`)
console.log(`  Contract: ${contractAddr}\n`)

for (const op of operatorsToRegister) {
  const mp = state.mps.find(m => m.name === op.name)
  if (!mp) {
    console.error(`  Wallet "${op.name}" not found in parliament-state.json`)
    continue
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })

  const balance = await client.getBalance(acc.address, DENOM)
  console.log(`  ── ${op.name} ──`)
  console.log(`  Address: ${acc.address}`)
  console.log(`  Balance: ${(BigInt(balance.amount) / 1_000_000n).toString()} JUNOX`)

  const fingerprint = `${op.name.toLowerCase().replace(/\s+/g, '-')}-${op.model}-${op.hardware}`

  const regMsg = {
    register_operator: {
      fingerprint,
    },
  }

  try {
    const regTx = await client.execute(
      acc.address, contractAddr, regMsg, 'auto',
      `Register ${op.name} as truth-market operator`,
      [{ denom: DENOM, amount: STAKE }],
    )
    console.log(`  ✓ Registered, tx: ${regTx.transactionHash}`)
    console.log(`  Fingerprint: ${fingerprint}`)
    console.log(`  Stake: ${STAKE} ujunox\n`)
  } catch (err) {
    console.error(`  ✗ Registration failed: ${err.message}\n`)
  }
}

// ── Verify final state ─────────────────────────────────────────────────────
const builderWallet = await DirectSecp256k1HdWallet.fromMnemonic(
  state.mps.find(m => m.name === 'The Builder').mnemonic,
  { prefix: 'juno' },
)
const [builderAcc] = await builderWallet.getAccounts()
const verifyClient = await SigningCosmWasmClient.connectWithSigner(RPC_URL, builderWallet, {
  gasPrice: GasPrice.fromString('0.075ujunox'),
})

const stats = await verifyClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log('  ── Truth Market Stats (after) ──')
console.log(`  total_operators:   ${stats.total_operators}`)
console.log(`  active_operators:  ${stats.active_operators}`)
console.log(`  total_staked:      ${stats.total_staked} ujunox`)
console.log(`  reward_pool:       ${stats.reward_pool} ujunox`)
console.log(`  epochs_finalized:  ${stats.epochs_finalized}`)

const opsList = await verifyClient.queryContractSmart(contractAddr, { list_operators: {} })
console.log('\n  ── Registered Operators ──')
for (const op of opsList.operators) {
  console.log(`  ${op.address}`)
  console.log(`    stake: ${op.stake} ujunox, active: ${op.active}, fingerprint: ${op.fingerprint}`)
}

console.log(`\n  ═══ ${stats.total_operators} OPERATORS REGISTERED ═══`)
if (stats.total_operators >= 3) {
  console.log('  Min operators threshold met — epoch finalization is now possible!')
} else {
  console.log(`  Need ${3 - stats.total_operators} more operators to reach min_operators=3`)
}
console.log('')

process.exit(0)
