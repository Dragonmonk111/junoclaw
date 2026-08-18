import { readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'
import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'

const __dir = dirname(fileURLToPath(import.meta.url))

const RPC_URL   = 'https://juno.rpc.t.stavr.tech'
const ARTIFACTS = 'C:\\Temp\\junoclaw-wasm-target\\wasm32-unknown-unknown\\release'

const state = JSON.parse(readFileSync(join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json'), 'utf8'))
const mp = state.mps.find(m => m.name === 'The Builder')

const deployed = JSON.parse(readFileSync(join(__dir, 'deployed.json'), 'utf8'))
const contractAddr = deployed['marketplace']?.address
if (!contractAddr) {
  console.error('marketplace address not found in deployed.json')
  process.exit(1)
}

const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mp.mnemonic, { prefix: 'juno' })
const [acc] = await wallet.getAccounts()
const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
  gasPrice: GasPrice.fromString('0.075ujunox'),
})

console.log('Deployer:', acc.address)
console.log('Migrating marketplace at', contractAddr)

const wasm = readFileSync(join(ARTIFACTS, 'marketplace.wasm'))
console.log(`Uploading new wasm (${(wasm.length / 1024).toFixed(1)} KB)...`)
const up = await client.upload(acc.address, wasm, 'auto', 'marketplace skill-registry cross-check')
console.log('New code_id:', up.codeId, 'tx:', up.transactionHash)

console.log('Migrating contract to code_id', up.codeId, '...')
const mig = await client.migrate(acc.address, contractAddr, up.codeId, {}, 'auto')
console.log('Migrated! tx:', mig.transactionHash)

const cfg = await client.queryContractSmart(contractAddr, { get_config: {} })
console.log('Verified config:', JSON.stringify(cfg))

deployed['marketplace'].code_id = up.codeId
deployed['marketplace'].migrate_tx = mig.transactionHash
writeFileSync(join(__dir, 'deployed.json'), JSON.stringify(deployed, null, 2))
console.log('Updated deployed.json')

process.exit(0)
