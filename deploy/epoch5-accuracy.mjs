import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL = 'https://juno.rpc.t.stavr.tech'
const DENOM   = 'ujunox'
const BATCH_HEIGHT = 5
const MESSAGES_HASH = '11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff'
const VERIFICATION_FEE = '50000'

const state = JSON.parse(readFileSync(join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json'), 'utf8'))
const deployed = JSON.parse(readFileSync(join(__dir, 'deployed.json'), 'utf8'))
const contractAddr = deployed['truth-market']?.address

async function getClientFor(name) {
  const mp = state.mps.find(m => m.name === name)
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, { prefix: 'juno' })
  const [acc] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString('0.075ujunox'),
  })
  return { client, address: acc.address }
}

const { client: builderClient, address: builderAddr } = await getClientFor('The Builder')

// ── 1. Switch to StakeTimesAccuracy ────────────────────────────────────────
console.log('\n  ═══ EPOCH 5: StakeTimesAccuracy REWARD MODE ═══\n')
console.log('  Switching reward_mode to stake_times_accuracy...')
await builderClient.execute(builderAddr, contractAddr, {
  update_config: { reward_mode: 'stake_times_accuracy' },
}, 'auto')

const cfg = await builderClient.queryContractSmart(contractAddr, { get_config: {} })
console.log(`  reward_mode: ${cfg.reward_mode}`)
console.log(`  verification_fee: ${cfg.verification_fee} ujunox`)

// ── 2. Pre-epoch operator states ───────────────────────────────────────────
console.log('\n  ── Pre-Epoch Operator States ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`  ${name}: stake=${op.stake} accuracy=${op.accuracy}% correct=${op.correct_verdicts} wrong=${op.incorrect_verdicts} rewards=${op.total_rewards}`)
}

// ── 3. Submit verdicts — all 3 say "green" this time ──────────────────────
console.log('\n  ── Submit Verdicts (all green) ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { client, address } = await getClientFor(name)
  const tx = await client.execute(address, contractAddr, {
    submit_verdict: {
      batch_height: BATCH_HEIGHT,
      verdict: 'green',
      messages_hash: MESSAGES_HASH,
    },
  }, 'auto', `Submit green verdict`)
  console.log(`  ✓ ${name} submitted "green" — tx: ${tx.transactionHash.slice(0, 16)}...`)
}

// ── 4. Pay verification fee ───────────────────────────────────────────────
console.log('\n  ── Pay Verification Fee ──')
const feeTx = await builderClient.execute(
  builderAddr, contractAddr, {
    pay_verification_fee: { batch_height: BATCH_HEIGHT, robot_id: 'rosie-unit-005' },
  }, 'auto', 'Pay verification fee for rosie-unit-005',
  [{ denom: DENOM, amount: VERIFICATION_FEE }],
)
console.log(`  ✓ Fee paid: ${VERIFICATION_FEE} ujunox — tx: ${feeTx.transactionHash.slice(0, 16)}...`)

// ── 5. Finalize epoch ──────────────────────────────────────────────────────
console.log('\n  ── Finalize Epoch (consensus: green) ──')
const finalTx = await builderClient.execute(
  builderAddr, contractAddr, {
    finalize_epoch: {
      batch_height: BATCH_HEIGHT,
      consensus_verdict: 'green',
      messages_hash: MESSAGES_HASH,
    },
  }, 'auto', 'Finalize epoch 5 — StakeTimesAccuracy',
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

// ── 6. Epoch result ────────────────────────────────────────────────────────
console.log('\n  ── Epoch 5 Result ──')
const epoch = await builderClient.queryContractSmart(contractAddr, {
  get_epoch: { batch_height: BATCH_HEIGHT },
})
console.log(`  matching: ${epoch.matching_operators}, diverging: ${epoch.diverging_operators}`)
console.log(`  rewards_distributed: ${epoch.rewards_distributed} ujunox`)
console.log(`  slashed: ${epoch.slashed_amount} ujunox`)

// ── 7. Operator states after — show accuracy-weighted rewards ─────────────
console.log('\n  ── Operator States After (StakeTimesAccuracy) ──')
for (const name of ['The Builder', 'The Technocrat', 'The Contrarian']) {
  const { address } = await getClientFor(name)
  const op = await builderClient.queryContractSmart(contractAddr, { get_operator: { address } })
  console.log(`\n  ${name}:`)
  console.log(`    stake:           ${op.stake} ujunox`)
  console.log(`    total_rewards:   ${op.total_rewards} ujunox`)
  console.log(`    correct:         ${op.correct_verdicts}`)
  console.log(`    accuracy:        ${op.accuracy}%`)
}

// ── 8. On-chain queries for article proof ─────────────────────────────────
console.log('\n\n  ═══ ON-CHAIN QUERIES FOR ARTICLE ═══\n')

// Config
const finalCfg = await builderClient.queryContractSmart(contractAddr, { get_config: {} })
console.log('  ── get_config ──')
console.log(JSON.stringify(finalCfg, null, 2))

// Stats
const stats = await builderClient.queryContractSmart(contractAddr, { get_stats: {} })
console.log('\n  ── get_stats ──')
console.log(JSON.stringify(stats, null, 2))

// List operators
const opsList = await builderClient.queryContractSmart(contractAddr, { list_operators: {} })
console.log('\n  ── list_operators ──')
console.log(JSON.stringify(opsList, null, 2))

// Fingerprints
const fingerprints = await builderClient.queryContractSmart(contractAddr, { get_fingerprints: {} })
console.log('\n  ── get_fingerprints ──')
console.log(JSON.stringify(fingerprints, null, 2))

// Reward pool
const pool = await builderClient.queryContractSmart(contractAddr, { get_reward_pool: {} })
console.log('\n  ── get_reward_pool ──')
console.log(JSON.stringify(pool, null, 2))

// Epoch 5
const ep5 = await builderClient.queryContractSmart(contractAddr, { get_epoch: { batch_height: 5 } })
console.log('\n  ── get_epoch(5) ──')
console.log(JSON.stringify(ep5, null, 2))

console.log('\n  ═══ DONE ═══\n')
process.exit(0)
