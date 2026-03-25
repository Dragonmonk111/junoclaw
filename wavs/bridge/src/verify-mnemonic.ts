import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import * as readline from "readline";

const EXPECTED_ADDRESS = "juno1scpm8wukdq52lqs2g9d9ulcza4yeyy5qxct7g2";

async function main() {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  console.log("[verify] This script checks if a mnemonic matches the Mother wallet.");
  console.log("[verify] Expected address: " + EXPECTED_ADDRESS);
  console.log("[verify] The mnemonic is NOT saved anywhere — only held in memory.\n");

  const mnemonic = await new Promise<string>((resolve) => {
    rl.question("Paste your mnemonic (then press Enter): ", (answer) => {
      rl.close();
      resolve(answer.trim());
    });
  });

  if (!mnemonic || mnemonic.split(" ").length < 12) {
    console.error("[verify] ERROR: Invalid mnemonic (need at least 12 words).");
    process.exit(1);
  }

  try {
    const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
      prefix: "juno",
    });
    const [account] = await wallet.getAccounts();

    console.log("\n[verify] Derived address: " + account.address);
    console.log("[verify] Expected:        " + EXPECTED_ADDRESS);

    if (account.address === EXPECTED_ADDRESS) {
      console.log("\n[verify] MATCH — this mnemonic controls the Mother wallet.");
    } else {
      console.log("\n[verify] NO MATCH — this mnemonic derives a different address.");
      console.log("[verify] Double-check your words and word order.");
    }
  } catch (err: any) {
    console.error("[verify] ERROR: Could not derive wallet — " + (err.message || err));
    console.error("[verify] The mnemonic may have invalid words or checksum.");
    process.exit(1);
  }
}

main().catch((err) => {
  console.error("[verify] Fatal: " + (err.message || err));
  process.exit(1);
});
