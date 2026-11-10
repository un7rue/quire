/**
 * Admin → Access, AI and Activity: everyone's access at a glance, the bank's AI settings
 * (what may be sent, providers, bank keys, who may send figures, what AI was used for) and the
 * activity log (sign-ins, administration, documents, edits, data work and AI requests) with
 * filters, a detail panel and CSV export. Users & roles and Groups stay in areas.js.
 */
import { S, call, render, setState } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { toast, modal } from './ui.js';
import { KEYS_AT } from './platform.js';
import { initials } from './auth.js';
import { appLanguage } from './i18n.js';
import { toggleAdminAI, linksHtml, bindLinks, openLink } from './aiadmin.js';
import { rich } from './brief.js';
import { proposalBodyHtml, statusPill, statusName, statusMenu, STATUSES } from './proposal.js';
import { areaHead, AI_LEVELS, aiLevelName, aiFor, ROLE_INFO, ROLE_ORDER, accessText, srcName, learnProducts } from './areas.js';

// everything the pages need about people, read together
async function loadPeople() {
  const [data, opts, grp, reps, st] = await Promise.all([
    call('auth', { cmd: 'users' }, { quiet: true }),
    call('auth', { cmd: 'options' }, { quiet: true }).catch(() => null),
    call('auth', { cmd: 'groups' }, { quiet: true }),
    call('auth', { cmd: 'reports' }, { quiet: true }),
    call('ai_status', {}, { quiet: true }).catch(() => null),
  ]);
  learnProducts(opts?.sources);
  const bank = st?.policy?.level || 'structure';
  const groupsOf = (login) => grp.groups.filter((g) => g.members.includes(login));
  // the reports a person gets: every one for administrators and designers
  const reportsOf = (u) => {
    if (u.roles.admin || u.roles.design) return { all: true, list: reps };
    if (!u.roles.user) return { none: true, list: [] };
    const gs = groupsOf(u.login).map((g) => g.id);
    return { list: reps.filter((r) => r.published !== false && (r.audience?.everyone || r.audience?.users?.includes(u.login) || r.audience?.groups?.some((g) => gs.includes(g)))) };
  };
  const levelOf = (u) => aiFor(u.ai_level === 'full' ? 'aggregates' : u.ai_level, groupsOf(u.login), bank);
  return { data, opts, grp, reps, st, bank, groupsOf, reportsOf, levelOf };
}

const openUser = (login) => { S.adminTab = 'users'; S.adminSel = login; render(); };
const levelPill = ([lvl, from]) => `<span class="lv-pill ${lvl === 'structure' ? '' : 'fig'}" title="${esc(from)}">${esc(aiLevelName(lvl))}</span>`;

// ---------------------------------------------------------------- Access

/** Admin → Access: one row per person — roles, clients, products, groups, reports and AI. */
/**
 * Python on the web server (Access page): off until an administrator switches it on, because a
 * Python script runs as the server and can read whatever the server can. Not shown in the desktop app.
 */
async function pythonCard(card) {
  let p;
  try { p = await call('python_policy', {}, { quiet: true }); } catch { return; }
  if (!p.server) return;
  card.hidden = false;
  card.innerHTML = `<div class="ar-card-t">${icon('python', 15)} Python on this server <span class="ac-py-st ${p.on ? 'on' : ''}">${p.on ? 'On' : 'Off'}</span><span class="grow"></span>
      <button class="btn ${p.on ? '' : 'primary'}" data-a="py">${p.on ? 'Switch off' : 'Switch on…'}</button></div>
    <p class="muted ac-py-tx">${p.on
      ? 'Designers and the Data role can run Python data sources and Python in the SQL workspace. Scripts run as the server and can read whatever it can, outside the folders data sources are limited to.'
      : 'Python data sources and the SQL workspace’s Python don’t run here. Sources that use Python show an error until it is switched on.'}</p>`;
  card.querySelector('[data-a="py"]').addEventListener('click', async () => {
    if (!p.on) {
      const ok = await modal({ title: 'Switch Python on?', iconName: 'warn', iconKind: 'warn',
        body: '<p>Python scripts run on the server as the server itself. Anyone who may write a data source (designers, the Data role) can then read any file the server can, and reach any address it can — outside the folders and database servers Quire limits data to.</p><p>Switch it on only if you trust everyone with those roles.</p>',
        buttons: [{ label: 'Cancel', value: false }, { label: 'Switch on', kind: 'danger', value: true }] });
      if (!ok) return;
    }
    try { await call('python_policy', { on: !p.on }); toast(p.on ? 'Python switched off' : 'Python switched on'); setState(await call('get_state')); pythonCard(card); } catch { /* toast shown */ }
  });
}

