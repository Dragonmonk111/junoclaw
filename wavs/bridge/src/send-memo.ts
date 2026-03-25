import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { GasPrice } from "@cosmjs/stargate";
import { config } from "./config.js";

async function main() {
  const recipient = process.argv[2];
  const memo = process.argv.slice(3).join(" ");

  if (!recipient || !memo) {
    console.error("Usage: npx tsx src/send-memo.ts <juno1-address> <memo text>");
    process.exit(1);
  }

  if (new TextEncoder().encode(memo).length > 256) {
    console.error("[memo] ERROR: Memo exceeds 256 bytes. Trim your message.");
    process.exit(1);
  }

  if (!config.mnemonic) {
    console.error("[memo] ERROR: WAVS_OPERATOR_MNEMONIC not set in wavs/.env");
    process.exit(1);
  }

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(config.mnemonic, {
    prefix: config.bech32Prefix,
  });
  const [account] = await wallet.getAccounts();

  const client = await SigningCosmWasmClient.connectWithSigner(
    config.rpcEndpoint,
    wallet,
    { gasPrice: GasPrice.fromString(config.gasPrice) }
  );

  const balance = await client.getBalance(account.address, config.denom);

  console.log("[memo] From:    " + account.address);
  console.log("[memo] To:      " + recipient);
  console.log("[memo] Memo:    " + memo);
  console.log("[memo] Balance: " + (Number(balance.amount) / 1e6).toFixed(6) + " JUNOX");
  console.log("[memo] Broadcasting...");

  const result = await client.sendTokens(
    account.address,
    recipient,
    [{ denom: config.denom, amount: "1" }],
    "auto",
    memo
  );

  console.log("[memo] TX Hash:    " + result.transactionHash);
  console.log("[memo] Block:      " + result.height);
  console.log("[memo] Gas used:   " + result.gasUsed);

  const postBal = await client.getBalance(account.address, config.denom);
  console.log("[memo] Remaining:  " + (Number(postBal.amount) / 1e6).toFixed(6) + " JUNOX");
  console.log("[memo] Message is now permanently on Juno testnet.");
}

main().catch((err) => {
  console.error("[memo] Fatal: " + (err.message || err));
  process.exit(1);
});
