// ────────────────────────────────────────────────────────────────────────────
//  JunoClaw Tier 1.5 — on-chain smoke tests for the new Constraint variants
//
//  Exercises the three variants added in Tier 1.5 against a migrated
//  task-ledger on uni-7:
//    T1  TimeAfter { unix_seconds }            — wall-clock gating
//    T2  BlockHeightAtLeast { height }         — block-count gating
//    T3  EscrowObligationConfirmed { ... }     — payment-settled gating
//
//  Each test uses the same "submit with hook → try complete → observe →
//  advance state → retry" pattern. The atomic-revert guarantee is what
//  makes this safe: a failing hook leaves the task Running, so the retry
//  is the exact same task.
//
//  Usage (PowerShell):
//    $env:PARLIAMENT_ROLE = 'The Builder'
//    $env:STRANGER_ROLE   = 'The Contrarian'
//    node deploy/smoke-tier15.mjs
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

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')
const DEPLOYED_FILE    = join(__dir, 'deployed.json')
const RESULTS_FILE     = join(__dir, 'smoke-tier15-results.json')

// Tunables. Keep these generous on a public testnet where block time varies.
const TIME_AFTER_OFFSET_SEC   = Number(process.env.TIME_AFTER_OFFSET_SEC || 60)
const TIME_AFTER_MAX_WAIT_SEC = Number(process.env.TIME_AFTER_MAX_WAIT_SEC || 900) // cap: 15 minutes
const BLOCK_HEIGHT_OFFSET     = Number(process.env.BLOCK_HEIGHT_OFFSET || 6)
const POLL_INTERVAL_MS        = 2000
const SMOKE_ONLY              = process.env.SMOKE_ONLY || '' // e.g. 'T1' or 'T1,T3'

// ── Helpers ─────────────────────────────────────────────────────────────────

function loadParliament() {
  if (!existsSync(PARLIAMENT_STATE)) {
    throw new Error(`parliament-state.json not found at ${PARLIAMENT_STATE}`)
  }
  return JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
}

function findMp(state, role) {
  const mp = (state.mps || []).find((m) => m.name === role)
  if (!mp) throw new Error(`No MP with name "${role}" in parliament-state.json`)
  return mp
}

async function connect(mnemonic) {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })
  return { wallet, address, client }
}

function loadDeployed() {
  if (!existsSync(DEPLOYED_FILE)) throw new Error(`${DEPLOYED_FILE} not found — run deploy.mjs first`)
  return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
}

function saveResults(results) {
  writeFileSync(RESULTS_FILE, JSON.stringify(results, null, 2))
  console.log(`  💾  ${RESULTS_FILE}`)
}

async function expectReject(label, fn) {
  try { await fn() } catch (err) {
    const msg = err?.message || String(err)
    console.log(`  ✅  ${label}: rejected → ${msg.split('\n')[0].slice(0, 160)}`)
    return { ok: true, error: msg }
  }
  console.log(`  ❌  ${label}: expected rejection but call succeeded`)
  return { ok: false, error: null }
}

async function expectOk(label, fn) {
  try {
    const r = await fn()
    const tx = r?.transactionHash || '(no tx)'
    console.log(`  ✅  ${label}: ok  tx: ${tx}`)
    return { ok: true, tx, result: r }
  } catch (err) {
    const msg = err?.message || String(err)
    console.log(`  ❌  ${label}: unexpected failure → ${msg.split('\n')[0].slice(0, 160)}`)
    return { ok: false, error: msg }
  }
}

function extractEventAttr(result, key) {
  for (const ev of result?.events || []) {
    for (const a of ev.attributes || []) {
      if (a.key === key) return a.value
    }
  }
  return null
}

async function registerAgent(registryAddr, stranger) {
  const reg = await expectOk('stranger registers agent', () =>
    stranger.client.execute(
      stranger.address,
      registryAddr,
      { register_agent: {
        name: `tier15-agent-${Date.now()}`,
        description: 'tier15 smoke',
        capabilities_hash: 'sha256:' + '1'.repeat(64),
        model: 'smoke',
      } },
      'auto',
    ),
  )
  if (!reg.ok) throw new Error(`register_agent failed: ${reg.error}`)
  const agentId = Number(extractEventAttr(reg.result, 'agent_id'))
  if (!Number.isFinite(agentId)) throw new Error('could not extract agent_id from events')
  console.log(`  ℹ   agent_id: ${agentId}`)
  return agentId
}

async function submitWithHooks(stranger, taskLedger, agentId, preHooks, postHooks = []) {
  const sub = await expectOk('stranger submits task', () =>
    stranger.client.execute(
      stranger.address,
      taskLedger,
      { submit_task: {
        agent_id: agentId,
        input_hash: 'sha256:' + Date.now().toString(16).padStart(64, '0').slice(-64),
        execution_tier: 'local',
        pre_hooks: preHooks,
        post_hooks: postHooks,
        proposal_id: null,
      } },
      'auto',
    ),
  )
  if (!sub.ok) throw new Error(`submit_task failed: ${sub.error}`)
  const taskId = Number(extractEventAttr(sub.result, 'task_id'))
  console.log(`  ℹ   task_id: ${taskId}`)
  return taskId
}

