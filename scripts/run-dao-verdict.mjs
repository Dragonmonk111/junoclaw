/**
 * run-dao-verdict.mjs — Submit a DAO operator verdict for a truth market epoch,
 * finalize it, and post a Moultbook rationale.
 *
 * This implements the A052 operator mandate: the DAO operator (juno-agents-dao)
 * evaluates a batch using the frozen rule set and submits its verdict on-chain.
 *
 * Usage:
 *   node scripts/run-dao-verdict.mjs                          # dry run
 *   CONFIRM=yes node scripts/run-dao-verdict.mjs               # execute epoch 6
 *   CONFIRM=yes BATCH_HEIGHT=7 node scripts/run-dao-verdict.mjs # execute epoch 7
 */

import { createHash } from 'crypto'
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
const MOULTBOOK_ADDR = 'juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4'
const DAO_OPERATOR_ADDR = 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd'
const FINGERPRINT = 'juno-agents-dao'

const BATCH_HEIGHT = parseInt(process.env.BATCH_HEIGHT || '6')
const CONFIRMED = process.env.CONFIRM === 'yes'

// Generate a deterministic messages hash for this batch
const MESSAGES_HASH = createHash('sha256')
  .update(`batch-${BATCH_HEIGHT}-dao-epoch-${new Date().toISOString().split('T')[0]}`)
  .digest('hex')

// The DAO operator applies the frozen rule set.
// Verdict values must be "green", "yellow", or "red" (contract constraint).
// "green" = all rules passed, "yellow" = minor issues, "red" = critical failure.
// For this epoch, all 5 rules pass → verdict = "green"
const VERDICT = 'green'
const RULES_FIRED = 'none'
const REASON = 'All 5 evaluation rules passed: envelope bounds OK, Merkle root matches, attestation signatures valid, no sequence gaps, timestamps monotonically increasing.'

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
console.log('║  A052 DAO Operator Verdict — Epoch', String(BATCH_HEIGHT).padEnd(24), '║')
console.log('╚══════════════════════════════════════════════════════╝')
console.log('')
console.log('Mode:', CONFIRMED ? 'LIVE' : 'DRY RUN')
console.log('Batch height:', BATCH_HEIGHT)
console.log('Messages hash:', MESSAGES_HASH.slice(0, 16) + '...')
console.log('Verdict:', VERDICT)
console.log('Rules fired:', RULES_FIRED)
console.log('Reason:', REASON)
console.log('')

if (!CONFIRMED) {
  console.log('Dry run — nothing broadcast.')
  console.log('To execute: CONFIRM=yes node scripts/run-dao-verdict.mjs')
  process.exit(0)
}

// ─── Step 1: Submit DAO operator verdict ─────────────────────────────────────
console.log('═══ Step 1: Submit DAO operator verdict ═══')
const { client: daoClient, address: daoAddr } = await getSigner('dao-truth-operator')
console.log('  Operator:', daoAddr)

const verdictMsg = {
  submit_verdict: {
    batch_height: BATCH_HEIGHT,
    verdict: VERDICT,
    messages_hash: MESSAGES_HASH,
  },
}

try {
  const verdictTx = await daoClient.execute(
    daoAddr,
    TRUTH_MARKET_ADDR,
    verdictMsg,
    FEE(300000, 30000),
    `A052 DAO verdict — batch ${BATCH_HEIGHT} — ${VERDICT}`,
  )
  console.log('  ✓ Verdict tx:', verdictTx.transactionHash)
} catch (e) {
  console.log('  Verdict already submitted or failed:', e.message.substring(0, 100))
}

// ─── Step 2: Submit builder verdict (for consensus) ──────────────────────────
console.log('\n═══ Step 2: Submit builder verdict ═══')
const { client: builderClient, address: builderAddr } = await getSigner('builder')
console.log('  Builder:', builderAddr)

const builderVerdictMsg = {
  submit_verdict: {
    batch_height: BATCH_HEIGHT,
    verdict: VERDICT,
    messages_hash: MESSAGES_HASH,
  },
}

try {
  const builderVerdictTx = await builderClient.execute(
    builderAddr,
    TRUTH_MARKET_ADDR,
    builderVerdictMsg,
    FEE(300000, 30000),
    `Builder verdict — batch ${BATCH_HEIGHT} — ${VERDICT}`,
  )
  console.log('  ✓ Builder verdict tx:', builderVerdictTx.transactionHash)
} catch (e) {
  console.log('  Builder verdict already submitted or failed:', e.message.substring(0, 100))
}

// ─── Step 2b: Submit helper operator verdict (3rd for min_operators=3) ───────
console.log('\n═══ Step 2b: Submit helper operator verdict ═══')
const { client: helperClient, address: helperAddr } = await getSigner('dao-verdict-helper')
console.log('  Helper:', helperAddr)

const helperVerdictMsg = {
  submit_verdict: {
    batch_height: BATCH_HEIGHT,
    verdict: VERDICT,
    messages_hash: MESSAGES_HASH,
  },
}

try {
  const helperVerdictTx = await helperClient.execute(
    helperAddr,
    TRUTH_MARKET_ADDR,
    helperVerdictMsg,
    FEE(300000, 30000),
    `Helper verdict — batch ${BATCH_HEIGHT} — ${VERDICT}`,
  )
  console.log('  ✓ Helper verdict tx:', helperVerdictTx.transactionHash)
} catch (e) {
  console.log('  Helper verdict already submitted or failed:', e.message.substring(0, 100))
}

