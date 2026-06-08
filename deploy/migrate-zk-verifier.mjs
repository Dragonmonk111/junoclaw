// ────────────────────────────────────────────────────────────────────────────
//  JunoClaw — live zk-verifier pure → precompile swap
//
//  Swaps the running code behind a single deployed zk-verifier instance from
//  the pure-Wasm (arkworks) build to the BN254-precompile build, in place,
//  WITHOUT redeploying. The MigrateMsg is empty ({}): the contract's `migrate`
//  entry point only re-stamps cw2 contract-version, and CONFIG / VK_BYTES /
//  LAST_VERIFICATION carry over untouched (see contracts/zk-verifier/src/
//  contract.rs:159 and state.rs). The stored verification key and admin are
//  therefore preserved across the swap — only the verification backend (and
//  hence the per-VerifyProof gas, 370498 → 203164 = 1.823×) changes.
//
//  Two modes (MODE env, default 'admin'):
//
//    admin  — testnet / any chain where the caller's wallet is the wasmd-level
//             admin of the contract. Uploads the precompile wasm, then sends
//             MsgMigrateContract directly via cosmjs. State-preserving sanity
//             check afterwards (VkStatus.has_vk must stay true).
//
//    gov    — mainnet, where the contract admin is the gov module account and
//             only on-chain governance can migrate. Does NOT broadcast a
//             migrate; instead writes a ready-to-submit MsgMigrateContract
//             governance proposal JSON to deploy/proposal-migrate-zk-verifier.json
//             (submit with `junod tx gov submit-proposal <file> --from <key>`).
//             Still uploads the precompile wasm first (store-code is permission-
//             less on Juno) unless a code_id is supplied.
//
//  Usage (PowerShell):
//    # testnet admin migrate
//    $env:PARLIAMENT_ROLE = 'The Builder'        # must be the contract's wasmd admin
//    $env:ZK_VERIFIER_ADDR = 'juno1...'          # the live (pure) verifier
//    $env:ZK_PRECOMPILE_WASM = 'C:\\path\\zk_verifier_precompile.wasm'
//    node deploy/migrate-zk-verifier.mjs
//
//    # mainnet governance proposal (no broadcast of the migrate)
//    $env:MODE = 'gov'
//    $env:CHAIN_ID = 'juno-1'; $env:RPC_URL = 'https://juno-rpc.polkachu.com'
//    $env:ZK_VERIFIER_ADDR = 'juno1...'
//    $env:GOV_AUTHORITY = 'juno10d07y265gmmuvt4z0w9aw880jnsr700j7g7ejq'  # gov module
//    node deploy/migrate-zk-verifier.mjs
//
//  Resolution order for contract address & precompile code_id (first wins):
//    1. explicit env (ZK_VERIFIER_ADDR / ZK_PRECOMPILE_CODE_ID)
//    2. devnet/deploy.env (PURE_ADDR / PRECOMPILE_CODE_ID) if present
//    3. deploy/deployed.json under 'zk-verifier' (address / precompile_code_id)
//
//  Opt-outs / knobs:
//    SKIP_UPLOAD=true          reuse a known precompile code_id, skip store-code
//    ZK_PRECOMPILE_CODE_ID=N   the precompile code_id to migrate to
//    DRY_RUN=true              simulate — print actions, broadcast nothing
//    DEPOSIT=10000000ujuno     gov mode: initial deposit (default 10000000ujuno)
// ────────────────────────────────────────────────────────────────────────────

import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

const MODE      = (process.env.MODE || 'admin').toLowerCase()
const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const SKIP_UPLOAD = process.env.SKIP_UPLOAD === 'true'
const DRY_RUN = process.env.DRY_RUN === 'true'
const DEPOSIT = process.env.DEPOSIT || '10000000ujuno'
const GOV_AUTHORITY = process.env.GOV_AUTHORITY
  || 'juno10d07y265gmmuvt4z0w9aw880jnsr700j7g7ejq' // gov module account on Juno

const WASM_PATH = process.env.ZK_PRECOMPILE_WASM
  || join(__dir, '..', 'devnet', 'zk_verifier_precompile.wasm')

const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')
const DEPLOYED_FILE    = join(__dir, 'deployed.json')
const DEPLOY_ENV_FILE  = join(__dir, '..', 'devnet', 'deploy.env')
const PROPOSAL_FILE    = join(__dir, 'proposal-migrate-zk-verifier.json')

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
  if (!existsSync(DEPLOYED_FILE)) return {}
  return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
  console.log(`  💾  ${DEPLOYED_FILE}`)
}

