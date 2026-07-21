/**
 * Determinism test for sealed signer's sign-cosmos-execute-tx.
 *
 * Runs the component 3 times with identical inputs and verifies
 * that tx_bytes and sign_doc_sha256_hex are byte-identical across
 * all runs. This proves the software path is deterministic.
 *
 * Usage:
 *   node scripts/determinism-test.js
 */

const { spawnSync } = require('child_process');
const crypto = require('crypto');
const path = require('path');

const wasmtime = process.env.WASMTIME || 'C:\\Users\\Taj\\.cargo\\bin\\wasmtime.exe';
const wasm = process.env.WASM || path.resolve(__dirname, '..', 'target', 'wasm32-wasip2', 'release', 'junoclaw_sealed_signer.wasm');

const passphrase = 'dettest';

function runWasmtime(invoke) {
  const result = spawnSync(wasmtime, [
    'run',
    '--env', `WAVS_ENV_SIGNER_PASSPHRASE=${passphrase}`,
    '--invoke', invoke,
    wasm,
  ], {
    encoding: 'utf8',
    maxBuffer: 10 * 1024 * 1024,
  });
  if (result.status !== 0) {
    console.error('wasmtime stderr:', result.stderr);
    throw new Error(`wasmtime failed (exit ${result.status}): ${result.stderr}`);
  }
  return result.stdout.trim();
}

function parseRecord(out) {
  const m = out.match(/^ok\(\{(.+)\}\)$/s);
  if (!m) throw new Error(`unexpected wasmtime output: ${out}`);
  const record = {};
  const fieldRe = /(\w[\w-]*):\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\])/g;
  let match;
  while ((match = fieldRe.exec(m[1])) !== null) {
    const key = match[1];
    let raw = match[2];
    if (raw.startsWith('"')) {
      record[key] = JSON.parse(raw);
    } else if (raw.startsWith('[')) {
      record[key] = raw === '[]' ? [] : raw.slice(1, -1).split(',').map(s => parseInt(s.trim(), 10));
    }
  }
  return record;
}

function main() {
  // Step 1: Generate a key
  console.log('=== Sealed Signer Determinism Test ===\n');
  console.log('Step 1: Generate key...');
  const genOut = runWasmtime('generate-key()');
  const keyInfo = parseRecord(genOut);
  console.log(`  address: ${keyInfo.address}`);
  console.log(`  pubkey:  ${keyInfo.pubkey}`);
  console.log(`  sealed-blob length: ${keyInfo['sealed-blob'].length} bytes`);

  const blobList = keyInfo['sealed-blob'].join(',');

  // Fixed inputs for determinism test
  const req = {
    sender: keyInfo.address,
    contract: 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j',
    exec_msg_json: JSON.stringify({ post: { commitment: 'test-commitment-hash', content_type: 'text/plain', size_bytes: 4, attestation_ref: null, visibility: 'public', refs: [] } }),
    funds_denom: '',
    funds_amount: '0',
    gas_limit: 200000,
    fee_denom: 'ujunox',
    fee_amount: '5000',
    memo: 'determinism test',
    chain_id: 'uni-7',
    account_number: 42,
    sequence: 7,
  };

  // Build WAVE invoke expression — field names are bare kebab-case identifiers (no quotes).
  // String values use \" for escaped inner quotes.
  // JSON.stringify on a string produces a WAVE-compatible quoted string with escaped inner quotes.
  const invokeExpr = `sign-cosmos-execute-tx([${blobList}], {sender: "${req.sender}", contract: "${req.contract}", exec-msg-json: ${JSON.stringify(req.exec_msg_json)}, funds-denom: "${req.funds_denom}", funds-amount: ${BigInt(req.funds_amount).toString()}, gas-limit: ${req.gas_limit}, fee-denom: "${req.fee_denom}", fee-amount: ${BigInt(req.fee_amount).toString()}, memo: ${JSON.stringify(req.memo)}, chain-id: "${req.chain_id}", account-number: ${req.account_number}, sequence: ${req.sequence}})`;

  // Step 2: Run sign-cosmos-execute-tx 3 times
  const results = [];
  for (let i = 0; i < 3; i++) {
    console.log(`\nStep 2.${i + 1}: sign-cosmos-execute-tx (run ${i + 1})...`);
    const out = runWasmtime(invokeExpr);
    const parsed = parseRecord(out);
    const txBytesHex = Buffer.from(parsed['tx-bytes']).toString('hex');
    const signDocHash = parsed['sign-doc-sha256-hex'];

    console.log(`  address:           ${parsed.address}`);
    console.log(`  pubkey:            ${parsed.pubkey}`);
    console.log(`  sign_doc_sha256:   ${signDocHash}`);
    console.log(`  tx_bytes length:   ${parsed['tx-bytes'].length} bytes`);
    console.log(`  tx_bytes (hex):    ${txBytesHex.slice(0, 64)}...`);

    results.push({
      run: i + 1,
      address: parsed.address,
      pubkey: parsed.pubkey,
      signDocHash,
      txBytesHex,
      txBytesLen: parsed['tx-bytes'].length,
    });
  }

  // Step 3: Compare
  console.log('\n=== Determinism Check ===\n');

  let allMatch = true;

  // Compare sign_doc_sha256_hex
  const hash0 = results[0].signDocHash;
  const hashMatch = results.every(r => r.signDocHash === hash0);
  console.log(`sign_doc_sha256_hex identical across all 3 runs: ${hashMatch ? 'YES ✓' : 'NO ✗'}`);
  if (!hashMatch) {
    results.forEach(r => console.log(`  run ${r.run}: ${r.signDocHash}`));
    allMatch = false;
  }

  // Compare tx_bytes
  const tx0 = results[0].txBytesHex;
  const txMatch = results.every(r => r.txBytesHex === tx0);
  console.log(`tx_bytes identical across all 3 runs:           ${txMatch ? 'YES ✓' : 'NO ✗'}`);
  if (!txMatch) {
    results.forEach(r => console.log(`  run ${r.run}: ${r.txBytesHex.slice(0, 64)}...`));
    allMatch = false;
  }

  // Compare addresses
  const addr0 = results[0].address;
  const addrMatch = results.every(r => r.address === addr0);
  console.log(`address identical across all 3 runs:             ${addrMatch ? 'YES ✓' : 'NO ✗'}`);
  if (!addrMatch) {
    allMatch = false;
  }

  // Compare pubkeys
  const pub0 = results[0].pubkey;
  const pubMatch = results.every(r => r.pubkey === pub0);
  console.log(`pubkey identical across all 3 runs:              ${pubMatch ? 'YES ✓' : 'NO ✗'}`);
  if (!pubMatch) {
    allMatch = false;
  }

  console.log(`\ntx_bytes length: ${results[0].txBytesLen} bytes`);
  console.log(`sign_doc_sha256_hex: ${hash0}`);

  if (allMatch) {
    console.log('\n✅ ALL CHECKS PASSED — sign-cosmos-execute-tx is deterministic in software.');
    console.log('   Next step: run on SGX hardware (Azure DC-series VM) to confirm hardware determinism.');
  } else {
    console.log('\n❌ DETERMINISM FAILED — outputs differ across runs.');
    process.exit(1);
  }
}

main();
