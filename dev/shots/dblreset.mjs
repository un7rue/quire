// Double-click a setting's name to reset it: as a designer (Design tab, card, text style) and as a
// report user (Fit). Also the Design home's Edit. node dev/shots/dblreset.mjs <out>
import { send, ev, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1100, deviceScaleFactor: 1, mobile: false });
const login = async (who) => {
  await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(2500);
  await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
  await send('Page.reload', {}); await sleep(2500);
  await ev(`document.querySelector('.lg-demo button[data-l=${who}]')?.click()`); await sleep(3500);
};
// the Design home: Edit opens the published report's template
await login('design');
console.log('design home edit →', await ev(`(async () => { document.querySelector('[data-edit]')?.click(); await new Promise((r) => setTimeout(r, 4000)); const s = await import('/ui/state.js'); return s.S.st?.doc?.kind + ' · ' + s.S.st?.doc?.meta?.title + ' · report ' + s.S.st?.doc?.meta?.report_id; })()`));
const tryReset = async (label, kind, patch, sel, check, tab = 'design') => ev(`(async () => {
  const s = await import('/ui/state.js'); const p = await import('/ui/panels.js');
  const hit = s.S.st.doc.pages.flatMap((pg) => pg.widgets).find((w) => w.kind === ${JSON.stringify(kind)} && w.binding);
  await s.edit({ op: 'update_widget', id: hit.id, patch: ${JSON.stringify(patch)} });
  s.S.sel = hit.id; s.S.inspTab = { ...(s.S.inspTab || {}), widget: ${JSON.stringify(tab)} }; s.render();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.insp-sec.closed .insp-title').forEach((b) => b.click());
  const el = [...document.querySelectorAll('.insp ' + ${JSON.stringify(sel)})].find((l) => l.textContent.trim() === ${JSON.stringify(label)});
  if (!el) return 'no such name shown';
  if (!el.classList.contains('dbl-reset')) return 'not marked as set';
  el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  await new Promise((r) => setTimeout(r, 500));
  const w = s.findWidget(hit.id).w;
  return (${check})(w) ? 'reset ✓' : 'still set ✗ ' + JSON.stringify(w.opts?.viz || {}).slice(0, 120);
})()`);
console.log('designer · KPI Align:', await tryReset('Align', 'kpi', { opts: { viz: { kpiAlign: 'right' } } }, '.f-row > label', '(w) => w.opts?.viz?.kpiAlign == null'));
console.log('designer · Card fill:', await tryReset('Fill', 'kpi', { style: { background: '#ff0000' } }, '.f-row > label', '(w) => w.style?.background == null'));
console.log('designer · Text size:', await tryReset('Size', 'kpi', { style: { fonts: { kpi: { size: 30 } } } }, 'details[data-role=kpi] .fc-l', '(w) => w.style?.fonts?.kpi?.size == null', 'text'));
await login('user');
await ev(`(async () => { const s = await import('/ui/state.js'); await s.api.newDoc('pub:uhnwi', 'James Hardenberg'); })()`); await sleep(5000);
console.log('report user · Spacing:', await tryReset('Spacing', 'kpi', { style: { pad: 16 } }, '.f-row > label', '(w) => w.style?.pad == null'));
console.log('report user · Decimals:', await tryReset('Decimals', 'kpi', { opts: { viz: { decimals: 2 } } }, '.f-row > label', '(w) => w.opts?.viz?.decimals == null'));
chrome.kill(); process.exit(0);
