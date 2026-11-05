// The SQL editor of the workspace: a textarea over a highlighted copy of its
// text (syntax colours, line numbers), completion from the connection's
// tables and columns, its own undo history, formatting, and drops of table
// and column names from the tables pane.

import { esc, h } from './util.js';
import { highlight as pyHighlight, complete as pyComplete } from './pylang.js';

const KEYWORDS = new Set(`select from where and or not in is null as on join left right full inner outer cross natural using
group by order having limit offset union all intersect except distinct case when then else end with recursive
asc desc nulls first last between like ilike similar exists any some values insert update delete into set
create table view drop alter qualify window over partition rows range unbounded preceding following current row
filter within lateral true false interval cast try_cast pivot unpivot returning materialized escape collate
semi anti positional asof sample tablesample replace exclude columns by`.split(/\s+/));
const FUNCTIONS = new Set(`count sum avg min max median mode stddev stddev_samp stddev_pop variance var_samp var_pop corr covar_pop
first last any_value arg_min arg_max argmin argmax string_agg group_concat list array_agg listagg bool_and bool_or
row_number rank dense_rank percent_rank cume_dist ntile lag lead first_value last_value nth_value
coalesce nullif ifnull greatest least abs round floor ceil ceiling sqrt power pow exp ln log log10 sign mod trunc
lower upper length len trim ltrim rtrim substr substring replace concat concat_ws split_part left right lpad rpad
strpos position instr regexp_matches regexp_replace regexp_extract starts_with ends_with contains format printf reverse
date_trunc date_part date_diff datediff date_add date_sub extract year month day quarter week dayofweek dayofyear hour minute second
strftime strptime to_char to_date to_timestamp now current_date current_timestamp today last_day make_date epoch age
to_months to_days to_years generate_series range unnest struct_pack list_value array_length cardinality
exp_sum product percentile_cont percentile_disc quantile quantile_cont approx_count_distinct histogram
hash md5 uuid random setseed typeof`.split(/\s+/));
const TYPES = new Set('integer int bigint smallint tinyint double float real decimal numeric varchar text char boolean bool date timestamp timestamptz time blob json uuid hugeint'.split(' '));

// clause keywords that start a line when formatting (multi-word ones joined by a space)
const MULTI = [
  ['left', 'outer', 'join'], ['right', 'outer', 'join'], ['full', 'outer', 'join'], ['union', 'all', 'by', 'name'], ['union', 'by', 'name'],
  ['group', 'by'], ['order', 'by'], ['partition', 'by'], ['left', 'join'], ['right', 'join'], ['full', 'join'], ['inner', 'join'],
  ['cross', 'join'], ['natural', 'join'], ['positional', 'join'], ['asof', 'join'], ['semi', 'join'], ['anti', 'join'], ['union', 'all'],
  ['is', 'not'], ['not', 'in'], ['not', 'like'], ['not', 'between'], ['not', 'exists'],
];
const CLAUSES = new Set(['select', 'from', 'where', 'group by', 'order by', 'having', 'limit', 'offset', 'qualify', 'window', 'union', 'union all',
  'union by name', 'union all by name', 'intersect', 'except', 'values', 'with', 'join', 'left join', 'right join', 'full join', 'inner join',
  'cross join', 'natural join', 'positional join', 'asof join', 'semi join', 'anti join', 'left outer join', 'right outer join', 'full outer join']);

const RX = /(--[^\n]*)|(\/\*[\s\S]*?(?:\*\/|$))|('(?:[^']|'')*'?)|("(?:[^"]|"")*"?)|(::)|(:[A-Za-z_]\w*)|(\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\.\d+)|([A-Za-z_À-￿][\w$À-￿]*)|(\s+)|(<=|>=|<>|!=|\|\||->>|->|[-+*/%=<>(),.;[\]{}])|([\s\S])/gy;
const KINDS = [null, 'cm', 'cm', 'str', 'qid', 'op', 'param', 'num', 'word', 'ws', 'op', 'other'];

