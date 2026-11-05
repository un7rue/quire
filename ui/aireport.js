// The report user's AI pane, on the right of the report: a brief of the report with the points
// to pay attention to and a pitch for presenting it to the client; a check before sending; and
// a draft note for the page in front, which the banker reviews before adding it. The model reads
// the figures the report shows, with the client and account codes masked, and only when the
// bank allows figures to leave (Settings → AI assistant → Data that may be sent).

import { openUrl } from './platform.js';
import { S, call, edit, onChange, setState } from './state.js';
import { icon } from './icons.js';
import { esc } from './util.js';
import { toast, modal, menu } from './ui.js';
import { appLanguage } from './i18n.js';
import { chatStore, providerChip, historyMenu, feedbackHtml } from './aichat.js';
import { currentPageId, newWidget } from './panels.js';
import { scrollToPage } from './canvas.js';
import { rich, briefEmail, copyHtml, briefPages, PDF_SIZE, clientEmail } from './brief.js';
import { session } from './auth.js';
import { proposalDialog, proposalNotes, proposals } from './proposal.js';

const TASKS = [
  ['brief', 'sparkles', 'Brief & pitch', 'What stands out, what to watch, and how to present the report to the client'],
  ['check', 'shield', 'Check before sending', 'Contradictions, empty parts, striking figures without a note, risky wording'],
  ['note', 'comment', 'Note for this page', 'A short note to the client about the page in front, for you to review'],
  ['proposal', 'briefcase', 'Proposal ideas', 'A draft proposal from what the report shows — risk profile, maturities, idle cash — for you to review, then add as a page'],
];

