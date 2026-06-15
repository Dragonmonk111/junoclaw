// MAYO precompile-vs-pure on-chain gas benchmark (junoclaw-bn254-1 devnet).
//
// Deploys BOTH jclaw-credential build flavours to the running MAYO-patched
// devnet and, for each MAYO variant, runs Bud -> VerifyMayoAttestation,
// recording gasUsed for each. Produces a side-by-side comparison table.
//
//   * pure       = devnet/jclaw_credential_pure.wasm        (in-Wasm verifier)
//   * precompile = devnet/jclaw_credential_precompile.wasm  (env.mayo_verify host fn)
//
// Signer: the in-container `admin` key. Export its hex privkey with
//   docker exec junoclaw-bn254-devnet junod keys export admin \
//       --unarmored-hex --unsafe --keyring-backend test --home /root/.juno
// and pass it as ADMIN_PRIVKEY (the wrapper script does this automatically).
//
// Results written to deploy/mayo-devnet-benchmark-results.json.

const { readFileSync, writeFileSync, existsSync } = require('fs');
const { join } = require('path');
const { DirectSecp256k1Wallet, DirectSecp256k1HdWallet } = require('@cosmjs/proto-signing');
const { SigningCosmWasmClient } = require('@cosmjs/cosmwasm-stargate');
const { GasPrice } = require('@cosmjs/stargate');
const { fromHex } = require('@cosmjs/encoding');

const REPO_ROOT = join(__dirname, '..');

const RPC_URL = process.env.RPC || 'http://localhost:26657';
const CHAIN_ID = process.env.CHAIN_ID || 'junoclaw-bn254-1';
const GAS_PRICE = process.env.GAS_PRICE || '0.075ujuno';
const DENOM = process.env.DENOM || 'ujuno';

const ADMIN_PRIVKEY = process.env.ADMIN_PRIVKEY;
if (!ADMIN_PRIVKEY) {
  console.error('Error: ADMIN_PRIVKEY (hex) not set. Run via deploy/run-mayo-devnet-benchmark.sh');
  process.exit(1);
}

const FLAVOURS = [
  { key: 'pure', wasm: join(REPO_ROOT, 'devnet', 'jclaw_credential_pure.wasm') },
  { key: 'precompile', wasm: join(REPO_ROOT, 'devnet', 'jclaw_credential_precompile.wasm') },
];

const RESULTS_FILE = join(__dirname, 'mayo-devnet-benchmark-results.json');

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
  { name: 'mayo2', nistLevel: 1, pk: extractLargeHex(vectorSource, 'PK_HEX'), sig: extractLargeHex(vectorSource, 'SIG_HEX') },
  { name: 'mayo3', nistLevel: 3, pk: extractLargeHex(vectorSource, 'MAYO3_PK_HEX'), sig: extractLargeHex(vectorSource, 'MAYO3_SIG_HEX') },
  { name: 'mayo5', nistLevel: 5, pk: extractLargeHex(vectorSource, 'MAYO5_PK_HEX'), sig: extractLargeHex(vectorSource, 'MAYO5_SIG_HEX') },
];

