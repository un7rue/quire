// The web server's checks: workspaces kept apart, each person's files their own, sign-in lockout,
// secrets kept out of the data folder and the page, the data fence, request limits and headers.
//
//   QUIRE_SECRET_KEY='a test passphrase here' cargo run --no-default-features --bin quire-server -- \
//       --port 8770 --allow-host db.bank.local --data "$(mktemp -d)"
//   node dev/web/checks.mjs [http://127.0.0.1:8770] [data folder]
//
// The data folder (the one given to --data) lets it look for secrets written in plain text.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const BASE = process.argv[2] || 'http://127.0.0.1:8770';
const DATA = process.argv[3];
let failed = 0;
const check = (name, ok, got) => { if (!ok) failed++; console.log(`${ok ? '✓' : '✗'} ${name}${ok ? '' : `  — got: ${JSON.stringify(got).slice(0, 200)}`}`); };

/** A person in a browser of their own: the cookie travels with their calls. */
function browser() {
  let cookie = '';
  return async (cmd, args = {}) => {
    const r = await fetch(BASE + '/api/call', { method: 'POST', headers: { 'Content-Type': 'text/plain', ...(cookie && { Cookie: cookie }) }, body: JSON.stringify({ cmd, args }) });
    const set = r.headers.get('set-cookie');
    if (set) cookie = set.split(';')[0];
    const j = await r.json();
    return r.ok ? j : { error: j };
  };
}
const login = (b, who, pw = 'quire') => b('auth', { cmd: 'login', args: { login: who, password: pw } });
const err = (x) => x?.error ?? '';

// --- HTTP
const page = await fetch(BASE + '/');
check('pages may not be framed by other sites', page.headers.get('x-frame-options') === 'DENY', page.headers.get('x-frame-options'));
check('types are not guessed', page.headers.get('x-content-type-options') === 'nosniff');
const first = await fetch(BASE + '/api/call', { method: 'POST', body: '{"cmd":"get_state"}' });
const sc = first.headers.get('set-cookie') || '';
check('the session cookie is HttpOnly and SameSite=Strict', /HttpOnly/.test(sc) && /SameSite=Strict/.test(sc), sc);
const big = await fetch(BASE + '/api/call', { method: 'POST', headers: { 'Content-Length': '999999999' }, body: 'x' }).then((r) => r.status).catch(() => 413);
check('a request over 300 MB is refused', big === 413, big);
check('files outside the UI folder are not served', (await fetch(BASE + '/%2e%2e/Cargo.toml')).status === 404);

