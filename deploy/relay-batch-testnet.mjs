// Relayer — submits a finalized batch to the coordination-settler contract on uni-7
//
// This script simulates a finalized batch from the coordination engine,
// computes the expected certificate (SHA256 of messages_hash || validators),
// and submits it to the on-chain contract for settlement verification.
//
// Usage:
//   $env:WALLET_ID="builder"; $env:CHAIN_ID="uni-7"; node relay-batch-testnet.mjs
//
// Or with mnemonic:
//   $env:JUNO_MNEMONIC="word1 word2 ..."; node relay-batch-testnet.mjs

import { readFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { createHash } from 'crypto'

const __dirname = dirname(fileURLToPath(import.meta.url))

const CHAIN_ID = process.env.CHAIN_ID || 'uni-7'
const RPC_URL = process.env.RPC_URL || 'https://juno.rpc.t.stavr.tech'
const DENOM = process.env.DENOM || 'ujunox'
const GAS_PRICE = process.env.GAS_PRICE || `0.075${DENOM}`
const WALLET_ID = process.env.WALLET_ID
const MNEMONIC = process.env.JUNO_MNEMONIC || null

const DEPLOYED_FILE = join(__dirname, 'deployed-testnet.json')

// Validator set must match what was used at instantiation:
// [[0x11; 48], [0x22; 48], [0x33; 48], [0x44; 48]]
const VALIDATORS = [
  Buffer.alloc(48, 0x11),
  Buffer.alloc(48, 0x22),
  Buffer.alloc(48, 0x33),
  Buffer.alloc(48, 0x44),
]

function computeCertificate(messagesHash, validators) {
  const hasher = createHash('sha256')
  hasher.update(Buffer.from(messagesHash))
  for (const vk of validators) {
    hasher.update(vk)
  }
  return hasher.digest()
}

async function main() {
  console.log('=== Relayer: Submit batch to coordination-settler on uni-7 ===')
  console.log(`  Chain:    ${CHAIN_ID}`)
  console.log(`  RPC:      ${RPC_URL}`)
  console.log()

  // Load deployed contract address
  if (!existsSync(DEPLOYED_FILE)) {
    console.error('ERROR: deployed-testnet.json not found. Deploy the contract first.')
    process.exit(1)
  }
  const deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'))
  const contractAddr = deployed['coordination-settler']?.address
  if (!contractAddr) {
    console.error('ERROR: coordination-settler address not found in deployed-testnet.json')
    process.exit(1)
  }
  console.log(`  Contract: ${contractAddr}`)

  // Connect wallet
  let client, address
  if (WALLET_ID) {
    console.log(`  Wallet:   encrypted store (id: "${WALLET_ID}")`)
    const { WalletStore } = await import('../mcp/dist/wallet/store.js')
    const store = WalletStore.defaultStore()
    const chainConfig = {
      chainId: CHAIN_ID,
      chainName: 'Juno Testnet',
      rpcEndpoint: RPC_URL,
      restEndpoint: 'https://juno.api.t.stavr.tech',
      denom: DENOM,
      bech32Prefix: 'juno',
      gasPrice: GAS_PRICE,
      slip44: 118,
      explorerTx: 'https://testnet.mintscan.io/juno-testnet/tx',
      isTestnet: true,
    }
    const ctx = await store.signFor(WALLET_ID, chainConfig)
    client = ctx.client
    address = ctx.address
  } else if (MNEMONIC) {
    console.log('  Wallet:   mnemonic from env')
    const { DirectSecp256k1HdWallet } = await import('@cosmjs/proto-signing')
    const { SigningCosmWasmClient } = await import('@cosmjs/cosmwasm-stargate')
    const { GasPrice } = await import('@cosmjs/stargate')
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' })
    const [{ addr }] = await wallet.getAccounts()
    address = addr
    client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
      gasPrice: GasPrice.fromString(GAS_PRICE),
    })
  } else {
    console.error('Set WALLET_ID or JUNO_MNEMONIC env var')
    process.exit(1)
  }

  console.log(`  Relayer:  ${address}`)
  const balance = await client.getBalance(address, DENOM)
  console.log(`  Balance:  ${balance.amount} ${balance.denom}`)
  console.log()

  // Query contract config to verify we're talking to the right contract
  const config = await client.queryContractSmart(contractAddr, { config: {} })
  console.log('Contract config:')
  console.log(`  admin:           ${config.admin}`)
  console.log(`  threshold:       ${config.threshold}`)
  console.log(`  validator_count: ${config.validator_count}`)
  console.log(`  relayer_count:   ${config.relayer_count}`)
  console.log(`  latest_height:   ${config.latest_height ?? 'none'}`)
  console.log()

  // Determine next height
  const nextHeight = config.latest_height ? config.latest_height + 1 : 1
  console.log(`  Next height:     ${nextHeight}`)

  // Simulate a finalized batch:
  // - messages_hash: SHA256 of a dummy message batch
  // - certificate: SHA256(messages_hash || validator_1 || ... || validator_n)
  // - timestamp: current time in ms
  const dummyMessages = JSON.stringify({
    batch: nextHeight,
    messages: [
      { from: 'agent-1', content: 'proposal draft: treasury parameter update', gate: 'green' },
      { from: 'agent-2', content: 'vote: YES on A45', gate: 'green' },
    ],
  })
  const messagesHash = createHash('sha256').update(dummyMessages).digest()
  const certificate = computeCertificate(messagesHash, VALIDATORS)
  const timestamp = Date.now()

  console.log('Simulated batch:')
  console.log(`  messages_hash:   ${messagesHash.toString('hex')}`)
  console.log(`  certificate:     ${certificate.toString('hex')}`)
  console.log(`  timestamp:       ${timestamp}`)
  console.log()

  // Submit batch
  const submitMsg = {
    submit_batch: {
      certificate: Buffer.from(certificate).toString('base64'),
      messages_hash: Array.from(messagesHash),
      commonware_height: nextHeight,
      timestamp,
    },
  }

  console.log('Submitting SubmitBatch transaction...')
  try {
    const result = await client.execute(
      address,
      contractAddr,
      submitMsg,
      'auto',
      'coordination-settler relayer',
    )
    console.log(`  tx hash: ${result.transactionHash}`)
    console.log(`  height:  ${result.height}`)
    console.log()
    console.log('=== Batch settled on-chain! ===')

    // Verify by querying the settled batch
    const settled = await client.queryContractSmart(contractAddr, {
      batch: { commonware_height: nextHeight },
    })
    console.log('Verification:')
    console.log(`  commonware_height: ${settled.commonware_height}`)
    console.log(`  messages_hash:     ${Buffer.from(settled.messages_hash).toString('hex')}`)
    console.log(`  certificate_hash:  ${createHash('sha256').update(settled.certificate).digest('hex')}`)
    console.log(`  timestamp:         ${settled.timestamp}`)
    console.log(`  submitter:         ${settled.submitter}`)
  } catch (err) {
    console.error('ERROR: Submit failed:', err.message || err)
    if (err.logs) {
      console.error('Logs:', JSON.stringify(err.logs, null, 2))
    }
    process.exit(1)
  }
}

main().catch((err) => {
  console.error('FATAL:', err)
  process.exit(1)
})