/** Mounts the pane into `pane`. `onToggle(open)` shows its splitter. */
export function reportAssistant({ pane, onToggle = () => {} }) {
  const chats = chatStore('quire.ai.chats.report');
  let all = chats.load();
  let chat = null;
  let busy = false;

  pane.innerHTML = `
    <div class="aip-head">
      <span class="aip-ic">${icon('sparkles', 15)}</span><b>AI assistant</b>
      <button class="aip-model" title="Which AI answers — click to change">…</button>
      <span class="grow"></span>
      <button class="tb-btn" data-a="history" title="Earlier conversations">${icon('clock', 15)}</button>
      <button class="tb-btn" data-a="new" title="New conversation">${icon('plus', 15)}</button>
      <button class="tb-btn" data-a="close" title="Close">${icon('x', 15)}</button>
    </div>
    <div class="aip-thread"></div>
    <div class="aip-tasks">${TASKS.map(([k, ic, label, tip]) => `<button class="aip-task" data-t="${k}" title="${esc(tip)}">${icon(ic, 13)}<span>${esc(label)}</span></button>`).join('')}</div>
    <div class="aip-compose">
      <textarea rows="2" placeholder="Anything to focus on (optional) — e.g. “the client worries about the outflows”. Then pick a task above."></textarea>
    </div>
    <div class="aip-note">${icon('lock', 11)} <span>Sends the figures this report shows, with the client’s name and account numbers replaced by placeholders. Nothing is added to the report until you add it.</span></div>`;
  const $ = (s) => pane.querySelector(s);
  const thread = $('.aip-thread'), ta = $('textarea');
  const chip = providerChip($('.aip-model'));

  const docKey = () => `${S.st?.doc?.meta?.title || ''}|${JSON.stringify(S.st?.doc?.params?.values || {})}`;
  const save = () => {
    if (!chat) return;
    all = [chat, ...all.filter((c) => c.id !== chat.id)];
    chats.save(all);
  };

  // pages by their number as the report shows it
  const pageIndexOf = (n) => (S.st?.pages || []).findIndex((p) => +p.number === +n);
  const goPage = (n) => {
    const pi = typeof n === 'string' ? (S.st?.doc?.pages || []).findIndex((p) => p.id === n) : pageIndexOf(n);
    if (pi < 0) { toast('That page isn’t in the report.', 'info'); return; }
    S.page = pi;
    document.dispatchEvent(new Event('pagechange'));
    scrollToPage(pi);
  };
  const pageLink = (n) => (n ? `<button class="aip-pg" data-pg="${esc(String(n))}" title="Go to the page">${esc(`p. ${n}`)}</button>` : '');

  const meta = (m) => `<div class="aip-meta">${esc(m.provider || '')} · ${esc(m.model || '')}${m.ms ? ` · ${(m.ms / 1000).toFixed(1)} s` : ''}${feedbackHtml(m.result?.log_at)}</div>`;
  const briefHtml = (r) => {
    const tone = (t) => (t === 'good' ? 'good' : t === 'bad' ? 'bad' : 'neutral');
    const p = r.pitch || {};
    return `${r.heading?.client || r.heading?.period ? `<div class="aip-for">${esc([r.heading.client, r.heading.period].filter(Boolean).join(' · '))}</div>` : ''}
      ${r.summary ? `<p class="aip-sum">${rich(r.summary)}</p>` : ''}
      ${(r.attention || []).length ? `<div class="aip-sec"><span>Pay attention</span>${r.attention.map((a) => `<div class="aip-pt ${tone(a.tone)}"><i></i><div>${rich(a.text, tone(a.tone))}</div>${pageLink(a.page)}</div>`).join('')}</div>` : ''}
      <div class="aip-sec pitch"><span>How to present it <button class="aip-mini" data-act="pitch" title="An e-mail to the client to send with the report — review, then copy or open in Mail">${icon('send', 11)} E-mail to the client</button></span>
        ${p.opening ? `<div class="aip-say">“${rich(p.opening)}”</div>` : ''}
        ${(p.messages || []).length ? `<ol>${p.messages.map((x) => `<li><div>${rich(x.text)}</div>${pageLink(x.page)}</li>`).join('')}</ol>` : ''}
        ${(p.questions || []).length ? `<div class="aip-qs"><span>If the client asks</span>${p.questions.map((q) => `<details><summary>${esc(q.q)}</summary><div>${rich(q.a)}</div></details>`).join('')}</div>` : ''}
        ${p.closing ? `<div class="aip-say">“${rich(p.closing)}”</div>` : ''}
      </div>`;
  };
  const checkHtml = (r) => {
    const order = { high: 0, medium: 1, low: 2 };
    const sorted = (list) => [...list].sort((a, b) => (order[a.severity] ?? 3) - (order[b.severity] ?? 3));
    // errors to fix first; suggestions (worth a look) after, folded when there are errors
    const all = r.findings || [];
    const errors = sorted(all.filter((f) => f.type !== 'suggestion'));
    const tips = sorted(all.filter((f) => f.type === 'suggestion'));
    const find = (f) => `<div class="aip-find ${esc(f.severity || 'low')}"><span class="sev">${esc(f.severity || '')}</span><div><div>${esc(f.text)}</div>${f.fix ? `<div class="fix">${esc(f.fix)}</div>` : ''}</div>${pageLink(f.page)}</div>`;
    return `${r.verdict ? `<p><b>${esc(r.verdict)}</b></p>` : ''}
      ${errors.length ? `<div class="aip-sec"><span>To fix · ${errors.length}</span>${errors.map(find).join('')}</div>` : `<div class="aip-sec"><div class="aip-pt good"><i></i><div>No errors found.</div></div></div>`}
      ${tips.length ? `<details class="aip-sec aip-tips"${errors.length ? '' : ' open'}><summary>Worth a look · ${tips.length}</summary>${tips.map(find).join('')}</details>` : ''}`;
  };
  const noteHtml = (m) => `${m.result.explanation ? `<p>${esc(m.result.explanation)}</p>` : ''}
      <div class="aip-draft">${esc(m.result.text || '')}</div>
      <div class="aip-acts">${m.added ? `<span class="ok">${icon('ok', 12)} Added to the page</span>` : ''}<span class="grow"></span>
        <button class="tb-btn" data-act="copy" title="Copy">${icon('copy', 14)}</button>
        ${m.added ? '' : `<button class="btn sm primary" data-act="add" title="Add it to the page as a note to the client">${icon('plus', 11)} Add to page</button>`}</div>`;

  // a drafted proposal: what it suggests, and buttons to put it in the report (as it is, or edited first)
  const proposalHtml = (m) => {
    const r = m.result || {};
    const dir = (a) => (/^(куп|докуп|откр|увелич|влож|размест|реинв|пополн|buy|add|open|invest)/i.test(a || '') ? 'in' : /^(прод|сократ|закр|зафикс|вывест|sell|reduce|close|trim)/i.test(a || '') ? 'out' : '');
    const list = (head, items, cls) => (items || []).length ? `<div class="aip-pl ${cls}"><b>${esc(head)}</b><ul>${items.map((x) => `<li>${rich(x)}</li>`).join('')}</ul></div>` : '';
    return `${r.explanation ? `<p class="aip-why">${icon('info', 12)} ${esc(r.explanation)}</p>` : ''}
      <div class="aip-prop">
        <div class="aip-pt">${esc(r.title || '')}</div>
        ${r.lead ? `<p>${rich(r.lead)}</p>` : ''}
        ${(r.actions || []).length ? `<div class="aip-pa">${r.actions.map((a) => `<div class="aip-pr"><span class="aip-pill ${dir(a.action)}">${esc(a.action || '')}</span><span class="aip-pw"><span class="aip-pwl"><b>${esc(a.what || '')}</b>${a.amount ? ` · ${esc(a.amount)}` : ''}</span>${a.why ? `<span class="aip-pwy">${esc(a.why)}</span>` : ''}</span>${a.page ? `<button class="aip-pgl" data-pg="${a.page}" title="Go to the page">${esc(String(a.page))}</button>` : ''}</div>`).join('')}</div>` : ''}
        ${list('Expected effect', r.effect, 'good')}${list('Why now', r.reasons, '')}${list('Risks', r.risks, 'bad')}${list('Next steps', r.steps, '')}
      </div>
      <div class="aip-acts">${m.added ? `<span class="ok">${icon('ok', 12)} Added as the proposal page</span>` : ''}<span class="grow"></span>
        <button class="tb-btn" data-act="copy" title="Copy as text">${icon('copy', 14)}</button>
        <button class="btn sm" data-act="prop-edit" title="Open it as notes in the proposal dialog: change anything, then make the page">${icon('rename', 12)} Edit first…</button>
        ${proposals().length ? `<button class="btn sm" data-act="prop-replace" title="Put it in place of one of the report’s proposals">Replace…</button>` : ''}
        <button class="btn sm primary" data-act="prop-add" title="Add it as ${proposals().length ? 'another proposal, after the ones there are' : 'the proposal, before the legal text'}">${icon('plus', 11)} ${proposals().length ? 'Add as a new proposal' : 'Add as a page'}</button></div>`;
  };

  const msgHtml = (m, i) => {
    const t = TASKS.find((x) => x[0] === m.kind) || TASKS[0];
    if (m.role === 'user') return `<div class="aip-msg me"><div class="aip-tag">${icon(t[1], 12)} ${esc(t[2])}${m.pageTitle ? ` · ${esc(m.pageTitle)}` : ''}</div>${m.text ? `<div>${esc(m.text)}</div>` : ''}</div>`;
    if (m.failed) {
      // an administrator can allow figures right here; anyone else asks one
      const allow = /^AI may not use client figures yet\. You can allow it/.test(m.failed) ? ` <button class="btn sm primary" data-allow="${i}">${icon('shield', 12)} Allow for the bank…</button>` : '';
      const set = !allow && /Settings → AI/.test(m.failed) ? ' <button class="btn sm" data-set>Open Settings</button>' : '';
      return `<div class="aip-msg ai bad">${icon('warn', 14)}<div>${esc(m.failed)}${allow}${set}</div></div>`;
    }
    const body = m.kind === 'check' ? checkHtml(m.result) : m.kind === 'note' ? noteHtml(m) : m.kind === 'proposal' ? proposalHtml(m) : briefHtml(m.result);
    const copy = m.kind === 'note' || m.kind === 'proposal' ? '' : m.kind === 'brief'
      ? `<div class="aip-acts"><span class="grow"></span><button class="tb-btn" data-act="copy" title="Copy as plain text">${icon('copy', 14)}</button><button class="btn sm" data-act="mail" title="Copy the whole brief, formatted, for yourself or a colleague (it is internal: not for the client)">${icon('copy', 12)} Copy brief</button><button class="btn sm" data-act="pdf" title="Save the brief as a PDF in the report’s style, with the client and period">${icon('file-pdf', 12)} Save as PDF</button></div>`
      : `<div class="aip-acts"><span class="grow"></span><button class="btn sm" data-act="copy">${icon('copy', 12)} Copy</button></div>`;
    return `<div class="aip-msg ai report" data-i="${i}">${body}${copy}${meta(m)}</div>`;
  };

  // the answer as plain text, for pasting into an e-mail or notes
  const plain = (m) => {
    const r = m.result || {};
    if (m.kind === 'note') return r.text || '';
    if (m.kind === 'proposal') return proposalNotes(r).replace(/\*\*/g, '');
    if (m.kind === 'check') {
      const line = (f) => `• [${f.severity}] p. ${f.page}: ${f.text}${f.fix ? ` — ${f.fix}` : ''}`;
      const errs = (r.findings || []).filter((f) => f.type !== 'suggestion'), tips = (r.findings || []).filter((f) => f.type === 'suggestion');
      return [r.verdict, errs.length ? 'To fix:' : '', ...errs.map(line), tips.length ? 'Worth a look:' : '', ...tips.map(line)].filter(Boolean).join('\n');
    }
    const p = r.pitch || {};
    return [r.summary, '', ...(r.attention || []).map((a) => `• p. ${a.page}: ${a.text}`), '', p.opening, ...(p.messages || []).map((x, k) => `${k + 1}. ${x.text} (p. ${x.page})`),
      '', ...(p.questions || []).map((q) => `— ${q.q}\n  ${q.a}`), '', p.closing].filter((x) => x != null).join('\n').replace(/\*\*/g, '').replace(/\n{3,}/g, '\n\n').trim();
  };

  // the e-mail to the client: shown to review and edit, then copied or opened in Mail
  async function emailDialog(m) {
    const me = session();
    const e = clientEmail(m.result, S.st, me?.name || me?.login || '');
    const body = document.createElement('div');
    body.className = 'ce-box';
    body.innerHTML = `<label class="ce-subj"><span>Subject</span><input value="${esc(e.subject)}"><button class="tb-btn" data-c="subj" title="Copy the subject">${icon('copy', 14)}</button></label>
      <div class="ce-body" contenteditable="true" spellcheck="true">${e.html}</div>
      <div class="hint">${icon('info', 12)} Edit it here if you like. Attach the report itself (Export → PDF) before sending.${m.result.email ? '' : ' This brief was made before e-mails were part of it: the text comes from the pitch.'}</div>`;
    const subj = body.querySelector('input'), ed = body.querySelector('.ce-body');
    [subj, ed].forEach((x) => x.addEventListener('keydown', (ev) => ev.stopPropagation()));
    body.querySelector('[data-c="subj"]').addEventListener('click', () => navigator.clipboard?.writeText(subj.value).then(() => toast('Subject copied.', 'info', 1200)));
    const res = await modal({ title: 'E-mail to the client', iconName: 'send', body, wide: true,
      buttons: [{ label: 'Close', value: null }, { label: 'Open in Mail', value: 'mail' }, { label: 'Copy e-mail', kind: 'primary', value: 'copy' }] });
    if (res === 'copy') {
      const ok = await copyHtml(ed.innerHTML, ed.innerText.trim());
      toast(ok ? 'Copied — paste it into your e-mail.' : 'Could not copy.', ok ? 'info' : 'error', 1800);
    } else if (res === 'mail') {
      const url = `mailto:?subject=${encodeURIComponent(subj.value)}&body=${encodeURIComponent(ed.innerText.trim())}`;
      try { await openUrl(url); } catch { /* toast shown */ }
    }
  }

  const draw = () => {
    if (!chat || !chat.msgs.length) {
      thread.innerHTML = `<div class="aip-empty">${icon('sparkles', 22)}<b>Get ready for the client</b><span>A brief of this report with what to pay attention to and how to present it, a check before you send it, a note for the page in front, or ideas for a proposal.</span>
        <div class="aip-ex">${TASKS.map(([k, ic, label, tip]) => `<button class="aip-chip" data-go="${k}">${icon(ic, 12)} ${esc(label)}<i>${esc(tip)}</i></button>`).join('')}</div></div>`;
      thread.querySelectorAll('[data-go]').forEach((b) => b.addEventListener('click', () => ask(b.dataset.go)));
      return;
    }
    thread.innerHTML = chat.msgs.map(msgHtml).join('') + (busy ? `<div class="aip-msg ai wait"><span class="spin"></span>Reading the report…</div>` : '');
    thread.querySelectorAll('[data-set]').forEach((b) => b.addEventListener('click', () => document.dispatchEvent(new CustomEvent('open-settings', { detail: /Aggregates/.test(b.closest('.aip-msg')?.textContent || '') ? 'ai-bank' : 'ai' }))));
    thread.querySelectorAll('[data-allow]').forEach((b) => b.addEventListener('click', () => allowFigures(+b.dataset.allow)));
    thread.querySelectorAll('[data-pg]').forEach((b) => b.addEventListener('click', () => goPage(+b.dataset.pg)));
    thread.querySelectorAll('.aip-msg.ai[data-i]').forEach((el) => {
      const m = chat.msgs[+el.dataset.i];
      el.querySelector('[data-act="copy"]')?.addEventListener('click', () => navigator.clipboard?.writeText(plain(m)).then(() => toast('Copied.', 'info', 1200)));
      el.querySelector('[data-act="mail"]')?.addEventListener('click', async () => {
        const ok = await copyHtml(briefEmail(m.result, S.st), plain(m));
        toast(ok ? 'Copied with its formatting.' : 'Could not copy.', ok ? 'info' : 'error', 1800);
      });
      el.querySelector('[data-act="pitch"]')?.addEventListener('click', () => emailDialog(m));
      el.querySelector('[data-act="pdf"]')?.addEventListener('click', async () => {
        const hd = m.result.heading || {};
        const name = [appLanguage() === 'ru' ? 'Бриф' : 'Brief', hd.client, hd.period].filter(Boolean).join(' — ');
        try {
          const path = await call('brief_pdf', { pages: briefPages(m.result, S.st), ...PDF_SIZE, name });
          if (path) toast(`Saved ${path.split('/').pop()}`, 'info', 2400);
        } catch { /* toast shown */ }
      });
      el.querySelector('[data-act="prop-edit"]')?.addEventListener('click', () => proposalDialog({ notes: proposalNotes(m.result) }));
      // into the report: as a new proposal, or in place of one there is
      const put = async (id) => {
        let st, made;
        try { const r = await call('proposal_set', { id, spec: m.result, notes: proposalNotes(m.result) }); setState(r.st); st = r.st; made = r.id; } catch { return; }
        m.added = true;
        save();
        draw();
        const pg = st.doc.pages.find((p) => p.group === `PROPOSAL:${made}`);
        if (pg) goPage(pg.id);
        toast(id ? 'Proposal replaced.' : 'Proposal added before the legal text.', 'info', 2600);
      };
      el.querySelector('[data-act="prop-add"]')?.addEventListener('click', () => put(null));
      el.querySelector('[data-act="prop-replace"]')?.addEventListener('click', (e) => menu(e.currentTarget, [
        { title: 'Replace which proposal?' },
        ...proposals().map((p) => ({ label: p.title, icon: 'briefcase', run: async () => {
          const ok = await modal({ title: 'Replace this proposal?', iconName: 'briefcase', body: `<p>«${esc(p.title)}» is replaced by this draft, in its place. Undo (⌘Z) brings it back.</p>`,
            buttons: [{ label: 'Cancel', value: false }, { label: 'Replace', kind: 'primary', value: true }] });
          if (ok) put(p.id);
        } })),
      ], { align: 'end' }));
      el.querySelector('[data-act="add"]')?.addEventListener('click', async () => {
        const page = (S.st?.doc?.pages || []).find((p) => p.id === m.page);
        if (!page) { toast('That page isn’t in the report.', 'info'); return; }
        const w = newWidget('comment');
        // the full width of the page, so a few lines of text stay a few rows high
        w.w = S.st.doc.page.cols || w.w;
        w.opts = { ...(w.opts || {}), text: m.result.text || '', signed: true, ai: true };
        const before = new Set(page.widgets.map((x) => x.id));
        let st = await edit({ op: 'add_widget', page: page.id, widget: w, at: null });
        if (!st) return;
        // a full page: the note goes on a page of its own right after it
        const pi = st.doc.pages.findIndex((p) => p.id === page.id);
        const added = st.doc.pages[pi]?.widgets.find((x) => !before.has(x.id));
        const rows = st.doc.page.rows;
        if (added && added.y + added.h > rows) {
          const moved = await edit({ op: 'move_to_page', id: added.id, page: null }, { quiet: true });
          if (moved) st = moved;
        }
        m.added = true;
        save();
        draw();
        const home = st.doc.pages.find((p) => p.widgets.some((x) => x.id === added?.id));
        goPage(home?.id || page.id);
      });
    });
    thread.scrollTop = thread.scrollHeight;
  };

  const history = () => (chat?.msgs || []).filter((m) => !m.failed).map((m) => (m.role === 'user'
    ? { role: 'user', content: `${m.kind.toUpperCase()}${m.text ? `: ${m.text}` : ''}` }
    : { role: 'assistant', content: JSON.stringify(m.result || {}) }));

  // an administrator allows client figures for the bank (AI policy "aggregates"), then the task runs again
  async function allowFigures(i) {
    const ok = await modal({ title: 'Allow AI to use client figures?', iconName: 'shield',
      body: `<p>For everyone in the bank, the report assistant may then send the figures a report shows — totals, returns, shares, monthly values, at most a dozen rows per table — to the AI provider.</p>
        <p>Names and numbers of clients, accounts and instruments never leave: they go as placeholders and are put back on this computer. Every request is logged, with what was sent. You can change this any time in Settings → AI assistant.</p>`,
      buttons: [{ label: 'Cancel', value: false }, { label: 'Allow', kind: 'primary', value: true }] });
    if (!ok) return;
    try {
      const st = await call('ai_status', {}, { quiet: true });
      const pol = st.policy || { providers: [], off: false };
      await call('ai_set_policy', { policy: { ...pol, level: 'aggregates', off: false } });
    } catch (e) { toast(String(e), 'error'); return; }
    const kind = chat.msgs[i]?.kind || 'brief';
    // the refusal and its request go; the task runs again
    chat.msgs.splice(i - 1, 2);
    save();
    ask(kind);
  }

  async function ask(kind) {
    if (busy) return;
    if (S.st?.doc?.kind !== 'report') { toast('The report assistant works on reports.', 'info'); return; }
    const text = ta.value.trim();
    const pageId = kind === 'note' ? currentPageId() : null;
    const pageTitle = kind === 'note' ? (S.st.pages[S.st.doc.pages.findIndex((p) => p.id === pageId)]?.title || '') : '';
    const hist = history();
    if (!chat) chat = { id: `c${Date.now()}`, title: `${TASKS.find((t) => t[0] === kind)[2]} · ${S.st.doc.meta?.title || ''}`.slice(0, 80), at: Date.now(), doc: docKey(), msgs: [] };
    chat.msgs.push({ role: 'user', kind, text, pageTitle });
    ta.value = '';
    busy = true;
    save();
    draw();
    try {
      const r = await call('ai_report', { kind, page: pageId, prompt: text, provider: chip.current()?.id || null, history: hist, lang: appLanguage() }, { quiet: true });
      chat.msgs.push({ role: 'ai', kind, result: r, page: pageId, provider: r.provider, model: r.model, ms: r.ms });
    } catch (e) {
      chat.msgs.push({ role: 'ai', kind, failed: String(e) });
    } finally {
      busy = false;
      save();
      draw();
    }
  }

  pane.querySelectorAll('[data-t]').forEach((b) => b.addEventListener('click', () => ask(b.dataset.t)));
  ta.addEventListener('keydown', (e) => { if (!(e.metaKey || e.ctrlKey)) e.stopPropagation(); });
  $('[data-a="new"]').addEventListener('click', () => { chat = null; draw(); });
  $('[data-a="close"]').addEventListener('click', () => close());
  $('[data-a="history"]').addEventListener('click', (e) => historyMenu(e.currentTarget, chats, chat?.id, (c) => { chat = c; draw(); }, () => { all = []; chat = null; draw(); }));

  // another report (or another client in it): another conversation
  let lastDoc = docKey();
  onChange(() => {
    if (docKey() !== lastDoc) { lastDoc = docKey(); chat = null; if (!pane.hidden) draw(); }
  });

  function open() {
    if (!pane.hidden) return;
    pane.hidden = false;
    onToggle(true);
    if (!chip.loaded()) chip.load();
    draw();
  }
  function close() {
    pane.hidden = true;
    onToggle(false);
  }
  return {
    open,
    close,
    toggle: () => (pane.hidden ? open() : close()),
    isOpen: () => !pane.hidden,
    available: () => S.st?.doc?.kind === 'report',
  };
}
