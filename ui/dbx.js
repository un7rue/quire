// Database connections and the SQL workspace: connect a database, browse
// its tables, run SQL (joins, CTEs, window functions) against it, and add
// the query as a data source when the result looks right.

import { S, edit, call, api } from './state.js';
import { icon } from './icons.js';
import { esc, h, fmtNum, fmtDate } from './util.js';
import { menu, modal, toast } from './ui.js';
import { sqlEditor } from './sqled.js';
import { sqlAssistant } from './aiassist.js';
import { registerAI, unregisterAI, setActiveView, paintAIButton } from './ai.js';
import { hasRole } from './auth.js';

const KINDS = [
  ['duckdb', 'DuckDB file', 'A .duckdb database file'],
  ['sqlite', 'SQLite file', 'A .sqlite or .db file'],
  ['postgres', 'PostgreSQL', 'A server: postgresql://user:password@host:5432/db'],
  ['clickhouse', 'ClickHouse', 'A server over HTTP: clickhouse://user:password@host:8123/database (clickhouses:// for TLS)'],
];
// servers have an address; DuckDB and SQLite a file
const SERVER = (k) => k === 'postgres' || k === 'clickhouse';
const URL_HINT = { postgres: 'postgresql://user:password@host:5432/db', clickhouse: 'clickhouse://user:password@host:8123/database' };
const kindLabel = (k) => (KINDS.find((x) => x[0] === k) || [, k])[1];

