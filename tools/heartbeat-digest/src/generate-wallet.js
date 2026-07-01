/**
 * One-off helper: generate a brand-new juno wallet (mnemonic + address) for
 * the heartbeat watcher to use as its Moultbook-posting identity.
 *
 * Run this yourself and keep the output private. This script does not send
 * the mnemonic anywhere — it only prints it to your local terminal.
 *
 * Why a NEW wallet instead of reusing the DAO agent key: Moultbook's `Post`
 * message (contracts/moultbook-v0/src/contract.rs::execute_post) has no
 * owner/allowlist check — any funded address can call it. So the heartbeat
 * watcher, which runs unattended and holds its mnemonic in an env var, should
 * use a small, isolated, low-privilege hot wallet rather than the DAO
 * steward/agent identity used for governance actions.
 *
 * Usage:
 *   node src/generate-wallet.js
 */

import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'

const wallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' })
const [account] = await wallet.getAccounts()
const mnemonic = wallet.mnemonic

console.log('\n=== New heartbeat-watcher wallet ===')
console.log(`Address:  ${account.address}`)
console.log(`Mnemonic: ${mnemonic}`)
console.log('\nNext steps:')
console.log('1. Fund this address with ~2 JUNO for gas (covers ~30-40 Moultbook posts).')
console.log('2. Save the mnemonic somewhere safe (password manager). It will not be shown again.')
console.log('3. Export it in this shell session only, then run the watcher:')
console.log(`   $env:JUNO_AGENT_MNEMONIC="${mnemonic}"`)
console.log('   $env:POST_TO_MOULTBOOK="true"; npm run watch:once')
console.log('')
