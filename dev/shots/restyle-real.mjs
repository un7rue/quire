// A real restyle (calls the AI provider set up in Settings — costs a request): the UHNWI
// template, pictures attached through the Look tab, pages shot before and after.
// node dev/shots/restyle-real.mjs <out dir> <picture file> [words]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const out = process.argv[2], file = process.argv[3], words = process.argv[4] || '';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`localStorage.removeItem('quire.ai.chats.design')`);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); })()`); await sleep(4000);
const pages = (process.env.PAGES || '3,6,10').split(',').map(Number);
const shoot = async (tag) => {
  for (const p of pages) {
    await ev(`(async () => { const c = await import('/ui/canvas.js'); c.scrollToPage(${p}); })()`); await sleep(1800);
    const r = await ev(`JSON.stringify(document.querySelector('.page-wrap[data-pi="${p}"] .page, .page-wrap[data-pi="${p}"]')?.getBoundingClientRect())`);
    const b = JSON.parse(r || 'null');
    if (b) await shot(`${tag}-p${p}`, { x: Math.max(0, b.x), y: Math.max(0, b.y), width: Math.min(b.width, 1600), height: Math.min(b.height, 1000 - Math.max(0, b.y)) });
  }
};
// zoom so a page fills the canvas
await ev(`document.querySelector('[data-a="zoom-in"], #zin')?.click()`); await sleep(500);
await shoot('before');
await ev(`document.getElementById('ai-btn')?.click()`); await sleep(1200);
await ev(`document.querySelector('#aipane [data-m=look]').click()`); await sleep(400);
const doc = await send('DOM.getDocument', {});
const q = await send('DOM.querySelector', { nodeId: doc.result.root.nodeId, selector: '#aipane input[type=file]' });
await send('DOM.setFileInputFiles', { nodeId: q.result.nodeId, files: [file] });
for (let i = 0; i < 60; i++) { await sleep(1000); if (await ev(`!!document.querySelector('#aipane .aip-pk')`)) break; }
await sleep(1000);
console.log('pictures', await ev(`document.querySelectorAll('#aipane .aip-pk').length`));
await shot('pane-attached', { x: 1160, y: 0, width: 440, height: 1000 });
await ev(`(() => { const t = document.querySelector('#aipane textarea'); t.value = ${JSON.stringify(words)}; document.querySelector('#aipane .aip-send').click(); })()`);
const t0 = Date.now();
for (let i = 0; i < 400; i++) { await sleep(2000); if (!(await ev(`!!document.querySelector('#aipane .aip-msg.wait')`))) break; }
console.log('seconds', Math.round((Date.now() - t0) / 1000));
await sleep(3000);
await shot('pane-answer', { x: 1160, y: 0, width: 440, height: 1000 });
console.log(await ev(`(() => { const m = [...document.querySelectorAll('#aipane .aip-msg.ai')].pop(); return m ? m.innerText.slice(0, 3000) : 'none'; })()`));
await ev(`document.querySelector('#aipane [data-a=close]')?.click()`); await sleep(1500);
await shoot('after');
chrome.kill(); process.exit(0);
