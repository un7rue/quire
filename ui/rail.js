// The rail: the one way around the app, on the left of every screen.
//
//   top     the areas the person's roles open (Reports, Templates, Data, Admin)
//   middle  what is open: the document, and SQL workspaces
//   bottom  the guide, settings and the person's account
//
// Going to an area keeps the document open: it stays in the rail until
// another document replaces it.

import { S, api, render, isReport, isTheme } from './state.js';
import { icon } from './icons.js';
import { esc } from './util.js';
import { myAreas, currentArea, setArea, accountChip, session, initials } from './auth.js';
import { openViews, activeViewKey, showView, closeView } from './dbx.js';

/** What the stage shows: 'view' (a workspace), 'home' (an area) or 'doc'. */
export function stageMode() {
  if (activeViewKey()) return 'view';
  if (!S.st?.doc || S.showHome) return 'home';
  return 'doc';
}

/** Opens an area's page (the document stays open, in the rail). */
export function goArea(key) {
  if (key) setArea(key);
  S.showHome = true;
  showView(null);
  render();
}

/** Back to the open document. */
export function goDoc() {
  S.showHome = false;
  showView(null);
  render();
  window.dispatchEvent(new Event('resize'));
}

/** The area a document belongs to: templates and themes are designed, reports are for users. */
export const docArea = () => (isReport() ? 'user' : 'design');

const KIND = { template: 'Template', report: 'Report', theme: 'Theme' };

