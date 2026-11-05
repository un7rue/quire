import { send, ev, shot, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('en', m.render); })()`); await sleep(500);
const meta = { provider: 'Claude', model: 'claude-sonnet-5-5' };
const write = [
  { role: 'user', kind: 'write', text: 'Value of each client’s portfolio at the end of the period, by asset class', sql: '' },
  { role: 'ai', kind: 'write', sql: "select\n  c.name as client,\n  i.instr_type as asset_class,\n  sum(b.amt_rub) as value_rub\nfrom balances b\njoin clients c on c.id = b.client_id\njoin instruments i on i.instr_cd = b.instr_cd\nwhere b.report_dt = :period_end\ngroup by 1, 2\norder by 1, 3 desc", explanation: 'Each client’s positions on the last day of the period, summed in roubles by asset class, largest first.', ok: true, columns: ['client','asset_class','value_rub'], rows: 230, attempts: 1, ms: 3400, ...meta },
  { role: 'user', kind: 'write', text: 'Only clients with more than 100 million in total', sql: '' },
  { role: 'ai', kind: 'write', sql: "with v as (\n  select\n    c.name as client,\n    i.instr_type as asset_class,\n    sum(b.amt_rub) as value_rub\n  from balances b\n  join clients c on c.id = b.client_id\n  join instruments i on i.instr_cd = b.instr_cd\n  where b.report_dt = :period_end\n  group by 1, 2\n)\nselect *\nfrom v\nwhere client in (\n  select client from v group by client having sum(value_rub) > 100000000\n)\norder by 1, 3 desc", explanation: 'The same breakdown, keeping only clients whose whole portfolio is above 100 million roubles.', ok: true, columns: ['client','asset_class','value_rub'], rows: 64, attempts: 1, ms: 4100, ...meta },
];
const source = [
  { role: 'user', kind: 'write', text: 'Where are FX rates kept, and how do they join to positions?', sql: '' },
  { role: 'ai', kind: 'write', sql: '', explanation: 'Rates are in `fx_rates` (`report_dt`, `cur`, `rate` — roubles per unit). Positions in `balances` carry their currency in `price_cur`: join on `f.cur = b.price_cur and f.report_dt = b.report_dt`. `balances` already holds the day’s rate in `fx_rate` and the value in roubles in `amt_rub`.', ok: true, attempts: 1, ms: 2300, ...meta },
  { role: 'user', kind: 'source', text: '', sql: "select b.acct_cd, b.instr_cd, b.amt_rub from balances b join accounts a on a.acct_cd = b.acct_cd where b.client_id = 'C1337' and b.report_dt = '2026-06-30' and a.acct_type = 'Брокерский счёт'" },
  { role: 'ai', kind: 'source', sql: "select\n  b.acct_cd as account,\n  b.report_dt as value_date,\n  b.instr_cd as instrument,\n  b.amt_rub as value_rub\nfrom balances b\njoin accounts a on a.acct_cd = b.acct_cd\nwhere b.client_id = :client\n  and b.report_dt = :period_end\n  and a.acct_type = 'Брокерский счёт'", explanation: 'The client’s brokerage positions at the end of the period, in roubles.', ok: true, columns: ['account','value_date','instrument','value_rub'], rows: 23, attempts: 1, ms: 3800, name: 'Brokerage positions at period end', date: 'value_date', changes: ["'C1337' → :client", "'2026-06-30' → :period_end"], left: ["'Брокерский счёт' — the account type is what this source is about"], ...meta },
];
const chats = [{ id: 'c2', title: 'src', at: Date.now(), msgs: source }, { id: 'c1', title: 'Value of each client’s portfolio', at: Date.now() - 6e5, msgs: write }];
await ev(`localStorage.setItem('quire.ai.chats.demo', ${JSON.stringify(JSON.stringify(chats))});`);
// for the pictures only: the page is told Claude has a key (nothing is sent to any AI)
await ev(`(() => { const inv = window.__QUIRE__.invoke; window.__QUIRE__.invoke = async (cmd, args) => { const r = await inv(cmd, args); if (cmd === 'ai_status') { const c = r.providers.find((p) => p.id === 'claude'); c.personal = true; } return r; }; })()`);
// screenshots show a set-up provider (no key is used: nothing is sent)
await ev(`(() => { const inv = window.__QUIRE__.invoke; window.__QUIRE__.invoke = async (cmd, args) => { const r = await inv(cmd, args); if (cmd === 'ai_status') { r.providers = r.providers.map((p) => p.id === 'claude' ? { ...p, personal: true } : p); r.settings = { ...(r.settings || {}), default: 'claude' }; } return r; }; })()`);
await ev(`document.querySelector('[data-area="data"]')?.click()`); await sleep(2500);
await ev(`[...document.querySelectorAll('.dx-row')].find(b => b.textContent.includes('Demo bank'))?.click()`); await sleep(1200);
await ev(`window.dispatchEvent(new KeyboardEvent('keydown', { key: 'i', metaKey: true, bubbles: true }))`); await sleep(3000);
const pane = `[...document.querySelectorAll('.dbx-aip')].find(x => !x.hidden)`;
const pickChat = async (n) => { await ev(`${pane}.querySelector('[data-a="history"]').click()`); await sleep(500); await ev(`document.querySelectorAll('.menu .mi, .menu button')[${n}]?.click()`); await sleep(800); };
// 1) writing: use the first answer so the results show
await pickChat(1);
await ev(`${pane}.querySelectorAll('[data-act="apply"]')[0].click()`); await sleep(3000);
await ev(`(() => { const t = ${pane}.querySelector('.aip-thread'); t.scrollTop = 0; })()`); await sleep(300);
await shot('49-ai-pane');
// 2) schema question + source
await pickChat(0);
await ev(`(() => { const t = ${pane}.querySelector('.aip-thread'); t.scrollTop = 0; })()`); await sleep(300);
await shot('50-ai-question');
await ev(`(() => { const t = ${pane}.querySelector('.aip-thread'); t.scrollTop = t.scrollHeight; })()`); await sleep(300);
await shot('51-ai-source');
// 3) Settings → AI assistant
await ev(`document.dispatchEvent(new CustomEvent('open-settings', { detail: 'ai' }))`); await sleep(2500);
await shot('52-ai-settings');
await ev(`document.querySelector('.modal-foot button, .modal button')?.closest('.modal')?.querySelector('button:not(.primary)')?.click()`); await sleep(600);
await ev(`document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`); await sleep(400);
// 4) the top-bar button where a role has no assistant yet: what AI offers there
console.log(await ev(`[...document.querySelectorAll('#rail [data-area]')].map((b) => b.dataset.area).join(',')`));
await ev(`document.querySelector('#rail [data-area="reports"], #rail [data-area="user"]')?.click()`); await sleep(2500);
await ev(`document.getElementById('ai-btn')?.click()`); await sleep(1500);
await shot('53-ai-button');
chrome.kill(); process.exit(0);
