// The report API end to end, against a running server built with the renderer: an administrator
// makes an API account, the account asks for reports with its key, waits, downloads PDFs and a zip.
//
//   QUIRE_SECRET_KEY='a test passphrase here' cargo run --no-default-features --features render --bin quire-server -- \
//       --port 8771 --data "$(mktemp -d)"
//   node dev/web/api-checks.mjs [http://127.0.0.1:8771]
//
// It adds an account “API check” and deletes it at the end.
const BASE = process.argv[2] || 'http://127.0.0.1:8771';
let failed = 0;
const check = (name, ok, got) => { if (!ok) failed++; console.log(`${ok ? '✓' : '✗'} ${name}${ok ? '' : `  — got: ${String(JSON.stringify(got)).slice(0, 300)}`}`); };

// the administrator, in a browser of their own
let cookie = '';
const admin = async (cmd, args = {}) => {
  const r = await fetch(BASE + '/api/call', { method: 'POST', headers: { 'Content-Type': 'text/plain', ...(cookie && { Cookie: cookie }) }, body: JSON.stringify({ cmd, args }) });
  const set = r.headers.get('set-cookie');
  if (set) cookie = set.split(';')[0];
  const j = await r.json();
  if (!r.ok) throw new Error(JSON.stringify(j));
  return j;
};
await admin('auth', { cmd: 'login', args: { login: 'admin', password: 'quire' } });
await admin('auth', { cmd: 'api_delete_client', args: { id: 'api-check' } }).catch(() => {});
const made = await admin('auth', { cmd: 'api_save_client', args: { client: { name: 'API check', active: true, access: { all: false, coverage: ['S0001'], sources: [], clients: [] } } } });
const KEY = made.key;
check('an API account gets its key once', /^qk\.api-check\./.test(KEY || ''), made);
await admin('auth', { cmd: 'api_save_report', args: { id: 'uhnwi', settings: { params: { product: { accept: true, default: null } } } } });
const mine = (await admin('auth', { cmd: 'preview', args: { access: { all: false, coverage: ['S0001'], sources: [], clients: [] } } })).clients.map((c) => c.id);
const everyone = (await admin('auth', { cmd: 'options' })).clients.map((c) => c.id);
const other = everyone.find((c) => !mine.includes(c));

// the system, with its key
const api = async (method, path, body, headers = {}) => {
  const r = await fetch(BASE + path, { method, headers: { ...(KEY && { Authorization: `Bearer ${KEY}` }), ...(body && { 'Content-Type': 'application/json' }), ...headers }, body: body && JSON.stringify(body) });
  const kind = r.headers.get('content-type') || '';
  return { status: r.status, headers: r.headers, json: kind.includes('json') ? await r.json() : null, bytes: kind.includes('json') ? null : Buffer.from(await r.arrayBuffer()) };
};
const wait = async (id) => {
  for (let i = 0; i < 1200; i++) {
    const r = await api('GET', `/api/v1/jobs/${id}`);
    if (!['queued', 'running'].includes(r.json?.status)) return r.json;
    await new Promise((res) => setTimeout(res, 250));
  }
  return null;
};

check('no key: 401', (await fetch(BASE + '/api/v1/templates')).status === 401);
check('a wrong key: 401', (await fetch(BASE + '/api/v1/templates', { headers: { Authorization: 'Bearer qk.api-check.nope' } })).status === 401);
const t = await api('GET', '/api/v1/templates');
check('the reports it may ask for, with what each takes', t.status === 200 && t.json.templates.some((x) => x.id === 'uhnwi' && x.params.some((p) => p.name === 'product')), t.json);

