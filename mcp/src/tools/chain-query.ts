/**
 * Chain Query Tools — read-only, no wallet required
 *
 * These tools let any AI assistant explore Cosmos chain state:
 * balances, contract state, governance, transaction history.
 *
 * Every query is free. Knowledge should be free.
 */

import { getQueryClient } from "../utils/cosmos-client.js";
import { getChain, type ChainConfig } from "../resources/chains.js";

function requireChain(chainId: string): ChainConfig {
  const chain = getChain(chainId);
  if (!chain) throw new Error(`Unknown chain: ${chainId}. Use list_chains to see available chains.`);
  return chain;
}

export async function queryBalance(
  chainId: string,
  address: string,
  denom?: string
): Promise<{ address: string; amount: string; denom: string; human: string }> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  const d = denom || chain.denom;
  const balance = await client.getBalance(address, d);

  // Convert to human-readable (assume 6 decimals for most Cosmos denoms)
  const decimals = 6;
  const human = (parseInt(balance.amount) / Math.pow(10, decimals)).toFixed(decimals);

  return {
    address,
    amount: balance.amount,
    denom: d,
    human: `${human} ${d.replace(/^u/, "").toUpperCase()}`,
  };
}

export async function queryAllBalances(
  chainId: string,
  address: string
): Promise<{ address: string; balances: Array<{ amount: string; denom: string }> }> {
  const chain = requireChain(chainId);
  // CosmWasmClient doesn't expose getAllBalances — use REST
  const resp = await fetch(
    `${chain.restEndpoint}/cosmos/bank/v1beta1/balances/${address}`
  );
  const data = (await resp.json()) as { balances: Array<{ amount: string; denom: string }> };
  return { address, balances: data.balances };
}

export async function queryContractState(
  chainId: string,
  contractAddress: string,
  queryMsg: Record<string, unknown>
): Promise<unknown> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  return client.queryContractSmart(contractAddress, queryMsg);
}

export async function queryContractInfo(
  chainId: string,
  contractAddress: string
): Promise<{
  address: string;
  codeId: number;
  creator: string;
  admin: string | undefined;
  label: string;
}> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  const info = await client.getContract(contractAddress);
  return {
    address: info.address,
    codeId: info.codeId,
    creator: info.creator,
    admin: info.admin ?? undefined,
    label: info.label,
  };
}

export async function queryTx(
  chainId: string,
  txHash: string
): Promise<{
  hash: string;
  height: number;
  code: number;
  gasUsed: string;
  gasWanted: string;
  events: Array<{ type: string; attributes: Array<{ key: string; value: string }> }>;
}> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  const tx = await client.getTx(txHash);
  if (!tx) throw new Error(`TX not found: ${txHash}`);
  return {
    hash: tx.hash,
    height: tx.height,
    code: tx.code,
    gasUsed: tx.gasUsed.toString(),
    gasWanted: tx.gasWanted.toString(),
    events: tx.events.map((e) => ({
      type: e.type,
      attributes: e.attributes.map((a) => ({ key: a.key, value: a.value })),
    })),
  };
}

export async function queryBlockHeight(chainId: string): Promise<{ chainId: string; height: number }> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  const height = await client.getHeight();
  return { chainId, height };
}

export async function queryCodeInfo(
  chainId: string,
  codeId: number
): Promise<{ codeId: number; creator: string; checksum: string }> {
  const chain = requireChain(chainId);
  const client = await getQueryClient(chain);
  const info = await client.getCodeDetails(codeId);
  return {
    codeId: info.id,
    creator: info.creator,
    checksum: info.checksum,
  };
}
