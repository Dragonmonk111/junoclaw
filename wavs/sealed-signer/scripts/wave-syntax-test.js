// Quick test of WAVE syntax for hyphenated field names
const { spawnSync } = require('child_process');
const path = require('path');

const wasmtime = 'C:\\Users\\Taj\\.cargo\\bin\\wasmtime.exe';
const wasm = path.resolve(__dirname, '..', 'target', 'wasm32-wasip2', 'release', 'junoclaw_sealed_signer.wasm');

const blob = [1,2,3]; // dummy - will fail at decrypt, but we can see if parsing works

// Try different quoting styles for hyphenated fields
const attempts = [
  // Style 1: double-quoted field names (current approach)
  `sign-cosmos-execute-tx([${blob.join(',')}], {sender: "a", contract: "b", "exec-msg-json": "{}", "funds-denom": "", "funds-amount": 0, "gas-limit": 100, "fee-denom": "u", "fee-amount": 1, memo: "m", "chain-id": "c", "account-number": 1, sequence: 1})`,
  // Style 2: backtick-quoted field names
  `sign-cosmos-execute-tx([${blob.join(',')}], {sender: "a", contract: "b", \`exec-msg-json\`: "{}", \`funds-denom\`: "", \`funds-amount\`: 0, \`gas-limit\`: 100, \`fee-denom\`: "u", \`fee-amount\`: 1, memo: "m", \`chain-id\`: "c", \`account-number\`: 1, sequence: 1})`,
  // Style 3: %-quoted field names
  `sign-cosmos-execute-tx([${blob.join(',')}], {sender: "a", contract: "b", %"exec-msg-json": "{}", %"funds-denom": "", %"funds-amount": 0, %"gas-limit": 100, %"fee-denom": "u", %"fee-amount": 1, memo: "m", %"chain-id": "c", %"account-number": 1, sequence: 1})`,
];

for (let i = 0; i < attempts.length; i++) {
  console.log(`\n--- Attempt ${i + 1} ---`);
  console.log(`Invoke: ${attempts[i].slice(0, 120)}...`);
  const result = spawnSync(wasmtime, [
    'run',
    '--env', 'WAVS_ENV_SIGNER_PASSPHRASE=test',
    '--invoke', attempts[i],
    wasm,
  ], { encoding: 'utf8', maxBuffer: 10 * 1024 * 1024 });
  
  if (result.status === 0) {
    console.log(`SUCCESS: ${result.stdout.trim().slice(0, 200)}`);
  } else {
    // Extract just the parse error line
    const stderr = result.stderr;
    const parseErr = stderr.match(/unexpected token:.*|Failed to parse.*/);
    console.log(`FAILED: ${parseErr ? parseErr[0] : stderr.slice(0, 200)}`);
  }
}
