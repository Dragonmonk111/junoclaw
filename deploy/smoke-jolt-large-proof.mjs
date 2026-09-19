import { readFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

// Smoke test: store a REAL ~68KB Jolt proof on jolt-cw-verifier.
// Regression for the 128KB MAX_LENGTH_DB_VALUE bug — Item<T> JSON-serialized
// Vec<u8> (~3.5x expansion) so 68KB proofs failed with "Value too big".
// Fixed by raw-bytes storage (commit 7e13d47). This script proves it on uni-7.

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno.rpc.t.stavr.tech'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const PROOF_PATH = process.env.PROOF_PATH || 'C:\\Temp\\jolt-artifacts\\fib_proof.bin'

const DEPLOYED_FILE = join(__dir, 'deployed-testnet.json')
const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

function loadMnemonic() {
  if (process.env.MNEMONIC) return process.env.MNEMONIC
  if (process.env.PARLIAMENT_ROLE) {
    const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
    const mp = (state.mps || []).find((m) => m.name === process.env.PARLIAMENT_ROLE)
    if (!mp) { console.error(`No MP "${process.env.PARLIAMENT_ROLE}"`); process.exit(1) }
    console.log(`  Wallet: ${mp.name} (${mp.address})`)
    return mp.mnemonic
  }
  console.error('Set MNEMONIC or PARLIAMENT_ROLE')
  process.exit(1)
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)) }

async function main() {
  console.log('\n  === Smoke: store real 68KB Jolt proof on uni-7 ===')
  console.log(`  Chain: ${CHAIN_ID}  RPC: ${RPC_URL}`)

  const deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  const contractAddr = deployed['jolt-cw-verifier']?.address
  if (!contractAddr) { console.error('  jolt-cw-verifier not in deployed-testnet.json'); process.exit(1) }
  console.log(`  Contract: ${contractAddr}`)

  if (!existsSync(PROOF_PATH)) { console.error(`  Proof not found: ${PROOF_PATH}`); process.exit(1) }
  const proofBytes = readFileSync(PROOF_PATH)
  console.log(`  Proof: ${PROOF_PATH} (${proofBytes.length} bytes)`)

  const mnemonic = loadMnemonic()
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  // store_proof — the tx that failed with "Value too big" on the old contract
  console.log('\n  Sending store_proof...')
  const msg = { store_proof: { proof_base64: proofBytes.toString('base64'), program_hash: null } }
  let txHash
  try {
    const res = await client.execute(address, contractAddr, msg, 'auto', 'smoke: 68KB proof raw storage')
    txHash = res.transactionHash
  } catch (err) {
    if ((err?.message || '').includes('transaction indexing is disabled')) {
      console.log('  (tx indexing disabled, waiting...)')
      await sleep(8000)
      txHash = 'unknown (tx indexing disabled)'
    } else throw err
  }
  console.log(`  store_proof tx: ${txHash}`)

  // verify_proof (structural mode on uni-7 — no bn254 host fns on stock wasmd)
  console.log('  Sending verify_proof (structural)...')
  try {
    const res = await client.execute(address, contractAddr, { verify_proof: { proof_base64: null, public_io_base64: null } }, 'auto', 'smoke: verify stored proof')
    console.log(`  verify_proof tx: ${res.transactionHash}`)
  } catch (err) {
    if ((err?.message || '').includes('transaction indexing is disabled')) {
      await sleep(8000)
      console.log('  verify_proof tx: unknown (tx indexing disabled)')
    } else throw err
  }

  const status = await client.queryContractSmart(contractAddr, { proof_status: {} })
  console.log('\n  proof_status:')
  console.log(`    has_proof:            ${status.has_proof}`)
  console.log(`    proof_size_bytes:     ${status.proof_size_bytes}`)
  console.log(`    proof_sha256:         ${status.proof_sha256}`)
  console.log(`    last_verify_verified: ${status.last_verify_verified}`)
  console.log(`    verifier_mode:        ${status.verifier_mode}`)

  if (status.has_proof && status.proof_size_bytes === proofBytes.length) {
    console.log('\n  ✅ PASS — 68KB proof stored raw on uni-7 (128KB limit no longer hit)')
  } else {
    console.log('\n  ⚠️  Unexpected status — check above')
    process.exit(1)
  }
}

main().catch((err) => {
  console.error('\n  ❌ Smoke test failed:', err.message || err)
  process.exit(1)
})
