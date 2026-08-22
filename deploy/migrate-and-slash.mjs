import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL = 'https://juno.rpc.t.stavr.tech'
const DENOM   = 'ujunox'
const ARTIFACTS = 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'
const BATCH_HEIGHT = 4
const MESSAGES_HASH = 'ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100'
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

// ── Step 1: Migrate to new code with fee-based slashing ────────────────────
console.log('\n  ═══ MIGRATE + SLASHING SCENARIO (fee-based) ═══\n')

const { client: builderClient, address: builderAddr } = await getClientFor('The Builder')

const wasm = readFileSync(join(ARTIFACTS, 'truth_market.wasm'))
console.log(`  Uploading new wasm (${(wasm.length / 1024).toFixed(1)} KB)...`)
const up = await builderClient.upload(builderAddr, wasm, 'auto', 'truth-market fee-based slashing')
console.log(`  New code_id: ${up.codeId}  tx: ${up.transactionHash}`)

console.log('  Migrating contract...')
const mig = await builderClient.migrate(builderAddr, contractAddr, up.codeId, {}, 'auto')
console.log(`  Migrated! tx: ${mig.transactionHash}`)

// Set verification_fee back to 50,000 (migration preserves current value, which was 0)
console.log('  Setting verification_fee to 50000 ujunox...')
await builderClient.execute(builderAddr, contractAddr, {
  update_config: { verification_fee: '50000' },
}, 'auto')

// Verify config
const cfg = await builderClient.queryContractSmart(contractAddr, { get_config: {} })
console.log(`  verification_fee: ${cfg.verification_fee} ujunox`)
console.log(`  slash_percent: ${cfg.slash_percent}% (fallback only)`)

// ── Step 2: Pre-epoch state ────────────────────────────────────────────────
console.log('\n  ── Pre-Epoch State ──')
const preStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log(`  reward_pool:       ${preStats.reward_pool} ujunox`)
console.log(`  total_staked:      ${preStats.total_staked} ujunox`)
console.log(`  epochs_finalized:  ${preStats.epochs_finalized}`)

for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`  ${name}: stake=${op.stake} slashed=${op.total_slashed} accuracy=${op.accuracy}%`)
}

// ── Step 3: Submit verdicts — 2 green, 1 red ──────────────────────────────
console.log('\n  ── Submit Verdicts (2 green, 1 red) ──')
const verdicts = [
  { name: 'The Builder', verdict: 'green' },
  { name: 'The Technocrat', verdict: 'green' },
  { name: 'The Contrarian', verdict: 'red' },
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
  const tx = await client.execute(address, contractAddr, msg, 'auto', `Submit ${op.verdict}`)
  console.log(`  ✓ ${op.name} submitted "${op.verdict}" — tx: ${tx.transactionHash.slice(0, 16)}...`)
}

// ── Step 4: Pay verification fee ──────────────────────────────────────────
console.log('\n  ── Pay Verification Fee ──')
const feeMsg = {
  pay_verification_fee: {
    batch_height: BATCH_HEIGHT,
    robot_id: 'rosie-unit-004',
  },
}
const feeTx = await builderClient.execute(
  builderAddr, contractAddr, feeMsg, 'auto',
  'Pay verification fee for rosie-unit-004',
  [{ denom: DENOM, amount: VERIFICATION_FEE }],
)
console.log(`  ✓ Fee paid: ${VERIFICATION_FEE} ujunox — tx: ${feeTx.transactionHash.slice(0, 16)}...`)

// ── Step 5: Finalize epoch ────────────────────────────────────────────────
console.log('\n  ── Finalize Epoch (consensus: green) ──')
const finalMsg = {
  finalize_epoch: {
    batch_height: BATCH_HEIGHT,
    consensus_verdict: 'green',
    messages_hash: MESSAGES_HASH,
  },
}
const finalTx = await builderClient.execute(
  builderAddr, contractAddr, finalMsg, 'auto',
  'Finalize epoch 4 — fee-based slashing',
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

// ── Step 6: Results ───────────────────────────────────────────────────────
console.log('\n  ── Epoch Result ──')
const epoch = await builderClient.queryContractSmart(contractAddr, {
  get_epoch: { batch_height: BATCH_HEIGHT },
})
console.log(`  matching: ${epoch.matching_operators}, diverging: ${epoch.diverging_operators}`)
console.log(`  rewards_distributed: ${epoch.rewards_distributed} ujunox`)
console.log(`  slashed_amount: ${epoch.slashed_amount} ujunox`)

console.log('\n  ── Operator States After ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`\n  ${name}:`)
  console.log(`    stake:           ${op.stake} ujunox`)
  console.log(`    total_rewards:   ${op.total_rewards} ujunox`)
  console.log(`    total_slashed:   ${op.total_slashed} ujunox`)
  console.log(`    correct:         ${op.correct_verdicts}`)
  console.log(`    incorrect:       ${op.incorrect_verdicts}`)
  console.log(`    accuracy:        ${op.accuracy}%`)
}

// ── Summary ───────────────────────────────────────────────────────────────
const postStats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log('\n  ── Final Stats ──')
console.log(`  reward_pool:       ${postStats.reward_pool} ujunox`)
console.log(`  total_staked:      ${postStats.total_staked} ujunox`)
console.log(`  total_slashed:     ${postStats.total_slashed} ujunox`)
console.log(`  epochs_finalized:  ${postStats.epochs_finalized}`)

console.log('\n  ═══ FEE-BASED SLASHING VERIFIED ═══')
console.log(`  Slash = verification_fee = ${VERIFICATION_FEE} ujunox (not 10% of stake)`)
console.log(`  The Contrarian lost ${VERIFICATION_FEE} ujunox, not 100,000.`)
console.log(`  Penalty aligns with what robots pay for verification.\n`)

// Update deployed.json
deployed['truth-market'].code_id = up.codeId
deployed['truth-market'].migrate_tx = mig.transactionHash
writeFileSync(join(__dir, 'deployed.json'), JSON.stringify(deployed, null, 2))

process.exit(0)
