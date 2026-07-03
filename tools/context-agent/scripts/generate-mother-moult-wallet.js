/**
 * One-off helper: generate a brand-new juno wallet (mnemonic + address) to use
 * as the Mother-Moult publisher identity (scripts/publish-mother-moult.js).
 *
 * SECURITY: run this YOURSELF, directly in your own terminal. Never paste its
 * output into a chat/agent session — the mnemonic must never leave your local
 * machine. This script only prints to your local terminal; it does not send
 * the mnemonic anywhere.
 *
 * Why a dedicated wallet instead of reusing the reply-bot's or heartbeat
 * watcher's: the Mother-Moult is the DAO's canonical constitution artifact.
 * Isolating its publishing key means a compromised reply-bot hot wallet can't
 * also be used to (re-)publish under the Mother-Moult's name.
 *
 * Usage:
 *   node scripts/generate-mother-moult-wallet.js
 */

import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'

const wallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' })
const [account] = await wallet.getAccounts()
const mnemonic = wallet.mnemonic

console.log('\n=== New Mother-Moult publisher wallet ===')
console.log(`Address:  ${account.address}`)
console.log(`Mnemonic: ${mnemonic}`)
console.log('\nNext steps:')
console.log('1. Save the mnemonic in a password manager NOW. It will not be shown again.')
console.log('2. Fund this address with ~1-2 JUNO for gas (one Post tx + headroom for a future re-publish).')
console.log('3. Export it in THIS shell session only (never commit it, never paste it elsewhere):')
console.log(`   $env:JUNO_MOTHER_MOULT_MNEMONIC="${mnemonic}"`)
console.log('4. Dry-run first (always safe, no funds/mnemonic required to preview):')
console.log('   node scripts/publish-mother-moult.js')
console.log('5. Real broadcast (both flags required, see script header):')
console.log('   $env:PUBLISH_MOTHER_MOULT_CONFIRM="yes"; node scripts/publish-mother-moult.js')
console.log('')
