const { readFileSync, writeFileSync, existsSync } = require('fs');
const { join } = require('path');
const { DirectSecp256k1HdWallet } = require('@cosmjs/proto-signing');
const { SigningCosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
const { GasPrice, calculateFee } = require('@cosmjs/stargate');

const MNEMONIC = process.env.JUNO_MNEMONIC;
if (!MNEMONIC) {
  console.error('Error: Set JUNO_MNEMONIC environment variable');
  process.exit(1);
}
const CHAIN_ID = 'uni-7';
const RPC_URL = 'https://juno.rpc.t.stavr.tech';
const GAS_PRICE = '0.075ujunox';

const REPO_ROOT = 'c:\\cosmos-node\\node-data\\config\\CascadeProjects\\windsurf-project\\junoclaw';
const ARTIFACTS_DIR = join(REPO_ROOT, 'artifacts');
const DEVNET_DIR = join(REPO_ROOT, 'devnet');
const DEPLOYED_FILE = join(__dirname, 'deployed-testnet.json');

async function main() {
  console.log('=== JunoClaw Testnet Deploy ===');
  console.log('Chain:', CHAIN_ID);
  console.log('RPC:', RPC_URL);

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
  const [{ address }] = await wallet.getAccounts();
  console.log('Deployer:', address);

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });

  const balance = await client.getBalance(address, 'ujunox');
  console.log('Balance:', balance.amount, balance.denom, '\n');

  if (BigInt(balance.amount) < 1000000n) {
    console.error('ERROR: Insufficient balance. Need at least 1 JUNOX for deployment.');
    process.exit(1);
  }

  let deployed = {};
  if (existsSync(DEPLOYED_FILE)) {
    deployed = JSON.parse(readFileSync(DEPLOYED_FILE, 'utf8'));
  }

  // --- 1. zk-verifier pure ---
  if (!deployed['zk-verifier-pure']?.codeId) {
    const wasm = readFileSync(join(ARTIFACTS_DIR, 'zk_verifier.wasm'));
    console.log('Storing zk-verifier-pure...');
    const fee = calculateFee(15_000_000, GasPrice.fromString(GAS_PRICE));
    const result = await client.upload(address, wasm, fee, 'zk-verifier-pure');
    console.log('  codeId:', result.codeId, 'tx:', result.transactionHash);
    deployed['zk-verifier-pure'] = { codeId: result.codeId, tx: result.transactionHash };
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  if (!deployed['zk-verifier-pure']?.address) {
    console.log('Instantiating zk-verifier-pure...');
    const instFee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
    const res = await client.instantiate(address, deployed['zk-verifier-pure'].codeId, { admin: address }, 'zk-verifier-pure', instFee, { admin: address });
    console.log('  address:', res.contractAddress, 'tx:', res.transactionHash);
    deployed['zk-verifier-pure'].address = res.contractAddress;
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  // --- 2. zk-verifier precompile (skip on uni-7 — no bn254 precompile) ---
  if (CHAIN_ID === 'uni-7') {
    console.log('Skipping zk-verifier-precompile: uni-7 lacks bn254_add precompile');
    deployed['zk-verifier-precompile'] = { skipped: true, reason: 'uni-7 lacks bn254_add precompile' };
  } else if (!deployed['zk-verifier-precompile']?.codeId) {
    const wasm = readFileSync(join(ARTIFACTS_DIR, 'zk_verifier_precompile.wasm'));
    console.log('Storing zk-verifier-precompile...');
    const fee = calculateFee(15_000_000, GasPrice.fromString(GAS_PRICE));
    const result = await client.upload(address, wasm, fee, 'zk-verifier-precompile');
    console.log('  codeId:', result.codeId, 'tx:', result.transactionHash);
    deployed['zk-verifier-precompile'] = { codeId: result.codeId, tx: result.transactionHash };
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  if (CHAIN_ID !== 'uni-7' && !deployed['zk-verifier-precompile']?.address) {
    console.log('Instantiating zk-verifier-precompile...');
    const instFee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
    const res = await client.instantiate(address, deployed['zk-verifier-precompile'].codeId, { admin: address }, 'zk-verifier-precompile', instFee, { admin: address });
    console.log('  address:', res.contractAddress, 'tx:', res.transactionHash);
    deployed['zk-verifier-precompile'].address = res.contractAddress;
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  // --- 3. jclaw-credential ---
  if (!deployed['jclaw-credential']?.codeId) {
    const wasm = readFileSync(join(DEVNET_DIR, 'artifacts', 'jclaw_credential.wasm'));
    console.log('Storing jclaw-credential...');
    const fee = calculateFee(15_000_000, GasPrice.fromString(GAS_PRICE));
    const result = await client.upload(address, wasm, fee, 'jclaw-credential');
    console.log('  codeId:', result.codeId, 'tx:', result.transactionHash);
    deployed['jclaw-credential'] = { codeId: result.codeId, tx: result.transactionHash };
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  if (!deployed['jclaw-credential']?.address) {
    console.log('Instantiating jclaw-credential...');
    const instFee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
    const res = await client.instantiate(address, deployed['jclaw-credential'].codeId, { admin: address }, 'jclaw-credential', instFee, { admin: address });
    console.log('  address:', res.contractAddress, 'tx:', res.transactionHash);
    deployed['jclaw-credential'].address = res.contractAddress;
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  // --- 4. moultbook ---
  if (!deployed['moultbook']?.codeId) {
    const wasm = readFileSync(join(DEVNET_DIR, 'moultbook_v0.wasm'));
    console.log('Storing moultbook...');
    const fee = calculateFee(15_000_000, GasPrice.fromString(GAS_PRICE));
    const result = await client.upload(address, wasm, fee, 'moultbook-v0');
    console.log('  codeId:', result.codeId, 'tx:', result.transactionHash);
    deployed['moultbook'] = { codeId: result.codeId, tx: result.transactionHash };
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  if (!deployed['moultbook']?.address) {
    console.log('Instantiating moultbook...');
    const msg = {
      admin: address,
      whoami_contract: null,
      max_size_bytes: 1048576,
      max_refs: 8,
      max_content_type_len: 64,
      max_group_size: 50,
      zk_verifier: deployed['zk-verifier-precompile']?.address || deployed['zk-verifier-pure']?.address || null,
      agent_registry: deployed['jclaw-credential']?.address || null,
      membership_vk_hash: null,
    };
    const instFee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
    const res = await client.instantiate(address, deployed['moultbook'].codeId, msg, 'moultbook-v0', instFee, { admin: address });
    console.log('  address:', res.contractAddress, 'tx:', res.transactionHash);
    deployed['moultbook'].address = res.contractAddress;
    writeFileSync(DEPLOYED_FILE, JSON.stringify(deployed, null, 2));
  }

  console.log('\n=== Deployment Complete ===');
  for (const [name, info] of Object.entries(deployed)) {
    console.log(`${name}: codeId=${info.codeId} addr=${info.address}`);
  }
}

main().catch((err) => {
  console.error('\nDeploy failed:', err.message);
  process.exit(1);
});
