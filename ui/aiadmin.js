// The administrator's AI assistant, a drawer on the right of the Admin area (the AI button or ⌘I):
// questions about the activity log, answered with links to the sessions and events they rest on,
// and access set up in words — a proposal shown as before/after, applied only when confirmed
// (through the same saves as the Users page, so each change is in the activity log). People,
// bankers and clients go to the model as placeholders.

import { S, call, render } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { toast, modal } from './ui.js';
import { appLanguage } from './i18n.js';
import { providerChip, feedbackHtml } from './aichat.js';
import { rich } from './brief.js';
import { srcName } from './areas.js';

const MODES = [
  ['ask', 'clock', 'Ask the log', ['Who exported PDFs in the last 7 days?', 'What did each banker do yesterday?', 'Why did sign-ins fail this week?', 'Who used the most AI this month, and for what?']],
  ['access', 'lock', 'Change access', ['Give Warren Buffington insurance clients only', 'Add Anna Petrova (apetrova) as a banker with the Private bankers group', 'Who may send client figures to AI? Make it Structure only for everyone outside Private bankers']],
];

let drawer = null, host = null;

/** The Admin page puts the drawer on itself: it goes when another area is shown and comes back, conversation and all. */
export function mountAdminAI(el) {
  host = el;
  if (drawer) { el.append(drawer.el); document.body.classList.toggle('adm-ai-on', !drawer.el.hidden); }
  else document.body.classList.remove('adm-ai-on');
}

/** Opens or closes the drawer (Admin area only); `mode`: "ask" or "access". */
export function toggleAdminAI(mode) {
  if (!drawer) drawer = build();
  if (host && drawer.el.parentNode !== host) host.append(drawer.el);
  if (!drawer.el.hidden && !mode) { drawer.close(); return; }
  drawer.open(mode);
}

/** Opens a link from an answer: a session or an event, in Admin → Activity. */
export function openLink(l) {
  S.adminTab = 'activity';
  if (l.type === 'session') { S.actView = 'sessions'; S.openSession = l.id; }
  else { S.actView = 'events'; S.openEvent = { at: l.at, user: l.user }; S.actFilter = { user: l.user, area: '', result: '', from: (l.at || '').slice(0, 10), to: (l.at || '').slice(0, 10), q: '' }; }
  render();
}

