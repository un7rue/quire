// Admin → API: adds an API account (its key shown once), a banker's clients as its access, then lets
// the UHNWI report take products over the API with a default; prints what it saw.
//   node dev/shots/admin-api.mjs <out> [en|ru]       (needs the web server and the static server)
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 1100, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html?web' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(800);
// from a clean start: no account of an earlier run
await ev(`(async () => { const m = await import('/ui/state.js'); await m.call('auth', { cmd: 'api_delete_client', args: { id: 'client-portal' } }, { quiet: true }).catch(() => {}); })()`);
await ev(`document.querySelector('.adm-item[data-part="api"]')?.click()`); await sleep(2500);
console.log('page:', await ev(`document.querySelector('.ar-head h2')?.innerText + ' · ' + (document.querySelector('.api-list')?.innerText || '').replace(/\\s+/g, ' ')`));
await shot(`admin-api-empty-${lang}`);
// a new account with one banker's clients
await ev(`document.querySelector('[data-a="add"]').click()`); await sleep(500);
await ev(`(() => { const f = document.querySelector('.api-edit'); f.querySelector('[name=name]').value = 'Client portal'; f.querySelector('[name=description]').value = 'The bank’s client portal (digital team)';
  f.querySelector('.ad-scope:not(.ad-prod) button[data-v="all"]').click(); f.querySelector('.ad-scope:not(.ad-prod) button[data-v="some"]').click(); })()`); await sleep(300);
await ev(`document.querySelector('.api-edit input[data-k="coverage"]').click()`); await sleep(800);
await shot(`admin-api-new-${lang}`);
await ev(`document.querySelector('.api-edit [data-a="save"]').click()`); await sleep(1500);
const key = await ev(`document.querySelector('.api-key code')?.innerText || ''`);
console.log('key shown:', /^qk\.client-portal\.[\w-]{40,}$/.test(key) ? 'yes' : `no (${key})`);
await shot(`admin-api-key-${lang}`);
await ev(`document.querySelector('.modal-foot .btn.primary')?.click()`); await sleep(800);
console.log('list:', await ev(`document.querySelector('.api-list').innerText.replace(/\\s+/g, ' ')`));
console.log('key row:', await ev(`document.querySelector('.api-keyrow')?.innerText.replace(/\\s+/g, ' ')`));
const secret = key.split('.').pop() || '(no key)';
console.log('key in page after:', await ev(`document.body.innerHTML.includes(${JSON.stringify(secret)}) ? 'yes (bad)' : 'no'`));
// the UHNWI report takes products, BROK by default
await ev(`(() => { const tr = document.querySelector('.api-rep[data-id="uhnwi"] tr[data-p="product"]'); tr.querySelector('[data-k="accept"]').checked = true; tr.querySelector('[data-k="default"]').value = 'BROK, BANK'; document.querySelector('.api-rep[data-id="uhnwi"] [data-a="save-rep"]').click(); })()`); await sleep(1200);
console.log('product default:', await ev(`document.querySelector('.api-rep[data-id="uhnwi"] tr[data-p="product"] [data-k="default"]').value`));
await ev(`document.querySelector('.api-reps').scrollIntoView()`); await sleep(400);
await shot(`admin-api-reports-${lang}`);
// Activity has the changes
const acts = await ev(`(async () => { const m = await import('/ui/state.js'); const r = await m.call('activity', { filter: { area: 'admin', limit: 5 } }, { quiet: true }); return r.rows.map((x) => x.detail).join(' | '); })()`);
console.log('activity:', acts);
console.log('key in activity:', acts.includes(secret) ? 'yes (bad)' : 'no');
chrome.kill(); process.exit(0);
