// The Look mode on screen with a stored answer (no AI provider is called): a drawn reference
// picture is attached through the file button; the answer applies a built-in theme.
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(500);
await ev(`localStorage.removeItem('quire.ai.chats.design')`);
const look = { colors: { page: '#eef3fb', card: '#ffffff', ink: '#141414', accent: '#21a038', good: '#1a9e32', bad: '#d92b3a', palette: ['#21a038', '#0e7c86', '#42e3b4', '#8fd0ff', '#2b5aa8', '#f2c94c', '#9aa5b1', '#d92b3a'] },
  type: { display: 'IBM Plex Sans', ui: 'Inter' }, shape: { bar_radius: 4, grid: 'dashed', cards: 'filled' } };
await ev(`(async () => {
  const m = await import('/ui/state.js');
  m.setState(await m.call('new_doc', { kind: 'uhnwi_report' }));
  m.setState(await m.call('apply_theme', { id: 'builtin:quire' })); // the guide's pictures are in Quire
  const inv = window.__QUIRE__.invoke;
  window.__QUIRE__.invoke = async (cmd, args) => {
    if (cmd === 'ai_status') { const r = await inv(cmd, args); r.providers = r.providers.map((p) => p.id === 'claude' ? { ...p, personal: true } : p); r.settings = { ...(r.settings || {}), default: 'claude' }; return r; }
    if (cmd !== 'ai_restyle') return inv(cmd, args);
    window.__sentImage = args.images.reduce((n, i) => n + i.data.length, 0);
    await new Promise((r) => setTimeout(r, 600));
    const st = await inv('apply_theme', { id: 'builtin:green' });
    return { st, ok: true, name: 'Зелёный банк', look: ${JSON.stringify(look)}, attempts: 1, ms: 9100, provider: 'Claude', model: 'claude-sonnet-5-5',
      explanation: ${JSON.stringify(lang === 'ru' ? 'Светло-голубая бумага, белые карточки с крупными скруглениями, зелёный фирменный цвет и пунктирная сетка; шрифт — IBM Plex Sans.' : 'Pale blue paper, white cards with large corners, a green brand colour and a dashed grid; the typeface is IBM Plex Sans.')} };
  };
})()`); await sleep(3500);
await ev(`document.getElementById('ai-btn')?.click()`); await sleep(1200);
await ev(`document.querySelector('#aipane [data-m=look]').click()`); await sleep(400);
await shot('restyle-empty-' + lang);
// a reference picture: drawn on a canvas, attached through the file input
await ev(`(async () => {
  const c = document.createElement('canvas'); c.width = 1200; c.height = 800; const g = c.getContext('2d');
  g.fillStyle = '#eef3fb'; g.fillRect(0, 0, 1200, 800); g.fillStyle = '#fff'; g.beginPath(); g.roundRect(60, 60, 1080, 680, 28); g.fill();
  g.fillStyle = '#141414'; g.font = '48px sans-serif'; g.fillText('Annual report 2025', 110, 150);
  ['#21a038', '#42e3b4', '#0e7c86', '#8fd0ff', '#21a038'].forEach((col, i) => { g.fillStyle = col; g.fillRect(140 + i * 170, 640 - (i + 2) * 70, 90, (i + 2) * 70); });
  const blob = await new Promise((r) => c.toBlob(r, 'image/png'));
  const dt = new DataTransfer(); dt.items.add(new File([blob], 'ref.png', { type: 'image/png' }));
  const inp = document.querySelector('#aipane input[type=file]'); inp.files = dt.files; inp.dispatchEvent(new Event('change'));
})()`); await sleep(1500);
await ev(`(() => { const t = document.querySelector('#aipane textarea'); t.value = ${JSON.stringify(lang === 'ru' ? 'Цвета и формы графиков, наши шрифты оставить' : 'Take the colours and the chart shapes')}; t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
await sleep(5000);
await shot('restyle-' + lang);
console.log(await ev(`(async () => JSON.stringify({ sent: window.__sentImage, theme: (await import('/ui/state.js')).S.st.doc.theme.name, undo: !!document.querySelector('#aipane [data-act=undo]'), logs: (window.__logs||[]).slice(-3) }))()`));
chrome.kill(); process.exit(0);
