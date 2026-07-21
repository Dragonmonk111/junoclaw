const { spawnSync } = require('child_process');
const crypto = require('crypto');
const path = require('path');

const wasmtime = process.env.WASMTIME || 'C:\\Users\\Taj\\.cargo\\bin\\wasmtime.exe';
const wasm = process.env.WASM || path.resolve(__dirname, '..', 'target', 'wasm32-wasip2', 'release', 'junoclaw_sealed_signer.wasm');

const passphrase = 'secret';
const message = Buffer.from('hello');

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
  // WAVE ok({...}) wrapper
  const m = out.match(/^ok\(\{(.+)\}\)$/s);
  if (!m) throw new Error(`unexpected wasmtime output: ${out}`);
  const record = {};
  // Match fields: name: value, where value is string "..." or list [...]
  const fieldRe = /(\w[\w-]*):\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\])/g;
  let match;
  while ((match = fieldRe.exec(m[1])) !== null) {
    const key = match[1];
    let raw = match[2];
    if (raw.startsWith('"')) {
      record[key] = JSON.parse(raw);
    } else if (raw.startsWith('[')) {
      record[key] = raw === '[]' ? [] : raw.slice(1, -1).split(',').map(s => parseInt(s.trim(), 10));
    } else {
      throw new Error(`unhandled field value: ${raw}`);
    }
  }
  return record;
}

function rawSigToDer(raw) {
  if (raw.length !== 64) throw new Error('raw signature must be 64 bytes');
  const r = raw.slice(0, 32);
  const s = raw.slice(32, 64);
  const encodeInt = (b) => {
    let i = 0;
    while (i < b.length && b[i] === 0) i++;
    const pos = b.slice(i);
    const needsZero = pos.length === 0 || (pos[0] & 0x80) !== 0;
    const body = needsZero ? Buffer.concat([Buffer.from([0]), pos]) : pos;
    return Buffer.concat([Buffer.from([0x02, body.length]), body]);
  };
  const rDer = encodeInt(r);
  const sDer = encodeInt(s);
  const seq = Buffer.concat([rDer, sDer]);
  return Buffer.concat([Buffer.from([0x30, seq.length]), seq]);
}

function spkiDerFromCompressedPoint(point) {
  if (point.length !== 33) throw new Error('compressed public key must be 33 bytes');
  // SPKI for secp256k1 (compressed point):
  // SEQUENCE { AlgorithmIdentifier { OID ecPublicKey, OID secp256k1 },
  //            BIT STRING { 0 unused bits, ECPoint }
  const prefix = Buffer.from([
    0x30, 0x36, // SEQUENCE length 54
    0x30, 0x10, // AlgorithmIdentifier length 16
    0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, // ecPublicKey
    0x06, 0x05, 0x2b, 0x81, 0x04, 0x00, 0x0a, // secp256k1
    0x03, 0x22, 0x00, // BIT STRING length 34, 0 unused bits
  ]);
  return Buffer.concat([prefix, point]);
}

function main() {
  console.log('Invoking generate-key (run 1)...');
  const genOut1 = runWasmtime('generate-key()');
  console.log(genOut1);
  const keyInfo1 = parseRecord(genOut1);
  console.log('address:', keyInfo1.address);
  console.log('pubkey:', keyInfo1.pubkey);
  console.log('sealed-blob length:', keyInfo1['sealed-blob'].length);

  console.log('Invoking generate-key (run 2)...');
  const genOut2 = runWasmtime('generate-key()');
  const keyInfo2 = parseRecord(genOut2);
  console.log('address:', keyInfo2.address);

  if (keyInfo1.address === keyInfo2.address) {
    throw new Error('generate-key must be non-deterministic when seeded by wasi:random');
  }
  console.log('Non-deterministic addresses confirmed');

  const messageList = Array.from(message).join(',');
  const blobList = keyInfo1['sealed-blob'].join(',');
  console.log('Invoking sign...');
  const signOut = runWasmtime(`sign([${messageList}], [${blobList}])`);
  console.log(signOut);
  const signInfo = parseRecord(signOut);
  console.log('signature:', signInfo.signature);

  if (signInfo.address !== keyInfo1.address) {
    throw new Error(`address mismatch: ${signInfo.address} !== ${keyInfo1.address}`);
  }

  const rawSig = Buffer.from(signInfo.signature, 'hex');
  const derSig = rawSigToDer(rawSig);
  const pubkeyBuf = Buffer.from(keyInfo1.pubkey, 'hex');
  const publicKey = crypto.createPublicKey({
    key: spkiDerFromCompressedPoint(pubkeyBuf),
    format: 'der',
    type: 'spki',
  });

  const ok = crypto.createVerify('SHA256').update(message).verify(publicKey, derSig);
  if (!ok) {
    throw new Error('signature verification failed');
  }
  console.log('Signature verified OK');
}

main();
