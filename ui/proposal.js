// The banker's proposal: their recommendations to the client as a page of the report, after the
// facts and before the legal text. Typed or pasted, or read from a Word / PowerPoint file; laid
// out as written, or written up by the AI from the notes and the report's figures (masked).
// The brief, the pitch and the check read the page like any other.

import { S, call, edit, setState } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { toast, modal, menu } from './ui.js';
import { appLanguage } from './i18n.js';
import { scrollToPage } from './canvas.js';
import { rich } from './brief.js';

// a report may hold several proposals: their pages' group is "PROPOSAL:<id>"
const PREFIX = 'PROPOSAL:';
/** The report's proposals, in page order: { id, title, page (its first page's id), notes }. */
export function proposals() {
  const out = [];
  for (const p of S.st?.doc?.pages || []) {
    if (!String(p.group || '').startsWith(PREFIX)) continue;
    const id = p.group.slice(PREFIX.length);
    if (out.some((x) => x.id === id)) continue;
    const notes = p.widgets?.find((w) => w.opts?.proposalNotes != null)?.opts.proposalNotes || '';
    out.push({ id, title: p.description || p.title, page: p.id, notes });
  }
  return out;
}
export const hasProposal = () => proposals().length > 0;

/** The toolbar's button: a new proposal straight away, or (with some already) a list to open one or add another. */
export function proposalMenu(anchor) {
  const list = proposals();
  if (!list.length) return proposalDialog();
  menu(anchor, [
    { title: list.length === 1 ? 'Proposal in this report' : `${list.length} proposals in this report` },
    ...list.map((p) => ({ label: p.title, icon: 'briefcase', desc: 'Open to change or remove', run: () => proposalDialog({ id: p.id, notes: p.notes }) })),
    { sep: true },
    { label: 'New proposal…', icon: 'plus', desc: 'Another proposal, after the ones there are', run: () => proposalDialog() },
  ], { align: 'end' });
}

const MODE_KEY = 'quire.proposal.mode';

// ---- the register: every proposal made, its status and history (bankers: their clients'; admins: all)

/** A proposal's statuses, in their usual order. */
export const STATUSES = [['draft', 'Draft'], ['presented', 'Presented'], ['accepted', 'Accepted'], ['partly', 'Partly accepted'], ['declined', 'Declined'], ['withdrawn', 'Withdrawn']];
export const statusName = (s) => STATUSES.find((x) => x[0] === s)?.[1] || s || '';
export const statusPill = (s) => `<span class="pr-st" data-s="${esc(s || '')}">${esc(statusName(s))}</span>`;
const actDir = (a) => (/^продл|^renew|^extend/i.test(a || '') ? '' : /^(куп|докуп|откр|увелич|влож|размест|реинв|пополн|buy|add|open|invest)/i.test(a || '') ? 'in' : /^(прод|сократ|закр|зафикс|вывест|sell|reduce|close|trim)/i.test(a || '') ? 'out' : '');

/** What a proposal says, read-only: title, lead, actions (buying green, selling red), effect, reasons, risks, steps. */
export function proposalBodyHtml(c = {}, { title = true } = {}) {
  const list = (head, items, cls) => ((items || []).length ? `<div class="pr-l ${cls}"><b>${esc(head)}</b><ul>${items.map((x) => `<li>${rich(String(x))}</li>`).join('')}</ul></div>` : '');
  return `<div class="pr-body">
    ${title && c.title ? `<div class="pr-t">${esc(c.title)}</div>` : ''}
    ${c.lead ? `<p class="pr-lead">${rich(c.lead)}</p>` : ''}
    ${(c.actions || []).length ? `<div class="pr-acts">${c.actions.map((a) => `<div class="pr-a"><span class="aip-pill ${actDir(a.action)}">${esc(a.action || '')}</span><span><span><b>${esc(String(a.what || '').replace(/\*\*/g, ''))}</b>${a.amount ? ` · ${esc(a.amount)}` : ''}</span>${a.why ? `<i>${esc(String(a.why).replace(/\*\*/g, ''))}</i>` : ''}</span></div>`).join('')}</div>` : ''}
    <div class="pr-ls">${list('Expected effect', c.effect, 'good')}${list('Why now', c.reasons, '')}${list('Risks', c.risks, 'bad')}${list('Next steps', c.steps, '')}</div>
  </div>`;
}

