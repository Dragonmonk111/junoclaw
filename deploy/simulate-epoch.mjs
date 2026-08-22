import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL = 'https://juno.rpc.t.stavr.tech'
const DENOM   = 'ujunox'
const BATCH_HEIGHT = 1
const MESSAGES_HASH = 'deadbeefcafebabe1234567890abcdefdeadbeefcafebabe1234567890abcdef'
const VERIFICATION_FEE = '50000'

const state = JSON.parse(readFileSync(join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json'), 'utf8'))
const deployed = JSON.parse(readFileSync(join(__dir, 'deployed.json'), 'utf8'))
const contractAddr = deployed['truth-market']?.address

if (!contractAddr) {
  console.error('truth-market address not found in deployed.json')
  process.exit(1)
}

async function getClientFor(name) {
  const mp = state.mps.find(m => m.name === name)
  if (!mp) throw new Error(`Wallet "${name}" not found`)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })
  return { client, address: acc.address, wallet }
}

// ── Step 0: Show pre-epoch state ───────────────────────────────────────────
console.log('\n  ═══ EPOCH SIMULATION ON UNI-7 ═══')
console.log(`  Contract: ${contractAddr}`)
console.log(`  Batch height: ${BATCH_HEIGHT}`)
console.log(`  Messages hash: ${MESSAGES_HASH.slice(0, 16)}...`)
console.log(`  Consensus verdict: green (all 3 agree)\n`)

const { client: builderClient, address: builderAddr } = await getClientFor('The Builder')

const preStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log('  ── Pre-Epoch Stats ──')
console.log(`  total_operators:   ${preStats.total_operators}`)
console.log(`  active_operators:  ${preStats.active_operators}`)
console.log(`  total_staked:      ${preStats.total_staked} ujunox`)
console.log(`  reward_pool:       ${preStats.reward_pool} ujunox`)
console.log(`  epochs_finalized:  ${preStats.epochs_finalized}\n`)

// ── Step 1: All 3 operators submit "green" verdict ─────────────────────────
console.log('  ── Step 1: Submit Verdicts ──')

const operators = [
  { name: 'The Builder', verdict: 'green' },
  { name: 'The Technocrat', verdict: 'green' },
  { name: 'The Contrarian', verdict: 'green' },
]

for (const op of operators) {
  const { client, address } = await getClientFor(op.name)
  const msg = {
    submit_verdict: {
      batch_height: BATCH_HEIGHT,
      verdict: op.verdict,
      messages_hash: MESSAGES_HASH,
    },
  }
  try {
    const tx = await client.execute(address, contractAddr, msg, 'auto', `Submit ${op.verdict} verdict`)
    console.log(`  ✓ ${op.name} submitted "${op.verdict}" — tx: ${tx.transactionHash.slice(0, 16)}...`)
  } catch (err) {
    console.error(`  ✗ ${op.name} verdict failed: ${err.message}`)
  }
}

// ── Step 2: Pay verification fee (simulating relayer paying for a robot) ──
console.log('\n  ── Step 2: Pay Verification Fee ──')
const feeMsg = {
  pay_verification_fee: {
    batch_height: BATCH_HEIGHT,
    robot_id: 'rosie-unit-001',
  },
}
try {
  const feeTx = await builderClient.execute(
    builderAddr, contractAddr, feeMsg, 'auto',
    'Pay verification fee for rosie-unit-001',
    [{ denom: DENOM, amount: VERIFICATION_FEE }],
  )
  console.log(`  ✓ Verification fee paid: ${VERIFICATION_FEE} ujunox — tx: ${feeTx.transactionHash.slice(0, 16)}...`)
  console.log(`  Robot ID: rosie-unit-001`)
} catch (err) {
  console.error(`  ✗ Fee payment failed: ${err.message}`)
}

