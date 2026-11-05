// Halves without a divider: no vertical divider left between two halves, the halves meet at column 12 (the
// forecast's thirds at 8 and 16), and the pages as drawn. node dev/shots/dividers.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1300, height: 950, deviceScaleFactor: 1.5, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.S.showHome = false; m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
const res = JSON.parse(await ev(`(async () => { const s = await import('/ui/state.js'); const r = await import('/ui/render.js');
  const vertical = s.S.st.doc.pages.flatMap((p, pi) => p.widgets.filter((w) => w.kind === 'divider' && (s.S.st.pages[pi].widgets.find((x) => x.id === w.id)?.eff?.opts || w.opts)?.vertical).map((w) => p.title + '@' + w.x));
  const pages = ['Портфель — кратко', 'Обзор портфеля', 'Факт и прогноз на 12 месяцев'].map((t) => {
    const pi = s.S.st.doc.pages.findIndex((p) => p.title === t); const ws = s.S.st.doc.pages[pi].widgets.filter((w) => w.y >= 5 && w.w < 24);
    const edges = [...new Set(ws.flatMap((w) => [w.x, w.x + w.w]))].filter((e) => e > 0 && e < 24).sort((a, b) => a - b);
    return { t, edges, issues: (s.S.st.pages[pi].issues || []).length, svg: r.pageSvg(s.S.st, pi, { exportMode: true }) }; });
  return JSON.stringify({ vertical, pages }); })()`));
console.log('vertical dividers left:', res.vertical.length ? res.vertical.join(', ') : 'none');
for (const [i, p] of res.pages.entries()) {
  console.log(`${p.t}: widget edges at columns ${p.edges.join(', ')} · ${p.issues} layout issue(s)`);
  await ev(`document.body.innerHTML = ${JSON.stringify('<div style="background:#888;padding:10px;width:1260px">' + p.svg.replace('<svg', '<svg width="1240"') + '</div>')}`); await sleep(700);
  await shot(`halves-${i}`, { x: 0, y: 0, width: 1280, height: 900 });
}
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