/** Changes a proposal's status (with an optional note: what the client said). `done(entry)` after. */
export function statusMenu(anchor, entry, done = () => {}) {
  menu(anchor, [{ title: 'Set the status' }, ...STATUSES.filter(([k]) => k !== entry.status).map(([k, label]) => ({ label, run: async () => {
    const body = h(`<div class="modal-body"><p class="muted">What the client said, or why — kept in the proposal's history (optional).</p><textarea class="pr-note" rows="3" placeholder="e.g. Agreed to the bonds; will think about the deposit"></textarea></div>`);
    body.querySelector('textarea').addEventListener('keydown', (e) => e.stopPropagation());
    const note = await modal({ title: `${statusName(k)} · ${entry.title || entry.content?.title || ''}`, iconName: 'briefcase', body,
      onOpen: () => body.querySelector('textarea').focus(),
      buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: (box) => box.querySelector('.pr-note').value.trim() }] });
    if (note == null) return;
    try { const r = await call('proposal_status', { uid: entry.uid, status: k, note }, { quiet: true }); toast(`Status: ${statusName(k)}.`, 'info', 1800); done(r); } catch (e) { toast(String(e), 'error'); }
  } }))]);
}

/** The client of the report in front (as the toolbar shows it). */
const reportClient = () => {
  const d = S.st?.doc;
  const def = (d?.param_defs || []).find((x) => /^(client|client_id|client_name|клиент)$/i.test(x.name));
  return def ? d.params?.values?.[def.name] || '' : '';
};

/** A proposal as notes in the dialog's own format (title, lead, actions, then the headed lists), to edit it. */
export function proposalNotes(r = {}) {
  const ru = (S.st?.doc?.theme?.locale || 'ru').startsWith('ru');
  const act = (a) => `- ${[a.action, a.what, a.amount ? (ru ? `на ${a.amount}` : `for ${a.amount}`) : ''].filter(Boolean).join(' ')}${a.why ? ` — ${a.why}` : ''}`;
  const block = (head, items) => ((items || []).length ? [`${head}:`, ...items.map((x) => `- ${x}`), ''] : []);
  return [r.title || '', '', r.lead || '', '', ...(r.actions || []).map(act), '',
    ...block(ru ? 'Ожидаемый эффект' : 'Expected effect', r.effect), ...block(ru ? 'Почему сейчас' : 'Why now', r.reasons),
    ...block(ru ? 'Риски' : 'Risks', r.risks), ...block(ru ? 'Следующие шаги' : 'Next steps', r.steps)].join('\n').replace(/\n{3,}/g, '\n\n').trim();
}

