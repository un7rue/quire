// The report assistant's brief: highlighted in the pane, the e-mail copy and the PDF pages, from a
// stored answer (no AI is called). node dev/shots/brief.mjs <out> <brief.json> [en|ru]
import { readFileSync, writeFileSync } from 'node:fs';
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const out = process.argv[2] || '.';
const brief = JSON.parse(readFileSync(process.argv[3], 'utf8'));
const lang = process.argv[4] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 940, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
const chat = [{ id: 'b1', title: 'Brief', at: Date.now(), msgs: [{ role: 'user', kind: 'brief', text: '' }, { role: 'ai', kind: 'brief', result: brief, provider: brief.provider, model: brief.model, ms: brief.ms }] }];
await ev(`localStorage.setItem('quire.ai.chats.report', ${JSON.stringify(JSON.stringify(chat))})`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=user]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(600);
await ev(`document.querySelector('.us-row .btn')?.click()`); await sleep(6000);
await ev(`document.getElementById('ai-btn')?.click()`); await sleep(1500);
const pane = `document.querySelector('#aipane-r')`;
await ev(`${pane}.querySelector('[data-a="history"]').click()`); await sleep(500);
await ev(`document.querySelectorAll('.menu .mi, .menu button')[0]?.click()`); await sleep(1000);
await ev(`${pane}.querySelector('.aip-thread').scrollTop = 0`); await sleep(300);
await shot(`brief-pane-${lang}`);
await ev(`(() => { const t = ${pane}.querySelector('.aip-thread'); t.querySelectorAll('details').forEach((d) => (d.open = true)); t.scrollTop = t.scrollHeight; })()`); await sleep(300);
await shot(`brief-pane-end-${lang}`);
// the e-mail copy and the PDF pages, drawn as the app makes them
const mail = await ev(`(async () => { const b = await import('/ui/brief.js'); const m = await import('/ui/state.js'); return b.briefEmail(${JSON.stringify(brief)}, m.S.st); })()`);
writeFileSync(`${out}/brief-mail-${lang}.html`, `<!doctype html><meta charset="utf-8"><body style="margin:24px;background:#fff">${mail}`);
const svgs = JSON.parse(await ev(`(async () => { const b = await import('/ui/brief.js'); const m = await import('/ui/state.js'); return JSON.stringify(b.briefPages(${JSON.stringify(brief)}, m.S.st)); })()`));
svgs.forEach((s, i) => writeFileSync(`${out}/brief-page${i + 1}-${lang}.svg`, s));
console.log('pages', svgs.length);
chrome.kill(); process.exit(0);