async function main() {
  console.log('=== MAYO Precompile vs Pure-Wasm Gas Benchmark (devnet) ===');
  console.log('Chain:', CHAIN_ID, '| RPC:', RPC_URL, '| gas price:', GAS_PRICE);

  const wallet = await DirectSecp256k1Wallet.fromKey(fromHex(ADMIN_PRIVKEY.replace(/^0x/, '')), 'juno');
  const [{ address: adminAddr }] = await wallet.getAccounts();
  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });
  const balance = await client.getBalance(adminAddr, DENOM);
  console.log('Admin:', adminAddr, '| Balance:', balance.amount, DENOM);
  for (const v of VARIANTS) {
    console.log(`  ${v.name}: pk=${v.pk.length} B, sig=${v.sig.length} B (NIST L${v.nistLevel})`);
  }
  console.log();

  const results = existsSync(RESULTS_FILE) ? JSON.parse(readFileSync(RESULTS_FILE, 'utf8')) : {};
  results.chain = CHAIN_ID;
  results.flavours = results.flavours || {};
  const save = () => writeFileSync(RESULTS_FILE, JSON.stringify(results, null, 2));

  for (const flavour of FLAVOURS) {
    const F = (results.flavours[flavour.key] = results.flavours[flavour.key] || {});

    if (!F.codeId) {
      console.log(`[${flavour.key}] Uploading ${flavour.wasm.split(/[\\/]/).pop()} ...`);
      const wasm = readFileSync(flavour.wasm);
      const up = await client.upload(adminAddr, wasm, 'auto', `jclaw-credential-${flavour.key}`);
      console.log(`  codeId: ${up.codeId} | storeGas: ${up.gasUsed} | tx: ${up.transactionHash}`);
      F.codeId = up.codeId;
      F.wasmBytes = wasm.length;
      F.storeGas = String(up.gasUsed);
      save();
    }

    if (!F.address) {
      console.log(`[${flavour.key}] Instantiating ...`);
      const inst = await client.instantiate(
        adminAddr,
        F.codeId,
        { admin: adminAddr },
        `jclaw-credential-${flavour.key}-bench`,
        'auto',
        { admin: adminAddr }
      );
      console.log(`  address: ${inst.contractAddress} | initGas: ${inst.gasUsed}`);
      F.address = inst.contractAddress;
      F.initGas = String(inst.gasUsed);
      save();
    }

    F.variants = F.variants || {};
    for (const v of VARIANTS) {
      if (F.variants[v.name]?.verifyGas) {
        console.log(`[${flavour.key}/${v.name}] cached verify gas: ${F.variants[v.name].verifyGas}`);
        continue;
      }
      console.log(`[${flavour.key}/${v.name}] Bud + VerifyMayoAttestation ...`);
      const childWallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' });
      const [{ address: childAddr }] = await childWallet.getAccounts();

      const bud = await client.execute(
        adminAddr, F.address,
        { bud: { parent: adminAddr, child: childAddr, child_weight: 100, mayo_pk: Array.from(v.pk) } },
        'auto', `bud ${flavour.key} ${v.name}`
      );
      console.log(`  Bud    gas: ${bud.gasUsed}`);

      const verify = await client.execute(
        adminAddr, F.address,
        { verify_mayo_attestation: {
            addr: childAddr,
            message: Array.from(MSG),
            signature: Array.from(v.sig),
            public_key: Array.from(v.pk),
            variant: v.name,
        } },
        'auto', `verify ${flavour.key} ${v.name}`
      );
      console.log(`  Verify gas: ${verify.gasUsed}`);

      F.variants[v.name] = {
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
    }
    console.log();
  }

  // ── Comparison table ──
  console.log('=== RESULTS: VerifyMayoAttestation gas (pure vs precompile) ===');
  console.log('| Variant | NIST | PK (B) | Sig (B) | Pure verify | Precompile verify | Speedup |');
  console.log('|---------|------|--------|---------|-------------|-------------------|---------|');
  const pure = results.flavours.pure?.variants || {};
  const prec = results.flavours.precompile?.variants || {};
  for (const v of VARIANTS) {
    const p = pure[v.name]; const c = prec[v.name];
    const pg = p ? Number(p.verifyGas) : null;
    const cg = c ? Number(c.verifyGas) : null;
    const speedup = pg && cg ? (pg / cg).toFixed(2) + '\u00d7' : '-';
    console.log(`| ${v.name.toUpperCase()} | L${v.nistLevel} | ${v.pk.length} | ${v.sig.length} | ${pg ?? '-'} | ${cg ?? '-'} | ${speedup} |`);
  }
  console.log('\nCode sizes: pure', results.flavours.pure?.wasmBytes, 'B | precompile', results.flavours.precompile?.wasmBytes, 'B');
  console.log('Results saved to', RESULTS_FILE);
}

main().catch(err => {
  console.error('FATAL:', err?.message || err);
  process.exit(1);
});
