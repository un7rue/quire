// The widget studio: open a built-in widget from the Design area's Widgets tab, change it, save it as your own;
// then open your widget, change it, save (version 2). node dev/shots/widget-studio.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`document.querySelector('.dh-tab[data-v=widgets]')?.click()`); await sleep(6000);
// a built-in KPI card ("Карточка с разбивкой")
await shot('widgets-table');
await ev(`[...document.querySelectorAll('.dh-wkind')].find((k) => /KPI/.test(k.textContent)).querySelectorAll('.dh-wrow')[2].click()`); await sleep(4000);
console.log('opened:', await ev(`(async () => { const s = await import('/ui/state.js'); const d = s.S.st.doc; return d.widget_studio + ' · ' + d.meta.title + ' · ' + d.pages[0].widgets[0].kind + '/' + d.pages[0].widgets[0].variant + ' · save button: ' + document.querySelector('#save')?.textContent.trim(); })()`));
await ev(`(async () => { const s = await import('/ui/state.js'); s.S.sel = 'w_studio'; s.render(); })()`); await sleep(1000);
await shot('studio-builtin');
// a smaller preview size from the pane
await ev(`[...document.querySelectorAll('.ws-pane .seg button')].find((b) => b.textContent.trim() === 'Small')?.click()`); await sleep(1500);
await shot('studio-small');
await ev(`[...document.querySelectorAll('.ws-pane .seg button')].find((b) => b.textContent.trim() === 'Large')?.click()`); await sleep(1500);
// change it and save as my widget
await ev(`(async () => { const s = await import('/ui/state.js'); await s.edit({ op: 'update_widget', id: 'w_studio', patch: { opts: { viz: { kpiAlign: 'center' } } } }); document.querySelector('#save').click(); })()`); await sleep(1200);
const name = 'Моя карточка ' + Date.now().toString(36).slice(-4);
await ev(`(() => { document.querySelector('.wt-name').value = ${JSON.stringify(name)}; [...document.querySelectorAll('.modal-foot .btn')].find((b) => b.textContent.trim() === 'Save type').click(); })()`); await sleep(2500);
console.log('saved:', await ev(`(async () => { const s = await import('/ui/state.js'); const d = s.S.st.doc; const t = d.theme.widget_types[d.widget_studio]; return d.widget_studio + ' · v' + t?.version + ' · binding kept out: ' + (t?.binding == null) + ' · align ' + t?.opts?.viz?.kpiAlign + ' · button: ' + document.querySelector('#save')?.textContent.trim() + ' · dirty ' + s.S.st.dirty; })()`));
// change again, Save widget (update)
await ev(`(async () => { const s = await import('/ui/state.js'); await s.edit({ op: 'update_widget', id: 'w_studio', patch: { opts: { viz: { arrow: true } } } }); document.querySelector('#save').click(); })()`); await sleep(2500);
console.log('updated:', await ev(`(async () => { const s = await import('/ui/state.js'); const d = s.S.st.doc; const t = d.theme.widget_types[d.widget_studio]; return 'v' + t?.version + ' · arrow ' + t?.opts?.viz?.arrow; })()`));
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