/** Where a connection points, for lists (passwords hidden). */
export function connWhere(c) {
  if (SERVER(c.kind)) return (c.url || '').replace(/:[^:@/]*@/, ':•••@').replace(/^(postgres(ql)?|clickhouses?|https?):\/\//, '');
  return (c.path || '').split('/').pop() || 'no file chosen';
}

// a connection's tables, cached for this session (the workspace and the list)
const TABLES = new Map();
async function tablesOf(c, { fresh = false } = {}) {
  const key = JSON.stringify([c.kind, c.path, c.url]);
  if (!fresh && TABLES.has(key)) return TABLES.get(key);
  const t = await call('db_tables', { connection: c }, { quiet: true });
  TABLES.set(key, t);
  return t;
}

/** The "Connections" part of the Data tab. */
export function connectionsSection() {
  const doc = S.st.doc;
  const conns = doc.connections || [];
  const el = h(`<div>
    <div class="sec-title row">Connections<button class="btn sm" id="add-conn">${icon('plus', 14)} Add</button></div>
    <div class="conn-list"></div>
  </div>`);
  const list = el.querySelector('.conn-list');
  if (!conns.length) list.append(h(`<div class="empty-note small">${icon('database', 16)}<span>Add a connection from the Data area to browse its tables, try SQL on them and turn the results into data sources.</span></div>`));
  for (const c of conns) {
    const used = doc.datasets.filter((d) => d.source?.type === 'db' && d.source.connection === c.id).length;
    const item = h(`<div class="ds-item conn-item" title="Open the SQL workspace">
      <span class="gi-ic">${icon('database', 16)}</span>
      <span class="gi-body"><b>${esc(c.name || kindLabel(c.kind))}</b><span>${esc(kindLabel(c.kind))} · ${esc(connWhere(c))}${used ? ` · ${used} source${used > 1 ? 's' : ''}` : ''}</span></span>
      <button class="gi-more">${icon('more', 15)}</button>
    </div>`);
    item.addEventListener('click', (e) => { if (!e.target.closest('.gi-more')) workspace(c); });
    item.querySelector('.gi-more').addEventListener('click', (e) => menu(e.currentTarget, [
      { label: 'Open SQL workspace', icon: 'code', run: () => workspace(c) },
      hasRole('data') && { label: 'Edit connection…', icon: 'rename', run: () => connectionDialog(c) },
      { label: 'Remove from this template', icon: 'trash', danger: true, run: () => removeConnection(c, used) },
    ], { align: 'end' }));
    list.append(item);
  }
  el.querySelector('#add-conn').addEventListener('click', (e) => addConnection(e.currentTarget));
  return el;
}

/**
 * Adds a connection to the template: one of the Data area's shared connections
 * (credentials are managed there, by the Data role); the Data role may also
 * make a new one, which joins the shared list.
 */
export async function addConnection(anchor) {
  let shared = [];
  try { shared = await call('auth', { cmd: 'connections' }, { quiet: true }); } catch { shared = []; }
  const have = new Set((S.st.doc.connections || []).map((x) => x.id));
  const add = async (c) => {
    await edit({ op: 'set_connections', connections: [...(S.st.doc.connections || []), c] });
    const saved = (S.st.doc.connections || []).find((x) => x.id === c.id);
    if (saved) workspace(saved);
  };
  menu(anchor, [
    { title: 'From the Data area' },
    ...shared.map((c) => ({ label: c.name, icon: 'database', desc: `${kindLabel(c.kind)} · ${connWhere(c)}${have.has(c.id) ? ' · already added' : ''}`, disabled: have.has(c.id), run: () => add(c) })),
    !shared.length && { label: 'No shared connections yet', icon: 'info', disabled: true, run: () => {} },
    { sep: true },
    hasRole('data')
      ? { label: 'New connection…', icon: 'plus', desc: 'Joins the Data area’s list', run: () => connectionDialog(null, { onSave: async (v) => {
        const out = await call('auth', { cmd: 'save_connections', args: { connections: [...shared, v] } });
        const made = out[out.length - 1];
        if (made) add(made);
      } }) }
      : { label: 'Connections are managed in the Data area', icon: 'lock', desc: 'Ask someone with the Data role to add one', disabled: true, run: () => {} },
  ]);
}

async function removeConnection(c, used) {
  if (used) {
    const ok = await modal({ title: 'Remove the connection?', body: `<p>${used} data source${used > 1 ? 's use' : ' uses'} “${esc(c.name)}”. They will stop loading until you connect it again.</p>`,
      buttons: [{ label: 'Cancel', value: false }, { label: 'Remove', value: true, kind: 'danger' }], iconName: 'warn', iconKind: 'warn' });
    if (!ok) return;
  }
  edit({ op: 'set_connections', connections: (S.st.doc.connections || []).filter((x) => x.id !== c.id) });
}

/** Add or edit a connection: its kind, name and where it is; Test lists its tables. */
export async function connectionDialog(c, { onSave = null } = {}) {
  const cur = { id: '', name: '', kind: 'duckdb', path: '', url: '', ...(c || {}) };
  const demo = await call('db_demo_path', {}, { quiet: true }).catch(() => null);
  const body = h(`<div class="modal-body conn-dlg">
    <div class="seg conn-kind">${KINDS.map(([k, l]) => `<button data-k="${k}" class="${k === cur.kind ? 'on' : ''}">${esc(l)}</button>`).join('')}</div>
    <label>Name<input name="name" value="${esc(cur.name)}" placeholder="e.g. Private bank DWH"></label>
    <label class="f-path">Database file<input name="path" value="${esc(cur.path)}" placeholder="/path/to/data.duckdb — or ~/… or next to the document"></label>
    <label class="f-url">Server<input name="url" value="${esc(cur.url)}" placeholder="${URL_HINT[cur.kind] || URL_HINT.postgres}"></label>
    <div class="hint f-kindhint"></div>
    ${demo ? `<button class="btn sm link" id="use-demo">${icon('database', 13)} Use the demo bank database</button>` : ''}
    <div class="conn-testrow"><button class="btn sm" id="test-conn">${icon('ok', 13)} Test connection</button><div class="conn-test"></div></div>
    <div class="hint">Queries only read data. Values change only when you edit a result in the SQL workspace and press Save.</div>
  </div>`);
  const $ = (n) => body.querySelector(`[name="${n}"]`);
  const show = () => {
    body.querySelector('.f-path').hidden = SERVER(cur.kind);
    body.querySelector('.f-url').hidden = !SERVER(cur.kind);
    $('url').placeholder = URL_HINT[cur.kind] || URL_HINT.postgres;
    body.querySelector('.f-kindhint').textContent = SERVER(cur.kind) ? (KINDS.find((x) => x[0] === cur.kind) || [])[2] || '' : '';
    body.querySelector('#use-demo')?.toggleAttribute('hidden', cur.kind !== 'duckdb');
  };
  body.querySelectorAll('.conn-kind button').forEach((b) => b.addEventListener('click', () => {
    cur.kind = b.dataset.k;
    body.querySelectorAll('.conn-kind button').forEach((x) => x.classList.toggle('on', x === b));
    show();
  }));
  body.querySelector('#use-demo')?.addEventListener('click', () => {
    $('path').value = demo;
    if (!$('name').value) $('name').value = 'Demo bank';
  });
  const read = () => ({ ...cur, name: $('name').value.trim() || kindLabel(cur.kind), path: $('path').value.trim(), url: $('url').value.trim() });
  const test = async () => {
    const out = body.querySelector('.conn-test');
    out.className = 'conn-test';
    out.textContent = 'Connecting…';
    try {
      const t = await tablesOf(read(), { fresh: true });
      out.classList.add('ok');
      out.innerHTML = `${icon('ok', 14)} Connected: ${t.length} table${t.length === 1 ? '' : 's'}${t.length ? ` (${t.slice(0, 6).map((x) => esc(x.name)).join(', ')}${t.length > 6 ? '…' : ''})` : ''}`;
      return true;
    } catch (e) {
      out.classList.add('bad');
      out.innerHTML = `${icon('warn', 14)} ${esc(String(e))}`;
      return false;
    }
  };
  body.querySelector('#test-conn').addEventListener('click', test);
  show();
  const v = await modal({ title: c ? 'Edit connection' : 'Connect a database', iconName: 'database', body,
    buttons: [{ label: 'Cancel', value: null }, { label: c ? 'Save' : 'Connect', value: () => read(), kind: 'primary' }] });
  if (!v) return;
  // the Data area keeps its own list
  if (onSave) return onSave(v);
  const conns = [...(S.st.doc.connections || [])];
  const at = conns.findIndex((x) => x.id === v.id && v.id);
  if (at >= 0) conns[at] = v;
  else conns.push(v);
  await edit({ op: 'set_connections', connections: conns });
  const saved = (S.st.doc.connections || []).find((x) => x.name === v.name && x.kind === v.kind && x.path === v.path && x.url === v.url);
  if (!c && saved) workspace(saved);
}

// ---------------------------------------------------------------- workspace

/** A result cell as the SQL workspace shows it: numbers grouped, dates in the locale, nulls dimmed. */
export function cellHtml(v, kind) {
  if (v == null) return '<span class="null">null</span>';
  if (kind === 'number' && typeof v === 'number') return esc(fmtNum(v, 'number', { decimals: Number.isInteger(v) ? 0 : null }));
  if (kind === 'date') return esc(fmtDate(v));
  if (typeof v === 'boolean') return `<span class="bool">${v}</span>`;
  return esc(String(v));
}

/** Rows (with their columns' kinds) as the SQL workspace's grid, read-only. */
export function gridHtml(fields, rows) {
  return `<div class="dbx-grid dx-grid"><table><thead><tr><th class="rn">#</th>${fields.map((f) => `<th class="k-${esc(f.kind)}"><span>${esc(f.name)}</span><i>${esc(f.kind)}</i></th>`).join('')}</tr></thead>
    <tbody>${rows.map((r, i) => `<tr><td class="rn">${i + 1}</td>${r.map((v, j) => `<td class="k-${esc(fields[j]?.kind || 'text')}">${cellHtml(v, fields[j]?.kind)}</td>`).join('')}</tr>`).join('')}</tbody></table></div>`;
}

const qid = (s) => (/^[a-z_][a-z0-9_]*$/.test(s) ? s : `"${s.replace(/"/g, '""')}"`);
const tname = (t) => (t.schema && t.schema !== 'main' && t.schema !== 'public' ? `${qid(t.schema)}.` : '') + qid(t.name);

// the workspace's own settings, kept between sessions on this computer
const PREFS_KEY = 'quire.sqlws';
const PREFS0 = { edFont: '', edSize: 12.5, outFont: '', outSize: 12, limit: 200, upper: false, stats: true };
const loadPrefs = () => { try { return { ...PREFS0, ...JSON.parse(localStorage.getItem(PREFS_KEY) || '{}') }; } catch { return { ...PREFS0 }; } };
const savePrefs = (p) => { try { localStorage.setItem(PREFS_KEY, JSON.stringify(p)); } catch { /* private window */ } };
const LIMITS = [100, 200, 500, 1000, 5000, 10000];
const isMac = /Mac/.test(navigator.platform);
const K = (s) => (isMac ? s : s.replace('⌘', 'Ctrl+').replace('⇧', 'Shift+').replace('⌥', 'Alt+'));

// ---------------------------------------------------------------- views
// SQL workspaces open as views docked in the main window, next to the report:
// tabs in the title bar (and ⌃Tab) switch between them.

const VIEWS = new Map(); // key → { key, title, tip, el, onShow, close }
let activeView = null, lastView = null;
// the top bar's AI button opens the assistant of the workspace in front
setActiveView(() => activeView);
const viewHost = () => {
  let v = document.getElementById('views');
  if (!v) {
    v = document.createElement('div');
    v.id = 'views';
    const stage = document.getElementById('stage');
    stage.insertBefore(v, document.getElementById('home'));
  }
  return v;
};

/** Shows a view (a workspace's key), or the report (null). */
export function showView(key) {
  if (activeView && activeView !== key) lastView = activeView;
  activeView = key && VIEWS.has(key) ? key : null;
  document.body.classList.toggle('sql-active', !!activeView);
  for (const v of VIEWS.values()) v.el.hidden = v.key !== activeView;
  paintAIButton();
  if (activeView) VIEWS.get(activeView).onShow?.();
  mountViewTabs();
  if (!activeView) window.dispatchEvent(new Event('resize')); // the canvas fits again
}

/** The open workspaces, for the rail: [{ key, title, tip, global, active }]. */
export function openViews() {
  return [...VIEWS.values()].map((v) => ({ key: v.key, title: v.title, tip: v.tip, global: !!v.global, active: v.key === activeView }));
}
export const activeViewKey = () => activeView;
export const closeView = (key) => VIEWS.get(key)?.close();

/** Tells the rail the open workspaces changed (the name is kept for the callers). */
export function mountViewTabs() {
  document.dispatchEvent(new Event('views-changed'));
}

/** Closes every workspace without asking (another document opened); `keepGlobal` keeps the Data area's. */
export function closeAllViews({ keepGlobal = false } = {}) {
  for (const [k, v] of [...VIEWS]) {
    if (keepGlobal && v.global) continue;
    v.destroy?.();
    v.el.remove();
    VIEWS.delete(k);
  }
  if (!activeView || !VIEWS.has(activeView)) showView(null);
  else mountViewTabs();
}

// ⌃Tab: between the report and the last workspace
window.addEventListener('keydown', (e) => {
  if (!(e.ctrlKey && e.key === 'Tab') || !VIEWS.size) return;
  e.preventDefault();
  e.stopPropagation();
  if (activeView) showView(null);
  else showView(lastView && VIEWS.has(lastView) ? lastView : [...VIEWS.keys()][0]);
}, true);

/**
 * The SQL workspace of a connection. `dataset`: a data source on this
 * connection to edit (its query opens in the editor; saving updates it).
 */
export async function workspace(c, { dataset = null, global = false, saveSource = null, fresh = false } = {}) {
  // opened from the Data area (no document): it stays when documents open and close
  if (!hasRole('data') && !hasRole('design')) { toast('The SQL workspace needs the Data or Design role.', 'info', 3200); return; }
  const canWrite = hasRole('data');
  global = global || !S.st?.doc;
  // where "Add as source / Save source" goes: the Data area's shared sources, or the open template
  const canSave = !!saveSource || !global;
  // `fresh`: a new, empty query in its own tab (rather than the connection's open workspace)
  const key = dataset ? `ds:${dataset.id}` : `${global ? 'g' : 'c'}:${c.id || c.name}${fresh ? `:${Date.now()}` : ''}`;
  if (VIEWS.has(key)) { showView(key); return; }
  const prefs = loadPrefs();
  const el = h(`<div class="dbx">
    <div class="dbx-head">
      <span class="ic">${icon('database', 18)}</span>
      <div class="dbx-title"><b>${esc(c.name)}</b><span>${esc(kindLabel(c.kind))} · ${esc(connWhere(c))}</span></div>
      <nav class="dbx-menus">${['File', 'Edit', 'Run', 'View', 'Help'].map((m) => `<button class="dbx-menu" data-menu="${m}">${m}</button>`).join('')}</nav>
      <span class="dbx-file"></span>
      <span class="grow"></span>
      <button class="tb-btn" data-act="close" title="Close this workspace">${icon('x', 18)}</button>
    </div>
    <div class="dbx-main">
      <aside class="dbx-tables">
        <div class="seg dbx-ltabs"><button data-t="tables" class="on">Tables</button><button data-t="params">Parameters</button></div>
        <div class="dbx-lp" data-p="tables">
          <input class="dbx-find" placeholder="Find a table or column">
          <div class="dbx-tlist"><div class="dbx-note">Loading tables…</div></div>
          <div class="dbx-tip">Drag a table or column into the editor · right-click for queries</div>
        </div>
        <div class="dbx-lp" data-p="params" hidden>
          <div class="dbx-plist"></div>
          <div class="dbx-tip">Click or drag a parameter into the query; Quire fills in its value when the query runs. Type <code>:</code> in the editor to pick one. Values follow the report's period and parameters.</div>
        </div>
      </aside>
      <div class="dbx-split v l" data-split="left" title="Drag to resize · double-click to reset"></div>
      <section class="dbx-work">
        <div class="dbx-bar">
          <div class="seg dbx-lang" title="SQL on the database, or Python on the last result (df)"><button data-l="sql" class="on">SQL</button>${S.st?.python_off ? '' : `<button data-l="py">${icon('python', 13)} Python</button>`}</div>
          <button class="btn primary dbx-run" data-act="run" title="Run (${K('⌘↵')}) — runs the selection when there is one">${icon('play', 14)} <span>Run</span> <kbd>${K('⌘↵')}</kbd></button>
          <label class="dbx-limit" title="How many rows to show">Rows <select>${LIMITS.map((n) => `<option value="${n}"${n === prefs.limit ? ' selected' : ''}>${fmtNum(n, 'number')}</option>`).join('')}</select></label>
          <span class="dbx-sep"></span>
          <button class="tb-btn" data-act="undo" title="Undo (${K('⌘Z')})">${icon('undo', 16)}</button>
          <button class="tb-btn" data-act="redo" title="Redo (${K('⇧⌘Z')})">${icon('redo', 16)}</button>
          <button class="tb-btn" data-act="format" title="Format SQL (${K('⇧⌘F')})">${icon('wand', 16)}</button>
          <button class="tb-btn" data-act="savefile" title="Save script as .sql (${K('⌘S')})">${icon('save', 16)}</button>
          <span class="dbx-status"></span>
          <span class="grow"></span>
          <span class="dbx-pend" hidden><b></b><button class="btn sm" data-act="revert" disabled>${icon('undo', 13)} Revert</button><button class="btn sm primary" data-act="save-edits" disabled>${icon('save', 13)} Save</button></span>
          <button class="btn sm" data-act="edit"${canWrite ? '' : ' hidden'} title="Change values of the queried table and save them to the database">${icon('rename', 14)} Edit</button>
          <button class="tb-btn" data-act="pane-left" title="Show or hide the tables pane (${K('⌘[')})">${icon('sidebar-left', 16)}</button>
          <button class="tb-btn" data-act="pane-right" title="Show or hide the row / column pane (${K('⌘]')})">${icon('sidebar-right', 16)}</button>
          <button class="tb-btn" data-act="max" title="Maximize the editor (${K('⇧⌘M')})">${icon('maximize', 16)}</button>
          <button class="btn sm" data-act="save"${canSave ? '' : ' hidden'} disabled title="${dataset ? 'Save the query to this data source' : 'Add the query as a data source of the report'}">${icon('plus', 14)} ${dataset ? 'Save source' : 'Add as source'}</button>
        </div>
        <div class="dbx-ed"></div>
        <div class="dbx-py" hidden><div class="dbx-pyed"></div><div class="dbx-py-bar"><span class="muted">${icon('info', 12)} <code>df</code> is the last SQL result; end with an expression to see it as a table. <code>params</code> holds the report's values.</span></div></div>
        <div class="dbx-split h" data-split="ed" title="Drag to resize · double-click to reset"></div>
        <div class="dbx-hint">Joins, CTEs (with …) and window functions run in the database. Parameters (<code>:period_start</code>, <code>:client</code>… — see <b>Parameters</b> on the left) are filled in with the report's values. ${K('⌃Space')} completes names.</div>
        <div class="dbx-out">
          <div class="dbx-result"><div class="dbx-note">Write a query and press Run, or right-click a table on the left.</div></div>
          <div class="dbx-split v r" data-split="right" title="Drag to resize · double-click to reset"></div>
          <aside class="dbx-stats" hidden></aside>
        </div>
      </section>
      <div class="dbx-split v a" data-split="ai" hidden title="Drag to resize · double-click to reset"></div>
      <aside class="dbx-aip" hidden></aside>
    </div>
  </div>`);
  const $ = (s) => el.querySelector(s);
  const status = $('.dbx-status');
  const result = $('.dbx-result');
  const statsEl = $('.dbx-stats');
  const saveBtn = $('[data-act="save"]');
  let last = null; // the last successful result
  let closeIt = () => {};
  let tables = [];
  let params = []; // the :names queries can use, with their values now
  let file = null;
  let curEd = null; // the editor showing: SQL (null = ed) or Python // the .sql file the script was opened from or saved to
  let savedText = '';

  const applyFonts = () => {
    el.style.setProperty('--dbx-ed-font', prefs.edFont ? `"${prefs.edFont}", var(--font-mono)` : 'var(--font-mono)');
    el.style.setProperty('--dbx-ed-size', `${prefs.edSize}px`);
    el.style.setProperty('--dbx-out-font', prefs.outFont ? `"${prefs.outFont}", var(--font-ui, system-ui)` : 'inherit');
    el.style.setProperty('--dbx-out-size', `${prefs.outSize}px`);
  };
  applyFonts();

  // ---- pane sizes (dragged, kept on this computer) and which panes show
  const applySizes = () => {
    el.style.setProperty('--dbx-left', `${prefs.left || 260}px`);
    el.style.setProperty('--dbx-right', `${prefs.right || 260}px`);
    el.style.setProperty('--dbx-ai', `${prefs.aiW || 400}px`);
    el.style.setProperty('--dbx-ed-h', prefs.edH ? `${prefs.edH}px` : '34%');
    el.classList.toggle('no-tables', !!prefs.noTables);
  };
  applySizes();
  el.querySelectorAll('.dbx-split').forEach((sp) => {
    sp.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      sp.setPointerCapture(e.pointerId);
      sp.classList.add('drag');
      const kind = sp.dataset.split, x0 = e.clientX, y0 = e.clientY;
      const start = kind === 'left' ? $('.dbx-tables').offsetWidth : kind === 'right' ? statsEl.offsetWidth : kind === 'ai' ? $('.dbx-aip').offsetWidth : $('.dbx-ed').offsetHeight;
      const clamp = (v, a, b) => Math.max(a, Math.min(b, v));
      const move = (ev) => {
        if (kind === 'left') prefs.left = clamp(start + ev.clientX - x0, 160, 560);
        else if (kind === 'ai') prefs.aiW = clamp(start - (ev.clientX - x0), 300, Math.max(320, el.clientWidth - 640));
        else if (kind === 'right') prefs.right = clamp(start - (ev.clientX - x0), 180, Math.max(200, $('.dbx-out').clientWidth - 240));
        else prefs.edH = clamp(start + ev.clientY - y0, 70, Math.max(80, $('.dbx-work').clientHeight - 160));
        applySizes();
      };
      const up = () => {
        sp.removeEventListener('pointermove', move);
        sp.classList.remove('drag');
        savePrefs(prefs);
      };
      sp.addEventListener('pointermove', move);
      sp.addEventListener('pointerup', up, { once: true });
      sp.addEventListener('pointercancel', up, { once: true });
    });
    sp.addEventListener('dblclick', () => {
      const kind = sp.dataset.split;
      if (kind === 'left') delete prefs.left; else if (kind === 'right') delete prefs.right; else if (kind === 'ai') delete prefs.aiW; else delete prefs.edH;
      applySizes();
      savePrefs(prefs);
    });
  });
  const toggleTablesPane = () => { prefs.noTables = !prefs.noTables; savePrefs(prefs); applySizes(); };
  const toggleRightPane = () => {
    prefs.stats = !prefs.stats;
    savePrefs(prefs);
    if (!prefs.stats) statsEl.hidden = true;
    else if (last) (pane === 'row' && anchor >= 0 ? showRow(anchor) : pickColumn(Math.max(0, selCol)));
    paintToggles();
  };
  const paintToggles = () => {
    $('[data-act="pane-left"]').classList.toggle('on', !prefs.noTables);
    $('[data-act="pane-right"]').classList.toggle('on', !!prefs.stats);
  };

  // ---- left pane: tables, or the parameters with their current values
  el.querySelectorAll('.dbx-ltabs button').forEach((b) => b.addEventListener('click', () => {
    el.querySelectorAll('.dbx-ltabs button').forEach((x) => x.classList.toggle('on', x === b));
    el.querySelectorAll('.dbx-lp').forEach((x) => { x.hidden = x.dataset.p !== b.dataset.t; });
    if (b.dataset.t === 'params') refreshParams();
  }));
  const refreshParams = async () => {
    params = await call('db_params', {}, { quiet: true }).catch(() => params);
    const box = $('.dbx-plist');
    const item = (p) => p.bad ? `<div class="dbx-p bad" title="${esc(p.about)}"><code>${esc(p.name)}</code><b>can’t be used</b><span>${esc(p.about)}</span></div>` : `<button class="dbx-p" draggable="true" data-n="${esc(p.name)}" title="${esc(p.about)} · click to insert">
      <code>:${esc(p.name)}</code><b>${esc(p.value || '—')}</b><span>${esc(p.about)}</span></button>`;
    const groups = [
      ['Dates', params.filter((p) => !p.param && !p.var)],
      ['Report parameters', params.filter((p) => p.param)],
      ['Variables', params.filter((p) => p.var)],
    ];
    box.innerHTML = groups.map(([t, ps]) => `<div class="dbx-pg">${esc(t)}</div>${ps.map(item).join('') || `<div class="dbx-note small">${t === 'Variables' ? 'None yet. Variables you add (Variables tab) appear here when their name is a plain word, e.g. min_value.' : 'None.'}</div>`}`).join('')
      + `<button class="btn sm link dbx-phelp">${icon('info', 13)} :data_start or :period_start — which to use?</button>`;
    box.querySelector('.dbx-phelp').addEventListener('click', aboutParams);
    box.querySelectorAll('button.dbx-p').forEach((b) => {
      b.addEventListener('click', () => ed.insert(`:${b.dataset.n}`));
      b.addEventListener('dragstart', (e) => { e.dataTransfer.setData('text/x-sql', `:${b.dataset.n}`); e.dataTransfer.setData('text/plain', `:${b.dataset.n}`); e.dataTransfer.effectAllowed = 'copy'; });
    });
  };

  // ---- the editor
  const ed = sqlEditor({
    value: dataset?.source?.query || '',
    tables: () => tables,
    params: () => params,
    onRun: (text, isSel) => run(text, isSel),
    onChange: () => updateBar(),
    onContext: (e) => editorMenu({ x: e.clientX, y: e.clientY }),
  });
  // the AI assistant: a conversation in its own pane on the right
  const assistant = sqlAssistant({
    pane: $('.dbx-aip'), conn: c, ed, run: (q) => run(q, false),
    saveAs: async (q, hint) => { ed.set(q); await run(q, false); if (last?.sql === q.trim()) saveSql(false, hint); },
    onToggle: (open) => { $('[data-split="ai"]').hidden = !open; prefs.aiOpen = open; savePrefs(prefs); paintAIButton(); },
  });
  registerAI(key, assistant);
  if (prefs.aiOpen) setTimeout(() => assistant.open());
  ed.fmt.upper = prefs.upper;
  $('.dbx-ed').append(ed.el);
  savedText = ed.value;
  ed.ta.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
  });
  ed.ta.addEventListener('select', updateBar);
  ed.ta.addEventListener('keyup', updateBar);
  ed.ta.addEventListener('mouseup', updateBar);
  function updateBar() {
    const E = curEd || ed;
    const py = !!curEd;
    const sel = py ? '' : ed.selection().trim();
    $('.dbx-run span').textContent = sel ? 'Run selection' : 'Run';
    $('[data-act="undo"]').disabled = !E.canUndo();
    $('[data-act="redo"]').disabled = !E.canRedo();
    $('[data-act="format"]').disabled = py;
    const f = py ? pyFile : file;
    const name = f ? f.split('/').pop() : '';
    $('.dbx-file').textContent = name ? `${name}${!py && ed.value !== savedText ? ' •' : ''}` : '';
    $('.dbx-file').title = file || '';
  }

  /** Puts a query in the editor: asks before replacing what is there. */
  const putQuery = async (text, { runIt = true } = {}) => {
    const cur = ed.value.trim();
    if (cur && cur !== text.trim()) {
      const v = await modal({ title: 'Replace the query in the editor?', iconName: 'warn', iconKind: 'warn',
        body: `<p>The editor already has a query. Replace it with this one, or insert this one at the cursor?</p><pre class="dbx-pre">${esc(text)}</pre>`,
        buttons: [{ label: 'Cancel', value: null }, { label: 'Insert at cursor', value: 'insert' }, { label: 'Replace', value: 'replace', kind: 'primary' }] });
      if (!v) return;
      if (v === 'insert') { ed.insert((ed.ta.selectionStart > 0 ? '\n\n' : '') + text); return; }
    }
    ed.set(text);
    if (runIt) run(text, false);
  };

  // ---- tables pane: click opens a table's columns; right-click for queries; drag names into the editor
  const renderTables = () => {
    const q = $('.dbx-find').value.trim().toLowerCase();
    const box = $('.dbx-tlist');
    box.replaceChildren();
    const shown = tables.filter((t) => !q || t.name.toLowerCase().includes(q) || t.columns.some((x) => x.name.toLowerCase().includes(q)));
    if (!shown.length) box.append(h(`<div class="dbx-note">${tables.length ? 'Nothing matches.' : 'No tables in this database.'}</div>`));
    for (const t of shown) {
      const open = q && !t.name.toLowerCase().includes(q);
      const item = h(`<div class="dbx-t${open ? ' open' : ''}">
        <button class="dbx-tn" draggable="true" title="Click to show columns · drag into the editor · right-click for queries"><span class="chev">${icon('chevr', 12)}</span>${icon(t.kind === 'view' ? 'eye' : 'table', 13)}<b>${esc(t.name)}</b><span class="n">${t.rows != null ? fmtNum(t.rows, 'number', { compact: t.rows >= 1e5 }) : ''}</span></button>
        <div class="dbx-cols">${t.columns.map((x) => `<button class="dbx-c" draggable="true" data-c="${esc(x.name)}" title="Double-click to insert · drag into the editor · right-click for queries"><span>${esc(x.name)}</span><i>${esc(x.type)}</i></button>`).join('')}</div>
      </div>`);
      const tn = item.querySelector('.dbx-tn');
      tn.addEventListener('click', () => item.classList.toggle('open'));
      tn.addEventListener('dblclick', () => ed.insert(tname(t)));
      tn.addEventListener('contextmenu', (e) => { e.preventDefault(); tableMenu({ x: e.clientX, y: e.clientY }, t); });
      tn.addEventListener('dragstart', (e) => { e.dataTransfer.setData('text/x-sql', tname(t)); e.dataTransfer.setData('text/plain', tname(t)); e.dataTransfer.effectAllowed = 'copy'; });
      item.querySelectorAll('.dbx-c').forEach((b) => {
        const col = t.columns.find((x) => x.name === b.dataset.c);
        b.addEventListener('dblclick', () => ed.insert(qid(col.name)));
        b.addEventListener('contextmenu', (e) => { e.preventDefault(); columnMenu({ x: e.clientX, y: e.clientY }, t, col); });
        b.addEventListener('dragstart', (e) => { e.dataTransfer.setData('text/x-sql', qid(col.name)); e.dataTransfer.setData('text/plain', qid(col.name)); e.dataTransfer.effectAllowed = 'copy'; });
      });
      box.append(item);
    }
  };
  $('.dbx-find').addEventListener('input', renderTables);
  const copy = (text) => navigator.clipboard?.writeText(text).then(() => toast('Copied.', 'info', 1200)).catch(() => toast('Could not copy.', 'error'));
  const n100 = Math.min(prefs.limit, 100);
  function tableMenu(at, t) {
    const T = tname(t);
    menu(at, [
      { title: t.name },
      { label: `select * (first ${n100} rows)`, icon: 'play', run: () => putQuery(`select *\nfrom ${T}\nlimit ${n100}`) },
      { label: 'select with every column', icon: 'table', run: () => putQuery(`select\n${t.columns.map((x) => `  ${qid(x.name)}`).join(',\n')}\nfrom ${T}\nlimit ${n100}`) },
      { label: 'Count rows', icon: 'hash', run: () => putQuery(`select count(*) as rows\nfrom ${T}`) },
      { label: 'Profile columns (min, max, distinct)', icon: 'stats', run: () => putQuery(`summarize ${T}`), disabled: c.kind !== 'duckdb' },
      { sep: true },
      { label: 'Insert name at cursor', icon: 'plus', run: () => ed.insert(T) },
      { label: 'Insert column list at cursor', icon: 'plus', run: () => ed.insert(t.columns.map((x) => qid(x.name)).join(', ')) },
      { label: 'Copy name', icon: 'copy', run: () => copy(T) },
    ]);
  }
  function columnMenu(at, t, col) {
    const T = tname(t), C = qid(col.name);
    menu(at, [
      { title: `${t.name}.${col.name} · ${col.type}` },
      { label: 'Distinct values with counts', icon: 'sort-desc', run: () => putQuery(`select ${C}, count(*) as n\nfrom ${T}\ngroup by 1\norder by n desc\nlimit ${n100}`) },
      { label: 'Min, max, nulls', icon: 'sigma', run: () => putQuery(`select\n  count(*) as rows,\n  count(${C}) as filled,\n  count(distinct ${C}) as distinct_values,\n  min(${C}) as min,\n  max(${C}) as max\nfrom ${T}`) },
      { sep: true },
      { label: 'Insert name at cursor', icon: 'plus', run: () => ed.insert(C) },
      { label: 'Insert as table.column', icon: 'plus', run: () => ed.insert(`${T}.${C}`) },
      { label: 'Copy name', icon: 'copy', run: () => copy(C) },
    ]);
  }

  // ---- running
  const DEC = fmtNum(1.5, 'number', { decimals: 1 }).includes(',') ? ',' : '.';
  const cell = cellHtml;
  let selCol = -1;
  let selRows = new Set();
  let anchor = -1;
  let pane = 'col'; // what the right pane shows: a column's figures or a row's values
  let editing = false; // edit mode: cells of the queried table can be changed
  const pending = new Map(); // "row|col" → new value
  let warnedEdit = false;

  /** The table a query reads, when its rows can be edited: one table, no joins, groups or windows. */
  const editTarget = (text) => {
    const q = text.trim().replace(/;\s*$/, '');
    const bare = q.replace(/'(?:[^']|'')*'/g, "''").replace(/--[^\n]*/g, '');
    if (/\b(join|group\s+by|distinct|union|intersect|except|having|over|qualify|with|pivot|summarize|describe)\b/i.test(bare) || /;/.test(bare)) return null;
    const m = bare.match(/^select\s+[\s\S]+?\s+from\s+((?:"[^"]+"|[\w]+)(?:\.(?:"[^"]+"|[\w]+))?)(?:\s+(?:as\s+)?("[^"]+"|(?!where\b|order\b|limit\b|offset\b)[a-z_]\w*))?\s*(?:(?:where|order|limit|offset)\b[\s\S]*)?$/i);
    if (!m || /\bfrom\b[\s\S]*,/.test(bare.replace(/\(([^()]|\([^()]*\))*\)/g, '').split(/\bfrom\b/i)[1]?.split(/\b(where|order|limit)\b/i)[0] || '')) return null;
    const parts = m[1].split('.').map((x) => x.replace(/^"|"$/g, ''));
    const name = parts.pop(), schema = parts.pop() || '';
    const t = tables.find((x) => x.name.toLowerCase() === name.toLowerCase() && (!schema || x.schema === schema));
    if (!t || t.kind === 'view') return null;
    const alias = m[2] ? m[2] : '';
    // ClickHouse has no row ids: its results are read-only here
    if (c.kind === 'clickhouse') return null;
    const key = c.kind === 'postgres' ? 'ctid::text' : 'rowid';
    return { table: t, sql: q.replace(/^(\s*select\s+)/i, `$1${alias ? `${alias}.` : ''}${key} as __qrow, `) };
  };

  // the result's columns that are the table's own (those can be edited), by index
  const editCols = () => {
    if (!last?.edit) return new Map();
    const byName = new Map(last.edit.table.columns.map((x) => [x.name.toLowerCase(), x]));
    const m = new Map();
    last.r.columns.forEach((x, j) => { const tc = byName.get(x.name.toLowerCase()); if (tc) m.set(j, tc); });
    return m;
  };
  const valueAt = (i, j) => (pending.has(`${i}|${j}`) ? pending.get(`${i}|${j}`) : last.r.rows[i][j]);

  const renderGrid = () => {
    const r = last.r;
    const ec = editing ? editCols() : new Map();
    const grid = h(`<div class="dbx-grid${editing ? ' editing' : ''}" tabindex="0"><table><thead><tr><th class="rn" title="Select all rows">#</th>${r.columns.map((x, j) => `<th class="k-${x.kind}${ec.has(j) ? ' ed' : ''}" data-j="${j}" title="Click for this column's figures"><span>${esc(x.name)}${ec.has(j) ? ` ${icon('rename', 10)}` : ''}</span><i>${esc(x.kind)}</i></th>`).join('')}</tr></thead><tbody></tbody></table></div>`);
    grid.querySelector('tbody').innerHTML = r.rows.map((row, i) => `<tr data-i="${i}"><td class="rn">${i + 1}</td>${row.map((_, j) => {
      const v = valueAt(i, j);
      return `<td class="k-${r.columns[j].kind}${pending.has(`${i}|${j}`) ? ' changed' : ''}" data-j="${j}">${cell(v, r.columns[j].kind)}</td>`;
    }).join('')}</tr>`).join('');
    grid.querySelector('thead').addEventListener('click', (e) => {
      const th = e.target.closest('th');
      if (th?.classList.contains('rn')) selectAll();
      else if (th) pickColumn(+th.dataset.j);
    });
    grid.addEventListener('mousedown', (e) => {
      const tr = e.target.closest('tbody tr');
      if (!tr || e.target.closest('input')) return;
      const i = +tr.dataset.i;
      if (e.shiftKey && anchor >= 0) { selRows = new Set(); for (let k = Math.min(anchor, i); k <= Math.max(anchor, i); k++) selRows.add(k); }
      else if (e.metaKey || e.ctrlKey) { selRows.has(i) ? selRows.delete(i) : selRows.add(i); anchor = i; }
      else if (!(e.button === 2 && selRows.has(i))) { selRows = new Set([i]); anchor = i; }
      if (e.shiftKey) e.preventDefault();
      paintSel();
      showRow(i);
    });
    grid.addEventListener('dblclick', (e) => {
      const td = e.target.closest('tbody td[data-j]');
      if (!td) return;
      if (!editing) { toast('Turn on Edit to change values.', 'info', 1600); return; }
      editCell(td);
    });
    grid.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const td = e.target.closest('tbody td[data-j]');
      gridMenu({ x: e.clientX, y: e.clientY }, td);
    });
    grid.addEventListener('keydown', (e) => {
      const mod = e.metaKey || e.ctrlKey;
      if (e.target.closest('input')) return;
      if (mod && e.key.toLowerCase() === 'a') { e.preventDefault(); selectAll(); }
      else if (mod && e.key.toLowerCase() === 'c') { e.preventDefault(); copyRows(); }
      else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && last.r.rows.length) {
        e.preventDefault();
        const i = Math.max(0, Math.min(last.r.rows.length - 1, (anchor < 0 ? -1 : anchor) + (e.key === 'ArrowDown' ? 1 : -1)));
        if (e.shiftKey) selRows.add(i); else selRows = new Set([i]);
        anchor = i;
        paintSel();
        showRow(i);
        grid.querySelector(`tr[data-i="${i}"]`)?.scrollIntoView({ block: 'nearest' });
      }
    });
    result.replaceChildren(grid);
    paintSel();
  };
  const paintSel = () => {
    result.querySelectorAll('tbody tr').forEach((tr) => tr.classList.toggle('rsel', selRows.has(+tr.dataset.i)));
    result.querySelectorAll('[data-j].sel').forEach((x) => x.classList.remove('sel'));
    if (selCol >= 0 && pane === 'col') result.querySelectorAll(`[data-j="${selCol}"]`).forEach((x) => x.classList.add('sel'));
  };
  const selectAll = () => {
    selRows = new Set(last.r.rows.map((_, i) => i));
    paintSel();
    status.dataset.keep = status.textContent;
    toast(`${fmtNum(selRows.size, 'number')} rows selected · ${K('⌘C')} copies them as a table.`, 'info', 1800);
    result.querySelector('.dbx-grid')?.focus();
  };

  // ---- copy rows as a table (text for anywhere, HTML for spreadsheets and documents)
  const copyRows = async (only = null) => {
    if (!last) return;
    const rows = (only || (selRows.size ? [...selRows].sort((a, b) => a - b) : last.r.rows.map((_, i) => i)));
    const cols = last.r.columns;
    const plain = (v, kind) => (v == null ? '' : kind === 'number' && typeof v === 'number' ? String(v).replace('.', DEC) : kind === 'date' ? fmtDate(v) : String(v));
    const tsv = [cols.map((x) => x.name).join('\t'), ...rows.map((i) => cols.map((x, j) => plain(valueAt(i, j), x.kind).replace(/[\t\n]/g, ' ')).join('\t'))].join('\n');
    const html = `<table><thead><tr>${cols.map((x) => `<th>${esc(x.name)}</th>`).join('')}</tr></thead><tbody>${rows.map((i) => `<tr>${cols.map((x, j) => `<td${x.kind === 'number' ? ' style="text-align:right"' : ''}>${esc(plain(valueAt(i, j), x.kind))}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
    try {
      if (window.ClipboardItem) await navigator.clipboard.write([new ClipboardItem({ 'text/plain': new Blob([tsv], { type: 'text/plain' }), 'text/html': new Blob([html], { type: 'text/html' }) })]);
      else await navigator.clipboard.writeText(tsv);
      toast(`Copied ${fmtNum(rows.length, 'number')} row${rows.length === 1 ? '' : 's'} × ${cols.length} columns${last.r.more && rows.length === last.r.rows.length ? ` (the ${fmtNum(rows.length, 'number')} shown — raise Rows for more)` : ''}. Paste into a spreadsheet or document.`, 'info', 2600);
    } catch {
      toast('Could not copy to the clipboard.', 'error');
    }
  };

  // ---- editing cells
  const parseInput = (text, kind) => {
    const t = text.trim();
    if (kind === 'number') {
      if (!t) return { v: null };
      const n = Number(t.replace(/[\s ]/g, '').replace(',', '.'));
      return Number.isFinite(n) ? { v: n } : { err: 'Enter a number.' };
    }
    if (kind === 'date') {
      if (!t) return { v: null };
      let m = t.match(/^(\d{4})-(\d{2})-(\d{2})/);
      if (m) return { v: `${m[1]}-${m[2]}-${m[3]}` };
      m = t.match(/^(\d{1,2})[./](\d{1,2})[./](\d{4})$/);
      if (m) return { v: `${m[3]}-${m[2].padStart(2, '0')}-${m[1].padStart(2, '0')}` };
      return { err: 'Enter a date as dd.mm.yyyy or yyyy-mm-dd.' };
    }
    return { v: text };
  };
  const setValue = (i, j, v) => {
    const orig = last.r.rows[i][j];
    const same = v === orig || (v != null && orig != null && String(v) === String(orig));
    if (same) pending.delete(`${i}|${j}`);
    else pending.set(`${i}|${j}`, v);
    const td = result.querySelector(`tr[data-i="${i}"] td[data-j="${j}"]`);
    if (td) { td.innerHTML = cell(valueAt(i, j), last.r.columns[j].kind); td.classList.toggle('changed', pending.has(`${i}|${j}`)); }
    updateEditBar();
    if (pane === 'row' && anchor === i) showRow(i);
  };
  const editCell = (td) => {
    const i = +td.closest('tr').dataset.i, j = +td.dataset.j;
    const ec = editCols();
    if (!ec.has(j)) { toast(`“${last.r.columns[j].name}” isn’t a column of ${last.edit.table.name}, so it can’t be edited.`, 'info', 2200); return; }
    const kind = last.r.columns[j].kind;
    const v = valueAt(i, j);
    const inp = h(`<input class="dbx-cell-in" value="${esc(v == null ? '' : kind === 'date' ? String(v).slice(0, 10) : String(v))}">`);
    td.replaceChildren(inp);
    inp.focus();
    inp.select();
    let done = false;
    const finish = (save, move = 0) => {
      if (done) return;
      if (save) {
        const p = parseInput(inp.value, kind);
        if (p.err) { toast(p.err, 'error', 1800); inp.focus(); return; }
        done = true;
        setValue(i, j, p.v);
      } else {
        done = true;
        td.innerHTML = cell(valueAt(i, j), kind);
      }
      if (move) {
        const cols = [...ec.keys()];
        const next = cols[(cols.indexOf(j) + move + cols.length) % cols.length];
        const nt = result.querySelector(`tr[data-i="${i}"] td[data-j="${next}"]`);
        if (nt) editCell(nt);
      } else result.querySelector('.dbx-grid')?.focus();
    };
    inp.addEventListener('keydown', (e) => {
      e.stopPropagation();
      if (e.key === 'Enter') { e.preventDefault(); finish(true); }
      if (e.key === 'Escape') { e.preventDefault(); finish(false); }
      if (e.key === 'Tab') { e.preventDefault(); finish(true, e.shiftKey ? -1 : 1); }
    });
    inp.setAttribute('data-own-esc', '');
    inp.addEventListener('blur', () => finish(true));
  };
  const updateEditBar = () => {
    const bar = $('.dbx-pend');
    bar.hidden = !editing;
    $('[data-act="edit"]').classList.toggle('on', editing);
    $('.dbx-pend b').textContent = pending.size ? `${pending.size} change${pending.size === 1 ? '' : 's'}` : `Editing ${last?.edit?.table.name || ''} · double-click a cell`;
    $('[data-act="save-edits"]').disabled = !pending.size;
    $('[data-act="revert"]').disabled = !pending.size;
  };
  const discardOk = async (what) => {
    if (!pending.size) return true;
    return modal({ title: 'Discard your changes?', iconName: 'warn', iconKind: 'warn', body: `<p>${pending.size} changed value${pending.size === 1 ? ' isn’t' : 's aren’t'} saved to the database yet. ${esc(what)} discards ${pending.size === 1 ? 'it' : 'them'}.</p>`,
      buttons: [{ label: 'Keep editing', value: false }, { label: 'Discard', value: true, kind: 'danger' }] });
  };
  const toggleEdit = async () => {
    if (!canWrite) { toast('Changing values in the database needs the Data role.', 'info', 3000); return; }
    if (editing) {
      if (!(await discardOk('Leaving edit mode'))) return;
      editing = false;
      pending.clear();
      updateEditBar();
      if (last) run(last.sql, false);
      return;
    }
    const text = last?.sql || ed.value;
    const t = editTarget(text);
    if (!t) {
      toast('Only rows of one table can be edited: run a query like select … from a table (where … order by … limit …), without joins, groups or windows.', 'info', 4200);
      return;
    }
    if (!warnedEdit) {
      const ok = await modal({ title: `Edit ${t.table.name}?`, iconName: 'rename', body: `<p>You can change values in the result, then <b>Save</b> writes them to <b>${esc(t.table.name)}</b> in the database itself, or <b>Revert</b> drops them. Data sources that read this table show the new values.</p>`,
        buttons: [{ label: 'Cancel', value: false }, { label: 'Edit', value: true, kind: 'primary' }] });
      if (!ok) return;
      warnedEdit = true;
    }
    editing = true;
    updateEditBar();
    run(text, false);
  };
  const saveEdits = async () => {
    if (!pending.size || !last?.edit) return;
    const t = last.edit.table;
    const ec = editCols();
    const edits = [...pending].map(([k, v]) => { const [i, j] = k.split('|').map(Number); return { key: last.keys[i], column: ec.get(j).name, value: v, type: ec.get(j).type }; });
    const rowsN = new Set([...pending.keys()].map((k) => k.split('|')[0])).size;
    const ok = await modal({ title: 'Save changes to the database?', iconName: 'database', body: `<p>${edits.length} value${edits.length === 1 ? '' : 's'} in ${rowsN} row${rowsN === 1 ? '' : 's'} of <b>${esc(t.name)}</b> will be written to “${esc(c.name)}”. This changes the database itself.</p>`,
      buttons: [{ label: 'Cancel', value: false }, { label: 'Save', value: true, kind: 'primary' }] });
    if (!ok) return;
    try {
      const r = await call('db_apply', { connection: c, schema: t.schema || '', table: t.name, edits }, { quiet: true });
      pending.clear();
      toast(`Saved ${r.saved} value${r.saved === 1 ? '' : 's'} to ${t.name}.`, 'ok');
      await run(last.sql, false);
      api.refresh().catch(() => {}); // the report reloads its data
    } catch (e) {
      modal({ title: 'Nothing was saved', iconName: 'warn', iconKind: 'warn', body: `<pre class="dbx-pre">${esc(String(e))}</pre>`, buttons: [{ label: 'OK', value: true, kind: 'primary' }] });
    }
  };
  const revertEdits = () => { pending.clear(); renderGrid(); updateEditBar(); if (pane === 'row' && anchor >= 0) showRow(anchor); };

  // ---- the right pane: a row's values (editable in edit mode)
  const showRow = (i) => {
    if (!last || !prefs.stats) return;
    pane = 'row';
    paintSel();
    const r = last.r;
    const ec = editing ? editCols() : new Map();
    statsEl.hidden = false;
    const n = selRows.size;
    statsEl.innerHTML = `<div class="dbx-st-head"><div><b>Row ${i + 1} <small>of ${fmtNum(r.rows.length, 'number')}</small></b>${n > 1 ? `<span>${fmtNum(n, 'number')} rows selected</span>` : ''}</div>
      <button class="tb-btn" data-act="copy-row" title="Copy ${n > 1 ? 'the selected rows' : 'this row'} as a table">${icon('copy', 14)}</button>
      <button class="tb-btn" data-act="hide-stats" title="Hide">${icon('x', 14)}</button></div>
      <div class="dbx-rowv">${r.columns.map((x, j) => {
        const v = valueAt(i, j);
        const changed = pending.has(`${i}|${j}`);
        const val = ec.has(j)
          ? `<input data-j="${j}" class="k-${x.kind}${changed ? ' changed' : ''}" value="${esc(v == null ? '' : x.kind === 'date' ? String(v).slice(0, 10) : String(v))}" placeholder="null">`
          : `<div class="v k-${x.kind}${changed ? ' changed' : ''}">${cell(v, x.kind)}</div>`;
        return `<label class="dbx-rv" title="${esc(x.name)} · ${esc(x.kind)}"><span>${esc(x.name)}</span>${val}</label>`;
      }).join('')}</div>`;
    statsEl.querySelector('[data-act="hide-stats"]').addEventListener('click', () => { statsEl.hidden = true; prefs.stats = false; savePrefs(prefs); paintToggles(); });
    statsEl.querySelector('[data-act="copy-row"]').addEventListener('click', () => copyRows(n > 1 ? null : [i]));
    statsEl.querySelectorAll('input[data-j]').forEach((inp) => {
      inp.setAttribute('data-own-esc', '');
      const commit = () => {
        const j = +inp.dataset.j;
        const p = parseInput(inp.value, r.columns[j].kind);
        if (p.err) { toast(p.err, 'error', 1800); return; }
        setValue(i, j, p.v);
      };
      inp.addEventListener('change', commit);
      inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); commit(); } });
    });
  };

  const gridMenu = (at, td) => {
    const i = td ? +td.closest('tr').dataset.i : -1, j = td ? +td.dataset.j : -1;
    const ec = editing ? editCols() : new Map();
    const n = selRows.size;
    menu(at, [
      { label: n > 1 ? `Copy ${fmtNum(n, 'number')} selected rows` : 'Copy row', icon: 'copy', hint: K('⌘C'), disabled: !n, run: () => copyRows() },
      { label: 'Copy all rows as a table', icon: 'table', run: () => copyRows(last.r.rows.map((_, k) => k)) },
      { label: 'Copy value', icon: 'copy', disabled: !td, run: () => copy(valueAt(i, j) == null ? '' : String(valueAt(i, j))) },
      { label: 'Select all rows', hint: K('⌘A'), run: selectAll },
      ...(editing ? [
        { sep: true },
        { label: 'Edit value', icon: 'rename', disabled: !td || !ec.has(j), run: () => editCell(td) },
        { label: 'Set to null', disabled: !td || !ec.has(j), run: () => setValue(i, j, null) },
        { label: 'Revert this value', disabled: !td || !pending.has(`${i}|${j}`), run: () => setValue(i, j, last.r.rows[i][j]) },
      ] : canWrite ? [{ sep: true }, { label: 'Edit values…', icon: 'rename', run: toggleEdit }] : []),
    ]);
  };

  const run = async (text = ed.value, isSel = false) => {
    text = text.trim();
    if (!text) return;
    if (pending.size && !(await discardOk('Running a query'))) return;
    pending.clear();
    let target = editing ? editTarget(text) : null;
    if (editing && !target) { editing = false; updateEditBar(); toast('Edit mode ended: this query doesn’t read rows of one table.', 'info', 2600); }
    status.className = 'dbx-status busy';
    status.textContent = isSel ? 'Running the selection…' : 'Running…';
    refreshParams();
    try {
      const raw = await call('db_run', { connection: c, sql: target ? target.sql : text, limit: prefs.limit }, { quiet: true });
      let r = raw, keys = null;
      if (target) {
        // the row ids stay out of sight
        keys = raw.rows.map((row) => row[0]);
        r = { ...raw, columns: raw.columns.slice(1), rows: raw.rows.map((row) => row.slice(1)) };
      }
      last = { sql: text, r, keys, edit: target };
      assistant.noteRun(text, null);
      saveBtn.disabled = false;
      status.className = 'dbx-status ok';
      status.textContent = `${isSel ? 'Selection · ' : ''}${r.total != null ? fmtNum(r.total, 'number') : `${r.rows.length}${r.more ? '+' : ''}`} row${r.total === 1 ? '' : 's'} · ${r.columns.length} column${r.columns.length === 1 ? '' : 's'} · ${r.ms} ms${r.more ? ` · showing the first ${fmtNum(prefs.limit, 'number')}` : ''}`;
      if (el.classList.contains('max')) toggleMax(false);
      selRows = new Set();
      anchor = -1;
      renderGrid();
      updateEditBar();
      if (prefs.stats && r.columns.length) pickColumn(selCol >= 0 && selCol < r.columns.length ? selCol : 0);
      else statsEl.hidden = true;
    } catch (e) {
      last = null;
      saveBtn.disabled = true;
      status.className = 'dbx-status bad';
      status.textContent = 'Error';
      statsEl.hidden = true;
      const errEl = h(`<div class="dbx-err">${icon('warn', 16)}<pre>${esc(String(e))}</pre><button class="btn sm dbx-fix">${icon('sparkles', 13)} Fix with AI</button></div>`);
      errEl.querySelector('.dbx-fix').addEventListener('click', () => assistant.fix(text, String(e)));
      assistant.noteRun(text, String(e));
      result.replaceChildren(errEl);
    }
  };

  // ---- a column's figures, computed by the database over the whole result
  const pickColumn = async (j) => {
    if (!last) return;
    selCol = j;
    pane = 'col';
    paintSel();
    const col = last.r.columns[j];
    statsEl.hidden = false;
    statsEl.innerHTML = `<div class="dbx-st-head"><div><b>${esc(col.name)}</b><span>${esc(col.kind)}</span></div><button class="tb-btn" data-act="hide-stats" title="Hide">${icon('x', 14)}</button></div><div class="dbx-note">Counting…</div>`;
    statsEl.querySelector('[data-act="hide-stats"]').addEventListener('click', () => { statsEl.hidden = true; prefs.stats = false; savePrefs(prefs); paintToggles(); });
    const C = `"${col.name.replace(/"/g, '""')}"`;
    const src = `(\n${last.sql.replace(/;\s*$/, '')}\n) as q`;
    const num = col.kind === 'number';
    const aggs = [
      ['Rows', 'count(*)'], ['Filled', `count(${C})`], ['Empty (null)', `count(*) - count(${C})`], ['Distinct', `count(distinct ${C})`],
      ['Min', `min(${C})`], ['Max', `max(${C})`],
      ...(num ? [['Sum', `sum(${C})`], ['Average', `avg(${C})`], ...(c.kind === 'duckdb' ? [['Median', `median(${C})`], ['Std deviation', `stddev_samp(${C})`]] : [])] : []),
    ];
    const fmt = (v) => (v == null ? '<span class="null">null</span>' : esc(num || typeof v === 'number' ? fmtNum(v, 'number', { decimals: Number.isInteger(v) ? 0 : 2 }) : col.kind === 'date' ? fmtDate(v) : String(v)));
    try {
      const [a, top] = await Promise.all([
        call('db_run', { connection: c, sql: `select ${aggs.map(([, e], i) => `${e} as a${i}`).join(', ')} from ${src}`, limit: 1 }, { quiet: true }),
        call('db_run', { connection: c, sql: `select ${C} as v, count(*) as n from ${src} group by 1 order by 2 desc, 1 limit 8`, limit: 8 }, { quiet: true }),
      ]);
      if (selCol !== j || pane !== 'col') return;
      const row = a.rows[0] || [];
      const total = row[0] || 0;
      statsEl.querySelector('.dbx-note').replaceWith(h(`<div>
        <div class="dbx-st-sub">All ${fmtNum(total, 'number')} rows of the result</div>
        <table class="dbx-st">${aggs.map(([l], i) => `<tr><th>${esc(l)}</th><td class="${i < 4 || num ? 'k-number' : `k-${col.kind}`}">${i < 4 ? esc(fmtNum(row[i] ?? 0, 'number')) : fmt(row[i])}</td></tr>`).join('')}</table>
        <div class="dbx-st-sub">Most frequent values</div>
        <div class="dbx-st-top">${top.rows.map(([v, n]) => `<div class="dbx-st-v"><span class="k-${col.kind}">${fmt(v)}</span><b>${fmtNum(n, 'number')}</b><i style="width:${total ? Math.max(2, (n / total) * 100) : 0}%"></i></div>`).join('')}</div>
      </div>`));
    } catch (e) {
      if (selCol !== j || pane !== 'col') return;
      statsEl.querySelector('.dbx-note')?.replaceWith(h(`<div class="dbx-err small">${icon('warn', 14)}<pre>${esc(String(e))}</pre></div>`));
    }
  };

  // ---- files
  let pyFile = null; // the .py file the Python was opened from or saved to
  const saveFile = async (asNew = false) => {
    if (lang === 'py') {
      try {
        const p = await call('sql_file_save', { text: pyText.value, path: asNew ? null : pyFile, name: dataset?.name || c.name || 'script', ext: 'py' });
        if (p) { pyFile = p; updateBar(); toast(`Saved ${p.split('/').pop()}.`, 'info', 1600); }
      } catch { /* reported */ }
      return;
    }
    try {
      const p = await call('sql_file_save', { text: ed.value, path: asNew ? null : file, name: dataset?.name || c.name || 'query' });
      if (!p) return;
      file = p;
      savedText = ed.value;
      updateBar();
      toast(`Saved ${p.split('/').pop()}.`, 'info', 1600);
    } catch { /* reported */ }
  };
  const openFile = async () => {
    if (lang === 'py') {
      const f = await call('sql_file_open', { ext: 'py' }).catch(() => null);
      if (f) { pyText.set(f.text); pyFile = f.path; updateBar(); }
      return;
    }
    const f = await call('sql_file_open', {}).catch(() => null);
    if (!f) return;
    await putQuery(f.text, { runIt: false });
    file = f.path;
    savedText = ed.value;
    updateBar();
  };
  const newQuery = async () => {
    if (ed.value.trim() && ed.value !== savedText) {
      const ok = await modal({ title: 'Start a new query?', iconName: 'warn', iconKind: 'warn', body: '<p>The query in the editor isn’t saved. Clear it anyway? (Undo brings it back.)</p>',
        buttons: [{ label: 'Cancel', value: false }, { label: 'Clear', value: true, kind: 'primary' }] });
      if (!ok) return;
    }
    ed.set('');
    file = null;
    savedText = '';
    updateBar();
  };

  // ---- view
  const toggleMax = (on = !el.classList.contains('max')) => {
    el.classList.toggle('max', on);
    $('[data-act="max"]').innerHTML = icon(on ? 'minimize' : 'maximize', 16);
    $('[data-act="max"]').title = on ? 'Restore the tables and results' : 'Maximize the editor';
    (curEd || ed).focus();
  };
  const setLimit = (n) => { prefs.limit = n; savePrefs(prefs); $('.dbx-limit select').value = String(n); };
  $('.dbx-limit select').addEventListener('change', (e) => setLimit(+e.target.value));
  const fontsDialog = async () => {
    const fams = await call('list_fonts', {}, { quiet: true }).catch(() => []);
    const mono = fams.filter((f) => f.monospace).map((f) => f.family);
    const all = fams.map((f) => f.family);
    const opts = (list, cur, def) => `<option value="">${def}</option>${list.map((f) => `<option${f === cur ? ' selected' : ''}>${esc(f)}</option>`).join('')}${cur && !list.includes(cur) ? `<option selected>${esc(cur)}</option>` : ''}`;
    const body = h(`<div class="modal-body form dbx-fonts">
      <div class="sec-title">Editor</div>
      <div class="row2"><label>Font<select name="edFont">${opts(mono.length ? mono : all, prefs.edFont, 'System monospace')}</select></label><label>Size<input name="edSize" type="number" min="9" max="24" step="0.5" value="${prefs.edSize}"></label></div>
      <div class="sec-title">Results</div>
      <div class="row2"><label>Font<select name="outFont">${opts(all, prefs.outFont, 'Interface font')}</select></label><label>Size<input name="outSize" type="number" min="9" max="20" step="0.5" value="${prefs.outSize}"></label></div>
      <pre class="dbx-pre sample">select name, count(*) as n\nfrom clients\ngroup by 1</pre>
    </div>`);
    const q = (n) => body.querySelector(`[name="${n}"]`);
    const preview = () => { const s = body.querySelector('.sample'); s.style.fontFamily = q('edFont').value ? `"${q('edFont').value}", monospace` : ''; s.style.fontSize = `${q('edSize').value}px`; };
    body.addEventListener('input', preview);
    preview();
    const v = await modal({ title: 'Fonts', iconName: 'type', body,
      buttons: [{ label: 'Reset', value: 'reset' }, { label: 'Cancel', value: null }, { label: 'Apply', kind: 'primary', value: () => ({ edFont: q('edFont').value, edSize: +q('edSize').value || 12.5, outFont: q('outFont').value, outSize: +q('outSize').value || 12 }) }] });
    if (!v) return;
    Object.assign(prefs, v === 'reset' ? { edFont: '', edSize: PREFS0.edSize, outFont: '', outSize: PREFS0.outSize } : v);
    savePrefs(prefs);
    applyFonts();
    ed.refresh();
  };
  const format = () => { ed.fmt.upper = prefs.upper; ed.format(); };

  // ---- menus: the menu bar, and right-click in the editor
  const editorMenu = (at) => {
    const sel = !!ed.selection().trim();
    menu(at, [
      { label: sel ? 'Run selection' : 'Run', icon: 'play', hint: K('⌘↵'), run: () => ed.run() },
      { label: sel ? 'Format selection' : 'Format SQL', icon: 'wand', hint: K('⇧⌘F'), run: format },
      { sep: true },
      { label: 'Undo', icon: 'undo', hint: K('⌘Z'), disabled: !ed.canUndo(), run: ed.undo },
      { label: 'Redo', icon: 'redo', hint: K('⇧⌘Z'), disabled: !ed.canRedo(), run: ed.redo },
      { sep: true },
      { label: 'Cut', icon: 'cut', disabled: !sel, run: () => { copy(ed.selection()); ed.replace(''); } },
      { label: 'Copy', icon: 'copy', disabled: !sel, run: () => copy(ed.selection()) },
      { label: 'Paste', icon: 'paste', run: () => navigator.clipboard?.readText().then((t) => ed.insert(t)).catch(() => toast(`Use ${K('⌘V')} to paste.`, 'info')) },
      { label: 'Select all', run: () => { ed.ta.focus(); ed.ta.select(); updateBar(); } },
      { sep: true },
      { label: 'Comment / uncomment lines', icon: 'comment', hint: K('⌘/'), run: ed.toggleComment },
    ]);
  };
  const menus = {
    File: () => [
      { label: 'New query', icon: 'file-new', run: newQuery },
      { label: 'Open .sql file…', icon: 'folder', hint: K('⌘O'), run: openFile },
      { label: 'Save script', icon: 'save', hint: K('⌘S'), run: () => saveFile(false) },
      { label: 'Save script as…', icon: 'save-as', hint: K('⇧⌘S'), run: () => saveFile(true) },
      { sep: true },
      canSave && { label: dataset ? 'Save data source' : 'Add as data source', icon: 'database', disabled: !last, run: () => saveBtn.click() },
    ],
    Edit: () => [
      { label: 'Undo', icon: 'undo', hint: K('⌘Z'), disabled: !ed.canUndo(), run: ed.undo },
      { label: 'Redo', icon: 'redo', hint: K('⇧⌘Z'), disabled: !ed.canRedo(), run: ed.redo },
      { sep: true },
      { label: 'Format SQL', icon: 'wand', hint: K('⇧⌘F'), run: format },
      { title: 'Keywords when formatting' },
      { label: 'lower case', checked: !prefs.upper, run: () => { prefs.upper = false; savePrefs(prefs); } },
      { label: 'UPPER CASE', checked: prefs.upper, run: () => { prefs.upper = true; savePrefs(prefs); } },
      { sep: true },
      { label: 'Comment / uncomment lines', icon: 'comment', hint: K('⌘/'), run: ed.toggleComment },
      { label: 'Indent', hint: 'Tab', run: () => ed.indent(false) },
      { label: 'Outdent', hint: '⇧Tab', run: () => ed.indent(true) },
      { title: 'Results' },
      { label: 'Select all rows', disabled: !last, run: selectAll },
      { label: 'Copy rows as a table', icon: 'copy', disabled: !last, run: () => copyRows() },
      canWrite && { label: editing ? 'Stop editing values' : 'Edit values…', icon: 'rename', run: toggleEdit },
      { label: 'Save changes to the database', icon: 'save', disabled: !pending.size, run: saveEdits },
      { label: 'Revert changes', icon: 'undo', disabled: !pending.size, run: revertEdits },
    ],
    Run: () => [
      { label: 'Run', icon: 'play', hint: K('⌘↵'), run: () => run(ed.value, false) },
      { label: 'Run selection', icon: 'play', disabled: !ed.selection().trim(), run: () => run(ed.selection(), true) },
      { title: 'Rows to show' },
      ...LIMITS.map((n) => ({ label: fmtNum(n, 'number'), checked: prefs.limit === n, run: () => setLimit(n) })),
    ],
    View: () => [
      { label: el.classList.contains('max') ? 'Restore panes' : 'Maximize editor', icon: el.classList.contains('max') ? 'minimize' : 'maximize', hint: K('⇧⌘M'), run: () => toggleMax() },
      { label: 'Tables pane', checked: !prefs.noTables, hint: K('⌘['), run: toggleTablesPane },
      { label: 'Row / column pane', checked: !!prefs.stats, hint: K('⌘]'), run: toggleRightPane },
      { label: 'Reset pane sizes', run: () => { delete prefs.left; delete prefs.right; delete prefs.edH; savePrefs(prefs); applySizes(); } },
      { sep: true },
      { label: 'Fonts…', icon: 'type', run: fontsDialog },
    ],
    Help: () => [
      { label: 'Keyboard shortcuts', icon: 'keyboard', hint: K('⇧⌘/'), run: sqlShortcuts },
      { label: 'Parameters: :data_start, :period_start…', icon: 'info', run: aboutParams },
    ],
  };
  el.querySelectorAll('.dbx-menu').forEach((b) => b.addEventListener('click', () => menu(b, menus[b.dataset.menu]())));

  // ---- Python on the last result: `df`
  let lang = 'sql';
  const pyBox = el.querySelector('.dbx-py');
  // the same editor as SQL, in Python: completes df's columns, pd., sources["…"], params["…"]
  const pyText = sqlEditor({
    lang: 'python',
    placeholder: '# df is the last SQL result — a pandas DataFrame\ndf.groupby(df.columns[0]).sum()',
    columns: () => (last ? last.r.columns.map((x) => x.name) : []),
    sources: () => (S.st?.doc?.datasets || []).map((d) => d.name),
    params: () => params,
    onRun: () => runPy(),
  });
  pyBox.querySelector('.dbx-pyed').append(pyText.el);
  const setLang = (l) => {
    lang = l;
    curEd = l === 'py' ? pyText : null;
    saveBtn.innerHTML = l === 'py' ? `${icon('python', 14)} Save as source` : `${icon('plus', 14)} ${dataset ? 'Save source' : 'Add as source'}`;
    saveBtn.title = l === 'py' ? 'Save the script as a Python data source that reads this query’s source' : dataset ? 'Save the query to this data source' : 'Add the query as a data source of the report';
    el.querySelectorAll('.dbx-lang button').forEach((b) => b.classList.toggle('on', b.dataset.l === l));
    el.querySelector('.dbx-ed').hidden = l === 'py';
    pyBox.hidden = l !== 'py';
    if (l === 'py') { pyText.focus(); if (!last) { status.className = 'dbx-status'; status.textContent = 'Run an SQL query first — its result is df.'; } }
    else { if (last) renderGrid(); ed.focus(); }
    updateBar();
  };
  ['keyup', 'mouseup', 'input'].forEach((t) => pyText.ta.addEventListener(t, updateBar));
  el.querySelectorAll('.dbx-lang button').forEach((b) => b.addEventListener('click', () => setLang(b.dataset.l)));
  const kindOf = (rows, j) => {
    const vals = rows.map((r) => r[j]).filter((v) => v != null);
    if (vals.length && vals.every((v) => typeof v === 'number')) return 'number';
    if (vals.length && vals.every((v) => typeof v === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(v))) return 'date';
    return 'text';
  };
  const runPy = async () => {
    const code = pyText.value;
    if (!code.trim()) return;
    if (!last) { status.className = 'dbx-status bad'; status.textContent = 'Run an SQL query first — its result is df.'; return; }
    status.className = 'dbx-status busy';
    status.textContent = 'Running Python…';
    try {
      const r = await call('py_run', { code, data: { columns: last.r.columns.map((x) => x.name), rows: last.r.rows } }, { quiet: true });
      const out = r.stdout ? `<pre class="dbx-pyout">${esc(r.stdout)}</pre>` : '';
      if (!r.ok) {
        status.className = 'dbx-status bad'; status.textContent = 'Python error';
        result.innerHTML = `${out}<div class="dbx-err">${icon('warn', 16)}<pre>${esc(r.error || 'Python failed')}</pre></div>`;
        return;
      }
      const t = r.value?.table;
      status.className = 'dbx-status ok';
      status.textContent = `Python · ${t ? `${fmtNum(t.rows.length, 'number')} rows · ${t.columns.length} columns` : r.value?.text != null ? 'a value' : 'done'} · ${Math.round(r.ms || 0)} ms`;
      result.innerHTML = '<div class="dbx-pyres">' + out + (t ? gridHtml(t.columns.map((n, j) => ({ name: n, kind: kindOf(t.rows, j) })), t.rows.slice(0, 2000))
        : r.value?.text != null ? `<pre class="dbx-pyout val">${esc(r.value.text)}</pre>` : (out ? '' : '<div class="dbx-note">Done. End the script with an expression (e.g. <code>df</code>) to see a result.</div>')) + '</div>';
      statsEl.hidden = true;
    } catch (e) {
      status.className = 'dbx-status bad'; status.textContent = 'Error';
      result.innerHTML = `<div class="dbx-err">${icon('warn', 16)}<pre>${esc(String(e))}</pre></div>`;
    }
  };
  // a Python data source that reads this query's source (saved first, when it isn't one yet)
  const savePy = async () => {
    if (!pyText.value.trim()) { toast('Write the Python first.', 'info'); return; }
    let srcName = dataset?.name || sqlSaved;
    if (!srcName) {
      toast('First the SQL becomes a data source — the Python source reads it by name.', 'info', 2600);
      srcName = await saveSql(true);
      if (!srcName) return;
    }
    const name = await modal({ title: 'Save as Python source', iconName: 'python', body: `<div class="modal-body form"><label>Name<input name="n" value="${esc(srcName)} · Python"></label><div class="hint">It reads <code>sources["${esc(srcName)}"]</code> as <code>df</code>, then runs your script — and refreshes with the report.</div></div>`,
      buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: (box) => box.querySelector('[name=n]').value.trim() }] });
    if (!name) return;
    const ds = { id: '', name, fields: [], calc_fields: [], transform: '', snapshot: false, source: { type: 'python', code: `df = sources["${srcName}"]\n${pyText.value.trim()}\n` } };
    if (saveSource) await saveSource(ds); else await edit({ op: 'upsert_dataset', dataset: ds });
    toast(`“${name}” saved — it runs after “${srcName}” each time the report loads.`, 'info', 3200);
  };

  el.querySelector('[data-act="run"]').addEventListener('click', () => (lang === 'py' ? runPy() : ed.run()));
  el.querySelector('[data-act="undo"]').addEventListener('click', () => (curEd || ed).undo());
  el.querySelector('[data-act="redo"]').addEventListener('click', () => (curEd || ed).redo());
  el.querySelector('[data-act="format"]').addEventListener('click', format);
  el.querySelector('[data-act="savefile"]').addEventListener('click', () => saveFile(false));
  el.querySelector('[data-act="max"]').addEventListener('click', () => toggleMax());
  el.querySelector('[data-act="pane-left"]').addEventListener('click', toggleTablesPane);
  el.querySelector('[data-act="pane-right"]').addEventListener('click', toggleRightPane);
  // the workspace's own keys while it is in front (the report's are paused)
  const onViewKey = (e) => {
    if (el.hidden || !document.body.classList.contains('sql-active') || document.querySelector('.modal-back')) return;
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    const k = e.key.toLowerCase();
    if (k === 's') { e.preventDefault(); saveFile(e.shiftKey); }
    else if (k === 'i') { e.preventDefault(); assistant.toggle(); }
    else if (k === 'o') { e.preventDefault(); openFile(); }
    else if (e.code === 'BracketLeft') { e.preventDefault(); toggleTablesPane(); }
    else if (e.code === 'BracketRight') { e.preventDefault(); toggleRightPane(); }
    else if (e.shiftKey && k === 'm') { e.preventDefault(); toggleMax(); }
    else if (e.key === '/' && e.shiftKey || e.key === '?') { e.preventDefault(); sqlShortcuts(); }
  };
  window.addEventListener('keydown', onViewKey);
  paintToggles();
  el.querySelector('[data-act="edit"]').addEventListener('click', toggleEdit);
  el.querySelector('[data-act="save-edits"]').addEventListener('click', saveEdits);
  el.querySelector('[data-act="revert"]').addEventListener('click', revertEdits);

  // saving: a data source with this query (its date column picked from the result)
  /** Saves the query as a data source; answers its name (null when cancelled). `keepOpen`: stay in the workspace;
   * `hint`: the AI assistant's suggested name and date column. */
  const saveSql = async (keepOpen = false, hint = null) => {
    if (!last) return null;
    const dates = last.r.columns.filter((x) => x.kind === 'date').map((x) => x.name);
    const from = last.sql.match(/\bfrom\s+("?[\w.]+"?)/i)?.[1]?.replace(/"/g, '').split('.').pop();
    const name0 = dataset?.name || hint?.name || (from ? from.charAt(0).toUpperCase() + from.slice(1).replace(/_/g, ' ') : 'Query');
    const form = h(`<div class="modal-body form">
      <label>Name<input name="name" value="${esc(name0)}"></label>
      <label>Date column<select name="date"><option value="">None (not dated)</option>${dates.map((d) => `<option${(dataset?.date_field || (dates.includes(hint?.date) ? hint.date : dates[0])) === d ? ' selected' : ''}>${esc(d)}</option>`).join('')}</select></label>
      <div class="hint">The date column lets widgets follow the reporting period and compare periods. ${fmtNum(last.r.total ?? last.r.rows.length, 'number')} rows now; the source runs this query each time the report loads.</div>
    </div>`);
    const v = await modal({ title: dataset ? 'Save data source' : 'Add as data source', iconName: 'database', body: form,
      buttons: [{ label: 'Cancel', value: null }, { label: dataset ? 'Save' : 'Add', kind: 'primary', value: (box) => ({ name: box.querySelector('[name=name]').value.trim(), date: box.querySelector('[name=date]').value }) }] });
    if (!v) return null;
    const ds = { ...(dataset || { id: '', fields: [], calc_fields: [], transform: '', snapshot: false }), name: v.name || name0, date_field: v.date || null, source: { type: 'db', connection: c.id, query: last.sql } };
    if (saveSource) await saveSource(ds);
    else await edit({ op: 'upsert_dataset', dataset: ds });
    toast(`“${ds.name}” ${dataset ? 'saved' : saveSource ? 'added to the Data area’s sources' : 'added to the template’s sources'}.`, 'info');
    sqlSaved = ds.name;
    if (dataset && !keepOpen) { dataset = { ...ds }; closeIt(); }
    return ds.name;
  };
  let sqlSaved = null; // the source the SQL was saved as in this workspace
  // one Save: the query as an SQL source — or, in Python, the script as a Python source reading it
  saveBtn.addEventListener('click', () => (lang === 'py' ? savePy() : saveSql()));

  el.querySelector('[data-act="close"]').addEventListener('click', () => closeIt());
  updateBar();
  el.classList.add('dbx-view');
  const view = {
    key,
    global,
    title: dataset ? dataset.name : c.name || kindLabel(c.kind),
    tip: dataset ? `Data source “${dataset.name}” on ${c.name}` : `SQL workspace · ${c.name}`,
    el,
    onShow: () => { refreshParams(); ed.refresh(); },
    destroy: () => { unregisterAI(key); ed.destroy(); pyText.destroy(); window.removeEventListener('keydown', onViewKey); },
    close: async () => {
      if (!(await discardOk('Closing the workspace'))) return;
      unregisterAI(key);
      ed.destroy();
      pyText.destroy();
      window.removeEventListener('keydown', onViewKey);
      el.remove();
      VIEWS.delete(key);
      if (lastView === key) lastView = null;
      showView(null);
    },
  };
  closeIt = view.close;
  VIEWS.set(key, view);
  viewHost().append(el);
  showView(key);
  ed.focus();
  tablesOf(c).then((t) => { tables = t; renderTables(); ed.refresh(); }).catch((e) => {
    $('.dbx-tlist').replaceChildren(h(`<div class="dbx-err">${icon('warn', 16)}<pre>${esc(String(e))}</pre></div>`));
  });
  if (dataset) run();
}

