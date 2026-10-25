// Pre-deploy gate for junoclaw.xyz.
// 1. Regenerates website/_headers from headers.template with sha256 hashes of
//    every inline <script> (Starlight emits a few), so the CSP never needs
//    'unsafe-inline' for scripts and never drifts out of date after a rebuild.
// 2. Fails on: broken internal links/assets, IPv4 literals in visible text,
//    secret-looking strings, missing required files.
// Usage: node tools/site/build-site.mjs   (from the junoclaw repo root)
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const site = resolve(here, '../../website');
const problems = [];

const walk = (d, out = []) => {
  for (const f of readdirSync(d)) {
    const p = join(d, f);
    statSync(p).isDirectory() ? walk(p, out) : out.push(p);
  }
  return out;
};
const files = walk(site);
const html = files.filter((f) => f.endsWith('.html'));

const required = ['index.html', '404.html', 'llms.txt', 'robots.txt', 'sitemap.xml',
  '.well-known/security.txt', 'assets/fonts/fonts.css', 'assets/logo-96.png', 'assets/banner.png'];
for (const r of required) if (!existsSync(join(site, r))) problems.push(`missing required file: ${r}`);

const hashes = new Set();
for (const f of html) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/<script(\s[^>]*)?>([\s\S]*?)<\/script>/g)) {
    const attrs = m[1] || '';
    if (/\bsrc=/.test(attrs) || /application\/(ld\+)?json/.test(attrs) || !m[2].trim()) continue;
    hashes.add(`'sha256-${createHash('sha256').update(m[2], 'utf8').digest('base64')}'`);
  }
  if (/\son[a-z]+\s*=\s*["']/i.test(src.replace(/<script[\s\S]*?<\/script>/g, '')))
    problems.push(`inline event handler (blocked by CSP): ${relative(site, f)}`);
}
const template = readFileSync(join(here, 'headers.template'), 'utf8');
writeFileSync(join(site, '_headers'), template.replace('{{SCRIPT_HASHES}}', [...hashes].sort().join(' ')));

const resolveLocal = (from, url) => {
  const clean = decodeURIComponent(url.split('#')[0].split('?')[0]);
  if (!clean) return true;
  const target = clean.startsWith('/') ? join(site, clean) : join(dirname(from), clean);
  if (existsSync(target) && statSync(target).isFile()) return true;
  return existsSync(join(target, 'index.html'));
};
for (const f of html) {
  const src = readFileSync(f, 'utf8');
  for (const m of src.matchAll(/\s(?:href|src)="([^"]+)"/g)) {
    const u = m[1];
    if (/^(https?:|mailto:|tel:|data:|#|javascript:)/.test(u) || u.startsWith('//')) continue;
    if (!resolveLocal(f, u)) problems.push(`broken link ${u} in ${relative(site, f)}`);
  }
  const text = src
    .replace(/<svg[\s\S]*?<\/svg>/g, '')
    .replace(/<script[\s\S]*?<\/script>/g, '')
    .replace(/<style[\s\S]*?<\/style>/g, '')
    .replace(/<[^>]+>/g, ' ');
  for (const m of text.matchAll(/\b(?:\d{1,3}\.){3}\d{1,3}\b/g))
    if (!['0.0.0.0', '127.0.0.1', '1.1.1.1'].includes(m[0])) problems.push(`IPv4 literal ${m[0]} in ${relative(site, f)}`);
}
const secretPatterns = [/-----BEGIN [A-Z ]*PRIVATE KEY-----/, /"dealer_seed"/, /\bbls_share_hex\b/, /\bmnemonic\s*[:=]\s*"/i];
for (const f of files.filter((x) => /\.(html|txt|js|json|xml|css)$/.test(x))) {
  const src = readFileSync(f, 'utf8');
  for (const re of secretPatterns) if (re.test(src)) problems.push(`secret-like pattern ${re} in ${relative(site, f)}`);
}

console.log(`site: ${html.length} html, ${files.length} files, ${hashes.size} inline-script hashes -> _headers`);
if (problems.length) {
  console.error(`FAIL (${problems.length}):\n  ` + problems.join('\n  '));
  process.exit(1);
}
console.log('OK: links, assets, IPv4 scan, secret scan, required files');