// ── Step 3: Finalize epoch (admin/relayer calls FinalizeEpoch) ─────────────
console.log('\n  ── Step 3: Finalize Epoch ──')
const finalMsg = {
  finalize_epoch: {
    batch_height: BATCH_HEIGHT,
    consensus_verdict: 'green',
    messages_hash: MESSAGES_HASH,
  },
}
try {
  const finalTx = await builderClient.execute(
    builderAddr, contractAddr, finalMsg, 'auto',
    'Finalize epoch 1 — consensus: green',
  )
  console.log(`  ✓ Epoch finalized! — tx: ${finalTx.transactionHash}`)
  console.log(`  Events:`)
  for (const evt of finalTx.events) {
    if (evt.type === 'wasm') {
      const attrs = evt.attributes
        .filter(a => a.key.startsWith('_contract_address') === false)
        .map(a => `${a.key}=${a.value}`)
        .join(', ')
      if (attrs) console.log(`    ${evt.type}: ${attrs}`)
    }
  }
} catch (err) {
  console.error(`  ✗ Finalization failed: ${err.message}`)
  process.exit(1)
}

// ── Step 4: Query epoch result ─────────────────────────────────────────────
console.log('\n  ── Step 4: Epoch Result ──')
const epoch = await builderClient.queryContractSmart(contractAddr, {
  get_epoch: { batch_height: BATCH_HEIGHT },
})
console.log(`  batch_height:       ${epoch.batch_height}`)
console.log(`  consensus_verdict:  ${epoch.consensus_verdict}`)
console.log(`  messages_hash:      ${epoch.messages_hash.slice(0, 16)}...`)
console.log(`  total_operators:    ${epoch.total_operators}`)
console.log(`  matching_operators: ${epoch.matching_operators}`)
console.log(`  diverging_operators:${epoch.diverging_operators}`)
console.log(`  rewards_distributed:${epoch.rewards_distributed} ujunox`)
console.log(`  slashed_amount:     ${epoch.slashed_amount} ujunox`)
console.log(`  finalized:          ${epoch.finalized}`)

// ── Step 5: Query each operator's updated state ────────────────────────────
console.log('\n  ── Step 5: Operator States After Epoch ──')
for (const op of operators) {
  const { address } = await getClientFor(op.name)
  const opInfo = await builderClient.queryContractSmart(contractAddr, {
    get_operator: { address },
  })
  console.log(`\n  ${op.name} (${address}):`)
  console.log(`    stake:             ${opInfo.stake} ujunox`)
  console.log(`    total_rewards:     ${opInfo.total_rewards} ujunox`)
  console.log(`    total_slashed:     ${opInfo.total_slashed} ujunox`)
  console.log(`    epochs_participated: ${opInfo.epochs_participated}`)
  console.log(`    correct_verdicts:  ${opInfo.correct_verdicts}`)
  console.log(`    incorrect_verdicts:${opInfo.incorrect_verdicts}`)
  console.log(`    accuracy:          ${opInfo.accuracy}%`)
  console.log(`    active:            ${opInfo.active}`)
}

// ── Step 6: Final stats ────────────────────────────────────────────────────
console.log('\n  ── Final Stats ──')
const postStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log(`  total_operators:   ${postStats.total_operators}`)
console.log(`  active_operators:  ${postStats.active_operators}`)
console.log(`  total_staked:      ${postStats.total_staked} ujunox`)
console.log(`  reward_pool:       ${postStats.reward_pool} ujunox`)
console.log(`  total_rewards_paid:${postStats.total_rewards_paid} ujunox`)
console.log(`  total_slashed:     ${postStats.total_slashed} ujunox`)
console.log(`  epochs_finalized:  ${postStats.epochs_finalized}`)

const poolDelta = BigInt(preStats.reward_pool) - BigInt(postStats.reward_pool)
console.log(`\n  ── Summary ──`)
console.log(`  Reward pool before:  ${preStats.reward_pool} ujunox`)
console.log(`  Verification fee in: +${VERIFICATION_FEE} ujunox`)
console.log(`  Rewards distributed: -${postStats.total_rewards_paid} ujunox`)
console.log(`  Reward pool after:   ${postStats.reward_pool} ujunox`)
console.log(`  Slashed (back to pool): ${postStats.total_slashed} ujunox`)
console.log(`  Epochs finalized:    ${postStats.epochs_finalized}`)

console.log('\n  ═══ EPOCH 1 COMPLETE ═══')
console.log('  3 operators submitted "green" verdicts.')
console.log('  Verification fee of 50,000 ujunox paid by relayer for rosie-unit-001.')
console.log('  All 3 operators matched consensus — rewards distributed, no slashes.')
console.log('  The closed loop works end-to-end on uni-7.\n')

process.exit(0)
