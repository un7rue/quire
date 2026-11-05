// Duplicate → it shows in Your templates as a draft → Publish… → published. node dev/shots/publish-flow.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1100, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(4000);
const name = 'Test copy ' + Date.now().toString(36);
await ev(`document.querySelector('[data-dupkind]')?.click()`); await sleep(1200);
await ev(`(() => { const n = document.querySelector('.dup-name'); n.value = ${JSON.stringify(name)}; const t = [...document.querySelectorAll('.dup-t')].find((l) => l.textContent.includes('Alpine')); t.querySelector('input').click(); t.querySelector('input').dispatchEvent(new Event('change')); [...document.querySelectorAll('.modal-foot .btn')].find((b) => b.textContent.trim() === 'Duplicate').click(); })()`); await sleep(5000);
console.log('after duplicate:', await ev(`(async () => { const s = await import('/ui/state.js'); return s.S.st.doc.meta.title + ' · ' + s.S.st.doc.theme.name + ' · saved ' + !s.S.st.dirty + ' · ' + s.S.st.path; })()`));
const home = async () => { await ev(`(async () => { const s = await import('/ui/state.js'); s.S.showHome = true; s.render(); })()`); await sleep(2500); };
const rowText = () => ev(`[...document.querySelectorAll('.dh-t tbody tr')].find((tr) => tr.textContent.includes(${JSON.stringify(name)}))?.innerText.replace(/\\s+/g, ' ') || 'NOT LISTED'`);
await home();
console.log('listed:', await rowText());
await shot('templates-list');
// Publish… from the row
await ev(`[...document.querySelectorAll('.dh-t tbody tr')].find((tr) => tr.textContent.includes(${JSON.stringify(name)})).querySelector('[data-path-pub]').click()`); await sleep(4000);
await shot('publish-dialog');
await ev(`document.querySelector('.pub-dlg [data-a=publish]')?.click()`); await sleep(2500);
await ev(`[...document.querySelectorAll('.modal-foot .btn')].find((b) => b.textContent.trim() === 'Done')?.click()`); await sleep(800);
await home();
console.log('after publish:', await rowText());
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
