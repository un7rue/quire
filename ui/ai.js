// The one place AI lives for everyone: a button at the right of the top bar
// (⌘I). Each screen with an assistant registers it here; the button opens the
// one in front. Where a role has none yet, it says what there is and what comes.

import { call } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { popover, closeLayer } from './ui.js';
import { hasRole } from './auth.js';

const TARGETS = new Map(); // view key → assistant ({ toggle, open, isOpen })

export const registerAI = (key, assistant) => { TARGETS.set(key, assistant); paintAIButton(); };
export const unregisterAI = (key) => { TARGETS.delete(key); paintAIButton(); };

let activeKey = () => null;
/** How the app tells which view is in front (the SQL workspaces' keys). */
export const setActiveView = (fn) => { activeKey = fn; };

// a workspace in front has its own; otherwise the open document's (the designer's), where it applies
const current = () => {
  const a = TARGETS.get(activeKey());
  if (a) return a;
  // the open document's: the design assistant for a template, the report assistant for a report
  if (where().mode !== 'doc') return null;
  return [...TARGETS.entries()].filter(([k]) => k.startsWith('doc:')).map(([, t]) => t).find((t) => !t.available || t.available()) || null;
};

// what the button does on an area's page (the Data area opens a workspace with the assistant)
const AREA_OPENERS = new Map();
export const setAreaAI = (area, open) => { if (open) AREA_OPENERS.set(area, open); else AREA_OPENERS.delete(area); };
let where = () => ({});
/** How the app tells where the person is: { mode: 'home' | 'doc' | 'view', area }. */
export const setWhere = (fn) => { where = fn; };

/** The top bar's button, always at the far right. */
export const aiButtonHtml = () => `<button class="tb-ai${current()?.isOpen() ? ' on' : ''}" id="ai-btn" title="AI assistant (⌘I)">${icon('sparkles', 15)}<span>AI</span></button>`;

export function paintAIButton() {
  document.getElementById('ai-btn')?.classList.toggle('on', !!current()?.isOpen());
}

/** The button or ⌘I: the assistant of the screen in front, or what AI offers here. */
export async function toggleAI() {
  // pressed again: whatever AI shows closes
  if (document.querySelector('.ai-about')) { closeLayer(); return; }
  const a = current();
  if (a) { a.toggle(); paintAIButton(); return; }
  const w = where();
  const opener = w.mode === 'home' && AREA_OPENERS.get(w.area);
  if (opener) {
    const r = await opener();
    // an area that opens its own assistant (Admin's drawer)
    if (r === 'handled') return;
    if (r) { current()?.open(); paintAIButton(); return; }
  }
  const btn = document.getElementById('ai-btn');
  if (btn) aboutAI(btn);
}

// the assistants by role: what's there today, what comes next
const ROLES = [
  { role: 'data', icon: 'database', name: 'Data', what: 'Write, fix and explain SQL, turn queries into report sources, answer questions about the database', ready: true, where: 'In any SQL workspace: Data → a source → Edit query, or a connection’s SQL.' },
  { role: 'design', icon: 'brush', name: 'Design', what: 'Build a page from a description, change the page in front, restyle the template after a picture', ready: true, where: 'In an open template: ⌘I or the AI button.' },
  { role: 'user', icon: 'page', name: 'Reports', what: 'Brief a report and how to present it, check it before sending, draft a note for a page', ready: true, where: 'In an open report: ⌘I or the AI button.' },
  { role: 'admin', icon: 'shield', name: 'Admin', what: 'Ask the activity log, explain a failure, change access in plain words, review who can do what', ready: true, where: 'In the Admin area: ⌘I or the AI button; Explain on a failed event; Review on the Access page.' },
];

async function aboutAI(anchor) {
  const el = h(`<div class="menu ai-about">
    <div class="ai-ab-head">${icon('sparkles', 16)}<b>AI assistant</b><span class="ai-ab-model">…</span></div>
    <div class="ai-ab-list">${ROLES.filter((r) => hasRole(r.role)).map((r) => `<div class="ai-ab-row${r.ready ? '' : ' soon'}">
      <span class="ai-ab-ic">${icon(r.icon, 15)}</span>
      <span class="ai-ab-t"><b>${esc(r.name)}</b><span>${esc(r.what)}</span>${r.ready ? `<em>${esc(r.where)}</em>` : ''}</span>
      <span class="ai-ab-badge">${r.ready ? 'Available' : 'Coming'}</span></div>`).join('') || '<div class="hint">Your roles have no assistant yet.</div>'}</div>
    <div class="ai-ab-foot">${hasRole('data') || hasRole('design') ? `<button class="btn sm" data-a="data">${icon('database', 13)} Go to Data</button>` : ''}<span class="grow"></span><button class="btn sm" data-a="settings">${icon('sliders', 13)} AI settings…</button></div>
  </div>`);
  el.querySelector('[data-a="settings"]').addEventListener('click', () => { closeLayer(); document.dispatchEvent(new CustomEvent('open-settings', { detail: 'ai' })); });
  el.querySelector('[data-a="data"]')?.addEventListener('click', () => { closeLayer(); document.dispatchEvent(new CustomEvent('go-area', { detail: 'data' })); });
  popover(anchor, el, { align: 'end' });
  // which AI answers, from Settings
  try {
    const st = await call('ai_status', {}, { quiet: true });
    const usable = st.providers.filter((p) => p.allowed && (p.personal || p.bank));
    const pick = usable.find((p) => p.id === (localStorage.getItem('quire.ai.provider') || st.settings?.default)) || usable[0];
    const model = pick ? (st.settings?.providers?.[pick.id]?.model || '').trim() || pick.model : '';
    el.querySelector('.ai-ab-model').innerHTML = st.policy?.off ? 'Switched off by the bank' : pick ? `${esc(pick.name)} · ${esc(model)}` : 'No key yet';
  } catch { el.querySelector('.ai-ab-model').textContent = ''; }
}