async function completeTask(admin, taskLedger, taskId) {
  return admin.client.execute(
    admin.address,
    taskLedger,
    { complete_task: {
      task_id: taskId,
      output_hash: 'sha256:' + '2'.repeat(64),
      cost_ujuno: null,
    } },
    'auto',
  )
}

async function waitUntilHeight(client, target) {
  process.stdout.write(`  ⏳  waiting for height >= ${target} `)
  while (true) {
    const h = await client.getHeight()
    if (h >= target) {
      process.stdout.write(` (now ${h})\n`)
      return h
    }
    process.stdout.write('.')
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS))
  }
}

// Fetches the latest block's header time (seconds since epoch). CosmWasm
// `env.block.time` is driven by the Tendermint block header, so we must
// test against chain time, not local wall-clock — public testnets like
// uni-7 regularly drift tens of minutes from NTP.
async function getChainTimeSec(client) {
  const block = await client.getBlock()
  return Math.floor(new Date(block.header.time).getTime() / 1000)
}

async function waitUntilChainTime(client, targetSec, maxWaitSec) {
  const start = Date.now()
  process.stdout.write(`  ⏳  waiting for chain time >= ${targetSec} `)
  while (true) {
    const t = await getChainTimeSec(client)
    if (t >= targetSec) {
      process.stdout.write(` (now ${t})\n`)
      return t
    }
    if ((Date.now() - start) / 1000 > maxWaitSec) {
      process.stdout.write(` (timeout after ${maxWaitSec}s at chain time ${t})\n`)
      throw new Error(`chain time did not reach ${targetSec} within ${maxWaitSec}s — last seen ${t}`)
    }
    process.stdout.write('.')
    await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS))
  }
}

