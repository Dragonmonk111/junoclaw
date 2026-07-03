// Deploy (store + instantiate) the knowledge-moults contract to Juno mainnet
// per A18c-5 (proposal id 27 on the Juno Agents DAO proposal module).
//
// SAFETY: this is a real mainnet deployment authorized by DAO vote. It
// defaults to dry-run no matter what, and it REFUSES to broadcast unless the
// A18c-5 proposal has actually passed/executed on-chain (checked live via
// the proposal module query, not trusted from a local file). To actually
// broadcast, you must set ALL of:
//   JUNO_DEPLOY_MNEMONIC (or JUNO_MOTHER_MOULT_MNEMONIC / JUNO_REPLY_BOT_MNEMONIC)
//   DEPLOY_KNOWLEDGE_MOULTS_CONFIRM=yes
//   a built optimized wasm at KNOWLEDGE_MOULTS_WASM (default: devnet/knowledge_moults.wasm)
//
// Build the wasm first with: devnet/scripts/build-knowledge-moults.sh
//
// Usage (dry-run, safe, default):
//   node scripts/deploy-knowledge-moults.js
//
// Usage (real deploy, after A18c-5 passes):
//   DEPLOY_KNOWLEDGE_MOULTS_CONFIRM=yes JUNO_DEPLOY_MNEMONIC="..." node scripts/deploy-knowledge-moults.js

import { readFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { SigningCosmWasmClient, CosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { GasPrice } from '@cosmjs/stargate'

const __dirname = dirname(fileURLToPath(import.meta.url))
const REPO_ROOT = join(__dirname, '..', '..', '..')

const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || 'https://juno-rpc.publicnode.com'
const GAS_PRICE = process.env.JUNO_GAS_PRICE || '0.075ujuno'
const MNEMONIC =
  process.env.JUNO_DEPLOY_MNEMONIC ||
  process.env.JUNO_MOTHER_MOULT_MNEMONIC ||
  process.env.JUNO_REPLY_BOT_MNEMONIC
const CONFIRMED = process.env.DEPLOY_KNOWLEDGE_MOULTS_CONFIRM === 'yes'

const DAO_CORE = process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
const PROPOSAL_MODULE = process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'
const PROPOSAL_ID = Number(process.env.A18C5_PROPOSAL_ID || 27)

const MOTHER_MOULT_ID =
  process.env.MOTHER_MOULT_ID ||
  'moult:49b0b8f5ee0832959920f6432bd6c15cc7551c07c48980a6efb0b28e657c7e2a'
const ADMIN = process.env.KNOWLEDGE_MOULTS_ADMIN || DAO_CORE
const WASM_PATH =
  process.env.KNOWLEDGE_MOULTS_WASM || join(REPO_ROOT, 'devnet', 'knowledge_moults.wasm')

async function assertProposalPassed(client) {
  const res = await client.queryContractSmart(PROPOSAL_MODULE, {
    proposal: { proposal_id: PROPOSAL_ID },
  })
  const status = res?.proposal?.status
  console.log(`[deploy-knowledge-moults] A18c-5 (proposal ${PROPOSAL_ID}) status: ${status}`)
  if (status !== 'passed' && status !== 'executed') {
    throw new Error(
      `A18c-5 has not passed yet (status: ${status}). Refusing to deploy. ` +
        `Set A18C5_PROPOSAL_ID if this is the wrong id, or wait for the vote.`
    )
  }
}

async function main() {
  const msg = {
    admin: ADMIN,
    mother_moult_id: MOTHER_MOULT_ID,
  }

  console.log('[deploy-knowledge-moults] wasm path:', WASM_PATH)
  console.log('[deploy-knowledge-moults] instantiate msg:', JSON.stringify(msg, null, 2))

  if (!MNEMONIC || !CONFIRMED) {
    console.log('\n[deploy-knowledge-moults] DRY RUN — nothing broadcast.')
    if (!MNEMONIC) console.log('  reason: no JUNO_DEPLOY_MNEMONIC set')
    if (!CONFIRMED) console.log('  reason: DEPLOY_KNOWLEDGE_MOULTS_CONFIRM=yes not set')
    const readOnly = await CosmWasmClient.connect(RPC_ENDPOINT)
    await assertProposalPassed(readOnly).catch((e) =>
      console.log(`  note: ${e.message}`)
    )
    if (!existsSync(WASM_PATH)) {
      console.log(`  note: wasm not built yet — run devnet/scripts/build-knowledge-moults.sh first`)
    }
    return { dryRun: true, msg }
  }

  if (!existsSync(WASM_PATH)) {
    throw new Error(`wasm not found at ${WASM_PATH} — run devnet/scripts/build-knowledge-moults.sh first`)
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
  const [account] = await wallet.getAccounts()
  const gasPrice = GasPrice.fromString(GAS_PRICE)
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_ENDPOINT, wallet, { gasPrice })

  await assertProposalPassed(client)

  const wasm = readFileSync(WASM_PATH)
  console.log(`\n[deploy-knowledge-moults] Storing (${(wasm.length / 1024).toFixed(1)} KB)...`)
  const uploadResult = await client.upload(account.address, wasm, 'auto', 'JunoClaw Knowledge Moults (A18c-5)')
  console.log('  code_id:', uploadResult.codeId, ' tx:', uploadResult.transactionHash)

  console.log('\n[deploy-knowledge-moults] Instantiating...')
  const initResult = await client.instantiate(
    account.address,
    uploadResult.codeId,
    msg,
    'JunoClaw Knowledge Moults (A18c-5)',
    'auto',
    { admin: ADMIN }
  )
  console.log('  address:', initResult.contractAddress, ' tx:', initResult.transactionHash)

  console.log('\n[deploy-knowledge-moults] DEPLOY SUCCEEDED')
  console.log('  code_id:', uploadResult.codeId)
  console.log('  contract:', initResult.contractAddress)
  console.log('\n  Next: post code_id + contract address as a Moultbook entry referencing A18c-5,')
  console.log('  then update contracts/knowledge-moults/README.md with the live address.')

  return {
    dryRun: false,
    codeId: uploadResult.codeId,
    storeTx: uploadResult.transactionHash,
    contractAddress: initResult.contractAddress,
    instantiateTx: initResult.transactionHash,
  }
}

main().catch((e) => {
  console.error('[deploy-knowledge-moults] failed:', e.message)
  process.exit(1)
})
