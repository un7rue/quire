// Click-a-part: on the UHNWI template, selects a widget, hovers and clicks each of its parts
// (legend, axis, a series, labels, a table column, a KPI's value…) and reports which inspector
// part opened. node dev/shots/parts.mjs <out>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(5000);
const mouse = async (type, x, y) => send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 });
const click = async (x, y) => { await mouse('mouseMoved', x, y); await sleep(150); await mouse('mousePressed', x, y); await mouse('mouseReleased', x, y); await sleep(700); };
// [widget kind, part]: the first widget of that kind having that part
const cases = [['waterfall', 'legend'], ['waterfall', 'axis'], ['waterfall', 'labels'], ['pie', 'centre'], ['pie', 'legend'], ['pie', 'series:'], ['bar', 'col:'], ['table', 'col:'], ['kpi', 'value'], ['kpi', 'title'], ['tiles', 'series:'], ['treemap', 'series:']];
let n = 0;
for (const [kind, part] of cases) {
  const where = JSON.parse(await ev(`(async () => {
    const s = await import('/ui/state.js');
    for (const [pi, p] of s.S.st.doc.pages.entries()) for (const w of p.widgets) {
      if (w.kind !== ${JSON.stringify(kind)}) continue;
      const c = await import('/ui/canvas.js'); c.scrollToPage?.(pi + 1);
      await new Promise((r) => setTimeout(r, 500));
      const g = [...document.querySelectorAll('[data-w="' + w.id + '"] [data-part]')].find((x) => x.dataset.part.startsWith(${JSON.stringify(part)}));
      if (!g) continue;
      g.scrollIntoView({ block: 'center' }); await new Promise((r) => setTimeout(r, 300));
      const r = [...g.querySelectorAll('*')].map((x) => x.getBoundingClientRect()).find((b) => b.width > 2 && b.height > 2) || g.getBoundingClientRect();
      const hit = document.querySelector('.hit[data-id="' + w.id + '"]')?.getBoundingClientRect();
      return JSON.stringify({ id: w.id, part: g.dataset.part, x: r.left + r.width / 2, y: r.top + r.height / 2, hx: hit ? hit.left + 4 : 0, hy: hit ? hit.top + 4 : 0 });
    }
    return 'null';
  })()`) || 'null');
  if (!where) { console.log(kind, part, '→ no such part'); continue; }
  await click(where.hx, where.hy);                 // select the widget first
  await mouse('mouseMoved', where.x, where.y); await sleep(250);
  const hl = await ev(`document.querySelector('.part-hl span')?.textContent || '(no outline)'`);
  if (n++ < 4) await shot(`hover-${kind}-${part.replace(/[:]/g, '')}`, { x: Math.max(0, where.x - 300), y: Math.max(0, where.y - 200), width: 600, height: 400 });
  await click(where.x, where.y);
  const got = await ev(`(() => { const c = document.querySelector('.insp .locate'); const sec = c?.closest('.insp-sec')?.dataset.sec || c?.dataset?.sec; const col = document.querySelector('.insp .ih-t span')?.textContent; return (sec || '') + (document.querySelector('.ci-head, .col-insp') ? ' [column inspector]' : '') + ' | head: ' + col; })()`);
  console.log(`${kind} ${where.part}: hover “${hl}” → ${got}`);
  await ev(`(async () => { const s = await import('/ui/state.js'); s.S.colEdit = null; })()`);
}
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
