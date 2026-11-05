// ⇧-click several widgets: the inspector shows the settings they all have, and a change, a move or a delete
// goes to all of them in one step. node dev/shots/multi-select.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.S.showHome = false; m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
// two widgets on one page by kind: [first, second]
const pick = (a, b) => ev(`(async () => { const s = await import('/ui/state.js');
  for (const [pi, p] of s.S.st.doc.pages.entries()) { const x = p.widgets.find((w) => w.kind === ${JSON.stringify(a)} && !w.opts?.table?.continues), y = p.widgets.find((w) => w.kind === ${JSON.stringify(b)} && w !== x && !w.opts?.table?.continues); if (x && y) return JSON.stringify({ pi, ids: [x.id, y.id] }); }
  return 'null'; })()`);
// a real click, with ⇧ for the second
const click = async (id, shift) => {
  await ev(`(async () => { const c = await import('/ui/canvas.js'); const s = await import('/ui/state.js'); c.scrollToWidget?.(${JSON.stringify(id)}); })()`); await sleep(600);
  const r = JSON.parse(await ev(`JSON.stringify(document.querySelector('.hit[data-id="${id}"]').getBoundingClientRect())`));
  const x = r.x + r.width / 2, y = r.y + Math.min(12, r.height / 2), modifiers = shift ? 8 : 0;
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1, modifiers });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1, modifiers }); await sleep(700);
};
const inspector = () => ev(`(() => { const head = document.querySelector('.insp-head b')?.textContent; const tabs = [...document.querySelectorAll('.insp-tabs .it')].map((b) => b.textContent.trim()).join(', ');
  const secs = [...document.querySelectorAll('.insp-pane .insp-sec .is-name')].map((n) => n.textContent).join(' · '); return head + ' | tabs: ' + tabs + ' | sections: ' + secs; })()`);
const tab = (k) => ev(`document.querySelector('.insp-tabs .it[data-k="${k}"]')?.click()`);

for (const [a, b] of [['bar', 'bar'], ['bar', 'kpi']]) {
  const p = JSON.parse(await pick(a, b));
  if (!p) { console.log(a, b, ': none on one page'); continue; }
  await click(p.ids[0], false); await click(p.ids[1], true);
  console.log(`${a} + ${b} selected:`, await ev(`(async () => (await import('/ui/state.js')).selection().join(', '))()`));
  for (const k of ['design', 'text', 'layout']) { await tab(k); await sleep(300); console.log(' ', k, '→', await inspector()); }
  await tab('design'); await sleep(300);
  await shot(`multi-${a}-${b}`);
}
// a change through the inspector reaches both (bar + kpi selected now): the card background
const res = await ev(`(async () => { const s = await import('/ui/state.js'); const ids = s.selection();
  await s.edit({ op: 'update_widget', id: ids[0], patch: { style: { background: '#ffeeaa' } } });
  const bg = ids.map((id) => s.findWidget(id).w.style?.background);
  const before = ids.map((id) => s.findWidget(id).w.x + ',' + s.findWidget(id).w.y);
  const w0 = s.findWidget(ids[0]).w; await s.edit({ op: 'set_rect', id: ids[0], rect: { x: w0.x, y: w0.y + 1, w: w0.w, h: w0.h } });
  const after = ids.map((id) => s.findWidget(id).w.x + ',' + s.findWidget(id).w.y);
  await s.api.undo(); await s.api.undo();
  const undone = ids.map((id) => s.findWidget(id).w.style?.background || 'none');
  return JSON.stringify({ bg, before, after, undone, stillSelected: s.selection().length }); })()`);
console.log('change / move / undo:', res);
// ⌫ deletes both, ⌘Z brings them back
const del = await ev(`(async () => { const s = await import('/ui/state.js'); const ids = s.selection(); await s.edit({ op: 'remove_widget', id: ids[0] });
  const gone = ids.filter((id) => !s.findWidget(id)).length; await s.api.undo(); return 'deleted ' + gone + ', back after undo ' + ids.filter((id) => s.findWidget(id)).length; })()`);
console.log(del);
// set differently: "Mixed" until one value is set for all (deleting ended the selection: select them again)
const two = JSON.parse(await pick('bar', 'kpi'));
const mixed = await ev(`(async () => { const s = await import('/ui/state.js'); const p = await import('/ui/panels.js'); const ids = ${JSON.stringify(two.ids)};
  s.S.sel = ids[0]; await s.edit({ op: 'update_widget', id: ids[1], patch: { style: { background: '#ddeeff' }, opts: { viz: { decimals: 2 } } } });
  s.toggleSelected(ids[1]); s.S.inspTab.widget = 'design'; p.renderRight(); await new Promise((r) => setTimeout(r, 200));
  const read = () => ({ mixed: [...document.querySelectorAll('.insp-pane .mixed')].map((n) => n.dataset.key).join(' '), tags: document.querySelectorAll('.insp-pane .mixed-tag').length });
  const before = read();
  await s.edit({ op: 'update_widget', id: ids[0], patch: { opts: { viz: { decimals: 1 } } } }); p.renderRight(); await new Promise((r) => setTimeout(r, 200));
  const after = read();
  return JSON.stringify({ before, afterSettingDecimals: after, decimalsNow: ids.map((id) => s.findWidget(id).w.opts?.viz?.decimals) }); })()`);
console.log('mixed:', mixed);
await shot('multi-mixed');
// search finds the settings they share
for (const q of ['colour', 'decimals', 'subtitle', 'background']) {
  const r = await ev(`(async () => { const s = await import('/ui/state.js'); const p = await import('/ui/panels.js'); s.S.inspQuery = ${JSON.stringify(q)}; p.renderRight(); await new Promise((r) => setTimeout(r, 200));
    const out = (document.querySelector('.sr-count') || document.querySelector('.sr-empty'))?.textContent.trim(); s.S.inspQuery = ''; return out; })()`);
  console.log(`search "${q}":`, r);
}
// a plain click selects one again
const one = JSON.parse(await pick('bar', 'kpi')); await click(one.ids[0], false);
console.log('plain click:', await ev(`(async () => (await import('/ui/state.js')).selection().length)()`), 'selected ·', await inspector());
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
