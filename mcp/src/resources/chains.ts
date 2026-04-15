/**
 * Cosmos Chain Registry — MCP Resources
 *
 * Each chain is a resource the AI can read to understand how to interact
 * with that chain. The registry is open: add any Cosmos chain.
 *
 * Philosophy: a validator doesn't pick favorites. Neither does the MCP.
 */

export interface IbcChannel {
  sourceChannel: string;
  destChannel: string;
}

export interface ChainConfig {
  chainId: string;
  chainName: string;
  rpcEndpoint: string;
  restEndpoint: string;
  denom: string;
  bech32Prefix: string;
  gasPrice: string;
  slip44: number;
  explorerTx: string;
  faucet?: string;
  isTestnet: boolean;
  ibcChannels?: Record<string, IbcChannel>;
}

export const CHAIN_REGISTRY: Record<string, ChainConfig> = {
  "uni-7": {
    chainId: "uni-7",
    chainName: "Juno Testnet",
    rpcEndpoint: "https://juno-testnet-rpc.polkachu.com",
    restEndpoint: "https://juno-testnet-api.polkachu.com",
    denom: "ujunox",
    bech32Prefix: "juno",
    gasPrice: "0.075ujunox",
    slip44: 118,
    explorerTx: "https://testnet.mintscan.io/juno-testnet/tx",
    faucet: "https://faucet.junonetwork.io/",
    isTestnet: true,
  },
  "juno-1": {
    chainId: "juno-1",
    chainName: "Juno Mainnet",
    rpcEndpoint: "https://juno-rpc.polkachu.com",
    restEndpoint: "https://juno-api.polkachu.com",
    denom: "ujuno",
    bech32Prefix: "juno",
    gasPrice: "0.075ujuno",
    slip44: 118,
    explorerTx: "https://mintscan.io/juno/tx",
    isTestnet: false,
    ibcChannels: {
      "osmosis-1": { sourceChannel: "channel-0", destChannel: "channel-42" },
      "stargaze-1": { sourceChannel: "channel-20", destChannel: "channel-5" },
      "neutron-1": { sourceChannel: "channel-548", destChannel: "channel-4328" },
    },
  },
  "osmosis-1": {
    chainId: "osmosis-1",
    chainName: "Osmosis Mainnet",
    rpcEndpoint: "https://osmosis-rpc.polkachu.com",
    restEndpoint: "https://osmosis-api.polkachu.com",
    denom: "uosmo",
    bech32Prefix: "osmo",
    gasPrice: "0.025uosmo",
    slip44: 118,
    explorerTx: "https://mintscan.io/osmosis/tx",
    isTestnet: false,
    ibcChannels: {
      "juno-1": { sourceChannel: "channel-42", destChannel: "channel-0" },
      "stargaze-1": { sourceChannel: "channel-75", destChannel: "channel-0" },
      "neutron-1": { sourceChannel: "channel-874", destChannel: "channel-10" },
    },
  },
  "stargaze-1": {
    chainId: "stargaze-1",
    chainName: "Stargaze Mainnet",
    rpcEndpoint: "https://stargaze-rpc.polkachu.com",
    restEndpoint: "https://stargaze-api.polkachu.com",
    denom: "ustars",
    bech32Prefix: "stars",
    gasPrice: "1.0ustars",
    slip44: 118,
    explorerTx: "https://mintscan.io/stargaze/tx",
    isTestnet: false,
    ibcChannels: {
      "juno-1": { sourceChannel: "channel-5", destChannel: "channel-20" },
      "osmosis-1": { sourceChannel: "channel-0", destChannel: "channel-75" },
    },
  },
  "neutron-1": {
    chainId: "neutron-1",
    chainName: "Neutron Mainnet",
    rpcEndpoint: "https://neutron-rpc.polkachu.com",
    restEndpoint: "https://neutron-api.polkachu.com",
    denom: "untrn",
    bech32Prefix: "neutron",
    gasPrice: "0.075untrn",
    slip44: 118,
    explorerTx: "https://mintscan.io/neutron/tx",
    isTestnet: false,
    ibcChannels: {
      "juno-1": { sourceChannel: "channel-4328", destChannel: "channel-548" },
      "osmosis-1": { sourceChannel: "channel-10", destChannel: "channel-874" },
    },
  },
  "celestia": {
    chainId: "celestia",
    chainName: "Celestia Mainnet",
    rpcEndpoint: "https://celestia-rpc.polkachu.com",
    restEndpoint: "https://celestia-api.polkachu.com",
    denom: "utia",
    bech32Prefix: "celestia",
    gasPrice: "0.002utia",
    slip44: 118,
    explorerTx: "https://mintscan.io/celestia/tx",
    isTestnet: false,
    ibcChannels: {
      "osmosis-1": { sourceChannel: "channel-2", destChannel: "channel-6994" },
      "neutron-1": { sourceChannel: "channel-8", destChannel: "channel-35" },
    },
  },
  "mocha-4": {
    chainId: "mocha-4",
    chainName: "Celestia Mocha Testnet",
    rpcEndpoint: "https://celestia-testnet-rpc.polkachu.com",
    restEndpoint: "https://celestia-testnet-api.polkachu.com",
    denom: "utia",
    bech32Prefix: "celestia",
    gasPrice: "0.002utia",
    slip44: 118,
    explorerTx: "https://testnet.mintscan.io/celestia-testnet/tx",
    faucet: "https://faucet.celestia-mocha.com/",
    isTestnet: true,
  },
};

export function getChain(chainId: string): ChainConfig | undefined {
  return CHAIN_REGISTRY[chainId];
}

export function listChains(): ChainConfig[] {
  return Object.values(CHAIN_REGISTRY);
}

export function listTestnets(): ChainConfig[] {
  return Object.values(CHAIN_REGISTRY).filter((c) => c.isTestnet);
}

export function listMainnets(): ChainConfig[] {
  return Object.values(CHAIN_REGISTRY).filter((c) => !c.isTestnet);
}

export function getIbcChannel(
  sourceChainId: string,
  destChainId: string
): IbcChannel | undefined {
  const chain = CHAIN_REGISTRY[sourceChainId];
  return chain?.ibcChannels?.[destChainId];
}
