/**
 * CosmJS Client Factory
 *
 * Two modes:
 *   1. Query-only (no wallet) — for reads, anyone can use
 *   2. Signing (with mnemonic) — for writes, explicit per-call
 *
 * The MCP server never persists keys. The mnemonic is passed in,
 * used for one session, and discarded. Detachment from custody.
 */

import { CosmWasmClient, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { type ChainConfig } from "../resources/chains.js";

// Cache query clients by chainId to avoid reconnecting
const queryClientCache = new Map<string, CosmWasmClient>();

export async function getQueryClient(chain: ChainConfig): Promise<CosmWasmClient> {
  const cached = queryClientCache.get(chain.chainId);
  if (cached) return cached;

  const client = await CosmWasmClient.connect(chain.rpcEndpoint);
  queryClientCache.set(chain.chainId, client);
  return client;
}

export interface SigningContext {
  client: SigningCosmWasmClient;
  address: string;
}

export async function getSigningClient(
  chain: ChainConfig,
  mnemonic: string
): Promise<SigningContext> {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix: chain.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();

  const client = await SigningCosmWasmClient.connectWithSigner(
    chain.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(chain.gasPrice) }
  );

  return { client, address: account.address };
}

export function clearClientCache(): void {
  queryClientCache.clear();
}
