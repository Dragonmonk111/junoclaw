/**
 * write-test — sign + broadcast a minimal MsgSend through the
 * JunoClawClient adapter using the DEVNET deployer key
 * (SHA256("junoclaw-deployer-v1") — a documented public seed,
 * safe to derive in-process; never enrolled in the wallet store).
 *
 *   npx tsx src/tmp-probe/write-test.ts
 */
import { Sha256 } from "@cosmjs/crypto";
import { DirectSecp256k1Wallet } from "@cosmjs/proto-signing";
import { JunoClawClient } from "../utils/junoclaw-client.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";

const chain = CHAIN_REGISTRY["junoclaw-g1"];
const ENDPOINT = chain.grpcEndpoint!;
const DEPLOYER = "juno1dz875zg8p78anpjv3f0qt4gu5a3awpjfhtw992";
const RECIPIENT = "juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z"; // builder

async function main() {
  const priv = new Sha256(
    new TextEncoder().encode("junoclaw-deployer-v1")
  ).digest();
  const wallet = await DirectSecp256k1Wallet.fromKey(priv, "juno");
  const client = await JunoClawClient.connectWithSigner(
    ENDPOINT,
    wallet,
    chain.chainId,
    chain.gasPrice
  );

  console.log("sending 1 ujclaw deployer -> builder (auto fee)…");
  const res = await client.sendTokens(
    DEPLOYER,
    RECIPIENT,
    [{ denom: "ujclaw", amount: "1" }],
    "auto",
    "mcp write-path smoke"
  );
  console.log(`broadcast accepted: code=${res.code} height=${res.height}`);
  console.log(`txhash=${res.transactionHash}`);
  console.log(`gas wanted=${res.gasWanted} used=${res.gasUsed}`);

  const found = await client.getTx(res.transactionHash);
  console.log(`getTx: ${found ? JSON.stringify((found as { height: number; code: number }).height ?? found) : "null"}`);
}

main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