/** Opens the proposal dialog; makes, changes or removes the page. */
export async function proposalDialog({ id = null, notes = '' } = {}) {
  if (S.st?.doc?.kind !== 'report') { toast('A proposal goes into a report.', 'info'); return; }
  let mode = 'ai';
  try { mode = localStorage.getItem(MODE_KEY) || 'ai'; } catch {}
  // an existing proposal is changed in place; without an id a new one is added
  const exists = !!id && proposals().some((p) => p.id === id);
  const body = h(`<div class="modal-body pr-dlg">
      <p class="muted">Your recommendations to the client become a page of the report, after the facts and before the legal text. The brief, the pitch and the check include it.</p>
      <div class="pr-src">
        <label class="btn sm">${icon('upload', 13)} Upload a file…<input type="file" accept=".docx,.pptx,.txt,.md" hidden></label>
        <span class="muted pr-file">Word (.docx), PowerPoint (.pptx) or text — or type below</span>
        <span class="grow"></span>
        <button class="btn sm" data-a="suggest" title="The AI drafts a proposal from what the report shows (risk profile, maturities, idle cash…), building on anything you have typed. You review it before making the page.">${icon('sparkles', 13)} Suggest from the report</button>
      </div>
      <div class="pr-ai muted" hidden></div>
      <details class="pr-earlier" hidden><summary>${icon('clock', 12)} <span>Earlier proposals for this client</span> <span class="muted pr-en"></span></summary><div class="pr-elist"></div></details>
      <textarea class="pr-notes" rows="12" placeholder="${esc('Снижение доли акций\n\nПредлагаем вернуть портфель к консервативному профилю.\n\n- Продать Банк ВТБ на 2 млн ₽ — доля акций выше профиля\n- Купить ОФЗ 26238 на 5 млн ₽\n\nРиски:\n- Ставка может снизиться\n\nСледующие шаги:\n- Встреча 5 ноября')}">${esc(notes)}</textarea>
      <div class="pr-modes">
        <label class="pr-mode"><input type="radio" name="pr-mode" value="ai"${mode === 'ai' ? ' checked' : ''}><span class="pr-mi">${icon('sparkles', 15)}</span>
          <span><b>Write it up with AI</b><span>A clear page from your notes, tied to the report’s figures. Your actions and amounts stay as you wrote them. Names and account numbers go as placeholders.</span></span></label>
        <label class="pr-mode"><input type="radio" name="pr-mode" value="plain"${mode !== 'ai' ? ' checked' : ''}><span class="pr-mi">${icon('page', 15)}</span>
          <span><b>Lay out as written</b><span>No AI. The first line is the title, list items are the actions; lines “Риски:”, “Следующие шаги:”, “Почему:” start their blocks.</span></span></label>
      </div>
    </div>`);
  const ta = body.querySelector('.pr-notes');
  // the client's earlier proposals (the register): to look back at, or start from
  const client = reportClient();
  const myUid = id ? (S.st?.doc?.pages || []).find((p) => p.group === `PROPOSAL:${id}`)?.widgets?.find((w) => w.opts?.proposalUid)?.opts.proposalUid : null;
  if (client) call('proposals_list', { client }, { quiet: true }).then((list) => {
    const mine = list.find((r) => r.uid === myUid);
    if (mine) {
      const st = h(`<div class="pr-mine">Status ${statusPill(mine.status)} <button class="btn sm" data-a="st">Change…</button><span class="muted">${esc(new Date(mine.updated).toLocaleDateString())}${(mine.versions || []).length ? ` · ${mine.versions.length} earlier version${mine.versions.length === 1 ? '' : 's'}` : ''}</span></div>`);
      st.querySelector('[data-a="st"]').addEventListener('click', (e) => statusMenu(e.currentTarget, mine, (r) => { st.querySelector('.pr-st').outerHTML = statusPill(r.status); mine.status = r.status; }));
      body.querySelector('.pr-src').before(st);
    }
    const others = list.filter((r) => r.uid !== myUid);
    if (!others.length) return;
    const box = body.querySelector('.pr-earlier');
    box.hidden = false;
    box.querySelector('.pr-en').textContent = others.length;
    box.querySelector('.pr-elist').innerHTML = others.slice(0, 10).map((r, i) => `<div class="pr-e"><div class="pr-eh">${statusPill(r.status)}<b>${esc(r.content?.title || '')}</b><span class="muted">${esc(new Date(r.created).toLocaleDateString())} · ${esc(r.banker || r.created_by || '')}</span><span class="grow"></span><button class="btn sm" data-i="${i}" title="Put its notes in the box above, to start from">Start from it</button></div>
      ${(r.history || []).filter((x) => x.note).slice(-1).map((x) => `<div class="muted pr-enote">“${esc(x.note)}”</div>`).join('')}</div>`).join('');
    box.querySelectorAll('[data-i]').forEach((b) => b.addEventListener('click', () => { const r = others[+b.dataset.i]; ta.value = r.notes || proposalNotes(r.content || {}); toast('Notes from the earlier proposal — change what you need.', 'info', 2200); }));
  }).catch(() => {});
  ta.addEventListener('keydown', (e) => { if (!(e.metaKey || e.ctrlKey)) e.stopPropagation(); });
  const read = async (file) => {
    const status = body.querySelector('.pr-file');
    status.textContent = `Reading ${file.name}…`;
    try {
      const buf = new Uint8Array(await file.arrayBuffer());
      let bin = '';
      for (let i = 0; i < buf.length; i += 0x8000) bin += String.fromCharCode(...buf.subarray(i, i + 0x8000));
      const text = await call('proposal_read', { name: file.name, data: btoa(bin) }, { quiet: true });
      ta.value = ta.value.trim() ? `${ta.value.trim()}\n\n${text}` : text;
      status.textContent = `${file.name} · read`;
    } catch (e) { status.textContent = String(e); status.classList.add('bad'); }
  };
  // a draft from the report's figures: it replaces the notes (what was typed goes in as the focus)
  body.querySelector('[data-a="suggest"]').addEventListener('click', async (e) => {
    const b = e.currentTarget, note = body.querySelector('.pr-ai');
    const own = ta.value.trim();
    b.disabled = true;
    note.hidden = false;
    note.classList.remove('bad');
    note.innerHTML = `<span class="spin"></span> Reading the report…`;
    try {
      const r = await call('ai_report', { kind: 'proposal', page: null, prompt: own ? `Build on the banker's own notes (keep their actions and amounts):\n${own}` : '', provider: null, history: [], lang: appLanguage() }, { quiet: true });
      ta.value = proposalNotes(r);
      note.innerHTML = `${icon('sparkles', 12)} <span></span>`;
      note.querySelector('span').textContent = r.explanation || 'A draft from the report — review it before making the page.';
      // the draft is written already: lay it out as it is (no second pass through the AI)
      body.querySelector('[name=pr-mode][value=plain]').checked = true;
    } catch (err) {
      note.classList.add('bad');
      note.textContent = String(err);
    } finally { b.disabled = false; }
  });
  body.querySelector('input[type=file]').addEventListener('change', (e) => { const f = e.target.files?.[0]; if (f) read(f); });
  ta.addEventListener('dragover', (e) => e.preventDefault());
  ta.addEventListener('drop', (e) => { const f = e.dataTransfer?.files?.[0]; if (f) { e.preventDefault(); read(f); } });

  const pick = await modal({ title: exists ? `Proposal · ${proposals().find((p) => p.id === id)?.title || ''}` : hasProposal() ? 'New proposal' : 'Add a proposal', iconName: 'briefcase', wide: true, body,
    onOpen: () => ta.focus(),
    buttons: [
      ...(exists ? [{ label: 'Remove the page', kind: 'danger-ghost', value: 'remove' }] : []),
      { label: 'Cancel', value: null },
      { label: exists ? 'Update the page' : 'Make the page', kind: 'primary', value: (box) => ({ notes: box.querySelector('.pr-notes').value, mode: box.querySelector('[name=pr-mode]:checked')?.value || 'ai' }) },
    ] });
  if (!pick) return;
  if (pick === 'remove') {
    const ok = await modal({ title: 'Remove the proposal from the report?', iconName: 'trash', iconKind: 'danger', body: '<p>Its pages go from the report. Quire keeps the proposal in the client’s list of proposals (with its status and history). Undo (⌘Z) brings the pages back.</p>',
      buttons: [{ label: 'Cancel', value: false }, { label: 'Remove', kind: 'danger', value: true }] });
    if (ok) { try { setState(await call('proposal_remove', { id })); toast('Proposal removed from the report. Quire keeps it in the client’s proposals.', 'info', 3200); } catch (e) { toast(String(e), 'error'); } }
    return;
  }
  if (!pick.notes.trim()) { toast('Write or upload the proposal first.', 'info'); return proposalDialog({ id, notes: pick.notes }); }
  try { localStorage.setItem(MODE_KEY, pick.mode); } catch {}
  const wait = pick.mode === 'ai' ? toast('Writing the proposal page…', 'info', 60000) : null;
  try {
    const r = await call('proposal_make', { id: exists ? id : null, notes: pick.notes, mode: pick.mode, provider: null, lang: appLanguage() }, { quiet: true });
    wait?.remove?.();
    setState(r.st);
    const pi = (S.st.doc.pages || []).findIndex((p) => p.id === r.page);
    if (pi >= 0) { S.page = pi; scrollToPage(pi); }
    toast(r.explanation || (exists ? 'Proposal page updated.' : 'Proposal page added before the legal text.'), 'info', r.explanation ? 9000 : 3000);
  } catch (e) {
    wait?.remove?.();
    toast(String(e), 'error', 8000);
    // the notes stay: try again, or lay them out as written
    return proposalDialog({ id, notes: pick.notes });
  }
}
