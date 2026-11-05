// Every built-in widget opened in the studio looks as its preview on the Widgets tab, and the studio comes to the
// front. node dev/shots/widget-open.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
// compare: the gallery's drawing of each built-in widget against the studio's (texts on them)
console.log(await ev(`(async () => {
  const s = await import('/ui/state.js'); const r = await import('/ui/render.js');
  const g = await s.call('auth', { cmd: 'widget_gallery', args: {} });
  const words = (svg) => [...String(svg?.svg || svg).matchAll(/>([^<>]+)<\\/text>/g)].map((m) => m[1]).filter((t) => t.trim()).slice(0, 40).join('|');
  const bad = [];
  let n = 0;
  for (const it of g.items.filter((x) => x.group === 'builtin')) {
    const a = words(r.widgetSvg(g.state, it.page, 0, { pad: 4 }));
    const st = await s.call('auth', { cmd: 'widget_open', args: { kind: it.kind, view: it.view } });
    const b = words(r.widgetSvg(st, 0, 0, { pad: 4 }));
    n++;
    // the same data and texts (the studio's widget is bigger, so a few labels may fit differently)
    const sa = new Set(a.split('|')), sb = new Set(b.split('|'));
    const common = [...sa].filter((x) => sb.has(x)).length;
    if (common < Math.min(sa.size, sb.size) * 0.5) bad.push(it.kind + '/' + it.view + ' (' + it.name + ')');
    await s.call('doc_close', { id: st.active_id || st.id || 0 }, { quiet: true }).catch(() => {});
  }
  return n + ' compared · differ: ' + (bad.join(', ') || 'none');
})()`));
// clicking a row brings the studio to the front
await ev(`document.querySelector('.dh-tab[data-v=widgets]')?.click()`); await sleep(6000);
await ev(`[...document.querySelectorAll('.dh-wrow')].find((r) => r.textContent.includes('По месяцам и итог'))?.click()`); await sleep(4000);
console.log('front:', await ev(`(async () => { const s = await import('/ui/state.js'); return 'home hidden ' + document.getElementById('home').hidden + ' · ' + s.S.st.doc.meta.title; })()`));
await shot('opened-monthly');
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
