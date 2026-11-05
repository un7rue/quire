// Widget types: save a KPI as a type, add another from the Widgets panel, update the type, detach.
// node dev/shots/widget-types.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1100, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
const name = 'Карточка итога ' + Date.now().toString(36).slice(-4);
// Save as widget type… on a KPI (the dialog filled in)
await ev(`(async () => { const s = await import('/ui/state.js'); const p = await import('/ui/panels.js');
  const w = s.S.st.doc.pages.flatMap((pg) => pg.widgets).find((x) => x.kind === 'kpi' && x.binding);
  window.__kpi = w.id; s.S.sel = w.id; p.saveWidgetType(s.findWidget(w.id)); })()`); await sleep(800);
await shot('wt-save-dialog');
await ev(`(() => { document.querySelector('.wt-name').value = ${JSON.stringify(name)}; document.querySelector('.wt-desc').value = 'Итог с примечанием'; [...document.querySelectorAll('.modal-foot .btn')].find((b) => b.textContent.trim() === 'Save type').click(); })()`); await sleep(2500);
console.log('saved:', await ev(`(async () => { const s = await import('/ui/state.js'); const w = s.findWidget(window.__kpi).w; return w.type + ' · types in doc ' + Object.keys(s.S.st.doc.theme.widget_types || {}).length; })()`));
// the Widgets panel lists it; click adds one to the page
await ev(`(async () => { const s = await import('/ui/state.js'); s.S.leftTab = 'widgets'; s.render(); })()`); await sleep(1500);
console.log('palette:', await ev(`[...document.querySelectorAll('.wt-item b')].map((b) => b.textContent).join(', ')`));
await shot('wt-palette', { x: 0, y: 40, width: 360, height: 900 });
await ev(`[...document.querySelectorAll('.wt-item')].find((x) => x.textContent.includes(${JSON.stringify(name)})).click()`); await sleep(2500);
console.log('added:', await ev(`(async () => { const s = await import('/ui/state.js'); const all = s.S.st.doc.pages.flatMap((pg) => pg.widgets).filter((x) => x.type && s.S.st.doc.theme.widget_types[x.type]?.name === ${JSON.stringify(name)}); return all.length + ' widgets of the type'; })()`));
// Design tab of the typed widget
await ev(`(async () => { const s = await import('/ui/state.js'); const p = await import('/ui/panels.js'); s.S.sel = window.__kpi; s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'design' }; p.renderRight(); })()`); await sleep(800);
const b = JSON.parse(await ev(`JSON.stringify(document.querySelector('.insp').getBoundingClientRect())`));
await shot('wt-design', { x: b.x, y: b.y, width: b.width, height: 520 });
// change the first widget, update the type: the other follows
console.log('update:', await ev(`(async () => { const s = await import('/ui/state.js'); const p = await import('/ui/panels.js');
  await s.edit({ op: 'update_widget', id: window.__kpi, patch: { opts: { viz: { kpiAlign: 'center' } } } });
  await p.saveWidgetType(s.findWidget(window.__kpi), { update: true });
  const tid = s.findWidget(window.__kpi).w.type; const t = s.S.st.doc.theme.widget_types[tid];
  return 'version ' + t.version + ' · align in type ' + t.opts?.viz?.kpiAlign;
})()`));
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
