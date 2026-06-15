// MAYO multi-variant on-chain gas benchmark (uni-7).
// Stores the multi-variant jclaw-credential wasm, instantiates a fresh
// instance, then for each variant: Bud (child + PK hash) -> VerifyMayoAttestation.
// Results written to deploy/mayo-benchmark-results.json.
const { readFileSync, writeFileSync, existsSync } = require('fs');
const { join } = require('path');
const { DirectSecp256k1HdWallet } = require('@cosmjs/proto-signing');
const { SigningCosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
const { GasPrice, calculateFee } = require('@cosmjs/stargate');

const REPO_ROOT = join(__dirname, '..');

// ── Load .env if present ──
const envPath = join(REPO_ROOT, '.env');
if (existsSync(envPath)) {
  readFileSync(envPath, 'utf8')
    .split('\n')
    .forEach(line => {
      const m = line.match(/^([^#=\s]+)=(.*)$/);
      if (m && !process.env[m[1]]) process.env[m[1]] = m[2].trim();
    });
}

const MNEMONIC = process.env.JUNO_MNEMONIC;
if (!MNEMONIC) {
  console.error('Error: Create junoclaw/.env with JUNO_MNEMONIC=your words');
  process.exit(1);
}
const CHAIN_ID = 'uni-7';
// Candidate RPCs — first reachable one wins. Override with UNI7_RPC env var.
const RPC_CANDIDATES = [
  process.env.UNI7_RPC,
  'https://juno.rpc.t.stavr.tech',
  'https://rpc.uni.junonetwork.io',
  'https://juno-testnet-rpc.polkachu.com',
  'https://uni-rpc.reece.sh',
].filter(Boolean);
const GAS_PRICE = '0.075ujunox';

async function pickRpc() {
  for (const url of RPC_CANDIDATES) {
    try {
      const res = await fetch(url + '/status', { signal: AbortSignal.timeout(8000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const body = await res.json();
      const net = body?.result?.node_info?.network;
      if (net !== CHAIN_ID) throw new Error(`wrong chain: ${net}`);
      const behind = body?.result?.sync_info?.catching_up;
      if (behind) throw new Error('node catching up');
      console.log('RPC selected:', url);
      return url;
    } catch (e) {
      console.log(`RPC unavailable: ${url} (${e.message})`);
    }
  }
  throw new Error('No reachable uni-7 RPC. Set UNI7_RPC=<url> and retry.');
}

const WASM_PATH = join(REPO_ROOT, 'devnet', 'artifacts', 'jclaw_credential.wasm');
const RESULTS_FILE = join(__dirname, 'mayo-benchmark-results.json');

// ── Vectors from contract source ──
const vectorSource = readFileSync(
  join(REPO_ROOT, 'contracts', 'jclaw-credential', 'src', 'mayo_vectors.rs'),
  'utf8'
);

function extractConst(source, name) {
  const m = source.match(new RegExp(`pub const ${name}: &\\[u8\\] = b"([^"]+)";`));
  if (!m) throw new Error(`Cannot extract ${name}`);
  return Buffer.from(m[1], 'utf8');
}

function extractLargeHex(source, name) {
  const prefix = `pub const ${name}: &str = "`;
  const start = source.indexOf(prefix);
  if (start === -1) throw new Error(`Cannot find ${name} start`);
  const quoteStart = start + prefix.length;
  const quoteEnd = source.indexOf('"', quoteStart);
  if (quoteEnd === -1) throw new Error(`Cannot find ${name} end`);
  return Buffer.from(source.slice(quoteStart, quoteEnd), 'hex');
}

const MSG = extractConst(vectorSource, 'MSG');

const VARIANTS = [
  {
    name: 'mayo2',
    nistLevel: 1,
    pk: extractLargeHex(vectorSource, 'PK_HEX'),
    sig: extractLargeHex(vectorSource, 'SIG_HEX'),
  },
  {
    name: 'mayo3',
    nistLevel: 3,
    pk: extractLargeHex(vectorSource, 'MAYO3_PK_HEX'),
    sig: extractLargeHex(vectorSource, 'MAYO3_SIG_HEX'),
  },
  {
    name: 'mayo5',
    nistLevel: 5,
    pk: extractLargeHex(vectorSource, 'MAYO5_PK_HEX'),
    sig: extractLargeHex(vectorSource, 'MAYO5_SIG_HEX'),
  },
];

async function main() {
  console.log('=== MAYO Multi-Variant Gas Benchmark ===');
  const RPC_URL = await pickRpc();
  console.log('Chain:', CHAIN_ID, '| RPC:', RPC_URL);
  for (const v of VARIANTS) {
    console.log(`  ${v.name}: pk=${v.pk.length} B, sig=${v.sig.length} B (NIST L${v.nistLevel})`);
  }
  console.log();

  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
  const [{ address: adminAddr }] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });
  const balance = await client.getBalance(adminAddr, 'ujunox');
  console.log('Admin:', adminAddr, '| Balance:', balance.amount, 'ujunox\n');

  let results = existsSync(RESULTS_FILE)
    ? JSON.parse(readFileSync(RESULTS_FILE, 'utf8'))
    : {};
  const save = () => writeFileSync(RESULTS_FILE, JSON.stringify(results, null, 2));

  // ── Store multi-variant code (idempotent via results file) ──
  if (!results.codeId) {
    console.log('[store] Uploading multi-variant jclaw_credential.wasm...');
    const wasm = readFileSync(WASM_PATH);
    const fee = calculateFee(15_000_000, GasPrice.fromString(GAS_PRICE));
    const up = await client.upload(adminAddr, wasm, fee, 'jclaw-credential-multivariant');
    console.log('  codeId:', up.codeId, '| tx:', up.transactionHash, '| gas:', up.gasUsed);
    results.codeId = up.codeId;
    results.storeTx = up.transactionHash;
    results.storeGas = String(up.gasUsed);
    results.wasmBytes = wasm.length;
    save();
  }

  // ── Instantiate fresh benchmark instance ──
  if (!results.address) {
    console.log('[init] Instantiating benchmark instance...');
    const fee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
    const inst = await client.instantiate(
      adminAddr,
      results.codeId,
      { admin: adminAddr },
      'jclaw-credential-mayo-bench',
      fee,
      { admin: adminAddr }
    );
    console.log('  address:', inst.contractAddress, '| tx:', inst.transactionHash);
    results.address = inst.contractAddress;
    save();
  }
  console.log();

  // ── Benchmark ladder ──
  results.variants = results.variants || {};
  for (const v of VARIANTS) {
    if (results.variants[v.name]?.verifyGas) {
      console.log(`[${v.name}] already benchmarked — verify gas: ${results.variants[v.name].verifyGas}`);
      continue;
    }
    console.log(`[${v.name}] Bud + Verify...`);
    const childWallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' });
    const [{ address: childAddr }] = await childWallet.getAccounts();

    const budFee = calculateFee(1_000_000, GasPrice.fromString(GAS_PRICE));
    const bud = await client.execute(
      adminAddr,
      results.address,
      {
        bud: {
          parent: adminAddr,
          child: childAddr,
          child_weight: 100,
          mayo_pk: Array.from(v.pk),
        },
      },
      budFee,
      `MAYO bench bud ${v.name}`
    );
    console.log(`  Bud     gas: ${bud.gasUsed} | tx: ${bud.transactionHash}`);

    const verifyFee = calculateFee(10_000_000, GasPrice.fromString(GAS_PRICE));
    const verify = await client.execute(
      adminAddr,
      results.address,
      {
        verify_mayo_attestation: {
          addr: childAddr,
          message: Array.from(MSG),
          signature: Array.from(v.sig),
          public_key: Array.from(v.pk),
          variant: v.name,
        },
      },
      verifyFee,
      `MAYO bench verify ${v.name}`
    );
    console.log(`  Verify  gas: ${verify.gasUsed} | tx: ${verify.transactionHash}`);

    results.variants[v.name] = {
      nistLevel: v.nistLevel,
      pkBytes: v.pk.length,
      sigBytes: v.sig.length,
      child: childAddr,
      budGas: String(bud.gasUsed),
      budTx: bud.transactionHash,
      verifyGas: String(verify.gasUsed),
      verifyTx: verify.transactionHash,
    };
    save();
    console.log();
  }

  // ── Summary table ──
  console.log('=== RESULTS ===');
  console.log('| Variant | NIST | PK (B) | Sig (B) | Bud gas | Verify gas |');
  console.log('|---------|------|--------|---------|---------|------------|');
  for (const [name, r] of Object.entries(results.variants)) {
    console.log(
      `| ${name.toUpperCase()} | L${r.nistLevel} | ${r.pkBytes} | ${r.sigBytes} | ${r.budGas} | ${r.verifyGas} |`
    );
  }
  console.log('\nResults saved to', RESULTS_FILE);
}

main().catch(err => {
  console.error('FATAL:', err?.message || err);
  process.exit(1);
});