/** The text as tokens: {t: kind, v: text, at: offset}. */
export function tokenize(text) {
  const out = [];
  RX.lastIndex = 0;
  let m;
  while (RX.lastIndex < text.length && (m = RX.exec(text))) {
    const i = m.findIndex((g, k) => k > 0 && g !== undefined);
    out.push({ t: KINDS[i], v: m[0], at: m.index });
  }
  return out;
}

const wordKind = (v, next, known) => {
  const l = v.toLowerCase();
  if (next === '(' && FUNCTIONS.has(l)) return 'fn';
  if (KEYWORDS.has(l)) return 'kw';
  if (TYPES.has(l)) return 'type';
  if (FUNCTIONS.has(l) && next === '(') return 'fn';
  if (known?.tables.has(l)) return 'tbl';
  if (known?.columns.has(l)) return 'col';
  if (next === '(') return 'fn';
  return 'id';
};

/** Highlighted HTML of SQL text. `known`: {tables: Set, columns: Set} (lower case). */
export function highlight(text, known) {
  const toks = tokenize(text);
  let html = '';
  for (let i = 0; i < toks.length; i++) {
    const { t, v } = toks[i];
    if (t === 'ws') { html += esc(v); continue; }
    let cls = t;
    if (t === 'word') {
      let j = i + 1;
      while (toks[j]?.t === 'ws') j++;
      cls = wordKind(v, toks[j]?.v, known);
    }
    html += `<span class="s-${cls}">${esc(v)}</span>`;
  }
  // a trailing newline needs a line to show
  return html + (text.endsWith('\n') || !text ? ' ' : '');
}

// ---------------------------------------------------------------- formatting

