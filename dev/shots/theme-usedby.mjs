// A theme used by many templates: makes 12 copies in Alpine, then the Themes view's "Used by" list and
// the Templates view filtered to the theme. node dev/shots/theme-usedby.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3500);
await ev(`(async () => { const s = await import('/ui/state.js'); for (let i = 1; i <= 12; i++) { await s.call('auth', { cmd: 'duplicate_template', args: { kind: 'uhnwi_report', title: 'Филиал ' + i + ' — отчёт', theme: 'builtin:alpine' } }); } s.S.showHome = true; s.S.designView = 'themes'; s.render(); })()`); await sleep(8000);
await ev(`document.querySelector('[data-usedby="Alpine"]')?.click()`); await sleep(800);
await shot('usedby-popover');
console.log('popover rows:', await ev(`document.querySelectorAll('.dh-usedpop-i').length`));
await ev(`document.querySelector('.dh-usedpop-all')?.click()`); await sleep(2500);
console.log('templates filtered:', await ev(`document.querySelector('.dh-fchip')?.innerText + ' · rows ' + document.querySelectorAll('.dh-t tbody tr').length`));
await shot('templates-by-theme');
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
