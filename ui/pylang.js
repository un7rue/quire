// Python for the code editor: highlighting and completion that know the
// workspace — `df` (the last SQL result) and its columns, `pd`, `np`,
// `sources["…"]` (the data sources) and `params["…"]` (the report's values).

import { esc } from './util.js';

const KEYWORDS = new Set('and as assert async await break class continue def del elif else except finally for from global if import in is lambda nonlocal not or pass raise return try while with yield match case'.split(' '));
const CONSTS = new Set(['True', 'False', 'None']);
const BUILTINS = new Set(`abs all any bool dict dir divmod enumerate filter float format frozenset getattr hasattr hash int isinstance iter len list map max min next object
  open pow print range repr reversed round set setattr slice sorted str sum tuple type zip`.split(/\s+/).filter(Boolean));
// the names the script starts with
const GIVEN = { df: 'the last SQL result (DataFrame)', pd: 'pandas', np: 'numpy (import numpy as np)', params: "the report's values", sources: 'the data sources, by name' };

const DF = `head tail describe info shape columns dtypes index values copy groupby agg aggregate sum mean median min max count nunique std var cumsum cumprod pct_change diff
  shift rolling expanding resample sort_values sort_index reset_index set_index rename drop drop_duplicates dropna fillna isna notna isin where mask query assign apply
  map merge join concat pivot pivot_table melt stack unstack explode round abs clip astype to_dict to_list to_csv loc iloc at iat T empty size nlargest nsmallest
  value_counts unique between rank corr cov quantile idxmax idxmin sample select_dtypes filter any all prod eval`.split(/\s+/).filter(Boolean);
const PD = `DataFrame Series concat merge to_datetime to_numeric date_range period_range pivot_table crosstab cut qcut isna notna read_csv read_excel read_json
  read_parquet Timestamp Timedelta NaT NA options set_option get_dummies melt wide_to_long`.split(/\s+/).filter(Boolean);
const NP = 'array arange linspace zeros ones where nan isnan sum mean median std round abs log exp sqrt clip percentile cumsum diff sign maximum minimum unique'.split(' ');
const ATTRS = new Set(['shape', 'columns', 'dtypes', 'index', 'values', 'T', 'empty', 'size', 'loc', 'iloc', 'at', 'iat', 'NaT', 'NA', 'nan', 'options']);

