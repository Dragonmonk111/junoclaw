/**
 * Transaction Builder Tools — write operations, addressed by wallet_id
 *
 * Every write tool takes an opaque `wallet_id` (registered once via
 * `cosmos-mcp wallet add ...`). The mnemonic is decrypted from disk
 * inside `WalletStore.signFor`, used to construct one signing client,
 * and scrubbed from memory in the same call. It never crosses this
 * file's API boundary.
 *
 * Per Ffern C-3 (April 2026): the `mnemonic` parameter is gone. The
 * model never sees the mnemonic; the MCP transport never carries it;
 * conversation logs cannot leak it. See `mcp/src/wallet/` and the
 * `cosmos-mcp wallet ...` CLI for enrollment.
 */

import { validateWasmPath } from "../utils/path-guard.js";
import { getChain, getIbcChannel, type ChainConfig } from "../resources/chains.js";
import { getDefaultWalletStore } from "../wallet/store.js";

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
  walletId: string,
  recipient: string,
  amount: string,
  denom?: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);
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
  walletId: string,
  contractAddress: string,
  msg: Record<string, unknown>,
  funds?: Array<{ denom: string; amount: string }>,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

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
  walletId: string,
  wasmPath: string,
  memo?: string
): Promise<TxResult & { codeId: number }> {
  const chain = requireChain(chainId);

  // Path-guard FIRST (Ffern C-4): allow-root, symlink reject, size cap, magic
  // bytes. Failing here returns a clear error without ever decrypting the
  // wallet — pure input validation, no key material touched.
  const { bytes: wasmBytes } = validateWasmPath(wasmPath);

  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);
  const result = await client.upload(address, wasmBytes, "auto", memo || "cosmos-mcp upload");

  return {
    ...formatResult(chain, result),
    codeId: result.codeId,
  };
}

