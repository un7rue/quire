// What every AI pane shares: the chip that shows (and switches) which AI answers,
// and conversations kept on this computer.

import { call } from './state.js';
import { icon } from './icons.js';
import { esc } from './util.js';
import { menu, toast } from './ui.js';

const KEEP_CHATS = 30, KEEP_MSGS = 80;

/** Conversations under one key (newest first). */
export const chatStore = (key) => ({
  load() { try { return JSON.parse(localStorage.getItem(key) || '[]'); } catch { return []; } },
  save(chats) {
    for (const c of chats) c.msgs = c.msgs.slice(-KEEP_MSGS);
    try { localStorage.setItem(key, JSON.stringify(chats.slice(0, KEEP_CHATS))); } catch {}
  },
});

export const when = (t) => new Date(t).toLocaleString(undefined, { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' });

const PREF = 'quire.ai.provider';
const pref = { get: () => { try { return localStorage.getItem(PREF) || ''; } catch { return ''; } }, set: (v) => { try { localStorage.setItem(PREF, v); } catch {} } };

/**
 * The provider chip in a pane's header: shows the provider and model that answer; click to
 * switch among those with a key, or to open Settings → AI. Answers `{ current, load }`.
 */
export function providerChip(btn) {
  let status = null;
  const usable = () => (status?.providers || []).filter((p) => p.allowed && (p.personal || p.bank));
  const current = () => {
    const list = usable();
    const want = pref.get() || status?.settings?.default || '';
    return list.find((p) => p.id === want) || list[0] || null;
  };
  const modelOf = (p) => (status?.settings?.providers?.[p.id]?.model || '').trim() || p.model;
  const paint = () => {
    const p = current();
    btn.innerHTML = p ? `<span>${esc(p.name)}</span><i>${esc(modelOf(p))}</i>${icon('chevd', 11)}` : `<span>No AI set up</span>${icon('chevd', 11)}`;
    btn.classList.toggle('none', !p);
  };
  const load = async () => {
    try { status = await call('ai_status', {}, { quiet: true }); } catch { status = null; }
    paint();
  };
  btn.addEventListener('click', () => {
    const list = usable();
    menu(btn, [
      ...list.map((p) => ({ label: `${p.name} · ${modelOf(p)}`, icon: current()?.id === p.id ? 'ok' : 'sparkles', desc: p.personal ? 'Your key' : 'The bank’s key', run: () => { pref.set(p.id); paint(); } })),
      ...(list.length ? [{ sep: true }] : []),
      { label: list.length ? 'Keys and models…' : 'Set up AI in Settings…', icon: 'sliders', run: () => document.dispatchEvent(new CustomEvent('open-settings', { detail: 'ai' })) },
    ]);
  });
  // a key added in Settings shows here at once
  document.addEventListener('settings-saved', load);
  return { current, load, loaded: () => !!status };
}

/** The pane's history button: earlier conversations, and clearing them. */
export function historyMenu(anchor, store, currentId, pick, clear) {
  const all = store.load();
  const asked = (c) => c.msgs.filter((m) => m.role === 'user').length;
  menu(anchor, all.length ? [
    ...all.slice(0, 15).map((c) => ({ label: c.title || 'Conversation', desc: `${when(c.at)} · ${asked(c)} question${asked(c) === 1 ? '' : 's'}`, icon: currentId === c.id ? 'ok' : 'chat', run: () => pick(c) })),
    { sep: true },
    { label: 'Clear all conversations', icon: 'trash', run: () => { store.save([]); clear(); } },
  ] : [{ label: 'No earlier conversations', disabled: true }], { align: 'end' });
}


// ---------------------------------------------------------------- 👍 / 👎 on an answer

// what this person said about each answer (by its request's log time), to keep the buttons lit
const VOTES = 'quire.ai.votes';
const votes = () => { try { return JSON.parse(localStorage.getItem(VOTES) || '{}'); } catch { return {}; } };
const keepVote = (at, v) => { try { const all = votes(); if (v) all[at] = v; else delete all[at]; localStorage.setItem(VOTES, JSON.stringify(all)); } catch {} };

/** Thumbs up / down for an answer, to put on its footer line (`at`: when its request was logged). */
export function feedbackHtml(at) {
  if (!at) return '';
  const v = votes()[at];
  return `<span class="aip-fb" data-fb="${esc(at)}"><button data-v="up" class="${v === 'up' ? 'on' : ''}" title="This helped">${icon('thumbs-up', 13)}</button><button data-v="down" class="${v === 'down' ? 'on' : ''}" title="This didn’t help — tell us why">${icon('thumbs-down', 13)}</button></span>`;
}

const REASONS = ['Wrong or made up', 'Not what I asked', 'Not useful', 'Too slow'];

async function sendVote(at, vote, reasons = [], comment = '') {
  try {
    await call('ai_feedback', { at, vote, reasons, comment }, { quiet: true });
    keepVote(at, vote);
    document.querySelectorAll(`.aip-fb[data-fb="${CSS.escape(at)}"] button`).forEach((b) => b.classList.toggle('on', b.dataset.v === vote));
    toast(vote ? 'Thanks — your administrator sees this.' : 'Vote taken back.', 'info', 1600);
  } catch (e) { toast(String(e), 'error'); }
}

// one handler for every assistant's answers
document.addEventListener('click', (e) => {
  const b = e.target.closest('.aip-fb button');
  if (!b) return;
  e.stopPropagation();
  const box = b.closest('.aip-fb'), at = box.dataset.fb, was = votes()[at];
  const msg = box.closest('.aip-msg') || box.parentElement;
  msg.querySelector('.aip-fbform')?.remove();
  // the same thumb again takes the vote back
  if (was === b.dataset.v) { sendVote(at, ''); return; }
  if (b.dataset.v === 'up') { sendVote(at, 'up'); return; }
  // not helpful: what went wrong (optional), then send
  const form = document.createElement('div');
  form.className = 'aip-fbform';
  form.innerHTML = `<b>What went wrong?</b><div class="aip-fbr">${REASONS.map((r) => `<button data-r="${esc(r)}">${esc(r)}</button>`).join('')}</div>
    <textarea rows="2" placeholder="Anything else (optional) — what you expected"></textarea>
    <div class="aip-acts"><span class="grow"></span><button class="btn sm" data-a="cancel">Cancel</button><button class="btn sm primary" data-a="send">${icon('thumbs-down', 12)} Send</button></div>`;
  msg.append(form);
  form.querySelectorAll('[data-r]').forEach((x) => x.addEventListener('click', () => x.classList.toggle('on')));
  const ta = form.querySelector('textarea');
  ta.addEventListener('keydown', (ev) => ev.stopPropagation());
  form.querySelector('[data-a="cancel"]').addEventListener('click', () => form.remove());
  form.querySelector('[data-a="send"]').addEventListener('click', async () => {
    await sendVote(at, 'down', [...form.querySelectorAll('[data-r].on')].map((x) => x.dataset.r), ta.value.trim());
    form.remove();
  });
  ta.focus();
}, true);
