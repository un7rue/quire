// Series and a right axis on the UHNWI template's column + line chart: the Series section, then the
// line on a right axis with its own style. node dev/shots/series.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1100, deviceScaleFactor: 1.5, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
const pick = `const s = await import('/ui/state.js'); const all = s.S.st.doc.pages.flatMap((pg, pi) => pg.widgets.map((w) => ({ w, pi }))); const hit = all.find(({ w }) => w.kind === 'bar' && (w.binding?.measures || []).some((m) => m.chart === 'line'));`;
const drawW = async (tag) => {
  await ev(`(async () => { ${pick} const r = await import('/ui/render.js');
    const svg = r.widgetSvg(s.S.st, hit.pi, s.S.st.doc.pages[hit.pi].widgets.findIndex((x) => x.id === hit.w.id), { pad: 6 });
    const d = document.createElement('div'); d.id = 'wshot'; d.style.cssText = 'position:fixed;left:0;top:0;z-index:9999;background:#fff;padding:10px'; d.innerHTML = svg.svg || svg;
    document.getElementById('wshot')?.remove(); document.body.append(d); const el = d.querySelector('svg'); el.style.width = '640px'; el.style.height = 'auto'; })()`);
  await sleep(600); await shot(tag, { x: 0, y: 0, width: 680, height: 330 });
  await ev(`document.getElementById('wshot')?.remove()`);
};
await drawW('series-before');
// the line on a right axis, dashed, with markers
console.log(await ev(`(async () => { ${pick}
  const line = hit.w.binding.measures.find((m) => m.chart === 'line');
  await s.edit({ op: 'update_widget', id: hit.w.id, patch: { opts: { viz: { seriesOpts: { [line.id]: { axis: 'right', dash: 'dashed', markers: 'all', name: 'Итог с начала года' } } } } } });
  return hit.w.id + ' ' + line.id;
})()`));
await drawW('series-right');
await ev(`(async () => { ${pick} const p = await import('/ui/panels.js'); s.S.sel = hit.w.id; s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'design' }; p.renderRight(); await new Promise((r) => setTimeout(r, 300)); const sec = document.querySelector('.insp-sec[data-sec="Axes & grid"]'); if (sec?.classList.contains('closed')) sec.querySelector('.insp-title').click(); sec?.scrollIntoView(); })()`);
await sleep(600);
const b = JSON.parse(await ev(`JSON.stringify(document.querySelector('.insp').getBoundingClientRect())`));
await shot('axes-inspector', { x: b.x, y: b.y, width: b.width, height: Math.min(b.height, 1100) });
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
