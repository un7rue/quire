// The banker's client list: figures, coming up, recent. node dev/shots/clients.mjs <out> [en|ru]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=user]')?.click()`); await sleep(3500);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(1000);
await ev(`(async () => { const a = await import('/ui/areas.js'); const m = await import('/ui/state.js'); m.render(); document.querySelector('.rail [data-area=user], [data-area=user]')?.click(); })()`); await sleep(3000);
console.log(await ev(`document.querySelector('.us-clients')?.innerText.slice(0, 600) || document.body.innerText.slice(0, 300)`));
await shot(`clients-${lang}`);
// a client's details, from the Coming up card
await ev(`document.querySelector('.us-si[data-sev=warn]')?.click()`); await sleep(1200);
await shot(`clients-details-${lang}`);
await ev(`document.querySelector('.us-si[data-sev=high]')?.click()`); await sleep(1200);
await ev(`document.querySelector('.us-det')?.scrollIntoView({ block: 'center' })`); await sleep(500);
await shot(`clients-withdrawal-${lang}`);
// ⌘] closes the details
await ev(`document.dispatchEvent(new KeyboardEvent('keydown', { code: 'BracketRight', key: ']', metaKey: true, ctrlKey: true, bubbles: true }))`); await sleep(600);
console.log('details after ⌘]:', await ev(`document.querySelectorAll('.us-det').length`));
chrome.kill(); process.exit(0);
