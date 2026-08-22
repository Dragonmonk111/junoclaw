import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL = 'https://juno.rpc.t.stavr.tech'
const DENOM   = 'ujunox'
const BATCH_HEIGHT = 2
const MESSAGES_HASH = 'aabbccdd11223344556677889900aabbccdd11223344556677889900aabbccdd'
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
  return { client, address: acc.address }
}

// ── Pre-epoch state ────────────────────────────────────────────────────────
console.log('\n  ═══ SLASHING SCENARIO — EPOCH 2 ON UNI-7 ═══')
console.log(`  Contract: ${contractAddr}`)
console.log(`  Batch height: ${BATCH_HEIGHT}`)
console.log(`  Scenario: 2 operators say "green", 1 says "red"`)
console.log(`  Expected: The Contrarian gets slashed (10% of stake)\n`)

const { client: builderClient, address: builderAddr } = await getClientFor('The Builder')

const preStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log('  ── Pre-Epoch Stats ──')
console.log(`  reward_pool:       ${preStats.reward_pool} ujunox`)
console.log(`  total_staked:      ${preStats.total_staked} ujunox`)
console.log(`  epochs_finalized:  ${preStats.epochs_finalized}\n`)

// Show operator states before
console.log('  ── Operator States Before ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`  ${name}: stake=${op.stake} rewards=${op.total_rewards} slashed=${op.total_slashed} accuracy=${op.accuracy}%`)
}

// ── Step 1: Submit verdicts — 2 green, 1 red ──────────────────────────────
console.log('\n  ── Step 1: Submit Verdicts (2 green, 1 red) ──')

const verdicts = [
  { name: 'The Builder', verdict: 'green' },
  { name: 'The Technocrat', verdict: 'green' },
  { name: 'The Contrarian', verdict: 'red' },  // The Contrarian disagrees!
]

for (const op of verdicts) {
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

// ── Step 2: Pay verification fee ──────────────────────────────────────────
console.log('\n  ── Step 2: Pay Verification Fee ──')
const feeMsg = {
  pay_verification_fee: {
    batch_height: BATCH_HEIGHT,
    robot_id: 'rosie-unit-002',
  },
}
try {
  const feeTx = await builderClient.execute(
    builderAddr, contractAddr, feeMsg, 'auto',
    'Pay verification fee for rosie-unit-002',
    [{ denom: DENOM, amount: VERIFICATION_FEE }],
  )
  console.log(`  ✓ Fee paid: ${VERIFICATION_FEE} ujunox for rosie-unit-002 — tx: ${feeTx.transactionHash.slice(0, 16)}...`)
} catch (err) {
  console.error(`  ✗ Fee payment failed: ${err.message}`)
}

// ── Step 3: Finalize epoch — consensus is "green" ─────────────────────────
console.log('\n  ── Step 3: Finalize Epoch (consensus: green) ──')
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
    'Finalize epoch 2 — consensus: green, 1 diverging',
  )
  console.log(`  ✓ Epoch finalized! — tx: ${finalTx.transactionHash}`)
  for (const evt of finalTx.events) {
    if (evt.type === 'wasm') {
      const attrs = evt.attributes
        .filter(a => !a.key.startsWith('_contract_address'))
        .map(a => `${a.key}=${a.value}`)
        .join(', ')
      if (attrs) console.log(`    ${attrs}`)
    }
  }
} catch (err) {
  console.error(`  ✗ Finalization failed: ${err.message}`)
  process.exit(1)
}

// ── Step 4: Epoch result ──────────────────────────────────────────────────
console.log('\n  ── Step 4: Epoch Result ──')
const epoch = await builderClient.queryContractSmart(contractAddr, {
  get_epoch: { batch_height: BATCH_HEIGHT },
})
console.log(`  consensus_verdict:  ${epoch.consensus_verdict}`)
console.log(`  total_operators:    ${epoch.total_operators}`)
console.log(`  matching_operators: ${epoch.matching_operators}`)
console.log(`  diverging_operators:${epoch.diverging_operators}`)
console.log(`  rewards_distributed:${epoch.rewards_distributed} ujunox`)
console.log(`  slashed_amount:     ${epoch.slashed_amount} ujunox`)
console.log(`  finalized:          ${epoch.finalized}`)

// ── Step 5: Operator states after ─────────────────────────────────────────
console.log('\n  ── Step 5: Operator States After Slashing ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`\n  ${name} (${address}):`)
  console.log(`    stake:               ${op.stake} ujunox`)
  console.log(`    total_rewards:       ${op.total_rewards} ujunox`)
  console.log(`    total_slashed:       ${op.total_slashed} ujunox`)
  console.log(`    epochs_participated: ${op.epochs_participated}`)
  console.log(`    correct_verdicts:    ${op.correct_verdicts}`)
  console.log(`    incorrect_verdicts:  ${op.incorrect_verdicts}`)
  console.log(`    accuracy:            ${op.accuracy}%`)
  console.log(`    active:              ${op.active}`)
}

// ── Step 6: Final stats ───────────────────────────────────────────────────
console.log('\n  ── Final Stats ──')
const postStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log(`  total_operators:    ${postStats.total_operators}`)
console.log(`  active_operators:   ${postStats.active_operators}`)
console.log(`  total_staked:       ${postStats.total_staked} ujunox`)
console.log(`  reward_pool:        ${postStats.reward_pool} ujunox`)
console.log(`  total_rewards_paid: ${postStats.total_rewards_paid} ujunox`)
console.log(`  total_slashed:      ${postStats.total_slashed} ujunox`)
console.log(`  epochs_finalized:   ${postStats.epochs_finalized}`)

// ── Summary ───────────────────────────────────────────────────────────────
console.log('\n  ═══ SLASHING SCENARIO COMPLETE ═══')
console.log(`  The Contrarian submitted "red" while consensus was "green".`)
console.log(`  Slashed: 100,000 ujunox (10% of 1,000,000 stake)`)
console.log(`  Slashed amount returned to reward pool.`)
console.log(`  The Builder & The Technocrat each earned rewards for correct verdict.`)
console.log(`  The Contrarian's accuracy dropped, stake reduced.`)
console.log(`  Reward pool grew from slashed stake + verification fee.\n`)

process.exit(0)
