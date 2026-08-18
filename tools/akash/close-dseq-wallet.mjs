import { execFileSync } from 'child_process';
import { mkdtempSync, writeFileSync, unlinkSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';
import { pathToFileURL } from 'url';

const DSEQ = process.argv[2];
if (!DSEQ) {
  console.error('Usage: node close-dseq-wallet.mjs <dseq>');
  process.exit(1);
}

const MCP_DIST = join(process.cwd(), 'dist');
function distImport(...segments) { return import(pathToFileURL(join(MCP_DIST, ...segments)).href); }

const ws = (await distImport('wallet', 'store.js')).getDefaultWalletStore();
const mnemonic = await ws.exportMnemonicForExternalSigner('akash-jlens');

const keyringDir = mkdtempSync(join(tmpdir(), 'akash-close-'));
const mnemonicFile = join(keyringDir, 'mnemonic.txt');
writeFileSync(mnemonicFile, mnemonic, { mode: 0o600 });

const wslPath = keyringDir.replace(/^([A-Za-z]):[\\\/](.*)/, (_, drive, rest) => 
  `/mnt/${drive.toLowerCase()}/${rest.replace(/\\/g, '/')}`);

try {
  execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'keys', 'add', 
    'junoclaw-close', '--recover', '--keyring-backend', 'test', '--home', wslPath], 
    { input: mnemonic + '\n', encoding: 'utf-8', timeout: 30000 });

  console.log(`Closing deployment dseq=${DSEQ}...`);
  const result = execFileSync('wsl.exe', ['-d', 'Ubuntu-24.04', '--', 'akash', 'tx', 'deployment', 'close',
    '--dseq', DSEQ, '--from', 'junoclaw-close', '--keyring-backend', 'test', '--home', wslPath,
    '--node', 'https://akash-rpc.polkachu.com:443', '--chain-id', 'akashnet-2',
    '--gas', 'auto', '--gas-adjustment', '1.5', '--fees', '80000uakt', '--output', 'json', '-y'], 
    { encoding: 'utf-8', timeout: 120000, maxBuffer: 10 * 1024 * 1024 });
  const parsed = JSON.parse(result);
  console.log(`Closed. TX hash: ${parsed.txhash}, code: ${parsed.code}`);
} finally {
  try { unlinkSync(mnemonicFile); } catch {}
  try { rmSync(keyringDir, { recursive: true, force: true }); } catch {}
  console.log('Cleanup done.');
}
