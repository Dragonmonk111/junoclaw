import { readFileSync, writeFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice, coins } from '@cosmjs/stargate'
import 'dotenv/config'

const __dir = dirname(fileURLToPath(import.meta.url))

// ── Config ──────────────────────────────────────────────────────────────────

const CHAIN_ID  = process.env.CHAIN_ID  || 'uni-7'
const RPC_URL   = process.env.RPC_URL   || 'https://juno-testnet-rpc.polkachu.com'
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujunox'
const AUTO      = process.env.AUTO_CONFIRM === 'true'

// Mnemonic sourcing: either MNEMONIC env var, or PARLIAMENT_ROLE (e.g. "The Builder")
// which reads wavs/bridge/parliament-state.json for the wallet with that `name`.
// parliament-state.json is gitignored and kept out of logs.
const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

function loadMnemonic() {
  if (process.env.MNEMONIC) return process.env.MNEMONIC
  if (process.env.PARLIAMENT_ROLE) {
    if (!existsSync(PARLIAMENT_STATE)) {
      console.error(`❌  PARLIAMENT_ROLE set but ${PARLIAMENT_STATE} not found`)
      process.exit(1)
    }
    const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
    const role = process.env.PARLIAMENT_ROLE
    const mp = (state.mps || []).find((m) => m.name === role)
    if (!mp) {
      console.error(`❌  No MP with name "${role}" in parliament-state.json`)
      process.exit(1)
    }
    console.log(`  Wallet:   ${role} (${mp.address})`)
    return mp.mnemonic
  }
  console.error('❌  Set MNEMONIC or PARLIAMENT_ROLE (e.g. "The Builder"). See .env.example.')
  process.exit(1)
}
const MNEMONIC = loadMnemonic()

// Wasm file locations — copy built artifacts here, or set ARTIFACTS_DIR env var
const ARTIFACTS_DIR = process.env.ARTIFACTS_DIR
  || 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

// Each contract has an optimized and a raw fallback. Raw release wasms are
// accepted on testnet (just larger); `_opt.wasm` is preferred when the
// cosmwasm-optimizer Docker image is available.
const CONTRACTS = [
  { name: 'agent-registry', wasm: ['agent_registry_opt.wasm', 'agent_registry.wasm'] },
  { name: 'task-ledger',    wasm: ['task_ledger_opt.wasm',    'task_ledger.wasm'] },
  { name: 'escrow',         wasm: ['escrow_opt.wasm',         'escrow.wasm'] },
  { name: 'agent-company',  wasm: ['agent_company_opt.wasm',  'agent_company.wasm'] },
  { name: 'builder-grant',  wasm: ['builder_grant_opt.wasm',  'builder_grant.wasm'], optional: true },
  { name: 'junoswap-pair',  wasm: ['junoswap_pair_opt.wasm',  'junoswap_pair.wasm'], optional: true },
]

function resolveWasm(contract) {
  const candidates = Array.isArray(contract.wasm) ? contract.wasm : [contract.wasm]
  for (const name of candidates) {
    const p = join(ARTIFACTS_DIR, name)
    if (existsSync(p)) return { path: p, name }
  }
  return null
}

const DEPLOYED_FILE = join(__dir, 'deployed.json')

// ── Helpers ──────────────────────────────────────────────────────────────────

function loadDeployed() {
  if (existsSync(DEPLOYED_FILE)) {
    return JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  }
  return {}
}

function saveDeployed(data) {
  writeFileSync(DEPLOYED_FILE, JSON.stringify(data, null, 2))
  console.log(`\n💾  Saved to ${DEPLOYED_FILE}`)
}

async function confirm(prompt) {
  if (AUTO) return true
  process.stdout.write(`\n${prompt} [y/N] `)
  return new Promise((resolve) => {
    process.stdin.resume()
    process.stdin.setEncoding('utf8')
    process.stdin.once('data', (data) => {
      process.stdin.pause()
      resolve(data.trim().toLowerCase() === 'y')
    })
  })
}

// ── Main ─────────────────────────────────────────────────────────────────────

