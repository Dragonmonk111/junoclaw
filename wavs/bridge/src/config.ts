import dotenv from "dotenv";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
dotenv.config({ path: resolve(__dirname, "../../.env") });

export const config = {
  // Juno testnet (uni-7)
  rpcEndpoint: process.env.JUNO_RPC || "https://juno-testnet-rpc.polkachu.com",
  chainId: process.env.CHAIN_ID || "uni-7",
  gasPrice: process.env.GAS_PRICE || "0.025ujunox",
  denom: process.env.DENOM || "ujunox",
  bech32Prefix: "juno",

  // Contract addresses (set in .env after deployment)
  agentCompanyContract: process.env.AGENT_COMPANY_CONTRACT || "",

  // Operator wallet mnemonic for signing txs
  mnemonic: process.env.WAVS_OPERATOR_MNEMONIC || "",

  // WAVS aggregator endpoint (for polling results)
  wavsAggregatorUrl: process.env.WAVS_AGGREGATOR_URL || "http://provider.akash-palmito.org:31812",

  // Polling interval in ms
  pollIntervalMs: parseInt(process.env.POLL_INTERVAL_MS || "5000"),
};

export function validateConfig() {
  if (!config.agentCompanyContract) {
    throw new Error("AGENT_COMPANY_CONTRACT not set in .env");
  }
  if (!config.mnemonic) {
    throw new Error("WAVS_OPERATOR_MNEMONIC not set in .env");
  }
}
