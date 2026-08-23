import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function distImport(...segments) {
  return import(pathToFileURL(join(MCP_DIST, ...segments)).href)
}

const RPC = 'https://juno.rpc.t.stavr.tech'
const TRUTH_MARKET_ADDR = 'juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p'
const MOULTBOOK_ADDR = 'juno1nm0mu2uwxnphn2hqnuyywyvxp6qfdfuhe64svrnq3vjh66pwxlhskt3dx4'
const OPERATOR_ADDR = 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd'
const FINGERPRINT = 'juno-agents-dao'

function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

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
    gasAdjustment: 1.5,
  })
  return { client, address: acc.address }
}

// ─── Step 2: Fund operator wallet ────────────────────────────────────────────
console.log('\n═══ Step 2: Fund operator wallet with 2 JUNOX ═══')
const { client: builderClient, address: builderAddr } = await getSigner('builder')
console.log('  From:', builderAddr)
console.log('  To:  ', OPERATOR_ADDR)

// Check if already funded
const existingBal = await builderClient.getBalance(OPERATOR_ADDR, 'ujunox')
if (BigInt(existingBal.amount) > 0n) {
  console.log('  Already funded:', existingBal.amount + 'ujunox — skipping')
} else {
  const fundTx = await builderClient.sendTokens(
    builderAddr,
    OPERATOR_ADDR,
    [{ denom: 'ujunox', amount: '2000000' }],
    { amount: [{ denom: 'ujunox', amount: '30000' }], gas: '200000' },
    'A052 operator mandate — fund dao-truth-operator'
  )
  console.log('  ✓ Funding tx:', fundTx.transactionHash)
}

const opBalance = await builderClient.getBalance(OPERATOR_ADDR, 'ujunox')
console.log('  Operator balance:', opBalance.amount + 'ujunox')

// ─── Step 3: Register as operator #4 ─────────────────────────────────────────
console.log('\n═══ Step 3: Register as operator #4 ═══')

// Check if already registered
const opsCheck = await builderClient.queryContractSmart(TRUTH_MARKET_ADDR, { list_operators: {} })
const alreadyRegistered = (opsCheck.operators || []).some(o => o.address === OPERATOR_ADDR)
if (alreadyRegistered) {
  console.log('  Already registered — skipping')
  for (const op of opsCheck.operators || []) {
    console.log('    ', op.address, 'fingerprint:', op.fingerprint, 'stake:', op.stake)
  }
} else {
  const { client: opClient, address: opAddr } = await getSigner('dao-truth-operator')
  console.log('  Operator:', opAddr)

  const regMsg = {
    register_operator: {
      fingerprint: FINGERPRINT,
    },
  }

  const regTx = await opClient.execute(
    opAddr,
    TRUTH_MARKET_ADDR,
    regMsg,
    { amount: [{ denom: 'ujunox', amount: '50000' }], gas: '400000' },
    'A052 operator mandate — register as operator #4 (juno-agents-dao)',
    [{ denom: 'ujunox', amount: '1000000' }]
  )
  console.log('  ✓ Registration tx:', regTx.transactionHash)

  // Verify
  const ops = await opClient.queryContractSmart(TRUTH_MARKET_ADDR, { list_operators: {} })
  console.log('  Total operators:', ops.operators?.length || 'unknown')
  for (const op of ops.operators || []) {
    console.log('    ', op.address, 'fingerprint:', op.fingerprint, 'stake:', op.stake)
  }
}

// ─── Step 4: Publish rule set on Moultbook ───────────────────────────────────
console.log('\n═══ Step 4: Publish frozen rule set on Moultbook ═══')
import { createHash } from 'crypto'

const ruleSet = `A052 DAO Operator Rule Set — FROZEN

Operator: ${OPERATOR_ADDR}
Fingerprint: ${FINGERPRINT}
Contract: ${TRUTH_MARKET_ADDR}
Published: ${new Date().toISOString()}

Evaluation rules (applied to each batch):
1. Envelope bounds: sensor values must be within [min, max] ranges defined per sensor type
2. Merkle consistency: batch hash must match the committed Merkle root
3. Attestation signature validity: proof verification must pass
4. Sequence gap detection: no missing batch heights in the sequence
5. Timestamp ordering: batch timestamps must be monotonically increasing

Verdict logic:
- If all 5 rules pass: verdict = "consistent"
- If any rule fails: verdict = "inconsistent"
- If batch is empty or unavailable: skip (no verdict submitted)

Rationale format (posted to Moultbook per verdict):
  batch_height: <N>
  verdict: <consistent|inconsistent>
  rules_fired: [<rule numbers that failed, or "none">]
  reason: <one sentence>

This rule set is frozen for the 7-day A052 mandate. No changes after publication.`

const commitment = Buffer.from(createHash('sha256').update(ruleSet, 'utf8').digest())
const sizeBytes = Buffer.byteLength(ruleSet, 'utf8')

const moultMsg = {
  post: {
    commitment: commitment.toString('base64'),
    content_type: 'text/plain+a052-ruleset',
    size_bytes: sizeBytes,
    attestation_ref: null,
    visibility: 'public',
    refs: [],
  },
}

console.log('  Size:', sizeBytes, 'bytes')
const moultTx = await builderClient.execute(
  builderAddr,
  MOULTBOOK_ADDR,
  moultMsg,
  { amount: [{ denom: 'ujunox', amount: '45000' }], gas: '300000' },
  'A052 operator mandate — publish frozen rule set',
)
console.log('  ✓ Rule set tx:', moultTx.transactionHash)

// Find moult ID
const events = (moultTx.logs || []).flatMap((l) => l.events || []).concat(moultTx.events || [])
for (const ev of events) {
  if (ev.type === 'wasm') {
    const idAttr = ev.attributes.find((a) => a.key === 'id')
    if (idAttr?.value) console.log('  moult_id:', idAttr.value)
  }
}

console.log('\n══════════════════════════════════════════════════')
console.log('  Operator mandate execution complete!')
console.log('  Operator address:', OPERATOR_ADDR)
console.log('  Fingerprint:', FINGERPRINT)
console.log('  Next: run verdicts with junoclaw-miner')
console.log('══════════════════════════════════════════════════')
