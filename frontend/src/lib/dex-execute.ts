// ── Signed transactions against Junoswap v2 pair contracts ──

import { coin } from '@cosmjs/stargate'
import {
  connectKeplr,
  getWalletAddress,
  isWalletConnected,
} from './contract-execute'

// Re-export wallet state for convenience
export { getWalletAddress, isWalletConnected, connectKeplr }

// We need the signing client from contract-execute — but it's module-private.
// Instead, we import connectKeplr and use the same pattern:
// the signing client is cached inside contract-execute.ts.
// For DEX execute, we duplicate the signer access pattern.

import { SigningCosmWasmClient } from '@cosmjs/cosmwasm-stargate'
import { GasPrice } from '@cosmjs/stargate'
import { CHAIN_CONFIG, KEPLR_CHAIN_INFO } from './chain-config'

interface KeplrWindow {
  keplr?: {
    enable: (chainId: string) => Promise<void>
    experimentalSuggestChain: (chainInfo: unknown) => Promise<void>
    getOfflineSigner: (chainId: string) => unknown
    getKey: (chainId: string) => Promise<{ bech32Address: string; name: string }>
  }
}
declare const window: KeplrWindow & typeof globalThis

let _dexClient: SigningCosmWasmClient | null = null
let _dexAddress: string | null = null

async function requireDexSigner(): Promise<{ client: SigningCosmWasmClient; sender: string }> {
  if (_dexClient && _dexAddress) return { client: _dexClient, sender: _dexAddress }

  // Try to get from existing Keplr connection
  if (!window.keplr) throw new Error('Keplr not found')

  try { await window.keplr.experimentalSuggestChain(KEPLR_CHAIN_INFO) } catch { /* ok */ }
  await window.keplr.enable(CHAIN_CONFIG.chainId)

  const signer = window.keplr.getOfflineSigner(CHAIN_CONFIG.chainId)
  const key = await window.keplr.getKey(CHAIN_CONFIG.chainId)

  _dexClient = await SigningCosmWasmClient.connectWithSigner(
    CHAIN_CONFIG.rpc,
    signer as any,
    { gasPrice: GasPrice.fromString(CHAIN_CONFIG.gasPrice) },
  )
  _dexAddress = key.bech32Address

  return { client: _dexClient, sender: _dexAddress }
}

// ── Swap ──
// Sends native tokens as funds with the Swap message

export async function executeSwap(
  pairAddr: string,
  offerDenom: string,
  offerAmount: string,
  minReturn?: string,
) {
  const { client, sender } = await requireDexSigner()

  const msg = {
    swap: {
      offer_asset: { native: offerDenom },
      min_return: minReturn ?? null,
    },
  }

  return client.execute(
    sender,
    pairAddr,
    msg,
    'auto',
    undefined,
    [coin(offerAmount, offerDenom)],
  )
}

// ── Provide Liquidity ──
// Sends both native tokens as funds with empty ProvideLiquidity message

export async function executeProvideLiquidity(
  pairAddr: string,
  denomA: string,
  amountA: string,
  denomB: string,
  amountB: string,
) {
  const { client, sender } = await requireDexSigner()

  const funds = [
    coin(amountA, denomA),
    coin(amountB, denomB),
  ].sort((a, b) => a.denom.localeCompare(b.denom)) // Cosmos requires sorted funds

  return client.execute(
    sender,
    pairAddr,
    { provide_liquidity: {} },
    'auto',
    undefined,
    funds,
  )
}

// ── Withdraw Liquidity ──
// Burns LP shares and receives proportional reserves

export async function executeWithdrawLiquidity(
  pairAddr: string,
  lpAmount: string,
) {
  const { client, sender } = await requireDexSigner()

  return client.execute(
    sender,
    pairAddr,
    { withdraw_liquidity: { lp_amount: lpAmount } },
    'auto',
  )
}
