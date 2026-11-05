// The design assistant on screen, with a stored answer (no AI provider is called): the plan
// below goes through the real "ai_page" edit, so the page is built as the app builds it.
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(500);
await ev(`localStorage.removeItem('quire.ai.chats.design')`);
const ru = lang === 'ru';
const plan = [
  { kind: 'heading', x: 0, y: 0, w: 24, h: 2, text: 'Обзор портфеля', variant: 'page' },
  { kind: 'kpi', x: 0, y: 2, w: 8, h: 4, title: 'Итого активы на {{end}}, ₽', variant: 'hero', binding: { dataset: 'port', measures: [{ field: 'value', agg: 'sum' }], window: 'all' } },
  { kind: 'kpi', x: 8, y: 2, w: 8, h: 4, title: 'Результат в деньгах', variant: 'card', binding: { dataset: 'port', measures: [{ field: 'income', agg: 'sum', format: 'currency' }], window: 'all' } },
  { kind: 'kpi', x: 16, y: 2, w: 8, h: 4, title: 'Доходность за период', variant: 'card', binding: { dataset: 'port', measures: [{ field: 'ret', agg: 'sum', format: 'percent' }], window: 'all' } },
  { kind: 'pie', x: 0, y: 6, w: 10, h: 11, title: 'Классы активов', variant: 'ring', binding: { dataset: 'cls', dims: ['class'], measures: [{ field: 'value', agg: 'sum' }], window: 'all', sort: 'value_desc' } },
  { kind: 'table', x: 10, y: 6, w: 14, h: 11, title: 'Крупнейшие позиции', variant: 'lines', binding: { dataset: 'top5', dims: ['name'], measures: [{ field: 'value', agg: 'sum' }, { field: 'share', agg: 'sum', format: 'percent' }], window: 'all', sort: 'value_desc' } },
  { kind: 'bar', x: 0, y: 17, w: 24, h: 7, title: 'Доход по месяцам', variant: 'monthly', binding: { dataset: 'mon', dims: ['date:month'], measures: [{ field: 'income', agg: 'sum' }], window: 'all' } },
];
const full = (w) => ({ ...w, opts: w.text ? { text: w.text } : {}, binding: w.binding ? { dims: [], filters: [], sort: '', limit: 0, others: false, mode: '', columns: [], pivot: '', where_sql: '', ...w.binding, measures: w.binding.measures.map((m) => ({ label: '', format: 'number', calc: '', ...m })) } : null });
await ev(`(async () => {
  const m = await import('/ui/state.js');
  m.setState(await m.call('new_doc', { kind: 'uhnwi_report' }));
  m.setState(await m.call('apply_theme', { id: 'builtin:quire' })); // the guide's pictures are in Quire
  const inv = window.__QUIRE__.invoke;
  window.__QUIRE__.invoke = async (cmd, args) => {
    if (cmd === 'ai_status') { const r = await inv(cmd, args); r.providers = r.providers.map((p) => p.id === 'claude' ? { ...p, personal: true } : p); r.settings = { ...(r.settings || {}), default: 'claude' }; return r; }
    if (cmd !== 'ai_design') return inv(cmd, args);
    await new Promise((r) => setTimeout(r, 600));
    const before = m.S.st.doc.pages.map((p) => p.id);
    const st = await inv('edit', { op: { op: 'ai_page', page: null, after: args.page, title: 'Обзор портфеля', widgets: ${JSON.stringify(plan.map(full))} } });
    const np = st.doc.pages.find((p) => !before.includes(p.id));
    return { st, ok: true, page: np.id, title: np.title, added: ${plan.length}, changed: 0, removed: 0, issues: [], plan: ${JSON.stringify(plan)},
      explanation: ${JSON.stringify(ru ? 'Страница-обзор: три главные цифры в ряд, структура по классам активов кольцом, крупнейшие позиции таблицей и доход по месяцам столбиками.' : 'An overview page: three headline figures in a row, allocation by asset class as a ring, the largest holdings as a table and income by month as bars.')},
      provider: 'Claude', model: 'claude-sonnet-5-5', attempts: 1, ms: 7400 };
  };
})()`); await sleep(3500);
await ev(`document.getElementById('ai-btn')?.click()`); await sleep(1500);
await shot('design-empty-' + lang);
const ask = ru ? 'Страница-обзор: активы, результат и доходность карточками, классы активов кольцом, крупнейшие позиции таблицей, доход по месяцам' : 'An overview page: total assets, result and return as KPI cards, allocation by asset class as a donut, the largest holdings in a table and income by month';
await ev(`(() => { const t = document.querySelector('#aipane textarea'); t.value = ${JSON.stringify(ask)}; t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
await sleep(5000);
await shot('design-page-' + lang);
console.log(await ev(`JSON.stringify({ page: document.querySelector('#aipane .aip-src')?.textContent, undo: !!document.querySelector('#aipane [data-act=undo]'), err: (window.__logs||[]).slice(-3) })`));
chrome.kill(); process.exit(0);