/**
 * A Python data source, edited as a tab of the Data area like an SQL source:
 * the Python editor (sources["…"], params["…"], pd. completion), the other
 * sources on the left, Run shows what the source returns, Save stores it —
 * in the template, or with `saveSource` (a shared or a report's source).
 */
export function pythonSource(ds, { saveSource = null } = {}) {
  let key = `py:${ds.id || 'new'}`;
  if (VIEWS.has(key)) { showView(key); return; }
  const others = () => (S.st?.doc?.datasets || []).filter((d) => d.id !== ds.id);
  const el = h(`<div class="dbx dbx-pysrc">
    <div class="dbx-head">
      <span class="ic">${icon('python', 18)}</span>
      <div class="dbx-title"><b>${esc(ds.name || 'New Python source')}</b><span>Python data source</span></div>
      <span class="grow"></span>
      <button class="tb-btn" data-act="close" title="Close">${icon('x', 18)}</button>
    </div>
    <div class="dbx-main">
      <aside class="dbx-tables">
        <div class="dbx-lp"><div class="sec-title">Sources it can read</div><div class="dbx-tlist"></div>
        <div class="dbx-tip">Click to insert <code>sources["…"]</code> — a DataFrame. <code>params["…"]</code> holds the report's values.</div></div>
      </aside>
      <section class="dbx-work">
        <div class="dbx-bar">
          <button class="btn primary dbx-run" data-act="run" title="Run (${K('⌘↵')})">${icon('play', 14)} <span>Run</span> <kbd>${K('⌘↵')}</kbd></button>
          <span class="dbx-sep"></span>
          <button class="tb-btn" data-act="undo" title="Undo (${K('⌘Z')})">${icon('undo', 16)}</button>
          <button class="tb-btn" data-act="redo" title="Redo (${K('⇧⌘Z')})">${icon('redo', 16)}</button>
          <span class="dbx-status"></span>
          <span class="grow"></span>
          <label class="dbx-nm">Name <input value="${esc(ds.name || '')}" placeholder="e.g. Positions by type"></label>
          <button class="btn sm primary" data-act="save">${icon('save', 14)} Save source</button>
        </div>
        <div class="dbx-ed"></div>
        <div class="dbx-hint">The script leaves its table in <code>df</code> or as its last line; it runs each time the report loads.</div>
        <div class="dbx-out"><div class="dbx-result"><div class="dbx-note">Press Run to see what this source returns.</div></div></div>
      </section>
    </div>
  </div>`);
  const $ = (q) => el.querySelector(q);
  const status = $('.dbx-status'), result = $('.dbx-result');
  let saved = ds.source?.code || '';
  const ed = sqlEditor({
    lang: 'python', value: saved,
    placeholder: '# e.g.\ndf = sources["Позиции на дату"]\ndf.groupby("instr_type_ru")["amt_rub"].sum().reset_index()',
    sources: () => others().map((d) => d.name),
    params: () => (S.st?.doc?.param_defs || []).map((p) => ({ name: p.name, value: S.st.doc.params.values?.[p.name] ?? '' })),
    onRun: () => run(),
  });
  $('.dbx-ed').append(ed.el);
  $('.dbx-tlist').innerHTML = others().map((d) => `<button class="dbx-tn" data-n="${esc(d.name)}">${icon('table', 13)}<b>${esc(d.name)}</b></button>`).join('') || '<div class="dbx-note">No other sources yet.</div>';
  el.querySelectorAll('.dbx-tn').forEach((b) => b.addEventListener('click', () => ed.insert(`sources["${b.dataset.n}"]`)));
  const build = () => ({ ...ds, name: $('.dbx-nm input').value.trim() || ds.name || 'Python source', source: { type: 'python', code: ed.value } });
  const run = async () => {
    status.className = 'dbx-status busy';
    status.textContent = 'Running…';
    try {
      const p = await call('preview_dataset', { dataset: build() }, { quiet: true });
      status.className = 'dbx-status ok';
      status.textContent = `${fmtNum(p.total, 'number')} rows · ${p.columns.length} columns`;
      result.innerHTML = `<div class="dbx-pyres">${gridHtml(p.fields, p.rows.slice(0, 500))}</div>`;
    } catch (e) {
      status.className = 'dbx-status bad';
      status.textContent = 'Error';
      result.innerHTML = `<div class="dbx-err">${icon('warn', 16)}<pre>${esc(String(e))}</pre></div>`;
    }
  };
  $('[data-act="run"]').addEventListener('click', run);
  $('[data-act="undo"]').addEventListener('click', () => ed.undo());
  $('[data-act="redo"]').addEventListener('click', () => ed.redo());
  $('.dbx-nm input').addEventListener('keydown', (e) => e.stopPropagation());
  $('[data-act="save"]').addEventListener('click', async () => {
    const next = build();
    const r = saveSource ? await saveSource(next) : await edit({ op: 'upsert_dataset', dataset: next });
    // a new source has its id now: later saves update it
    if (!ds.id) ds.id = r?.id || (S.st?.doc?.datasets || []).filter((d) => d.name === next.name).pop()?.id || '';
    ds.name = next.name;
    saved = ed.value;
    // the tab takes the source's name (and id: opening it again finds this tab)
    const v = VIEWS.get(key);
    if (v) {
      v.title = next.name;
      el.querySelector('.dbx-title b').textContent = next.name;
      const nk = `py:${ds.id}`;
      if (ds.id && nk !== key && !VIEWS.has(nk)) { VIEWS.delete(key); v.key = nk; VIEWS.set(nk, v); if (activeView === key) activeView = nk; key = nk; }
      mountViewTabs();
    }
    toast(`“${next.name}” saved.`, 'info');
  });
  const close = async () => {
    if (ed.value !== saved) {
      const ok = await modal({ title: 'Close without saving?', iconName: 'warn', iconKind: 'warn', body: '<p>The script has changes that aren’t saved to the source.</p>', buttons: [{ label: 'Cancel', value: false }, { label: 'Close', kind: 'danger', value: true }] });
      if (!ok) return;
    }
    ed.destroy();
    el.remove();
    VIEWS.delete(key);
    if (lastView === key) lastView = null;
    showView(null);
  };
  $('[data-act="close"]').addEventListener('click', close);
  el.classList.add('dbx-view');
  VIEWS.set(key, { key, global: !S.st?.doc, title: ds.name || 'Python', tip: `Python source “${ds.name || 'new'}”`, el, onShow: () => ed.refresh(), destroy: () => ed.destroy(), close });
  viewHost().append(el);
  showView(key);
  ed.focus();
  if (saved.trim()) run();
}

