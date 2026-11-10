// Load check for the report API: quarter-end batches and single requests at the same time, from
// one API account. Prints how many reports a minute the server makes, how long single requests
// wait behind the batches, failures, and (with the server's pid) its memory.
//
//   QUIRE_SECRET_KEY='a test passphrase here' cargo run --release --no-default-features --features render \
//       --bin quire-server -- --port 8773 --data "$(mktemp -d)" --api-workers 4
//   node dev/load/api.mjs [--url http://127.0.0.1:8773] [--batches 3] [--singles 20] [--pid <server pid>]
//
// A batch asks for every client the bank has (41 in the demo database). It adds an API account
// “Load test” and deletes it at the end.
import { execFileSync } from 'node:child_process';

const arg = (name, dflt) => { const i = process.argv.indexOf(`--${name}`); return i > 0 ? process.argv[i + 1] : dflt; };
const BASE = arg('url', 'http://127.0.0.1:8773');
const BATCHES = +arg('batches', 3);
const SINGLES = +arg('singles', 20);
const PID = arg('pid', '');

let cookie = '';
const admin = async (cmd, args = {}) => {
  const r = await fetch(BASE + '/api/call', { method: 'POST', headers: { 'Content-Type': 'text/plain', ...(cookie && { Cookie: cookie }) }, body: JSON.stringify({ cmd, args }) });
  const set = r.headers.get('set-cookie');
  if (set) cookie = set.split(';')[0];
  const j = await r.json();
  if (!r.ok) throw new Error(`${cmd}: ${JSON.stringify(j)}`);
  return j;
};
await admin('auth', { cmd: 'login', args: { login: 'admin', password: 'quire' } });
await admin('auth', { cmd: 'api_delete_client', args: { id: 'load-test' } }).catch(() => {});
const KEY = (await admin('auth', { cmd: 'api_save_client', args: { client: { name: 'Load test', active: true, access: { all: true, coverage: [], sources: [], clients: [] } } } })).key;
const clients = (await admin('auth', { cmd: 'options' })).clients.map((c) => c.id);

const H = { Authorization: `Bearer ${KEY}`, 'Content-Type': 'application/json' };
const post = (body) => fetch(BASE + '/api/v1/reports', { method: 'POST', headers: H, body: JSON.stringify(body) }).then(async (r) => ({ status: r.status, json: await r.json() }));
// a job's status; past the account's limit, wait as told and ask again
const job = async (id) => {
  for (;;) {
    const r = await fetch(`${BASE}/api/v1/jobs/${id}`, { headers: H });
    if (r.status !== 429) return r.json();
    limited++;
    await new Promise((res) => setTimeout(res, 1000 * +(r.headers.get('retry-after') || 1)));
  }
};
let limited = 0;

// the server's memory, sampled while it works
const rss = [];
const sample = () => { if (!PID) return; try { rss.push(+execFileSync('ps', ['-o', 'rss=', '-p', PID]).toString().trim() / 1024); } catch { /* gone */ } };
const sampler = setInterval(sample, 500);
sample();
const idle = rss[0];

const t0 = performance.now();
const periods = [{ preset: 'last_quarter' }, { preset: 'ytd' }, { start: '2026-01-01', end: '2026-06-30' }];
const batches = await Promise.all(Array.from({ length: BATCHES }, (_, i) => post({ clients, period: periods[i % periods.length] })));
const accepted = performance.now() - t0;
// single requests arrive one by one while the batches run, a second apart; each is watched (every
// half second) from when it is sent, as a CRM waiting on its click would
const singles = [];
const watching = [];
for (let i = 0; i < SINGLES; i++) {
  const at = performance.now();
  const r = await post({ client_id: clients[(i * 7) % clients.length], period: { preset: 'last_month' } });
  const one = { at, id: r.json.job_id, status: r.status, done: null };
  singles.push(one);
  watching.push((async () => {
    while (one.id) {
      const j = await job(one.id);
      if (!['queued', 'running'].includes(j.status)) { one.done = performance.now() - one.at; return; }
      await new Promise((res) => setTimeout(res, 500));
    }
  })());
  await new Promise((res) => setTimeout(res, 1000));
}
await Promise.all(watching);
// wait for everything, noting when each single request was done
const ids = [...batches.map((b) => b.json.job_id), ...singles.map((s) => s.id)].filter(Boolean);
const finished = new Map();
while (finished.size < ids.length) {
  for (const id of ids.filter((x) => !finished.has(x))) {
    const j = await job(id);
    if (!['queued', 'running'].includes(j.status)) {
      finished.set(id, { j, at: performance.now() });
    }
  }
  // as a calling system would: every 2 s
  await new Promise((res) => setTimeout(res, 2000));
}
const total = (performance.now() - t0) / 1000;
clearInterval(sampler);
sample();

const all = [...finished.values()].map((f) => f.j);
const reports = all.reduce((n, j) => n + j.reports, 0);
const failed = all.reduce((n, j) => n + j.failed, 0);
const pages = all.flatMap((j) => j.items).reduce((n, i) => n + (i.pages || 0), 0);
const errors = [...new Set(all.flatMap((j) => j.items).filter((i) => i.error).map((i) => i.error.message))];
const waits = singles.map((s) => s.done / 1000).filter((x) => x != null).sort((a, b) => a - b);
const pct = (a, p) => a[Math.min(a.length - 1, Math.floor(p * a.length))];

console.log(`${BATCHES} batch(es) of ${clients.length} clients + ${SINGLES} single requests: ${reports} reports, ${pages} pages, ${failed} failed`);
console.log(`requests accepted in ${(accepted / 1000).toFixed(2)} s; all done in ${total.toFixed(1)} s → ${(reports / total * 60).toFixed(0)} reports a minute (${(total / reports).toFixed(2)} s each)`);
if (waits.length) console.log(`single requests behind the batches: median ${pct(waits, 0.5).toFixed(1)} s, 95% ${pct(waits, 0.95).toFixed(1)} s, slowest ${waits.at(-1).toFixed(1)} s`);
if (rss.length) console.log(`server memory: ${idle?.toFixed(0)} MB idle, ${Math.max(...rss).toFixed(0)} MB at most, ${rss.at(-1).toFixed(0)} MB after`);
if (limited) console.log(`status checks past the account's limit (waited): ${limited}`);
if (errors.length) console.log('errors:', errors.slice(0, 5).join(' | '));
if (batches.some((b) => b.status !== 202) || singles.some((s) => s.status !== 202)) console.log('refused:', [...batches, ...singles].filter((x) => x.status !== 202).map((x) => x.status).join(', '));

await admin('auth', { cmd: 'api_delete_client', args: { id: 'load-test' } });
process.exit(failed ? 1 : 0);
