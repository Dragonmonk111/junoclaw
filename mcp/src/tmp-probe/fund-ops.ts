/**
 * fund-ops — fund operational accounts from the deterministic testnet
 * deployer key (SHA256("junoclaw-deployer-v1") — documented public
 * testnet seed, like write-test.ts; never enrolled in the wallet store).
 *
 *   npx tsx src/tmp-probe/fund-ops.ts
 *
 * Recipients (2026-11-08):
 *   - claw-relay checkpoint author (0 ujclaw — relay checkpoints failing
 *     hourly since Nov 3 with Bank(InsufficientFunds))
 *   - jlens-ops wallet (down to 1.9 ujclaw after the J-Lens deploys)
 */
import { Sha256 } from "@cosmjs/crypto";
import { DirectSecp256k1Wallet } from "@cosmjs/proto-signing";
import { JunoClawClient } from "../utils/junoclaw-client.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";

const chain = CHAIN_REGISTRY["junoclaw-g1"];
const DEPLOYER = "juno1dz875zg8p78anpjv3f0qt4gu5a3awpjfhtw992";
const SENDS: Array<[string, string, string]> = [
  // [recipient, amount ujclaw, memo]
  // NOTE: deployer holds ~63.8 JCLAW, not the genesis allocation — the
  // 100 JCLAW ops top-up from the original ask doesn't fit; 50+10 instead.
  ["juno134zsp2umr4vkhkvrn35uy3fv4n5uss233vnraz", "50000000", "fund claw-relay checkpoint author"],
  ["juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6", "10000000", "fund jlens-ops"],
];

async function main() {
  const priv = new Sha256(
    new TextEncoder().encode("junoclaw-deployer-v1")
  ).digest();
  const wallet = await DirectSecp256k1Wallet.fromKey(priv, "juno");
  const client = await JunoClawClient.connectWithSigner(
    chain.grpcEndpoint!,
    wallet,
    chain.chainId,
    chain.gasPrice
  );

  for (const [to, amount, memo] of SENDS) {
    const res = await client.sendTokens(
      DEPLOYER,
      to,
      [{ denom: chain.denom, amount }],
      "auto",
      memo
    );
    console.log(`${memo}: ${amount}${chain.denom} -> ${to}`);
    console.log(`  tx=${res.transactionHash} height=${res.height} code=${res.code}`);
    if (res.code !== 0) process.exit(1);
  }
}

main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