/** Reformats SQL for reading: one clause per line, select lists and conditions one per line, subqueries indented. */
export function formatSql(text, { upper = false } = {}) {
  const raw = tokenize(text);
  // drop whitespace; remember which tokens followed whitespace
  const toks = [];
  for (let i = 0; i < raw.length; i++) {
    if (raw[i].t === 'ws') continue;
    toks.push({ ...raw[i], sp: raw[i - 1]?.t === 'ws', nl: raw[i - 1]?.t === 'ws' && raw[i - 1].v.includes('\n') });
  }
  // join multi-word keywords
  const T = [];
  for (let i = 0; i < toks.length; i++) {
    const tk = toks[i];
    if (tk.t === 'word') {
      const m = MULTI.find((ws) => ws.every((w, k) => toks[i + k]?.t === 'word' && toks[i + k].v.toLowerCase() === w));
      if (m) { T.push({ ...tk, v: toks.slice(i, i + m.length).map((x) => x.v).join(' '), kw: m.join(' ') }); i += m.length - 1; continue; }
      const l = tk.v.toLowerCase();
      if (KEYWORDS.has(l) && !(toks[i + 1]?.v === '(' && !toks[i + 1].sp && !['in', 'as', 'over', 'values', 'exists', 'using', 'filter', 'on', 'and', 'or', 'not', 'from', 'join', 'when', 'then', 'else'].includes(l))) tk.kw = l;
    }
    T.push(tk);
  }
  const kcase = (tk) => (tk.kw ? (upper ? tk.v.toUpperCase() : tk.v.toLowerCase()) : tk.t === 'word' && FUNCTIONS.has(tk.v.toLowerCase()) ? (upper ? tk.v.toUpperCase() : tk.v.toLowerCase()) : tk.v);

  // does the select starting at i list several items (a comma at its own depth before the next clause)?
  const multiList = (i) => {
    let d = 0;
    for (let j = i + 1; j < T.length; j++) {
      const v = T[j].v;
      if (v === '(') d++;
      else if (v === ')') { if (--d < 0) return false; }
      else if (d === 0 && v === ',') return true;
      else if (d === 0 && (CLAUSES.has(T[j].kw) || v === ';')) return false;
    }
    return false;
  };

  let out = '';
  let ind = 0; // indent of the current level
  const frames = []; // open parentheses: {sub, ind, clause}
  let clause = null, listBreak = false, between = false, prev = null;
  const nl = (n) => { out = out.replace(/[ \t]+$/, '') + '\n' + ' '.repeat(Math.max(0, n)); prev = null; };
  const atLevel = () => !frames.length || frames[frames.length - 1].sub;
  const space = (tk) => {
    if (!prev || /\n *$/.test(out)) return false;
    const v = tk.v, p = prev.v;
    if ([',', ')', ';', '.', '::', ']'].includes(v) || ['(', '.', '::', '['].includes(p)) return false;
    if (prev.unary) return false;
    if (v === '(') return tk.sp || !!prev.kw;
    if (v === '[') return false;
    return true;
  };
  const emit = (tk, text = kcase(tk)) => { if (space(tk)) out += ' '; out += text; prev = tk; };

  for (let i = 0; i < T.length; i++) {
    const tk = T[i];
    const v = tk.v, k = tk.kw;
    if (tk.t === 'cm') {
      if (tk.v.startsWith('--')) { if (tk.nl && out) nl(ind + (listBreak ? 2 : 0)); emit(tk, tk.v.trimEnd()); nl(ind + (clause && clause !== 'with' ? 2 : 0)); }
      else emit(tk);
      continue;
    }
    if (k && CLAUSES.has(k) && atLevel()) {
      if (out.trim() && !/\n *$/.test(out)) nl(ind);
      else if (out.trim()) out = out.replace(/ *$/, ' '.repeat(ind));
      emit(tk);
      clause = k.endsWith('join') ? 'join' : k;
      between = false;
      listBreak = k === 'select' && multiList(i);
      if (listBreak) nl(ind + 2);
      continue;
    }
    if ((k === 'and' || k === 'or') && atLevel() && ['where', 'having', 'qualify', 'join', 'on'].includes(clause)) {
      if (k === 'and' && between) { between = false; emit(tk); continue; }
      nl(ind + 2);
      emit(tk);
      continue;
    }
    if (k === 'on' && atLevel()) { clause = 'on'; emit(tk); continue; }
    if (k === 'between' || k === 'not between') between = true;
    if (v === '(') {
      let j = i + 1;
      while (T[j]?.t === 'cm') j++;
      const sub = ['select', 'with', 'values'].includes(T[j]?.kw);
      emit(tk);
      frames.push({ sub, ind, clause, listBreak });
      if (sub) { ind += (frames.length > 1 || clause === 'with' || clause === 'from' || clause === 'join' ? 2 : 2) + (listBreak ? 2 : 0); clause = null; listBreak = false; nl(ind); }
      continue;
    }
    if (v === ')') {
      const f = frames.pop();
      if (f?.sub) { ind = f.ind; nl(ind + (f.listBreak ? 2 : 0)); }
      if (f) { clause = f.clause; listBreak = f.listBreak; }
      emit(tk);
      continue;
    }
    if (v === ',' && atLevel()) {
      emit(tk);
      if (clause === 'select' && listBreak) nl(ind + 2);
      else if (clause === 'with') nl(ind);
      continue;
    }
    if (v === ';') { emit(tk); out += '\n'; nl(0); ind = 0; frames.length = 0; clause = null; listBreak = false; continue; }
    if (v === '-' || v === '+') {
      const p = prev;
      tk.unary = !p || (p.t === 'op' && p.v !== ')' && p.v !== ']') || !!p.kw;
    }
    emit(tk);
  }
  return out.replace(/[ \t]+$/gm, '').replace(/\n{3,}/g, '\n\n').trim() + '\n';
}

// ---------------------------------------------------------------- completion

