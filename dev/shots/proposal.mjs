// The banker's proposal: the dialog, the page laid out as written, and a fuller page from a stored
// answer (no AI provider is called). node dev/shots/proposal.mjs <out> ["<client>"]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const client = process.argv[3] || 'Draymond Greenway';
const notes = `Снижение доли акций до профиля

Предлагаем вернуть портфель к консервативному профилю: акции сейчас 62% при лимите 40%.

- Продать Банк ВТБ на 12 млн ₽ — доля акций выше профиля
- Продать Роснефть на 8 млн ₽ — зафиксировать результат
- Купить ОФЗ 26238 на 15 млн ₽ — фиксированный доход 7,1%
- Открыть вклад «Капитал» на 5 млн ₽ — для свободных денег

Риски:
- Ставки могут снизиться, доход по вкладу — тоже
- Цены облигаций меняются до погашения

Следующие шаги:
- Обсудить на встрече 5 ноября
- Подписать поручения после согласования`;
const stored = {
  title: 'Вернуть портфель к консервативному профилю',
  lead: 'Сейчас **акции — 62% портфеля** при лимите 40% для Вашего профиля. Предлагаем сократить их и направить средства в облигации и вклад: доход станет **предсказуемее**, риск — ниже.',
  actions: [
    { action: 'Продать', what: 'Банк ВТБ', amount: '12 млн ₽', why: 'Доля акций выше профиля' },
    { action: 'Продать', what: 'Роснефть', amount: '8 млн ₽', why: 'Зафиксировать результат' },
    { action: 'Купить', what: 'ОФЗ 26238', amount: '15 млн ₽', why: 'Фиксированный доход 7,1%' },
    { action: 'Открыть', what: 'Вклад «Капитал»', amount: '5 млн ₽', why: 'Для свободных денег' },
  ],
  effect: ['**Доля акций — около 40%** (сейчас 62%, стр. 4)', '**Регулярный доход** — купоны и проценты', '**Свободные деньги** в работе'],
  reasons: ['**Профиль — консервативный**, а доля акций выше лимита', '**Ставки высоки**: облигации и вклады дают привлекательный доход'],
  risks: ['**Ставки могут снизиться** — доход по вкладу тоже', '**Цены облигаций меняются** до погашения'],
  steps: ['**Встреча 5 ноября** — обсудить предложение', '**Поручения** — подписать после согласования'],
};
await send('Emulation.setDeviceMetricsOverride', { width: 1400, height: 900, deviceScaleFactor: 1.5, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=user]')?.click()`); await sleep(3000);
await ev(`(async () => { const s = await import('/ui/state.js'); await s.api.newDoc('pub:uhnwi', ${JSON.stringify(client)}); })()`); await sleep(5000);
// the dialog, with the notes typed
await ev(`document.querySelector('#proposal')?.click()`); await sleep(800);
await ev(`(() => { const t = document.querySelector('.pr-notes'); t.value = ${JSON.stringify(notes)}; document.querySelector('[name=pr-mode][value=plain]').click(); })()`); await sleep(300);
await shot('proposal-dialog');
await ev(`[...document.querySelectorAll('.modal-foot .btn')].pop().click()`); await sleep(3000);
const pageShot = async (name) => {
  await ev(`(async () => { const r = await import('/ui/render.js'); const s = await import('/ui/state.js');
    const pi = s.S.st.doc.pages.findIndex((p) => String(p.group || '').startsWith('PROPOSAL'));
    window.__pi = pi;
    const svg = r.pageSvg(s.S.st, pi, { exportMode: true });
    const host = document.createElement('div'); host.id = 'shot-host'; host.style.cssText = 'position:fixed;inset:0;z-index:99999;background:#888;padding:10px;overflow:auto';
    host.innerHTML = svg; document.body.append(host);
    const el = host.querySelector('svg'); el.style.width = '1200px'; el.style.height = 'auto'; el.style.display = 'block'; })()`);
  await sleep(1200);
  await shot(name);
  await ev(`document.querySelector('#shot-host')?.remove()`);
};
await pageShot('proposal-plain');
console.log('issues (as written):', await ev(`(async () => JSON.stringify((await import('/ui/state.js')).S.st.pages[window.__pi].issues.map((i) => i.message)))()`));
// the fuller page, as the AI would write it
await ev(`(async () => { const s = await import('/ui/state.js'); await s.edit({ op: 'set_proposal', spec: ${JSON.stringify(stored)}, notes: ${JSON.stringify(notes)} }); })()`); await sleep(2500);
await pageShot('proposal-ai');
console.log('issues (written up):', await ev(`(async () => JSON.stringify((await import('/ui/state.js')).S.st.pages[window.__pi].issues.map((i) => i.message)))()`));
chrome.kill(); process.exit(0);
