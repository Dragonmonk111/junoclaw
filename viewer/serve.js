const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = 3847;
const ROOT = path.resolve(__dirname, '..');

// Read markdown files
const article   = fs.readFileSync(path.join(ROOT, 'MEDIUM_STANDALONE_ARTICLE.md'), 'utf-8');
const brief     = fs.readFileSync(path.join(ROOT, 'MEDIUM_UPDATE_BRIEF.md'), 'utf-8');
const tweets    = fs.readFileSync(path.join(ROOT, 'UPDATE_TWEET_REPLY.md'), 'utf-8');
const midjourney = fs.readFileSync(path.join(ROOT, 'MIDJOURNEY_PROMPT.md'), 'utf-8');

// Read HTML template and inject content
let html = fs.readFileSync(path.join(__dirname, 'index.html'), 'utf-8');

function escapeForJS(str) {
  return str
    .replace(/\\/g, '\\\\')
    .replace(/`/g, '\\`')
    .replace(/\$/g, '\\$');
}

html = html.replace('`ARTICLE_PLACEHOLDER`',    '`' + escapeForJS(article) + '`');
html = html.replace('`BRIEF_PLACEHOLDER`',      '`' + escapeForJS(brief) + '`');
html = html.replace('`TWEETS_PLACEHOLDER`',     '`' + escapeForJS(tweets) + '`');
html = html.replace('`MIDJOURNEY_PLACEHOLDER`', '`' + escapeForJS(midjourney) + '`');

const server = http.createServer((req, res) => {
  res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
  res.end(html);
});

server.listen(PORT, () => {
  console.log(`JunoClaw Content Viewer running at http://localhost:${PORT}`);
});
