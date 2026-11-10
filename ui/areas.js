// The home screen once signed in: a bar with the areas the person's roles
// open (User, Design, Data, Admin) and the chosen area below it.
//
// - User: reports for the clients in the person's access.
// - Design: templates and themes (the designer's start page, from app.js).
// - Data: the shared connections and their SQL workspaces.
// - Admin: accounts, passwords, roles and client access.

import { setAreaAI } from './ai.js';
import { KEYS_AT, WEB } from './platform.js';
import { appLanguage } from './i18n.js';
import { accessPage, aiAdminPage, activityPage, proposalsAdminPage } from './admin.js';
import { apiAdminPage } from './apiadmin.js';
import { toggleAreaFold } from './rail.js';
import { batchDialog } from './batch.js';
import { proposalBodyHtml, statusPill, statusMenu, statusName } from './proposal.js';
import { toggleAdminAI, mountAdminAI } from './aiadmin.js';
import { S, api, call, edit, render, setState } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { menu, modal, toast } from './ui.js';
import { AREAS, session, currentArea, initials, hasRole } from './auth.js';
import { sourceLabel, srcTag, datasetEditor, calcFieldEditor, paramEditor, ownSourceKinds, sharedSourceKinds } from './panels.js';
import { workspace, connectionDialog, connWhere, editDbSource, gridHtml } from './dbx.js';

/** The bank's products (accounts.src_cd), as its `products` table names them — learnt from Admin's options; else the code. */
const PRODUCTS = {};
export const learnProducts = (list) => { for (const x of list || []) PRODUCTS[x.id] = x; };
const ru = () => appLanguage() === 'ru';
export const srcName = (id) => { const p = PRODUCTS[id]; return (p && (ru() ? p.name || p.name_en : p.name_en || p.name)) || id; };
export const srcDesc = (id) => { const p = PRODUCTS[id]; return (p && (ru() ? p.description || p.description_en : p.description_en || p.description)) || ''; };

/** Draws the chosen area's page into `home`; `designHome(el)` fills Templates. */
export function renderAreas(home, { designHome }) {
  const area = currentArea();
  document.body.classList.remove('adm-ai-on');
  home.innerHTML = `<main class="ar-body" data-area="${area}"></main>`;
  const body = home.querySelector('.ar-body');
  if (area === 'design') designHome(body);
  else if (area === 'user') userArea(body);
  else if (area === 'data') dataArea(body);
  else if (area === 'admin') adminArea(body);
}

export const areaHead = (a, lead, actions = '', title = '') => `<div class="ar-head"><span class="ar-ic">${icon(AREAS.find((x) => x.key === a).icon, 20)}</span><div><h2>${esc(title || AREAS.find((x) => x.key === a).title)}</h2><p>${lead}</p></div><span class="grow"></span>${actions}</div>`;

// ---------------------------------------------------------------- User


