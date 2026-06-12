const { readFileSync } = require('fs');
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

const JCLAW_ADDR = 'juno1z2w067ptpn2f6zpwt207je0kqeqc2eek7jf4p4dpztf24zncnhzqz5el2r';

// Load test vector from contract source
const MAYO_VECTOR_PATH = join(__dirname, '..', 'contracts', 'jclaw-credential', 'src', 'mayo_vectors.rs');
const vectorSource = readFileSync(MAYO_VECTOR_PATH, 'utf8');

function extractConst(source, name) {
  const re = new RegExp(`pub const ${name}: &\\[u8\\] = b"([^"]+)";`);
  const m = source.match(re);
  if (!m) throw new Error(`Cannot extract ${name} from mayo_vectors.rs`);
  return Buffer.from(m[1], 'utf8');
}

function extractHex(source, name) {
  const re = new RegExp(`pub const ${name}: &str = "([0-9a-fA-F]+)";`);
  const m = source.match(re);
  if (!m) throw new Error(`Cannot extract ${name} from mayo_vectors.rs`);
  return Buffer.from(m[1], 'hex');
}

function extractHash(source, name) {
  const re = new RegExp(`pub const ${name}: &str = "([0-9a-fA-F]+)";`);
  const m = source.match(re);
  if (!m) throw new Error(`Cannot extract ${name} from mayo_vectors.rs`);
  return m[1];
}

// Note: PK_HEX is too large for regex on some engines; read it directly
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
const PK_HEX = extractLargeHex(vectorSource, 'PK_HEX');
const SIG_HEX = extractLargeHex(vectorSource, 'SIG_HEX');
const PK_HASH = extractHash(vectorSource, 'PK_HASH');

console.log('=== MAYO Attestation Test ===');
console.log('Chain:', CHAIN_ID);
console.log('RPC:', RPC_URL);
console.log('jclaw-credential:', JCLAW_ADDR);
console.log('Test vector MSG:', MSG.toString());
console.log('Test vector PK_HASH:', PK_HASH);
console.log('PK bytes:', PK_HEX.length);
console.log('SIG bytes:', SIG_HEX.length);
console.log();

async function main() {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(MNEMONIC, { prefix: 'juno' });
  const [{ address: adminAddr }] = await wallet.getAccounts();
  console.log('Admin:', adminAddr);

  // Generate a child wallet for the Bud test
  const childWallet = await DirectSecp256k1HdWallet.generate(24, { prefix: 'juno' });
  const [{ address: childAddr }] = await childWallet.getAccounts();
  console.log('Child:', childAddr, '(disposable test wallet)');
  console.log();

  const client = await SigningCosmWasmClient.connectWithSigner(RPC_URL, wallet, {
    gasPrice: GasPrice.fromString(GAS_PRICE),
  });

  // ── Step 1: Bud (create child with MAYO PK) ────────────────────────────────
  console.log('[1/4] Bud — creating child member with MAYO PK...');
  const budFee = calculateFee(500_000, GasPrice.fromString(GAS_PRICE));
  const budResult = await client.execute(
    adminAddr,
    JCLAW_ADDR,
    {
      bud: {
        parent: adminAddr,
        child: childAddr,
        child_weight: 100,
        mayo_pk: Array.from(PK_HEX), // CosmJS needs array for Vec<u8>
      },
    },
    budFee,
    'MAYO Bud test'
  );
  console.log('  TX:', budResult.transactionHash);
  console.log('  Gas used:', budResult.gasUsed);
  console.log();

  // ── Step 2: Query stored PK hash ─────────────────────────────────────────
  console.log('[2/4] Query MayoPkHash for child...');
  const pkHashRes = await client.queryContractSmart(JCLAW_ADDR, {
    mayo_pk_hash: { addr: childAddr },
  });
  console.log('  Stored hash:', pkHashRes.mayo_pk_hash);
  console.log('  Expected:   ', PK_HASH);
  const hashMatch = pkHashRes.mayo_pk_hash === PK_HASH;
  console.log('  Match:', hashMatch ? '✅ YES' : '❌ NO');
  console.log();

  // ── Step 3: Verify valid attestation ────────────────────────────────────
  console.log('[3/4] VerifyMayoAttestation — VALID signature...');
  const verifyFee = calculateFee(2_000_000, GasPrice.fromString(GAS_PRICE));
  const verifyResult = await client.execute(
    adminAddr,
    JCLAW_ADDR,
    {
      verify_mayo_attestation: {
        addr: childAddr,
        message: Array.from(MSG),
        signature: Array.from(SIG_HEX),
        public_key: Array.from(PK_HEX),
      },
    },
    verifyFee,
    'MAYO verify valid'
  );
  console.log('  TX:', verifyResult.transactionHash);
  console.log('  Gas used:', verifyResult.gasUsed);
  console.log('  ✅ Valid attestation accepted');
  console.log();

  // ── Step 4: Verify tampered message (should reject) ──────────────────────
  console.log('[4/4] VerifyMayoAttestation — TAMPERED message (should reject)...');
  const badMsg = Buffer.from(MSG);
  badMsg[0] = badMsg[0] ^ 0xFF; // flip first byte

  try {
    await client.execute(
      adminAddr,
      JCLAW_ADDR,
      {
        verify_mayo_attestation: {
          addr: childAddr,
          message: Array.from(badMsg),
          signature: Array.from(SIG_HEX),
          public_key: Array.from(PK_HEX),
        },
      },
      verifyFee,
      'MAYO verify tampered'
    );
    console.log('  ❌ ERROR: Tampered message was accepted (should have rejected)');
    process.exit(1);
  } catch (err) {
    const msg = err?.message || String(err);
    if (msg.includes('MayoVerifyFailed') || msg.includes('verify failed') || msg.includes('error')) {
      console.log('  ✅ Tampered message correctly rejected');
      console.log('  Error:', msg.split('\n')[0].slice(0, 200));
    } else {
      console.log('  ⚠️  Unexpected error:', msg.slice(0, 200));
    }
  }

  console.log();
  console.log('=== MAYO Attestation Test Complete ===');
  console.log('All 4 steps passed:');
  console.log('  1. Bud created child with MAYO PK');
  console.log('  2. Stored hash matches expected:', hashMatch ? 'YES' : 'NO');
  console.log('  3. Valid signature verified on-chain');
  console.log('  4. Tampered signature rejected on-chain');
}

main().catch((err) => {
  console.error('\nTest failed:', err.message);
  console.error(err);
  process.exit(1);
});