export function renderRail({ onGuide, onSettings }) {
  const rail = document.getElementById('rail');
  if (!rail) return;
  // the rail is redrawn: a card left from the old one goes
  docCard(null);
  if (!session()) { rail.hidden = true; return; }
  rail.hidden = false;
  const mode = stageMode();
  // a SQL workspace is part of the Data area: it shows as a tab there
  if (mode === 'view' && myAreas().some((a) => a.key === 'data')) setArea('data');
  const area = currentArea();
  const doc = S.st?.doc;
  const views = openViews();
  // every open document (up to 10): the shown one and the others
  const docs = S.st?.open_docs || (doc ? [{ id: 0, kind: isTheme() ? 'theme' : isReport() ? 'report' : 'template', title: isTheme() ? doc.theme.name : S.st.file || doc.meta.title || 'Untitled', dirty: S.st.dirty, active: true }] : []);
  rail.innerHTML = `
    <div class="rl-group">${myAreas().map((a) => `<button class="rl-item${(mode === 'home' && a.key === area) || (mode === 'view' && a.key === 'data') ? ' on' : ''}${mode === 'doc' && doc && a.key === docArea() ? ' ctx' : ''}" data-area="${a.key}" title="${esc(a.desc)}">${icon(a.icon, 20)}<span>${esc(a.label)}</span>${a.key === 'data' && views.length ? `<i class="rl-badge" title="${views.length} SQL workspace${views.length > 1 ? 's' : ''} open">${views.length}</i>` : ''}</button>`).join('')}</div>
    ${docs.length ? `<div class="rl-sep"><span>Open</span></div><div class="rl-group rl-open">
      ${docs.map((d) => `<button class="rl-item rl-doc k-${d.kind}${mode === 'doc' && d.active ? ' on' : ''}${d.active ? ' cur' : ''}" data-doc="${d.id}" aria-label="${esc(KIND[d.kind])} · ${esc(d.title)}">
        <span class="rl-card">${d.kind === 'report' && d.client ? `<b>${esc(initials(d.client))}</b>` : icon(d.kind === 'theme' ? 'palette' : d.kind === 'report' ? 'page' : 'layout', 18)}${d.dirty ? '<i class="rl-dirty" title="Unsaved changes"></i>' : ''}</span>
        <span class="rl-name">${esc(shortName(d))}</span>
        <i class="rl-x" data-close="${d.id}" title="Close">${icon('x', 10, 2.4)}</i></button>`).join('')}
    </div>` : ''}
    <span class="grow"></span>
    <div class="rl-group rl-bottom">
      <button class="rl-item sm" data-a="guide" title="User guide">${icon('book', 18)}</button>
      <button class="rl-item sm" data-a="settings" title="Settings (⌘,)">${icon('sliders', 18)}</button>
    </div>`;
  const chip = accountChip(goArea);
  if (chip) rail.append(chip);
  rail.querySelectorAll('[data-area]').forEach((b) => b.addEventListener('click', () => goArea(b.dataset.area)));
  rail.querySelectorAll('[data-doc]').forEach((b) => b.addEventListener('click', async (e) => {
    const d = docs.find((x) => String(x.id) === b.dataset.doc);
    if (!d) return;
    if (e.target.closest('[data-close]')) {
      e.stopPropagation();
      if (d.dirty) {
        const { modal } = await import('./ui.js');
        const ok = await modal({ title: 'Close without saving?', body: `<p>«${esc(d.title)}» has unsaved changes. They will be lost.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Close', kind: 'danger', value: true }] });
        if (!ok) return;
      }
      await api.closeDoc(d.id);
      return;
    }
    if (d.active) return goDoc();
    await api.activateDoc(d.id);
    goDoc();
  }));
  // the full name, client and period beside the rail, while the pointer is on a document
  rail.querySelectorAll('[data-doc]').forEach((b) => {
    const d = docs.find((x) => String(x.id) === b.dataset.doc);
    b.addEventListener('mouseenter', () => docCard(b, d));
    b.addEventListener('mouseleave', () => docCard(null));
    b.addEventListener('focus', () => docCard(b, d));
    b.addEventListener('blur', () => docCard(null));
  });
  applyAreaFold();
  rail.querySelector('[data-a="guide"]').addEventListener('click', onGuide);
  rail.querySelector('[data-a="settings"]').addEventListener('click', onSettings);
}

// under a document's card: who a report is for, else the document's own name
const shortName = (d) => (d.kind === 'report' && d.client ? d.client : d.title || KIND[d.kind]);

/** A card beside the rail with what a document is: kind, full name, client, period, unsaved. */
function docCard(btn, d) {
  let card = document.getElementById('rl-card');
  if (!btn || !d) { card?.remove(); return; }
  if (!card) { card = document.createElement('div'); card.id = 'rl-card'; document.body.append(card); }
  card.className = `rl-pop k-${d.kind}`;
  card.innerHTML = `<div class="rp-kind">${icon(d.kind === 'theme' ? 'palette' : d.kind === 'report' ? 'page' : 'layout', 12)} ${esc(KIND[d.kind])}${d.active ? ' · <span>in front</span>' : ''}</div>
    <b class="rp-title">${esc(d.title || KIND[d.kind])}</b>
    ${d.client ? `<div class="rp-row">${icon('user', 12)} ${esc(d.client)}</div>` : ''}
    ${d.period ? `<div class="rp-row">${icon('calendar', 12)} ${esc(d.period)}</div>` : ''}
    ${d.dirty ? `<div class="rp-row warn"><i class="rl-dot"></i> Unsaved changes</div>` : ''}
    ${d.link ? `<div class="rp-row">${icon('link', 12)} Linked with its theme studio</div>` : ''}`;
  const r = btn.getBoundingClientRect();
  card.style.left = `${r.right + 8}px`;
  card.style.top = `${Math.max(8, Math.min(window.innerHeight - card.offsetHeight - 8, r.top + r.height / 2 - card.offsetHeight / 2))}px`;
}

// ---------------------------------------------------------------- folding an area's left part (⌘[)

const FOLD_KEY = (a) => `quire.fold.${a}`;
const folded = (a) => { try { return localStorage.getItem(FOLD_KEY(a)) === '1'; } catch { return false; } };

/**
 * Folds or unfolds the left part of an area's page (remembered per area): Admin's menu shrinks to
 * its icons, the Data area's list and the Reports area's report picker step aside. The sidebar stays.
 */
export function toggleAreaFold(area = currentArea()) {
  try { localStorage.setItem(FOLD_KEY(area), folded(area) ? '0' : '1'); } catch {}
  applyAreaFold();
}

/** Puts the area in front's fold on the page (each render). */
export function applyAreaFold() {
  for (const a of ['admin', 'data', 'user']) document.body.classList.toggle(`fold-${a}`, folded(a));
}

/** A short name for the title bar of an area's page. */
export const areaCrumb = () => myAreas().find((a) => a.key === currentArea());


/**
 * The Data area's tabs: its overview and the open SQL workspaces, above
 * whichever of them shows. Only while in the Data area and something is open.
 */
export function renderDataTabs() {
  const stage = document.getElementById('stage');
  if (!stage) return;
  let bar = document.getElementById('datatabs');
  if (!bar) {
    bar = document.createElement('div');
    bar.id = 'datatabs';
    stage.insertBefore(bar, stage.firstChild);
  }
  const views = openViews();
  const mode = stageMode();
  const show = !!session() && views.length > 0 && (mode === 'view' || (mode === 'home' && currentArea() === 'data'));
  bar.hidden = !show;
  if (!show) return;
  bar.innerHTML = `<button class="dt-tab${mode === 'home' ? ' on' : ''}" data-v="">${icon('database', 14)}<span>Overview</span></button>
    ${views.map((v) => `<button class="dt-tab${v.active ? ' on' : ''}" data-v="${esc(v.key)}" title="${esc(v.tip)}">${icon('code', 14)}<span>${esc(v.title)}</span><i class="dt-x" title="Close">${icon('x', 11, 2.2)}</i></button>`).join('')}`;
  bar.querySelectorAll('.dt-tab').forEach((b) => b.addEventListener('click', (e) => {
    const k = b.dataset.v;
    // closing a tab stays in the Data area
    if (e.target.closest('.dt-x')) { S.showHome = true; return closeView(k); }
    if (!k) goArea('data');
    else showView(k);
  }));
}