export async function instantiateContract(
  chainId: string,
  walletId: string,
  codeId: number,
  msg: Record<string, unknown>,
  label: string,
  admin?: string,
  funds?: Array<{ denom: string; amount: string }>,
  memo?: string
): Promise<TxResult & { contractAddress: string }> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

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
  walletId: string,
  contractAddress: string,
  newCodeId: number,
  migrateMsg: Record<string, unknown>,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

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
  walletId: string,
  namespaceHex: string,
  data: string,
  memo?: string
): Promise<BlobResult> {
  const chain = requireChain(chainId);
  if (chain.bech32Prefix !== "celestia") {
    throw new Error(`submit_blob only works on Celestia chains, got ${chainId}`);
  }

  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

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

// ═══════════════════════════════════════════════════
//  GOVERNANCE / STAKING TOOLS — targeted high-value message types
//
//  Added 2026-07-21 in response to FlipDAscript's Cosmos-chat question:
//  "can it compose sign and broadcast any message types?" Rather than
//  exposing a single unrestricted generic-message tool, the highest-value
//  known gaps (vote, delegate, undelegate, redelegate, withdraw rewards)
//  get their own explicit, schema-validated tools — smaller attack
//  surface than a raw type-url + JSON blob, same posture as every other
//  write tool in this file. All five typeUrls below are part of CosmJS's
//  default registry (no custom registration needed).
// ══════════════════════════════════════════════════

export async function voteOnProposal(
  chainId: string,
  walletId: string,
  proposalId: string,
  option: "yes" | "no" | "abstain" | "no_with_veto",
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

  const optionMap: Record<string, number> = {
    yes: 1,
    abstain: 2,
    no: 3,
    no_with_veto: 4,
  };

  const voteMsg = {
    typeUrl: "/cosmos.gov.v1beta1.MsgVote",
    value: {
      proposalId: BigInt(proposalId),
      voter: address,
      option: optionMap[option],
    },
  };

  const result = await client.signAndBroadcast(address, [voteMsg], "auto", memo || "cosmos-mcp vote");
  if (result.code !== 0) {
    throw new Error(`Vote failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

export async function delegateTokens(
  chainId: string,
  walletId: string,
  validatorAddress: string,
  amount: string,
  denom?: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);
  const d = denom || chain.denom;

  const delegateMsg = {
    typeUrl: "/cosmos.staking.v1beta1.MsgDelegate",
    value: {
      delegatorAddress: address,
      validatorAddress,
      amount: { denom: d, amount },
    },
  };

  const result = await client.signAndBroadcast(address, [delegateMsg], "auto", memo || "cosmos-mcp delegate");
  if (result.code !== 0) {
    throw new Error(`Delegate failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

export async function undelegateTokens(
  chainId: string,
  walletId: string,
  validatorAddress: string,
  amount: string,
  denom?: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);
  const d = denom || chain.denom;

  const undelegateMsg = {
    typeUrl: "/cosmos.staking.v1beta1.MsgUndelegate",
    value: {
      delegatorAddress: address,
      validatorAddress,
      amount: { denom: d, amount },
    },
  };

  const result = await client.signAndBroadcast(address, [undelegateMsg], "auto", memo || "cosmos-mcp undelegate");
  if (result.code !== 0) {
    throw new Error(`Undelegate failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

export async function redelegateTokens(
  chainId: string,
  walletId: string,
  validatorSrcAddress: string,
  validatorDstAddress: string,
  amount: string,
  denom?: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);
  const d = denom || chain.denom;

  const redelegateMsg = {
    typeUrl: "/cosmos.staking.v1beta1.MsgBeginRedelegate",
    value: {
      delegatorAddress: address,
      validatorSrcAddress,
      validatorDstAddress,
      amount: { denom: d, amount },
    },
  };

  const result = await client.signAndBroadcast(address, [redelegateMsg], "auto", memo || "cosmos-mcp redelegate");
  if (result.code !== 0) {
    throw new Error(`Redelegate failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

export async function withdrawRewards(
  chainId: string,
  walletId: string,
  validatorAddress: string,
  memo?: string
): Promise<TxResult> {
  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

  const withdrawMsg = {
    typeUrl: "/cosmos.distribution.v1beta1.MsgWithdrawDelegatorReward",
    value: {
      delegatorAddress: address,
      validatorAddress,
    },
  };

  const result = await client.signAndBroadcast(address, [withdrawMsg], "auto", memo || "cosmos-mcp withdraw-rewards");
  if (result.code !== 0) {
    throw new Error(`Withdraw rewards failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

// ═══════════════════════════════════════════════════
//  GENERIC MESSAGE COMPOSER — gated, off by default
//
//  Answers FlipDAscript's question directly: yes, this CAN compose, sign,
//  and broadcast any registered Cosmos SDK message type — but only if the
//  operator explicitly opts a specific typeUrl into
//  JUNOCLAW_ALLOWED_MSG_TYPES. Unset or empty (the default) means this
//  tool always refuses, mirroring the admin-RPC's "off by default, fails
//  loudly if half-configured" posture (see README.md "Admin RPC" section)
//  rather than the fully-open shape that would let a compromised or
//  prompt-injected model sign literally anything the chain accepts.
// ══════════════════════════════════════════════════

function getAllowedMsgTypes(): Set<string> {
  const raw = process.env.JUNOCLAW_ALLOWED_MSG_TYPES;
  if (!raw || raw.trim() === "") return new Set();
  return new Set(raw.split(",").map((s) => s.trim()).filter((s) => s.length > 0));
}

export async function composeAndBroadcastMsg(
  chainId: string,
  walletId: string,
  typeUrl: string,
  value: Record<string, unknown>,
  memo?: string
): Promise<TxResult> {
  const allowed = getAllowedMsgTypes();
  if (allowed.size === 0) {
    throw new Error(
      "Generic message composer is disabled (fail-closed default). " +
        "Set JUNOCLAW_ALLOWED_MSG_TYPES to a comma-separated list of Cosmos SDK " +
        "type URLs (e.g. '/cosmos.gov.v1.MsgVote,/cosmos.staking.v1beta1.MsgDelegate') " +
        "to enable specific message types. Prefer the dedicated typed tools " +
        "(vote_on_proposal, delegate_tokens, etc.) when available instead."
    );
  }
  if (!allowed.has(typeUrl)) {
    throw new Error(
      `Message type '${typeUrl}' is not in JUNOCLAW_ALLOWED_MSG_TYPES. ` +
        `Allowed: [${Array.from(allowed).join(", ")}]`
    );
  }

  const chain = requireChain(chainId);
  const { client, address } = await getDefaultWalletStore().signFor(walletId, chain);

  const result = await client.signAndBroadcast(
    address,
    [{ typeUrl, value }],
    "auto",
    memo || "cosmos-mcp compose-and-broadcast"
  );
  if (result.code !== 0) {
    throw new Error(`Broadcast failed with code ${result.code}: ${result.rawLog}`);
  }
  return formatResult(chain, result);
}

export async function ibcTransfer(
  sourceChainId: string,
  destChainId: string,
  walletId: string,
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

  const { client, address } = await getDefaultWalletStore().signFor(walletId, sourceChain);
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