/** Tables named in the query with their aliases, and the query's CTE names. */
function scope(text, tables) {
  const toks = tokenize(text).filter((t) => t.t !== 'ws' && t.t !== 'cm');
  const byName = new Map(tables.map((t) => [t.name.toLowerCase(), t]));
  const aliases = new Map(); // alias (lower) → table
  const used = [];
  const ctes = [];
  const unq = (v) => (v.startsWith('"') ? v.slice(1, -1).replace(/""/g, '"') : v);
  for (let i = 0; i < toks.length; i++) {
    const l = toks[i].v.toLowerCase();
    if ((l === 'with' || toks[i].v === ',') && toks[i + 1] && toks[i + 2]?.v.toLowerCase() === 'as' && toks[i + 3]?.v === '(') ctes.push(unq(toks[i + 1].v));
    if (l !== 'from' && l !== 'join') continue;
    let j = i + 1;
    let name = toks[j] && unq(toks[j].v);
    if (toks[j + 1]?.v === '.' && toks[j + 2]) { j += 2; name = unq(toks[j].v); }
    const t = name && byName.get(name.toLowerCase());
    if (!t) continue;
    used.push(t);
    aliases.set(t.name.toLowerCase(), t);
    let a = toks[j + 1];
    if (a?.v.toLowerCase() === 'as') a = toks[j + 2];
    if (a && (a.t === 'word' || a.t === 'qid') && !KEYWORDS.has(a.v.toLowerCase())) aliases.set(unq(a.v).toLowerCase(), t);
  }
  return { aliases, used, ctes };
}