// Parse a KEY=VALUE shell env file (devnet/deploy.env) into a plain object.
function loadDeployEnv() {
  if (!existsSync(DEPLOY_ENV_FILE)) return {}
  const out = {}
  for (const line of readFileSync(DEPLOY_ENV_FILE, 'utf8').split(/\r?\n/)) {
    const m = line.match(/^\s*([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
    if (m) out[m[1]] = m[2].trim()
  }
  return out
}

// Resolve the live verifier address and (optionally) a precompile code_id from
// the first source that has them.
function resolveTargets(deployed, denv) {
  const addr = process.env.ZK_VERIFIER_ADDR
    || denv.PURE_ADDR
    || deployed['zk-verifier']?.address
  const codeId = process.env.ZK_PRECOMPILE_CODE_ID
    || denv.PRECOMPILE_CODE_ID
    || deployed['zk-verifier']?.precompile_code_id
  return { addr, codeId: codeId ? Number(codeId) : undefined }
}

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║   JunoClaw — zk-verifier pure → precompile ║')
  console.log('║   in-place code swap (MsgMigrateContract)  ║')
  console.log('╚══════════════════════════════════════════╝')
  console.log(`\n  Mode:     ${MODE}`)
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Wasm:     ${WASM_PATH}`)
  if (DRY_RUN) console.log('  ** DRY RUN — no transactions will be broadcast **')

  if (MODE !== 'admin' && MODE !== 'gov') {
    console.error(`❌  Unknown MODE "${MODE}". Use 'admin' or 'gov'.`)
    process.exit(1)
  }

  const deployed = loadDeployed()
  const denv = loadDeployEnv()
  const { addr, codeId: presetCodeId } = resolveTargets(deployed, denv)

  if (!addr) {
    console.error(
      '❌  Could not resolve the verifier contract address.\n' +
      '    Set ZK_VERIFIER_ADDR, or provide devnet/deploy.env (PURE_ADDR),\n' +
      "    or deploy/deployed.json with a 'zk-verifier'.address entry.",
    )
    process.exit(1)
  }
  console.log(`  Target:   zk-verifier @ ${addr}`)

  const mnemonic = loadMnemonic()
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, { prefix: 'juno' })
  const [{ address: sender }] = await wallet.getAccounts()
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })
  console.log(`  Sender:   ${sender}\n`)

  // ── Inspect current on-chain state of the contract ────────────────────────
  const info = await client.getContract(addr)
  console.log(`  Current code_id:  ${info.codeId}`)
  console.log(`  Contract admin:   ${info.admin || '(none — immutable)'}`)

  // Capture pre-swap verification state so we can prove it survives the migrate.
  let preVk
  try {
    preVk = await client.queryContractSmart(addr, { vk_status: {} })
    console.log(`  Pre-swap VkStatus: has_vk=${preVk.has_vk} (${preVk.vk_size_bytes} bytes)`)
  } catch (e) {
    console.warn(`  ⚠  Could not query VkStatus pre-swap: ${e.message}`)
  }

  // Authority that must be the contract admin for the migrate to be accepted.
  const requiredAuthority = MODE === 'gov' ? GOV_AUTHORITY : sender
  if (info.admin && info.admin !== requiredAuthority) {
    console.error(
      `\n❌  Migrate authority mismatch.\n` +
      `    Contract admin on chain: ${info.admin}\n` +
      `    ${MODE === 'gov' ? 'Gov authority' : 'Sender wallet'}:        ${requiredAuthority}\n` +
      (MODE === 'gov'
        ? `    The gov proposal will be rejected unless the contract admin is the\n` +
          `    gov module account. Set GOV_AUTHORITY to the chain's gov module addr.`
        : `    Only the wasmd-level admin can migrate. Use MODE=gov if the admin\n` +
          `    is the gov module, or set PARLIAMENT_ROLE to the admin wallet.`),
    )
    process.exit(1)
  }
  if (!info.admin) {
    console.error(
      `\n❌  Contract @ ${addr} has no admin — it was instantiated --no-admin and\n` +
      `    is permanently immutable. It cannot be migrated; a fresh deploy with\n` +
      `    an admin is required to support the precompile swap.`,
    )
    process.exit(1)
  }

  // ── Step 1: upload precompile wasm → code_id ───────────────────────────────
  console.log('\n━━━  Step 1: ensure precompile code_id  ━━━\n')

  let newCodeId = presetCodeId
  if (SKIP_UPLOAD && newCodeId) {
    console.log(`  ⏭  SKIP_UPLOAD=true — reusing code_id ${newCodeId}`)
  } else if (newCodeId && SKIP_UPLOAD) {
    console.log(`  ⏭  reusing code_id ${newCodeId}`)
  } else {
    if (!existsSync(WASM_PATH)) {
      console.error(
        `❌  Precompile wasm not found: ${WASM_PATH}\n` +
        `    Build it: bash devnet/scripts/build-zk-verifier.sh (emits\n` +
        `    devnet/zk_verifier_precompile.wasm), or set ZK_PRECOMPILE_WASM.`,
      )
      process.exit(1)
    }
    const wasm = readFileSync(WASM_PATH)
    console.log(`  📦  Uploading ${(wasm.length / 1024).toFixed(1)} KB precompile build…`)
    if (DRY_RUN) {
      console.log('  (dry run: skipping upload)')
    } else {
      const up = await client.upload(sender, wasm, 'auto', 'JunoClaw zk-verifier (BN254 precompile)')
      newCodeId = up.codeId
      console.log(`  ✅  code_id: ${newCodeId}  tx: ${up.transactionHash}`)
    }
  }

  if (!newCodeId && !DRY_RUN) {
    console.error('❌  No precompile code_id available after upload step.')
    process.exit(1)
  }

  if (newCodeId === info.codeId) {
    console.warn(`  ⚠  Target code_id ${newCodeId} equals current code_id — nothing to swap.`)
  }

  // ── Step 2: migrate (admin) or emit gov proposal ───────────────────────────
  if (MODE === 'gov') {
    console.log('\n━━━  Step 2: write MsgMigrateContract gov proposal  ━━━\n')
    const proposal = {
      messages: [
        {
          '@type': '/cosmwasm.wasm.v1.MsgMigrateContract',
          sender: GOV_AUTHORITY, // gov module account = contract admin
          contract: addr,
          code_id: String(newCodeId ?? '<PRECOMPILE_CODE_ID>'),
          msg: Buffer.from(JSON.stringify({})).toString('base64'), // MigrateMsg {}
        },
      ],
      metadata: 'ipfs://<proposal-metadata-cid>',
      deposit: DEPOSIT,
      title: 'Migrate zk-verifier to the BN254 precompile build',
      summary:
        'Swap the running code behind the zk-verifier contract from the ' +
        'pure-Wasm (arkworks) Groth16 backend to the BN254-precompile backend. ' +
        'The migrate is state-preserving (empty MigrateMsg): the stored ' +
        'verification key, admin, and last-verification state are retained. ' +
        'Measured effect: per-VerifyProof gas drops from 370498 to 203164 ' +
        '(1.823x reduction). Requires the BN254 wasmvm host functions enabled ' +
        'by the v30 chain upgrade to be live before this proposal executes.',
    }
    if (DRY_RUN) {
      console.log('  (dry run) proposal that would be written:\n')
      console.log(JSON.stringify(proposal, null, 2))
    } else {
      writeFileSync(PROPOSAL_FILE, JSON.stringify(proposal, null, 2))
      console.log(`  ✅  wrote ${PROPOSAL_FILE}`)
      console.log('\n  Submit with:')
      console.log(`    junod tx gov submit-proposal ${PROPOSAL_FILE} \\`)
      console.log(`      --from <key> --chain-id ${CHAIN_ID} --gas auto --gas-adjustment 1.4 \\`)
      console.log(`      --gas-prices 0.075ujuno --yes`)
    }
    console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
    console.log('  Gov proposal prepared. No migrate was broadcast (governance executes it).')
    process.exit(0)
  }

  console.log('\n━━━  Step 2: migrate verifier to precompile code_id  ━━━\n')
  if (DRY_RUN) {
    console.log(`  (dry run) would send: client.migrate(${sender}, ${addr}, ${newCodeId}, {})`)
  } else {
    const mig = await client.migrate(sender, addr, newCodeId, {}, 'auto', 'JunoClaw zk-verifier precompile swap')
    console.log(`  ✅  migrate tx: ${mig.transactionHash}`)
    const entry = deployed['zk-verifier'] || {}
    entry.address = addr
    entry.pre_precompile_code_id = info.codeId
    entry.code_id = newCodeId
    entry.precompile_code_id = newCodeId
    entry.precompile_migrate_tx = mig.transactionHash
    entry.precompile_migrated_at = new Date().toISOString()
    deployed['zk-verifier'] = entry
    saveDeployed(deployed)
  }

  // ── Step 3: state-preserving sanity check ──────────────────────────────────
  console.log('\n━━━  Step 3: sanity check (state must survive swap)  ━━━\n')
  if (DRY_RUN) {
    console.log('  (dry run: skipping sanity check)')
  } else {
    const postVk = await client.queryContractSmart(addr, { vk_status: {} })
    console.log(`  Post-swap VkStatus: has_vk=${postVk.has_vk} (${postVk.vk_size_bytes} bytes)`)
    if (preVk && preVk.has_vk && !postVk.has_vk) {
      console.error('❌  Verification key was LOST across the migrate — investigate before reuse.')
      process.exit(1)
    }
    if (preVk && preVk.vk_size_bytes !== postVk.vk_size_bytes) {
      console.warn(`  ⚠  VK size changed (${preVk.vk_size_bytes} → ${postVk.vk_size_bytes}).`)
    } else if (postVk.has_vk) {
      console.log('  ✅  Stored VK preserved across the code swap.')
    }
  }

  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━')
  console.log('  zk-verifier precompile swap complete.')
  console.log('  Verify the gas drop: re-run a VerifyProof and compare against')
  console.log('  docs/BN254_BENCHMARK_RESULTS.md (expect ~203164 gas).')
  process.exit(0)
}

main().catch((e) => {
  console.error('\n❌  migrate-zk-verifier failed:', e?.message || e)
  process.exit(1)
})
