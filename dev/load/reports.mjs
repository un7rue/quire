// Load check for the web server: N bankers at once, each in a workspace of their own (cookie),
// sign in and open the UHNWI report for a client, then change the period once.
//   node dev/load/reports.mjs [users=20] [url=http://127.0.0.1:8766]
const N = +(process.argv[2] || 20);
const URL = (process.argv[3] || 'http://127.0.0.1:8766') + '/api/call';
const CLIENTS = ['LeBron Jameson', 'Stephen Curryman', 'Kevin Durantino', 'Giannis Antetokos', 'Nikola Jokich', 'Кирилл Шарихин'];

async function person(i) {
  let cookie = '';
  const call = async (cmd, args = {}) => {
    const t = performance.now();
    const r = await fetch(URL, { method: 'POST', headers: { 'Content-Type': 'text/plain', ...(cookie && { Cookie: cookie }) }, body: JSON.stringify({ cmd, args }) });
    const set = r.headers.get('set-cookie');
    if (set) cookie = set.split(';')[0];
    const j = await r.json();
    if (!r.ok) throw new Error(`${cmd}: ${j}`);
    return [j, performance.now() - t];
  };
  const out = {};
  [, out.signIn] = await call('auth', { cmd: 'login', args: { login: 'user', password: 'quire' } });
  const [st, open] = await call('new_doc', { kind: 'pub:uhnwi', client: CLIENTS[i % CLIENTS.length] });
  out.open = open;
  out.pages = st.doc.pages.length;
  out.errors = st.pages.flatMap((p) => p.widgets || []).filter((w) => w.data?.error).length;
  [, out.period] = await call('edit', { op: { op: 'set_params', params: { ...st.doc.params, grain: 'quarter' } } }).catch((e) => [null, NaN]);
  return out;
}

const pct = (a, p) => a.slice().sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(p * a.length))];
const t0 = performance.now();
const res = await Promise.allSettled(Array.from({ length: N }, (_, i) => person(i)));
const wall = performance.now() - t0;
const ok = res.filter((r) => r.status === 'fulfilled').map((r) => r.value);
const fail = res.filter((r) => r.status === 'rejected');
const line = (k) => { const a = ok.map((o) => o[k]).filter((x) => !Number.isNaN(x)); return a.length ? `median ${(pct(a, 0.5) / 1000).toFixed(2)} s · p95 ${(pct(a, 0.95) / 1000).toFixed(2)} s · max ${(Math.max(...a) / 1000).toFixed(2)} s` : 'n/a'; };
console.log(`${N} people at once · ${ok.length} ok · ${fail.length} failed · all done in ${(wall / 1000).toFixed(1)} s`);
console.log(`  sign in      ${line('signIn')}`);
console.log(`  open report  ${line('open')}   (${ok[0]?.pages} pages; widgets with data errors: ${ok.reduce((t, o) => t + o.errors, 0)})`);
console.log(`  change period ${line('period')}`);
for (const f of fail.slice(0, 3)) console.log('  failed:', String(f.reason).slice(0, 200));
