import { DirectSecp256k1HdWallet } from '@cosmjs/proto-signing'

const wallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' })
const [{ address }] = await wallet.getAccounts()

console.log(`ADDRESS=${address}`)
console.log(`MNEMONIC=${wallet.mnemonic}`)
