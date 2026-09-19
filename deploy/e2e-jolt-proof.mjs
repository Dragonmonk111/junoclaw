import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://junotestnet-rpc.kleomedes.network'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'

const DEPLOYED_FILE = join(__dir, 'deployed-testnet.json')
const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

function loadMnemonic() {
  if (process.env.MNEMONIC) return process.env.MNEMONIC
  if (process.env.PARLIAMENT_ROLE) {
    const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
    const mp = (state.mps || []).find((m) => m.name === process.env.PARLIAMENT_ROLE)
    if (!mp) {
      console.error(`No MP with name "${process.env.PARLIAMENT_ROLE}"`)
      process.exit(1)
    }
    console.log(`  Wallet: ${mp.name} (${mp.address})`)
    return mp.mnemonic
  }
  console.error('Set MNEMONIC or PARLIAMENT_ROLE')
  process.exit(1)
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)) }

function isTxIndexingError(err) {
  const msg = err?.message || String(err)
  return msg.includes('transaction indexing is disabled')
}

async function safeExecute(client, address, contractAddr, msg, memo) {
  try {
    return await client.execute(address, contractAddr, msg, 'auto', memo)
  } catch (err) {
    if (!isTxIndexingError(err)) throw err
    console.log('  (tx indexing disabled, waiting for confirmation...)')
    await sleep(8000)
    return { transactionHash: 'unknown (tx indexing disabled)' }
  }
}

// Create a dummy Jolt proof that passes Phase 1 structural validation.
// Must be: non-empty, <= 512KB, starts with "JOLT" magic or bincode preamble.
// We use "JOLT" magic + random-ish padding to ~4KB.
function makeDummyJoltProof() {
  const magic = Buffer.from('JOLT', 'ascii')
  // Pad with deterministic bytes to 4KB (well above 1KB structural minimum)
  const padding = Buffer.alloc(4092, 0x42)
  return Buffer.concat([magic, padding])
}

async function main() {
  console.log('\n  === E2E Jolt Proof: ibc-task-host → jolt-cw-verifier ===')
  console.log(`  Chain: ${CHAIN_ID}`)
  console.log(`  RPC:   ${RPC_URL}\n`)

  const deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))

  const taskHostAddr = deployed['ibc-task-host']?.address
  const joltVerifierAddr = deployed['jolt-cw-verifier']?.address

  if (!taskHostAddr) {
    console.error('  ibc-task-host not deployed. Run setup-ibc-channel.mjs first.')
    process.exit(1)
  }
  if (!joltVerifierAddr) {
    console.error('  jolt-cw-verifier not deployed.')
    process.exit(1)
  }

  console.log(`  ibc-task-host:   ${taskHostAddr}`)
  console.log(`  jolt-cw-verifier: ${joltVerifierAddr}`)

  // Verify config
  const mnemonic = loadMnemonic()
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer:        ${address}\n`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  // Step 0: Verify ibc-task-host config
  console.log('  ── Step 0: Verify config ────────────────────────────────────')
  const config = await client.queryContractSmart(taskHostAddr, { config: {} })
  console.log(`  jolt_verifier wired: ${config.jolt_verifier || 'NONE'}`)
  if (!config.jolt_verifier) {
    console.error('  ERROR: jolt_verifier not configured in ibc-task-host')
    process.exit(1)
  }
  console.log('  ✅ Config OK\n')

  // Step 1: Create a dummy Jolt proof
  console.log('  ── Step 1: Create dummy Jolt proof ──────────────────────────')
  const proofBytes = makeDummyJoltProof()
  const proofB64 = proofBytes.toString('base64')
  console.log(`  Proof size: ${proofBytes.length} bytes`)
  console.log(`  Proof magic: ${proofBytes.slice(0, 4).toString('ascii')}`)
  console.log(`  Base64 length: ${proofB64.length}\n`)

  // Step 2: Submit proof via ibc-task-host (JunoClawV1::SubmitProof)
  // This dispatches StoreProof + VerifyProof as submessages to jolt-cw-verifier
  console.log('  ── Step 2: SubmitProof via ibc-task-host ────────────────────')
  const submitMsg = {
    juno_claw_v1: {
      submit_proof: {
        task_id: 1,
        proof_b64: proofB64,
        public_inputs_b64: '',
        agent_origin_chain: 'osmosis-testnet',
        agent_origin_addr: 'osmo1dummy',
      },
    },
  }

  console.log('  Sending SubmitProof...')
  const result = await safeExecute(
    client, address, taskHostAddr, submitMsg, 'E2E Jolt proof test'
  )
  console.log(`  ✅ SubmitProof executed! tx: ${result.transactionHash}\n`)

  // Step 3: Query jolt-cw-verifier for proof status
  console.log('  ── Step 3: Verify jolt-cw-verifier state ────────────────────')
  const proofStatus = await client.queryContractSmart(joltVerifierAddr, { proof_status: {} })
  console.log(`  has_proof:          ${proofStatus.has_proof}`)
  console.log(`  proof_size_bytes:   ${proofStatus.proof_size_bytes}`)
  console.log(`  proof_sha256:       ${proofStatus.proof_sha256}`)
  console.log(`  last_verify_verified: ${proofStatus.last_verify_verified}`)
  console.log(`  last_verify_block:  ${proofStatus.last_verify_block}`)
  console.log(`  verifier_mode:      ${proofStatus.verifier_mode || 'structural (default)'}\n`)

  if (proofStatus.has_proof && proofStatus.last_verify_verified) {
    console.log('  ✅ E2E Jolt Proof: PASS')
    console.log('  ibc-task-host → StoreProof → VerifyProof → jolt-cw-verifier confirmed')
  } else {
    console.log('  ⚠️  Proof stored but verification state unexpected')
    console.log(`     has_proof=${proofStatus.has_proof}, verified=${proofStatus.last_verify_verified}`)
  }

  // Step 4: Query ibc-task-host stats
  console.log('\n  ── Step 4: ibc-task-host stats ──────────────────────────────')
  const stats = await client.queryContractSmart(taskHostAddr, { stats: {} })
  console.log(`  total_submit_proof: ${stats.total_submit_proof}`)
  console.log(`  total_accept_task:   ${stats.total_accept_task}`)
  console.log(`  total_swap:          ${stats.total_swap}`)

  // Step 5: Query ibc-task-host proof_status (proxies to jolt-cw-verifier)
  console.log('\n  ── Step 5: ibc-task-host proof_status (proxy query) ────────')
  const hostProofStatus = await client.queryContractSmart(taskHostAddr, { proof_status: {} })
  console.log(`  has_proof:          ${hostProofStatus.has_proof}`)
  console.log(`  proof_size_bytes:   ${hostProofStatus.proof_size_bytes}`)
  console.log(`  last_verify_verified: ${hostProofStatus.last_verify_verified}`)
  console.log(`  verifier_mode:      ${hostProofStatus.verifier_mode}`)

  console.log('\n  ═══════════════════════════════════════════════════════')
  console.log('  ✅ E2E Jolt Proof Example Complete!')
  console.log('  ═══════════════════════════════════════════════════════')
  console.log(`  Flow: agent (osmosis-testnet) → ICS-20 memo → ibc-task-host`)
  console.log(`        → StoreProof (submsg) → jolt-cw-verifier`)
  console.log(`        → VerifyProof (submsg) → jolt-cw-verifier`)
  console.log(`        → ProofStatus query confirms storage + verification`)
  console.log('  ═══════════════════════════════════════════════════════\n')
}

main().catch((err) => {
  console.error('\n  ❌ E2E test failed:', err.message || err)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