async function sleep(ms) {
  await new Promise((r) => setTimeout(r, ms))
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║   JunoClaw Tier 1.5 — on-chain smoke     ║')
  console.log('╚══════════════════════════════════════════╝\n')

  const parliament = loadParliament()
  const adminRole    = process.env.PARLIAMENT_ROLE || 'The Builder'
  const strangerRole = process.env.STRANGER_ROLE   || 'The Contrarian'
  const adminMp    = findMp(parliament, adminRole)
  const strangerMp = findMp(parliament, strangerRole)

  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Admin:    ${adminRole} (${adminMp.address})`)
  console.log(`  Stranger: ${strangerRole} (${strangerMp.address})\n`)

  const admin    = await connect(adminMp.mnemonic)
  const stranger = await connect(strangerMp.mnemonic)

  const deployed = loadDeployed()
  const registryAddr = deployed['agent-registry']?.address
  const taskLedger   = deployed['task-ledger']?.address
  const escrowAddr   = deployed['escrow']?.address
  if (!registryAddr || !taskLedger || !escrowAddr) {
    throw new Error('agent-registry / task-ledger / escrow addresses missing from deployed.json')
  }
  const tier15CodeId = deployed['task-ledger']?.tier15_code_id
  if (!tier15CodeId) {
    console.warn('  ⚠  deployed.json has no tier15_code_id — run migrate-tier15.mjs first, or proceed only if you know the contract is already on Tier 1.5.')
  }

  const results = {
    chain_id: CHAIN_ID,
    ts: new Date().toISOString(),
    admin_role: adminRole,
    stranger_role: strangerRole,
    addresses: { registryAddr, taskLedger, escrowAddr, tier15CodeId },
    tests: {},
  }

  // ── T1 — TimeAfter ──────────────────────────────────────────
  if (!process.env.SMOKE_ONLY || process.env.SMOKE_ONLY.split(',').map(s => s.trim()).includes('T1')) {
  console.log('━━━  T1: TimeAfter  ━━━\n')
  try {
    const agentId = await registerAgent(registryAddr, stranger)
    // Base the threshold on CHAIN time, not local wall-clock. Public
    // testnets (uni-7 especially) drift minutes behind NTP — using
    // local time would produce a threshold the chain may not reach
    // for a very long while.
    const chainNow = await getChainTimeSec(admin.client)
    const threshold = chainNow + TIME_AFTER_OFFSET_SEC
    console.log(`  ℹ   chain time: ${chainNow}`)
    console.log(`  ℹ   threshold:  ${threshold} (chain_time + ${TIME_AFTER_OFFSET_SEC}s)`)

    const taskId = await submitWithHooks(stranger, taskLedger, agentId, [
      { time_after: { unix_seconds: threshold } },
    ])

    const earlyFail = await expectReject('admin CompleteTask before threshold', () =>
      completeTask(admin, taskLedger, taskId),
    )

    await waitUntilChainTime(admin.client, threshold, TIME_AFTER_MAX_WAIT_SEC)

    const lateOk = await expectOk('admin CompleteTask after threshold', () =>
      completeTask(admin, taskLedger, taskId),
    )

    results.tests.T1 = {
      ok: earlyFail.ok && lateOk.ok,
      task_id: taskId,
      agent_id: agentId,
      chain_time_at_submit: chainNow,
      threshold_unix: threshold,
      early_reject_error: earlyFail.error,
      late_ok_tx: lateOk.tx,
    }
  } catch (e) {
    console.log(`  ❌  T1 setup failed: ${e.message}`)
    results.tests.T1 = { ok: false, error: e.message }
  }
  saveResults(results)
  }

  // ── T2 — BlockHeightAtLeast ────────────────────────────────
  if (!process.env.SMOKE_ONLY || process.env.SMOKE_ONLY.split(',').map(s => s.trim()).includes('T2')) {
  console.log('\n━━━  T2: BlockHeightAtLeast  ━━━\n')
  try {
    const agentId = await registerAgent(registryAddr, stranger)
    const currentHeight = await admin.client.getHeight()
    const threshold = currentHeight + BLOCK_HEIGHT_OFFSET
    console.log(`  ℹ   threshold: ${threshold} (current ${currentHeight} + ${BLOCK_HEIGHT_OFFSET})`)

    const taskId = await submitWithHooks(stranger, taskLedger, agentId, [
      { block_height_at_least: { height: threshold } },
    ])

    const earlyFail = await expectReject('admin CompleteTask before threshold', () =>
      completeTask(admin, taskLedger, taskId),
    )

    await waitUntilHeight(admin.client, threshold)

    const lateOk = await expectOk('admin CompleteTask after threshold', () =>
      completeTask(admin, taskLedger, taskId),
    )

    results.tests.T2 = {
      ok: earlyFail.ok && lateOk.ok,
      task_id: taskId,
      agent_id: agentId,
      threshold_height: threshold,
      early_reject_error: earlyFail.error,
      late_ok_tx: lateOk.tx,
    }
  } catch (e) {
    console.log(`  ❌  T2 setup failed: ${e.message}`)
    results.tests.T2 = { ok: false, error: e.message }
  }
  saveResults(results)
  }

  // ── T3 — EscrowObligationConfirmed ────────────────────────────
  if (!process.env.SMOKE_ONLY || process.env.SMOKE_ONLY.split(',').map(s => s.trim()).includes('T3')) {
  console.log('\n━━━  T3: EscrowObligationConfirmed  ━━━\n')
  try {
    const agentId = await registerAgent(registryAddr, stranger)

    // A task_id chosen for the escrow obligation. Must not collide with
    // any existing obligation on escrow — using the unix ms modulo
    // u53-safe so the JSON number encoding is lossless.
    const escrowTaskId = Date.now() % 1_000_000_000
    console.log(`  ℹ   escrow task_id slot: ${escrowTaskId}`)

    // Admin Authorize — records a Pending obligation keyed by escrowTaskId.
    const auth = await expectOk('admin Authorize escrow obligation', () =>
      admin.client.execute(
        admin.address,
        escrowAddr,
        { authorize: {
          task_id: escrowTaskId,
          payee: stranger.address,
          amount: '1',
        } },
        'auto',
      ),
    )
    if (!auth.ok) throw new Error(`authorize failed: ${auth.error}`)

    const ledgerTaskId = await submitWithHooks(stranger, taskLedger, agentId, [
      { escrow_obligation_confirmed: { escrow: escrowAddr, task_id: escrowTaskId } },
    ])

    const pendingFail = await expectReject(
      'admin CompleteTask while escrow is still Pending',
      () => completeTask(admin, taskLedger, ledgerTaskId),
    )

    // Flip the obligation to Confirmed.
    const confirm = await expectOk('admin Confirm escrow obligation', () =>
      admin.client.execute(
        admin.address,
        escrowAddr,
        { confirm: { task_id: escrowTaskId, tx_hash: null } },
        'auto',
      ),
    )
    if (!confirm.ok) throw new Error(`confirm failed: ${confirm.error}`)

    const confirmedOk = await expectOk(
      'admin CompleteTask after Confirmed',
      () => completeTask(admin, taskLedger, ledgerTaskId),
    )

    results.tests.T3 = {
      ok: pendingFail.ok && confirmedOk.ok,
      ledger_task_id: ledgerTaskId,
      escrow_task_id: escrowTaskId,
      agent_id: agentId,
      authorize_tx: auth.tx,
      pending_reject_error: pendingFail.error,
      confirm_tx: confirm.tx,
      complete_ok_tx: confirmedOk.tx,
    }
  } catch (e) {
    console.log(`  ❌  T3 setup failed: ${e.message}`)
    results.tests.T3 = { ok: false, error: e.message }
  }
  saveResults(results)
  }

  // ── Summary ──────────────────────────────────────────────────────────────
  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
  console.log('  Summary:')
  for (const [id, r] of Object.entries(results.tests)) {
    const icon = r.ok === true ? '✅' : r.ok === false ? '❌' : '⏭ '
    console.log(`    ${icon}  ${id}`)
  }
  console.log(`\n  Results: ${RESULTS_FILE}\n`)
  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  smoke-tier15 failed:', err.message)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
