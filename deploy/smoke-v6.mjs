// ────────────────────────────────────────────────────────────────────────────
//  JunoClaw V6 on-chain smoke tests
//
//  Runs F1/F2/F3 regressions against a freshly deployed v6 stack on uni-7.
//  Uses two parliament wallets (admin + stranger) loaded from
//  ../wavs/bridge/parliament-state.json (gitignored).
//
//  F1  task-ledger submitter cannot self-complete (Unauthorized)
//  F2  agent-company DistributePayment rejects non-admin / non-member
//  F3  builder-grant rejects duplicate work_hash (DuplicateWorkHash)
//  F4  junoswap-pair unexpected-denom rejection  →  covered by unit tests only;
//      minting a rogue denom is not feasible on live testnet.
//
//  Usage (PowerShell):
//    $env:PARLIAMENT_ROLE = 'The Builder'
//    $env:STRANGER_ROLE   = 'The Contrarian'
//    node deploy/smoke-v6.mjs
// ────────────────────────────────────────────────────────────────────────────

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice, coins } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')
const DEPLOYED_FILE    = join(__dir, 'deployed.json')
const RESULTS_FILE     = join(__dir, 'smoke-v6-results.json')

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

// Assert an execute call REJECTS. Returns the captured error message.
async function expectReject(label, fn) {
  try {
    await fn()
  } catch (err) {
    const msg = err?.message || String(err)
    console.log(`  ✅  ${label}: rejected → ${msg.split('\n')[0].slice(0, 140)}`)
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
    console.log(`  ❌  ${label}: unexpected failure → ${msg.split('\n')[0].slice(0, 140)}`)
    return { ok: false, error: msg }
  }
}

