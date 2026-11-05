// Admin's AI assistant from stored answers (no AI provider is called): a question about the log,
// access set up in words, an access review and a failed event explained. node dev/shots/admin-ai.mjs <out> [en|ru]
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const lang = process.argv[3] || 'en';
await send('Emulation.setDeviceMetricsOverride', { width: 1500, height: 940, deviceScaleFactor: 1, mobile: false });
// light unless SCHEME=dark
await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: process.env.SCHEME || 'light' }] });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`document.querySelector('.lg-demo button[data-l=admin]')?.click()`); await sleep(3000);
await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('${lang}', m.render); })()`); await sleep(600);
// stored answers, shaped as the app's own (the access plan as the app resolves it)
await ev(`(async () => {
  const m = await import('/ui/state.js');
  const ses = (await m.call('work_sessions', { filter: {} }, { quiet: true })).sessions;
  const evs = (await m.call('activity', { filter: { result: 'failed' } }, { quiet: true })).rows;
  const users = (await m.call('auth', { cmd: 'users' }, { quiet: true })).users;
  const w = users.find((u) => u.login === 'user');
  window.__fake = {
    ask: { answer: '**Warren Buffington** worked on 3 reports this week: **Кирилл Шарихин** twice and Anthony Edwardson once; one was exported to PDF. Sign-ins failed twice, both for a login that doesn’t exist (eve).', refs: ['S1', 'S2'], links: ses.slice(0, 2).map((s, i) => ({ type: 'session', id: s.id, label: 'S' + (i + 1) })).concat(evs.slice(0, 1).map((e) => ({ type: 'event', at: e.at, user: e.user, label: 'E4' }))), provider: 'Claude', model: 'claude-sonnet-5-5', ms: 6100 },
    access: { explanation: 'Warren Buffington: **insurance only** for the clients he has now; he joins Private bankers and Insurance desk; AI stays at Figures through his group.', questions: [], provider: 'Claude', model: 'claude-sonnet-5-5', ms: 9400,
      plan: [{ new: false, before: w, after: { ...w, access: { ...w.access, sources: ['INS'] } }, groups_before: ['bankers'], groups: ['bankers', 'insurance'], view_before: { bankers: [], clients: ['Warren Buffington'], groups: ['Private bankers'] }, view_after: { bankers: [], clients: ['Warren Buffington'], groups: ['Private bankers', 'Insurance desk'] }, reports_add: [], reports_remove: [], password: '', problems: [] },
        { new: true, before: null, after: { login: 'apetrova', name: 'Anna Petrova', email: '', roles: { admin: false, data: false, design: false, user: true }, access: { all: false, coverage: ['s01'], sources: [], clients: [] }, active: true, must_change: true, ai_level: '' }, groups_before: [], groups: ['bankers'], view_before: null, view_after: { bankers: ['Ivan Sokolov'], clients: [], groups: ['Private bankers'] }, reports_add: ['uhnwi'], reports_remove: [], password: 'k7m2xq9pza', problems: [] }] },
    review: { summary: 'Four people, one administrator. **All four are still on the demo password**, and one banker may send client figures without having used the report assistant.', provider: 'Claude', model: 'claude-sonnet-5-5', ms: 11800,
      findings: [{ severity: 'high', title: 'Demo passwords', detail: 'Everyone, including the administrator, still signs in with the demo password.', login: 'admin', action: 'open', links: [] },
        { severity: 'medium', title: 'Sign-ins for a login that doesn’t exist', detail: '“eve” failed to sign in twice this week.', login: null, action: 'none', links: evs.slice(0, 1).map((e) => ({ type: 'event', at: e.at, user: e.user, label: 'E4' })) },
        { severity: 'low', title: 'Figures without use', detail: 'Warren Buffington may send client figures (Private bankers) but used the report assistant once in 30 days.', login: 'user', action: 'ai_structure', links: [] }] },
    explain: { cause: 'Someone tried to sign in as **eve**, a login that doesn’t exist in Quire — a typo, or a person without an account.', fix: 'If eve should have access, add her in Users & roles; otherwise nothing to do. Repeated tries from the same computer are worth a word with IT.', refs: [], links: [], provider: 'Claude', model: 'claude-sonnet-5-5', ms: 3900 },
  };
  const inv = window.__QUIRE__.invoke;
  window.__QUIRE__.invoke = async (cmd, args) => { if (cmd === 'ai_admin') { await new Promise((r) => setTimeout(r, 300)); return window.__fake[args.kind]; } const r = await inv(cmd, args); if (cmd === 'ai_status') r.providers = r.providers.map((p) => p.id === 'claude' ? { ...p, personal: true } : p); return r; };
})()`);
await ev(`document.querySelector('[data-area="admin"]')?.click()`); await sleep(1200);
await ev(`document.querySelector('.adm-item[data-part="activity"]')?.click()`); await sleep(2500);
// 1) a question about the log
await ev(`document.querySelector('[data-a="askai"]')?.click()`); await sleep(800);
await ev(`(() => { const t = document.querySelector('.adm-ai textarea'); t.value = 'What did Warren do this week, and were there failed sign-ins?'; document.querySelector('.adm-ai [data-a="send"]').click(); })()`); await sleep(1500);
await shot(`admin-ai-ask-${lang}`);
// 2) access in words
await ev(`document.querySelector('.adm-ai [data-m="access"]')?.click()`); await sleep(400);
await ev(`(() => { const t = document.querySelector('.adm-ai textarea'); t.value = 'Warren: insurance only, add him to Insurance desk. Add Anna Petrova (apetrova) for the clients of Ivan Sokolov, Private bankers, with the UHNWI report.'; document.querySelector('.adm-ai [data-a="send"]').click(); })()`); await sleep(1500);
await shot(`admin-ai-access-${lang}`);
await ev(`document.querySelector('.adm-ai [data-a="close"]')?.click()`); await sleep(300);
// 3) the access review
await ev(`document.querySelector('.adm-item[data-part="access"]')?.click()`); await sleep(2500);
await ev(`document.querySelector('[data-a="review"]')?.click()`); await sleep(1500);
await shot(`admin-ai-review-${lang}`);
// 4) a failed event, explained
await ev(`document.querySelector('.adm-item[data-part="activity"]')?.click()`); await sleep(1500);
await ev(`document.querySelector('.lg-view button[data-v="events"]')?.click()`); await sleep(2000);
await ev(`(() => { const s = document.querySelector('select[name=result]'); s.value = 'failed'; s.dispatchEvent(new Event('change')); })()`); await sleep(2000);
await ev(`document.querySelector('.lg-t tr[data-k]')?.click()`); await sleep(600);
await ev(`document.querySelector('[data-explain]')?.click()`); await sleep(1200);
await shot(`admin-ai-explain-${lang}`);
// 5) the AI page: cost and tips
await ev(`document.querySelector('.adm-item[data-part="ai"]')?.click()`); await sleep(2500);
await shot(`admin-ai-cost-${lang}`);
chrome.kill(); process.exit(0);
