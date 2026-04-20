// ────────────────────────────────────────────────────────────────────────────
//  JunoClaw — Tier 1.5 fresh deploy for `task-ledger` on uni-7
//
//  Context: the v6 `task-ledger` at juno17aq… was instantiated without a
//  wasmd-level migrate admin (deploy.mjs:240 omitted the 6th arg to
//  `client.instantiate`). That makes the contract permanently frozen at
//  code_id 70 — migrate-tier15.mjs cannot run against it.
//
//  This script follows yesterday's "Beating the Bounds" playbook (fresh
//  deploy, not migrate): upload new wasm, instantiate a new task-ledger
//  **with** the wasmd admin set this time, and re-wire the existing
//  agent-registry to point at the new address. The existing agent-registry
//  and escrow contracts are reused untouched (only their pointers change).
//
//  End state in `deployed.json`:
//    - `task-ledger`                 → new Tier 1.5 contract (migratable)
//    - `task-ledger-v6-frozen`       → preserved record of the old contract
//                                      (no wasmd admin, can never migrate)
//
//  Usage (PowerShell):
//    $env:PARLIAMENT_ROLE = 'The Builder'      # must match agent-registry's internal admin
//    node deploy/deploy-tier15-fresh.mjs
//
//  Opt-outs:
//    SKIP_UPLOAD=true    reuse deployed['task-ledger'].tier15_code_id from prior run
//    DRY_RUN=true        simulate — print the actions, don't broadcast
// ────────────────────────────────────────────────────────────────────────────

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const SKIP_UPLOAD = process.env.SKIP_UPLOAD === 'true'
const DRY_RUN = process.env.DRY_RUN === 'true'

const WASM_PATH = process.env.TASK_LEDGER_OPT_WASM
  || 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release\\task_ledger_opt.wasm'

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')
const DEPLOYED_FILE    = join(__dir, 'deployed.json')

function loadMnemonic() {
  if (process.env.MNEMONIC) return process.env.MNEMONIC
  if (!process.env.PARLIAMENT_ROLE) {
    console.error('❌  Set PARLIAMENT_ROLE (e.g. "The Builder") or MNEMONIC.')
    process.exit(1)
  }
  if (!existsSync(PARLIAMENT_STATE)) {
    console.error(`❌  ${PARLIAMENT_STATE} not found`)
    process.exit(1)
  }
  const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
  const mp = (state.mps || []).find((m) => m.name === process.env.PARLIAMENT_ROLE)
  if (!mp) {
    console.error(`❌  No MP named "${process.env.PARLIAMENT_ROLE}"`)
    process.exit(1)
  }
  console.log(`  Wallet:   ${process.env.PARLIAMENT_ROLE} (${mp.address})`)
  return mp.mnemonic
}

