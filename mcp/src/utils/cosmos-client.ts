/**
 * CosmJS Client Factory
 *
 * Read path: `getQueryClient(chain)` — anyone can use, no wallet.
 * Write path: `WalletStore.signFor(walletId, chain)` — see
 *             `../wallet/store.ts`.
 *
 * Per Ffern C-3 (April 2026), this module no longer exposes a
 * mnemonic-taking `getSigningClient`. All signing flows go through
 * the encrypted wallet registry by `wallet_id`; the mnemonic is
 * decrypted in-process for one signing-client construction and
 * scrubbed from memory immediately afterwards. See
 * `mcp/src/wallet/store.ts` for the replacement.
 */

import { CosmWasmClient, SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
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

export function clearClientCache(): void {
  queryClientCache.clear();
}
