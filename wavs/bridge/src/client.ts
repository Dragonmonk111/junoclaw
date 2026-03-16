import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

let _client: SigningCosmWasmClient | null = null;
let _senderAddress: string | null = null;

export async function getClient(): Promise<{
  client: SigningCosmWasmClient;
  sender: string;
}> {
  if (_client && _senderAddress) {
    return { client: _client, sender: _senderAddress };
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();
  _senderAddress = account.address;

  _client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  console.log(`[bridge] Connected as ${_senderAddress}`);
  return { client: _client, sender: _senderAddress };
}

export async function submitAttestation(
  proposalId: number,
  taskType: string,
  dataHash: string,
  attestationHash: string
): Promise<string> {
  const { client, sender } = await getClient();

  const msg = {
    submit_attestation: {
      proposal_id: proposalId,
      task_type: taskType,
      data_hash: dataHash,
      attestation_hash: attestationHash,
    },
  };

  const result = await client.execute(
    sender,
    config.agentCompanyContract,
    msg,
    "auto",
    `WAVS attestation for proposal ${proposalId}`
  );

  console.log(
    `[bridge] SubmitAttestation tx: ${result.transactionHash} (proposal=${proposalId}, type=${taskType})`
  );
  return result.transactionHash;
}

export async function submitRandomness(
  jobId: string,
  randomnessHex: string,
  attestationHash: string
): Promise<string> {
  const { client, sender } = await getClient();

  const msg = {
    submit_randomness: {
      job_id: jobId,
      randomness_hex: randomnessHex,
      attestation_hash: attestationHash,
    },
  };

  const result = await client.execute(
    sender,
    config.agentCompanyContract,
    msg,
    "auto",
    `WAVS drand randomness for job ${jobId}`
  );

  console.log(
    `[bridge] SubmitRandomness tx: ${result.transactionHash} (job=${jobId})`
  );
  return result.transactionHash;
}
