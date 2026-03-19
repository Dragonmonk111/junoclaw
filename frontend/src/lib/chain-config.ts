// ── Chain configuration for Juno testnet (uni-7) ──

export const CHAIN_CONFIG = {
  chainId: 'uni-7',
  chainName: 'Juno Testnet',
  rpc: 'https://juno-testnet-rpc.polkachu.com',
  rest: 'https://juno-testnet-api.polkachu.com',
  denom: 'ujunox',
  displayDenom: 'JUNOX',
  decimals: 6,
  gasPrice: '0.025ujunox',
  bech32Prefix: 'juno',
  coinType: 118,
} as const

// Live contract addresses on uni-7
export const CONTRACTS = {
  agentCompany: 'juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6',
  junoswapFactory: 'juno12v0t60msclf3hcj56clrnh575ct35clglqunr489aj0xsvawghvq3wtkkh',
  junoswapPairJunoUsdc: 'juno1xn4mtv9cfc7q3zphvstkhqgn4g864pppvq64zvdnmcsen3jwacwqfr6e98',
  junoswapPairJunoStake: 'juno156t270zr84xskkj6k6yq6w4pj8xu646kfjsngscpjdhhmmdt7f7s8ttg4s',
} as const

// Keplr chain suggestion (for adding uni-7 if not already added)
export const KEPLR_CHAIN_INFO = {
  chainId: CHAIN_CONFIG.chainId,
  chainName: CHAIN_CONFIG.chainName,
  rpc: CHAIN_CONFIG.rpc,
  rest: CHAIN_CONFIG.rest,
  bip44: { coinType: CHAIN_CONFIG.coinType },
  bech32Config: {
    bech32PrefixAccAddr: 'juno',
    bech32PrefixAccPub: 'junopub',
    bech32PrefixValAddr: 'junovaloper',
    bech32PrefixValPub: 'junovaloperpub',
    bech32PrefixConsAddr: 'junovalcons',
    bech32PrefixConsPub: 'junovalconspub',
  },
  currencies: [{ coinDenom: 'JUNOX', coinMinimalDenom: 'ujunox', coinDecimals: 6 }],
  feeCurrencies: [{
    coinDenom: 'JUNOX',
    coinMinimalDenom: 'ujunox',
    coinDecimals: 6,
    gasPriceStep: { low: 0.025, average: 0.03, high: 0.04 },
  }],
  stakeCurrency: { coinDenom: 'JUNOX', coinMinimalDenom: 'ujunox', coinDecimals: 6 },
}
