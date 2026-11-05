// The Design tab of one widget per kind in the UHNWI template: its parts in order, as text, plus
// screenshots of the inspector. node dev/shots/inspector.mjs <out> [kinds, e.g. bar,kpi]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const want = (process.argv[3] || 'bar,line,pie,treemap,tiles,kpi,table,waterfall').split(',');
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1400, deviceScaleFactor: 1, mobile: false });
await send('Runtime.enable', {});
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message)); window.addEventListener('unhandledrejection', (e) => window.__errs.push(String(e.reason)))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
for (const kind of want) {
  const r = await ev(`(async () => {
    const s = await import('/ui/state.js');
    const all = s.S.st.doc.pages.flatMap((p, pi) => p.widgets.map((w) => ({ w, pi })));
    const hit = all.find(({ w }) => w.kind === ${JSON.stringify(kind)} && w.binding);
    if (!hit) return 'none';
    s.S.sel = hit.w.id; s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'design' };
    const c = await import('/ui/canvas.js'); c.scrollToPage?.(hit.pi + 1);
    const p = await import('/ui/panels.js'); p.renderRight();
    await new Promise((r) => setTimeout(r, 400));
    const names = [...document.querySelectorAll('.insp-pane .insp-title .is-name')].map((n) => n.textContent);
    return hit.w.id + ' ' + (hit.w.variant || '') + ' → ' + names.join(' · ');
  })()`);
  console.log(kind, ':', r);
  const b = JSON.parse(await ev(`JSON.stringify(document.querySelector('.insp')?.getBoundingClientRect() || null)`) || 'null');
  if (b) await shot(`insp-${kind}`, { x: b.x, y: b.y, width: b.width, height: Math.min(b.height, 1400) });
}
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
