import { execFileSync } from 'child_process';
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { pathToFileURL } from 'url';

const MCP_DIST = join(process.cwd(), 'dist');
function distImport(...segments) { return import(pathToFileURL(join(MCP_DIST, ...segments)).href); }

const ws = (await distImport('wallet', 'store.js')).getDefaultWalletStore();
const mnemonic = await ws.exportMnemonicForExternalSigner('akash-jlens');

const keyringDir = mkdtempSync(join(tmpdir(), 'akash-mint-'));
const mnemonicFile = join(keyringDir, 'mnemonic.txt');
writeFileSync(mnemonicFile, mnemonic, { mode: 0o600 });

// Convert Windows path to WSL path
const wslPath = keyringDir.replace(/^([A-Za-z]):[\\\/](.*)/, (_, drive, rest) => 
  `/mnt/${drive.toLowerCase()}/${rest.replace(/\\/g, '/')}`);

try {
  // Import key into temp keyring
  execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'keys', 'add', 
    'junoclaw-mint', '--recover', '--keyring-backend', 'test', '--home', wslPath], 
    { input: mnemonic + '\n', encoding: 'utf-8', timeout: 30000 });

  // Get address
  const addrRaw = execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'keys', 'show', 
    'junoclaw-mint', '--keyring-backend', 'test', '--home', wslPath, '--output', 'json'], 
    { encoding: 'utf-8', timeout: 15000 });
  const address = JSON.parse(addrRaw).address;
  console.log('Wallet address:', address);

  // Check balance before mint
  const balBefore = execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'query', 'bank', 'balances', 
    address, '--node', 'https://akash-rpc.polkachu.com:443'], 
    { encoding: 'utf-8', timeout: 30000 });
  console.log('Balance before mint:', balBefore);

  // Mint 60 ACT by burning 60 AKT
  console.log('Minting 60 ACT (burning 60 AKT)...');
  const result = execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'tx', 'bme', 'mint-act', 
    '60000000uakt', '--from', 'junoclaw-mint', '--keyring-backend', 'test', '--home', wslPath,
    '--node', 'https://akash-rpc.polkachu.com:443', '--chain-id', 'akashnet-2',
    '--gas', 'auto', '--gas-adjustment', '1.5', '--fees', '80000uakt', '--output', 'json', '-y'], 
    { encoding: 'utf-8', timeout: 120000, maxBuffer: 10 * 1024 * 1024 });
  console.log('Mint result:', result);

  // Wait for tx to be included
  console.log('Waiting 10s for tx confirmation...');
  await new Promise(r => setTimeout(r, 10000));

  // Check balance after mint
  const balAfter = execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'query', 'bank', 'balances', 
    address, '--node', 'https://akash-rpc.polkachu.com:443'], 
    { encoding: 'utf-8', timeout: 30000 });
  console.log('Balance after mint:', balAfter);

} finally {
  try { unlinkSync(mnemonicFile); } catch {}
  try { rmSync(keyringDir, { recursive: true, force: true }); } catch {}
  console.log('Cleanup done.');
}
