// The SQL workspace's AI pane: a conversation on the right. Ask for data and
// get a query (the app runs it first), have the editor's query explained, or a
// failing one fixed, or turned into a report source (literals become parameters);
// questions about the database are answered in words. Follow-ups build on the conversation. The model sees the
// database's structure only — tables, columns, parameter names — never values.
// Conversations are kept per connection on this computer.

import { call } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { toast } from './ui.js';
import { chatStore, providerChip, historyMenu, feedbackHtml } from './aichat.js';
import { formatSql } from './sqled.js';
import { appLanguage } from './i18n.js';

// the editor's own formatting: clauses on their lines, never a one-liner
const tidy = (sql) => { try { return formatSql(sql).trim(); } catch { return sql; } };


/**
 * Mounts the pane. `pane` is the empty aside, `ed` the SQL editor, `run(sql)` runs a query like
 * the Run button, `conn` the connection, `saveAs(sql, { name, date })` saves a query as a data source,
 * `onToggle(open)` lets the workspace show its splitter.
 */
export function sqlAssistant({ pane, conn, ed, run, saveAs = null, onToggle = () => {} }) {
  const chats = chatStore(`quire.ai.chats.${conn.id || conn.name}`);
  let all = chats.load();
  let chat = all[0] && all[0].msgs.length ? all[0] : null;
  let lastError = null; // the editor's last failed run: { sql, error }
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
    <div class="aip-quick">
      <button class="aip-chip" data-a="explain">${icon('info', 12)} Explain the editor’s query</button>
      <button class="aip-chip" data-a="fix" disabled>${icon('wrench', 12)} Fix the last error</button>
      <button class="aip-chip" data-a="source">${icon('database', 12)} Make it a report source</button>
    </div>
    <div class="aip-compose">
      <textarea rows="3" placeholder="Ask for data — e.g. “each client’s portfolio value at the end of the period, by asset class” — or about the database: “where are FX rates kept?”. Follow-ups refine the last query."></textarea>
      <button class="btn primary aip-send" title="Send (↵ · ⇧↵ for a new line)">${icon('send', 15)}</button>
    </div>
    <div class="aip-note">${icon('lock', 11)} Sends table and column names, types and parameter names — never values. The app runs each query it writes.</div>`;
  const $ = (s) => pane.querySelector(s);
  const thread = $('.aip-thread'), ta = $('textarea'), modelBtn = $('.aip-model');

  // ---- which AI
  const chip = providerChip(modelBtn);
  const current = chip.current;

  // ---- the conversation
  const save = () => {
    if (!chat) return;
    all = [chat, ...all.filter((c) => c.id !== chat.id)];
    chats.save(all);
  };
  const newChat = () => { chat = null; draw(); ta.focus(); };
  const ensureChat = (title) => {
    if (!chat) chat = { id: `c${Date.now()}`, title: title.slice(0, 80), at: Date.now(), msgs: [] };
    return chat;
  };
  // the model's words: plain text, `names` as code
  const words = (t) => esc(t).replace(/`([^`]+)`/g, '<code>$1</code>');
  const LABEL = { explain: ['info', 'Explain this query'], fix: ['wrench', 'Fix this query'], source: ['database', 'Make this a report source'] };
  const sqlBlock = (sql) => `<pre class="aip-sql">${esc(tidy(sql))}</pre>`;
  const msgHtml = (m, i) => {
    if (m.role === 'user') {
      const label = LABEL[m.kind] ? `${icon(LABEL[m.kind][0], 12)} ${LABEL[m.kind][1]}` : '';
      return `<div class="aip-msg me">${label ? `<div class="aip-tag">${label}</div>` : ''}${m.text ? `<div>${esc(m.text)}</div>` : ''}${m.sql && m.kind !== 'write' ? `<details><summary>Query</summary>${sqlBlock(m.sql)}</details>` : ''}${m.error ? `<div class="aip-err">${esc(m.error)}</div>` : ''}</div>`;
    }
    if (m.failed) return `<div class="aip-msg ai bad">${icon('warn', 14)}<div>${esc(m.failed)}${/Settings → AI/.test(m.failed) ? ' <button class="btn sm" data-set>Open Settings</button>' : ''}</div></div>`;
    const meta = `<div class="aip-meta">${esc(m.provider || '')} · ${esc(m.model || '')}${m.attempts > 1 ? ` · ${m.attempts} tries` : ''}${m.ms ? ` · ${(m.ms / 1000).toFixed(1)} s` : ''}${feedbackHtml(m.logAt)}</div>`;
    if (!m.sql) return `<div class="aip-msg ai">${m.explanation ? `<p>${words(m.explanation)}</p>` : ''}${meta}</div>`;
    // a source: what became a parameter, and what stayed as it was
    const src = m.kind === 'source' ? `${m.name ? `<div class="aip-src">${icon('database', 12)} <b>${esc(m.name)}</b>${m.date ? ` · dated by <code>${esc(m.date)}</code>` : ''}</div>` : ''}
      ${(m.changes || []).length ? `<div class="aip-list"><span>Now parameters</span>${m.changes.map((x) => `<div>${words(x)}</div>`).join('')}</div>` : '<div class="aip-list"><span>No values to turn into parameters</span></div>'}
      ${(m.left || []).length ? `<div class="aip-list kept"><span>Kept as they are</span>${m.left.map((x) => `<div>${words(x)}</div>`).join('')}</div>` : ''}` : '';
    const ran = m.ok ? `<span class="ok">${icon('ok', 12)} Runs · ${(m.columns || []).length} columns${m.rows != null ? ` · ${Number(m.rows).toLocaleString()} rows` : ''}</span>` : `<span class="bad">${icon('warn', 12)} Still fails: ${esc(m.error || '')}</span>`;
    return `<div class="aip-msg ai${m.ok ? '' : ' warn'}" data-i="${i}">${m.explanation ? `<p>${words(m.explanation)}</p>` : ''}${src}${sqlBlock(m.sql)}
      <div class="aip-acts${m.kind === 'source' && saveAs && m.ok ? ' src' : ''}">${ran}<span class="grow"></span>
        <button class="tb-btn" data-act="copy" title="Copy">${icon('copy', 14)}</button>
        <button class="btn sm" data-act="insert" title="Insert at the cursor">Insert</button>
        <button class="btn sm${m.kind === 'source' && saveAs ? '' : ' primary'}" data-act="apply" title="Put it in the editor and run it">${icon('play', 11)} Use</button>
        ${m.kind === 'source' && saveAs && m.ok ? `<button class="btn sm primary" data-act="save" title="Put it in the editor, run it and save it as a data source">${icon('database', 11)} Save as source…</button>` : ''}</div>${meta}</div>`;
  };
  const draw = () => {
    if (!chat || !chat.msgs.length) {
      thread.innerHTML = `<div class="aip-empty">${icon('sparkles', 22)}<b>Ask for the data you need</b><span>The assistant writes the query, the app runs it, and you choose what goes into the editor.</span>
        <div class="aip-ex">${['Value of each client’s portfolio at the end of the period', 'Top 10 holdings by value with their share', 'Monthly deposits and withdrawals for the period', 'Where are FX rates kept, and how do they join to positions?'].map((x) => `<button class="aip-chip" data-ex>${esc(x)}</button>`).join('')}</div></div>`;
      thread.querySelectorAll('[data-ex]').forEach((b) => b.addEventListener('click', () => { ta.value = b.textContent; ask('write'); }));
      return;
    }
    thread.innerHTML = chat.msgs.map(msgHtml).join('') + (busy ? `<div class="aip-msg ai wait"><span class="spin"></span>Thinking — the app runs each query it writes…</div>` : '');
    thread.querySelectorAll('[data-set]').forEach((b) => b.addEventListener('click', () => document.dispatchEvent(new CustomEvent('open-settings', { detail: 'ai' }))));
    thread.querySelectorAll('.aip-msg.ai[data-i]').forEach((el) => {
      const m = chat.msgs[+el.dataset.i];
      el.querySelector('[data-act="copy"]').addEventListener('click', () => navigator.clipboard?.writeText(tidy(m.sql)).then(() => toast('Copied.', 'info', 1200)));
      el.querySelector('[data-act="insert"]').addEventListener('click', () => { ed.insert(tidy(m.sql)); ed.focus(); });
      el.querySelector('[data-act="apply"]').addEventListener('click', () => { const q = tidy(m.sql); ed.set(q); run(q); });
      el.querySelector('[data-act="save"]')?.addEventListener('click', () => saveAs(tidy(m.sql), { name: m.name || '', date: m.date || '' }));
    });
    thread.scrollTop = thread.scrollHeight;
  };

  // what the model has already seen of this conversation (structure only: prompts, its SQL and explanations)
  const history = () => (chat?.msgs || []).filter((m) => !m.failed).map((m) => (m.role === 'user'
    ? { role: 'user', content: [{ explain: 'Explain this query:', fix: 'Fix this query:', source: 'Turn this query into a report source:' }[m.kind] || '', m.text, m.kind !== 'write' ? m.sql : ''].filter(Boolean).join('\n') }
    : { role: 'assistant', content: JSON.stringify(m.sql ? { sql: m.sql, explanation: m.explanation } : { explanation: m.explanation }) }));

  async function ask(kind, extra = {}) {
    if (busy) return;
    const text = kind === 'write' ? ta.value.trim() : extra.prompt || '';
    const sql = extra.sql ?? ed.value.trim();
    if (kind === 'write' && !text) { ta.focus(); return; }
    if (kind !== 'write' && !sql) { toast('There is no query in the editor.', 'info'); return; }
    open();
    const hist = history();
    ensureChat(text || { explain: 'Explain a query', fix: 'Fix a query', source: 'Make a report source' }[kind]);
    chat.msgs.push({ role: 'user', kind, text, sql: kind === 'write' ? '' : sql, error: extra.error || '' });
    if (kind === 'write') ta.value = '';
    busy = true;
    save();
    draw();
    try {
      // a follow-up refines the editor's query; a first question starts from it if there is one
      const r = await call('ai_sql', { connection: conn, kind, prompt: text, sql, error: extra.error || '', provider: current()?.id || null, history: hist, lang: appLanguage() }, { quiet: true });
      chat.msgs.push({ role: 'ai', kind, sql: r.sql ? tidy(r.sql) : '', explanation: r.explanation || '', ok: !!r.ok, error: r.error || '', columns: r.columns || [], rows: r.rows, provider: r.provider, model: r.model, attempts: r.attempts, ms: r.ms, logAt: r.log_at,
        ...(kind === 'source' ? { name: r.name || '', date: r.date_column || '', changes: r.changes || [], left: r.left || [] } : {}) });
    } catch (e) {
      chat.msgs.push({ role: 'ai', failed: String(e) });
    } finally {
      busy = false;
      save();
      draw();
    }
  }

  ta.addEventListener('keydown', (e) => {
    // typing stays here; ⌘ shortcuts (⌘[ ⌘] ⌘I ⌘S…) reach the workspace as from anywhere in it
    if (!(e.metaKey || e.ctrlKey)) e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); ask('write'); }
  });
  $('.aip-send').addEventListener('click', () => ask('write'));
  $('[data-a="explain"]').addEventListener('click', () => ask('explain'));
  $('[data-a="fix"]').addEventListener('click', () => lastError && ask('fix', lastError));
  $('[data-a="source"]').addEventListener('click', () => ask('source'));
  $('[data-a="new"]').addEventListener('click', newChat);
  $('[data-a="close"]').addEventListener('click', () => close());
  $('[data-a="history"]').addEventListener('click', (e) => historyMenu(e.currentTarget, chats, chat?.id, (c) => { chat = c; draw(); }, () => { all = []; chat = null; draw(); }));

  function open() {
    if (!pane.hidden) return;
    pane.hidden = false;
    onToggle(true);
    if (!chip.loaded()) chip.load();
    draw();
    setTimeout(() => ta.focus());
  }
  function close() {
    pane.hidden = true;
    onToggle(false);
  }
  return {
    open,
    close,
    toggle: () => (pane.hidden ? open() : close()),
    /** Fix a query the database refused. */
    fix: (sql, error) => { lastError = { sql, error }; ask('fix', { sql, error }); },
    /** The editor's last run: a failure enables "Fix the last error". */
    noteRun: (sql, error) => { lastError = error ? { sql, error } : null; $('[data-a="fix"]').disabled = !lastError; },
    isOpen: () => !pane.hidden,
  };
}
