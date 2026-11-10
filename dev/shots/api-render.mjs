// The report API's pages (drawn by the server's V8 engine, text measured by the core) against the
// same report drawn in Chrome as the app draws it (text measured by the browser): every text item
// on every page, the ones whose words differ (a line wrapped or cut elsewhere) and how far apart
// the rest are.
//
//   cargo test --lib --no-default-features --features render -- report_api::tests::draws
//   node dev/shots/api-render.mjs [<files>]     # <files>: $TMPDIR/quire_api_report (.json, .svg)
import http from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, normalize, extname } from 'node:path';

const base = process.argv[3] || join(tmpdir(), 'quire_api_report');
const root = new URL('../..', import.meta.url).pathname;
const state = readFileSync(base + '.json', 'utf8');
const engine = readFileSync(base + '.svg', 'utf8').split('\n<svg').map((s, i) => (i ? '<svg' + s : s));
const TYPES = { '.js': 'text/javascript', '.css': 'text/css', '.ttf': 'font/ttf', '.otf': 'font/otf', '.html': 'text/html', '.json': 'application/json' };
const PAGE = `<!doctype html><meta charset="utf-8"><link rel="stylesheet" href="/ui/styles.css"><link rel="stylesheet" href="/ui/fonts/private/fonts.css">
<script type="module">import { pageSvg } from '/ui/render.js'; window.pageSvg = pageSvg;</script>`;
const server = http.createServer((q, r) => {
  const url = decodeURIComponent(q.url.split('?')[0]);
  if (url === '/') { r.writeHead(200, { 'content-type': 'text/html' }); return r.end(PAGE); }
  if (url === '/state.json') { r.writeHead(200, { 'content-type': 'application/json' }); return r.end(state); }
  const f = normalize(join(root, url));
  if (!f.startsWith(root) || !existsSync(f)) { r.writeHead(404); return r.end(); }
  r.writeHead(200, { 'content-type': TYPES[extname(f)] || 'application/octet-stream' });
  r.end(readFileSync(f));
}).listen(0);
const port = server.address().port;

// cdp.mjs reads its output folder from argv[2]
process.argv[2] = tmpdir();
const { send, ev, sleep, chrome } = await import('./cdp.mjs');
await send('Page.enable');
await send('Page.navigate', { url: `http://127.0.0.1:${port}/` });
for (let i = 0; i < 50 && (await ev('typeof window.pageSvg')) !== 'function'; i++) await sleep(100);
const browser = await ev(`(async () => {
  const st = await (await fetch('/state.json')).json();
  const draw = () => st.doc.pages.map((_, i) => i).filter((i) => !st.pages[i]?.hidden).map((i) => pageSvg(st, i, { exportMode: true }));
  // every face the pages use, loaded before measuring
  const faces = new Set();
  for (const s of draw()) for (const m of s.matchAll(/font-family="([^"]*)"[^>]*?font-size="[^"]*"(?: font-weight="(\\d+)")?(?:[^>]*font-style="(italic)")?/g)) faces.add(\`\${m[3] || ''} \${m[2] || 400} 12px \${m[1].replace(/&#39;/g, "'")}\`);
  await Promise.all([...faces].map((f) => document.fonts.load(f).catch(() => {})));
  await document.fonts.ready;
  return draw();
})()`);
chrome.kill();
server.close();
if (!Array.isArray(browser)) { console.log('Chrome failed:', browser); process.exit(1); }

const items = (svg) => [...svg.matchAll(/<text([^>]*)>(.*?)<\/text>/g)].map((m) => ({
  x: +(m[1].match(/ x="([-\d.]+)"/)?.[1] ?? 0), y: +(m[1].match(/ y="([-\d.]+)"/)?.[1] ?? 0),
  text: m[2].replace(/<[^>]+>/g, '').replace(/&#39;/g, "'").replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>'),
}));
console.log(`pages: engine ${engine.length}, Chrome ${browser.length}`);
let total = 0, same = 0, worded = [], moved = [];
for (let p = 0; p < Math.min(engine.length, browser.length); p++) {
  const a = items(engine[p]), b = items(browser[p]);
  if (a.length !== b.length) worded.push(`page ${p + 1}: ${a.length} text items here, ${b.length} in Chrome`);
  for (let i = 0; i < Math.min(a.length, b.length); i++) {
    total++;
    if (a[i].text !== b[i].text) { worded.push(`page ${p + 1}: “${a[i].text}” — Chrome “${b[i].text}”`); continue; }
    const d = Math.hypot(a[i].x - b[i].x, a[i].y - b[i].y);
    if (d < 0.05) same++;
    else moved.push({ p: p + 1, d, text: a[i].text });
  }
}
moved.sort((a, b) => b.d - a.d);
const within = (n) => moved.filter((m) => m.d <= n).length;
console.log(`text items: ${total}; same words ${total - worded.length}, same place ${same}; moved ≤0.5pt ${within(0.5)}, ≤2pt ${within(2)}, more ${moved.length - within(2)}`);
if (worded.length) console.log(`\nwords differ (${worded.length}):\n  ` + worded.slice(0, 30).join('\n  '));
if (moved.length) console.log('\nmoved most:\n  ' + moved.slice(0, 12).map((m) => `page ${m.p}: ${m.d.toFixed(2)}pt “${m.text}”`).join('\n  '));
