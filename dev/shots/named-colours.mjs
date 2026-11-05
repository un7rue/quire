// Theme colours by name: pick "Accent" in a widget's colour field → kept as $accent, drawn in the accent,
// and following a new accent. node dev/shots/named-colours.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1.5, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.S.showHome = false; m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
// a waterfall, selected, its Design tab open
const id = await ev(`(async () => { const s = await import('/ui/state.js'); const w = s.S.st.doc.pages.flatMap((p) => p.widgets).find((w) => w.kind === 'waterfall');
  await s.edit({ op: 'update_widget', id: w.id, patch: { opts: { viz: { grid: true, gridWidth: 2 } } } });
  s.S.sel = w.id; s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'design' };
  const p = await import('/ui/panels.js'); p.renderRight(); return w.id; })()`);
await sleep(1500);
const opened = await ev(`(() => { const b = document.querySelector('[data-key="viz.gridColor"]'); if (!b) return 'no field'; b.scrollIntoView({ block: 'center' }); b.click(); return 'open'; })()`);
await sleep(700);
console.log('field:', opened, '· named items:', await ev(`[...document.querySelectorAll('.cf-pop .cf-nm')].map((x) => x.textContent.trim()).join(', ')`));
await shot('popover');
await ev(`[...document.querySelectorAll('.cf-pop .cf-nm')].find((x) => /Accent|Акцент/.test(x.textContent))?.click()`); await sleep(1500);
const res = await ev(`(async () => { const s = await import('/ui/state.js'); const r = await import('/ui/render.js');
  const pi = s.S.st.doc.pages.findIndex((p) => p.widgets.some((x) => x.id === ${JSON.stringify(id)})); const w = s.S.st.doc.pages[pi].widgets.find((x) => x.id === ${JSON.stringify(id)});
  const acc = s.S.st.doc.theme.accent; const svg1 = r.pageSvg(s.S.st, pi, { exportMode: true });
  const stored = s.S.st.doc.pages[pi].widgets.find((x) => x.id === w.id).opts.viz.gridColor;
  await s.edit({ op: 'set_theme', patch: { accent: '#00aa55' } });
  const svg2 = r.pageSvg(s.S.st, pi, { exportMode: true });
  const field = document.querySelector('[data-key="viz.gridColor"]');
  return { stored, accentDrawn: svg1.toLowerCase().includes(acc.toLowerCase()), followsNew: svg2.includes('#00aa55'), noRawToken: !svg1.includes('$accent') && !svg2.includes('$accent'), fieldNamed: field?.classList.contains('named'), fieldTitle: field?.title };
})()`);
console.log(JSON.stringify(res));
await shot('after');
console.log('errors:', await ev(`JSON.stringify((window.__errors || []).slice(0, 3))`));
chrome.kill(); process.exit(0);
