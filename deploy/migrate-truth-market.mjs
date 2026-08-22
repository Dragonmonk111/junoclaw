import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL   = 'https://juno.rpc.t.stavr.tech'
const DENOM     = 'ujunox'
const ARTIFACTS = 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

const state = JSON.parse(readFileSync(join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json'), 'utf8'))
const mp = state.mps.find(m => m.name === 'The Builder')

const deployed = JSON.parse(readFileSync(join(__dir, 'deployed.json'), 'utf8'))
const contractAddr = deployed['truth-market']?.address
if (!contractAddr) {
  console.error('truth-market address not found in deployed.json')
  process.exit(1)
}

const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, { prefix: 'juno' })
const [acc] = await wallet.getAccounts()
const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
  gasPrice: GasPrice.fromString('0.075ujunox'),
})

console.log('Deployer:', acc.address)
console.log('Migrating truth-market at', contractAddr)

const wasm = readFileSync(join(ARTIFACTS, 'truth_market.wasm'))
console.log(`Uploading new wasm (${(wasm.length / 1024).toFixed(1)} KB)...`)
const up = await client.upload(acc.address, wasm, 'auto', 'truth-market min-operators fix')
console.log('New code_id:', up.codeId, 'tx:', up.transactionHash)

console.log('Migrating contract to code_id', up.codeId, '...')
const mig = await client.migrate(acc.address, contractAddr, up.codeId, {}, 'auto')
console.log('Migrated! tx:', mig.transactionHash)

// ── Verify migration ──────────────────────────────────────────────────────
const cfg = await client.queryContractSmart(contractAddr, { get_config: {} })
console.log('\n  ── Contract Config ──')
console.log('  admin:             ', cfg.admin)
console.log('  min_stake:         ', cfg.min_stake, 'ujunox')
console.log('  slash_percent:     ', cfg.slash_percent, '%')
console.log('  reward_percent:    ', cfg.reward_percent, '%')
console.log('  denom:             ', cfg.denom)
console.log('  unstake_cooldown:  ', cfg.unstake_cooldown_secs, 'secs')
console.log('  min_operators:     ', cfg.min_operators)
console.log('  reward_mode:       ', cfg.reward_mode)
console.log('  verification_fee:  ', cfg.verification_fee, 'ujunox')

// ── Set verification fee (50,000 ujunox = 0.05 JUNOX per batch) ───────────
console.log('\n  Setting verification_fee to 50000 ujunox...')
const feeMsg = {
  update_config: {
    verification_fee: '50000',
  },
}
const feeTx = await client.execute(acc.address, contractAddr, feeMsg, 'auto')
console.log('  ✓ verification_fee set, tx:', feeTx.transactionHash)

// ── Register first operator (The Builder wallet) ──────────────────────────
console.log('\n  Registering first operator:', acc.address)
const stakeAmount = '1000000' // 1 JUNOX = min_stake
const fingerprint = `builder-${acc.address.slice(-8)}-rule-v1`
const regMsg = {
  register_operator: {
    fingerprint,
  },
}
const regTx = await client.execute(
  acc.address, contractAddr, regMsg, 'auto',
  undefined, [{ denom: DENOM, amount: stakeAmount }],
)
console.log('  ✓ Operator registered, tx:', regTx.transactionHash)
console.log('  Fingerprint:', fingerprint)
console.log('  Stake:', stakeAmount, 'ujunox')

// ── Deposit rewards to fund the pool (500,000 ujunox = 0.5 JUNOX) ─────────
console.log('\n  Depositing 500000 ujunox into reward pool...')
const depMsg = { deposit_rewards: {} }
const depTx = await client.execute(
  acc.address, contractAddr, depMsg, 'auto',
  undefined, [{ denom: DENOM, amount: '500000' }],
)
console.log('  ✓ Rewards deposited, tx:', depTx.transactionHash)

// ── Verify operator registration ──────────────────────────────────────────
const opInfo = await client.queryContractSmart(contractAddr, {
  get_operator: { address: acc.address },
})
console.log('\n  ── Operator Info ──')
console.log('  address:          ', opInfo.address)
console.log('  stake:            ', opInfo.stake, 'ujunox')
console.log('  active:           ', opInfo.active)
console.log('  fingerprint:      ', opInfo.fingerprint)
console.log('  rewards:          ', opInfo.total_rewards, 'ujunox')
console.log('  slashed:          ', opInfo.total_slashed, 'ujunox')
console.log('  epochs:           ', opInfo.epochs_participated)
console.log('  correct_verdicts: ', opInfo.correct_verdicts)
console.log('  incorrect_verdicts:', opInfo.incorrect_verdicts)
console.log('  accuracy:         ', opInfo.accuracy, '%')

// ── Verify stats ──────────────────────────────────────────────────────────
const stats = await client.queryContractSmart(contractAddr, { get_stats: {} })
console.log('\n  ── Truth Market Stats ──')
console.log('  total_operators:   ', stats.total_operators)
console.log('  active_operators:  ', stats.active_operators)
console.log('  total_staked:      ', stats.total_staked, 'ujunox')
console.log('  reward_pool:       ', stats.reward_pool, 'ujunox')
console.log('  total_rewards_paid:', stats.total_rewards_paid, 'ujunox')
console.log('  total_slashed:     ', stats.total_slashed, 'ujunox')
console.log('  epochs_finalized:  ', stats.epochs_finalized)

// ── Update deployed.json ──────────────────────────────────────────────────
deployed['truth-market'].code_id = up.codeId
deployed['truth-market'].migrate_tx = mig.transactionHash
deployed['truth-market'].first_operator = acc.address
deployed['truth-market'].first_operator_tx = regTx.transactionHash
const { writeFileSync } = await import('fs')
writeFileSync(join(__dir, 'deployed.json'), JSON.stringify(deployed, null, 2))
console.log('\n  Updated deployed.json')

console.log('\n  ═══ FIRST OPERATOR REGISTERED ═══')
console.log('  The truth market now has 1 operator.')
console.log('  Reward pool funded with 500,000 ujunox.')
console.log('  Verification fee set to 50,000 ujunox per batch.')
console.log('  Next: run junoclaw-miner to start evaluating batches.\n')

process.exit(0)
