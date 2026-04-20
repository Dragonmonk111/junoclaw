// ────────────────────────────────────────────────────────────────────────────
//  JunoClaw — Tier 1.5 migration for `task-ledger` on uni-7
//
//  Flow:
//    1. Upload the new task_ledger_opt.wasm → fresh code_id.
//    2. Migrate the existing task-ledger contract (from deployed.json) to
//       the new code_id. The MigrateMsg is empty: every new field on
//       TaskRecord (pre_hooks, post_hooks) is #[serde(default)], so existing
//       stored records deserialise with empty Vecs and no state rewrite is
//       needed.
//    3. Sanity-check via GetConfig that the contract still answers.
//    4. Record { tier15_code_id, migrate_tx, migrated_at } in deployed.json
//       under task-ledger.
//
//  Usage (PowerShell):
//    $env:PARLIAMENT_ROLE = 'The Builder'   # must be the wasmd admin of the contract
//    node deploy/migrate-tier15.mjs
//
//  Opt-outs:
//    SKIP_UPLOAD=true   reuse deployed['task-ledger'].tier15_code_id from prior run
//    DRY_RUN=true       simulate — print the actions, don't broadcast
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
    console.error(`❌  ${DEPLOYED_FILE} not found — is this a fresh machine? Run deploy.mjs first.`)
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
  console.log('║   JunoClaw Tier 1.5 — task-ledger        ║')
  console.log('║   migration (v6 → hooks)                 ║')
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
  const tl = deployed['task-ledger']
  if (!tl?.address) {
    console.error('❌  deployed.json has no task-ledger.address — nothing to migrate.')
    process.exit(1)
  }

  console.log(`  Target:   task-ledger @ ${tl.address}`)
  console.log(`  Current code_id: ${tl.code_id}\n`)

  // Verify the sender is the wasmd-level admin of the contract. If not, the
  // migrate tx will be rejected by the chain; we catch it here with a clear
  // diagnostic rather than after a failed broadcast.
  const info = await client.getContract(tl.address)
  if (info.admin !== sender) {
    console.error(
      `❌  Migrate authority mismatch.\n` +
      `    Contract admin on chain: ${info.admin || '(none)'}\n` +
      `    Sender wallet:           ${sender}\n` +
      `    Only the wasmd-level admin can migrate. Set PARLIAMENT_ROLE to the right wallet.`,
    )
    process.exit(1)
  }

  // ── Step 1: upload new wasm ────────────────────────────────────────────
  console.log('━━━  Step 1: upload task_ledger_opt.wasm  ━━━\n')

  let newCodeId = tl.tier15_code_id
  if (SKIP_UPLOAD && newCodeId) {
    console.log(`  ⏭  SKIP_UPLOAD=true — reusing tier15_code_id ${newCodeId}`)
  } else {
    if (!existsSync(WASM_PATH)) {
      console.error(
        `❌  Wasm not found: ${WASM_PATH}\n` +
        `    Build it first (see docs/V6_TESTNET_RUN.md build notes):\n` +
        `      $env:CARGO_TARGET_DIR='C:\\Temp\\junoclaw-wasm-target'\n` +
        `      $env:RUSTFLAGS='-C link-arg=-s'\n` +
        `      cargo build --target wasm32-unknown-unknown --release --lib -p task-ledger\n` +
        `      wasm-opt --enable-sign-ext --signext-lowering --strip-target-features --strip-debug -Oz \` \n` +
        `        -o C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release\\task_ledger_opt.wasm \` \n` +
        `        C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release\\task_ledger.wasm`,
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
      tl.tier15_store_tx = up.transactionHash
    }
  }

  if (!newCodeId && !DRY_RUN) {
    console.error('❌  No code_id available after upload step.')
    process.exit(1)
  }

  // ── Step 2: migrate contract to new code_id ────────────────────────────
  console.log('\n━━━  Step 2: migrate task-ledger to new code_id  ━━━\n')

  if (DRY_RUN) {
    console.log(`  (dry run) would send: client.migrate(${sender}, ${tl.address}, ${newCodeId}, {})`)
  } else {
    const mig = await client.migrate(sender, tl.address, newCodeId, {}, 'auto', 'JunoClaw Tier 1.5')
    console.log(`  ✅  migrate tx: ${mig.transactionHash}`)
    tl.tier15_code_id = newCodeId
    tl.tier15_migrate_tx = mig.transactionHash
    tl.tier15_migrated_at = new Date().toISOString()
    // Keep the pre-migration code_id visible for rollback / audit.
    tl.pre_tier15_code_id = tl.code_id
    tl.code_id = newCodeId
    saveDeployed(deployed)
  }

  // ── Step 3: sanity check ───────────────────────────────────────────────
  console.log('\n━━━  Step 3: sanity check  ━━━\n')

  if (DRY_RUN) {
    console.log('  (dry run: skipping sanity check)')
  } else {
    const cfg = await client.queryContractSmart(tl.address, { get_config: {} })
    console.log(`  ✅  GetConfig responds — admin=${cfg.admin}, operators=[${(cfg.operators||[]).length}]`)
    // The `agent_company` field was added in v6. If it's still present after
    // migrate, serde round-trip held.
    if ('agent_company' in cfg) {
      console.log(`  ✅  v6 agent_company preserved: ${cfg.agent_company ?? '(null)'}`)
    }
  }

  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
  console.log('  Tier 1.5 migration complete.')
  console.log('  Next: node deploy/smoke-tier15.mjs\n')
  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  migrate-tier15 failed:', err.message)
  if (err.stack) console.error(err.stack)
  process.exit(1)
})