// Walk the `events` array returned by execute/instantiate and return the first
// attribute value matching `key`. Returns `Number(value)` so the caller can
// use the id directly in subsequent messages.
function extractEventAttr(result, key) {
  for (const ev of result?.events || []) {
    for (const a of ev.attributes || []) {
      if (a.key === key) return Number(a.value)
    }
  }
  return null
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║     JunoClaw V6 on-chain smoke tests     ║')
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
  const agentCo      = deployed['agent-company']?.address
  const builderGrantCodeId = deployed['builder-grant']?.code_id
  if (!registryAddr || !taskLedger || !agentCo) {
    throw new Error('Core stack not deployed. Run deploy.mjs first.')
  }

  const results = {
    chain_id: CHAIN_ID,
    ts: new Date().toISOString(),
    admin_role: adminRole,
    stranger_role: strangerRole,
    addresses: { registryAddr, taskLedger, agentCo, builderGrantCodeId },
    tests: {},
  }

  // ── F1: task-ledger submitter cannot self-complete ───────────────────────
  console.log('━━━  F1: task-ledger submitter self-complete must be rejected  ━━━\n')

  // Register a fresh agent each run — agent-registry emits `agent_id` as a wasm
  // event attribute so we don't need a follow-up query (AgentProfile doesn't
  // carry the id).
  const reg = await expectOk('stranger registers agent', () =>
    stranger.client.execute(
      stranger.address,
      registryAddr,
      { register_agent: {
        name: `smoke-agent-${Date.now()}`,
        description: 'v6 smoke test',
        capabilities_hash: 'sha256:0000000000000000000000000000000000000000000000000000000000000001',
        model: 'smoke',
      } },
      'auto',
    ),
  )
  if (!reg.ok) {
    results.tests.F1 = { ok: false, stage: 'register_agent', error: reg.error }
    saveResults(results); return
  }
  const agentId = extractEventAttr(reg.result, 'agent_id')
  if (agentId === null) {
    results.tests.F1 = { ok: false, stage: 'extract_agent_id', error: 'no agent_id event attr' }
    saveResults(results); return
  }
  console.log(`  ℹ   agent_id: ${agentId}`)

  const inputHash = 'sha256:' + '1'.repeat(64)
  const submit = await expectOk('stranger submits task', () =>
    stranger.client.execute(
      stranger.address,
      taskLedger,
      { submit_task: {
        agent_id: agentId,
        input_hash: inputHash,
        execution_tier: 'local',
      } },
      'auto',
    ),
  )
  if (!submit.ok) { results.tests.F1 = { ok: false, stage: 'submit_task', error: submit.error }; saveResults(results); return }
  const taskId = extractEventAttr(submit.result, 'task_id')
  console.log(`  ℹ   task_id: ${taskId}`)

  const selfComplete = await expectReject('stranger tries CompleteTask on own task', () =>
    stranger.client.execute(
      stranger.address,
      taskLedger,
      { complete_task: {
        task_id: taskId,
        output_hash: 'sha256:' + '2'.repeat(64),
        cost_ujuno: null,
      } },
      'auto',
    ),
  )

  const adminComplete = await expectOk('admin completes task', () =>
    admin.client.execute(
      admin.address,
      taskLedger,
      { complete_task: {
        task_id: taskId,
        output_hash: 'sha256:' + '2'.repeat(64),
        cost_ujuno: null,
      } },
      'auto',
    ),
  )

  results.tests.F1 = {
    ok: selfComplete.ok && adminComplete.ok,
    task_id: taskId,
    agent_id: agentId,
    self_complete_rejected: selfComplete.ok,
    admin_complete_ok: adminComplete.ok,
    self_complete_error: selfComplete.error,
  }
  saveResults(results)

  // ── F2: DistributePayment gated to admin / member ────────────────────────
  console.log('\n━━━  F2: DistributePayment must reject non-admin / non-member  ━━━\n')

  const distTaskId = taskId ?? 0
  const strangerDist = await expectReject('stranger calls DistributePayment', () =>
    stranger.client.execute(
      stranger.address,
      agentCo,
      { distribute_payment: { task_id: distTaskId } },
      'auto',
      undefined,
      coins(1000, 'ujunox'),
    ),
  )

  const adminDist = await expectOk('admin calls DistributePayment', () =>
    admin.client.execute(
      admin.address,
      agentCo,
      { distribute_payment: { task_id: distTaskId } },
      'auto',
      undefined,
      coins(1000, 'ujunox'),
    ),
  )

  results.tests.F2 = {
    ok: strangerDist.ok && adminDist.ok,
    stranger_rejected: strangerDist.ok,
    admin_ok: adminDist.ok,
    stranger_error: strangerDist.error,
  }
  saveResults(results)

  // ── F3: builder-grant duplicate work_hash rejected ───────────────────────
  console.log('\n━━━  F3: builder-grant duplicate work_hash must be rejected  ━━━\n')

  if (!builderGrantCodeId) {
    console.log('  ⏭  builder-grant not stored — skipping F3')
    results.tests.F3 = { ok: null, skipped: 'builder-grant code_id not in deployed.json' }
    saveResults(results)
  } else {
    const bgInst = await expectOk('instantiate fresh builder-grant', () =>
      admin.client.instantiate(
        admin.address,
        builderGrantCodeId,
        {
          denom: 'ujunox',
          operators: [admin.address],
          agent_company: null,
        },
        'v6 smoke builder-grant',
        'auto',
      ),
    )
    if (!bgInst.ok) {
      results.tests.F3 = { ok: false, stage: 'instantiate', error: bgInst.error }
    } else {
      const bgAddr = bgInst.result.contractAddress
      console.log(`  ℹ   builder-grant: ${bgAddr}`)
      // builder-grant `is_valid_hex64` enforces exactly 64 ASCII hex chars,
      // no prefix. Using a timestamp-seeded hash so reruns don't collide
      // with state left from earlier smoke invocations against the same
      // builder-grant instance.
      const seed = Date.now().toString(16).padStart(16, '0')
      const workHash = (seed + 'a'.repeat(64)).slice(0, 64)
      const first = await expectOk('first work submission', () =>
        stranger.client.execute(
          stranger.address,
          bgAddr,
          { submit_work: {
            tier: 'contract_deploy',
            evidence: 'tx/ABC123',
            work_hash: workHash,
          } },
          'auto',
        ),
      )
      const dup = await expectReject('duplicate work_hash rejected', () =>
        stranger.client.execute(
          stranger.address,
          bgAddr,
          { submit_work: {
            tier: 'contract_deploy',
            evidence: 'tx/DEF456',
            work_hash: workHash,
          } },
          'auto',
        ),
      )
      results.tests.F3 = {
        ok: first.ok && dup.ok,
        builder_grant_addr: bgAddr,
        first_ok: first.ok,
        duplicate_rejected: dup.ok,
        duplicate_error: dup.error,
      }
    }
    saveResults(results)
  }

  // ── Summary ──────────────────────────────────────────────────────────────
  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
  console.log('  Summary:')
  for (const [id, r] of Object.entries(results.tests)) {
    const icon = r.ok === true ? '✅' : r.ok === false ? '❌' : '⏭ '
    console.log(`    ${icon}  ${id}  ${r.skipped ? r.skipped : ''}`)
  }
  console.log(`\n  Results: ${RESULTS_FILE}`)
  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  smoke-v6 failed:', err.message)
  console.error(err.stack)
  process.exit(1)
})
