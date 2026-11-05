// The Design area's start page as a designer sees it. node dev/shots/design-home.mjs <out> [en|ru]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 1200, deviceScaleFactor: 1, mobile: false });
await send('Runtime.enable', {});
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
if (process.argv[3] === 'ru') await ev(`localStorage.setItem('quire.lang', 'ru')`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(4000);
await shot('design-home');
// Duplicate… a published report's template into another theme
await ev(`document.querySelector('[data-dup]')?.click()`); await sleep(1200);
await shot('design-duplicate');
await ev(`(() => { const t = [...document.querySelectorAll('.dup-t')].find((l) => l.textContent.includes('Bold Blocks')); t?.querySelector('input').click(); t?.querySelector('input').dispatchEvent(new Event('change')); })()`); await sleep(300);
await ev(`[...document.querySelectorAll('.modal-foot .btn')].find((b) => b.textContent.trim() === 'Duplicate')?.click()`); await sleep(5000);
console.log('duplicate →', await ev(`(async () => { const s = await import('/ui/state.js'); return s.S.st?.doc?.kind + ' · ' + s.S.st?.doc?.meta?.title + ' · theme ' + s.S.st?.doc?.theme?.name + ' · dirty ' + s.S.st?.dirty; })()`));
await ev(`(async () => { const s = await import('/ui/state.js'); s.S.showHome = true; s.render(); })()`); await sleep(2000);
await ev(`document.querySelector('.dh-tab[data-v=themes]')?.click()`); await sleep(2500);
await shot('design-themes');
await ev(`document.querySelector('.dh-tab[data-v=widgets]')?.click()`); await sleep(6000);
await shot('design-widgets');
console.log('widgets drawn:', await ev(`document.querySelectorAll('.dh-wthumb svg').length + ' of ' + document.querySelectorAll('.dh-wrow').length`));
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
console.log(await ev(`document.querySelector('.dh')?.innerText.slice(0, 1500)`));
chrome.kill(); process.exit(0);