const t0 = Date.now();
const one = await api('POST', '/api/v1/reports', { client_id: mine[0], period: { preset: 'last_quarter' }, params: { product: ['BROK', 'BANK'] } }, { 'Idempotency-Key': `check-${t0}` });
check('one report: 202 and a job', one.status === 202 && one.json.job_id && one.headers.get('location') === `/api/v1/jobs/${one.json.job_id}`, one.json);
const replay = await api('POST', '/api/v1/reports', { client_id: mine[0], period: { preset: 'last_quarter' }, params: { product: ['BROK', 'BANK'] } }, { 'Idempotency-Key': `check-${t0}` });
check('…the same Idempotency-Key gets the same job', replay.json?.job_id === one.json.job_id && replay.headers.get('idempotent-replayed') === 'true', replay.json);
const j1 = await wait(one.json.job_id);
check(`…done (${((Date.now() - t0) / 1000).toFixed(1)} s), with its pages and the values used`, j1?.status === 'done' && j1.items[0].pages > 10 && j1.items[0].values.params.product.join() === 'BROK,BANK', j1);
const pdf = await api('GET', `/api/v1/jobs/${one.json.job_id}/file`);
check('…its PDF downloads', pdf.status === 200 && pdf.bytes.subarray(0, 4).toString() === '%PDF' && /filename\*=UTF-8''/.test(pdf.headers.get('content-disposition') || ''), pdf.status);

const t1 = Date.now();
const batch = await api('POST', '/api/v1/reports', { clients: mine.slice(0, 3), period: { start: '2026-01-01', end: '2026-06-30' } });
check('a batch of 3: one job', batch.status === 202 && batch.json.reports === 3, batch.json);
const early = await api('GET', `/api/v1/jobs/${batch.json.job_id}/file`);
check('…its zip isn’t given half made (409 while running)', early.status === 409 || early.status === 200, early.status);
const j2 = await wait(batch.json.job_id);
check(`…done (${((Date.now() - t1) / 1000).toFixed(1)} s)`, j2?.status === 'done', j2);
const zip = await api('GET', `/api/v1/jobs/${batch.json.job_id}/file`);
check('…a zip of the PDFs', zip.status === 200 && zip.bytes.subarray(0, 2).toString() === 'PK' && zip.headers.get('content-type') === 'application/zip', zip.status);
const each = await api('GET', `/api/v1/jobs/${batch.json.job_id}/files/${mine[1]}`);
check('…and each client’s PDF on its own', each.status === 200 && each.bytes.subarray(0, 4).toString() === '%PDF', each.status);

const foreign = await api('POST', '/api/v1/reports', { clients: [mine[0], other], period: { preset: 'ytd' } });
check('another banker’s client: 403, nothing started, the client named', foreign.status === 403 && foreign.json.error.clients?.[0]?.client_id === other, foreign.json);
const unknown = await api('POST', '/api/v1/reports', { client_id: 'C9999', period: { preset: 'ytd' } });
check('an unknown client: 404', unknown.status === 404, unknown.json);
const fixed = await api('POST', '/api/v1/reports', { client_id: mine[0], period: { preset: 'ytd' }, params: { colour: 'red' } });
check('a parameter the report doesn’t take: 422', fixed.status === 422, fixed.json);
const bad = await api('POST', '/api/v1/reports', { client_id: mine[0], period: { preset: 'next_year' } });
check('an unknown period: 422', bad.status === 422, bad.json);

// Activity has it, under the account, and never the key
const log = await admin('activity', { filter: { limit: 200 } });
const rows = log.rows.filter((r) => r.user === 'api:api-check');
check('Activity: the requests and the reports, under the account', rows.some((r) => r.action === 'report request') && rows.some((r) => r.action === 'api export'), rows.slice(0, 3));
check('…and never the key', !JSON.stringify(log).includes(KEY.split('.').pop()));

// switched off: its key stops working
await admin('auth', { cmd: 'api_save_client', args: { client: { was: 'api-check', name: 'API check', active: false, access: { all: false, coverage: ['S0001'], sources: [], clients: [] } } } });
check('switched off: its key is refused', (await api('GET', '/api/v1/templates')).status === 401);
await admin('auth', { cmd: 'api_delete_client', args: { id: 'api-check' } });

console.log(failed ? `\n${failed} check(s) failed` : '\nall checks passed');
process.exit(failed ? 1 : 0);