function loadDeployed() {
  if (!existsSync(DEPLOYED_FILE)) {
    console.error(`❌  ${DEPLOYED_FILE} not found — run deploy.mjs first.`)
    process.exit(1)
  }
  return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
  console.log(`  💾  ${DEPLOYED_FILE}`)
}

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║   JunoClaw Tier 1.5 — fresh task-ledger  ║')
  console.log('║   (v6 frozen; new contract with admin)   ║')
  console.log('╚══════════════════════════════════════════╝')
  console.log(`\n  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Gas:      ${GAS_PRICE}`)
  console.log(`  Wasm:     ${WASM_PATH}`)
  if (DRY_RUN) console.log('  ** DRY RUN — no transactions will be broadcast **')

  const mnemonic = loadMnemonic()
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address: sender }] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })
  console.log(`  Sender:   ${sender}\n`)

  const balance = await client.getBalance(sender, 'ujunox')
  console.log(`  Balance:  ${balance.amount} ${balance.denom}`)
  if (BigInt(balance.amount) < 5_000_000n) {
    console.warn('  ⚠  Low balance — faucet at https://faucet.reece.sh/?chain=uni-7\n')
  }

  const deployed = loadDeployed()
  const registryAddr = deployed['agent-registry']?.address
  const escrowAddr   = deployed['escrow']?.address
  const oldTl        = deployed['task-ledger']
  if (!registryAddr)  { console.error('❌  deployed.json missing agent-registry.address'); process.exit(1) }
  if (!escrowAddr)    { console.error('❌  deployed.json missing escrow.address'); process.exit(1) }
  if (!oldTl?.address){ console.error('❌  deployed.json missing task-ledger.address — nothing to replace'); process.exit(1) }

  console.log(`  agent-registry:  ${registryAddr}  (reused)`)
  console.log(`  escrow:          ${escrowAddr}    (reused)`)
  console.log(`  old task-ledger: ${oldTl.address}  (code_id ${oldTl.code_id}, will be frozen)\n`)

  // ── Pre-flight: confirm our sender is the registry's internal admin,
  //    because Step 3 below requires that permission. This catches a
  //    misconfigured PARLIAMENT_ROLE before burning any tx fee.
  try {
    const rcfg = await client.queryContractSmart(registryAddr, { get_config: {} })
    if (rcfg.admin !== sender) {
      console.error(
        `❌  agent-registry internal admin mismatch.\n` +
        `    Registry admin on chain: ${rcfg.admin}\n` +
        `    Sender wallet:           ${sender}\n` +
        `    UpdateRegistry in Step 3 requires admin permission.`,
      )
      process.exit(1)
    }
    console.log(`  ✅  agent-registry internal admin matches sender`)
  } catch (e) {
    console.error(`❌  agent-registry GetConfig failed: ${e.message}`)
    process.exit(1)
  }

  // ── Step 1: upload new wasm ────────────────────────────────────────────
  console.log('\n━━━  Step 1: upload task_ledger_opt.wasm  ━━━\n')

  let newCodeId = oldTl.tier15_code_id
  if (SKIP_UPLOAD && newCodeId) {
    console.log(`  ⏭  SKIP_UPLOAD=true — reusing tier15_code_id ${newCodeId}`)
  } else {
    if (!existsSync(WASM_PATH)) {
      console.error(
        `❌  Wasm not found: ${WASM_PATH}\n` +
        `    Build it first (see docs/V6_TESTNET_RUN.md build notes).`,
      )
      process.exit(1)
    }
    const wasm = readFileSync(WASM_PATH)
    console.log(`  📦  Uploading ${(wasm.length / 1024).toFixed(1)} KB…`)
    if (DRY_RUN) {
      console.log('  (dry run: skipping upload)')
    } else {
      const up = await client.upload(sender, wasm, 'auto', 'JunoClaw task-ledger Tier 1.5')
      newCodeId = up.codeId
      console.log(`  ✅  code_id: ${newCodeId}  tx: ${up.transactionHash}`)
      // Record the upload tx on the pending object; it lands in deployed.json
      // when Step 2 completes.
      oldTl.tier15_store_tx = up.transactionHash
    }
  }

  if (!newCodeId && !DRY_RUN) {
    console.error('❌  No code_id available after upload step.')
    process.exit(1)
  }

  // ── Step 2: instantiate a new task-ledger WITH wasmd admin ────────────
  console.log('\n━━━  Step 2: instantiate new task-ledger (with wasmd admin!)  ━━━\n')

  // The task-ledger InstantiateMsg shape used in deploy.mjs step 4 —
  // admin (internal), agent_registry, operators, agent_company=null.
  // agent_company is left null on purpose: smoke-tier15 doesn't need it.
  // It can be wired later via an admin-only UpdateConfig if the real
  // end-to-end flow ever requires CompleteTask → agent-company callbacks.
  const instantiateMsg = {
    admin: sender,
    agent_registry: registryAddr,
    operators: [],
    agent_company: null,
  }

  let newTlAddress, instantiateTx
  if (DRY_RUN) {
    console.log(`  (dry run) would send: client.instantiate(${sender}, ${newCodeId}, ${JSON.stringify(instantiateMsg)}, 'JunoClaw Task Ledger Tier 1.5', 'auto', { admin: ${sender} })`)
    newTlAddress = '(dry-run-placeholder)'
  } else {
    const res = await client.instantiate(
      sender,
      newCodeId,
      instantiateMsg,
      'JunoClaw Task Ledger Tier 1.5',
      'auto',
      // This is the critical line that yesterday's deploy.mjs omitted.
      // Setting the wasmd-level admin lets a future migrate succeed.
      { admin: sender },
    )
    newTlAddress = res.contractAddress
    instantiateTx = res.transactionHash
    console.log(`  ✅  new task-ledger address: ${newTlAddress}`)
    console.log(`      instantiate tx:         ${instantiateTx}`)
    console.log(`      wasmd admin:            ${sender}`)
  }

  // Verify the wasmd admin actually got set (defensive — the 6th-arg shape
  // depends on the cosmjs version we're running).
  if (!DRY_RUN) {
    const info = await client.getContract(newTlAddress)
    if (info.admin !== sender) {
      console.error(
        `❌  wasmd admin not set on new contract!\n` +
        `    Contract admin: ${info.admin || '(none)'}\n` +
        `    Expected:       ${sender}\n` +
        `    The instantiation shape needs fixing for this cosmjs version.`,
      )
      process.exit(1)
    }
    console.log(`  ✅  wasmd admin verified on-chain: ${info.admin}`)
  }

  // ── Step 3: rewire agent-registry.registry.task_ledger ─────────────────
  console.log('\n━━━  Step 3: UpdateRegistry on agent-registry  ━━━\n')

  if (DRY_RUN) {
    console.log(`  (dry run) would send: agent-registry.UpdateRegistry { task_ledger: ${newTlAddress}, escrow: ${escrowAddr} }`)
  } else {
    const updateMsg = { update_registry: {
      agent_registry: null,
      task_ledger: newTlAddress,
      escrow: escrowAddr,
    } }
    const res = await client.execute(sender, registryAddr, updateMsg, 'auto', 'JunoClaw Tier 1.5 rewire')
    console.log(`  ✅  wired agent-registry.registry.task_ledger  tx: ${res.transactionHash}`)

    // Read-back verification.
    const cfg = await client.queryContractSmart(registryAddr, { get_config: {} })
    if (cfg?.registry?.task_ledger !== newTlAddress) {
      console.error(
        `❌  UpdateRegistry read-back mismatch!\n` +
        `    Expected registry.task_ledger: ${newTlAddress}\n` +
        `    Got:                           ${cfg?.registry?.task_ledger}`,
      )
      process.exit(1)
    }
    console.log(`  ✅  registry.task_ledger now points at Tier 1.5 contract`)
  }

  // ── Step 4: update deployed.json ───────────────────────────────────────
  console.log('\n━━━  Step 4: record to deployed.json  ━━━\n')

  if (!DRY_RUN) {
    // Preserve the frozen v6 record for audit history.
    deployed['task-ledger-v6-frozen'] = {
      ...oldTl,
      note: 'Frozen v6 contract (no wasmd admin, non-migratable). Retired in favour of Tier 1.5 fresh deploy at the address in task-ledger.',
      retired_at: new Date().toISOString(),
    }

    // Overwrite the primary task-ledger entry so smoke-tier15.mjs and future
    // tooling pick up the new address.
    deployed['task-ledger'] = {
      code_id: newCodeId,
      address: newTlAddress,
      store_tx: oldTl.tier15_store_tx,            // upload tx for the new code
      instantiate_tx: instantiateTx,
      wasmd_admin: sender,
      tier15_code_id: newCodeId,                  // lets smoke-tier15 skip its warning
      tier15_store_tx: oldTl.tier15_store_tx,
      tier15_instantiate_tx: instantiateTx,
      tier15_deployed_at: new Date().toISOString(),
      deployment_mode: 'tier15-fresh',
    }

    saveDeployed(deployed)
  } else {
    console.log('  (dry run: skipping deployed.json update)')
  }

  // ── Summary ─────────────────────────────────────────────────────────────
  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
  console.log('  Tier 1.5 fresh deploy complete.\n')
  if (!DRY_RUN) {
    console.log(`  New task-ledger:  ${newTlAddress}`)
    console.log(`  code_id:          ${newCodeId}`)
    console.log(`  wasmd admin:      ${sender}`)
    console.log(`  v6 frozen copy:   ${oldTl.address}`)
  }
  console.log('\n  Next: node deploy/smoke-tier15.mjs\n')
  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  deploy-tier15-fresh failed:', err.message)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
