/**
 * Transaction Builder Tools — write operations, require mnemonic
 *
 * Every write tool requires the caller to pass a mnemonic.
 * The MCP server never stores it. The key lives for one call and dies.
 *
 * This is the difference between custody and service.
 * VairagyaNode validates your TX. It doesn't hold your coins.
 * The MCP builds your TX. It doesn't hold your keys.
 */

import { readFileSync } from "fs";
import { getSigningClient } from "../utils/cosmos-client.js";
import { getChain, type ChainConfig } from "../resources/chains.js";

function requireChain(chainId: string): ChainConfig {
  const chain = getChain(chainId);
  if (!chain) throw new Error(`Unknown chain: ${chainId}`);
  return chain;
}

export interface TxResult {
  txHash: string;
  height: number;
  gasUsed: string;
  gasWanted: string;
  explorerUrl: string;
}

function formatResult(chain: ChainConfig, raw: { transactionHash: string; height: number; gasUsed: bigint; gasWanted: bigint }): TxResult {
  return {
    txHash: raw.transactionHash,
    height: raw.height,
    gasUsed: raw.gasUsed.toString(),
    gasWanted: raw.gasWanted.toString(),
    explorerUrl: `${chain.explorerTx}/${raw.transactionHash}`,
  };
}

export async function sendTokens(
  chainId: string,
  mnemonic: string,
  recipient: string,
  amount: string,
  denom?: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getSigningClient(chain, mnemonic);
  const d = denom || chain.denom;

  const result = await client.sendTokens(
    address,
    recipient,
    [{ denom: d, amount }],
    "auto",
    memo || "cosmos-mcp"
  );

  return formatResult(chain, result);
}

export async function executeContract(
  chainId: string,
  mnemonic: string,
  contractAddress: string,
  msg: Record<string, unknown>,
  funds?: Array<{ denom: string; amount: string }>,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getSigningClient(chain, mnemonic);

  const result = await client.execute(
    address,
    contractAddress,
    msg,
    "auto",
    memo || "cosmos-mcp",
    funds
  );

  return formatResult(chain, result);
}

export async function uploadWasm(
  chainId: string,
  mnemonic: string,
  wasmPath: string,
  memo?: string
): Promise<TxResult & { codeId: number }> {
  const chain = requireChain(chainId);
  const { client, address } = await getSigningClient(chain, mnemonic);

  const wasmBytes = readFileSync(wasmPath);
  const result = await client.upload(address, wasmBytes, "auto", memo || "cosmos-mcp upload");

  return {
    ...formatResult(chain, result),
    codeId: result.codeId,
  };
}

export async function instantiateContract(
  chainId: string,
  mnemonic: string,
  codeId: number,
  msg: Record<string, unknown>,
  label: string,
  admin?: string,
  funds?: Array<{ denom: string; amount: string }>,
  memo?: string
): Promise<TxResult & { contractAddress: string }> {
  const chain = requireChain(chainId);
  const { client, address } = await getSigningClient(chain, mnemonic);

  const result = await client.instantiate(
    address,
    codeId,
    msg,
    label,
    "auto",
    {
      memo: memo || "cosmos-mcp instantiate",
      admin: admin || address,
      funds,
    }
  );

  return {
    ...formatResult(chain, result),
    contractAddress: result.contractAddress,
  };
}

export async function migrateContract(
  chainId: string,
  mnemonic: string,
  contractAddress: string,
  newCodeId: number,
  migrateMsg: Record<string, unknown>,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getSigningClient(chain, mnemonic);

  const result = await client.migrate(
    address,
    contractAddress,
    newCodeId,
    migrateMsg,
    "auto",
    memo || "cosmos-mcp migrate"
  );

  return formatResult(chain, result);
}
