// Local preview of website/ that applies the "/*" block of website/_headers
// (incl. the CSP) and serves 404.html, so CSP breakage shows up before deploy.
// Usage: node tools/site/preview-server.mjs [port]
import { createReadStream, existsSync, readFileSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, join, normalize, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../../website');
const port = Number(process.argv[2] || 8787);
const types = { '.html': 'text/html; charset=utf-8', '.css': 'text/css', '.js': 'text/javascript',
  '.png': 'image/png', '.webp': 'image/webp', '.svg': 'image/svg+xml', '.woff2': 'font/woff2',
  '.txt': 'text/plain; charset=utf-8', '.xml': 'application/xml', '.json': 'application/json',
  '.wasm': 'application/wasm' };

const globalHeaders = {};
const block = readFileSync(join(root, '_headers'), 'utf8').split(/\r?\n\r?\n/)[0].split(/\r?\n/).slice(1);
for (const line of block) {
  const i = line.indexOf(':');
  if (i > 0) globalHeaders[line.slice(0, i).trim()] = line.slice(i + 1).trim();
}
delete globalHeaders['Strict-Transport-Security'];
globalHeaders['Content-Security-Policy'] = globalHeaders['Content-Security-Policy'].replace('; upgrade-insecure-requests', '');

createServer((req, res) => {
  let p = normalize(join(root, decodeURIComponent(req.url.split('?')[0])));
  if (!p.startsWith(root)) { res.writeHead(403); return res.end(); }
  if (existsSync(p) && statSync(p).isDirectory()) p = join(p, 'index.html');
  let status = 200;
  if (!existsSync(p)) { status = 404; p = join(root, '404.html'); }
  res.writeHead(status, { ...globalHeaders, 'Content-Type': types[extname(p)] || 'application/octet-stream' });
  createReadStream(p).pipe(res);
}).listen(port, '127.0.0.1', () => console.log(`preview (with _headers CSP) on http://127.0.0.1:${port}`));