async function main() {
  console.log('\n╔══════════════════════════════════════════╗')
  console.log('║       JunoClaw Contract Deployer         ║')
  console.log('╚══════════════════════════════════════════╝')
  console.log(`\n  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log(`  Gas:      ${GAS_PRICE}`)
  console.log(`  Artifacts: ${ARTIFACTS_DIR}\n`)

  // Connect
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [{ address }] = await wallet.getAccounts()
  console.log(`  Deployer: ${address}`)

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  })

  const balance = await client.getBalance(address, 'ujunox')
  console.log(`  Balance:  ${balance.amount} ${balance.denom}\n`)

  if (BigInt(balance.amount) < 5_000_000n) {
    console.warn('⚠  Low balance — get testnet tokens from https://faucet.reece.sh/?chain=uni-7')
  }

  const deployed = loadDeployed()

  // ── STEP 1: Store all contracts ──────────────────────────────────────────

  console.log('━━━  Step 1: Store WASM artifacts  ━━━\n')

  for (const contract of CONTRACTS) {
    const resolved = resolveWasm(contract)
    if (!resolved) {
      if (contract.optional) {
        console.log(`  ⏭  ${contract.name} (optional) — no wasm found, skipping`)
        continue
      }
      console.error(`❌  Not found in ${ARTIFACTS_DIR}:`)
      console.error(`    Expected one of: ${contract.wasm.join(', ')}`)
      console.error(`    Build WASMs: cd contracts && cargo build --release --target wasm32-unknown-unknown --lib`)
      process.exit(1)
    }

    if (deployed[contract.name]?.code_id) {
      console.log(`  ⏭  ${contract.name} already stored (code_id ${deployed[contract.name].code_id})`)
      continue
    }

    const ok = await confirm(`  Store ${contract.name} (${resolved.name})?`)
    if (!ok) { console.log('  Skipped.'); continue }

    const wasm = readFileSync(resolved.path)
    console.log(`  📦 Storing ${contract.name} (${(wasm.length / 1024).toFixed(1)} KB)…`)

    const result = await client.upload(address, wasm, 'auto', `JunoClaw ${contract.name}`)
    console.log(`  ✅  code_id: ${result.codeId}  tx: ${result.transactionHash}`)

    deployed[contract.name] = {
      code_id: result.codeId,
      store_tx: result.transactionHash,
      wasm_file: resolved.name,
    }
    saveDeployed(deployed)
  }

  // ── STEP 2: Instantiate agent-registry ──────────────────────────────────

  console.log('\n━━━  Step 2: Instantiate agent-registry  ━━━\n')

  if (!deployed['agent-registry']?.address) {
    const codeId = deployed['agent-registry']?.code_id
    if (!codeId) { console.log('  ⏭  Skipping — not stored yet'); }
    else {
      const ok = await confirm('  Instantiate agent-registry?')
      if (ok) {
        const msg = {
          admin: address,
          max_agents: 1000,
          registration_fee_ujuno: '0',
          denom: 'ujunox',
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Agent Registry', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['agent-registry'].address = res.contractAddress
        deployed['agent-registry'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['agent-registry'].address}`)
  }

  // ── STEP 3: Instantiate escrow ───────────────────────────────────────────

  console.log('\n━━━  Step 3: Instantiate escrow  ━━━\n')

  if (!deployed['escrow']?.address) {
    const codeId = deployed['escrow']?.code_id
    const taskLedgerAddr = deployed['task-ledger']?.address || address  // placeholder if not yet deployed
    if (!codeId) { console.log('  ⏭  Skipping — not stored yet') }
    else {
      const ok = await confirm('  Instantiate escrow?')
      if (ok) {
        const msg = {
          admin: address,
          task_ledger: taskLedgerAddr,
          timeout_blocks: 1000,
          denom: 'ujunox',
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Escrow', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['escrow'].address = res.contractAddress
        deployed['escrow'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['escrow'].address}`)
  }

  // ── STEP 4: Instantiate task-ledger ─────────────────────────────────────

  console.log('\n━━━  Step 4: Instantiate task-ledger  ━━━\n')

  if (!deployed['task-ledger']?.address) {
    const codeId = deployed['task-ledger']?.code_id
    const registryAddr = deployed['agent-registry']?.address
    if (!codeId || !registryAddr) {
      console.log('  ⏭  Skipping — needs agent-registry address first')
    } else {
      const ok = await confirm('  Instantiate task-ledger?')
      if (ok) {
        const msg = {
          admin: address,
          agent_registry: registryAddr,
          operators: [],
          agent_company: null,
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Task Ledger', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['task-ledger'].address = res.contractAddress
        deployed['task-ledger'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['task-ledger'].address}`)
  }

  // ── STEP 5: Instantiate agent-company ───────────────────────────────────

  console.log('\n━━━  Step 5: Instantiate agent-company (example)  ━━━\n')

  if (!deployed['agent-company']?.address) {
    const codeId = deployed['agent-company']?.code_id
    const escrowAddr   = deployed['escrow']?.address
    const registryAddr = deployed['agent-registry']?.address
    if (!codeId || !escrowAddr || !registryAddr) {
      console.log('  ⏭  Skipping — needs escrow + registry addresses first')
    } else {
      const taskLedgerAddr = deployed['task-ledger']?.address || null
      const ok = await confirm('  Instantiate agent-company (demo "JunoClaw Core Team")?')
      if (ok) {
        const msg = {
          name: 'JunoClaw Core Team',
          admin: address,
          governance: null,
          wavs_operator: address,
          escrow_contract: escrowAddr,
          agent_registry: registryAddr,
          task_ledger: taskLedgerAddr,
          nois_proxy: null,
          members: [
            { addr: address, weight: 10000, role: 'human' },
          ],
          denom: 'ujunox',
          voting_period_blocks: 100,
          quorum_percent: 51,
          adaptive_threshold_blocks: 10,
          adaptive_min_blocks: 13,
          verification: {
            model: 'witness_and_wavs',
            required_attestations: 2,
            total_witnesses: 3,
            attestation_timeout_blocks: 200,
            auto_release_on_verify: true,
          },
        }
        const res = await client.instantiate(address, codeId, msg, 'JunoClaw Agent Company', 'auto')
        console.log(`  ✅  address: ${res.contractAddress}  tx: ${res.transactionHash}`)
        deployed['agent-company'].address = res.contractAddress
        deployed['agent-company'].instantiate_tx = res.transactionHash
        saveDeployed(deployed)

        if (taskLedgerAddr) {
          const updateRes = await client.execute(
            address,
            taskLedgerAddr,
            {
              update_config: {
                admin: null,
                agent_registry: null,
                agent_company: res.contractAddress,
              },
            },
            'auto'
          )
          console.log(`  ✅  wired task-ledger.agent_company  tx: ${updateRes.transactionHash}`)
        }
      }
    }
  } else {
    console.log(`  ⏭  Already instantiated: ${deployed['agent-company'].address}`)
  }

  // ── STEP 6: Cross-contract registry wiring ──────────────────────────────
  //
  // `CompleteTask` / `FailTask` on task-ledger fire an `IncrementTasks`
  // callback into agent-registry. Agent-registry gates that call on its own
  // `config.registry.task_ledger` pointer — which is None at instantiate
  // time because the registry's task_ledger address wasn't yet known.
  // Without this wiring step, the first real CompleteTask reverts with
  // Unauthorized (a sub-message failure bubbles into the parent tx), and the
  // whole stack looks broken despite every contract being healthy on its own.
  // Keeping the wiring in deploy (admin-only UpdateRegistry) makes the full
  // stack usable from block 1 of instantiation.
  console.log('\n━━━  Step 6: Wire agent-registry.registry.task_ledger  ━━━\n')

  if (deployed['agent-registry']?.address && deployed['task-ledger']?.address) {
    const registryAddr   = deployed['agent-registry'].address
    const taskLedgerAddr = deployed['task-ledger'].address
    let already = false
    try {
      const cfg = await client.queryContractSmart(registryAddr, { get_config: {} })
      already = cfg?.registry?.task_ledger === taskLedgerAddr
    } catch (e) { /* ignore */ }
    if (already) {
      console.log(`  ⏭  agent-registry already knows task-ledger`)
    } else {
      const ok = await confirm('  Wire agent-registry.registry.task_ledger?')
      if (ok) {
        const res = await client.execute(
          address,
          registryAddr,
          { update_registry: {
            agent_registry: null,
            task_ledger: taskLedgerAddr,
            escrow: deployed['escrow']?.address || null,
          } },
          'auto',
        )
        console.log(`  ✅  wired agent-registry.registry  tx: ${res.transactionHash}`)
        deployed['agent-registry'].registry_wired_tx = res.transactionHash
        saveDeployed(deployed)
      }
    }
  }

  // ── Summary ──────────────────────────────────────────────────────────────

  console.log('\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n')
  console.log('  Deployment summary:\n')
  for (const [name, info] of Object.entries(deployed)) {
    console.log(`  ${name}`)
    if (info.code_id)  console.log(`    code_id:  ${info.code_id}`)
    if (info.address)  console.log(`    address:  ${info.address}`)
  }
  console.log(`\n  Full details: ${DEPLOYED_FILE}`)
  console.log('\n  View on Mintscan: https://testnet.mintscan.io/juno-testnet\n')

  process.exit(0)
}

main().catch((err) => {
  console.error('\n❌  Deploy failed:', err.message)
  process.exit(1)
})
