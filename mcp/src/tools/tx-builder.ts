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
import { getChain, getIbcChannel, type ChainConfig } from "../resources/chains.js";

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

export interface IbcTransferResult extends TxResult {
  sourceChain: string;
  destChain: string;
  sourceChannel: string;
  receiver: string;
  amount: string;
  denom: string;
}

/**
 * Submit a data blob to Celestia (celestiaorg/celestia-app — Apache 2.0).
 *
 * This posts data to Celestia's DA layer using MsgPayForBlobs.
 * Used by sovereign rollup chains to store block data.
 * For JunoClaw: task-execution chains can post DA to Celestia,
 * decoupling execution from data availability for 10-100x throughput.
 */
export interface BlobResult extends TxResult {
  namespace: string;
  blobSize: number;
  commitment: string;
}

export async function submitBlob(
  chainId: string,
  mnemonic: string,
  namespaceHex: string,
  data: string,
  memo?: string
): Promise<BlobResult> {
  const chain = requireChain(chainId);
  if (chain.bech32Prefix !== "celestia") {
    throw new Error(`submit_blob only works on Celestia chains, got ${chainId}`);
  }

  const { client, address } = await getSigningClient(chain, mnemonic);

  // Encode data to bytes
  const dataBytes = Buffer.from(data, "utf-8");

  // Namespace: 29 bytes (1 byte version + 28 bytes ID)
  // For user namespaces, version = 0x00, pad hex ID to 28 bytes
  const nsHex = namespaceHex.replace(/^0x/, "");
  const namespacePadded = "00" + nsHex.padStart(56, "0");
  const namespaceBytes = Buffer.from(namespacePadded, "hex");

  // Share commitment (simplified: SHA-256 of namespace + data for MCP purposes)
  const crypto = await import("crypto");
  const commitment = crypto
    .createHash("sha256")
    .update(Buffer.concat([namespaceBytes, dataBytes]))
    .digest();

  const blobMsg = {
    typeUrl: "/celestia.blob.v1.MsgPayForBlobs",
    value: {
      signer: address,
      namespaces: [namespaceBytes],
      blobSizes: [dataBytes.length],
      shareCommitments: [commitment],
      shareVersions: [0],
    },
  };

  const result = await client.signAndBroadcast(
    address,
    [blobMsg],
    "auto",
    memo || "cosmos-mcp submit-blob"
  );

  if (result.code !== 0) {
    throw new Error(`Blob submission failed with code ${result.code}: ${result.rawLog}`);
  }

  return {
    txHash: result.transactionHash,
    height: result.height,
    gasUsed: result.gasUsed.toString(),
    gasWanted: result.gasWanted.toString(),
    explorerUrl: `${chain.explorerTx}/${result.transactionHash}`,
    namespace: namespaceHex,
    blobSize: dataBytes.length,
    commitment: commitment.toString("base64"),
  };
}

export async function ibcTransfer(
  sourceChainId: string,
  destChainId: string,
  mnemonic: string,
  receiver: string,
  amount: string,
  denom?: string,
  memo?: string,
  timeoutMinutes?: number
): Promise<IbcTransferResult> {
  const sourceChain = requireChain(sourceChainId);
  requireChain(destChainId); // validate dest exists

  const ibcChannel = getIbcChannel(sourceChainId, destChainId);
  if (!ibcChannel) {
    throw new Error(
      `No IBC channel configured from ${sourceChainId} to ${destChainId}. ` +
      `Use list_chains to see available IBC routes.`
    );
  }

  const { client, address } = await getSigningClient(sourceChain, mnemonic);
  const d = denom || sourceChain.denom;
  const timeout = (timeoutMinutes || 10) * 60; // seconds

  // IBC MsgTransfer via SigningCosmWasmClient (extends SigningStargateClient)
  const timeoutTimestamp = BigInt((Date.now() + timeout * 1000) * 1_000_000); // nanoseconds

  const transferMsg = {
    typeUrl: "/ibc.applications.transfer.v1.MsgTransfer",
    value: {
      sourcePort: "transfer",
      sourceChannel: ibcChannel.sourceChannel,
      token: { denom: d, amount },
      sender: address,
      receiver,
      timeoutHeight: { revisionNumber: BigInt(0), revisionHeight: BigInt(0) },
      timeoutTimestamp,
      memo: memo || "cosmos-mcp ibc-transfer",
    },
  };

  const result = await client.signAndBroadcast(
    address,
    [transferMsg],
    "auto",
    memo || "cosmos-mcp ibc-transfer"
  );

  if (result.code !== 0) {
    throw new Error(`IBC transfer failed with code ${result.code}: ${result.rawLog}`);
  }

  return {
    txHash: result.transactionHash,
    height: result.height,
    gasUsed: result.gasUsed.toString(),
    gasWanted: result.gasWanted.toString(),
    explorerUrl: `${sourceChain.explorerTx}/${result.transactionHash}`,
    sourceChain: sourceChainId,
    destChain: destChainId,
    sourceChannel: ibcChannel.sourceChannel,
    receiver,
    amount,
    denom: d,
  };
}