async function userArea(el) {
  const s = session();
  el.innerHTML = `${areaHead('user', s.restricted ? `You report on <b>${s.clients}</b> client${s.clients === 1 ? '' : 's'} in your access.` : 'You can report on every client.',
    `<button class="btn" data-a="batch" title="One report for many clients, saved into a folder">${icon('layers', 14)} Batch reports…</button><button class="btn" data-a="open">${icon('folder', 14)} Open a report…</button><button class="btn" data-a="from">${icon('page', 14)} From a template file…</button>`)}
    <div class="us-grid">
      <div class="us-side">
        <section class="ar-card us-tpl">
          <div class="ar-card-t">${icon('layout', 15)} Report</div>
          <div class="us-reps"><div class="ar-note">Loading…</div></div>
        </section>
        <section class="ar-card us-saved" hidden>
          <div class="ar-card-t">${icon('folder', 15)} Saved <span class="muted us-saved-n"></span></div>
          <div class="us-saved-list"></div>
        </section>
        <section class="ar-card us-soon">
          <div class="ar-card-t">${icon('bell', 15)} Needs attention <span class="muted us-soon-n"></span></div>
          <div class="us-soon-list"><div class="ar-note">Loading…</div></div>
        </section>
      </div>
      <section class="ar-card us-clients">
        <div class="ar-card-t">${icon('users', 15)} Clients <span class="muted us-n"></span><span class="us-sum muted"></span><span class="grow"></span>
          <button class="btn sm us-cols" title="Columns">${icon('sliders', 13)} Columns</button>
          <select class="us-sort" title="Sort clients"><option value="name">By name</option><option value="value">By value</option><option value="ret">By return</option><option value="peers">By return vs peers</option><option value="fees">By fees</option><option value="last">By last report</option></select>
          <input class="us-find" placeholder="Find a client"></div>
        <div class="us-recent"></div>
        <div class="us-list"><div class="ar-note">Loading clients…</div></div>
      </section>
    </div>`;
  // the published reports this person may use
  call('auth', { cmd: 'reports' }, { quiet: true }).then((list) => {
    const live = list.filter((r) => r.published);
    const box = el.querySelector('.us-reps');
    box.innerHTML = live.map((r, i) => `<label class="us-rep"><input type="radio" name="rep" value="pub:${esc(r.id)}"${i ? '' : ' checked'}><span class="ic">${icon('page', 18)}</span><span><b>${esc(r.title)}</b><span>${esc(r.description || '')}</span></span></label>`).join('')
      || `<div class="ar-note">${icon('lock', 14)} No reports are published to you yet. A designer publishes them; an administrator can add you to a group.</div>`;
  }).catch((e) => { el.querySelector('.us-reps').innerHTML = `<div class="ar-note bad">${esc(String(e))}</div>`; });
  el.querySelector('[data-a="open"]').addEventListener('click', () => api.open());
  savedReports(el);
  el.querySelector('[data-a="batch"]').addEventListener('click', () => batchDialog());
  el.querySelector('[data-a="from"]').addEventListener('click', () => api.fromTemplate(false));
  const list = el.querySelector('.us-list');
  const make = (name) => { const k = el.querySelector('[name=rep]:checked')?.value; if (k) createReport(k, name); else toast('No report to make — none is published to you yet.', 'info'); };
  let clients = [];
  try { clients = await call('auth', { cmd: 'my_clients' }, { quiet: true }); } catch (e) { list.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
  // figures, what's coming up and the person's own reports come a moment later: the list shows first
  let ov = null;
  const sortSel = el.querySelector('.us-sort');
  try { sortSel.value = localStorage.getItem('quire.clientSort') || 'name'; } catch {}
  const fig = (c) => ov?.clients?.[c.id] || {};
  // which columns show (remembered)
  const COLS = [['risk', 'Risk profile'], ['trend', 'Value over 12 months'], ['flow', 'Deposits and withdrawals'], ['peers', 'Return vs peers'], ['fees', 'Fees this year'], ['last', 'Last report']];
  let cols = Object.fromEntries(COLS.map(([k]) => [k, true]));
  try { Object.assign(cols, JSON.parse(localStorage.getItem('quire.clientCols') || '{}')); } catch {}
  el.querySelector('.us-cols').addEventListener('click', (e) => menu(e.currentTarget, [{ title: 'Show in the list' }, ...COLS.map(([k, label]) => ({ label, checked: !!cols[k], run: () => {
    cols = { ...cols, [k]: !cols[k] };
    try { localStorage.setItem('quire.clientCols', JSON.stringify(cols)); } catch {}
    draw();
  } }))]));
  const last = (c) => ov?.reports?.[c.name]?.opened || '';
  // a long list is drawn a page of rows at a time; more as it scrolls into view
  const PAGE = 100;
  let limit = PAGE;
  let lastShown = [];
  let more = null;
  const loadMore = new IntersectionObserver((es) => { if (es.some((e) => e.isIntersecting)) { limit += PAGE; draw(); } }, { rootMargin: '400px' });
  const draw = () => {
    const q = el.querySelector('.us-find').value.trim().toLowerCase();
    const by = sortSel.value;
    const shown = clients.filter((c) => !q || `${c.name} ${c.id} ${c.banker}`.toLowerCase().includes(q));
    const num = (x) => (typeof x === 'number' ? x : -Infinity);
    if (by === 'value') shown.sort((a, b) => num(fig(b).value) - num(fig(a).value));
    else if (by === 'ret') shown.sort((a, b) => num(fig(b).ret) - num(fig(a).ret));
    else if (by === 'peers') shown.sort((a, b) => num(fig(b).vs_peers) - num(fig(a).vs_peers));
    else if (by === 'fees') shown.sort((a, b) => num(fig(b).fees) - num(fig(a).fees));
    else if (by === 'last') shown.sort((a, b) => last(b).localeCompare(last(a)) || a.name.localeCompare(b.name));
    el.querySelector('.us-n').textContent = `${shown.length}${shown.length !== clients.length ? ` of ${clients.length}` : ''}`;
    if (!clients.length) { list.innerHTML = `<div class="ar-note">${icon('lock', 15)} No clients are in your access yet. Ask an administrator.</div>`; return; }
    lastShown = shown;
    const rest = shown.length - Math.min(limit, shown.length);
    list.innerHTML = shown.slice(0, limit).map((c) => {
      const f = fig(c), r = ov?.reports?.[c.name];
      const prods = (f.products || []).map((p) => esc(srcName(p))).join(' · ');
      const ret = typeof f.ret === 'number' ? `<span class="us-ret ${f.ret < 0 ? 'neg' : 'pos'}">${esc(pct(f.ret))}</span>` : '';
      const flow = f.flow ? `<span class="us-flow" title="Deposits minus withdrawals this year">${f.flow > 0 ? icon('arrowDown', 11) : icon('arrowUp', 11)}<span class="no-i18n">${esc(money(Math.abs(f.flow)))}</span> <span data-i18n="flow">${f.flow > 0 ? 'in' : 'out'}</span></span>` : '';
      const items = attOf(c.id);
      const top = items[0];
      const props = ov?.proposals?.[c.name] || [];
      const pchip = props.length ? `<button class="us-pchip" title="This client's proposals — click to see them">${icon('briefcase', 11)}<span>${props.length > 1 ? `${props.length} proposals` : 'Proposal'}</span>${statusPill(props[0].status)}</button>` : '';
      const note = top ? `<button class="us-note" data-sev="${sev(top)}" title="What's coming up for this client">${icon(soonText(top).icon, 12)}<span>${esc(soonText(top).short)}</span>${isNow(top) ? '' : `<span class="no-i18n">${esc(day(top.date))}</span>`}${items.length > 1 ? `<span class="us-note-more">+${items.length - 1}</span>` : ''}${icon('chevd', 12)}</button>` : '';
      return `<div class="us-row${top ? ` has-note sev-${sev(top)}` : ''}${open.has(c.id) ? ' open' : ''}" data-name="${esc(c.name)}" data-id="${esc(c.id)}">
        <span class="av">${esc(initials(c.name))}</span>
        <span class="us-who"><span class="us-name"><b>${esc(c.name)}</b>${note}${pchip}</span><span>${esc(c.id)}${c.banker ? ` · ${esc(c.banker)}` : ''}</span>
          ${prods ? `<span class="us-prods" title="Products the client holds">${icon('briefcase', 11)}<span class="no-i18n">${prods}</span></span>` : ''}</span>
        ${cols.risk ? `<span class="us-riskc">${f.risk ? riskMeter(f.risk) : ''}</span>` : ''}
        ${cols.trend ? `<span class="us-spark">${ov ? spark(f.spark || []) : ''}</span>` : ''}
        <span class="us-fig">${f.value != null ? `<b class="no-i18n">${esc(money(f.value))}</b><span>${ret}${ret ? ' <span class="muted">this year</span>' : ''}</span>${cols.flow ? flow : ''}` : ''}</span>
        ${cols.peers ? `<span class="us-col" title="${f.peers ? `Return this year minus the median of ${f.peers} other clients with the same risk profile` : ''}">${typeof f.vs_peers === 'number' ? `<b class="no-i18n ${f.vs_peers < 0 ? 'neg' : 'pos'}">${esc(pp(f.vs_peers))}</b><span>vs peers</span>` : ''}</span>` : ''}
        ${cols.fees ? `<span class="us-col" title="Fees the client paid this year">${f.fees != null ? `<b class="no-i18n">${esc(money(f.fees))}</b><span>fees this year</span>` : ''}</span>` : ''}
        ${cols.last ? `<span class="us-last">${!ov ? '' : r?.opened ? `<span>Last report</span><b>${esc(ago(r.opened))}</b>${r.exported ? `<span class="muted">exported ${esc(ago(r.exported))}</span>` : ''}` : '<span class="muted">No report yet</span>'}</span>` : ''}
        <button class="btn sm us-make" title="Create report">${icon('page', 14)}</button>
      </div>${open.has(c.id) ? details(c, items, props) : ''}`;
    }).join('') + (rest > 0 ? `<div class="us-more-rows"><button class="btn sm">Show ${Math.min(PAGE, rest)} more · ${Math.min(limit, shown.length)} of ${shown.length}</button></div>` : '')
      || '<div class="ar-note">No client matches.</div>';
    if (more) loadMore.unobserve(more);
    more = list.querySelector('.us-more-rows');
    if (more) {
      more.querySelector('button').addEventListener('click', () => { limit += PAGE; draw(); });
      loadMore.observe(more);
    }
    list.querySelectorAll('[data-pst]').forEach((b) => b.addEventListener('click', (e) => {
      e.stopPropagation();
      const p = Object.values(ov?.proposals || {}).flat().find((x) => x.uid === b.dataset.pst);
      if (p) statusMenu(e.currentTarget, p, async () => { try { ov = await call('auth', { cmd: 'my_overview' }, { quiet: true }); attFor = undefined; } catch {} draw(); });
    }));
    list.querySelectorAll('.us-row').forEach((r) => {
      r.querySelector('.us-make').addEventListener('click', () => make(r.dataset.name));
      r.addEventListener('dblclick', (e) => { if (!e.target.closest('.us-note')) make(r.dataset.name); });
      r.querySelector('.us-note')?.addEventListener('click', (e) => { e.stopPropagation(); toggle(r.dataset.id); });
      r.querySelector('.us-pchip')?.addEventListener('click', (e) => { e.stopPropagation(); toggle(r.dataset.id); });
    });
  };
  // the details of what's coming up, opened under the client's row
  const open = new Set();
  let folded = [];
  // ⌘]: close the details opened under the rows (again: open them back)
  foldRight = () => {
    if (!el.isConnected) return;
    if (open.size) { folded = [...open]; open.clear(); } else folded.forEach((id) => open.add(id));
    draw();
  };
  const toggle = (id, on = !open.has(id), focus = null) => {
    if (on) open.add(id); else open.delete(id);
    // a client further down than drawn so far: draw down to it
    const at = lastShown.findIndex((c) => c.id === id);
    if (on && at >= limit) limit = Math.ceil((at + 1) / PAGE) * PAGE;
    draw();
    const row = list.querySelector(`.us-row[data-id="${CSS.escape(id)}"]`);
    if (!row || !on) return;
    const det = row.nextElementSibling?.classList.contains('us-det') ? row.nextElementSibling : row;
    det.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    const card = focus != null ? det.querySelector(`.us-dc[data-i="${focus}"]`) : null;
    if (card) { card.classList.remove('flash'); void card.offsetWidth; card.classList.add('flash'); }
  };
  // each client's items, looked up by client (not searched for every row)
  let attMap = new Map(), attFor;
  const attOf = (id) => {
    if (attFor !== ov) {
      attFor = ov;
      attMap = new Map();
      for (const a of ov?.attention || []) (attMap.get(a.id) || attMap.set(a.id, []).get(a.id)).push(a);
      for (const l of attMap.values()) l.sort((a, b) => sevRank(a) - sevRank(b) || a.date.localeCompare(b.date));
    }
    return attMap.get(id) || [];
  };
  // search once typing pauses, from the top of the list
  let typing = null;
  el.querySelector('.us-find').addEventListener('input', () => { clearTimeout(typing); typing = setTimeout(() => { limit = PAGE; draw(); }, 180); });
  sortSel.addEventListener('change', () => { try { localStorage.setItem('quire.clientSort', sortSel.value); } catch {} limit = PAGE; draw(); });
  draw();
  try { ov = await call('auth', { cmd: 'my_overview' }, { quiet: true }); } catch (e) {
    el.querySelector('.us-soon-list').innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`;
    return;
  }
  learnProducts(ov.products);
  const total = Object.values(ov.clients || {}).reduce((t, c) => t + (c.value || 0), 0);
  el.querySelector('.us-sum').innerHTML = total ? `· <span class="no-i18n">${esc(money(total))}</span> · as of <span class="no-i18n">${esc(day(ov.as_of))}</span>` : '';
  draw();
  // the clients this person worked on last, one click away
  const recent = clients.filter((c) => last(c)).sort((a, b) => last(b).localeCompare(last(a))).slice(0, 4);
  const rec = el.querySelector('.us-recent');
  rec.innerHTML = recent.length ? `<span class="muted">Recent</span>${recent.map((c) => `<button class="us-rc" data-name="${esc(c.name)}" title="Create a report for “${esc(c.name)}”"><span class="av">${esc(initials(c.name))}</span><span><b>${esc(c.name)}</b><span>${esc(ago(last(c)))}</span></span></button>`).join('')}` : '';
  rec.querySelectorAll('.us-rc').forEach((b) => b.addEventListener('click', () => make(b.dataset.name)));
  // what needs attention soon
  const att = (ov.attention || []).slice().sort((a, b) => Math.min(sevRank(a), 4) - Math.min(sevRank(b), 4) || a.date.localeCompare(b.date));
  const soon = el.querySelector('.us-soon-list');
  el.querySelector('.us-soon-n').textContent = att.length || '';
  const item = (a, i) => {
    const t = soonText(a);
    return `<button class="us-si" data-k="${a.kind}" data-sev="${sev(a)}" data-i="${i}"><span class="us-si-ic">${icon(t.icon, 14)}</span>
      <span class="us-si-tx"><b>${esc(t.title)}</b><span>${esc(a.client)}${t.what ? ` · <span class="no-i18n">${esc(t.what)}</span>` : ''}</span></span>
      <span class="us-si-r">${t.amount ? `<b class="no-i18n">${esc(t.amount)}</b>` : ''}${isNow(a) ? '<span>now</span>' : `<span class="no-i18n">${esc(day(a.date))}</span>`}</span></button>`;
  };
  const MAX = 7;
  const fill = (all) => {
    soon.innerHTML = att.length
      ? att.slice(0, all ? att.length : MAX).map((a, i) => item(a, i)).join('') + (!all && att.length > MAX ? `<button class="btn sm us-more">Show all ${att.length}</button>` : '')
      : `<div class="ar-note">${icon('check', 14)} Nothing needs attention: no payments due in 30 days, no flags.</div>`;
    soon.querySelector('.us-more')?.addEventListener('click', () => fill(true));
    // a click opens the client's details under their row
    soon.querySelectorAll('.us-si').forEach((b) => b.addEventListener('click', () => {
      const a = att[+b.dataset.i];
      if (el.querySelector('.us-find').value) { el.querySelector('.us-find').value = ''; }
      open.clear();
      toggle(a.id, true, attOf(a.id).findIndex((x) => x.date === a.date && x.kind === a.kind && x.what === a.what));
    }));
  };
  fill(false);
}

// ---- the client list's figures, in the interface language
const ruUI = () => appLanguage() === 'ru';
function money(v) {
  const a = Math.abs(v);
  const [n, u] = a >= 1e9 ? [a / 1e9, ruUI() ? ' млрд' : ' bn'] : a >= 1e6 ? [a / 1e6, ruUI() ? ' млн' : ' M'] : a >= 1e3 ? [a / 1e3, ruUI() ? ' тыс' : ' K'] : [a, ''];
  const s = n.toLocaleString(ruUI() ? 'ru-RU' : 'en-US', { maximumFractionDigits: n >= 100 || !u ? 0 : 1 });
  return `${v < 0 ? '−' : ''}${s}${u} ₽`;
}
const pp = (v) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v * 100).toLocaleString(ruUI() ? 'ru-RU' : 'en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 })} pp`;
// the client list's ⌘] (set while the list shows)
let foldRight = null;
document.addEventListener('quire:fold-right', () => foldRight?.());
// after a batch, the Reports page shows its new reports
document.addEventListener('quire:batch-done', () => { const b = document.querySelector('.ar-body[data-area="user"]'); if (b?.isConnected) userArea(b); });
const pct = (v) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v * 100).toLocaleString(ruUI() ? 'ru-RU' : 'en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 })}%`;
const day = (iso) => {
  if (!iso) return '';
  const d = new Date(`${String(iso).slice(0, 10)}T12:00:00`);
  return d.toLocaleDateString(ruUI() ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'short', ...(d.getFullYear() !== new Date().getFullYear() ? { year: 'numeric' } : {}) }).replace('.', '');
};
/**
 * The person's saved reports (the browser version keeps them on the server, in their own folder):
 * newest first, each to open again or delete. Not shown when there are none (the desktop app's
 * reports are files wherever they were saved).
 */
/** When a report was saved: today and yesterday with the time (two saves the same day differ), else as `ago`. */
function savedAt(at) {
  const when = ago(at);
  const hm = new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  return when === 'today' || when === 'yesterday' ? `${when}, ${hm}` : when;
}

async function savedReports(el) {
  const card = el.querySelector('.us-saved');
  let list = [];
  try { list = (await call('saved_documents', {}, { quiet: true })).filter((d) => d.kind === 'report'); } catch { return; }
  card.hidden = !list.length;
  el.querySelector('.us-saved-n').textContent = list.length > 6 ? list.length : '';
  const box = el.querySelector('.us-saved-list');
  const draw = (all) => {
    box.innerHTML = list.slice(0, all ? list.length : 6).map((d, i) => `<div class="us-sv" data-i="${i}"><button class="us-sv-open" title="Open “${esc(d.file)}”">${icon('page', 15)}<span><b>${esc(d.client || d.title)}</b><span>${esc(savedAt(d.saved))}${d.client ? ` · ${esc(d.title)}` : ''}</span></span></button><button class="tb-btn us-sv-del" title="Delete…">${icon('trash', 13)}</button></div>`).join('')
      + (!all && list.length > 6 ? `<button class="btn sm us-sv-more">Show all ${list.length}</button>` : '');
    box.querySelector('.us-sv-more')?.addEventListener('click', () => draw(true));
    box.querySelectorAll('.us-sv').forEach((row) => {
      const d = list[+row.dataset.i];
      row.querySelector('.us-sv-open').addEventListener('click', async () => {
        try { setState(await call('open_path', { path: d.path })); } catch { /* toast shown */ }
      });
      row.querySelector('.us-sv-del').addEventListener('click', async () => {
        const ok = await modal({ title: 'Delete the saved report?', iconName: 'warn', iconKind: 'warn', body: `<p>“${esc(d.file)}” is deleted from your saved reports. This can’t be undone.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
        if (!ok) return;
        try { await call('delete_saved', { path: d.path }); toast('Deleted'); savedReports(el); } catch { /* toast shown */ }
      });
    });
  };
  draw(false);
}

function ago(at) {
  const d = new Date(at);
  const days = Math.floor((new Date().setHours(0, 0, 0, 0) - new Date(d).setHours(0, 0, 0, 0)) / 864e5);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 7) return `${days} days ago`;
  return day(at);
}
const RISK = { 'Консервативный': [1, 'Conservative'], 'Умеренный': [2, 'Moderate'], 'Умеренно-агрессивный': [3, 'Moderately aggressive'], 'Агрессивный': [4, 'Aggressive'] };
const riskName = (r) => (ruUI() ? r : RISK[r]?.[1] || r);
/** The risk profile as a level (four bars) with its name: a scale, not a tag like the products. */
function riskMeter(r) {
  const n = RISK[r]?.[0] || 0;
  return `<span class="us-meter" data-n="${n}" title="Risk profile">${[1, 2, 3, 4].map((i) => `<i class="${i <= n ? 'on' : ''}"></i>`).join('')}</span><span class="us-risk-l"><span>Risk</span><b>${esc(riskName(r))}</b></span>`;
}
/** A year of month-end values as a small line (neutral: value moves with flows too, so it isn't the return). */
function spark(v) {
  if (v.length < 2) return '';
  const w = 72, hgt = 26, lo = Math.min(...v), hi = Math.max(...v), span = hi - lo || 1;
  const pts = v.map((x, i) => `${(i / (v.length - 1) * (w - 4) + 2).toFixed(1)},${(hgt - 3 - (x - lo) / span * (hgt - 6)).toFixed(1)}`);
  const [lx, ly] = pts[pts.length - 1].split(',');
  return `<svg width="${w}" height="${hgt}" viewBox="0 0 ${w} ${hgt}" aria-label="Value over 12 months"><polyline points="${pts.join(' ')}" fill="none" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round"/><circle cx="${lx}" cy="${ly}" r="2.2"/></svg>`;
}
// how much an item asks of the banker: a large withdrawal first, then holdings outside the risk profile,
// money coming free, idle cash, a policy to renew, a birthday, then routine income
const RANK = { withdrawal: 0, risk: 1, maturity: 2, proposal: 3, cash: 4, policy: 5, birthday: 6, income: 7 };
const kindOf = (a) => (a.kind === 'payment' ? (a.type === 'Погашение' ? 'maturity' : 'income') : a.kind);
const sevRank = (a) => RANK[kindOf(a)] ?? 6;
const sev = (a) => ({ withdrawal: 'high', risk: 'warn', maturity: 'act', proposal: 'act', cash: 'act', policy: 'act', birthday: 'info', income: 'low' })[kindOf(a)] || 'low';
/** Flags about the portfolio now (no date ahead). */
const isNow = (a) => a.kind === 'cash' || a.kind === 'risk' || a.kind === 'proposal';
const isDeposit = (a) => a.instr_type === 'Deposit' || /вклад|депозит/i.test(a.what || '');
function soonText(a) {
  if (a.kind === 'proposal') return { icon: 'briefcase', title: 'Follow up on the proposal', short: 'Follow up proposal', what: a.what };
  if (a.kind === 'birthday') return { icon: 'gift', title: `Birthday · turns ${a.age}`, short: 'Birthday' };
  if (a.kind === 'policy') return { icon: 'shield', title: 'Policy ends', short: 'Policy ends', what: a.what || a.type };
  if (a.kind === 'withdrawal') return { icon: 'arrowUp', title: `Large withdrawal · ${Math.round(a.share * 100)}%`, short: 'Large withdrawal', amount: money(a.amount) };
  if (a.kind === 'cash') return { icon: 'wallet', title: `Idle cash · ${Math.round(a.share * 100)}%`, short: 'Idle cash', amount: money(a.amount) };
  if (a.kind === 'risk') return { icon: 'warn', title: `Above the risk profile · ${Math.round(a.share * 100)}% in shares`, short: 'Above the risk profile', amount: money(a.amount) };
  const t = { 'Купон': 'Coupon', 'Дивиденд': 'Dividend', 'Погашение': isDeposit(a) ? 'Deposit matures' : 'Bond matures' }[a.type] || a.type;
  return { icon: a.type === 'Погашение' ? 'flag' : 'coins', title: t, short: t, what: a.what, amount: money(a.amount) };
}
/** Why an item matters, and what to do about it. */
function soonWhy(a) {
  if (a.kind === 'proposal') return `Presented ${a.days} days ago with no answer yet: ask how the client sees it, then record the outcome.`;
  if (a.kind === 'withdrawal') return 'A large withdrawal can mean the client is moving money elsewhere. Worth a call before the next report.';
  if (a.kind === 'cash') return 'Cash earns little: offer a deposit, bonds or a fund that fits the profile.';
  if (a.kind === 'risk') return `Shares are ${Math.round(a.share * 100)}% of the portfolio; the profile allows up to ${Math.round(a.limit * 100)}%. Review the holdings, or the profile, with the client.`;
  if (a.kind === 'policy') return 'The policy ends soon: talk about renewing it or where the money goes next.';
  if (a.kind === 'birthday') return 'A good moment to get in touch.';
  if (a.type === 'Погашение') return isDeposit(a) ? 'The deposit comes back to the client: talk about reinvesting before the money sits idle or leaves.' : 'The bond is repaid: talk about reinvesting the money.';
  return 'Income the client receives: it can be reinvested.';
}
const until = (iso) => {
  const n = Math.round((new Date(`${iso}T12:00:00`) - new Date(new Date().toDateString() + ' 12:00')) / 864e5);
  return n === 0 ? 'today' : n === 1 ? 'tomorrow' : n > 0 ? `in ${n} days` : n === -1 ? 'yesterday' : `${-n} days ago`;
};
const qtyFmt = (v) => v.toLocaleString(ruUI() ? 'ru-RU' : 'en-US', { maximumFractionDigits: 2 });
/** The details of a client's items: what, when, how much, from which account, and why it matters. */
function details(c, items, props = []) {
  const fact = (k, v) => (v || v === 0 ? `<span class="us-df"><span>${k}</span><b class="no-i18n">${esc(String(v))}</b></span>` : '');
  const card = (a, i) => {
    const t = soonText(a);
    let facts = '';
    if (a.kind === 'payment') {
      facts = fact('Instrument', a.what) + fact((a.accounts || []).length > 1 ? 'Accounts' : 'Account', (a.accounts || []).join(', ')) + fact('Quantity', qtyFmt(a.qty))
        + fact('Per unit', `${qtyFmt(a.per_unit)} ${a.cur}`) + (a.coupon != null && a.type !== 'Дивиденд' ? fact('Rate', `${qtyFmt(a.coupon)}%`) : '')
        + (a.isin ? fact('ISIN', a.isin) : '') + (a.maturity && a.type !== 'Погашение' ? fact('Matures', day(a.maturity)) : '');
    } else if (a.kind === 'policy') {
      facts = fact('Policy', a.what) + fact('Account', a.acct_no) + (a.opened ? fact('Since', day(a.opened)) : '');
    } else if (a.kind === 'withdrawal') {
      facts = fact('Share of the portfolio', `${Math.round(a.share * 100)}%`) + fact('Portfolio now', money(a.value));
      facts += `<div class="us-ops">${(a.ops || []).map((o) => `<span class="no-i18n">${esc(day(o.date))}</span><span class="no-i18n">${esc(o.account)} ${esc(o.acct_no)}</span><b class="no-i18n">−${esc(money(o.amount))}</b>`).join('')}</div>`;
    } else if (a.kind === 'birthday') {
      facts = fact('Turns', a.age);
    } else if (a.kind === 'cash') {
      facts = fact('Share of the portfolio', `${Math.round(a.share * 100)}%`) + fact('Portfolio now', money(a.value));
    } else if (a.kind === 'proposal') {
      facts = fact('Proposal', a.what) + fact('Presented', day(a.since)) + `<button class="btn sm us-dfb" data-pst="${esc(a.uid)}">Record the outcome…</button>`;
    } else if (a.kind === 'risk') {
      facts = fact('Risk profile', riskName(a.profile)) + fact('Limit for shares', `${Math.round(a.limit * 100)}%`) + fact('In shares now', `${Math.round(a.share * 100)}%`) + fact('Portfolio now', money(a.value));
    }
    return `<div class="us-dc" data-sev="${sev(a)}" data-i="${i}">
      <div class="us-dc-h"><span class="us-si-ic">${icon(t.icon, 15)}</span><span class="us-dc-t"><b>${esc(t.title)}</b><span>${isNow(a) ? `<span>as of</span> <span class="no-i18n">${esc(day(a.date))}</span>` : `<span class="no-i18n">${esc(day(a.date))}</span> · <span>${esc(until(a.date))}</span>`}</span></span>${t.amount ? `<b class="us-dc-amt no-i18n">${esc(t.amount)}</b>` : ''}</div>
      <div class="us-dfs">${facts}</div>
      <p class="us-why">${icon('info', 13)}<span>${esc(soonWhy(a))}</span></p>
    </div>`;
  };
  // things to act on as cards; coupons and dividends (routine income) as a table
  const income = (a) => a.kind === 'payment' && a.type !== 'Погашение';
  const cards = items.map((a, i) => (income(a) ? '' : card(a, i))).join('');
  const rows = items.map((a, i) => (income(a) ? `<tr class="us-dc" data-i="${i}" data-sev="low"><td class="no-i18n">${esc(day(a.date))}</td><td>${esc(soonText(a).title)}</td>
      <td class="no-i18n" title="${esc(a.isin || '')}">${esc(a.what)}</td><td class="no-i18n">${esc((a.accounts || []).join(', '))}</td>
      <td class="no-i18n num">${esc(qtyFmt(a.qty))} × ${esc(qtyFmt(a.per_unit))} ${esc(a.cur)}</td><td class="no-i18n num"><b>${esc(money(a.amount))}</b></td></tr>` : '')).join('');
  const total = items.filter(income).reduce((t, a) => t + a.amount, 0);
  // the client's proposals (the register): what was proposed, its status and the last word on it
  const propHtml = props.length ? `<div class="us-props"><div class="us-inc-h">${icon('briefcase', 13)} <span>Proposals</span><span class="muted">${props.length}</span></div>
    ${props.map((p) => `<div class="us-prop">
      <div class="us-prop-h">${statusPill(p.status)}<b>${esc(p.title || '')}</b><span class="muted">${esc(day(String(p.created || '').slice(0, 10)))}${p.period ? ` · ${esc(p.period)}` : ''}${p.in_report === false ? ' · <span>not in a report</span>' : ''}</span><span class="grow"></span>
        <button class="btn sm" data-pst="${esc(p.uid)}">Status…</button></div>
      ${proposalBodyHtml({ lead: p.lead, actions: p.actions }, { title: false })}
      ${(p.history || []).length > 1 ? `<div class="us-prop-hist">${p.history.slice(1).map((x) => `<span>${esc(day(String(x.at).slice(0, 10)))} · ${esc(statusName(x.status))}${x.note ? ` — “${esc(x.note)}”` : ''}</span>`).join('')}</div>` : ''}
    </div>`).join('')}</div>` : '';
  return `<div class="us-det" data-i18n="det"><div class="us-det-h"><b>${items.length ? 'Needs attention' : 'Proposals'}: “${esc(c.name)}”</b><span class="grow"></span><span class="muted us-det-k"><kbd>⌘]</kbd> closes</span></div>
    ${cards ? `<div class="us-dcs">${cards}</div>` : ''}
    ${rows ? `<div class="us-inc"><div class="us-inc-h">${icon('coins', 13)} <span>Income in the next 30 days</span><b class="no-i18n">${esc(money(total))}</b></div>
      <table class="us-inc-t"><thead><tr><th>Date</th><th>Payment</th><th>Instrument</th><th>Account</th><th class="num">Quantity × per unit</th><th class="num">Amount</th></tr></thead><tbody>${rows}</tbody></table></div>` : ''}
    ${propHtml}
  </div>`;
}

async function createReport(kind, client) {
  S.page = 0; S.sel = null; S.preview = false; S.zoomFit = true;
  // made for the chosen client from the start (not for the template's default client first)
  await api.newDoc(kind, client);
  const d = S.st?.doc;
  if (!d) return;
  toast(`Report for ${client}. Save it to keep it.`, 'info', 2600);
}

// ---------------------------------------------------------------- Data

/**
 * Everything about data, in one place: the shared connections and data
 * sources (made and changed here, by the Data role; designers use them in
 * templates), and — with a template open — the sources and parameters that
 * are its own. A list on the left, the chosen item on the right.
 */
async function dataArea(el) {
  const doc = S.st?.doc;
  const tpl = doc && doc.kind === 'template' && hasRole('design') ? doc : null;
  const manage = hasRole('data');
  const docName = tpl ? (S.st.file || tpl.meta.title || 'Untitled') : '';
  el.innerHTML = `${areaHead('data', `Shared connections and data sources${tpl ? `, and the data of <b>${esc(docName)}</b>` : ''}.${manage ? '' : ' Shared ones are made and changed by the Data role.'}`,
    `${manage || tpl ? `<button class="btn" data-a="new-conn" title="${manage ? 'A database every template can read from' : 'A database only this template reads from'}">${icon('database', 14)} Connect a database</button>` : ''}<button class="btn primary" data-a="new">${icon('plus', 14)} New data source</button>`)}
    <div class="dx-ctx"></div>
    <div class="dx-split">
      <aside class="ar-card dx-list"><div class="ar-note">Loading…</div></aside>
      <section class="ar-card dx-detail"></section>
    </div>`;
  const listEl = el.querySelector('.dx-list');
  const detail = el.querySelector('.dx-detail');
  let ready = null;
  el.querySelector('[data-a="new"]').addEventListener('click', () => {
    S.dataSel = { kind: 'new' };
    if (ready) ready.pick({ kind: 'new' });
    else detail.innerHTML = `<div class="ar-note">${icon('plus', 16)} New data source — loading the lists…</div>`;
  });
  el.querySelector('[data-a="new-conn"]')?.addEventListener('click', () => (ready ? ready.addConn() : (S.dataAfterLoad = 'conn')));
  // which client and period previews and SQL use: the open document's, or the Data area's own
  const ctx = el.querySelector('.dx-ctx');
  if (doc) {
    const client = doc.params?.values?.client;
    ctx.innerHTML = `${icon('info', 14)}<span>Previews and SQL use the open ${doc.kind === 'report' ? 'report' : 'template'}'s values: ${client ? `<b>${esc(client)}</b> · ` : ''}<b>${esc(S.st.period?.label || '')}</b>.</span>`;
  } else {
    ctx.innerHTML = `${icon('eye', 14)}<span>Preview with</span><select class="dx-client"><option>…</option></select><span>from</span><input type="date" class="dx-d0"><span>to</span><input type="date" class="dx-d1"><span class="muted">— the values of <code>:client</code>, <code>:period_start</code>… while no report is open</span>`;
    const setP = async (patch) => {
      try { const r = await call('auth', { cmd: 'data_params', args: patch }, { quiet: true }); fillP(r); drawDetail(); } catch (e) { toast(String(e), 'error', 3000); }
    };
    const fillP = (r) => { ctx.querySelector('.dx-d0').value = r.start; ctx.querySelector('.dx-d1').value = r.end; ctx.querySelector('.dx-client').value = r.client; };
    Promise.all([call('auth', { cmd: 'my_clients' }, { quiet: true }).catch(() => []), call('auth', { cmd: 'data_params', args: {} }, { quiet: true })]).then(([cl, r]) => {
      ctx.querySelector('.dx-client').innerHTML = cl.map((c) => `<option>${esc(c.name)}</option>`).join('');
      fillP(r);
    }).catch(() => {});
    ctx.querySelector('.dx-client').addEventListener('change', (e) => setP({ client: e.target.value }));
    const dates = () => setP({ start: ctx.querySelector('.dx-d0').value, end: ctx.querySelector('.dx-d1').value });
    ctx.querySelector('.dx-d0').addEventListener('change', dates);
    ctx.querySelector('.dx-d1').addEventListener('change', dates);
  }
  let shared = [], lib = { datasets: [], usage: {} };
  // every published report's own data, a section each; the open template's own above them
  const reportsList = await call('auth', { cmd: 'reports' }, { quiet: true }).catch(() => []);
  const reps = reportsList.filter((r) => !tpl || r.id !== tpl.meta.report_id);
  const editRep = hasRole('data') || hasRole('design');
  const repData = {};
  const loadRep = async (id, peek = true) => { repData[id] = await call('auth', { cmd: 'report_data', args: { id, peek } }, { quiet: true }).catch(() => null); return repData[id]; };
  // folded groups, kept on this computer
  const FOLD_KEY = 'quire.dxfold';
  let fold = {};
  try { fold = JSON.parse(localStorage.getItem(FOLD_KEY) || '{}'); } catch { fold = {}; }
  const selRep = S.dataSel?.kind === 'rds' ? S.dataSel.id.split('::')[0] : S.dataSel?.kind === 'rparams' ? S.dataSel.id : null;
  const folded = (key) => (key in fold ? fold[key] : key.startsWith('r:') ? !(key === `r:${selRep}` || (!tpl && reps[0] && key === `r:${reps[0].id}`)) : false);
  if (selRep && fold[`r:${selRep}`]) delete fold[`r:${selRep}`];
  await Promise.all(reps.map((r) => (folded(`r:${r.id}`) ? null : loadRep(r.id))));
  const src = tpl; // the open template's own data
  const ownName = docName;
  try {
    [shared, lib] = await Promise.all([call('auth', { cmd: 'connections' }, { quiet: true }), call('auth', { cmd: 'library' }, { quiet: true })]);
  } catch (e) { listEl.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
  S.sharedConns = shared;
  const saveConns = async (next) => { shared = await call('auth', { cmd: 'save_connections', args: { connections: next } }); render(); };
  /** Saves a shared source in the Data area; the open template's copy follows. */
  const saveShared = async (ds) => {
    const r = await call('auth', { cmd: 'save_source', args: { dataset: ds } });
    S.dataSel = { kind: 'lib', id: r.id };
    setState(r.state);
    return r;
  };
  const connOf = (ds) => shared.find((c) => c.id === ds.source?.connection) || (src?.connections || []).find((c) => c.id === ds.source?.connection) || Object.values(repData).flatMap((d) => d?.connections || []).find((c) => c.id === ds.source?.connection);
  /** Saves a source of a published report (the Data area's change to it). */
  const saveRep = (rid) => async (ds) => {
    const r = await call('auth', { cmd: 'save_report_source', args: { id: rid, dataset: ds } });
    S.dataSel = { kind: 'rds', id: `${rid}::${r.id}` };
    render();
    return r;
  };
  const conns = [...shared.map((c) => ({ ...c, shared: true })), ...(tpl?.connections || []).filter((c) => !shared.some((x) => x.id === c.id)).map((c) => ({ ...c, shared: false }))];
  const inTpl = (c) => (tpl?.connections || []).some((x) => x.id === c.id);
  const linked = (id) => (tpl?.datasets || []).some((d) => d.id === id && d.shared);
  const own = (src?.datasets || []).filter((d) => !d.shared);
  const users = (dsId) => (tpl?.pages || []).flatMap((p, pi) => p.widgets.filter((w) => w.binding?.dataset === dsId).map((w) => ({ w, pi })));
  const usageOf = (id) => (lib.usage[id] || []).filter((u) => u.path !== S.st?.path);

  let sel = S.dataSel;
  const exists = (x) => x && (x.kind === 'rds' || x.kind === 'rparams' ? reps.some((r) => r.id === (x.kind === 'rds' ? x.id.split('::')[0] : x.id)) : x.kind === 'new' ? true : x.kind === 'params' ? !!src : x.kind === 'ds' ? own.some((d) => d.id === x.id) : x.kind === 'lib' ? lib.datasets.some((d) => d.id === x.id) : conns.some((c) => c.id === x.id));
  if (!exists(sel)) sel = lib.datasets[0] ? { kind: 'lib', id: lib.datasets[0].id } : own[0] ? { kind: 'ds', id: own[0].id } : conns[0] ? { kind: 'conn', id: conns[0].id } : null;
  const pick = (x) => { sel = x; S.dataSel = x; drawList(); drawDetail(); };
  // the top bar's AI here (⌘I): the selected data source's query with its assistant, or a new
  // query on the connection in view (or the first) when no source is selected
  setAreaAI('data', async () => {
    // the selected source: shared, the template's own, or a published report's (loaded when needed)
    let ds = null, saveSource = null;
    if (sel?.kind === 'lib') { ds = lib.datasets.find((d) => d.id === sel.id); saveSource = manage ? saveShared : null; }
    else if (sel?.kind === 'ds') ds = own.find((d) => d.id === sel.id);
    else if (sel?.kind === 'rds') {
      const [rid, dsid] = sel.id.split('::');
      const d = repData[rid] || (await loadRep(rid, false));
      ds = d?.datasets.find((x) => x.id === dsid) || null;
      saveSource = editRep ? saveRep(rid) : null;
    }
    const dsConn = ds?.source?.type === 'db' ? connOf(ds) : null;
    if (ds && dsConn) {
      await workspace(dsConn, { dataset: ds, saveSource });
      return true;
    }
    const c = (sel?.kind === 'conn' && conns.find((x) => x.id === sel.id)) || conns[0];
    if (!c) return false;
    await workspace(c, { global: true, saveSource: c.shared && manage ? saveShared : null, fresh: true });
    return true;
  });
  const on = (x) => sel && sel.kind === x.kind && sel.id === x.id;

  // `reveal`: scroll to the selected row (on arrival); otherwise the list keeps its scroll
  const drawList = (reveal = false) => {
    const keep = listEl.querySelector('.dx-scroll')?.scrollTop ?? 0;
    const q = (S.dataFind || '').trim().toLowerCase();
    const hit = (...t) => !q || t.join(' ').toLowerCase().includes(q);
    const row = (x, ic, title, sub, extra = '') => `<button class="dx-row${on(x) ? ' on' : ''}" data-k="${x.kind}" data-id="${esc(x.id || '')}"><span class="dx-ic">${icon(ic, 15)}</span><span class="dx-who"><b>${esc(title)}</b><span>${esc(sub)}</span></span>${extra}</button>`;
    const cs = conns.filter((c) => hit(c.name, connWhere(c)));
    const ls = lib.datasets.filter((d) => hit(d.name, d.id));
    const os = own.filter((d) => hit(d.name, d.id, sourceLabel(d.source)));
    // three kinds, each with its own colour: connections, shared sources, the template's own
    const group = (k, ic, title, sub, add, body, key = k) => {
      const f = folded(key) && !q;
      return `<section class="dx-group k-${k}${f ? ' folded' : ''}">
        <header data-fold="${esc(key)}" title="${f ? 'Show' : 'Hide'} the list"><span class="dx-chev">${icon('chevd', 13)}</span><span class="dx-gic">${icon(ic, 14)}</span><span class="dx-gt"><b>${title}</b><span>${sub}</span></span>${add}</header>
        ${f ? '' : `<div class="dx-rows">${body}</div>`}</section>`;
    };
    const addBtn = (a, tip, id = '') => `<button class="tb-btn sm dx-add" data-a="${a}"${id ? ` data-r="${esc(id)}"` : ''} title="${tip}">${icon('plus', 14)}</button>`;
    listEl.innerHTML = `
      <input class="dx-find" placeholder="Find a connection or data source" value="${esc(S.dataFind || '')}">
      <div class="dx-scroll">
      ${group('conn', 'database', 'Connections', `${conns.length} · databases to read from`, manage ? addBtn('add-conn', 'Connect a database') : '',
        cs.map((c) => row({ kind: 'conn', id: c.id }, 'database', c.name, `${({ duckdb: 'DuckDB', sqlite: 'SQLite', postgres: 'PostgreSQL', clickhouse: 'ClickHouse' })[c.kind] || c.kind} · ${connWhere(c)}`, !c.shared ? '<i class="dx-tag own">template only</i>' : tpl && inTpl(c) ? '<i class="dx-tag">in template</i>' : '')).join('') || '<div class="ar-note">No connections.</div>')}
      ${group('shared', 'link', 'Shared data sources', `${lib.datasets.length} · any template can use them`, manage ? addBtn('add-lib', 'New shared data source') : '',
        ls.map((d) => { const n = usageOf(d.id).length + (linked(d.id) ? 1 : 0); return row({ kind: 'lib', id: d.id }, 'link', d.name, `${connOf(d)?.name ? 'SQL on ' + connOf(d).name : sourceLabel(d.source, true)}`, `${srcTag(d.source)}${tpl && linked(d.id) ? '<i class="dx-tag">in template</i>' : ''}${n ? `<i class="dx-n" title="Templates using it">${n}</i>` : ''}`); }).join('') || '<div class="ar-note">No shared data sources yet.</div>')}
      ${src ? group('own', 'layout', `Only in “${esc(ownName)}”`, `${own.length} · this template's own`, addBtn('add-own', 'New data source for this template only'),
        (hit('report parameters', ...src.param_defs.map((p) => p.label || p.name)) ? row({ kind: 'params', id: '' }, 'sliders', 'Report parameters', src.param_defs.map((p) => p.label || p.name).join(', ') || 'none — the period is always there') : '')
        + os.map((d) => row({ kind: 'ds', id: d.id }, 'table', d.name || d.id, sourceLabel(d.source), `${srcTag(d.source)}${users(d.id).length ? `<i class="dx-n" title="Widgets using it">${users(d.id).length}</i>` : ''}`)).join(''))
      : ''}
      ${reps.map((r) => {
        const d = repData[r.id];
        const mine = (d?.datasets || []).filter((x) => !x.shared);
        const rows = !d ? '<div class="ar-note">Loading…</div>'
          : (hit('report parameters', ...d.param_defs.map((p) => p.label || p.name)) ? row({ kind: 'rparams', id: r.id }, 'sliders', 'Report parameters', d.param_defs.map((p) => p.label || p.name).join(', ') || 'none') : '')
            + mine.filter((x) => hit(x.name, x.id, sourceLabel(x.source, true))).map((x) => row({ kind: 'rds', id: `${r.id}::${x.id}` }, 'table', x.name || x.id, `${connOf(x)?.name ? 'SQL on ' + connOf(x).name : sourceLabel(x.source, true)}`, `${srcTag(x.source)}${d.changed.includes(x.id) ? '<i class="dx-tag own" title="Added or changed in the Data area — goes out with the next publish">changed</i>' : ''}`)).join('');
        const sub = d ? `${mine.length} · the report's own${d.changed.length ? ` · ${d.changed.length} changed here` : ''}${editRep ? '' : ' · read-only'}` : `${r.published ? 'published' : 'unpublished'} report`;
        return group('own', 'page', `Report “${esc(r.title)}”`, sub, editRep ? addBtn('add-rep', 'New data source for this report', r.id) : '', rows, `r:${r.id}`);
      }).join('')}
      ${!src && !reps.length ? `<div class="dx-own-note">${icon('layout', 13)}<span>A template or report can also have data sources of its own (not shared). They show here once a template is open or a report is published.</span></div>` : ''}
      </div>`;
    const find = listEl.querySelector('.dx-find');
    find.addEventListener('input', () => { S.dataFind = find.value; drawList(); const f = listEl.querySelector('.dx-find'); f.focus(); f.setSelectionRange(f.value.length, f.value.length); });
    listEl.querySelectorAll('.dx-row').forEach((b) => b.addEventListener('click', () => pick({ kind: b.dataset.k, id: b.dataset.id })));
    listEl.querySelector('[data-a="add-conn"]')?.addEventListener('click', (e) => { e.stopPropagation(); addConn(); });
    // fold / unfold a group (not when the + is clicked)
    listEl.querySelectorAll('[data-fold]').forEach((hd) => hd.addEventListener('click', async (e) => {
      if (e.target.closest('.dx-add')) return;
      const key = hd.dataset.fold;
      fold[key] = !folded(key);
      try { localStorage.setItem(FOLD_KEY, JSON.stringify(fold)); } catch { /* private window */ }
      if (!fold[key] && key.startsWith('r:') && !repData[key.slice(2)]) { drawList(); await loadRep(key.slice(2)); }
      drawList();
    }));
    listEl.querySelectorAll('[data-a="add-rep"]').forEach((b) => b.addEventListener('click', (e) => {
      e.stopPropagation();
      const rid = b.dataset.r;
      const conns2 = [...shared, ...(repData[rid]?.connections || []).filter((c) => !shared.some((x) => x.id === c.id))];
      menu(b, [{ title: `New data source for “${reps.find((r) => r.id === rid)?.title || rid}”` }, ...sharedSourceKinds(conns2, saveRep(rid))]);
    }));
    listEl.querySelector('[data-a="add-lib"]')?.addEventListener('click', (e) => e.stopPropagation() || menu(e.currentTarget, sharedSourceKinds(shared, saveShared)));
    listEl.querySelector('[data-a="add-own"]')?.addEventListener('click', (e) => e.stopPropagation() || menu(e.currentTarget, [{ title: 'Data source of this template' }, ...ownSourceKinds(), { sep: true }, { label: 'Report parameter…', icon: 'sliders', desc: 'A choice on the report’s toolbar', run: () => paramEditor({ addNew: true }) }]));
    const sc = listEl.querySelector('.dx-scroll');
    if (sc) sc.scrollTop = keep;
    if (reveal) listEl.querySelector('.dx-row.on')?.scrollIntoView({ block: 'nearest' });
  };
  // a new connection: shared (Data role), or the open template's own (its designer)
  const addConn = () => (manage ? connectionDialog(null, { onSave: async (v) => { await saveConns([...shared, v]); } }) : tpl ? connectionDialog(null).then(() => render()) : null);
  ready = { pick, addConn };
  if (S.dataAfterLoad === 'conn') { S.dataAfterLoad = null; addConn(); }

  const previewInto = async (box, status, ds, fresh = false) => {
    status.textContent = 'Loading…';
    try {
      const p = await call('preview_dataset', { dataset: ds, keep: !fresh }, { quiet: true });
      status.textContent = `${p.total.toLocaleString()} rows · ${p.columns.length} columns${p.total > p.rows.length ? ` · first ${p.rows.length}` : ''}`;
      box.innerHTML = gridHtml(p.fields, p.rows.slice(0, 60));
      return p;
    } catch (e) {
      status.textContent = '';
      box.innerHTML = `<div class="err-box">${esc(String(e))}</div>`;
      return null;
    }
  };

  // one source's page: shared (Data area) or the template's own
  const sourcePage = (ds, isShared, repCtx = null) => {
    const used = repCtx ? [] : users(ds.id);
    const isDb = ds.source?.type === 'db';
    const canEdit = isShared ? manage : repCtx ? editRep : !!tpl;
    const opts = isShared ? { onSave: saveShared, conn: connOf(ds) } : repCtx ? { onSave: saveRep(repCtx.id), conn: connOf(ds) } : {};
    const others = isShared ? usageOf(ds.id) : [];
    const kind = isShared ? 'shared' : 'own';
    // one change to the source: shared ones go to the Data area, own ones to the template
    const saveProps = (patch) => {
      const next = { ...ds, ...patch };
      return isShared ? saveShared(next) : repCtx ? saveRep(repCtx.id)(next) : edit({ op: 'upsert_dataset', dataset: next });
    };
    detail.innerHTML = `<div class="dx-head k-${kind}"><span class="dx-hic">${icon(isShared ? 'link' : 'layout', 18)}</span><div class="dx-ht"><span class="dx-pill">${isShared ? 'Shared data source' : repCtx ? `Report “${esc(repCtx.title)}”` : `Only in “${esc(ownName)}”`}</span>${repCtx?.changed.includes(ds.id) ? '<span class="dx-pill dx-chg">changed in Data</span>' : ''}<h3>${esc(ds.name || ds.id)} ${srcTag(ds.source)}</h3><span>${esc(isShared && connOf(ds) ? 'SQL on ' + connOf(ds).name : sourceLabel(ds.source, isShared))}</span></div><span class="grow"></span>
        ${isShared && tpl ? (linked(ds.id) ? `<span class="dx-linked">${icon('check', 13)} In this template</span>` : `<button class="btn" data-a="use">${icon('plus', 14)} Use in this template</button>`) : ''}
 ${!canEdit && isDb ? `<button class="btn" data-a="query" title="Look at the query in the SQL workspace (read-only)">${icon('code', 14)} Open query</button>` : ''}
        ${canEdit ? `<button class="btn primary" data-a="${isDb ? 'query' : 'edit'}" title="${isDb ? 'Open the query in the SQL workspace' : 'Change where the data comes from'}">${icon(isDb ? 'code' : 'rename', 14)} ${isDb ? 'Edit query' : 'Edit source'}</button>` : ''}
        <button class="tb-btn" data-a="more" title="More">${icon('more', 16)}</button></div>
      ${repCtx && canEdit ? `<div class="dx-lock">${icon('info', 13)}<span>Changes here are kept for this report: report users get them at once, and they reach its template when the designer opens it — then go out with the next publish.</span></div>` : ''}
      ${!canEdit ? `<div class="dx-lock">${icon('lock', 13)}<span>${isShared ? 'Shared data sources are changed by the Data role. Use it in a template as it is — or make a source of the template\'s own.' : 'This source belongs to the published report. Its designer changes it in the template; here you can look at it, preview it and run its query.'}</span></div>` : ''}
      <div class="dx-props">
        <label><span>Name</span><input data-p="name" value="${esc(ds.name || '')}"${canEdit ? '' : ' disabled'}></label>
        <label><span>Date field <i>— lets widgets follow the period</i></span><select data-p="date"${canEdit ? '' : ' disabled'}><option value="">None</option>${ds.date_field ? `<option selected>${esc(ds.date_field)}</option>` : ''}</select></label>
        <div class="dx-prop"><span>Values over time</span><div class="seg dx-snap"${canEdit ? '' : ' data-off'}><button data-v="0" class="${ds.snapshot ? '' : 'on'}" title="Amounts that add up: income, trades, flows">Add up</button><button data-v="1" class="${ds.snapshot ? 'on' : ''}" title="Balances, prices, headcount: the latest date counts">Snapshots</button></div></div>
        <div class="dx-prop"><span>Used by</span><b>${tpl && (!isShared || linked(ds.id)) ? (used.length ? `${used.length} widget${used.length > 1 ? 's' : ''} · page${new Set(used.map((u) => u.pi)).size > 1 ? 's' : ''} ${[...new Set(used.map((u) => u.pi + 1))].slice(0, 8).join(', ')}` : 'no widget yet') : ''}${isShared ? `${tpl && linked(ds.id) ? ' · ' : ''}${others.length ? others.map((u) => esc(u.title || u.path.split('/').pop())).slice(0, 3).join(', ') + (others.length > 3 ? '…' : '') : tpl && linked(ds.id) ? 'no other template' : 'no saved template yet'}` : ''}</b></div>
      </div>
      <div class="dx-calcbar"><b>Calculated fields</b>${(ds.calc_fields || []).map((c, i) => `<button class="calc-chip" data-ci="${i}" title="${esc(c.expr)}"${canEdit ? '' : ' disabled'}><span class="fx">fx</span><b>${esc(c.name)}</b><span class="muted">${esc(c.expr)}</span></button>`).join('') || '<span class="muted">none</span>'}${canEdit ? `<button class="btn sm" data-a="calc">${icon('plus', 12)} Add</button>` : ''}</div>
      <div class="dx-pvbar"><b>Preview</b><span class="muted dx-status"></span><span class="grow"></span><button class="btn sm" data-a="reload">${icon('refresh', 13)} Reload</button></div>
      <div class="dx-pv"></div>
      <div class="dx-foot"></div>`;
    const $ = (a) => detail.querySelector(`[data-a="${a}"]`);
    // deleting, in plain sight: what it means depends on whose source it is
    const foot = detail.querySelector('.dx-foot');
    const added = repCtx?.added?.includes(ds.id), changed = repCtx?.changed.includes(ds.id);
    const del = isShared ? (manage && { label: 'Delete from the Data area…', hint: 'Every template using it keeps its last copy', run: () => removeSource(ds, true, used, others) })
      : repCtx ? (added ? { label: 'Delete this source', hint: 'It was added here, in the Data area', run: () => dropRep(true) }
        : changed ? { label: 'Undo the change made here', hint: 'Back to the published version', run: () => dropRep(false), undo: true }
          : null)
        : { label: 'Delete from this template…', hint: used.length ? `${used.length} widget${used.length > 1 ? 's' : ''} use it` : 'No widget uses it', run: () => removeSource(ds, false, used, others) };
    const dropRep = async (isAdded) => {
      if (isAdded) {
        const ok = await modal({ title: 'Delete the data source?', iconName: 'warn', iconKind: 'warn', body: `<p>“${esc(ds.name)}” was added to “${esc(repCtx.title)}” in the Data area. It goes from the report (and won't reach the template).</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
        if (!ok) return;
      }
      await call('auth', { cmd: 'drop_report_source', args: { id: repCtx.id, dataset: ds.id } });
      S.dataSel = isAdded ? null : S.dataSel;
      render();
    };
    if (del) {
      foot.innerHTML = `<button class="btn ${del.undo ? '' : 'danger-ghost'}" data-a="del">${icon(del.undo ? 'undo' : 'trash', 14)} ${esc(del.label)}</button><span class="muted">${esc(del.hint)}</span>`;
      $('del').addEventListener('click', del.run);
    } else if (repCtx) {
      foot.innerHTML = `<span class="muted">${icon('lock', 12)} This source comes with the published report: its designer removes it in the template.</span>`;
    }
    $('use')?.addEventListener('click', () => call('auth', { cmd: 'use_source', args: { id: ds.id } }).then((st) => { setState(st); toast(`“${ds.name}” is in the template — pick it in a widget’s Data tab.`, 'info'); }));
    $('query')?.addEventListener('click', () => editDbSource(ds, { saveSource: canEdit ? opts.onSave : null, conn: opts.conn || connOf(ds) }));
    $('edit')?.addEventListener('click', () => datasetEditor(ds, false, opts));
    $('calc')?.addEventListener('click', () => calcFieldEditor(ds, null, opts));
    detail.querySelectorAll('[data-ci]').forEach((b) => b.addEventListener('click', () => calcFieldEditor(ds, +b.dataset.ci, opts)));
    if (canEdit) {
      const nm = detail.querySelector('[data-p="name"]');
      nm.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') nm.blur(); });
      nm.addEventListener('change', () => { const v = nm.value.trim(); if (v && v !== ds.name) saveProps({ name: v }); });
      detail.querySelector('[data-p="date"]').addEventListener('change', (e) => saveProps({ date_field: e.target.value || null }));
      detail.querySelectorAll('.dx-snap button').forEach((b) => b.addEventListener('click', () => { if ((b.dataset.v === '1') !== !!ds.snapshot) saveProps({ snapshot: b.dataset.v === '1' }); }));
    }
    const items = [
      isShared && tpl && linked(ds.id) && { label: 'Remove from this template', icon: 'minus', desc: used.length ? `${used.length} widget${used.length > 1 ? 's' : ''} will show no data` : '', run: () => edit({ op: 'remove_dataset', id: ds.id }) },
      repCtx && repCtx.changed.includes(ds.id) && { label: repCtx.added?.includes(ds.id) ? 'Delete this source' : 'Undo the change made here', icon: repCtx.added?.includes(ds.id) ? 'trash' : 'undo', danger: !!repCtx.added?.includes(ds.id), run: () => dropRep(!!repCtx.added?.includes(ds.id)) },
      canEdit && !repCtx && { label: isShared ? 'Delete from the Data area…' : 'Delete…', icon: 'trash', danger: true, run: () => removeSource(ds, isShared, used, others) },
    ].filter(Boolean);
    $('more').hidden = !items.length;
    $('more').addEventListener('click', (e) => menu(e.currentTarget, items, { align: 'end' }));
    const box = detail.querySelector('.dx-pv'), status = detail.querySelector('.dx-status');
    // the date choices come from the loaded columns
    const fillDates = (fields) => {
      const sel = detail.querySelector('[data-p="date"]');
      const dates = fields.filter((f) => f.kind === 'date').map((f) => f.name);
      if (ds.date_field && !dates.includes(ds.date_field)) dates.unshift(ds.date_field);
      sel.innerHTML = `<option value="">None</option>${dates.map((d) => `<option${d === ds.date_field ? ' selected' : ''}>${esc(d)}</option>`).join('')}`;
    };
    $('reload').addEventListener('click', () => previewInto(box, status, ds, true).then((p) => p && fillDates(p.fields)));
    previewInto(box, status, ds).then((p) => p && fillDates(p.fields));
  };

  // deleting says what it breaks
  const removeSource = async (ds, isShared, used, others) => {
    const lines = [];
    if (!used.length && isShared && linked(ds.id)) lines.push('<li><b>This template</b> has it (no widget uses it yet) and keeps a copy, which no longer gets changes.</li>');
    if (used.length) lines.push(`<li><b>This template</b>: ${used.length} widget${used.length > 1 ? 's' : ''} on page${new Set(used.map((u) => u.pi)).size > 1 ? 's' : ''} ${[...new Set(used.map((u) => u.pi + 1))].join(', ')} ${isShared ? 'keep a copy of it, which no longer gets changes from the Data area' : 'will show no data'}.</li>`);
    for (const u of others) lines.push(`<li><b>${esc(u.title || u.path.split('/').pop())}</b> (${u.widgets} widget${u.widgets === 1 ? '' : 's'}) keeps its own copy, which no longer gets changes.</li>`);
    const body = `<p>${isShared ? `“${esc(ds.name)}” leaves the Data area for everyone.` : `“${esc(ds.name)}” is removed from this template.`}</p>
      ${lines.length ? `<p>What it affects:</p><ul class="dx-warn">${lines.join('')}</ul>` : '<p>Nothing uses it.</p>'}
      ${isShared ? '<p class="muted">Templates not opened on this computer are not listed.</p>' : ''}`;
    const ok = await modal({ title: 'Delete the data source?', iconName: 'warn', iconKind: 'warn', body, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
    if (!ok) return;
    S.dataSel = null;
    if (isShared) { const r = await call('auth', { cmd: 'delete_source', args: { id: ds.id } }); setState(r.state); }
    else edit({ op: 'remove_dataset', id: ds.id });
  };

  /** A template's or a report's parameters, as cards; `editable` for the open template. */
  const paramsPage = (P, title, editable) => {
      detail.innerHTML = `<div class="dx-head k-own"><span class="dx-hic">${icon('sliders', 18)}</span><div class="dx-ht"><span class="dx-pill">${editable ? `Only in “${esc(title)}”` : `Report “${esc(title)}”`}</span><h3>Report parameters</h3><span>Choices on the report's toolbar — the values queries and texts follow</span></div><span class="grow"></span>${editable ? `<button class="btn" data-a="edit">${icon('rename', 14)} Edit parameters</button><button class="btn primary" data-a="add">${icon('plus', 14)} New parameter</button>` : ''}</div>
        <div class="pm-sec">Report parameters</div><div class="pm-grid" data-g="params"><div class="ar-note">Loading…</div></div>
        <div class="pm-sec">Always available</div><div class="pm-grid" data-g="fixed"></div>
        <div class="pm-sec vars-sec" hidden>Variables SQL can use <span class="muted">— made on the Variables tab</span></div><div class="pm-grid" data-g="vars"></div>`;
      detail.querySelector('[data-a="edit"]')?.addEventListener('click', () => paramEditor());
      detail.querySelector('[data-a="add"]')?.addEventListener('click', () => paramEditor({ addNew: true }));
      const tok = (t, what) => `<button class="pm-tok" data-copy="${esc(t)}" title="Copy — ${esc(what)}"><code>${esc(t)}</code>${icon('copy', 11)}</button>`;
      const card = (o) => `<div class="pm-card${o.cls ? ' ' + o.cls : ''}${o.edit ? ' pm-edit' : ''}"${o.edit ? ` data-edit="${esc(o.edit)}" title="Click to edit this parameter"` : ''}>
          <div class="pm-top"><b>${esc(o.label)}</b>${o.badge ? `<span class="pm-badge">${esc(o.badge)}</span>` : ''}</div>
          <div class="pm-toks">${o.toks.join('')}</div>
          <div class="pm-val"><span>Value now</span>${o.value ? esc(o.value) : '<em>not set</em>'}</div>
          ${o.lines.map(([k, v]) => `<div class="pm-line"><span>${esc(k)}</span>${v}</div>`).join('')}
        </div>`;
      Promise.all([call('db_params', {}, { quiet: true }), call('auth', { cmd: 'param_reach' }, { quiet: true }).catch(() => [])]).then(([list, reach]) => {
        const reachOf = (n) => reach.find((r) => r?.name === n);
        const reachLine = (n) => {
          const r = reachOf(n);
          if (!r) return null;
          const gaps = r.not || [];
          const ok = !gaps.length;
          const body = ok
            ? `${icon('ok', 12)}<span>followed by all ${r.follow} sources${r.apart?.length ? ` · left out on purpose: ${r.apart.map(esc).join(', ')}` : ''}</span>`
            : `${icon('warn', 12)}<span><b>${gaps.length} source${gaps.length > 1 ? 's don’t' : ' doesn’t'} follow it</b>: ${gaps.map((g) => `${esc(g.name)}${g.widgets.length ? ` (${g.widgets.length} widget${g.widgets.length > 1 ? 's' : ''}, page${g.widgets.length > 1 ? 's' : ''} ${[...new Set(g.widgets.map((w) => w.page))].join(', ')})` : ''}`).join('; ')} — they show every value. Use <code>:${esc(n)}</code> in their SQL, pick their column in the parameter's settings, or set them to “Not filtered” if that is meant.</span>`;
          return [ok ? 'Reach' : 'Check', `<span class="pm-reach ${ok ? 'ok' : 'gap'}">${body}</span>`];
        };
        const val = (n) => list.find((x) => x.name === n)?.value || '';
        const userWord = { change: 'can change it', show: 'see it, can’t change it', hide: 'don’t see it' };
        detail.querySelector('[data-g="params"]').innerHTML = P.param_defs.map((p) => card({
          edit: editable ? p.name : null, label: p.label || p.name, badge: p.multi ? 'several values' : p.kind === 'list' || p.source || p.options?.length ? 'a list' : p.kind || 'text',
          toks: [tok(`:${p.name}`, 'in SQL'), tok(`{{${p.name}}}`, 'in texts and widget filters')], value: val(p.name),
          lines: [
            ['Choices', esc(p.source ? `from “${P.datasets.find((x) => x.id === p.source.dataset)?.name || p.source.dataset}” · ${p.source.field}` : p.options.length ? p.options.join(', ') : 'free text')],
            ['Filters', esc(p.field ? `the column ${p.field} in every source that has it` : 'only where you use the token')],
            ['Report users', esc(userWord[p.user || 'change'] || 'can change it')],
            ...(reachLine(p.name) ? [reachLine(p.name)] : []),
          ],
        })).join('') + (editable ? `<button class="pm-card pm-new" data-a="add2">${icon('plus', 18)}<b>New parameter</b><span>A choice on the report's toolbar — a client, a region, a currency…</span></button>` : '');
        detail.querySelector('[data-a="add2"]')?.addEventListener('click', () => paramEditor({ addNew: true }));
        detail.querySelectorAll('[data-edit]').forEach((c) => c.addEventListener('click', (e) => { if (!e.target.closest('[data-copy]')) paramEditor({ only: c.dataset.edit }); }));
        const fixed = [['period_start', 'Period start', 'First day of the report period'], ['period_end', 'Period end', 'Last day of the report period'], ['data_start', 'Data start', 'First day loaded: the period plus what comparisons and history need'], ['data_end', 'Data end', 'Last day loaded']];
        detail.querySelector('[data-g="fixed"]').innerHTML = fixed.map(([n, l, about]) => card({ label: l, cls: 'fixed', toks: [tok(`:${n}`, 'in SQL')], value: val(n) ? fmtDay(val(n)) : '', lines: [['', esc(about)]] })).join('');
        const vars = list.filter((x) => x.var && !x.bad);
        detail.querySelector('.vars-sec').hidden = !vars.length;
        detail.querySelector('[data-g="vars"]').innerHTML = vars.map((x) => card({ label: x.name, cls: 'var', toks: [tok(`:${x.name}`, 'in SQL')], value: x.value, lines: [['', esc(x.about || '')]] })).join('');
        detail.querySelectorAll('[data-copy]').forEach((b) => b.addEventListener('click', async () => {
          try { await navigator.clipboard.writeText(b.dataset.copy); toast(`${b.dataset.copy} copied.`); } catch { toast(b.dataset.copy, 'info'); }
        }));
      }).catch((e) => { detail.querySelector('[data-g="params"]').innerHTML = `<div class="err-box">${esc(String(e))}</div>`; });
  };

  const drawDetail = () => {
    // a published report's own: its pages become the preview context first
    if (sel && (sel.kind === 'rds' || sel.kind === 'rparams')) {
      const [rid, dsid] = sel.kind === 'rds' ? sel.id.split('::') : [sel.id, ''];
      const r = reps.find((x) => x.id === rid);
      detail.innerHTML = '<div class="ar-note">Loading…</div>';
      loadRep(rid, false).then((d) => {
        if (!d) { detail.innerHTML = '<div class="ar-note bad">This report can’t be read.</div>'; return; }
        if (sel.kind === 'rparams') return paramsPage(d, r?.title || rid, false);
        const ds = d.datasets.find((x) => x.id === dsid);
        if (ds) sourcePage(ds, false, { id: rid, title: r?.title || rid, changed: d.changed, added: d.added || [] });
      });
      return;
    }
    if (!sel) { detail.innerHTML = `<div class="ar-note">${icon('database', 16)} ${manage ? 'Connect a database to start.' : 'Nothing here yet.'}</div>`; return; }
    if (sel.kind === 'new') {
      const canShared = manage && shared.length >= 0;
      detail.innerHTML = `<div class="dx-head"><span class="ar-ic">${icon('plus', 18)}</span><div><h3>New data source</h3><span>Where it lives decides who can use it</span></div></div>
        <div class="dx-new">
          <div class="dx-opt${canShared ? '' : ' off'}"><b>${icon('link', 15)} Shared</b><span>In the Data area: every template can use it, and changes here reach them all.${canShared ? '' : ' Made by the Data role.'}</span><div class="dx-kinds" data-w="shared"></div></div>
          <div class="dx-opt${tpl ? '' : ' off'}"><b>${icon('table', 15)} Only in ${tpl ? `“${esc(docName)}”` : 'a template'}</b><span>Belongs to the template; other templates can't use it.${tpl ? '' : ' Open a template first.'}</span><div class="dx-kinds" data-w="own"></div></div>
        </div>`;
      const fill = (box, items) => {
        box.innerHTML = items.map((it, i) => `<button class="dx-kind" data-i="${i}">${icon(it.icon, 15)}<span>${esc(it.label)}</span></button>`).join('');
        box.querySelectorAll('.dx-kind').forEach((b) => b.addEventListener('click', () => items[+b.dataset.i].run()));
      };
      const connect = { label: 'Connect a database…', icon: 'plus', run: addConn };
      if (canShared) fill(detail.querySelector('[data-w="shared"]'), [...sharedSourceKinds(shared, saveShared), connect]);
      if (tpl) fill(detail.querySelector('[data-w="own"]'), [...ownSourceKinds(), ...(manage ? [] : [connect]), { label: 'Report parameter (not a source)', icon: 'sliders', run: () => paramEditor({ addNew: true }) }]);
      return;
    }
    if (sel.kind === 'lib') return sourcePage(lib.datasets.find((d) => d.id === sel.id), true);
    if (sel.kind === 'ds') return sourcePage(own.find((d) => d.id === sel.id), false);
    if (sel.kind === 'params') return paramsPage(src, ownName, !!tpl);
    const c = conns.find((x) => x.id === sel.id);
    // shared connections are the Data role's; a template's own, its designer's
    const canEditConn = c.shared ? manage : !!tpl && inTpl(c);
    const editConn = () => (c.shared
      ? connectionDialog(c, { onSave: (v) => saveConns(shared.map((x) => (x.id === c.id ? { ...v, id: c.id } : x))) })
      : connectionDialog(c).then(() => render()));
    const feeds = [...lib.datasets.filter((d) => d.source?.connection === c.id).map((d) => ({ d, shared: true })), ...own.filter((d) => d.source?.type === 'db' && d.source.connection === c.id).map((d) => ({ d, shared: false }))];
    detail.innerHTML = `<div class="dx-head k-conn"><span class="dx-hic">${icon('database', 18)}</span><div class="dx-ht"><span class="dx-pill">${c.shared ? 'Shared connection' : `Connection only in “${esc(docName)}”`}</span><h3>${esc(c.name)}</h3><span>${esc(({ duckdb: 'DuckDB file', sqlite: 'SQLite file', postgres: 'PostgreSQL', clickhouse: 'ClickHouse' })[c.kind] || c.kind)} · ${esc(connWhere(c))}</span></div><span class="grow"></span>
        ${canEditConn ? `<button class="btn" data-a="edit-conn" title="Its name, type and where it is — Test checks it">${icon('rename', 14)} Edit connection</button>` : ''}
        <button class="btn primary" data-a="ws">${icon('code', 14)} SQL workspace</button>
        <button class="tb-btn" data-a="more" title="More">${icon('more', 16)}</button></div>
      <div class="dx-facts">
        <span><i>Type</i>${esc(({ duckdb: 'DuckDB file', sqlite: 'SQLite file', postgres: 'PostgreSQL server', clickhouse: 'ClickHouse server' })[c.kind] || c.kind)}</span>
        <span><i>${c.kind === 'postgres' || c.kind === 'clickhouse' ? 'Server' : 'File'}</i>${esc(c.kind === 'postgres' || c.kind === 'clickhouse' ? connWhere(c) : c.path || 'not chosen')}</span>
        <span><i>Managed</i>${c.shared ? 'in the Data area — shared' : 'only in this template'}</span>
        <span><i>Data sources on it</i>${feeds.length ? feeds.slice(0, 6).map((f) => esc(f.d.name)).join(', ') + (feeds.length > 6 ? ` +${feeds.length - 6}` : '') : 'none yet'}</span>
      </div>
      <div class="dx-pvbar"><b>Tables</b><span class="muted dx-status">Loading…</span></div>
      <div class="dx-tables"></div>`;
    const $ = (a) => detail.querySelector(`[data-a="${a}"]`);
    const open = () => workspace(c, { global: true, saveSource: c.shared && manage ? saveShared : null });
    $('ws').addEventListener('click', open);
    $('edit-conn')?.addEventListener('click', editConn);
    $('more').addEventListener('click', (e) => menu(e.currentTarget, [
      { label: 'Open SQL workspace', icon: 'code', run: open },
      canEditConn && { label: 'Edit connection…', icon: 'rename', run: editConn },
      tpl && inTpl(c) && !feeds.some((f) => !f.shared || linked(f.d.id)) && { label: 'Remove from this template', icon: 'minus', run: () => edit({ op: 'set_connections', connections: tpl.connections.filter((x) => x.id !== c.id) }) },
      manage && c.shared && { label: 'Delete from the Data area…', icon: 'trash', danger: true, run: async () => {
        const n = lib.datasets.filter((d) => d.source?.connection === c.id).length;
        const ok = await modal({ title: 'Delete the connection?', iconName: 'warn', iconKind: 'warn', body: `<p>“${esc(c.name)}” leaves the Data area for everyone.${n ? ` <b>${n} shared data source${n > 1 ? 's' : ''}</b> on it will stop loading.` : ''} Templates that already use it keep their copy.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
        if (ok) saveConns(shared.filter((x) => x.id !== c.id));
      } },
    ], { align: 'end' }));
    call('db_tables', { connection: c }, { quiet: true }).then((t) => {
      detail.querySelector('.dx-status').textContent = `${t.length} table${t.length === 1 ? '' : 's'}`;
      detail.querySelector('.dx-tables').innerHTML = t.map((x) => `<button class="dx-tbl" data-t="${esc(x.name)}" title="Open in the SQL workspace">${icon('table', 14)}<b>${esc(x.name)}</b><span>${x.rows != null ? fmtCount(x.rows) + ' rows' : ''}${x.columns?.length ? ` · ${x.columns.length} columns` : ''}</span></button>`).join('');
      detail.querySelectorAll('.dx-tbl').forEach((b) => b.addEventListener('click', open));
    }).catch((e) => { detail.querySelector('.dx-status').textContent = ''; detail.querySelector('.dx-tables').innerHTML = `<div class="err-box">${esc(String(e))}</div>`; });
  };
  drawList(true);
  drawDetail();
}

/** Admin → Groups: a group's name, members and the reports given to it. */
async function groupsPage(el) {
  el.innerHTML = `${areaHead('admin', 'Groups of people: give a report to a group instead of person by person.',
    `<button class="btn primary" data-a="addg">${icon('plus', 14)} New group</button>`, 'Groups')}
    <div class="ad-split">
      <section class="ar-card"><div class="ar-card-t">${icon('users', 15)} Groups</div><div class="gp-list"><div class="ar-note">Loading…</div></div></section>
      <section class="ar-card gp-edit"></section>
    </div>`;
  let g, reps;
  try { [g, reps] = await Promise.all([call('auth', { cmd: 'groups' }, { quiet: true }), call('auth', { cmd: 'reports' }, { quiet: true })]); } catch (e) { el.querySelector('.gp-list').innerHTML = `<div class="ar-note bad">${esc(String(e))}</div>`; return; }
  let sel = S.groupSel && g.groups.some((x) => x.id === S.groupSel) ? S.groupSel : g.groups[0]?.id ?? null;
  const listEl = el.querySelector('.gp-list'), ed = el.querySelector('.gp-edit');
  const drawList = () => {
    listEl.innerHTML = g.groups.map((x) => `<button class="gp-row${x.id === sel ? ' on' : ''}" data-id="${esc(x.id)}"><span class="pub-ic">${icon('users', 14)}</span><span class="pub-t"><b>${esc(x.name)}</b><span>${x.members.length} member${x.members.length === 1 ? '' : 's'} · ${reps.filter((r) => r.audience?.groups?.includes(x.id)).length} report${reps.filter((r) => r.audience?.groups?.includes(x.id)).length === 1 ? '' : 's'}</span></span></button>`).join('') || '<div class="ar-note">No groups yet.</div>';
    listEl.querySelectorAll('.gp-row').forEach((b) => b.addEventListener('click', () => { sel = b.dataset.id; S.groupSel = sel; drawList(); drawEd(); }));
  };
  const drawEd = () => {
    const isNew = sel === '';
    const x = isNew ? { id: '', name: '', description: '', members: [], ai_level: '' } : g.groups.find((y) => y.id === sel);
    if (!x) { ed.innerHTML = '<div class="ar-note">Pick a group, or make one.</div>'; return; }
    const members = new Set(x.members);
    const given = reps.filter((r) => r.audience?.groups?.includes(x.id));
    ed.innerHTML = `<div class="ad-form">
      <div class="ad-title"><span class="av lg">${icon('users', 18)}</span><div><h3>${isNew ? 'New group' : esc(x.name)}</h3><span>${isNew ? 'Name it and pick its members' : `${x.members.length} member${x.members.length === 1 ? '' : 's'}`}</span></div></div>
      <div class="ad-grid"><label>Name<input name="gname" value="${esc(x.name)}" placeholder="e.g. Private bankers"></label><label>About<input name="gdesc" value="${esc(x.description)}" placeholder="optional"></label></div>
      <div class="ad-sec">Members</div>
      <div class="gp-people">${g.people.map((p) => `<label class="pub-item"><input type="checkbox" value="${esc(p.login)}"${members.has(p.login) ? ' checked' : ''}><span class="pub-av">${esc(initials(p.name || p.login))}</span><span class="pub-t"><b>${esc(p.name || p.login)}</b><span>${esc(p.login)}${p.user ? '' : ' · no Reports role'}</span></span></label>`).join('')}</div>
      <div class="ad-sec">AI <span class="muted">— what AI assistants may send for its members</span></div>
      <div class="ad-ai"><select name="gai"><option value="">Not set — the bank’s setting</option>${AI_LEVELS.map(([v, l]) => `<option value="${v}"${(x.ai_level === 'full' ? 'aggregates' : x.ai_level) === v ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>
        <div class="hint">A member’s own setting (Users) comes first; in several groups, a member gets the widest.</div></div>
      ${isNew ? '' : `<div class="ad-sec">Reports given to it</div><div class="gp-reps">${given.map((r) => `<span class="gp-rep">${icon('page', 12)} ${esc(r.title)}${r.published ? '' : ' <i>unpublished</i>'}</span>`).join('') || '<span class="muted">None yet — a designer gives reports to groups in the template’s Access tab.</span>'}</div>`}
      <div class="ad-err" hidden></div>
      <div class="ad-acts">${isNew ? '' : `<button class="btn danger-ghost" data-a="delg">${icon('trash', 14)} Delete</button>`}<span class="grow"></span><button class="btn primary" data-a="saveg">${icon('save', 14)} ${isNew ? 'Make group' : 'Save'}</button></div>
    </div>`;
    ed.querySelector('[data-a="saveg"]').addEventListener('click', async () => {
      const group = { id: x.id, name: ed.querySelector('[name=gname]').value.trim(), description: ed.querySelector('[name=gdesc]').value.trim(), members: [...ed.querySelectorAll('.gp-people input:checked')].map((i) => i.value), ai_level: ed.querySelector('[name=gai]').value };
      try {
        const r = await call('auth', { cmd: 'save_group', args: { group } }, { quiet: true });
        g.groups = r.groups;
        sel = isNew ? r.groups.find((y) => y.name === group.name)?.id ?? null : x.id;
        S.groupSel = sel;
        toast(isNew ? `Group “${group.name}” made` : 'Saved');
        drawList(); drawEd();
      } catch (e) { const er = ed.querySelector('.ad-err'); er.textContent = String(e); er.hidden = false; }
    });
    ed.querySelector('[data-a="delg"]')?.addEventListener('click', async () => {
      const ok = await modal({ title: 'Delete the group?', iconName: 'warn', iconKind: 'warn', body: `<p>“${esc(x.name)}” goes. ${given.length ? `Its members lose <b>${given.map((r) => esc(r.title)).join(', ')}</b> unless they get it another way.` : 'No report is given to it.'}</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
      if (!ok) return;
      g.groups = (await call('auth', { cmd: 'delete_group', args: { id: x.id } })).groups;
      sel = g.groups[0]?.id ?? null;
      drawList(); drawEd();
    });
  };
  el.querySelector('[data-a="addg"]').addEventListener('click', () => { sel = ''; drawList(); drawEd(); setTimeout(() => ed.querySelector('[name=gname]')?.focus(), 30); });
  drawList();
  drawEd();
}

const fmtDay = (d) => { const [y, m, dd] = String(d).split('-'); return dd ? `${dd}.${m}.${y}` : d; };
const fmtCount = (n) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)} M` : n >= 1e4 ? `${Math.round(n / 1e3)} K` : String(n));

// ---------------------------------------------------------------- Admin

// what AI may send for a person: their own setting, else the widest of their groups', else the bank's
export const AI_LEVELS = [['structure', 'Structure only'], ['aggregates', 'Figures — what a report shows, names and IDs masked']];
// (an older "full" counts as figures: identities are masked at every level)
const AI_RANK = { structure: 0, aggregates: 1, full: 1 };
export const aiLevelName = (v) => (AI_LEVELS.find((x) => x[0] === v) || AI_LEVELS[0])[1].split(' — ')[0];
// the bank's own level (Settings → AI assistant), read fresh: it may have just changed
const bankLevel = async () => { try { return (await call('ai_status', {}, { quiet: true })).policy?.level || 'structure'; } catch { return 'structure'; } };
export const aiFor = (own, groups, bank) => {
  if (own) return [own, 'set for this person'];
  const g = groups.filter((x) => x.ai_level).sort((a, b) => AI_RANK[b.ai_level] - AI_RANK[a.ai_level])[0];
  return g ? [g.ai_level, `set for the group “${g.name}”`] : [bank, 'the bank’s setting'];
};

export const ROLE_INFO = {
  admin: ['Admin', 'Users, passwords, roles and access — and every other role'],
  data: ['Data', 'Connections and the SQL workspace'],
  design: ['Design', 'Templates and themes'],
  user: ['User', 'Reports for the clients in their access'],
};
export const ROLE_ORDER = ['admin', 'data', 'design', 'user'];

// the Admin area: its parts on the left, the chosen one on the right
const ADMIN_PARTS = [
  ['users', 'user', 'Users & roles', 'Who can sign in and what each person can do'],
  ['groups', 'users', 'Groups', 'People who share reports and settings'],
  ['access', 'lock', 'Access', 'Who sees which clients, products and reports'],
  ['ai', 'sparkles', 'AI', 'What AI may send, providers, keys'],
  ['activity', 'clock', 'Activity', 'What people did: sign-ins, documents, edits, data, AI'],
  ['proposals', 'briefcase', 'Proposals', 'Every proposal bankers made: content, versions, status, the AI behind it'],
  // the web server answers the report API; the desktop app doesn't
  ...(WEB ? [['api', 'link', 'API', 'Systems that ask for reports: their keys, access and what each report accepts']] : []),
];

async function adminArea(el) {
  // the AI button (⌘I) here: the administrator's assistant
  setAreaAI('admin', () => { toggleAdminAI(); return 'handled'; });
  const part = ADMIN_PARTS.some((p) => p[0] === S.adminTab) ? S.adminTab : 'users';
  el.innerHTML = `<div class="adm"><nav class="adm-nav">${ADMIN_PARTS.map(([k, ic, label, tip]) => `<button class="adm-item${k === part ? ' on' : ''}" data-part="${k}" title="${esc(label)} — ${esc(tip)}">${icon(ic, 16)}<span>${esc(label)}</span></button>`).join('')}
    <button class="adm-item adm-fold" data-a="fold" title="Fold or unfold this menu (⌘[)">${icon('sidebar-left', 16)}<span>Fold menu</span></button></nav><div class="adm-body"></div></div>`;
  el.querySelectorAll('[data-part]').forEach((b) => b.addEventListener('click', () => { S.adminTab = b.dataset.part; render(); }));
  el.querySelector('[data-a="fold"]').addEventListener('click', () => toggleAreaFold('admin'));
  // the assistant's drawer belongs to the page (its parts redraw only .adm-body)
  mountAdminAI(el);
  const body = el.querySelector('.adm-body');
  if (part === 'groups') return groupsPage(body);
  if (part === 'access') return accessPage(body);
  if (part === 'ai') return aiAdminPage(body);
  if (part === 'activity') return activityPage(body);
  if (part === 'proposals') return proposalsAdminPage(body);
  if (part === 'api') return apiAdminPage(body);
  return usersPage(body);
}

/**
 * The client access editor (Users & roles, Admin → API): every client, or bankers' coverage and
 * clients one by one; every product or some; with who that comes to. Changes `u.access` in place
 * (`u` is a person or an API account; `u._prodOpen` keeps "Selected products" open while none is ticked).
 */
export function accessEditor(accEl, u, opts) {
  const drawAccess = () => {
    if (u.roles?.admin) { accEl.innerHTML = `<div class="ar-note">${icon('shield', 14)} Administrators see every client.</div>`; return; }
    const a = u.access;
    const prodAll = !a.sources.length && !u._prodOpen;
    accEl.innerHTML = `<div class="ad-part"><div class="ad-pt">Clients <span class="muted">— who they report on</span></div>
      <div class="seg ad-scope"><button data-v="all" class="${a.all ? 'on' : ''}">Every client</button><button data-v="some" class="${a.all ? '' : 'on'}">Selected clients</button></div>
      ${a.all ? '' : `<div class="ad-cols">
        <div class="ad-col"><div class="ad-ct">Coverage <span class="muted">— the clients of these bankers</span></div>
          <div class="ad-checks">${[...(opts?.bankers || [])].sort((x, y) => a.coverage.includes(y.id) - a.coverage.includes(x.id)).map((b) => `<label><input type="checkbox" data-k="coverage" value="${esc(b.id)}"${a.coverage.includes(b.id) ? ' checked' : ''}><span>${esc(b.name)}</span><i>${b.clients}</i></label>`).join('')}</div></div>
        <div class="ad-col"><div class="ad-ct">Clients <span class="muted">— one by one</span></div>
          <input class="ad-cfind" placeholder="Find a client">
          <div class="ad-checks tall">${[...(opts?.clients || [])].sort((x, y) => a.clients.includes(y.id) - a.clients.includes(x.id)).map((c) => `<label data-q="${esc(`${c.name} ${c.id} ${c.banker}`.toLowerCase())}"><input type="checkbox" data-k="clients" value="${esc(c.id)}"${a.clients.includes(c.id) ? ' checked' : ''}><span>${esc(c.name)}</span><i>${esc(c.id)}</i></label>`).join('')}</div></div>
      </div>`}</div>
      <div class="ad-part"><div class="ad-pt">Products <span class="muted">— which accounts their reports show</span></div>
        <div class="seg ad-scope ad-prod"><button data-v="all" class="${prodAll ? 'on' : ''}">Every product</button><button data-v="some" class="${prodAll ? '' : 'on'}">Selected products</button></div>
        ${prodAll ? '' : `<div class="ad-prods">${(opts?.sources || []).map((x) => `<label class="ad-prodc${a.sources.includes(x.id) ? ' on' : ''}"><input type="checkbox" data-k="sources" value="${esc(x.id)}"${a.sources.includes(x.id) ? ' checked' : ''}><span class="ad-rbox">${icon('check', 13, 2.4)}</span><span class="ad-rtx"><b>${esc(srcName(x.id))} <code>${esc(x.id)}</code></b><span>${srcDesc(x.id) ? `${esc(srcDesc(x.id))} · ` : ''}${x.clients} clients</span></span></label>`).join('')}</div>
        <div class="hint">Only clients with such an account are listed, and their reports show only these accounts — pages of other products are left out.</div>`}
      </div>
      <div class="ad-prev"></div>
      ${opts?.db ? `<div class="ad-src">${icon('database', 12)} Read from ${esc(opts.db.split('/').pop())}: <code>clients.sales_id</code>, <code>accounts.src_cd</code></div>` : ''}`;
    accEl.querySelectorAll('.ad-prod button').forEach((b) => b.addEventListener('click', () => {
      if (b.dataset.v === 'all') { u.access.sources = []; u._prodOpen = false; } else u._prodOpen = true;
      drawAccess();
    }));
    accEl.querySelectorAll('.ad-scope:not(.ad-prod) button').forEach((b) => b.addEventListener('click', () => { u.access.all = b.dataset.v === 'all'; drawAccess(); }));
    accEl.querySelectorAll('input[data-k]').forEach((x) => x.addEventListener('change', () => {
      const k = x.dataset.k;
      u.access[k] = x.checked ? [...new Set([...u.access[k], x.value])] : u.access[k].filter((v) => v !== x.value);
      x.closest('.ad-prodc')?.classList.toggle('on', x.checked);
      preview();
    }));
    accEl.querySelector('.ad-cfind')?.addEventListener('input', (e) => {
      const q = e.target.value.trim().toLowerCase();
      accEl.querySelectorAll('.ad-checks.tall label').forEach((l) => { l.hidden = !!q && !l.dataset.q.includes(q); });
    });
    preview();
  };
  const preview = async () => {
    const pv = accEl.querySelector('.ad-prev');
    if (!pv) return;
    // the latest preview wins (the editor is drawn again as roles change)
    const seq = accEl._seq = (accEl._seq || 0) + 1;
    try {
      const r = await call('auth', { cmd: 'preview', args: { access: u.access } }, { quiet: true });
      if (seq !== accEl._seq) return;
      if (r.all) { pv.innerHTML = `${icon('users', 14)} <span>Sees <b>every client</b>.</span>`; pv.className = 'ad-prev'; return; }
      const n = r.clients.length;
      pv.className = `ad-prev${n ? '' : ' none'}`;
      pv.innerHTML = n
        ? `${icon('users', 14)} <span>Sees <b>${n} client${n === 1 ? '' : 's'}</b>: ${r.clients.slice(0, 10).map((c) => esc(c.name)).join(', ')}${n > 10 ? ' …' : ''}</span>`
        : `${icon('lock', 14)} <span>Sees <b>no clients</b> — tick a banker, a source system or a client.</span>`;
    } catch (e) { pv.innerHTML = `${icon('warn', 14)} ${esc(String(e))}`; }
  };
  drawAccess();
}

// what a person's client access is, in a few words
export function accessText(u, opts) {
  if (u.roles.admin) return 'Every client (admin)';
  const a = u.access || {};
  let who = 'Every client';
  if (!a.all) {
    const parts = [];
    if (a.coverage?.length) parts.push(a.coverage.length > 2 ? `${a.coverage.length} bankers` : a.coverage.map((id) => opts?.bankers.find((b) => b.id === id)?.name || id).join(', '));
    if (a.clients?.length) parts.push(`${a.clients.length} client${a.clients.length > 1 ? 's' : ''}`);
    who = parts.length ? parts.join(' + ') : 'No clients';
  }
  return a.sources?.length ? `${who} · ${a.sources.map(srcName).join(', ')} only` : who;
}

/** Admin → Users & roles: the people, and one person's account, roles, access, groups, reports and AI. */
async function usersPage(el) {
  el.innerHTML = `${areaHead('admin', 'Who can sign in, what each person can do, which clients they can report on and which reports they get.',
    `<button class="btn" data-a="aiaccess">${icon('sparkles', 14)} Change with AI</button><button class="btn primary" data-a="add">${icon('user-plus', 14)} Add user</button>`, 'Users & roles')}
    <div class="ad-split">
      <section class="ar-card ad-users">
        <div class="ar-card-t">${icon('users', 15)} Users <span class="muted ad-n"></span><span class="grow"></span><input class="ad-find" placeholder="Find"></div>
        <div class="ad-head"><span>Person</span><span class="ad-roles-h">${ROLE_ORDER.map((r) => `<i title="${esc(ROLE_INFO[r][0])} — ${esc(ROLE_INFO[r][1])}">${icon(AREAS.find((a) => a.key === r).icon, 14)}</i>`).join('')}</span><span>Clients</span></div>
        <div class="ad-list"><div class="ar-note">Loading…</div></div>
      </section>
      <section class="ar-card ad-edit"><div class="ar-note">Pick a user to edit, or add one.</div></section>
    </div>`;
  let data = { users: [], me: '' };
  let opts = null;
  let grp = { groups: [], people: [] }, reps = [];
  let sel = null; // login being edited ('' = new)
  const listEl = el.querySelector('.ad-list');
  const editEl = el.querySelector('.ad-edit');
  const accessTextFor = (u) => accessText(u, opts);
  const drawList = () => {
    const q = el.querySelector('.ad-find').value.trim().toLowerCase();
    const shown = data.users.filter((u) => !q || `${u.name} ${u.login} ${u.email}`.toLowerCase().includes(q));
    el.querySelector('.ad-n').textContent = String(data.users.length);
    listEl.innerHTML = shown.map((u) => {
      const eff = u.roles.admin ? { admin: true, data: true, design: true, user: true } : u.roles;
      return `<div class="ad-row${u.login === sel ? ' on' : ''}${u.active ? '' : ' off'}" data-l="${esc(u.login)}">
        <span class="av">${esc(initials(u.name || u.login))}</span>
        <span class="ad-who"><b>${esc(u.name || u.login)}${u.login === data.me ? ' <em>you</em>' : ''}</b><span>${esc(u.login)}${u.active ? '' : ' · disabled'}${u.demo_password ? ` <span class="pw-tag" title="Still the demo password — set a new one">demo pw</span>` : ''}</span></span>
        <span class="ad-marks">${ROLE_ORDER.map((r) => `<i class="mk${eff[r] ? ' on' : ''}${r !== 'admin' && u.roles.admin ? ' via' : ''}" title="${esc(ROLE_INFO[r][0])}${r !== 'admin' && u.roles.admin ? ' (with Admin)' : ''}">${eff[r] ? icon('check', 12, 2.4) : ''}</i>`).join('')}</span>
        <span class="ad-acc">${esc(accessTextFor(u))}${(() => { const gs = grp.groups.filter((g) => g.members.includes(u.login)).map((g) => g.name); return gs.length ? `<i class="ad-gl">${esc(gs.join(', '))}</i>` : ''; })()}</span>
      </div>`;
    }).join('') || '<div class="ar-note">No user matches.</div>';
    listEl.querySelectorAll('.ad-row').forEach((r) => r.addEventListener('click', () => { sel = r.dataset.l; drawList(); drawEditor(); }));
  };

  const drawEditor = () => {
    const isNew = sel === '';
    const u0 = isNew ? { login: '', name: '', email: '', roles: { user: true }, access: { all: false, coverage: [], sources: [], clients: [] }, active: true, must_change: true } : data.users.find((u) => u.login === sel);
    if (!u0) { editEl.innerHTML = '<div class="ar-note">Pick a user to edit, or add one.</div>'; return; }
    const u = JSON.parse(JSON.stringify(u0));
    u.access = { all: true, coverage: [], sources: [], clients: [], ...(u.access || {}) };
    const self = u.login === data.me;
    editEl.innerHTML = `<div class="ad-form">
      <div class="ad-title"><span class="av lg">${esc(initials(u.name || u.login || '+'))}</span><div><h3>${isNew ? 'New user' : esc(u.name || u.login)}</h3><span>${isNew ? 'Fill in the details and save' : esc(u.login)}</span></div></div>
      <div class="ad-sec">Account</div>
      <div class="ad-grid">
        <label>Name<input name="name" value="${esc(u.name)}" placeholder="e.g. Anna Petrova"></label>
        <label>Login<input name="login" value="${esc(u.login)}" placeholder="e.g. apetrova" autocapitalize="off" spellcheck="false"></label>
        <label>E-mail<input name="email" value="${esc(u.email || '')}" placeholder="optional"></label>
        <label>${isNew ? 'Password' : 'New password'}<input name="password" type="text" autocomplete="off" placeholder="${isNew ? '4 characters or more' : 'leave empty to keep the current one'}"></label>
      </div>
      <div class="ad-toggles">
        <label class="tgl"><input type="checkbox" name="active"${u.active ? ' checked' : ''}${self ? ' disabled' : ''}><span class="track"></span><span>Can sign in</span></label>
        <label class="tgl"><input type="checkbox" name="must"${u.must_change ? ' checked' : ''}><span class="track"></span><span>Choose a new password at the next sign-in</span></label>
      </div>
      <div class="ad-sec">Roles <span class="muted">— Admin includes every other role</span></div>
      <div class="ad-roles">${ROLE_ORDER.map((r) => `<label class="ad-role" data-r="${r}"><input type="checkbox" name="r_${r}"><span class="ad-rbox">${icon('check', 13, 2.4)}</span><span class="ad-rtx"><b>${ROLE_INFO[r][0]}</b><span>${esc(ROLE_INFO[r][1])}</span></span><span class="ad-via">with Admin</span></label>`).join('')}</div>
      <div class="ad-sec">Client access</div>
      <div class="ad-access"></div>
      <div class="ad-sec">Groups <span class="muted">— reports can be given to a whole group</span></div>
      <div class="ad-groups">${grp.groups.map((g) => `<label class="ad-gchip${g.members.includes(u.login) ? ' on' : ''}" title="${esc(g.description || '')}"><input type="checkbox" value="${esc(g.id)}"${g.members.includes(u.login) ? ' checked' : ''}>${icon('users', 12)}<span>${esc(g.name)}</span></label>`).join('') || '<span class="muted">No groups yet — make them in Groups above.</span>'}</div>
      <div class="ad-sec">Reports <span class="muted">— which published reports they can use</span></div>
      <div class="ad-reps"></div>
      <div class="ad-sec">AI <span class="muted">— what AI assistants may send for this person</span></div>
      <div class="ad-ai"><select name="ai"><option value="">Not set — from their groups, else the bank</option>${AI_LEVELS.map(([v, l]) => `<option value="${v}"${(u.ai_level === 'full' ? 'aggregates' : u.ai_level) === v ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select><div class="hint ad-ai-now"></div></div>
      ${isNew ? '' : `<div class="ad-sec">AI keys <span class="muted">— their own keys; saved at once ${KEYS_AT} and never shown again</span></div>
      <div class="ad-keys"></div>`}
      <div class="ad-err" hidden></div>
      <div class="ad-acts">
        ${isNew || self ? '' : `<button class="btn danger-ghost" data-a="del">${icon('trash', 14)} Delete</button>`}
        <span class="grow"></span>
        <button class="btn" data-a="cancel">${isNew ? 'Cancel' : 'Revert'}</button>
        <button class="btn primary" data-a="save">${icon('save', 14)} ${isNew ? 'Add user' : 'Save'}</button>
      </div>
    </div>`;
    const $ = (n) => editEl.querySelector(`[name="${n}"]`);
    // roles: Admin marks every other role (and they can't be cleared while it's on)
    const syncRoles = () => {
      const adm = $('r_admin').checked;
      for (const r of ROLE_ORDER) {
        const box = $(`r_${r}`);
        if (r !== 'admin') {
          box.checked = adm || !!u.roles[r];
          box.disabled = adm;
          box.closest('.ad-role').classList.toggle('via', adm);
        }
        box.closest('.ad-role').classList.toggle('on', box.checked);
      }
      if (self) { $('r_admin').disabled = true; $('r_admin').closest('.ad-role').title = 'You can’t take the Admin role from yourself'; }
      drawAccess();
    };
    for (const r of ROLE_ORDER) {
      $(`r_${r}`).checked = !!u.roles[r];
      $(`r_${r}`).addEventListener('change', (e) => { u.roles[r] = e.target.checked; syncRoles(); });
    }
    // access: every client, or bankers' coverage + source systems + named clients
    const accEl = editEl.querySelector('.ad-access');
    const drawAccess = () => accessEditor(accEl, u, opts);
    syncRoles();
    // AI: what applies now and why (their setting, a group's, or the bank's)
    const aiNow = async () => {
      const box = editEl.querySelector('.ad-ai-now');
      if (!box) return;
      const mine = grp.groups.filter((x) => chosenGroups().includes(x.id));
      const [lvl, why] = aiFor($('ai').value, mine, await bankLevel());
      box.innerHTML = `${icon(lvl === 'structure' ? 'lock' : 'sparkles', 12)} <span>Now:</span> <b>${esc(aiLevelName(lvl))}</b> <span class="muted">(${esc(why)})</span> <span>${lvl === 'structure' ? 'The report assistant (brief, check, notes) is not available to them.' : 'The report assistant works for them.'}</span>`;
    };
    $('ai').addEventListener('change', aiNow);
    editEl.querySelectorAll('.ad-gchip input').forEach((x) => x.addEventListener('change', aiNow));
    // AI keys: the person's own, set or removed here at once (they can also do it in Settings → AI)
    const drawKeys = async () => {
      const box = editEl.querySelector('.ad-keys');
      if (!box) return;
      let list;
      try { list = await call('ai_user_keys', { login: u0.login }, { quiet: true }); } catch (e) { box.innerHTML = `<div class="hint">${esc(String(e))}</div>`; return; }
      box.innerHTML = list.map((p) => `<div class="ad-key" data-p="${esc(p.id)}">
        <b>${esc(p.name)}</b>
        <span class="ad-kst ${p.personal ? 'ok' : p.bank ? '' : 'none'}">${!p.allowed ? 'not allowed by the bank' : p.personal ? 'Own key' : p.bank ? 'Uses the bank’s key' : 'No key'}</span>
        <input type="password" autocomplete="off" spellcheck="false" placeholder="${p.personal ? '•••••••• saved — paste to replace' : p.gigachat ? 'Authorization key (Base64)' : 'Paste the API key'}">
        <button class="btn sm" data-k="save">Save key</button>
        ${p.personal ? `<button class="btn sm" data-k="del" title="Remove their own key">${icon('trash', 13)}</button>` : '<span></span>'}
      </div>`).join('');
      box.querySelectorAll('.ad-key').forEach((row) => {
        const id = row.dataset.p, name = row.querySelector('b').textContent, inp = row.querySelector('input');
        row.querySelector('[data-k="save"]').addEventListener('click', async () => {
          if (!inp.value.trim()) { inp.focus(); return; }
          try { await call('ai_set_key', { provider: id, key: inp.value.trim(), bank: false, user: u0.login }); toast(`Key saved: ${name} · ${u0.name || u0.login}`); drawKeys(); } catch {}
        });
        row.querySelector('[data-k="del"]')?.addEventListener('click', async () => {
          try { await call('ai_set_key', { provider: id, key: null, bank: false, user: u0.login }); toast(`Key removed: ${name} · ${u0.name || u0.login}`); drawKeys(); } catch {}
        });
      });
    };
    if (!isNew) drawKeys();
    const err = editEl.querySelector('.ad-err');
    // groups: ticked here, saved with the account
    editEl.querySelectorAll('.ad-gchip input').forEach((x) => x.addEventListener('change', () => { x.closest('.ad-gchip').classList.toggle('on', x.checked); drawReps(); }));
    const chosenGroups = () => [...editEl.querySelectorAll('.ad-gchip input:checked')].map((x) => x.value);
    aiNow();
    // reports: through everyone / a group, or given to this person (changed at once — it's the report's setting)
    const drawReps = () => {
      const box = editEl.querySelector('.ad-reps');
      const isUser = $('r_user').checked || $('r_admin').checked;
      if (u.roles.admin || $('r_admin').checked || $('r_design').checked) { box.innerHTML = `<div class="ar-note">${icon('shield', 14)} ${$('r_admin').checked ? 'Administrators' : 'Designers'} see every report.</div>`; return; }
      if (!isUser) { box.innerHTML = `<div class="ar-note">${icon('lock', 14)} Reports need the User role.</div>`; return; }
      const mine = chosenGroups();
      box.innerHTML = reps.map((r) => {
        const via = r.audience.everyone ? 'everyone with Reports' : grp.groups.filter((g) => r.audience.groups.includes(g.id) && mine.includes(g.id)).map((g) => g.name).join(', ');
        const named = r.audience.users.includes(u.login);
        const on = !!via || named;
        return `<label class="ad-rep${on ? ' on' : ''}${r.published ? '' : ' draft'}"><input type="checkbox" data-r="${esc(r.id)}"${on ? ' checked' : ''}${via || isNew ? ' disabled' : ''}><span class="ad-rbox">${icon('check', 13, 2.4)}</span><span class="ad-rtx"><b>${esc(r.title)}</b><span>${!r.published ? 'unpublished · ' : ''}${via ? `through ${esc(via)}` : named ? 'given to this person' : 'not given'}</span></span></label>`;
      }).join('') || '<div class="ar-note">No reports published yet.</div>';
      box.querySelectorAll('input[data-r]').forEach((x) => x.addEventListener('change', async () => {
        const r = reps.find((y) => y.id === x.dataset.r);
        const users = x.checked ? [...new Set([...r.audience.users, u.login])] : r.audience.users.filter((l) => l !== u.login);
        try { const out = await call('auth', { cmd: 'set_audience', args: { id: r.id, audience: { ...r.audience, users } } }); Object.assign(r, out.report); drawReps(); drawList(); } catch (e) { toast(String(e), 'error'); }
      }));
    };
    ['r_user', 'r_admin', 'r_design'].forEach((n) => $(n).addEventListener('change', drawReps));
    drawReps();
    editEl.querySelector('[data-a="cancel"]').addEventListener('click', () => { if (isNew) sel = null; drawList(); drawEditor(); });
    editEl.querySelector('[data-a="save"]').addEventListener('click', async () => {
      err.hidden = true;
      const account = {
        was: isNew ? '' : u0.login, login: $('login').value.trim(), name: $('name').value.trim(), email: $('email').value.trim(),
        password: $('password').value, active: $('active').checked, must_change: $('must').checked,
        roles: { admin: $('r_admin').checked, data: !!u.roles.data, design: !!u.roles.design, user: !!u.roles.user }, access: u.access,
        ai_level: $('ai').value,
      };
      try {
        data = await call('auth', { cmd: 'save_user', args: { account } }, { quiet: true });
        grp = { ...grp, groups: (await call('auth', { cmd: 'set_user_groups', args: { login: account.login, groups: chosenGroups() } }, { quiet: true })).groups };
        sel = account.login;
        toast(isNew ? `${account.name || account.login} added` : 'Saved');
        drawList(); drawEditor();
        if (account.was === data.me) render();
      } catch (e) { err.textContent = String(e); err.hidden = false; }
    });
    editEl.querySelector('[data-a="del"]')?.addEventListener('click', async () => {
      const ok = await modal({ title: 'Delete the user?', iconName: 'warn', iconKind: 'warn', body: `<p>${esc(u0.name || u0.login)} (${esc(u0.login)}) can no longer sign in. Their reports and templates stay where they were saved.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
      if (!ok) return;
      try { data = await call('auth', { cmd: 'delete_user', args: { login: u0.login } }, { quiet: true }); sel = null; drawList(); drawEditor(); toast('User deleted'); } catch (e) { toast(String(e), 'error', 3600); }
    });
    editEl.querySelector('[name=name]').addEventListener('input', (e) => { editEl.querySelector('.av.lg').textContent = initials(e.target.value || $('login').value || '+'); });
  };

  el.querySelector('[data-a="aiaccess"]').addEventListener('click', () => toggleAdminAI('access'));
  el.querySelector('[data-a="add"]').addEventListener('click', () => { sel = ''; drawList(); drawEditor(); setTimeout(() => editEl.querySelector('[name=name]')?.focus(), 30); });
  el.querySelector('.ad-find').addEventListener('input', drawList);
  try {
    [data, opts, grp, reps] = await Promise.all([call('auth', { cmd: 'users' }, { quiet: true }), call('auth', { cmd: 'options' }, { quiet: true }).catch((e) => { toast(String(e), 'error', 4000); return null; }),
      call('auth', { cmd: 'groups' }, { quiet: true }), call('auth', { cmd: 'reports' }, { quiet: true })]);
    learnProducts(opts?.sources);
  } catch (e) { listEl.innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
  sel = S.adminSel && data.users.some((u) => u.login === S.adminSel) ? S.adminSel : data.users[0]?.login ?? null;
  S.adminSel = null;
  drawList();
  drawEditor();
}