/** The SQL workspace's keyboard shortcuts. */
export function sqlShortcuts() {
  const groups = [
    ['Run', [['⌘↵', 'Run the query — or only the selected text'], ['⌃Space', 'Complete a name (tables, columns, functions, :parameters)'], [':', 'Pick a parameter or variable']]],
    ['Edit', [['⌘Z / ⇧⌘Z', 'Undo / redo'], ['⇧⌘F', 'Format SQL (the selection or everything)'], ['⌘/', 'Comment / uncomment lines'], ['Tab / ⇧Tab', 'Indent / outdent lines'], ['↑ ↓ ↵ Tab Esc', 'In the completion list: move, insert, close']]],
    ['Files', [['⌘S', 'Save the script as .sql'], ['⇧⌘S', 'Save the script as…'], ['⌘O', 'Open a .sql file']]],
    ['View', [['⌃Tab', 'Switch between the report and the SQL workspace'], ['⌘[', 'Show / hide the tables pane'], ['⌘]', 'Show / hide the row / column pane'], ['⇧⌘M', 'Maximize the editor / restore'], ['⇧⌘/', 'These shortcuts']]],
    ['Results', [['Click a header', 'The column’s figures'], ['Click / ⇧-click / ⌘-click', 'Select a row / a range / more rows'], ['↑ ↓', 'Previous / next row'], ['⌘A, ⌘C', 'Select all rows, copy as a table'], ['Double-click', 'Edit a value (in Edit mode)'], ['↵ / Esc / Tab', 'Keep / drop the value, next column']]],
  ];
  const k = (x) => (isMac ? x : x.replace(/⌘/g, 'Ctrl+').replace(/⇧/g, 'Shift+').replace(/⌃/g, 'Ctrl+'));
  modal({ title: 'SQL editor shortcuts', iconName: 'keyboard', wide: true,
    body: `<div class="modal-body kbd-groups">${groups.map(([t, rows]) => `<div><div class="sec-title">${esc(t)}</div><table class="kbd-table">${rows.map(([a, b]) => `<tr><td><kbd>${esc(k(a))}</kbd></td><td>${esc(b)}</td></tr>`).join('')}</table></div>`).join('')}</div>`,
    buttons: [{ label: 'Done', kind: 'primary', value: true }] });
}

