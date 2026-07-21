import { readFileSync, existsSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dir = dirname(fileURLToPath(import.meta.url))
const PARLIAMENT_STATE = join(__dir, '..', 'wavs', 'bridge', 'parliament-state.json')

if (!existsSync(PARLIAMENT_STATE)) {
  console.error(`parliament-state.json not found at ${PARLIAMENT_STATE}`)
  process.exit(1)
}

const state = JSON.parse(readFileSync(PARLIAMENT_STATE, 'utf8'))
const mp = (state.mps || []).find((m) => m.name === 'The Builder')
if (!mp) {
  console.error('No MP named "The Builder" in parliament-state.json')
  process.exit(1)
}

console.log(`  Found:    The Builder (${mp.address})`)

const { WalletStore } = await import('../mcp/dist/wallet/store.js')
const store = WalletStore.defaultStore()

const walletId = process.env.WALLET_ID || 'builder'
const chainId = process.env.CHAIN_ID || 'juno-1'

try {
  const entry = await store.add(walletId, mp.mnemonic, {
    bech32Prefix: 'juno',
    backend: 'keychain',
  })
  console.log(`  Enrolled: ${walletId} -> ${entry.address} (backend: keychain)`)
  console.log(`  Mnemonic source: parliament-state.json (read once, encrypted by DPAPI, never stored in env)`)
} catch (e) {
  if (e.message?.includes('already exists')) {
    console.log(`  Wallet "${walletId}" already exists — verifying address...`)
    const addr = await store.verifyAddress(walletId)
    console.log(`  Verified: ${walletId} -> ${addr}`)
  } else {
    console.error(`  Enrollment failed: ${e.message}`)
    process.exit(1)
  }
}

// Scrub
mp.mnemonic = ''
process.exit(0)