const RX = /(#[^\n]*)|([rRbBuUfF]{0,2}"""[\s\S]*?(?:"""|$)|[rRbBuUfF]{0,2}'''[\s\S]*?(?:'''|$))|([rRbBuUfF]{0,2}"(?:[^"\\\n]|\\.)*"?|[rRbBuUfF]{0,2}'(?:[^'\\\n]|\\.)*'?)|(\d[\d_]*(?:\.\d*)?(?:[eE][+-]?\d+)?j?|\.\d+)|(@[A-Za-z_]\w*)|([A-Za-z_À-￿][\wÀ-￿]*)|(\s+)|(\*\*|\/\/|==|!=|<=|>=|->|:=|[-+*/%=<>()[\]{},.:;&|^~])|([\s\S])/gy;
const KINDS = [null, 'cm', 'str', 'str', 'num', 'deco', 'word', 'ws', 'op', 'other'];

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

/** Highlighted HTML of Python. `ctx.columns()`: the last result's columns (names). */
export function highlight(text, ctx = {}) {
  const toks = tokenize(text);
  const cols = new Set((ctx.columns?.() || []).map(String));
  let html = '';
  for (let i = 0; i < toks.length; i++) {
    const { t, v } = toks[i];
    if (t === 'ws') { html += esc(v); continue; }
    let cls = t === 'deco' ? 'fn' : t;
    if (t === 'word') {
      let j = i + 1;
      while (toks[j]?.t === 'ws' && !toks[j].v.includes('\n')) j++;
      let k = i - 1;
      while (toks[k]?.t === 'ws' && !toks[k].v.includes('\n')) k--;
      const prev = toks[k]?.v, next = toks[j]?.v;
      if (KEYWORDS.has(v)) cls = 'kw';
      else if (CONSTS.has(v)) cls = 'num';
      else if (prev === 'def' || prev === 'class') cls = 'fn';
      else if (prev === '.') cls = next === '(' ? 'fn' : 'col';
      else if (v in GIVEN) cls = 'tbl';
      else if (BUILTINS.has(v)) cls = next === '(' ? 'type' : 'id';
      else if (next === '(') cls = 'fn';
      else cls = 'id';
    } else if (t === 'str') {
      // a column of the result, in df['…']
      const inner = v.replace(/^[rRbBuUfF]*(['"])/, '').replace(/['"]$/, '');
      if (cols.has(inner) && toks[i - 1]?.v === '[') cls = 'col';
    }
    html += `<span class="s-${cls}">${esc(v)}</span>`;
  }
  return html + (text.endsWith('\n') || !text ? ' ' : '');
}

/** Names assigned in the script (a = …, for a in …, def a, import a as b). */
function locals(text) {
  const out = new Set();
  for (const m of text.matchAll(/^[ \t]*([A-Za-z_]\w*)(?:\s*,\s*[A-Za-z_]\w*)*\s*=(?!=)/gm)) out.add(m[1]);
  for (const m of text.matchAll(/\bfor\s+([A-Za-z_]\w*)/g)) out.add(m[1]);
  for (const m of text.matchAll(/\b(?:def|class)\s+([A-Za-z_]\w*)/g)) out.add(m[1]);
  for (const m of text.matchAll(/\bimport\s+[\w.]+\s+as\s+([A-Za-z_]\w*)/g)) out.add(m[1]);
  return out;
}

/**
 * Completion at `caret`: { items: [{label, kind, detail, insert}], prefix }.
 * `ctx`: columns() (the last result's), sources() (data source names), params() ([{name, value}]).
 */
export function complete(text, caret, ctx = {}) {
  const before = text.slice(0, caret);
  // inside df['…  /  sources["…  /  params["…: what goes in the quotes
  const q = before.match(/\b([A-Za-z_]\w*)\s*\[\s*(['"])([^'"\n]*)$/);
  if (q) {
    const [, obj, quote, typed] = q;
    let names = [];
    if (obj === 'sources') names = (ctx.sources?.() || []).map((n) => ({ label: n, kind: 'tbl', detail: 'data source' }));
    else if (obj === 'params') names = (ctx.params?.() || []).map((p) => ({ label: p.name, kind: 'param', detail: String(p.value ?? '') }));
    else names = (ctx.columns?.() || []).map((c) => ({ label: String(c), kind: 'col', detail: 'column of df' }));
    const p = typed.toLowerCase();
    const items = names.filter((x) => x.label.toLowerCase().includes(p)).sort((a, b) => a.label.toLowerCase().startsWith(p) === b.label.toLowerCase().startsWith(p) ? 0 : a.label.toLowerCase().startsWith(p) ? -1 : 1)
      .map((x) => ({ ...x, insert: `${x.label}${quote}]` }));
    // the closing quote and bracket, when they aren't there already
    const after = text.slice(caret);
    if (after.startsWith(quote + ']')) items.forEach((x) => { x.insert = x.label; });
    return { items: items.slice(0, 60), prefix: typed };
  }
  // after "x." : what x has
  const dot = before.match(/([A-Za-z_]\w*|\))\s*\.\s*([A-Za-z_]\w*)?$/);
  if (dot) {
    const [, obj, typed = ''] = dot;
    const list = obj === 'pd' ? PD.map((n) => ({ label: n, kind: ATTRS.has(n) || n === 'NaT' || n === 'NA' ? 'col' : 'fn', detail: 'pandas' }))
      : obj === 'np' ? NP.map((n) => ({ label: n, kind: n === 'nan' ? 'col' : 'fn', detail: 'numpy' }))
        : DF.map((n) => ({ label: n, kind: ATTRS.has(n) ? 'col' : 'fn', detail: obj === 'df' ? 'DataFrame' : 'DataFrame / Series' }));
    // a column as an attribute (df.value), when it's a plain name
    if (obj === 'df') for (const c of ctx.columns?.() || []) if (/^[A-Za-z_]\w*$/.test(String(c))) list.unshift({ label: String(c), kind: 'col', detail: 'column of df' });
    const p = typed.toLowerCase();
    const items = list.filter((x) => x.label.toLowerCase().startsWith(p) && x.label.toLowerCase() !== p)
      .map((x) => ({ ...x, insert: x.kind === 'fn' ? `${x.label}()` : x.label }));
    return { items: items.slice(0, 60), prefix: typed };
  }
  const m = before.match(/([A-Za-z_]\w*)$/);
  const typed = m ? m[1] : '';
  const p = typed.toLowerCase();
  const seen = new Set();
  const add = (arr) => arr.filter((x) => { if (seen.has(x.label)) return false; seen.add(x.label); return true; });
  const all = [
    ...add(Object.entries(GIVEN).map(([k, d]) => ({ label: k, kind: 'tbl', detail: d }))),
    ...add([...locals(text)].filter((v) => v !== typed).map((v) => ({ label: v, kind: 'id', detail: 'variable' }))),
    ...add([...KEYWORDS].map((k) => ({ label: k, kind: 'kw', detail: 'keyword' }))),
    ...add([...CONSTS].map((k) => ({ label: k, kind: 'num', detail: 'constant' }))),
    ...add([...BUILTINS].map((b) => ({ label: b, kind: 'fn', detail: 'built-in', insert: `${b}()` }))),
  ];
  const items = all.filter((x) => x.label.toLowerCase().startsWith(p) && x.label.toLowerCase() !== p);
  return { items: items.slice(0, 60), prefix: typed };
}
