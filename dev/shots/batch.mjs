// Batch reports: the dialog, the panel while it runs and when done. Writes PDFs into the
// bridge's temp folder (…/quire-batch). node dev/shots/batch.mjs <out> [en|ru]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=user]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(1500);
await ev(`document.querySelector('[data-a="batch"]')?.click()`); await sleep(1500);
await ev(`(() => { const p = document.querySelector('.bt-period'); p.value = 'quarter'; p.dispatchEvent(new Event('change')); })()`);
await ev(`document.querySelector('[data-a="folder"]')?.click()`); await sleep(800);
await shot(`batch-dialog-${lang}`);
await ev(`[...document.querySelectorAll('.modal-foot .btn')].pop().click()`); await sleep(9000);
await shot(`batch-running-${lang}`);
for (let i = 0; i < 40 && !(await ev(`!!document.querySelector('.bt-panel.finished')`)); i++) await sleep(2000);
await shot(`batch-done-${lang}`);
console.log(await ev(`document.querySelector('.bt-panel')?.innerText`));
chrome.kill(); process.exit(0);