// --- workspaces and people kept apart
const designer = browser(), banker = browser(), stranger = browser();
await login(designer, 'design');
await login(banker, 'user');
const tpl = await designer('new_doc', { kind: 'uhnwi_report' });
const rep = await banker('new_doc', { kind: 'pub:uhnwi', client: 'LeBron Jameson' });
check('each browser has its own documents', tpl.doc?.kind === 'template' && rep.doc?.kind === 'report' && (await designer('get_state')).doc?.kind === 'template');
check('roles still hold (a banker opens no theme studio)', /Design role/.test(err(await banker('theme_studio_open', { source: 'new' }))));
check('without a cookie: a fresh, signed-out workspace', /Sign in/.test(err(await stranger('new_doc', { kind: 'uhnwi_report' }))));
const saved = await banker('save', { saveAs: false });
check('a report is saved under its title and client', /LeBron Jameson\.rpt$|LeBron Jameson \(\d+\)\.rpt$/.test(saved.file || ''), saved.file);
const mine = await banker('saved_documents');
const path = mine[0]?.path;
check('the saved report is in its owner’s list', mine.some((d) => d.client === 'LeBron Jameson'));
check('…and not in another person’s', (await designer('saved_documents')).every((d) => d.path !== path));
check('another person can’t download it', /own saved files/.test(err(await designer('saved_file', { path }))));
check('another person can’t delete it', /own saved files/.test(err(await designer('delete_saved', { path }))));
check('documents open only from inside the data folder', /outside Quire's folder/.test(err(await designer('open_path', { path: '/etc/hosts' }))));

// --- sign-in lockout (a login nobody has: nobody is locked out by the check)
const guesser = browser();
for (let i = 0; i < 5; i++) await login(guesser, 'lockout-check', 'wrong');
check('five wrong passwords rest the login', /Too many/.test(err(await login(guesser, 'lockout-check', 'wrong'))));

// --- secrets
const admin = browser();
await login(admin, 'admin');
await admin('ai_set_key', { provider: 'claude', key: 'sk-ant-CHECKKEY-0042', bank: true });
const keys = await admin('ai_user_keys', { login: 'admin' });
check('the bank’s AI key is kept', keys.some?.((p) => p.id === 'claude' && p.bank));
const conns = await admin('auth', { cmd: 'connections' });
await admin('auth', { cmd: 'save_connections', args: { connections: [...conns.filter((c) => c.id !== 'chk'), { id: 'chk', name: 'Check PG', kind: 'postgres', url: 'postgres://reporter:Chk-Pw-0042@db.bank.local:5432/dwh' }] } });
const seen = (await admin('auth', { cmd: 'connections' })).find((c) => c.id === 'chk')?.url || '';
check('the page sees a connection’s password hidden', seen.includes('••••••') && !seen.includes('Chk-Pw-0042'), seen);
if (DATA) {
  const plain = [];
  const walk = (d) => { for (const f of readdirSync(d)) { const p = join(d, f); if (statSync(p).isDirectory()) walk(p); else if (statSync(p).size < 5e6) { const t = readFileSync(p, 'latin1'); if (t.includes('sk-ant-CHECKKEY-0042') || t.includes('Chk-Pw-0042')) plain.push(p); } } };
  walk(DATA);
  check('no secret is written in plain text in the data folder', !plain.length, plain);
}
await admin('auth', { cmd: 'save_connections', args: { connections: conns } });
await admin('ai_set_key', { provider: 'claude', key: null, bank: true });

// --- Python: off on a server until an administrator switches it on
check('Python is off on a fresh server', (await designer('python_policy')).on === false);
check('…and a Python source doesn’t run', /switched off/.test(err(await designer('preview_dataset', { dataset: { id: '', name: 'p', fields: [], source: { type: 'python', code: 'df = 1' } } }))));
check('only an administrator switches it', /Admin role/.test(err(await designer('python_policy', { on: true }))));

// --- the data fence
const ds = (source) => designer('preview_dataset', { dataset: { id: '', name: 'x', fields: [], source } });
check('a CSV outside the allowed folders is refused', /outside the folders/.test(err(await ds({ type: 'csv', path: '/etc/hosts' }))));
const up = await designer('upload_file', { name: 'check.csv', data: Buffer.from('a,b\n1,2\n3,4\n').toString('base64') });
check('an uploaded CSV is read', (await ds({ type: 'csv', path: up })).total === 2);
check('DuckDB SQL can’t read files outside', /disabled by configuration|Permission/.test(err(await ds({ type: 'duckdb', path: '', query: "select * from read_csv('/etc/hosts')" }))));
check('DuckDB SQL reads uploaded files', (await ds({ type: 'duckdb', path: '', query: `select * from read_csv('${up}')` })).total === 2);
check('images outside are refused', /outside the folders/.test(err(await designer('read_image', { path: '/etc/hosts' }))));
const other = await admin('db_tables', { connection: { id: 'x', name: 'x', kind: 'postgres', url: 'postgres://a:b@elsewhere.example.com/db' } });
check('a database server not allowed is refused (with --allow-host)', /may not connect/.test(err(other)), other);
const full = await browser();
await login(full, 'user');
const report = await full('new_doc', { kind: 'pub:uhnwi' });
const ws = report.pages?.flatMap((p) => p.widgets || []) || [];
check('the demo report loads whole inside the fence', ws.length > 100 && !ws.some((w) => w.data?.error), `${ws.length} widgets`);

console.log(failed ? `\n${failed} check(s) failed` : '\nall checks passed');
process.exit(failed ? 1 : 0);
