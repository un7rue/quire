// A chart with two series: Colours, Marks, Highlights and Data labels have what the series share, then a part per
// series under its name; the Series cards keep name, columns or line, and axis. Each per-series setting changes
// only the drawing (checked one by one). node dev/shots/series-parts.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 2400, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.S.showHome = false; m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
console.log(await ev(`(async () => { try {
  const s = await import('/ui/state.js'); const r = await import('/ui/render.js'); const p = await import('/ui/panels.js');
  const w = s.S.st.doc.pages.flatMap((p) => p.widgets).find((w) => /нарастающим/.test(w.title || ''));
  const pi = s.findWidget(w.id).pi; const svg = () => r.pageSvg(s.S.st, pi, { exportMode: true });
  const open = () => { s.S.sel = w.id; s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'design' }; s.S.inspQuery = ''; p.renderRight(); document.querySelectorAll('.insp-sec.closed .insp-title').forEach((b) => b.click()); };
  open();
  const out = [];
  for (const t of ['Series', 'Colours', 'Marks', 'Highlights', 'Data labels']) {
    const sec = document.querySelector('.insp-sec[data-sec="' + t + '"]');
    if (!sec) { out.push(t + ': (none)'); continue; }
    out.push(t + ': ' + [...sec.querySelectorAll('.insp-body > .vz-sub, .insp-body > .f-row > label, .ser-card .ser-body .f-row > label, .ser-card input')].map((n) => n.classList?.contains('vz-sub') ? '## ' + n.textContent : n.tagName === 'INPUT' ? '[' + (n.value || n.placeholder) + ']' : n.textContent).join(' | '));
  }
  // one setting per section, on one series: each changes the drawing
  const ms = w.binding.measures; const kb = ms.find((m) => m.chart !== 'line')?.id, kl = ms.find((m) => m.chart === 'line')?.id;
  const tries = [['columns: colour by up / down', kb, { colorMode: 'sign' }], ['columns: lighten fills 60%', kb, { tint: 0.6 }], ['columns: outline', kb, { outline: true }],
    ['columns: best & worst', kb, { extremes: true, bands: true }], ['columns: callout on the largest', kb, { callout: 'max' }],
    ['line: smooth', kl, { curve: 'smooth' }], ['line: area under', kl, { areaFill: 'flat' }], ['line: markers all, hollow', kl, { markers: 'all', markerFill: 'hollow' }], ['line: callout last', kl, { callout: 'last' }]];
  for (const [label, key, patch] of tries) {
    if (!key) { out.push(label + ': no such series'); continue; }
    const before = svg(); await s.edit({ op: 'update_widget', id: w.id, patch: { opts: { viz: { seriesOpts: { [key]: patch } } } } }); const after = svg(); await s.api.undo();
    out.push((before === after ? '✗ ' : '✓ ') + label);
  }
  return out.join('\\n');
} catch (e) { return 'ERR ' + e.stack; } })()`));
const b = JSON.parse(await ev(`JSON.stringify(document.querySelector('.insp').getBoundingClientRect())`));
await shot('inspector', { x: b.x, y: b.y, width: b.width, height: Math.min(b.height, 2400) });
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