export async function accessPage(el) {
  el.innerHTML = `${areaHead('admin', 'Everyone at a glance: what they can do, whose data they see, which reports they get and what AI may send for them. Click a person to change it.',
    `<button class="btn" data-a="aiaccess">${icon('lock', 14)} Change with AI</button><button class="btn primary" data-a="review">${icon('sparkles', 14)} Review with AI</button>`, 'Access')}
    <div class="aa-review" hidden></div>
    <div class="ac-tiles"></div>
    <section class="ar-card ac-py" hidden></section>
    <section class="ar-card">
      <div class="ar-card-t">${icon('lock', 15)} Who can see what <span class="muted ac-n"></span><span class="grow"></span><input class="ac-find" placeholder="Find a person, client or group"></div>
      <div class="ac-wrap"><div class="ar-note">Loading…</div></div>
    </section>`;
  pythonCard(el.querySelector('.ac-py'));
  let P;
  try { P = await loadPeople(); } catch (e) { el.querySelector('.ac-wrap').innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
  const { data, opts, groupsOf, reportsOf, levelOf } = P;
  const users = data.users;
  const act = users.filter((u) => u.active);
  const tile = (n, label, tip, tone = '') => `<div class="ac-tile ${tone}" title="${esc(tip)}"><b>${n}</b><span>${esc(label)}</span></div>`;
  el.querySelector('.ac-tiles').innerHTML = [
    tile(act.length, `can sign in${users.length > act.length ? ` · ${users.length - act.length} disabled` : ''}`, 'People with an active account'),
    tile(users.filter((u) => u.roles.admin).length, 'administrators', 'They see and change everything', 'warn'),
    tile(users.filter((u) => !u.roles.admin && (u.access?.all ?? true) && !u.access?.sources?.length).length, 'see every client', 'Not administrators, but with access to every client and product'),
    tile(users.filter((u) => !u.roles.admin && (!(u.access?.all ?? true) || u.access?.sources?.length)).length, 'restricted', 'Limited to some clients or products'),
    tile(users.filter((u) => levelOf(u)[0] !== 'structure').length, 'may send figures to AI', 'The report assistant can work with what their reports show (names and IDs masked)', 'fig'),
    tile(users.filter((u) => u.demo_password).length, 'still on the demo password', 'They should choose a new one', users.some((u) => u.demo_password) ? 'bad' : ''),
  ].join('');
  const wrap = el.querySelector('.ac-wrap');
  const draw = () => {
    const q = el.querySelector('.ac-find').value.trim().toLowerCase();
    const rows = users.map((u) => {
      const gs = groupsOf(u.login);
      const rp = reportsOf(u);
      const prods = u.roles.admin || !u.access?.sources?.length ? 'Every product' : u.access.sources.map(srcName).join(', ');
      const text = `${u.name} ${u.login} ${accessText(u, opts)} ${prods} ${gs.map((g) => g.name).join(' ')} ${rp.list.map((r) => r.title).join(' ')}`.toLowerCase();
      return { u, gs, rp, prods, text };
    }).filter((r) => !q || r.text.includes(q));
    el.querySelector('.ac-n').textContent = String(rows.length);
    wrap.innerHTML = rows.length ? `<table class="ac-t"><thead><tr><th>Person</th><th>Roles</th><th>Clients</th><th>Products</th><th>Groups</th><th>Reports</th><th>AI may send</th></tr></thead><tbody>
      ${rows.map(({ u, gs, rp, prods }) => `<tr data-l="${esc(u.login)}" class="${u.active ? '' : 'off'}">
        <td><div class="ac-who"><span class="av">${esc(initials(u.name || u.login))}</span><span><b>${esc(u.name || u.login)}</b><i>${esc(u.login)}${u.active ? '' : ' · disabled'}${u.demo_password ? ' · <em>demo pw</em>' : ''}</i></span></div></td>
        <td><div class="ac-roles">${ROLE_ORDER.filter((r) => u.roles[r]).map((r) => `<span class="rl rl-${r}" title="${esc(ROLE_INFO[r][1])}">${esc(ROLE_INFO[r][0])}</span>`).join('') || '<span class="muted">—</span>'}</div></td>
        <td>${u.roles.admin ? '<span class="muted">Every client</span>' : esc(accessText(u, opts).split(' · ')[0])}</td>
        <td>${u.roles.admin || !u.access?.sources?.length ? '<span class="muted">Every product</span>' : esc(prods)}</td>
        <td>${gs.map((g) => `<span class="ac-g">${esc(g.name)}</span>`).join('') || '<span class="muted">—</span>'}</td>
        <td title="${esc(rp.list.map((r) => r.title).join('\n'))}">${rp.all ? `<span class="muted">All (${rp.list.length})</span>` : rp.none ? '<span class="muted">No User role</span>' : rp.list.length ? `${rp.list.length} <span class="muted">· ${esc(rp.list.slice(0, 2).map((r) => r.title).join(', '))}${rp.list.length > 2 ? '…' : ''}</span>` : '<span class="ac-none">None</span>'}</td>
        <td>${levelPill(levelOf(u))}<i class="ac-from">${esc(levelOf(u)[1])}</i></td>
      </tr>`).join('')}</tbody></table>` : '<div class="ar-note">Nobody matches.</div>';
    wrap.querySelectorAll('tr[data-l]').forEach((tr) => tr.addEventListener('click', () => openUser(tr.dataset.l)));
  };
  el.querySelector('.ac-find').addEventListener('input', draw);
  el.querySelector('.ac-find').addEventListener('keydown', (e) => e.stopPropagation());
  draw();
  el.querySelector('[data-a="aiaccess"]').addEventListener('click', () => toggleAdminAI('access'));
  el.querySelector('[data-a="review"]').addEventListener('click', () => reviewAccess(el.querySelector('.aa-review'), P));
  if (lastReview) showReview(el.querySelector('.aa-review'), lastReview, P);
}

// the last review stays on the Access page until another is asked for
let lastReview = null;
const SEV = { high: ['High', 'bad'], medium: ['Medium', 'warn'], low: ['Low', ''] };

/** Who can do what and what looks wrong, read by AI from the people and their last 30 days. */
async function reviewAccess(box, P) {
  box.hidden = false;
  box.innerHTML = `<section class="ar-card aa-rev"><div class="ar-note"><span class="spin"></span> Reviewing who can do what, and the last 30 days of activity…</div></section>`;
  try {
    lastReview = await call('ai_admin', { kind: 'review', prompt: '', lang: appLanguage() }, { quiet: true });
    showReview(box, lastReview, P);
  } catch (e) { box.innerHTML = `<section class="ar-card aa-rev"><div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div></section>`; }
}

function showReview(box, r, P) {
  box.hidden = false;
  const order = { high: 0, medium: 1, low: 2 };
  const fs = [...(r.findings || [])].sort((a, b) => (order[a.severity] ?? 3) - (order[b.severity] ?? 3));
  box.innerHTML = `<section class="ar-card aa-rev">
    <div class="ar-card-t">${icon('sparkles', 15)} Access review <span class="muted">${esc(r.provider || '')} · ${esc(r.model || '')}</span><span class="grow"></span><button class="tb-btn" data-a="x" title="Close">${icon('x', 14)}</button></div>
    ${r.summary ? `<p class="aa-rsum">${rich(r.summary)}</p>` : ''}
    ${fs.length ? fs.map((f, i) => `<div class="aa-find" data-i="${i}">
      <span class="aa-sev ${SEV[f.severity]?.[1] || ''}">${esc(SEV[f.severity]?.[0] || f.severity || '')}</span>
      <div class="aa-fx"><b>${esc(f.title || '')}</b><span>${rich(f.detail || '')}</span>${linksHtml(f.links)}</div>
      <div class="aa-acts">${f.login ? `<button class="btn sm" data-act="open">${icon('user', 12)} Open</button>` : ''}
        ${f.login && f.action === 'disable' ? `<button class="btn sm danger-ghost" data-act="disable">Disable account</button>` : ''}
        ${f.login && f.action === 'ai_structure' ? `<button class="btn sm" data-act="ai">AI: Structure only</button>` : ''}</div></div>`).join('')
      : '<div class="ar-note">Nothing to report.</div>'}
  </section>`;
  box.querySelector('[data-a="x"]').addEventListener('click', () => { lastReview = null; box.hidden = true; });
  box.querySelectorAll('.aa-find').forEach((row) => {
    const f = fs[+row.dataset.i];
    bindLinks(row, f.links || []);
    const save = async (patch, what) => {
      const u = P.data.users.find((x) => x.login === f.login);
      if (!u) return;
      const ok = await modal({ title: `${what}?`, iconName: 'lock', body: `<p><b>${esc(u.name || u.login)}</b> (${esc(u.login)})</p>`, buttons: [{ label: 'Cancel', value: false }, { label: what, kind: 'primary', value: true }] });
      if (!ok) return;
      const account = { was: u.login, login: u.login, name: u.name, email: u.email || '', password: '', active: u.active, must_change: u.must_change, roles: u.roles, access: u.access, ai_level: u.ai_level || '', ...patch };
      try { await call('auth', { cmd: 'save_user', args: { account } }, { quiet: true }); toast('Saved.', 'info'); render(); } catch (e) { toast(String(e), 'error'); }
    };
    row.querySelector('[data-act="open"]')?.addEventListener('click', () => openUser(f.login));
    row.querySelector('[data-act="disable"]')?.addEventListener('click', () => save({ active: false }, 'Disable the account'));
    row.querySelector('[data-act="ai"]')?.addEventListener('click', () => save({ ai_level: 'structure' }, 'Set AI to Structure only'));
  });
}

// ---------------------------------------------------------------- AI

const LEVEL_TEXT = {
  structure: ['Structure only', 'Table and column names, types and SQL. Enough for the SQL and design assistants and for restyling. Right for data and design people, and the safe default for the bank.'],
  aggregates: ['Figures', 'Also what a report shows — totals, returns, allocations — with client, account and instrument names and IDs replaced by codes. Needed for the report assistant’s brief, check and notes: for bankers preparing client meetings.'],
};

/** Admin → AI: the bank's policy, who may send figures, bank-wide keys and recent use. */
export async function aiAdminPage(el) {
  el.innerHTML = `${areaHead('admin', 'What AI assistants may send out of the bank, which providers they use, and with which keys. Every request is in Activity.',
    `<button class="btn" data-a="log">${icon('clock', 14)} AI requests</button>`, 'AI')}
    <div class="aa-grid"><div class="aa-col"></div><div class="aa-col"></div></div>`;
  el.querySelector('[data-a="log"]').addEventListener('click', () => { S.actFilter = { area: 'ai' }; S.adminTab = 'activity'; render(); });
  const [left, right] = el.querySelectorAll('.aa-col');
  left.innerHTML = right.innerHTML = '<div class="ar-note">Loading…</div>';
  let P;
  try { P = await loadPeople(); } catch (e) { left.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; right.innerHTML = ''; return; }
  const st = P.st;
  if (!st) { left.innerHTML = '<div class="ar-note bad">AI settings could not be read.</div>'; right.innerHTML = ''; return; }
  const pol = { level: 'structure', providers: [], off: false, ...(st.policy || {}) };
  if (pol.level === 'full') pol.level = 'aggregates';

  // the bank's policy, saved the moment it changes
  const polCard = h(`<section class="ar-card"><div class="ar-card-t">${icon('shield', 15)} Bank policy <span class="muted">— for everyone, unless set for a person or group</span></div>
    <label class="tgl"><input type="checkbox" name="on"${pol.off ? '' : ' checked'}><span class="track"></span><span>AI assistants on</span></label>
    <div class="aa-levels">${Object.entries(LEVEL_TEXT).map(([k, [t, d]]) => `<label class="aa-lvl${pol.level === k ? ' on' : ''}"><input type="radio" name="lvl" value="${k}"${pol.level === k ? ' checked' : ''}><span class="aa-dot"></span><span><b>${esc(t)}</b><span>${esc(d)}</span></span></label>`).join('')}</div>
    <div class="ai-allow"><span>Providers allowed</span>${st.providers.map((p) => `<label class="ai-chk"><input type="checkbox" value="${p.id}"${!pol.providers.length || pol.providers.includes(p.id) ? ' checked' : ''}> ${esc(p.name)}</label>`).join('')}</div>
    <div class="hint">Names and numbers of clients, accounts and instruments never leave the bank at any level: they go as codes ([CLIENT], [ID-1], [INS-2]) and are put back here.</div></section>`);
  const savePolicy = async () => {
    const chosen = [...polCard.querySelectorAll('.ai-allow input:checked')].map((x) => x.value);
    if (!chosen.length) { toast('Allow at least one provider.', 'info'); return; }
    const level = polCard.querySelector('input[name=lvl]:checked').value;
    const next = { ...pol, level, off: !polCard.querySelector('input[name=on]').checked, providers: chosen.length === st.providers.length ? [] : chosen };
    polCard.querySelectorAll('.aa-lvl').forEach((l) => l.classList.toggle('on', l.querySelector('input').checked));
    try { await call('ai_set_policy', { policy: next }); toast('Bank AI policy saved.', 'info', 1600); P.bank = level; drawWho(); } catch {}
  };
  polCard.querySelectorAll('input').forEach((x) => x.addEventListener('change', savePolicy));

  // who may send figures: everyone's level and where it comes from
  const whoCard = h(`<section class="ar-card"><div class="ar-card-t">${icon('users', 15)} What AI may send, person by person</div><div class="aa-who"></div>
    <div class="hint">Set it for a person in Users & roles, or for a group in Groups. A person’s own setting wins; otherwise the widest of their groups’; otherwise the bank’s.</div></section>`);
  const drawWho = () => {
    const gset = P.grp.groups.filter((g) => g.ai_level);
    whoCard.querySelector('.aa-who').innerHTML = `<table class="ac-t sm"><tbody>${P.data.users.filter((u) => u.active).map((u) => {
      const lv = P.levelOf(u);
      return `<tr data-l="${esc(u.login)}"><td><div class="ac-who"><span class="av">${esc(initials(u.name || u.login))}</span><span><b>${esc(u.name || u.login)}</b><i>${esc(u.login)}</i></span></div></td><td>${levelPill(lv)}</td><td class="muted">${esc(lv[1])}</td></tr>`;
    }).join('')}</tbody></table>${gset.length ? `<div class="aa-gl">${gset.map((g) => `<span class="ac-g">${icon('users', 11)} ${esc(g.name)}: ${esc(aiLevelName(g.ai_level === 'full' ? 'aggregates' : g.ai_level))}</span>`).join('')}</div>` : ''}`;
    whoCard.querySelectorAll('tr[data-l]').forEach((tr) => tr.addEventListener('click', () => openUser(tr.dataset.l)));
  };
  drawWho();

  // bank-wide keys: used by anyone without a key of their own
  const keyCard = h(`<section class="ar-card"><div class="ar-card-t">${icon('key', 15)} Bank-wide keys <span class="muted">— for anyone without a key of their own</span></div><div class="aa-keys"></div>
    <div class="hint">Saved ${KEYS_AT} and never shown again. A person’s own keys are in their account (Users & roles).</div></section>`);
  const drawKeys = async () => {
    let s2 = st;
    try { s2 = await call('ai_status', {}, { quiet: true }); } catch {}
    const box = keyCard.querySelector('.aa-keys');
    box.innerHTML = s2.providers.map((p) => `<div class="aa-key${p.allowed ? '' : ' off'}" data-p="${p.id}"><b>${esc(p.name)}</b><span class="ai-st ${p.bank ? 'ok' : ''}">${p.bank ? `${icon('check', 12, 2.4)} Saved` : 'No key'}</span>
      <input type="password" autocomplete="off" spellcheck="false" placeholder="${p.bank ? '•••••••• — paste to replace' : p.id === 'gigachat' ? 'Authorization key (Base64)' : 'Paste the API key'}">
      <button class="btn sm" data-k="save">Save</button>${p.bank ? `<button class="btn sm" data-k="del" title="Remove the bank’s key">${icon('trash', 13)}</button>` : '<span></span>'}</div>`).join('');
    box.querySelectorAll('.aa-key').forEach((row) => {
      const id = row.dataset.p, name = s2.providers.find((p) => p.id === id).name, inp = row.querySelector('input');
      inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') row.querySelector('[data-k=save]').click(); });
      row.querySelector('[data-k=save]').addEventListener('click', async () => {
        if (!inp.value.trim()) { inp.focus(); return; }
        try { await call('ai_set_key', { provider: id, key: inp.value.trim(), bank: true }); toast(`${name} bank key saved.`); drawKeys(); } catch {}
      });
      row.querySelector('[data-k=del]')?.addEventListener('click', async () => {
        try { await call('ai_set_key', { provider: id, key: null, bank: true }); toast(`${name} bank key removed.`); drawKeys(); } catch {}
      });
    });
  };
  drawKeys();

  // what AI was used for in the last 30 days, by person and by task
  const useCard = h(`<section class="ar-card"><div class="ar-card-t">${icon('stats', 15)} Use and cost · last 30 days</div><div class="aa-use"><div class="ar-note">Loading…</div></div></section>`);
  const prices = { ...(pol.prices || {}) };
  const priceOf = (prov) => prices[prov] || LIST_PRICES[prov] || [0, 0];
  const usd = (x) => (x >= 100 ? `$${Math.round(x).toLocaleString()}` : `$${x.toFixed(2)}`);
  let rows = [];
  let drawUse = () => {
    const box = useCard.querySelector('.aa-use');
    const names = Object.fromEntries(P.data.users.map((u) => [u.login, u.name || u.login]));
    const prov = (r) => String(r.target || '').split(' · ')[0];
    const cost = (r) => { const [pi, po] = priceOf(prov(r)); return ((r.tokens_in || 0) * pi + (r.tokens_out || 0) * po) / 1e6; };
    const sum = (list) => list.reduce((a, r) => ({ n: a.n + 1, i: a.i + (r.tokens_in || 0), o: a.o + (r.tokens_out || 0), bad: a.bad + (r.ok ? 0 : 1), c: a.c + cost(r), counted: a.counted + (r.tokens_in != null ? 1 : 0) }), { n: 0, i: 0, o: 0, bad: 0, c: 0, counted: 0 });
    const by = (key) => Object.entries(rows.reduce((m, r) => ((m[key(r)] = m[key(r)] || []).push(r), m), {})).map(([k, l]) => [k, sum(l)]).sort((a, b) => b[1].c - a[1].c || b[1].i - a[1].i);
    const all = sum(rows);
    const t = (rowsOf, label) => `<table class="ac-t sm"><thead><tr><th>${label}</th><th class="r">Requests</th><th class="r">Tokens sent</th><th class="r">Received</th><th class="r">Cost</th></tr></thead><tbody>${rowsOf.map(([k, x]) => `<tr><td>${esc(k)}</td><td class="r">${x.n}${x.bad ? ` <span class="ac-none">${x.bad} failed</span>` : ''}</td><td class="r">${x.counted ? x.i.toLocaleString() : '—'}</td><td class="r">${x.counted ? x.o.toLocaleString() : '—'}</td><td class="r"><b>${x.counted ? usd(x.c) : '—'}</b></td></tr>`).join('')}</tbody></table>`;
    // what the numbers suggest
    const tips = [];
    const usedReport = new Set(rows.filter((r) => String(r.action).startsWith('report.')).map((r) => r.user));
    const figures = P.data.users.filter((u) => u.active && P.levelOf(u)[0] !== 'structure' && !usedReport.has(u.login));
    if (figures.length) tips.push(`${figures.map((u) => u.name || u.login).join(', ')} may send client figures but didn’t use the report assistant in 30 days — Structure only would do for ${figures.length > 1 ? 'them' : 'them'}.`);
    const people = by((r) => r.user || '—');
    if (people.filter((x) => x[1].c > 0).length > 1 && people[0][1].c / all.c > 0.6) tips.push(`${names[people[0][0]] || people[0][0]} accounts for ${Math.round((100 * people[0][1].c) / all.c)}% of the cost.`);
    if (all.n >= 5 && all.bad / all.n > 0.2) tips.push(`${Math.round((100 * all.bad) / all.n)}% of requests failed — check the keys, the provider’s limits, or open the failures in Activity.`);
    const restyle = rows.filter((r) => r.action === 'design.restyle');
    if (restyle.length && all.c > 0 && sum(restyle).c / all.c > 0.4) tips.push('Restyling from pictures is the most expensive task (it sends images): fine for occasional use.');
    box.innerHTML = rows.length ? `<div class="aa-sum"><span><b>${usd(all.c)}</b> approx. cost</span><span><b>${all.n}</b> requests</span><span><b>${all.i.toLocaleString()}</b> tokens sent</span><span><b>${all.o.toLocaleString()}</b> received</span>${all.bad ? `<span class="ac-none"><b>${all.bad}</b> failed</span>` : ''}</div>
      ${tips.length ? `<div class="aa-tips">${tips.map((x) => `<div>${icon('info', 13)}<span>${esc(x)}</span></div>`).join('')}</div>` : ''}
      ${t(by((r) => names[r.user] || r.user || '—'), 'Person')}${t(by((r) => TASKS[r.action] || r.action), 'Task')}${t(by((r) => r.target || '—'), 'Provider · model')}`
      : '<div class="ar-note">No AI requests in the last 30 days.</div>';
    box.append(h(`<details class="aa-prices"><summary>${icon('dollar', 12)} Prices per million tokens (USD) — set your contract’s</summary>
      <table class="ac-t sm"><thead><tr><th>Provider</th><th class="r">Sent</th><th class="r">Received</th></tr></thead><tbody>${st.providers.map((p) => `<tr><td>${esc(p.name)}</td>
        <td class="r"><input type="number" min="0" step="0.01" data-p="${p.id}" data-k="0" value="${priceOf(p.id)[0]}"></td><td class="r"><input type="number" min="0" step="0.01" data-p="${p.id}" data-k="1" value="${priceOf(p.id)[1]}"></td></tr>`).join('')}</tbody></table>
      <div class="hint">The defaults are list prices and may be out of date; GigaChat is billed in roubles — put its dollar equivalent. Saved for the bank as you change them.</div></details>`));
    box.querySelectorAll('.aa-prices input').forEach((inp) => {
      inp.addEventListener('keydown', (e) => e.stopPropagation());
      inp.addEventListener('change', async () => {
        const id = inp.dataset.p, k = +inp.dataset.k;
        const v = [...priceOf(id)];
        v[k] = Math.max(0, parseFloat(inp.value) || 0);
        prices[id] = v;
        try { await call('ai_set_policy', { policy: { ...pol, prices } }, { quiet: true }); pol.prices = { ...prices }; toast('Prices saved.', 'info', 1200); } catch (e) { toast(String(e), 'error'); }
        const open = box.querySelector('.aa-prices').open;
        drawUse();
        useCard.querySelector('.aa-prices').open = open;
      });
    });
  };
  (async () => {
    try { rows = (await call('activity', { filter: { area: 'ai', from: dayStr(-29), limit: 100000 } }, { quiet: true })).rows.filter((r) => r.ai); } catch (e) { useCard.querySelector('.aa-use').innerHTML = `<div class="ar-note bad">${esc(String(e))}</div>`; return; }
    drawUse();
  })();

  // what people said about the answers: helpful or not, by task and person, and the latest 👎 to look into
  const fbCard = h(`<section class="ar-card"><div class="ar-card-t">${icon('thumbs-up', 15)} Feedback · last 30 days</div><div class="aa-fbk"><div class="ar-note">Loading…</div></div></section>`);
  const drawFeedback = () => {
    const box = fbCard.querySelector('.aa-fbk');
    const voted = rows.filter((r) => r.feedback?.vote);
    if (!voted.length) { box.innerHTML = '<div class="ar-note">No feedback yet. Everyone can mark an AI answer 👍 or 👎 under it; what didn’t help shows here.</div>'; return; }
    const names = Object.fromEntries(P.data.users.map((u) => [u.login, u.name || u.login]));
    const up = voted.filter((r) => r.feedback.vote === 'up').length, down = voted.length - up;
    const by = (key) => Object.entries(voted.reduce((m, r) => { const k = key(r); (m[k] = m[k] || { up: 0, down: 0 })[r.feedback.vote === 'up' ? 'up' : 'down'] += 1; return m; }, {})).sort((a, b) => b[1].down - a[1].down || b[1].up - a[1].up);
    const t = (list, label) => `<table class="ac-t sm"><thead><tr><th>${label}</th><th class="r">👍</th><th class="r">👎</th><th class="r">Helpful</th></tr></thead><tbody>${list.map(([k, x]) => `<tr><td>${esc(k)}</td><td class="r">${x.up}</td><td class="r">${x.down ? `<span class="ac-none">${x.down}</span>` : 0}</td><td class="r"><b>${Math.round((100 * x.up) / (x.up + x.down))}%</b></td></tr>`).join('')}</tbody></table>`;
    const bad = voted.filter((r) => r.feedback.vote === 'down').slice(0, 8);
    box.innerHTML = `<div class="aa-sum"><span><b>${Math.round((100 * up) / voted.length)}%</b> helpful</span><span><b>${up}</b> 👍</span><span><b>${down}</b> 👎</span><span><b>${voted.length}</b> of ${rows.length} answers rated</span></div>
      ${bad.length ? `<div class="aa-bad"><span class="aa-bad-h">Didn’t help — the latest</span>${bad.map((r, i) => `<button class="aa-bad-row" data-bad="${i}"><span class="lg-vote down">${icon('thumbs-down', 12)}</span>
        <span class="aa-bad-t"><span><b>${esc(TASKS[r.action] || r.action)}</b> · ${esc(names[r.user] || r.user)} · ${esc((r.at || '').replace('T', ' ').slice(0, 16))}</span>
        ${r.feedback.reasons?.length ? `<i>${esc(r.feedback.reasons.join(' · '))}</i>` : ''}${r.feedback.comment ? `<i>“${esc(r.feedback.comment)}”</i>` : ''}</span>${icon('chevron-right', 13)}</button>`).join('')}</div>` : ''}
      ${t(by((r) => TASKS[r.action] || r.action), 'Task')}${t(by((r) => names[r.user] || r.user), 'Person')}`;
    box.querySelectorAll('[data-bad]').forEach((b) => b.addEventListener('click', () => { const r = bad[+b.dataset.bad]; openLink({ type: 'event', at: r.at, user: r.user }); }));
  };
  const shown = drawUse;
  drawUse = () => { shown(); drawFeedback(); };

  left.replaceChildren(polCard, keyCard);
  right.replaceChildren(whoCard, useCard, fbCard);
}

// list prices per million tokens sent / received (USD), until the bank sets its own
const LIST_PRICES = { claude: [3, 15], chatgpt: [2, 8], deepseek: [0.27, 1.1], qwen: [0.4, 1.2], gigachat: [1.5, 1.5] };

// the AI tasks, for people
const TASKS = {
  'admin.ask': 'Admin: question about activity', 'admin.explain': 'Admin: explain a failure', 'admin.access': 'Admin: access in words', 'admin.review': 'Admin: access review',
  'sql.write': 'SQL: write a query', 'sql.explain': 'SQL: explain', 'sql.fix': 'SQL: fix an error', 'sql.source': 'SQL: make a report source', 'sql.ask': 'SQL: question about the data',
  'design.page': 'Design: build a page', 'design.change': 'Design: change a page', 'design.restyle': 'Design: restyle',
  'report.brief': 'Report: brief & pitch', 'report.check': 'Report: check', 'report.note': 'Report: note', 'test': 'Key test',
};

// ---------------------------------------------------------------- Activity

// "document" events are files: new, open, save, export, close (edits are their own area)
const AREAS_LOG = [['auth', 'Sign-in', 'logout'], ['admin', 'Admin', 'shield'], ['session', 'Sessions', 'clock'], ['document', 'Files', 'page'], ['edit', 'Edits', 'brush'], ['data', 'Data', 'database'], ['ai', 'AI', 'sparkles'], ['api', 'API', 'link']];
// an action for people: "update_widget" → "Widget changed"
const EDITS = { set_params: 'Parameters changed', update_widget: 'Widget changed', set_rect: 'Widget moved or resized', add_widget: 'Widget added', add_from_catalog: 'Widget added from the library',
  remove_widget: 'Widget deleted', replace_widget: 'Widget replaced', duplicate_widget: 'Widget duplicated', move_to_page: 'Moved to another page', add_page: 'Page added', remove_page: 'Page deleted',
  rename_page: 'Page renamed', set_page_section: 'Page section changed', set_page_plain: 'Page header changed', set_page_repeat: 'Page repeat changed', set_page_props: 'Page changed', move_page: 'Page moved',
  duplicate_page: 'Page duplicated', ai_page: 'Page built by AI', set_meta: 'Title or details changed', set_formats: 'Formats changed', set_icons: 'Icons changed', set_variables: 'Variables changed',
  set_page_setup: 'Page size changed', set_theme: 'Theme changed', set_policy: 'Report access changed', set_arrange: 'Arrangement changed', set_param_defs: 'Parameters defined', set_connections: 'Connections changed',
  upsert_dataset: 'Data source changed', remove_dataset: 'Data source removed', apply_ops: 'Several changes', auto_arrange: 'Page arranged', batch: 'Several changes', undo: 'Undo', redo: 'Redo', 'apply theme': 'Theme applied' };
const SESSION_LINES = { start: 'Session started', context: 'Client or period picked', end: 'Session ended' };
const actionName = (r) => { const a = r.ai ? TASKS[r.action] || r.action : r.area === 'session' ? SESSION_LINES[r.action] || r.action : EDITS[r.action] || String(r.action || ''); const t = a.replace(/_/g, ' '); return t.charAt(0).toUpperCase() + t.slice(1); };
const areaName = (a) => (AREAS_LOG.find((x) => x[0] === a) || [a, a])[1];
// a local calendar day, `days` from today
function dayStr(days = 0) { const d = new Date(); d.setDate(d.getDate() + days); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`; }
const dayTitle = (d) => { try { return new Date(`${d}T12:00:00`).toLocaleDateString(appLanguage() === 'ru' ? 'ru-RU' : 'en-GB', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }); } catch { return d; } };
const tok = (n) => (n == null ? '—' : Number(n).toLocaleString());

// ---------------------------------------------------------------- an AI request, for the developer

// where each assistant lives in the code, so the paste says where to look
const AI_SOURCES = [
  [/^sql\./, 'the SQL assistant', 'src/ai.rs (its prompt: sql_system; schema_text), ai_sql_core in src/main.rs, ui/aiassist.js'],
  [/^design\.restyle/, 'restyling from a picture', 'src/ai_restyle.rs (system, look_from, check_views), ai_restyle_core in src/main.rs, ui/aidesign.js'],
  [/^design\./, 'the design assistant', 'src/ai_design.rs (system, plan), ai_design_core and design_apply in src/main.rs, ui/aidesign.js'],
  [/^report\./, 'the report assistant', 'src/ai_report.rs (system, facts, Masker), ai_report_core in src/main.rs, ui/aireport.js, ui/brief.js'],
  [/^admin\./, 'the Admin assistant', 'src/ai_admin.rs (system, activity, access_ctx, resolve_access), ai_admin_run in src/main.rs, ui/aiadmin.js'],
];

/** One AI request as Markdown to paste to Claude: what it was, what the person said, what was sent and received, where its code is. */
function requestForClaude(e) {
  const src = AI_SOURCES.find(([re]) => re.test(e.feature || '')) || [null, 'an AI assistant', 'src/ai.rs, src/main.rs'];
  const f = e.feedback || {};
  const verdict = f.vote === 'down' ? 'NOT helpful (👎)' : f.vote === 'up' ? 'helpful (👍)' : e.ok === false ? 'failed' : 'not rated';
  const roles = Object.entries(ROLES_OF[e.user] || {}).filter(([, v]) => v).map(([k]) => k).join(', ');
  const fence = (t, lang = 'text') => { const ticks = '`'.repeat(Math.max(3, ...(String(t).match(/`+/g) || []).map((x) => x.length + 1))); return `${ticks}${lang}\n${t}\n${ticks}`; };
  const sent = e.sent || [], got = e.received || [];
  const out = [
    `# Quire · AI request · ${verdict}`,
    '',
    `- **Assistant:** ${src[1]} — task \`${e.feature || ''}\` (${TASKS[e.feature] || e.feature || ''})`,
    `- **When / who:** ${(e.at || '').replace('T', ' ').slice(0, 19)} · \`${e.user || ''}\`${roles ? ` (roles: ${roles})` : ''}`,
    `- **Provider / model:** ${e.provider || ''} · ${e.model || ''}${e.key ? ` · ${e.key === 'bank' ? 'bank key' : 'own key'}` : ''}`,
    `- **Tokens:** ${tok(e.tokens_in)} sent → ${tok(e.tokens_out)} received · ${e.ms ? `${(e.ms / 1000).toFixed(1)} s` : '—'} · ${e.ok === false ? `**failed:** ${e.error || ''}` : 'ok'}${e.policy ? ` · data level: ${e.policy}` : ''}`,
    `- **Code:** ${src[2]}`,
    '',
    '## What the person said',
    f.vote ? `**${f.vote === 'down' ? '👎 Not helpful' : '👍 Helpful'}**${f.reasons?.length ? ` — ${f.reasons.join(', ')}` : ''}${f.comment ? `\n\n> ${String(f.comment).replace(/\n/g, '\n> ')}` : ''}` : '_No vote._',
    '',
    '## What they asked',
    e.prompt ? `> ${String(e.prompt).replace(/\n/g, '\n> ')}` : '_A button, no words._',
    '',
    ...sent.flatMap((t, k) => [`## Sent to the AI${sent.length > 1 ? ` — try ${k + 1} of ${sent.length}` : ''} (${Number(t.length).toLocaleString()} characters)`, fence(t), '']),
    ...got.flatMap((t, k) => [`## Received${got.length > 1 ? ` — ${k + 1} of ${got.length}` : ''}`, fence(t, /^\s*[{[]/.test(t) ? 'json' : 'text'), '']),
    '## What I need',
    f.vote === 'down' || e.ok === false
      ? `This answer ${e.ok === false ? 'failed' : 'didn’t help the person'}. Find why from what was sent and received above, and fix it in the code listed (the prompt, the context it was given, or how the answer is checked and used) so the next request does better. Identities in it are placeholders ([CLIENT], [P-1], [ID-2]).`
      : 'Look at this request and tell me what could be better.',
  ];
  return out.join('\n');
}

// "Copy for Claude" on any AI request shown in Admin (the request is kept by its key while shown)
const SHOWN_REQUESTS = new Map();
document.addEventListener('click', async (ev) => {
  const b = ev.target.closest('[data-copyai]');
  if (!b) return;
  ev.stopPropagation();
  const e = SHOWN_REQUESTS.get(b.dataset.copyai);
  if (!e) return;
  try { await navigator.clipboard.writeText(requestForClaude(e)); toast('Copied — paste it to Claude.', 'info', 1800); } catch (err) { toast(String(err), 'error'); }
}, true);

/** An AI request in full: tokens, time, key, what was asked, sent (each try) and received. */
function aiRequestHtml(e) {
  const fact = (l, v) => `<div><i>${esc(l)}</i><b>${v}</b></div>`;
  const sent = e.sent || [], got = e.received || [];
  const key = `${e.at}|${e.user}`;
  SHOWN_REQUESTS.set(key, e);
  return `<div class="lg-copyai"><button class="btn sm" data-copyai="${esc(key)}" title="The request, what the person said, what was sent and received, and where its code is — as Markdown to paste to Claude">${icon('copy', 12)} Copy for Claude</button></div>${feedbackBlock(e.feedback)}<div class="lgd-facts four">${fact('Tokens sent', tok(e.tokens_in))}${fact('Tokens received', tok(e.tokens_out))}${fact('Time', e.ms ? (e.ms / 1000).toFixed(1) + ' s' : '—')}${fact('Key', e.key === 'bank' ? 'Bank key' : e.key ? 'Own key' : '—')}</div>
    <div class="lgd-sec"><span>The person asked</span><div class="lgd-text">${esc(e.prompt || '— (a button, no words)')}</div></div>
    ${sent.length ? sent.map((t, k) => `<details class="lgd-sec"${k === sent.length - 1 ? ' open' : ''}><summary>Sent to the AI${sent.length > 1 ? ` · try ${k + 1} of ${sent.length}` : ''} <i>${Number(t.length).toLocaleString()} characters</i></summary><pre>${esc(t)}</pre></details>`).join('')
      : '<div class="hint">This request was logged before the log kept what was sent.</div>'}
    ${got.map((t, k) => `<details class="lgd-sec"${k === got.length - 1 ? ' open' : ''}><summary>Received${got.length > 1 ? ` · ${k + 1} of ${got.length}` : ''} <i>${Number(t.length).toLocaleString()} characters</i></summary><pre>${esc(t)}</pre></details>`).join('')}
    <div class="hint">What was sent is as the AI saw it: client names, account numbers and instruments appear as codes ([CLIENT], [ID-1], [INS-2]).</div>`;
}

/**
 * Rows that open a full-width panel right under themselves (one at a time): click a row to open
 * it, click it again or press Esc to close it, ↑ / ↓ to move to the row above or below.
 * `fill(tr, panel)` draws the panel.
 */
function expandable(table, colspan, fill) {
  let openTr = null;
  const close = () => {
    table.querySelector('tr.lg-open')?.remove();
    openTr?.classList.remove('on');
    openTr = null;
  };
  const open = (tr) => {
    if (openTr === tr) { close(); return; }
    close();
    openTr = tr;
    tr.classList.add('on');
    const row = document.createElement('tr');
    row.className = 'lg-open';
    row.innerHTML = `<td colspan="${colspan}"><div class="lg-panel"></div></td>`;
    tr.after(row);
    fill(tr, row.querySelector('.lg-panel'));
    table.focus({ preventScroll: true });
    requestAnimationFrame(() => tr.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
  };
  table.tabIndex = 0;
  table.querySelectorAll('tr[data-k]').forEach((tr) => tr.addEventListener('click', (e) => { if (!e.target.closest('.lg-open')) open(tr); }));
  table.onkeydown = (e) => {
    if (e.target.closest('input, textarea, select, button')) return;
    if (e.key === 'Escape' && openTr) { e.preventDefault(); e.stopPropagation(); const t = openTr; close(); t.scrollIntoView({ block: 'nearest' }); return; }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const rows = [...table.querySelectorAll('tr[data-k]')];
    const at = openTr ? rows.indexOf(openTr) : -1;
    const next = rows[Math.max(0, Math.min(rows.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)))];
    if (next && next !== openTr) { e.preventDefault(); open(next); }
  };
  return { close };
}

// a person's roles as small icons beside their name (an administrator: the shield alone — Admin has every role)
const ROLES_OF = {};
const learnRoles = (users) => { for (const u of users || []) ROLES_OF[u.login] = u.roles; };
const ROLE_ICON = [['admin', 'shield', 'Admin'], ['data', 'database', 'Data'], ['design', 'brush', 'Design'], ['user', 'page', 'User (reports)']];
function roleIcons(login) {
  const r = ROLES_OF[login];
  if (!r) return '';
  const shown = r.admin ? ROLE_ICON.slice(0, 1) : ROLE_ICON.filter(([k]) => r[k]);
  return shown.length ? `<span class="lg-roles" title="${esc(shown.map((x) => x[2]).join(', '))}">${shown.map(([k, ic]) => `<i class="rl-${k}">${icon(ic, 11)}</i>`).join('')}</span>` : '';
}

// a session in one line, for an event's "Part of a session" (the sessions are read once per visit)
let sessionsCache = null;
async function sessionLine(id) {
  try {
    sessionsCache = sessionsCache || call('work_sessions', { filter: { limit: 5000 } }, { quiet: true }).then((r) => r.sessions);
    const x = (await sessionsCache).find((y) => y.id === id);
    if (!x) return 'that session';
    const when = (x.start || '').replace('T', ' ').slice(0, 16);
    return x.kind === 'admin' ? `Administration · ${when} · ${fmtDur(x.active_s)} active`
      : `${x.client || x.title || ''} · ${x.title || ''} · ${when} · ${fmtDur(x.active_s)} active`;
  } catch { return 'that session'; }
}

// what a person said about an AI answer: a mark for rows, a block for the request's details
const voteMark = (f) => (f?.vote === 'up' ? `<span class="lg-vote up" title="Helpful">${icon('thumbs-up', 12)}</span>`
  : f?.vote === 'down' ? `<span class="lg-vote down" title="Not helpful${f.reasons?.length ? ` — ${esc(f.reasons.join(', '))}` : ''}${f.comment ? ` — “${esc(f.comment)}”` : ''}">${icon('thumbs-down', 12)}</span>` : '');
const feedbackBlock = (f) => (!f?.vote ? '' : `<div class="lg-fb ${f.vote}">${icon(f.vote === 'up' ? 'thumbs-up' : 'thumbs-down', 14)}<div><b>${f.vote === 'up' ? 'The person found this helpful' : 'The person found this not helpful'}</b>
  ${f.reasons?.length ? `<span>${f.reasons.map(esc).join(' · ')}</span>` : ''}${f.comment ? `<span>“${esc(f.comment)}”</span>` : ''}</div></div>`);

// Sessions | All events, at the top of Activity
const viewSwitch = (v) => `<div class="seg lg-view"><button data-v="sessions" class="${v === 'sessions' ? 'on' : ''}">${icon('clock', 13)} Sessions</button><button data-v="events" class="${v === 'events' ? 'on' : ''}">${icon('table', 13)} All events</button></div>`;
const bindSwitch = (el) => {
  el.querySelectorAll('.lg-view button').forEach((b) => b.addEventListener('click', () => { S.actView = b.dataset.v; render(); }));
  el.querySelector('[data-a="askai"]')?.addEventListener('click', () => toggleAdminAI('ask'));
};

/** Admin → Activity: work sessions (default), or every event newest first, with filters, details and CSV export. */
export async function activityPage(el) {
  sessionsCache = null;
  if ((S.actView || 'sessions') === 'sessions') return sessionsPage(el);
  const f = (S.actFilter = { user: '', area: '', result: '', from: '', to: '', q: '', ...(S.actFilter || {}) });
  el.innerHTML = `${areaHead('admin', 'What people did in Quire: sign-ins (failed ones too), administration, files, every edit, data work and AI requests. For finding out what happened and why.',
    `${viewSwitch('events')}<button class="btn" data-a="askai">${icon('sparkles', 14)} Ask AI</button><button class="btn" data-a="reload">${icon('refresh', 14)} Refresh</button><button class="btn" data-a="csv">${icon('download', 14)} Export CSV</button>`, 'Activity')}
    <section class="ar-card lg-filters">
      <div class="lg-frow">
        <div class="lg-search">${icon('search', 14)}<input name="q" placeholder="Search in everything: names, documents, SQL, errors…" value="${esc(f.q)}"></div>
        <select name="user"><option value="">Everyone</option></select>
        <select name="result"><option value="">Any result</option><option value="failed">Failed only</option><option value="ok">Succeeded</option><option value="unhelpful">AI: not helpful 👎</option><option value="helpful">AI: helpful 👍</option></select>
      </div>
      <div class="lg-frow">
        <div class="lg-areas"><button data-area="" class="${f.area ? '' : 'on'}">All</button>${AREAS_LOG.map(([k, l, ic]) => `<button data-area="${k}" class="ar-${k}${f.area === k ? ' on' : ''}">${icon(ic, 12)} ${esc(l)}</button>`).join('')}</div>
        <span class="grow"></span>
        <div class="lg-dates"><button data-d="0">Today</button><button data-d="6">7 days</button><button data-d="29">30 days</button><button data-d="">Any time</button>
          <input type="date" name="from" value="${esc(f.from)}"><span>–</span><input type="date" name="to" value="${esc(f.to)}"></div>
      </div>
    </section>
    <div class="lg-stats"></div>
    <section class="ar-card lg-tablecard"><div class="lg-table"><div class="ar-note">Loading…</div></div></section>
    <div class="lg-hint">${icon('info', 12)} Click an event to see everything about it · ↑ ↓ to move · Esc to close</div>`;
  bindSwitch(el);
  let names = {};
  let rows = [], shown = 300, selAt = null;
  const table = el.querySelector('.lg-table');
  let det = null;
  const nameOf = (login) => names[login] || login || '—';
  try { const us = (await call('auth', { cmd: 'users' }, { quiet: true })).users; names = Object.fromEntries(us.map((u) => [u.login, u.name || u.login])); learnRoles(us); } catch {}

  const load = async () => {
    table.innerHTML = '<div class="ar-note">Loading…</div>';
    let r;
    try { r = await call('activity', { filter: { ...f, limit: 5000 } }, { quiet: true }); } catch (e) { table.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
    rows = r.rows; shown = 300;
    const us = el.querySelector('select[name=user]');
    us.innerHTML = `<option value="">Everyone</option>${r.users.map((u) => `<option value="${esc(u)}"${f.user === u ? ' selected' : ''}>${esc(nameOf(u))}${names[u] && names[u] !== u ? ` (${esc(u)})` : ''}</option>`).join('')}`;
    const ai = rows.filter((x) => x.ai);
    const st = (n, l, tone = '') => `<div class="ac-tile ${tone}"><b>${n}</b><span>${esc(l)}</span></div>`;
    el.querySelector('.lg-stats').innerHTML = [
      st(rows.length.toLocaleString() + (rows.length >= 5000 ? '+' : ''), `events${r.total > rows.length ? ` of ${r.total.toLocaleString()}` : ''}`),
      st(new Set(rows.map((x) => x.user).filter(Boolean)).size, 'people'),
      st(rows.filter((x) => x.ok === false).length, 'failed', rows.some((x) => x.ok === false) ? 'bad' : ''),
      st(ai.length, 'requests to AI', 'fig'),
      st(fmtTok(ai.reduce((a, x) => a + (x.tokens_in || 0), 0)), 'tokens sent to AI'),
      st(fmtTok(ai.reduce((a, x) => a + (x.tokens_out || 0), 0)), 'tokens received'),
    ].join('');
    draw();
  };
  const draw = () => {
    if (!rows.length) { table.innerHTML = '<div class="ar-note">Nothing matches. Try a wider time or fewer filters.</div>'; return; }
    let day = '', out = '';
    rows.slice(0, shown).forEach((r, k) => {
      const d = (r.at || '').slice(0, 10);
      if (d !== day) { day = d; out += `<tr class="lg-day"><td colspan="6">${esc(dayTitle(d))} <span>${rows.filter((x) => (x.at || '').startsWith(d)).length}</span></td></tr>`; }
      out += `<tr data-k="${k}" class="${r.ok === false ? 'bad' : ''}${r.at === selAt ? ' on' : ''}">
        <td class="lg-time">${esc((r.at || '').slice(11, 19))}</td>
        <td><div class="lg-who"><span class="av sm">${esc(initials(nameOf(r.user)))}</span><span>${esc(nameOf(r.user))}</span>${roleIcons(r.user)}</div></td>
        <td><span class="lg-area ar-${esc(r.area)}">${esc(areaName(r.area))}</span></td>
        <td class="lg-act">${esc(actionName(r))}</td>
        <td class="lg-det"><span>${r.target ? `<b>${esc(r.target)}</b> ` : ''}${esc(r.ai ? (r.detail || '').replace(/ · [\d,]+ → [\d,]+ tokens.*$/, '') : r.detail || '')}</span>${r.ok === false && r.error ? `<i>${esc(r.error)}</i>` : ''}</td>
        <td class="lg-r">${voteMark(r.feedback)}${r.ai && r.tokens_in != null ? `<span class="lg-tok" title="Tokens sent → received">${fmtTok(r.tokens_in)} → ${fmtTok(r.tokens_out)}</span>` : ''}${r.ok === false ? `<span class="lg-bad" title="Failed">${icon('warn', 13)}</span>` : ''}${r.session ? `<button class="lg-sess" data-sess="${esc(r.session)}" title="Open the session this is part of">${icon('clock', 11)} session</button>` : ''}</td></tr>`;
    });
    table.innerHTML = `<table class="lg-t"><thead><tr><th>Time</th><th>Person</th><th>Area</th><th>What</th><th>Details</th><th></th></tr></thead><tbody>${out}</tbody></table>
      ${rows.length > shown ? `<div class="lg-more"><button class="btn sm">Show ${Math.min(300, rows.length - shown)} more · ${rows.length - shown} left</button></div>` : ''}`;
    table.querySelectorAll('[data-sess]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); S.actView = 'sessions'; S.openSession = b.dataset.sess; render(); }));
    expandable(table.querySelector('table'), 6, (tr, panel) => {
      const r = rows[+tr.dataset.k];
      selAt = r.at;
      det = panel;
      showDetail(r);
    });
    // from an AI answer's link: that event, opened
    if (S.openEvent) {
      const k = rows.slice(0, shown).findIndex((x) => x.at === S.openEvent.at && x.user === S.openEvent.user);
      S.openEvent = null;
      const tr = k >= 0 && table.querySelector(`tr[data-k="${k}"]`);
      // opened from a link: it and its details at the top of the screen
      if (tr) { tr.click(); setTimeout(() => tr.scrollIntoView({ block: 'start', behavior: 'smooth' }), 60); }
    }
    table.querySelector('.lg-more button')?.addEventListener('click', () => { shown += 300; draw(); });
  };

  const showDetail = async (r) => {
    const fact = (l, v) => `<div><i>${esc(l)}</i><b>${v}</b></div>`;
    const head = `<div class="lgd-head"><span class="lg-area ar-${esc(r.area)}">${esc(areaName(r.area))}</span><h3>${esc(actionName(r))}</h3>${r.ok === false ? '<span class="lgd-bad">Failed</span>' : '<span class="lgd-ok">OK</span>'}</div>
      <div class="lgd-facts">${fact('When', esc((r.at || '').replace('T', ' ').slice(0, 19)))}${fact('Person', `${esc(nameOf(r.user))}${names[r.user] && names[r.user] !== r.user ? ` <span class="muted">${esc(r.user)}</span>` : ''}`)}${r.target ? fact(r.ai ? 'Provider · model' : 'Subject', esc(r.target)) : ''}</div>
      ${r.error ? `<div class="lgd-err">${icon('warn', 14)} ${esc(r.error)}</div>` : ''}`;
    const partOf = r.session ? `<div class="ev-sess"><span class="ev-sess-ic">${icon('clock', 14)}</span><div><i>Part of a session</i><b class="ev-sess-what">…</b></div><button class="btn sm primary" data-session="${esc(r.session)}">Open session ${icon('chevron-right', 12)}</button></div>` : '';
    const toSession = (r.ok === false ? `<button class="btn sm aa-explain" data-explain="1">${icon('sparkles', 12)} Explain with AI</button><div class="aa-why"></div>` : '');
    const bindSession = () => {
      det.querySelector('[data-session]')?.addEventListener('click', (e) => { e.stopPropagation(); S.actView = 'sessions'; S.openSession = r.session; render(); });
      // the session in words: whose work, on which client and report, when, for how long
      const what = det.querySelector('.ev-sess-what');
      if (what) sessionLine(r.session).then((t) => { what.textContent = t; });
      det.querySelector('[data-explain]')?.addEventListener('click', async (e) => {
        e.stopPropagation();
        const b = e.currentTarget, out = det.querySelector('.aa-why');
        b.disabled = true;
        out.innerHTML = `<div class="ar-note"><span class="spin"></span> Reading what happened around it…</div>`;
        try {
          const x = await call('ai_admin', { kind: 'explain', prompt: '', at: r.at, of: r.user, lang: appLanguage() }, { quiet: true });
          out.innerHTML = `<div class="aa-card"><div class="aa-h">${icon('sparkles', 13)} Most likely</div><p>${rich(x.cause || '')}</p>${x.fix ? `<div class="aa-h">${icon('wrench', 13)} What to do</div><p>${rich(x.fix).replace(/\n/g, '<br>')}</p>` : ''}${linksHtml(x.links)}<div class="aip-meta">${esc(x.provider || '')} · ${esc(x.model || '')}</div></div>`;
          bindLinks(out, x.links || []);
        } catch (err) { out.innerHTML = `<div class="ar-note bad">${icon('warn', 13)} ${esc(String(err))}</div>`; b.disabled = false; }
      });
    };
    if (!r.ai) {
      det.innerHTML = `<div class="ev-wrap"><div class="ev-side">${head}${toSession}</div><div class="ev-main">${partOf}${r.detail ? `<div class="lgd-sec"><span>What happened</span><div class="lgd-text">${esc(r.detail)}</div></div>` : ''}${dataHtml(r) || (r.detail ? '' : '<div class="hint">Nothing more was recorded.</div>')}</div></div>`;
      bindSession();
      return;
    }
    det.innerHTML = `${head}<div class="ar-note">Loading the request…</div>`;
    let e;
    try { e = await call('ai_log_entry', { at: r.at, user: r.user }, { quiet: true }); } catch (x) { det.innerHTML = `${head}<div class="ar-note bad">${esc(String(x))}</div>`; return; }
    // the vote, as the row has it
    if (!e.feedback && r.feedback) e.feedback = r.feedback;
    if (selAt !== r.at) return;
    det.innerHTML = `<div class="ev-wrap"><div class="ev-side">${head}${toSession}</div><div class="ev-main">${partOf}${aiRequestHtml(e)}</div></div>`;
    bindSession();
  };
  // filters
  const set = (patch) => { Object.assign(f, patch); selAt = null; load(); };
  let qt;
  const qIn = el.querySelector('input[name=q]');
  qIn.addEventListener('input', () => { clearTimeout(qt); qt = setTimeout(() => set({ q: qIn.value.trim() }), 300); });
  qIn.addEventListener('keydown', (e) => e.stopPropagation());
  el.querySelector('select[name=user]').addEventListener('change', (e) => set({ user: e.target.value }));
  const res = el.querySelector('select[name=result]');
  res.value = f.result;
  res.addEventListener('change', () => set({ result: res.value }));
  el.querySelectorAll('.lg-areas button').forEach((b) => b.addEventListener('click', () => {
    el.querySelectorAll('.lg-areas button').forEach((x) => x.classList.toggle('on', x === b));
    set({ area: b.dataset.area });
  }));
  const [fromIn, toIn] = [el.querySelector('input[name=from]'), el.querySelector('input[name=to]')];
  fromIn.addEventListener('change', () => set({ from: fromIn.value }));
  toIn.addEventListener('change', () => set({ to: toIn.value }));
  el.querySelectorAll('.lg-dates button').forEach((b) => b.addEventListener('click', () => {
    const from = b.dataset.d === '' ? '' : dayStr(-Number(b.dataset.d));
    fromIn.value = from; toIn.value = '';
    set({ from, to: '' });
  }));
  el.querySelector('[data-a="reload"]').addEventListener('click', load);
  el.querySelector('[data-a="csv"]').addEventListener('click', async () => {
    if (!rows.length) { toast('Nothing to export.', 'info'); return; }
    const cell = (v) => { const s = v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v); return /[",\n;]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s; };
    const cols = ['at', 'user', 'name', 'area', 'action', 'target', 'detail', 'ok', 'error', 'tokens_in', 'tokens_out', 'data'];
    const text = [cols.join(','), ...rows.map((r) => cols.map((c) => cell(c === 'name' ? nameOf(r.user) : r[c])).join(','))].join('\n');
    try {
      const p = await call('save_text_file', { text, name: `Quire activity ${dayStr()}`, ext: 'csv', what: 'CSV' });
      if (p) toast(`Saved ${rows.length} events to ${p.split('/').pop()}`);
    } catch {}
  });
  load();
}

const fmtTok = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e4 ? `${Math.round(n / 1e3)} K` : Number(n || 0).toLocaleString());

// what an event carries: a before/after of an account, SQL, or the raw request
function dataHtml(r) {
  const d = r.data;
  if (d == null || d === '') return '';
  if (d.before !== undefined && d.after) {
    const b = d.before || {}, a = d.after;
    const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])].filter((k) => JSON.stringify(a[k]) !== JSON.stringify(b[k]));
    const v = (x) => (x == null || x === '' ? '<span class="muted">—</span>' : esc(typeof x === 'object' ? JSON.stringify(x) : String(x)));
    return `<div class="lgd-sec"><span>${d.before ? 'Changed' : 'New account'}</span>${keys.length ? `<table class="lgd-diff"><thead><tr><th></th><th>Before</th><th>After</th></tr></thead><tbody>${keys.map((k) => `<tr><td>${esc(k)}</td><td class="was">${v(b[k])}</td><td class="now">${v(a[k])}</td></tr>`).join('')}</tbody></table>` : '<div class="hint">Saved without changes.</div>'}</div>`;
  }
  if (d.sql) return `<div class="lgd-sec"><span>SQL</span><pre>${esc(d.sql)}</pre></div>`;
  if (Array.isArray(d.changes) && d.changes.length) return `<div class="lgd-sec"><span>Changed</span>${changesTable(d.changes)}</div><details class="lgd-sec"><summary>The edit in full</summary><pre>${esc(JSON.stringify(d.op, null, 2))}</pre></details>`;
  return `<details class="lgd-sec"><summary>The request in full</summary><pre>${esc(typeof d === 'string' ? d : JSON.stringify(d, null, 2))}</pre></details>`;
}

// an edit's changes: what each setting was and what it became
const val = (x) => (x == null || x === '' ? '<span class="muted">—</span>' : esc(typeof x === 'object' ? JSON.stringify(x) : String(x)));
function changesTable(ch) {
  return `<table class="lgd-diff"><thead><tr><th></th><th>Before</th><th>After</th></tr></thead><tbody>${ch.map((c) => `<tr><td>${esc(String(c.path || '').replace(/^values\./, ''))}</td><td class="was">${val(c.from)}</td><td class="now">${val(c.to)}</td></tr>`).join('')}</tbody></table>`;
}

// an admin session in a line: what it changed
function adminSummary(x) {
  const ev = x.events.filter((e) => e.area === 'admin' || (e.area === 'ai' && !e.ai));
  const people = new Set(ev.filter((e) => /user/.test(e.action)).map((e) => e.target));
  const parts = [people.size ? `${people.size} ${people.size === 1 ? 'person' : 'people'} changed` : '', ev.some((e) => /group/.test(e.action)) ? 'groups' : '',
    ev.some((e) => e.action === 'audience' || e.action === 'publish') ? 'reports' : '', ev.some((e) => e.area === 'ai') ? 'AI settings' : '', x.ai ? `${x.ai} AI request${x.ai > 1 ? 's' : ''}` : ''].filter(Boolean);
  return parts.length ? parts.join(' · ') : 'looked around';
}

// an account's change (before → after), as the changes table shows it; the password never
function accountChanges(d) {
  const b = d.before || {}, a = d.after || {};
  return [...new Set([...Object.keys(a), ...Object.keys(b)])].filter((k) => k !== 'password' && JSON.stringify(a[k]) !== JSON.stringify(b[k]))
    .map((k) => ({ path: k, from: b[k] ?? null, to: a[k] ?? null }));
}

const fmtDur = (s) => { s = Math.round(s || 0); return s >= 3600 ? `${Math.floor(s / 3600)} h ${String(Math.floor((s % 3600) / 60)).padStart(2, '0')} min` : s >= 60 ? `${Math.floor(s / 60)} min` : `${s} s`; };
const REASONS = { closed: 'closed', 'another client': 'switched to another client', 'signed out': 'signed out', 'app closed': 'app closed', left: 'away from Admin for 30 min',
  'signed in again': 'signed in again', idle: '30 min without activity', 'another document': 'another document opened', rebuilt: 'end of the log' };
// active time: a session rebuilt from the older log has none (it wasn't measured)
const activeText = (x) => (x.active_s == null ? '—' : fmtDur(x.active_s));
const KIND = { report: 'Report', template: 'Template', admin: 'Admin' };

/** Admin → Activity → Sessions: each time someone worked on a report or template for a client. */
async function sessionsPage(el) {
  const f = (S.actFilter = { user: '', area: '', result: '', from: '', to: '', q: '', ...(S.actFilter || {}) });
  el.innerHTML = `${areaHead('admin', 'Each time someone worked on a report or a template, for one client: from opening it until another client, closing it, signing out or quitting. With the template, client, period and parameters, the time spent, every change, saves and exports, and the AI used.',
    `${viewSwitch('sessions')}<button class="btn" data-a="askai">${icon('sparkles', 14)} Ask AI</button><button class="btn" data-a="reload">${icon('refresh', 14)} Refresh</button><button class="btn" data-a="csv">${icon('download', 14)} Export CSV</button>`, 'Activity')}
    <section class="ar-card lg-filters"><div class="lg-frow">
      <div class="lg-search">${icon('search', 14)}<input name="q" placeholder="Search: client, report, change, export, AI question…" value="${esc(f.q)}"></div>
      <select name="user"><option value="">Everyone</option></select>
      <select name="result"><option value="">Any result</option><option value="failed">With failures</option></select>
      <div class="lg-dates"><button data-d="0">Today</button><button data-d="6">7 days</button><button data-d="29">30 days</button><button data-d="">Any time</button>
        <input type="date" name="from" value="${esc(f.from)}"><span>–</span><input type="date" name="to" value="${esc(f.to)}"></div>
    </div></section>
    <div class="lg-stats"></div>
    <section class="ar-card lg-tablecard"><div class="lg-table"><div class="ar-note">Loading…</div></div></section>
    <div class="lg-hint">${icon('info', 12)} Click a session to open it · ↑ ↓ to move · Esc to close</div>`;
  bindSwitch(el);
  let names = {}, list = [], selId = null;
  try { const us = (await call('auth', { cmd: 'users' }, { quiet: true })).users; names = Object.fromEntries(us.map((u) => [u.login, u.name || u.login])); learnRoles(us); } catch {}
  const nameOf = (l) => names[l] || l || '—';
  const table = el.querySelector('.lg-table');
  const when = (at) => (at || '').replace('T', ' ').slice(0, 16);

  const load = async () => {
    table.innerHTML = '<div class="ar-note">Loading…</div>';
    let r;
    try { r = await call('work_sessions', { filter: { ...f, area: '', limit: 1000 } }, { quiet: true }); } catch (e) { table.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
    list = r.sessions;
    const us = el.querySelector('select[name=user]');
    us.innerHTML = `<option value="">Everyone</option>${r.users.filter(Boolean).map((u) => `<option value="${esc(u)}"${f.user === u ? ' selected' : ''}>${esc(nameOf(u))}</option>`).join('')}`;
    const sum = (k) => list.reduce((a, x) => a + (x[k] || 0), 0);
    const st = (n, l, tone = '') => `<div class="ac-tile ${tone}"><b>${n}</b><span>${esc(l)}</span></div>`;
    el.querySelector('.lg-stats').innerHTML = [
      st(list.length, 'sessions'), st(new Set(list.map((x) => x.client).filter(Boolean)).size, 'clients'),
      st(fmtDur(sum('active_s')), 'active time'), st(sum('edits'), 'edits'),
      st(list.reduce((a, x) => a + x.exports.length, 0), 'exports'), st(sum('ai'), 'AI requests', 'fig'),
    ].join('');
    draw();
  };
  const draw = () => {
    if (!list.length) { table.innerHTML = '<div class="ar-note">No sessions yet for these filters. A session starts when someone opens a report or a template.</div>'; return; }
    let day = '', out = '';
    list.forEach((x, k) => {
      const d = (x.start || '').slice(0, 10);
      if (d !== day) { day = d; out += `<tr class="lg-day"><td colspan="6">${esc(dayTitle(d))} <span>${list.filter((y) => (y.start || '').startsWith(d)).length}</span></td></tr>`; }
      const did = [
        x.edits ? `<span class="ss-chip">${icon('brush', 11)} ${x.edits}</span>` : '',
        x.exports.length ? `<span class="ss-chip ex">${icon('file-pdf', 11)} ${x.exports.length}</span>` : '',
        x.ai ? `<span class="ss-chip ai">${icon('sparkles', 11)} ${x.ai}</span>` : '',
        x.failed ? `<span class="lg-bad" title="Something failed">${icon('warn', 13)}</span>` : '',
      ].join('');
      out += `<tr data-k="${k}" class="${x.id === selId ? 'on' : ''}">
        <td class="lg-time">${esc((x.start || '').slice(11, 16))}</td>
        <td><div class="lg-who"><span class="av sm">${esc(initials(nameOf(x.user)))}</span><span>${esc(nameOf(x.user))}</span>${roleIcons(x.user)}</div></td>
        <td class="ss-what"><b>${esc(x.kind === 'admin' ? 'Administration' : x.client || (x.rebuilt ? 'Client not recorded' : '—'))}</b><span><i class="ss-kind ${esc(x.kind || '')}">${esc(KIND[x.kind] || x.kind || '')}</i> ${esc(x.kind === 'admin' ? adminSummary(x) : `${x.title || ''}${x.period ? ` · ${x.period}` : ''}`)}</span></td>
        <td class="lg-r"><b>${activeText(x)}</b><span class="ss-wall">of ${fmtDur(x.wall_s)}</span></td>
        <td class="ss-did">${did}</td>
        <td class="ss-end">${x.open ? '<span class="ss-open">open</span>' : `<span class="muted">${esc(REASONS[x.reason] || x.reason || '')}</span>`}${x.rebuilt ? '<i class="ss-rebuilt" title="Rebuilt from the log kept before sessions existed: no active time">rebuilt</i>' : ''}</td></tr>`;
    });
    table.innerHTML = `<table class="lg-t"><thead><tr><th>Start</th><th>Person</th><th>Client · report</th><th class="r">Active</th><th>Did</th><th class="ss-end">Ended</th></tr></thead><tbody>${out}</tbody></table>`;
    expandable(table.querySelector('table'), 6, (tr, panel) => {
      const x = list[+tr.dataset.k];
      selId = x.id;
      showSession(x, panel);
    });
    // from an event's "Open its session"
    if (S.openSession) {
      const k = list.findIndex((x) => x.id === S.openSession);
      S.openSession = null;
      const tr = k >= 0 && table.querySelector(`tr[data-k="${k}"]`);
      // opened from a link: it and its details at the top of the screen
      if (tr) { tr.click(); setTimeout(() => tr.scrollIntoView({ block: 'start', behavior: 'smooth' }), 60); }
    }
  };

  const ICON = { document: 'page', edit: 'brush', ai: 'sparkles', data: 'database', session: 'clock', admin: 'shield', auth: 'logout' };
  const exportName = (e) => (/PPTX/i.test(e.detail || '') ? 'PowerPoint' : /brief/i.test(e.detail || '') ? 'Brief PDF' : 'PDF');
  const showSession = (x, panel) => {
    const params = Object.entries(x.params || {}).filter(([k]) => !/^client/i.test(k));
    // opening and its "new"/"open" line are one moment: shown once
    const t0 = Date.parse(x.start);
    const how = x.events.find((e) => e.area === 'document' && (e.action === 'new' || e.action === 'open' || e.action === 'new report') && Date.parse(e.at) - t0 < 5000);
    const items = x.events.filter((e) => !(e.area === 'session' && e.action === 'time') && e !== how);
    const kpi = (label, value, sub = '', tone = '') => `<div class="sx-kpi ${tone}"><i>${esc(label)}</i><b>${value}</b>${sub ? `<span>${sub}</span>` : ''}</div>`;
    const dl = (label, value) => (value ? `<dt>${esc(label)}</dt><dd>${value}</dd>` : '');
    const title = (e) => (e.area === 'session' ? ({ start: how?.action === 'open' ? 'Opened from a file' : 'Opened', context: 'Client or period picked', end: 'Session ended' }[e.action] || e.action) : actionName(e));
    const meta = (e) => (e.ai ? `${voteMark(e.feedback)} ${tok(e.tokens_in)} → ${tok(e.tokens_out)} tokens${e.ms ? ` · ${(e.ms / 1000).toFixed(0)} s` : ''}` : e.area === 'session' && e.action === 'end' ? esc(REASONS[x.reason] || x.reason || '') : '');
    const body = (e) => {
      if (e.ai) return `<div class="tl-d">${e.detail ? `“${esc(String(e.detail))}”` : '<span class="muted">Started with a button, no words</span>'}</div>
        <button class="tl-more" data-ai="1">${icon('eye', 12)} Show what was sent and received</button><div class="tl-ai"></div>`;
      const ch = Array.isArray(e.data?.changes) ? e.data.changes : e.data?.after ? accountChanges(e.data) : [];
      const d = e.area === 'session' && e.action === 'start' ? (x.kind === 'admin' ? 'Admin area' : `${esc(x.title || '')}${x.client ? ` · ${esc(x.client)}` : ''}${x.period ? ` · ${esc(x.period)}` : ''}`)
        : e.area === 'session' ? '' : esc(ch.length ? (/^“/.test(e.detail || '') ? String(e.detail).split(' · ')[0] : '') : e.detail || '');
      return `${d ? `<div class="tl-d">${d}</div>` : ''}${ch.length ? changesTable(ch) : ''}`;
    };
    panel.innerHTML = `<div class="sx">${x.rebuilt ? `<div class="ss-note">${icon('info', 13)} Rebuilt from the log kept before sessions existed: grouped by document, client and time, without active time${x.client ? '' : '; the client wasn’t recorded then'}.</div>` : ''}
      <div class="sx-kpis">
        ${kpi('Active time', activeText(x), x.rebuilt ? 'not measured then' : `open ${fmtDur(x.wall_s)}`)}
        ${kpi('Changes', String(x.edits), x.kind === 'admin' ? 'to people, groups, reports and AI' : x.saves ? `saved ${x.saves}×` : 'not saved')}
        ${kpi('Exported', x.exports.length ? esc([...new Set(x.exports.map(exportName))].join(', ')) : '—', x.exports.length ? `${x.exports.length}×` : 'not exported', x.exports.length ? 'ex' : '')}
        ${kpi('AI', x.ai ? `${x.ai} request${x.ai > 1 ? 's' : ''}` : '—', x.ai ? `${tok(x.tokens_in)} → ${tok(x.tokens_out)} tokens` : 'not used', x.ai ? 'ai' : '')}
        ${x.failed ? kpi('Failed', String(x.failed), 'see below', 'bad') : ''}
      </div>
      <div class="sx-body">
        <dl class="sx-ctx">
          ${x.kind === 'admin' ? dl('Work', '<b>Administration</b>') : dl('Client', `<b>${esc(x.client || '—')}</b>`)}
          ${x.kind === 'admin' ? '' : dl(KIND[x.kind] || 'Document', esc(x.title || '—'))}
          ${x.template && x.template !== x.title ? dl('From template', esc(x.template)) : ''}
          ${x.kind === 'admin' ? '' : dl('Period', esc(x.period || '—'))}
          ${params.map(([k, v]) => dl(k, val(Array.isArray(v) ? v.join(', ') : v))).join('')}
          ${dl('Person', esc(nameOf(x.user)))}
          ${dl('Started', esc(when(x.start)))}
          ${dl(x.open ? 'Last action' : 'Ended', `${esc(when(x.end))}${x.open ? ' <span class="ss-open">open</span>' : `<br><span class="muted">${esc(REASONS[x.reason] || x.reason || '')}</span>`}`)}
        </dl>
        <div class="sx-tl">${items.map((e, i) => `<div class="tl-ev a-${esc(e.area)}${e.ok === false ? ' bad' : ''}" data-i="${i}">
          <span class="tl-t">${esc((e.at || '').slice(11, 19))}</span>
          <span class="tl-dot">${icon(ICON[e.area] || 'clock', 12)}</span>
          <div class="tl-main"><div class="tl-h"><b>${esc(title(e))}</b><span class="tl-meta">${meta(e)}</span></div>${body(e)}${e.error ? `<div class="tl-err">${icon('warn', 12)} ${esc(e.error)}</div>` : ''}</div>
        </div>`).join('')}</div>
      </div></div>`;
    // an AI request opens in place: what the person asked, what was sent, what came back
    panel.querySelectorAll('.tl-more').forEach((b) => b.addEventListener('click', async (ev) => {
      ev.stopPropagation();
      const row = b.closest('.tl-ev'), box = row.querySelector('.tl-ai');
      if (box.innerHTML) { box.innerHTML = ''; b.innerHTML = `${icon('eye', 12)} Show what was sent and received`; return; }
      const e = items[+row.dataset.i];
      box.innerHTML = '<div class="ar-note">Loading…</div>';
      try { const full = await call('ai_log_entry', { at: e.at, user: e.user }, { quiet: true }); if (!full.feedback && e.feedback) full.feedback = e.feedback; box.innerHTML = aiRequestHtml(full); b.innerHTML = `${icon('chevron-up', 12)} Hide the request`; } catch (err) { box.innerHTML = `<div class="ar-note bad">${esc(String(err))}</div>`; }
    }));
  };

  // filters
  const set = (patch) => { Object.assign(f, patch); selId = null; load(); };
  let qt;
  const qIn = el.querySelector('input[name=q]');
  qIn.addEventListener('input', () => { clearTimeout(qt); qt = setTimeout(() => set({ q: qIn.value.trim() }), 300); });
  qIn.addEventListener('keydown', (e) => e.stopPropagation());
  el.querySelector('select[name=user]').addEventListener('change', (e) => set({ user: e.target.value }));
  const res = el.querySelector('select[name=result]');
  res.value = f.result === 'failed' ? 'failed' : '';
  res.addEventListener('change', () => set({ result: res.value }));
  const [fromIn, toIn] = [el.querySelector('input[name=from]'), el.querySelector('input[name=to]')];
  fromIn.addEventListener('change', () => set({ from: fromIn.value }));
  toIn.addEventListener('change', () => set({ to: toIn.value }));
  el.querySelectorAll('.lg-dates button').forEach((b) => b.addEventListener('click', () => {
    const from = b.dataset.d === '' ? '' : dayStr(-Number(b.dataset.d));
    fromIn.value = from; toIn.value = '';
    set({ from, to: '' });
  }));
  el.querySelector('[data-a="reload"]').addEventListener('click', load);
  el.querySelector('[data-a="csv"]').addEventListener('click', async () => {
    if (!list.length) { toast('Nothing to export.', 'info'); return; }
    const cell = (v) => { const t = v == null ? '' : typeof v === 'object' ? JSON.stringify(v) : String(v); return /[",\n;]/.test(t) ? `"${t.replace(/"/g, '""')}"` : t; };
    const cols = ['start', 'end', 'user', 'name', 'kind', 'title', 'template', 'client', 'period', 'active_min', 'open_min', 'edits', 'saves', 'exports', 'ai', 'tokens_in', 'tokens_out', 'ended', 'params'];
    const row = (x) => ({ ...x, name: nameOf(x.user), active_min: Math.round((x.active_s || 0) / 6) / 10, open_min: Math.round((x.wall_s || 0) / 6) / 10, exports: x.exports.length, ended: x.open ? 'open' : x.reason });
    const text = [cols.join(','), ...list.map((x) => { const r = row(x); return cols.map((c) => cell(r[c])).join(','); })].join('\n');
    try { const p = await call('save_text_file', { text, name: `Quire sessions ${dayStr()}`, ext: 'csv', what: 'CSV' }); if (p) toast(`Saved ${list.length} sessions to ${p.split('/').pop()}`); } catch {}
  });
  load();
}


// ---------------------------------------------------------------- proposals

/** Admin → Proposals: every proposal bankers have made — for support: what it said, every version, its
 * status and history, which report and client, where it came from (the AI request behind it). */
export async function proposalsAdminPage(el) {
  const f = (S.propFilter = { q: '', status: '', banker: '', ...(S.propFilter || {}) });
  el.innerHTML = `${areaHead('admin', 'Every proposal bankers have put in reports: what it said, each earlier version, its status and history, and where it came from. For support and review.',
    `<button class="btn" data-a="reload">${icon('refresh', 14)} Refresh</button>`, 'Proposals')}
    <section class="ar-card lg-filters"><div class="lg-frow">
      <div class="lg-search">${icon('search', 14)}<input name="q" placeholder="Search: client, title, actions, notes…" value="${esc(f.q)}"></div>
      <select name="banker"><option value="">Every banker</option></select>
      <select name="status"><option value="">Any status</option>${STATUSES.map(([k, l]) => `<option value="${k}"${f.status === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>
    </div></section>
    <div class="lg-stats"></div>
    <section class="ar-card lg-tablecard"><div class="lg-table"><div class="ar-note">Loading…</div></div></section>
    <div class="lg-hint">${icon('info', 12)} Click a proposal to see everything about it · ↑ ↓ to move · Esc to close</div>`;
  const table = el.querySelector('.lg-table');
  let all = [];
  let names = {};
  try { const us = (await call('auth', { cmd: 'users' }, { quiet: true })).users; names = Object.fromEntries(us.map((u) => [u.login, u.name || u.login])); learnRoles(us); } catch {}
  const nameOf = (login) => names[login] || login || '—';
  const SOURCE = { notes: 'Laid out from notes', ai: 'Written up by AI from notes', draft: 'Drafted by AI (proposal ideas)' };
  const draw = () => {
    const q = f.q.trim().toLowerCase();
    const rows = all.filter((r) => (!f.status || r.status === f.status) && (!f.banker || r.created_by === f.banker) && (!q || JSON.stringify([r.place, r.content, r.notes, r.history]).toLowerCase().includes(q)));
    const n = (k) => all.filter((r) => r.status === k).length;
    const st = (v, l, tone = '') => `<div class="ac-tile ${tone}"><b>${v}</b><span>${esc(l)}</span></div>`;
    el.querySelector('.lg-stats').innerHTML = [st(all.length, 'proposals'), st(n('draft'), 'drafts'), st(n('presented'), 'presented, no answer yet', 'fig'), st(n('accepted') + n('partly'), 'accepted (or partly)', 'ok'), st(n('declined'), 'declined', n('declined') ? 'bad' : ''),
      st(new Set(all.map((r) => r.place?.client)).size, 'clients')].join('');
    if (!rows.length) { table.innerHTML = `<div class="ar-note">${all.length ? 'Nothing matches.' : 'No proposals yet. They appear here as bankers add them to reports.'}</div>`; return; }
    table.innerHTML = `<table class="lg-t"><thead><tr><th>Updated</th><th>Banker</th><th>Client</th><th>Proposal</th><th>Status</th><th></th></tr></thead><tbody>${rows.map((r, k) => `<tr data-k="${k}">
        <td class="lg-time">${esc(String(r.updated || '').slice(0, 10))} ${esc(String(r.updated || '').slice(11, 16))}</td>
        <td><div class="lg-who"><span class="av sm">${esc(initials(nameOf(r.created_by)))}</span><span>${esc(nameOf(r.created_by))}</span>${roleIcons(r.created_by)}</div></td>
        <td>${esc(r.place?.client || '')}</td>
        <td class="lg-det"><span><b>${esc(r.content?.title || '')}</b> ${esc((r.content?.actions || []).length)} action${(r.content?.actions || []).length === 1 ? '' : 's'}${(r.versions || []).length ? ` · ${r.versions.length} earlier version${r.versions.length === 1 ? '' : 's'}` : ''}${r.in_report === false ? ' · removed from its report' : ''}</span></td>
        <td>${statusPill(r.status)}</td>
        <td class="lg-r">${r.source === 'notes' ? '' : `<span class="lg-tok" title="${esc(SOURCE[r.source] || '')}">${icon('sparkles', 11)} AI</span>`}</td></tr>`).join('')}</tbody></table>`;
    expandable(table.querySelector('table'), 6, (tr, box) => fill(rows[+tr.dataset.k], box));
  };
  // everything about one proposal: for answering "what did it say, who changed what, where did it come from"
  const fill = (r, box) => {
    const p = r.place || {};
    const ver = (r.versions || []).slice().reverse();
    box.innerHTML = `<div class="pa-grid">
      <div class="pa-main">
        <div class="pa-h">${statusPill(r.status)}<b>${esc(r.content?.title || '')}</b><span class="grow"></span>
          <button class="btn sm" data-a="status">Status…</button><button class="btn sm" data-a="copy" title="Everything about this proposal as text, for a support ticket or to paste to Claude">${icon('copy', 12)} Copy for support</button></div>
        ${proposalBodyHtml(r.content || {}, { title: false })}
        ${r.notes ? `<details class="pa-notes"><summary>The banker's notes</summary><pre>${esc(r.notes)}</pre></details>` : ''}
        ${ver.length ? `<div class="pa-sec"><b>Earlier versions</b>${ver.map((v, i) => `<details class="pa-ver"><summary>${esc(String(v.at || '').slice(0, 16).replace('T', ' '))} · ${esc(nameOf(v.by))} · ${esc(SOURCE[v.source] || v.source || '')} · <i>${esc(v.content?.title || '')}</i></summary>${proposalBodyHtml(v.content || {}, { title: false })}</details>`).join('')}</div>` : ''}
      </div>
      <div class="pa-side">
        <div class="pa-kv"><span>Client</span><b>${esc(p.client || '')}${p.client_id ? ` · ${esc(p.client_id)}` : ''}</b></div>
        <div class="pa-kv"><span>Report</span><b>${esc(p.report || '')}</b></div>
        <div class="pa-kv"><span>Period</span><b>${esc(p.period || '')}</b></div>
        ${p.file ? `<div class="pa-kv"><span>Saved as</span><b class="pa-file">${esc(p.file)}</b></div>` : ''}
        <div class="pa-kv"><span>Pages</span><b>${esc(String(p.pages || ''))}${r.in_report === false ? ' · removed from the report' : ''}</b></div>
        <div class="pa-kv"><span>Made</span><b>${esc(String(r.created || '').slice(0, 16).replace('T', ' '))} · ${esc(nameOf(r.created_by))}</b></div>
        <div class="pa-kv"><span>Source</span><b>${esc(SOURCE[r.source] || r.source || '')}</b></div>
        ${r.ai_at ? `<div class="pa-kv"><span>AI request</span><button class="btn sm" data-a="ai">${icon('sparkles', 12)} Open in Activity</button></div>` : ''}
        ${r.exported ? `<div class="pa-kv"><span>Exported</span><b>${esc(String(r.exported).slice(0, 16).replace('T', ' '))}</b></div>` : ''}
        <div class="pa-sec"><b>Status history</b>${(r.history || []).map((x) => `<div class="pa-h1">${esc(String(x.at || '').slice(0, 16).replace('T', ' '))} · ${statusPill(x.status)} · ${esc(nameOf(x.by))}${x.note ? `<i>“${esc(x.note)}”</i>` : ''}</div>`).join('')}</div>
        <div class="pa-kv muted"><span>Register id</span><code>${esc(r.uid)}</code></div>
      </div></div>`;
    box.querySelector('[data-a="status"]').addEventListener('click', (e) => statusMenu(e.currentTarget, r, () => load()));
    box.querySelector('[data-a="copy"]').addEventListener('click', () => navigator.clipboard?.writeText(supportText(r, nameOf, SOURCE)).then(() => toast('Copied.', 'info', 1200)));
    box.querySelector('[data-a="ai"]')?.addEventListener('click', () => {
      S.adminTab = 'activity'; S.actView = 'events';
      S.actFilter = { ...(S.actFilter || {}), area: 'ai', q: '', user: '', from: String(r.ai_at).slice(0, 10), to: String(r.ai_at).slice(0, 10) };
      S.openEvent = { at: r.ai_at, user: r.updated_by || r.created_by };
      render();
    });
  };
  const load = async () => {
    table.innerHTML = '<div class="ar-note">Loading…</div>';
    try { all = await call('proposals_list', { all: true }, { quiet: true }); } catch (e) { table.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
    const bs = [...new Set(all.map((r) => r.created_by).filter(Boolean))];
    el.querySelector('select[name=banker]').innerHTML = `<option value="">Every banker</option>${bs.map((b) => `<option value="${esc(b)}"${f.banker === b ? ' selected' : ''}>${esc(nameOf(b))}</option>`).join('')}`;
    draw();
  };
  el.querySelector('[name=q]').addEventListener('input', (e) => { f.q = e.target.value; draw(); });
  el.querySelector('[name=status]').addEventListener('change', (e) => { f.status = e.target.value; draw(); });
  el.querySelector('[name=banker]').addEventListener('change', (e) => { f.banker = e.target.value; draw(); });
  el.querySelector('[data-a="reload"]').addEventListener('click', load);
  load();
}

/** A proposal as plain text for support: where, who, what, versions, history. */
function supportText(r, nameOf, SOURCE) {
  const c = r.content || {}, p = r.place || {};
  const acts = (c.actions || []).map((a, i) => `  ${i + 1}. ${[a.action, a.what, a.amount].filter(Boolean).join(' · ')}${a.why ? ` — ${a.why}` : ''}`);
  const list = (h, xs) => ((xs || []).length ? [`${h}:`, ...xs.map((x) => `  - ${String(x).replace(/\*\*/g, '')}`)] : []);
  return [`# Proposal ${r.uid}`, `Client: ${p.client || ''}${p.client_id ? ` (${p.client_id})` : ''} · Report: ${p.report || ''} · Period: ${p.period || ''}${p.file ? ` · File: ${p.file}` : ''}`,
    `Made ${r.created} by ${nameOf(r.created_by)} · updated ${r.updated} by ${nameOf(r.updated_by)} · source: ${SOURCE[r.source] || r.source}${r.ai_at ? ` (AI request at ${r.ai_at})` : ''}`,
    `Status: ${statusName(r.status)}${r.in_report === false ? ' · removed from its report' : ''}${r.exported ? ` · exported ${r.exported}` : ''}`, '',
    `## ${c.title || ''}`, String(c.lead || '').replace(/\*\*/g, ''), '', 'Actions:', ...acts, ...list('Expected effect', c.effect), ...list('Why now', c.reasons), ...list('Risks', c.risks), ...list('Next steps', c.steps), '',
    '## Status history', ...(r.history || []).map((x) => `- ${x.at} · ${statusName(x.status)} · ${nameOf(x.by)}${x.note ? ` — “${x.note}”` : ''}`),
    ...((r.versions || []).length ? ['', '## Earlier versions', ...r.versions.map((v) => `- ${v.at} · ${nameOf(v.by)} · ${v.content?.title || ''}`)] : []),
    ...(r.notes ? ['', "## The banker's notes", r.notes] : [])].join('\n');
}
