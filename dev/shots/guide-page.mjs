// Screenshots of the in-app guide at a chapter: node dev/shots/guide-page.mjs <out dir> <anchor>
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(2500);
await ev(`(async () => { const m = await import('/ui/help.js'); m.showGuide(); })()`); await sleep(2500);
const id = process.argv[3] || '16-ai-assistant';
for (const [n, sel] of [[1, `#${id}`], [2, 'pre.hg-code'], [3, '#make-it-a-report-source, h3#the-sql-assistant']]) {
  await ev(`(() => { const e = document.querySelector(${JSON.stringify(sel)}) || [...document.querySelectorAll('pre.hg-code')][3]; e?.scrollIntoView({ block: 'start' }); })()`); await sleep(500);
  await shot('guide-' + n);
}
chrome.kill(); process.exit(0);