/** Links an answer rests on, as chips. */
export const linksHtml = (links) => (links?.length ? `<div class="aa-links">${links.map((l, i) => `<button class="aa-link" data-link="${i}">${icon(l.type === 'session' ? 'clock' : 'table', 11)} ${esc(l.type === 'session' ? 'Session' : 'Event')} ${esc(l.label)}</button>`).join('')}</div>` : '');
export const bindLinks = (root, links) => root.querySelectorAll('[data-link]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); openLink(links[+b.dataset.link]); }));

function build() {
  const el = h(`<aside class="adm-ai" hidden>
    <div class="aip-head"><span class="aip-ic">${icon('sparkles', 15)}</span><b>Admin assistant</b>
      <button class="aip-model" title="Which AI answers — click to change">…</button><span class="grow"></span>
      <button class="tb-btn" data-a="new" title="New conversation">${icon('plus', 15)}</button>
      <button class="tb-btn" data-a="close" title="Close (⌘I)">${icon('x', 15)}</button></div>
    <div class="seg aa-modes">${MODES.map(([k, ic, label]) => `<button data-m="${k}">${icon(ic, 13)} ${esc(label)}</button>`).join('')}</div>
    <div class="aip-thread"></div>
    <div class="aip-compose"><textarea rows="3"></textarea><button class="btn primary sm" data-a="send">${icon('send', 13)} Send</button></div>
    <div class="aip-note">${icon('lock', 11)} <span>People, bankers and clients go to the AI as codes, never by name; logins and client codes stay here. Nothing changes until you apply it.</span></div>
  </aside>`);
  const $ = (s) => el.querySelector(s);
  const thread = $('.aip-thread'), ta = $('textarea');
  const chip = providerChip($('.aip-model'));
  const chats = { ask: [], access: [] };
  let mode = 'ask', busy = false;

  const placeholder = () => (mode === 'ask' ? 'Ask about what people did — e.g. “who opened Шарихин’s report this week?”' : 'Say what to change — e.g. “give Warren insurance clients only and the Private bankers group”');
  const draw = () => {
    el.querySelectorAll('[data-m]').forEach((b) => b.classList.toggle('on', b.dataset.m === mode));
    ta.placeholder = placeholder();
    const msgs = chats[mode];
    if (!msgs.length) {
      const m = MODES.find((x) => x[0] === mode);
      thread.innerHTML = `<div class="aip-empty">${icon(m[1], 22)}<b>${esc(m[2])}</b><span>${mode === 'ask' ? 'Questions about sign-ins, sessions, edits, exports and AI use in the last 30 days — answered with links to what they rest on.' : 'Describe the change in your words. You’ll see exactly what changes for whom before anything is saved.'}</span>
        <div class="aip-ex">${m[3].map((q) => `<button class="aip-chip" data-q="${esc(q)}">${esc(q)}</button>`).join('')}</div></div>`;
      thread.querySelectorAll('[data-q]').forEach((b) => b.addEventListener('click', () => { ta.value = b.dataset.q; ta.focus(); }));
      return;
    }
    thread.innerHTML = msgs.map((m, i) => (m.role === 'user' ? `<div class="aip-msg me"><div>${esc(m.text)}</div></div>`
      : m.failed ? `<div class="aip-msg ai bad">${icon('warn', 14)}<div>${esc(m.failed)}</div></div>`
      : `<div class="aip-msg ai report" data-i="${i}">${mode === 'ask' ? askHtml(m.r) : accessHtml(m)}<div class="aip-meta">${esc(m.r.provider || '')} · ${esc(m.r.model || '')}${m.r.ms ? ` · ${(m.r.ms / 1000).toFixed(1)} s` : ''}${feedbackHtml(m.r.log_at)}</div></div>`)).join('')
      + (busy ? `<div class="aip-msg ai wait"><span class="spin"></span>${mode === 'ask' ? 'Reading the log…' : 'Working out the changes…'}</div>` : '');
    thread.querySelectorAll('.aip-msg.ai[data-i]').forEach((box) => {
      const m = msgs[+box.dataset.i];
      bindLinks(box, m.r.links || []);
      box.querySelector('[data-a="apply"]')?.addEventListener('click', () => apply(m));
      box.querySelector('[data-a="discard"]')?.addEventListener('click', () => { m.discarded = true; draw(); });
    });
    thread.scrollTop = thread.scrollHeight;
  };

  const askHtml = (r) => `<p class="aa-answer">${rich(r.answer || '')}</p>${linksHtml(r.links)}`;

  // a proposed change: for each person, what changes (before → after), and what's unclear
  const accessHtml = (m) => {
    const r = m.r, plan = r.plan || [];
    const bad = plan.some((c) => c.problems?.length);
    return `${r.explanation ? `<p>${rich(r.explanation)}</p>` : ''}
      ${(r.questions || []).length ? `<div class="aa-qs">${icon('info', 13)}<div>${r.questions.map((q) => `<div>${esc(q)}</div>`).join('')}</div></div>` : ''}
      ${plan.map((c) => changeCard(c)).join('')}
      ${plan.length ? (m.applied ? `<div class="aa-done">${icon('ok', 13)} Applied${m.passwords ? ` · ${m.passwords}` : ''}</div>`
        : m.discarded ? '<div class="muted">Discarded.</div>'
        : `<div class="aip-acts"><span class="grow"></span><button class="btn sm" data-a="discard">Discard</button><button class="btn sm primary" data-a="apply"${bad ? ' disabled title="Fix the problems first — ask again more precisely"' : ''}>${icon('check', 12)} Apply ${plan.length} change${plan.length > 1 ? 's' : ''}</button></div>`) : ''}`;
  };

  draw();
  el.querySelectorAll('[data-m]').forEach((b) => b.addEventListener('click', () => { mode = b.dataset.m; draw(); }));
  $('[data-a="close"]').addEventListener('click', () => close());
  $('[data-a="new"]').addEventListener('click', () => { chats[mode] = []; draw(); });
  ta.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); }
    else if (!(e.metaKey || e.ctrlKey)) e.stopPropagation();
  });
  $('[data-a="send"]').addEventListener('click', send);

  async function send() {
    const text = ta.value.trim();
    if (!text || busy) return;
    const msgs = chats[mode];
    const history = msgs.filter((m) => !m.failed).map((m) => (m.role === 'user' ? { role: 'user', content: m.text } : { role: 'assistant', content: JSON.stringify(mode === 'ask' ? { answer: m.r.answer } : { explanation: m.r.explanation, changes: m.r.changes }) }));
    msgs.push({ role: 'user', text });
    ta.value = '';
    busy = true;
    draw();
    try {
      const r = await call('ai_admin', { kind: mode, prompt: text, provider: chip.current()?.id || null, history, lang: appLanguage() }, { quiet: true });
      msgs.push({ role: 'ai', r });
    } catch (e) { msgs.push({ role: 'ai', failed: String(e) }); }
    busy = false;
    draw();
  }

  // the changes go through the Users page's own saves (each one in the activity log)
  async function apply(m) {
    const plan = m.r.plan || [];
    const ok = await modal({ title: `Apply ${plan.length} change${plan.length > 1 ? 's' : ''}?`, iconName: 'lock',
      body: `<p>${plan.map((c) => `<b>${esc(c.after.name || c.after.login)}</b>${c.new ? ' (new account)' : ''}`).join(', ')}.</p><p class="muted">Each change is saved as if made on the Users page, and recorded in the activity log.</p>`,
      buttons: [{ label: 'Cancel', value: false }, { label: 'Apply', kind: 'primary', value: true }] });
    if (!ok) return;
    const pw = [];
    try {
      let reports = null;
      for (const c of plan) {
        const a = c.after;
        const account = { was: c.new ? '' : c.before.login, login: a.login, name: a.name, email: a.email || '', password: c.new ? c.password : '', active: a.active !== false,
          must_change: c.new ? true : !!a.must_change, roles: a.roles, access: a.access, ai_level: a.ai_level || '' };
        await call('auth', { cmd: 'save_user', args: { account } }, { quiet: true });
        if (c.new) pw.push(`${a.login}: first password ${c.password}`);
        if (JSON.stringify([...c.groups].sort()) !== JSON.stringify([...(c.groups_before || [])].sort())) await call('auth', { cmd: 'set_user_groups', args: { login: a.login, groups: c.groups } }, { quiet: true });
        if (c.reports_add.length || c.reports_remove.length) {
          reports = reports || (await call('auth', { cmd: 'reports' }, { quiet: true }));
          for (const r of reports.filter((x) => c.reports_add.includes(x.id) || c.reports_remove.includes(x.id))) {
            const users = c.reports_add.includes(r.id) ? [...new Set([...r.audience.users, a.login])] : r.audience.users.filter((l) => l !== a.login);
            const out = await call('auth', { cmd: 'set_audience', args: { id: r.id, audience: { ...r.audience, users } } }, { quiet: true });
            Object.assign(r, out.report);
          }
        }
      }
      m.applied = true;
      m.passwords = pw.join(' · ');
      toast('Access changed.', 'info');
      if (S.area === 'admin') render();
    } catch (e) { toast(String(e), 'error', 5000); }
    draw();
  }

  function open(want) {
    if (want) mode = want;
    el.hidden = false;
    document.body.classList.add('adm-ai-on');
    if (!chip.loaded()) chip.load();
    draw();
    setTimeout(() => ta.focus(), 30);
  }
  function close() {
    el.hidden = true;
    document.body.classList.remove('adm-ai-on');
  }
  // leaving Admin closes it
  return { el, open, close };
}


