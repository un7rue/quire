// Admin → Activity → Sessions: a banker's session is made first (a report, a client, an edit,
// time, another client), then shown to the administrator. node dev/shots/sessions.mjs <out> [en|ru]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 940, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
const out = async () => { await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`); await send('Page.reload', {}); await sleep(3000); };
await out();
await ev(`document.querySelector('.lg-demo button[data-l=user]')?.click()`); await sleep(3000);
await ev(`document.querySelector('.us-row .btn')?.click()`); await sleep(7000);
console.log(await ev(`(async () => {
  const m = await import('/ui/state.js');
  const d = m.S.st.doc;
  const w = d.pages[2].widgets.find((x) => x.kind === 'table');
  await m.edit({ op: 'update_widget', id: w.id, patch: { title: 'Результат по продуктам · с начала года' } });
  for (let i = 0; i < 4; i++) await m.call('activity_ping', { seconds: 30 }, { quiet: true });
  const cl = (await m.call('auth', { cmd: 'my_clients' }, { quiet: true }))[1]?.name;
  await m.edit({ op: 'set_params', params: { ...m.S.st.doc.params, values: { ...m.S.st.doc.params.values, client: cl } } });
  await m.call('activity_ping', { seconds: 30 }, { quiet: true });
  return cl;
})()`));
await out();
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(600);
await ev(`document.querySelector('.adm-item[data-part="activity"]')?.click()`); await sleep(2500);
// a session with AI requests if there is one (else the second), opened under its row
await ev(`(() => { const rows = [...document.querySelectorAll('.lg-t tr[data-k]')]; (rows.find((r) => r.querySelector('.ss-chip.ai')) || rows[1])?.click(); })()`); await sleep(1200);
await ev(`document.querySelector('.lg-t tr.on')?.scrollIntoView({ block: 'start' })`); await sleep(400);
await shot(`sessions-${lang}`);
await ev(`document.querySelector('.lg-panel .tl-more')?.click()`); await sleep(1500);
await ev(`document.querySelector('.lg-panel .tl-ai')?.scrollIntoView({ block: 'center' })`); await sleep(400);
await shot(`sessions-detail-${lang}`);
// the events view opens the same way
await ev(`document.querySelector('.lg-view button[data-v="events"]')?.click()`); await sleep(2500);
await ev(`document.querySelectorAll('.lg-t tr[data-k]')[2]?.click()`); await sleep(800);
await shot(`events-${lang}`);
chrome.kill(); process.exit(0);
