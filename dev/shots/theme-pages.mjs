// Template pages in a theme from a file. node dev/shots/theme-pages.mjs <out> <theme file or builtin:id> <page numbers>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const theme = process.argv[3];
const nums = (process.argv[4] || '1').split(',').map(Number);
await send('Emulation.setDeviceMetricsOverride', { width: 1240, height: 880, deviceScaleFactor: 1.5, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); m.setState(await m.call('new_doc', { kind: 'uhnwi_report' })); m.setState(await m.call('apply_theme', { id: ${JSON.stringify(theme.includes(':') ? theme : 'file:' + theme)} })); })()`); await sleep(5000);
for (const n of nums) {
  await ev(`(async () => { const r = await import('/ui/render.js'); const s = await import('/ui/state.js');
    const svg = r.pageSvg(s.S.st, ${n - 1}, { exportMode: true });
    document.body.innerHTML = '<div style="background:#888;padding:10px;min-height:100vh">' + svg + '</div>';
    const el = document.querySelector('svg'); el.style.width = '1200px'; el.style.height = 'auto'; el.style.display = 'block'; })()`);
  await sleep(1500);
  if (process.env.SVG) { const fs = await import('node:fs'); fs.writeFileSync(`${process.argv[2]}/page${n}.svg`, await ev(`document.querySelector('svg').outerHTML`)); }
  else await shot(`page${n}`);
}
chrome.kill(); process.exit(0);