/** What :data_start, :data_end, :period_start and :period_end are for. */
function aboutParams() {
  modal({ title: 'Parameters in SQL', iconName: 'info', wide: true, body: `<div class="modal-body prose">
    <p>Write <code>:name</code> in a query and Quire puts in the value when the query runs — in the workspace and each time the report loads its data.</p>
    <p>Dates come in two pairs, because a report often reads more data than the period it shows.</p>
    <p><b><code>:period_start</code> / <code>:period_end</code> — the report period</b>, exactly as chosen in the toolbar (e.g. 01.01.2026 – 15.06.2026). Use them when the query itself works out figures <i>of the period</i>: a return from net_twr_ltd at the period's end and the day before it starts, operations in the period, balances on its last day.</p>
    <pre class="dbx-pre">where op_dt between :period_start and :period_end</pre>
    <p><b><code>:data_start</code> / <code>:data_end</code> — the data to load.</b> They span the period <i>plus</i> whatever else the report shows: with a comparison (vs previous period or same period last year) <code>:data_start</code> moves back to cover it, and widgets with history (trailing months, sparklines) widen it too. Use them in a source that returns <i>rows with a date column</i>; Quire then cuts the period, the comparison and the history out of that one load.</p>
    <pre class="dbx-pre">select report_dt, acct_cd, bal_out from portf_acct where report_dt between :data_start and :data_end</pre>
    <p><b>Rule of thumb:</b> a dated source that Quire filters → <code>:data_start</code>/<code>:data_end</code>; a query that computes the period's result itself → <code>:period_start</code>/<code>:period_end</code>. With no comparison and no history the two pairs are the same dates. <code>:prev_start</code>/<code>:prev_end</code> are the comparison period, when there is one.</p>
    <p><b>Report parameters</b> (<code>:client</code>…) come from the toolbar's pickers; a multi-choice one becomes a list: <code>name in (:client)</code>. <b>Variables</b> from the Variables tab work the same way when their name is a plain word (<code>min_value</code>): text variables with their placeholders filled in (<code>{{end}}</code> → the date), SQL and data variables as computed.</p>
  </div>`, buttons: [{ label: 'Done', kind: 'primary', value: true }] });
}

/** Opens a data source on a connection in its workspace. */
export function editDbSource(ds, { saveSource = null, conn = null } = {}) {
  const c = conn || (S.st.doc?.connections || []).find((x) => x.id === ds.source.connection);
  if (!c) { toast('This data source’s connection was removed. Connect the database again in Data → Connections.', 'error'); return; }
  workspace(c, { dataset: ds, saveSource });
}
