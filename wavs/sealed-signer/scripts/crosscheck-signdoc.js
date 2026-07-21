#!/usr/bin/env node
//
// Cross-checks the Rust sealed-signer's Cosmos SIGN_MODE_DIRECT SignDoc bytes
// against an independent implementation built directly from cosmjs-types
// (the same primitives @cosmjs/proto-signing itself is built on).
//
// This does NOT use @cosmjs/proto-signing's Registry/makeSignDoc helpers on
// purpose — the point is to reconstruct the exact TxBody/AuthInfo/SignDoc
// bytes from first principles and confirm they match byte-for-byte with
// what wavs/sealed-signer/src/crypto.rs::sign_execute_contract_tx produced,
// not to test cosmjs against itself.
//
// Usage:
//   cd wavs/sealed-signer
//   cargo run --example print_signdoc_fixture | node scripts/crosscheck-signdoc.js
//
// Requires cosmjs-types, resolved from tools/context-agent/node_modules.

const path = require("path");
const crypto = require("crypto");

const nodeModules = path.resolve(__dirname, "../../../tools/context-agent/node_modules");
function req(pkg) {
  return require(path.join(nodeModules, pkg));
}

const { MsgExecuteContract } = req("cosmjs-types/cosmwasm/wasm/v1/tx");
const { TxBody, AuthInfo, SignDoc, Fee } = req("cosmjs-types/cosmos/tx/v1beta1/tx");
const { SignerInfo, ModeInfo } = req("cosmjs-types/cosmos/tx/v1beta1/tx");
const { PubKey } = req("cosmjs-types/cosmos/crypto/secp256k1/keys");
const { SignMode } = req("cosmjs-types/cosmos/tx/signing/v1beta1/signing");
const { Any } = req("cosmjs-types/google/protobuf/any");

// Must exactly match the fixed test vector in
// wavs/sealed-signer/examples/print_signdoc_fixture.rs.
const FIXTURE_REQUEST = {
  execMsgJson: '{"post":{"text":"hello moultbook"}}',
  feeDenom: "ujuno",
  feeAmount: "5000",
  gasLimit: 200000n,
  memo: "AKB export from junoclaw",
  chainId: "uni-7",
  accountNumber: 42n,
  sequence: 7n,
};

function readStdin() {
  return new Promise((resolve, reject) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => (data += chunk));
    process.stdin.on("end", () => resolve(data));
    process.stdin.on("error", reject);
  });
}

async function main() {
  const raw = await readStdin();
  const fixture = JSON.parse(raw.trim().split("\n").pop());

  const address = fixture.address;
  const pubkeyBytes = Buffer.from(fixture.pubkey_compressed_hex, "hex");

  const execMsgAny = Any.fromPartial({
    typeUrl: "/cosmwasm.wasm.v1.MsgExecuteContract",
    value: MsgExecuteContract.encode(
      MsgExecuteContract.fromPartial({
        sender: address,
        contract: address,
        msg: Buffer.from(FIXTURE_REQUEST.execMsgJson, "utf8"),
        funds: [],
      })
    ).finish(),
  });

  const bodyBytes = TxBody.encode(
    TxBody.fromPartial({
      messages: [execMsgAny],
      memo: FIXTURE_REQUEST.memo,
      timeoutHeight: 0n,
    })
  ).finish();

  const pubkeyAny = Any.fromPartial({
    typeUrl: "/cosmos.crypto.secp256k1.PubKey",
    value: PubKey.encode(PubKey.fromPartial({ key: pubkeyBytes })).finish(),
  });

  const signerInfo = SignerInfo.fromPartial({
    publicKey: pubkeyAny,
    modeInfo: ModeInfo.fromPartial({
      single: { mode: SignMode.SIGN_MODE_DIRECT },
    }),
    sequence: FIXTURE_REQUEST.sequence,
  });

  const authInfoBytes = AuthInfo.encode(
    AuthInfo.fromPartial({
      signerInfos: [signerInfo],
      fee: Fee.fromPartial({
        amount: [{ denom: FIXTURE_REQUEST.feeDenom, amount: FIXTURE_REQUEST.feeAmount }],
        gasLimit: FIXTURE_REQUEST.gasLimit,
      }),
    })
  ).finish();

  const signDocBytes = SignDoc.encode(
    SignDoc.fromPartial({
      bodyBytes,
      authInfoBytes,
      chainId: FIXTURE_REQUEST.chainId,
      accountNumber: FIXTURE_REQUEST.accountNumber,
    })
  ).finish();

  const signDocSha256Hex = crypto.createHash("sha256").update(signDocBytes).digest("hex");

  console.log("Rust  sign_doc_sha256_hex:", fixture.sign_doc_sha256_hex);
  console.log("Node  sign_doc_sha256_hex:", signDocSha256Hex);

  if (signDocSha256Hex !== fixture.sign_doc_sha256_hex) {
    console.error("\nMISMATCH: cosmjs-types and cosmrs produced different SignDoc bytes.");
    process.exit(1);
  }

  console.log("\nMATCH: cosmrs and cosmjs-types independently agree on the exact SignDoc bytes.");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