// ---------------------------------------------------------------- before → after for one person

const roleNames = (r) => ['admin', 'data', 'design', 'user'].filter((k) => r?.[k]).map((k) => ({ admin: 'Admin', data: 'Data', design: 'Design', user: 'User' }[k])).join(', ') || '—';
const clientsText = (a, roles, v) => (roles?.admin ? 'every client (admin)' : !a || a.all ? 'every client' : [v?.bankers?.length ? `clients of ${v.bankers.join(', ')}` : '', v?.clients?.length ? (v.clients.length > 3 ? `${v.clients.length} clients` : v.clients.join(', ')) : ''].filter(Boolean).join(' + ') || 'no clients');
const productsText = (a) => (!a?.sources?.length ? 'every product' : a.sources.map(srcName).join(', '));
const levelText = (l) => (l === 'aggregates' || l === 'full' ? 'Figures' : l === 'structure' ? 'Structure only' : 'not set');

function changeCard(c) {
  const b = c.before || {}, a = c.after;
  const rows = [
    ['Roles', roleNames(b.roles), roleNames(a.roles)],
    ['Clients', c.new ? '—' : clientsText(b.access, b.roles, c.view_before), clientsText(a.access, a.roles, c.view_after)],
    ['Products', productsText(b.access), productsText(a.access)],
    ['Groups', (c.view_before?.groups || []).join(', ') || '—', (c.view_after?.groups || []).join(', ') || '—'],
    ['AI may send', levelText(b.ai_level), levelText(a.ai_level)],
    ['Can sign in', c.new ? '—' : b.active === false ? 'no' : 'yes', a.active === false ? 'no' : 'yes'],
  ];
  if (c.reports_add.length || c.reports_remove.length) rows.push(['Reports', '', [c.reports_add.length ? `+ ${c.reports_add.join(', ')}` : '', c.reports_remove.length ? `− ${c.reports_remove.join(', ')}` : ''].filter(Boolean).join(' · ')]);
  // a new account has nothing before it
  if (c.new) rows.forEach((r) => { r[1] = '—'; });
  const changed = rows.filter((r) => c.new || r[1] !== r[2]);
  return `<div class="aa-change${c.problems?.length ? ' bad' : ''}">
    <div class="aa-who">${icon(c.new ? 'user-plus' : 'user', 14)}<b>${esc(a.name || a.login)}</b><span>${esc(a.login)}${c.new ? ' · new account' : ''}</span></div>
    ${changed.length ? `<table class="lgd-diff"><thead><tr><th></th><th>Now</th><th>After</th></tr></thead><tbody>${changed.map(([k, x, y]) => `<tr><td>${esc(k)}</td><td class="${c.new ? '' : 'was'}">${esc(x)}</td><td class="now">${esc(y)}</td></tr>`).join('')}</tbody></table>` : '<div class="muted">Nothing changes for this person.</div>'}
    ${c.problems?.length ? `<div class="tl-err">${icon('warn', 12)} ${c.problems.map(esc).join(' ')}</div>` : ''}
  </div>`;
}