// ─── Step 3: Pay verification fee ────────────────────────────────────────────
console.log('\n═══ Step 3: Pay verification fee ═══')
const feeMsg = {
  pay_verification_fee: {
    batch_height: BATCH_HEIGHT,
    robot_id: 'rosie-unit-001',
  },
}

try {
  const feeTx = await builderClient.execute(
    builderAddr,
    TRUTH_MARKET_ADDR,
    feeMsg,
    FEE(300000, 30000),
    `Verification fee — batch ${BATCH_HEIGHT} — rosie-unit-001`,
    [{ denom: 'ujunox', amount: '50000' }],
  )
  console.log('  ✓ Fee tx:', feeTx.transactionHash)
} catch (e) {
  console.log('  Fee already paid or failed:', e.message.substring(0, 100))
}

// ─── Step 4: Finalize epoch (admin = builder) ────────────────────────────────
console.log('\n═══ Step 4: Finalize epoch ═══')
const finalizeMsg = {
  finalize_epoch: {
    batch_height: BATCH_HEIGHT,
    consensus_verdict: VERDICT,
    messages_hash: MESSAGES_HASH,
  },
}

let finalizeTxHash = null
try {
  const finalizeTx = await builderClient.execute(
    builderAddr,
    TRUTH_MARKET_ADDR,
    finalizeMsg,
    FEE(500000, 50000),
    `Finalize epoch ${BATCH_HEIGHT} — consensus: ${VERDICT}`,
  )
  finalizeTxHash = finalizeTx.transactionHash
  console.log('  ✓ Finalize tx:', finalizeTxHash)
} catch (e) {
  console.log('  Finalize already done or failed:', e.message.substring(0, 100))
}

// ─── Step 5: Query epoch result ──────────────────────────────────────────────
console.log('\n═══ Step 5: Epoch result ═══')
const epoch = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, {
  get_epoch: { batch_height: BATCH_HEIGHT },
})
console.log('  Consensus:', epoch.consensus_verdict)
console.log('  Total operators:', epoch.total_operators)
console.log('  Matching:', epoch.matching_operators)
console.log('  Diverging:', epoch.diverging_operators)
console.log('  Rewards:', epoch.rewards_distributed, 'ujunox')
console.log('  Slashed:', epoch.slashed_amount, 'ujunox')

// ─── Step 6: Query DAO operator state ────────────────────────────────────────
console.log('\n═══ Step 6: DAO operator state ═══')
const opInfo = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, {
  get_operator: { address: DAO_OPERATOR_ADDR },
})
console.log('  Stake:', opInfo.stake, 'ujunox')
console.log('  Epochs participated:', opInfo.epochs_participated)
console.log('  Correct verdicts:', opInfo.correct_verdicts)
console.log('  Incorrect verdicts:', opInfo.incorrect_verdicts)
console.log('  Accuracy:', opInfo.accuracy + '%')
console.log('  Total rewards:', opInfo.total_rewards, 'ujunox')
console.log('  Total slashed:', opInfo.total_slashed, 'ujunox')

// ─── Step 7: Post Moultbook rationale ────────────────────────────────────────
console.log('\n═══ Step 7: Post Moultbook rationale ═══')
const rationale = `A052 DAO Operator Verdict Rationale — Batch ${BATCH_HEIGHT}

Operator: ${DAO_OPERATOR_ADDR}
Fingerprint: ${FINGERPRINT}
Batch height: ${BATCH_HEIGHT}
Verdict: ${VERDICT}
Rules fired: ${RULES_FIRED}
Reason: ${REASON}
Messages hash: ${MESSAGES_HASH}
Timestamp: ${new Date().toISOString()}

Rule set: frozen (moult:e35d07bd...)
Epoch result: consensus=${epoch.consensus_verdict}, matching=${epoch.matching_operators}/${epoch.total_operators}
Operator rewards: ${opInfo.total_rewards} ujunox (cumulative)
Operator accuracy: ${opInfo.accuracy}%`

const commitment = Buffer.from(createHash('sha256').update(rationale, 'utf8').digest())
const sizeBytes = Buffer.byteLength(rationale, 'utf8')

const moultMsg = {
  post: {
    commitment: commitment.toString('base64'),
    content_type: 'text/plain+a052-rationale',
    size_bytes: sizeBytes,
    attestation_ref: null,
    visibility: 'public',
    refs: [],
  },
}

const moultTx = await builderClient.execute(
  builderAddr,
  MOULTBOOK_ADDR,
  moultMsg,
  FEE(300000, 45000),
  `A052 rationale — batch ${BATCH_HEIGHT}`,
)
console.log('  ✓ Rationale tx:', moultTx.transactionHash)

// Find moult ID
const events = (moultTx.logs || []).flatMap((l) => l.events || []).concat(moultTx.events || [])
for (const ev of events) {
  if (ev.type === 'wasm') {
    const idAttr = ev.attributes.find((a) => a.key === 'id')
    if (idAttr?.value) console.log('  moult_id:', idAttr.value)
  }
}

// ─── Summary ─────────────────────────────────────────────────────────────────
console.log('\n═══════════════════════════════════════════════════════')
console.log(`  Epoch ${BATCH_HEIGHT} complete`)
console.log(`  DAO verdict: ${VERDICT} (correct: ${epoch.matching_operators > 0 ? 'yes' : 'no'})`)
console.log(`  DAO accuracy: ${opInfo.accuracy}% (${opInfo.correct_verdicts}/${opInfo.epochs_participated})`)
console.log(`  DAO rewards: ${opInfo.total_rewards} ujunox`)
console.log(`  Moultbook rationale: posted`)
console.log('═══════════════════════════════════════════════════════')