/** Completion items for the caret: [{label, kind, detail, insert}] and the prefix they replace. */
function complete(text, caret, tables, params = []) {
  const before = text.slice(0, caret);
  // ":name" → the report's parameters, with their values now
  const pm = before.match(/(^|[^:\w]):(\w*)$/);
  if (pm) {
    const p = pm[2].toLowerCase();
    const items = params.filter((x) => !x.bad && x.name.toLowerCase().startsWith(p)).map((x) => ({ label: `:${x.name}`, kind: 'param', detail: x.value, insert: `:${x.name}` }));
    return { items, prefix: `:${pm[2]}` };
  }
  const m = before.match(/(?:("?)([\wÀ-￿]+)\1\.)?("?[\wÀ-￿]*)$/);
  const prefix = m[3] || '';
  const p = prefix.replace(/^"/, '').toLowerCase();
  const sc = scope(text, tables);
  let items = [];
  if (m[2]) {
    // after "alias." or "table.": that table's columns
    const t = sc.aliases.get(m[2].toLowerCase()) || tables.find((x) => x.name.toLowerCase() === m[2].toLowerCase());
    if (t) items = t.columns.map((c) => ({ label: c.name, kind: 'col', detail: c.type }));
  } else {
    // the keyword before decides what fits first: tables after from/join
    const prevWord = (before.slice(0, before.length - prefix.length).match(/([\w]+)\s*$/) || [])[1]?.toLowerCase();
    const wantTable = ['from', 'join', 'into', 'table', 'update'].includes(prevWord);
    const tbl = [...sc.ctes.map((n) => ({ label: n, kind: 'tbl', detail: 'with' })), ...tables.map((t) => ({ label: t.name, kind: 'tbl', detail: t.kind === 'view' ? 'view' : `${t.rows ?? ''} rows`.trim() }))];
    const cols = [];
    const seen = new Set();
    for (const t of sc.used.length ? sc.used : []) for (const c of t.columns) if (!seen.has(c.name)) { seen.add(c.name); cols.push({ label: c.name, kind: 'col', detail: `${t.name} · ${c.type}` }); }
    const kws = [...KEYWORDS].map((k) => ({ label: k, kind: 'kw', detail: 'keyword' }));
    const fns = [...FUNCTIONS].map((f) => ({ label: f, kind: 'fn', detail: 'function', insert: `${f}()` }));
    const allCols = [];
    if (!wantTable && !sc.used.length) for (const t of tables) for (const c of t.columns) if (!seen.has(c.name)) { seen.add(c.name); allCols.push({ label: c.name, kind: 'col', detail: `${t.name} · ${c.type}` }); }
    items = wantTable ? [...tbl, ...kws] : [...cols, ...[...sc.aliases.keys()].filter((a) => !sc.used.some((t) => t.name.toLowerCase() === a)).map((a) => ({ label: a, kind: 'alias', detail: sc.aliases.get(a).name })), ...allCols, ...tbl, ...fns, ...kws];
  }
  if (p) {
    const starts = items.filter((x) => x.label.toLowerCase().startsWith(p));
    const has = items.filter((x) => !x.label.toLowerCase().startsWith(p) && x.label.toLowerCase().includes(p));
    items = [...starts, ...has];
  }
  items = items.filter((x) => x.label.toLowerCase() !== p);
  return { items: items.slice(0, 60), prefix };
}

// ---------------------------------------------------------------- editor

const quoteIf = (s) => (/^[a-z_][a-z0-9_]*$/.test(s) && !KEYWORDS.has(s) ? s : `"${s.replace(/"/g, '""')}"`);

/**
 * An SQL editor. `tables()`: the connection's tables for completion;
 * `onRun(text, isSelection)`; `onChange()`; `onContext(e)` for its menu.
 */
export function sqlEditor({ value = '', tables = () => [], params = () => [], onRun = () => {}, onChange = () => {}, onContext = null, lang = 'sql', columns = () => [], sources = () => [], placeholder = null } = {}) {
  // Python: its own highlighting and completion (df, pd, sources["…"], params["…"])
  const py = lang === 'python';
  const pyCtx = { columns, sources, params };
  const unit = py ? '    ' : '  ';
  const el = h(`<div class="sqled">
    <div class="sqled-gut" aria-hidden="true"></div>
    <div class="sqled-area">
      <pre class="sqled-hl" aria-hidden="true"></pre>
      <textarea class="sqled-ta" spellcheck="false" autocapitalize="off" autocomplete="off" wrap="off" data-own-esc placeholder="${esc(placeholder || (py ? '# df is the last SQL result' : 'select * from …'))}"></textarea>
    </div>
  </div>`);
  const ta = el.querySelector('textarea');
  const hl = el.querySelector('.sqled-hl');
  const gut = el.querySelector('.sqled-gut');
  let known = { tables: new Set(), columns: new Set() };
  const refreshKnown = () => {
    const ts = tables() || [];
    known = { tables: new Set(ts.map((t) => t.name.toLowerCase())), columns: new Set(ts.flatMap((t) => t.columns.map((c) => c.name.toLowerCase()))) };
  };

  const paint = () => {
    hl.innerHTML = py ? pyHighlight(ta.value, pyCtx) : highlight(ta.value, known);
    const n = ta.value.split('\n').length;
    if (gut.childElementCount !== n) gut.innerHTML = Array.from({ length: n }, (_, i) => `<div>${i + 1}</div>`).join('');
    sync();
  };
  const sync = () => {
    hl.scrollTop = ta.scrollTop;
    hl.scrollLeft = ta.scrollLeft;
    gut.scrollTop = ta.scrollTop;
  };
  ta.addEventListener('scroll', sync);

  // ---- history (the textarea's own undo can't see changes made by code)
  const hist = [{ v: value, s: value.length, e: value.length }];
  let hi = 0, lastType = 0;
  const snap = () => ({ v: ta.value, s: ta.selectionStart, e: ta.selectionEnd });
  const record = (merge = false) => {
    if (hist[hi].v === ta.value) { hist[hi].s = ta.selectionStart; hist[hi].e = ta.selectionEnd; return; }
    hist.length = hi + 1;
    if (merge && hi > 0) hist[hi] = snap();
    else { hist.push(snap()); hi++; }
    if (hist.length > 300) { hist.shift(); hi--; }
  };
  const restore = (st) => { ta.value = st.v; ta.setSelectionRange(st.s, st.e); paint(); onChange(); };
  const undo = () => { if (hi > 0) restore(hist[--hi]); ta.focus(); };
  const redo = () => { if (hi < hist.length - 1) restore(hist[++hi]); ta.focus(); };

  /** Replaces the text from a to b (recorded as one step). */
  const replace = (text, a = ta.selectionStart, b = ta.selectionEnd, select = 'end') => {
    record();
    ta.focus();
    ta.setRangeText(text, a, b, select);
    record();
    paint();
    onChange();
  };
  const set = (text) => replace(text, 0, ta.value.length, 'end');

  // typing a closing bracket or quote that is already there steps over it (after an inserted "()")
  ta.addEventListener('beforeinput', (e) => {
    if (e.inputType !== 'insertText' || !e.data || e.data.length !== 1 || !')]}\'"'.includes(e.data)) return;
    const at = ta.selectionStart;
    if (at !== ta.selectionEnd || ta.value[at] !== e.data) return;
    // a quote only steps over when it closes a string just typed
    if ((e.data === '"' || e.data === "'") && ((ta.value.slice(0, at).split(e.data).length - 1) % 2 === 0)) return;
    e.preventDefault();
    ta.setSelectionRange(at + 1, at + 1);
  });

  ta.addEventListener('input', (e) => {
    const now = Date.now();
    // typing merges into one step until a pause or a new word
    const merge = e.inputType === 'insertText' && now - lastType < 900 && !/\s/.test(e.data || '');
    lastType = now;
    record(merge);
    paint();
    onChange();
    if (e.inputType === 'insertText' && (py ? /[\p{L}\p{N}_.'"]/u : /[\w.À-￿":]/).test(e.data || '')) suggest(false);
    else if (e.inputType !== 'insertText' || !/[\p{L}\p{N}_]/u.test(e.data || '')) hide();
  });

  // ---- line editing helpers
  const lineRange = () => {
    const v = ta.value;
    const a = v.lastIndexOf('\n', ta.selectionStart - 1) + 1;
    let b = v.indexOf('\n', Math.max(ta.selectionEnd - (ta.selectionEnd > ta.selectionStart && v[ta.selectionEnd - 1] === '\n' ? 1 : 0), ta.selectionStart));
    if (b < 0) b = v.length;
    return [a, b];
  };
  const mapLines = (fn) => {
    const [a, b] = lineRange();
    const lines = ta.value.slice(a, b).split('\n');
    const out = fn(lines).join('\n');
    replace(out, a, b, 'select');
  };
  const indent = (back) => mapLines((ls) => ls.map((l) => (back ? l.replace(py ? /^ {1,4}/ : /^ {1,2}/, '') : unit + l)));
  const cm = py ? '#' : '--';
  const toggleComment = () => mapLines((ls) => {
    const all = ls.filter((l) => l.trim()).every((l) => l.trimStart().startsWith(cm));
    return ls.map((l) => (all ? l.replace(new RegExp(`^(\\s*)${cm} ?`), '$1') : l.trim() ? l.replace(/^(\s*)/, `$1${cm} `) : l));
  });

  /** The selected text, or '' when nothing is selected. */
  const selection = () => ta.value.slice(ta.selectionStart, ta.selectionEnd);
  const run = () => { const s = selection(); onRun(s.trim() ? s : ta.value, !!s.trim()); };

  ta.addEventListener('keydown', (e) => {
    const mod = e.metaKey || e.ctrlKey;
    if (pop && !pop.hidden) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); move(e.key === 'ArrowDown' ? 1 : -1); return; }
      if (e.key === 'Enter' || e.key === 'Tab') { e.preventDefault(); accept(); return; }
      if (e.key === 'Escape') { e.preventDefault(); hide(); return; }
    }
    if (e.key === 'Escape') return; // the workspace closes
    if (mod && e.key === 'Enter') { e.preventDefault(); run(); return; }
    if (mod && !e.shiftKey && e.key.toLowerCase() === 'z') { e.preventDefault(); undo(); return; }
    if (mod && ((e.shiftKey && e.key.toLowerCase() === 'z') || e.key.toLowerCase() === 'y')) { e.preventDefault(); redo(); return; }
    if (mod && e.shiftKey && e.key.toLowerCase() === 'f') { e.preventDefault(); api.format(); return; }
    if (mod && e.key === '/') { e.preventDefault(); toggleComment(); return; }
    if (e.ctrlKey && e.key === ' ') { e.preventDefault(); suggest(true); return; }
    if (e.key === 'Tab') {
      e.preventDefault();
      if (ta.selectionStart !== ta.selectionEnd && selection().includes('\n')) indent(e.shiftKey);
      else if (e.shiftKey) indent(true);
      else replace(unit);
      return;
    }
    if (e.key === 'Enter' && !mod) {
      // keep the line's indent
      e.preventDefault();
      const v = ta.value;
      const ls = v.lastIndexOf('\n', ta.selectionStart - 1) + 1;
      const lead = v.slice(ls, ta.selectionStart).match(/^[ \t]*/)[0];
      // a block opens after "(" (SQL) or ":" (Python)
      const extra = (py ? /[:([{]\s*(#.*)?$/ : /\(\s*$/).test(v.slice(ls, ta.selectionStart)) ? unit : '';
      replace('\n' + lead + extra);
    }
  });
  ta.addEventListener('blur', () => setTimeout(() => { if (!el.contains(document.activeElement)) hide(); }, 120));
  ta.addEventListener('click', () => hide());
  if (onContext) ta.addEventListener('contextmenu', (e) => { e.preventDefault(); onContext(e); });

  // ---- geometry: monospace cells (for the completion list and drops)
  const metrics = () => {
    const cs = getComputedStyle(ta);
    const probe = document.createElement('span');
    probe.style.cssText = `font:${cs.font};position:absolute;visibility:hidden;white-space:pre`;
    probe.textContent = 'M'.repeat(40);
    document.body.append(probe);
    const cw = probe.getBoundingClientRect().width / 40;
    probe.remove();
    return { cw, lh: parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.55, pl: parseFloat(cs.paddingLeft), pt: parseFloat(cs.paddingTop) };
  };
  const caretXY = (pos) => {
    const { cw, lh, pl, pt } = metrics();
    const before = ta.value.slice(0, pos).split('\n');
    const r = ta.getBoundingClientRect();
    return { x: r.left + pl + before[before.length - 1].length * cw - ta.scrollLeft, y: r.top + pt + before.length * lh - ta.scrollTop, lh };
  };
  const posAt = (x, y) => {
    const { cw, lh, pl, pt } = metrics();
    const r = ta.getBoundingClientRect();
    const lines = ta.value.split('\n');
    const li = Math.max(0, Math.min(lines.length - 1, Math.floor((y - r.top - pt + ta.scrollTop) / lh)));
    const col = Math.max(0, Math.min(lines[li].length, Math.round((x - r.left - pl + ta.scrollLeft) / cw)));
    return lines.slice(0, li).reduce((n, l) => n + l.length + 1, 0) + col;
  };

  // ---- drops from the tables pane (text/x-sql) and plain text
  ta.addEventListener('dragover', (e) => {
    if (![...e.dataTransfer.types].some((t) => t === 'text/x-sql' || t === 'text/plain')) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'copy';
    const p = posAt(e.clientX, e.clientY);
    ta.focus();
    ta.setSelectionRange(p, p);
  });
  ta.addEventListener('drop', (e) => {
    const text = e.dataTransfer.getData('text/x-sql') || e.dataTransfer.getData('text/plain');
    if (!text) return;
    e.preventDefault();
    const p = posAt(e.clientX, e.clientY);
    // a separating space when dropped against a word
    const v = ta.value;
    const pre = p > 0 && /[\w"]/.test(v[p - 1]) ? ' ' : '';
    const post = /[\w"]/.test(v[p] || '') ? ' ' : '';
    replace(pre + text + post, p, p);
  });

  // ---- completion list
  let pop = null, items = [], sel = 0, prefixLen = 0;
  const hide = () => { if (pop) pop.hidden = true; };
  const move = (d) => { sel = (sel + d + items.length) % items.length; drawPop(); };
  const drawPop = () => {
    pop.innerHTML = items.map((x, i) => `<div class="sqc-i${i === sel ? ' on' : ''}" data-i="${i}"><span class="sqc-k s-${x.kind === 'alias' ? 'tbl' : x.kind}">${{ kw: 'K', fn: 'ƒ', tbl: py ? '◆' : 'T', col: 'C', alias: 'A', param: ':', id: 'v', num: '·' }[x.kind] || ''}</span><b>${esc(x.label)}</b><i>${esc(x.detail || '')}</i></div>`).join('');
    pop.querySelector('.on')?.scrollIntoView({ block: 'nearest' });
  };
  const accept = () => {
    const it = items[sel];
    if (!it) return hide();
    const end = ta.selectionStart;
    let text = it.insert || (py || it.kind === 'kw' ? it.label : quoteIf(it.label));
    const caretBack = text.endsWith('()') ? 1 : 0;
    replace(text, end - prefixLen, end);
    if (caretBack) ta.setSelectionRange(ta.selectionStart - 1, ta.selectionStart - 1);
    hide();
  };
  const suggest = (force) => {
    refreshKnown();
    const r = py ? pyComplete(ta.value, ta.selectionStart, pyCtx) : complete(ta.value, ta.selectionStart, tables() || [], params() || []);
    if (!force && !r.prefix && !(py ? /(\.|\[\s*['"])$/ : /\.$/).test(ta.value.slice(0, ta.selectionStart))) return hide();
    items = r.items;
    prefixLen = r.prefix.length;
    if (!items.length) return hide();
    sel = 0;
    if (!pop) {
      pop = h('<div class="sqc"></div>');
      pop.addEventListener('mousedown', (e) => {
        const i = e.target.closest('.sqc-i');
        if (!i) return;
        e.preventDefault();
        sel = +i.dataset.i;
        accept();
      });
      document.body.append(pop);
    }
    pop.hidden = false;
    drawPop();
    const { x, y } = caretXY(ta.selectionStart - prefixLen);
    const w = 320, hgt = Math.min(240, items.length * 24 + 8);
    pop.style.left = Math.max(8, Math.min(x - 28, innerWidth - w - 8)) + 'px';
    pop.style.top = (y + hgt + 4 > innerHeight ? y - hgt - 24 : y + 2) + 'px';
  };

  const api = {
    el, ta,
    get value() { return ta.value; },
    set, replace, undo, redo, selection, run, indent, toggleComment,
    canUndo: () => hi > 0,
    canRedo: () => hi < hist.length - 1,
    /** Inserts text at the caret. */
    insert: (text) => replace(text),
    /** Formats the selection, or everything. */
    fmt: { upper: false },
    format(opts = api.fmt) {
      if (py) return;
      const s = selection();
      if (s.trim()) replace(formatSql(s, opts).trimEnd(), ta.selectionStart, ta.selectionEnd, 'select');
      else set(formatSql(ta.value, opts));
    },
    focus: () => ta.focus(),
    refresh: () => { refreshKnown(); paint(); },
    destroy: () => pop?.remove(),
  };
  ta.value = value;
  refreshKnown();
  paint();
  return api;
}
