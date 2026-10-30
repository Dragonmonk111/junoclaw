/**
 * skill-remove-e2e — admin hygiene: remove the e2e placeholder entry
 * `junoclaw-verifier-20261020114123` from the junoclaw-g1
 * skill-registry. Only the contract admin may call remove_skill; the
 * admin is the deployer (public devnet key, per docs/KEY_MANAGEMENT.md).
 * The derived address is asserted against the known deployer BEFORE
 * signing — if derivation ever drifted, we abort rather than broadcast.
 *
 *   npx tsx src/tmp-probe/skill-remove-e2e.ts
 */
import { createHash } from "crypto";
import { DirectSecp256k1Wallet } from "@cosmjs/proto-signing";
import { JunoClawClient } from "../utils/junoclaw-client.js";
import { CHAIN_REGISTRY } from "../resources/chains.js";

const DEPLOYER = "juno1dz875zg8p78anpjv3f0qt4gu5a3awpjfhtw992";
const E2E_NAME = "junoclaw-verifier-20261020114123";
const chain = CHAIN_REGISTRY["junoclaw-g1"];
const SR = chain.skillRegistry!;

async function main() {
  // Public devnet seed — testnet-only, documented in docs/KEY_MANAGEMENT.md.
  const priv = createHash("sha256").update("junoclaw-deployer-v1").digest();
  const wallet = await DirectSecp256k1Wallet.fromKey(priv, chain.bech32Prefix);
  const [acct] = await wallet.getAccounts();
  if (acct.address !== DEPLOYER) {
    throw new Error(
      `derived ${acct.address} != deployer ${DEPLOYER} — aborting`
    );
  }
  console.log(`admin key verified: ${acct.address}`);

  const client = await JunoClawClient.connectWithSigner(
    chain.grpcEndpoint!,
    wallet,
    chain.chainId,
    chain.gasPrice
  );
  const res = await client.execute(
    acct.address,
    SR,
    { remove_skill: { dapp_name: E2E_NAME } },
    "auto",
    "admin: remove e2e placeholder from skill-registry"
  );
  console.log(`execute: code=${res.code} height=${res.height}`);
  console.log(`tx=${res.transactionHash}`);
  if (res.code !== 0) {
    console.log(`rawLog=${res.rawLog}`);
    process.exit(1);
  }
  const remaining = await client.queryContractSmart(SR, { list_skills: {} });
  console.log(`list_skills: ${JSON.stringify(remaining)}`);
}
main().catch((e) => {
  console.error(`FAIL: ${(e as Error).message}`);
  process.exit(1);
});
