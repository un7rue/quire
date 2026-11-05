// Tables: column formats, conditional rules, pills, data bars, colour scales
// and arrows. A styled table is a grid of cells that renders to SVG (screen,
// PDF) and to a native PowerPoint table (or an exact picture when it uses
// pills or bars, which PowerPoint tables can't draw).
//
// Style lives in widget.opts.table:
//   size, header {fill,color,upper,bold}, zebra, zebraFill, lines ('rows'|'grid'|'header'|'none'),
//   totalLabel, pptx ('auto'|'native'|'picture'), row_h, head_h (pt), lineColor, headLineColor, headLineWidth,
//   groupFill ('none' = no fill), groupColor, groupUpper, groupLine (rule under group rows), groupCount,
//   indent (false = no tree indent), totalFill, totalColor, totalLine ('single'|'double'|'thick'|'none'),
//   totalFont {family,size,weight} for the totals label, totalLabel may use {n} (number of rows),
//   column extras: swatch ('square'|'dot'|'tile'), tag (key of a column shown as a pill after the text),
//   bar.pos ('below'), bar.scope ('row'), bar.base ('zero': diverging from zero), bar.trackFill, bar.ticks (divisions), bar.h (pt),
//   size (× the cell size), weight, subWeight, sub may list several keys (joined with ' · '), badge (rank number in a dot on a pill row),
//   rules[].rowPill (a rounded fill for the whole row, with stroke), band (fill for the whole column), rule ('left'|'right'), rules[].pill,
//   icon {map: {value: glyph}, default, shape ('circle'|'square'|'none'), fill, fillMap, color}, swatchMap {value: colour},
//   rules[].icon (a glyph chip after the row's name, with iconFill / iconColor),
//   cols: { [columnKey]: {
//     label, hidden, align, width (% of table), format, decimals, prefix, suffix, bold, color, fill,
//     arrows, signColor, bar {on,color}, scale {on,from,to},
//     pill {on, fill, color, rules:[{match, fill, color}]},
//     rules: [{op, value, value2, color, fill, bold, row}] } }

import { ellipsize, esc, fmtNum, fmtDate, isIsoDate, textWidth, inkOn, mixHex, fontStack, getLocale, legible } from './util.js';
import { glyph, glyphFor, glyphWidth, letterText } from './glyphs.js';

const r2 = (v) => Math.round(v * 100) / 100;

export const PILL_PRESETS = [
  { name: 'Green', fill: '#e3f4e7', color: '#0f6b2c' },
  { name: 'Red', fill: '#fde6e4', color: '#b42318' },
  { name: 'Blue', fill: '#e1ecfb', color: '#1c5cab' },
  { name: 'Amber', fill: '#fff1d6', color: '#8a5a00' },
  { name: 'Violet', fill: '#ece9fb', color: '#4a3aa7' },
  { name: 'Grey', fill: '#eceae4', color: '#55534e' },
  { name: 'Solid green', fill: '#1d7f3a', color: '#ffffff' },
  { name: 'Solid red', fill: '#d03b3b', color: '#ffffff' },
  { name: 'Solid blue', fill: '#2a78d6', color: '#ffffff' },
  { name: 'Solid ink', fill: '#1b1b19', color: '#ffffff' },
];

/** A sensible pill colour for common words (buy/sell, up/down, yes/no…). */
export function guessPill(value) {
  const v = String(value).trim().toLowerCase();
  if (/^(buy|long|bull|up|yes|y|true|win|won|ok|pass|done|active|paid|approved|open|in)$/.test(v)) return PILL_PRESETS[0];
  if (/^(sell|short|bear|down|no|n|false|loss|lost|fail|failed|overdue|rejected|closed|out|cancel(l)?ed)$/.test(v)) return PILL_PRESETS[1];
  if (/^(hold|pending|wait|neutral|flat|draft|review)$/.test(v)) return PILL_PRESETS[3];
  return null;
}

export const RULE_OPS = [
  ['>', 'greater than'], ['>=', 'at least'], ['<', 'less than'], ['<=', 'at most'],
  ['=', 'equals'], ['!=', 'is not'], ['between', 'between'], ['contains', 'contains'],
  ['positive', 'is positive'], ['negative', 'is negative'], ['empty', 'is empty'],
  ['top', 'is in top N'], ['bottom', 'is in bottom N'],
];

function ruleMatches(rule, v, stats) {
  const num = typeof v === 'number' ? v : null;
  const a = rule.value === '' || rule.value == null ? null : Number(rule.value);
  const b = rule.value2 === '' || rule.value2 == null ? null : Number(rule.value2);
  const text = v == null ? '' : String(v);
  switch (rule.op) {
    case '>': return num != null && a != null && num > a;
    case '>=': return num != null && a != null && num >= a;
    case '<': return num != null && a != null && num < a;
    case '<=': return num != null && a != null && num <= a;
    case 'between': return num != null && a != null && b != null && num >= Math.min(a, b) && num <= Math.max(a, b);
    case '=': return num != null && a != null && !isNaN(a) ? num === a : text.toLowerCase() === String(rule.value ?? '').toLowerCase();
    case '!=': return num != null && a != null && !isNaN(a) ? num !== a : text.toLowerCase() !== String(rule.value ?? '').toLowerCase();
    case 'contains': return text.toLowerCase().includes(String(rule.value ?? '').toLowerCase());
    case 'positive': return num != null && num > 0;
    case 'negative': return num != null && num < 0;
    case 'empty': return v == null || text === '';
    case 'top': return num != null && stats.desc.slice(0, Math.max(1, a || 3)).includes(num);
    case 'bottom': return num != null && stats.asc.slice(0, Math.max(1, a || 3)).includes(num);
    default: return false;
  }
}

/** Formats one value the way its column asks. */
export function formatCell(v, col, cs, currency) {
  if (v == null || v === '') return '';
  if (v === 0 && cs.zero === 'dash') return '—';
  const fmt = cs.format && cs.format !== 'auto' ? cs.format : isIsoDate(v) ? 'date' : col.role === 'measure' ? col.format || 'number' : 'text';
  let s;
  if (fmt === 'date') s = fmtDate(v);
  else if (fmt === 'text' || typeof v !== 'number') s = String(v);
  else s = fmtNum(v, fmt, { currency, decimals: cs.decimals, signed: !!cs.signed, compact: !!cs.compact && fmt !== 'percent' });
  return `${cs.prefix || ''}${s}${cs.suffix || ''}`;
}

export function usesPicture(w) {
  const t = w.opts?.table || {};
  if (t.pptx === 'native') return false;
  if (t.pptx === 'picture') return true;
  return Object.values(t.cols || {}).some((c) => c.pill?.on || c.bar?.on || c.icons?.on || c.sub || c.swatch || c.icon || c.tag || c.band || c.rule || (c.rules || []).some((r) => r.accent || r.pill || r.icon)) || !!t.groupLine || (w.binding?.measures || []).some((m) => m.calc === 'trend');
}

/** Applies a saved display order (column keys); unknown columns keep their place at the end. */
export function orderColumns(cols, order) {
  if (!order?.length) return cols;
  // pivot columns follow their value's position
  const rank = (c) => { let i = order.indexOf(c.key); if (i < 0 && c.base) i = order.indexOf(c.base); return i < 0 ? 1e6 : i; };
  return cols.map((c, i) => [c, i]).sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1]).map((x) => x[0]);
}

/** The totals row's default label by language. */
const TOTAL = { ru: 'Итого', de: 'Gesamt' };

export const ICON_SETS = [['traffic', 'Traffic lights ●'], ['arrows', 'Arrows ▲ ▶ ▼'], ['checks', 'Checks ✓ ! ✗']];

/** Columns as the table shows them: styles resolved, order applied, a tree column for grouped rows. */
export function displayColumns(w, data) {
  const ts = w.opts?.table || {};
  const styleOf = (c) => ({ ...(c.base && c.base !== c.key ? ts.cols?.[c.base] : null), ...(ts.cols?.[c.key] || {}) });
  let all = (data?.columns || []).map((c, j) => ({ ...c, j, s: styleOf(c) }));
  if (data?.row_meta) {
    // grouped rows: the dimension columns become one indented tree column
    const dims = all.filter((c) => c.role === 'dim');
    const at = all.findIndex((c) => c.role === 'dim');
    const tree = { key: '__tree', label: dims.map((d) => d.label).join(' / '), role: 'dim', j: -1, tree: true, dims: dims.map((d) => d.j), s: ts.cols?.__tree || {} };
    all = all.filter((c) => c.role !== 'dim');
    all.splice(Math.max(0, at), 0, tree);
  }
  return orderColumns(all, ts.order);
}

/** Lays out a styled table into rows of cells (points, page coordinates). */
export function tablePart(st, w, vm, data) {
  const th = st.doc.theme;
  const t = vm.table;
  const b = vm.body;
  const ts = w.opts?.table || {};
  const all = displayColumns(w, data);
  const cols = all.filter((c) => !c.s.hidden);
  const allRows = data?.rows || [];
  const meta = data?.row_meta || null;
  if (!cols.length) return null;
  const cf = vm.fonts?.table_cell || { size: t.font, weight: 400, family: 'Inter', italic: false, color: th.ink };
  const hf = vm.fonts?.table_head || { size: t.font, weight: 600, family: 'Inter', italic: false, color: th.muted };
  const size = cf.size;
  // space at the cell's sides (Lines & rows → Cell padding), 5 pt unless set
  const pad = ts.cellPad != null && ts.cellPad !== '' ? Math.max(0, Math.min(20, +ts.cellPad)) : 5;
  const currency = th.currency;
  const fam = (c) => c.s.family || cf.family;
  const ital = (c) => c.s.italic ?? cf.italic;
  const W = (text, c, weight) => textWidth(text, size * (c.s.size || 1), weight, fam(c), ital(c));
  const off = t.offset || 0;
  const rows = allRows.slice(off, off + t.visible);
  const metas = meta ? meta.slice(off, off + t.visible) : null;
  const leaf = (k) => !meta || meta[k]?.kind === 'row';
  // columns usable as a second line, by key (hidden ones too)
  const subCols = Object.fromEntries((data?.columns || []).map((c, j) => [c.key, { ...c, j, s: ts.cols?.[c.key] || {} }]));

  // per-column numeric stats over the leaf rows (scales, bars, top/bottom rules, totals)
  const stats = {};
  for (const c of cols) {
    const nums = c.tree ? [] : allRows.filter((_, k) => leaf(k)).map((r) => r[c.j]).filter((v) => typeof v === 'number');
    stats[c.j] = { nums, min: Math.min(0, ...nums), max: Math.max(0, ...nums), lo: Math.min(...nums), hi: Math.max(...nums), desc: [...nums].sort((a, z) => z - a), asc: [...nums].sort((a, z) => a - z) };
  }
  const alignOf = (c) => (c.s.align && c.s.align !== 'auto' ? c.s.align : c.role === 'measure' ? 'right' : 'left');
  const groupFill = ts.groupFill === 'none' ? null : ts.groupFill || '#f1f0eb';
  // category colours follow the same order as charts (alphabetical → palette)
  const swatchOrder = {};
  const swatchColor = (c, value) => {
    const key = c.key;
    if (!swatchOrder[key]) {
      const j = c.tree ? c.dims[0] : c.j;
      swatchOrder[key] = [...new Set(allRows.map((r) => String(r[j] ?? '')))].sort((a, z) => a.localeCompare(z));
    }
    if (c.s.swatchMap) { const m = glyphFor(c.s.swatchMap, value, ''); if (m) return m; }
    const pal = c.s.swatchColors || th.palette;
    const i = swatchOrder[key].indexOf(String(value ?? ''));
    return pal[(i < 0 ? 0 : i) % pal.length];
  };
  // rows under each group (for "Shares · 11")
  const groupCount = (k) => {
    const lvl = meta[k].level ?? 0;
    let n = 0;
    for (let q = k + 1; q < meta.length; q++) {
      if (meta[q].kind === 'group' && (meta[q].level ?? 0) <= lvl) break;
      if (meta[q].kind === 'row') n++;
    }
    return n;
  };
  const ruleOnly = all.filter((c) => c.s.hidden && !c.tree && (c.s.rules || []).length);
  for (const c of ruleOnly) {
    const nums = allRows.filter((_, k) => leaf(k)).map((r) => r[c.j]).filter((v) => typeof v === 'number');
    stats[c.j] = { nums, min: Math.min(0, ...nums), max: Math.max(0, ...nums), lo: Math.min(...nums), hi: Math.max(...nums), desc: [...nums].sort((a, z) => z - a), asc: [...nums].sort((a, z) => a - z) };
  }
  const leafCount = meta ? meta.filter((m) => m.kind === 'row').length : allRows.length;
  const tagCols = Object.fromEntries((data?.columns || []).map((c, j) => [c.key, { ...c, j, s: ts.cols?.[c.key] || {} }]));

  function cell(c, v, row, m) {
    const s = c.s;
    let value = v;
    let indent = 0;
    if (c.tree) {
      const level = m?.level ?? 0;
      value = row[c.dims[Math.min(level, c.dims.length - 1)]];
      indent = ts.indent === false ? 0 : level * size * 1.2;
    }
    const out = { text: formatCell(value, c, s, currency), color: s.color || cf.color, fill: s.fill || null, bold: !!s.bold, align: alignOf(c), pill: null, bar: null, icon: null, spark: null, indent, family: fam(c), italic: ital(c), weight: s.weight || (s.bold ? Math.max(600, cf.weight) : cf.weight), size: s.size ? size * s.size : undefined };
    if (Array.isArray(v)) {
      out.text = '';
      out.spark = { values: v, color: s.color || th.palette[0] };
      return { cell: out, rowStyle: null };
    }
    if (typeof v === 'number') {
      if (s.signColor && v !== 0) out.color = v > 0 ? th.good : th.bad;
      if (s.arrows && v !== 0) {
        out.text = `${v > 0 ? '▲' : '▼'} ${out.text.replace(/^[−-]/, '')}`;
        out.color = s.signColor === false ? out.color : v > 0 ? th.good : th.bad;
      }
      if (s.scale?.on) {
        const st2 = stats[c.j];
        const k = st2.hi > st2.lo ? (v - st2.lo) / (st2.hi - st2.lo) : 1;
        out.fill = mixHex(s.scale.from || '#ffffff', s.scale.to || '#2a78d6', Math.max(0, Math.min(1, k)));
        out.color = inkOn(out.fill);
      }
      if (s.bar?.on) {
        const st2 = stats[c.j];
        let span = Math.max(st2.max, -st2.min) || 1;
        if (s.bar.scope === 'row') {
          // share of the row: this column against its siblings (pivot columns of one value)
          const sib = cols.filter((x) => !x.tree && (x.base || x.key) === (c.base || c.key) && typeof row[x.j] === 'number');
          span = sib.reduce((a, x) => a + Math.abs(row[x.j]), 0) || 1;
        }
        out.bar = { frac: Math.abs(v) / span, neg: v < 0, color: v < 0 ? s.bar.negColor || th.bad : s.bar.color || th.palette[0], below: s.bar.pos === 'below', track: s.bar.track, k: { thin: 0.55, thick: 1.5 }[s.bar.size] || 1, round: !!s.bar.round,
          trackFill: s.bar.trackFill || null, ticks: s.bar.ticks || 0, h: s.bar.h || null, opacity: s.bar.opacity ?? 0.85, strokeWidth: s.bar.strokeWidth || 0.5 };
        // an outline: its own colour, or the bar's colour darker ('auto')
        if (s.bar.stroke) out.bar.stroke = s.bar.stroke === 'auto' ? mixHex(out.bar.color, '#000000', 0.35) : s.bar.stroke;
        if (s.bar.base === 'zero') {
          // diverging: the bar runs from zero, the scale spans the lowest to the highest value
          const lo = Math.min(0, st2.lo), hi = Math.max(0, st2.hi), w2 = hi - lo || 1;
          out.bar.zero = -lo / w2;
          out.bar.to = (v - lo) / w2;
        }
      }
      if (s.icons?.on) {
        const hi = Number(s.icons.hi), lo = Number(s.icons.lo);
        let level = v >= hi ? 2 : v >= lo ? 1 : 0;
        if (s.icons.reverse) level = 2 - level;
        out.icon = { type: s.icons.type || 'traffic', level, color: [th.bad, '#e0a100', th.good][level] };
      }
    }
    let rowStyle = null;
    for (const rule of s.rules || []) {
      if (c.tree || !ruleMatches(rule, v, stats[c.j])) continue;
      const style = { color: rule.color || null, fill: rule.fill || null, bold: !!rule.bold, accent: rule.accent ? (rule.accent === true ? rule.color || rule.fill || th.bad : rule.accent) : null };
      if ((rule.note || rule.tag || rule.icon) && m?.kind === 'group') continue;
      if (rule.icon) rowStyle = { ...(rowStyle || {}), mark: { glyph: rule.icon, fill: rule.iconFill || mixHex(rule.iconColor || th.ink, th.page_bg, 0.8), color: rule.iconColor || th.ink } };
      if (rule.icon && !rule.note && !rule.tag && !rule.fill && !rule.accent && !rule.color && !rule.bold) continue;
      if (rule.note || rule.tag) {
        // a label for the whole row: a line under its name, or a tag after it
        const fill = (t2) => String(t2).replace('{v}', typeof v === 'number' ? fmtNum(v, 'number', { decimals: 0 }) : String(v ?? ''));
        rowStyle = { ...(rowStyle || {}), ...(rule.note ? { note: fill(rule.note), noteColor: rule.noteColor || rule.color || null } : {}), ...(rule.tag ? { tag: { text: fill(rule.tag), fill: rule.tagFill || rule.fill || '#eceae4', color: rule.tagColor || rule.color || th.ink, at: rule.tagAt || null } } : {}) };
        if (!rule.fill && !rule.accent && !rule.color && !rule.bold) continue;
      }
      if (rule.rowPill) { rowStyle = { ...(rowStyle || {}), pill: { fill: rule.fill || mixHex(th.good, th.page_bg, 0.86), stroke: rule.stroke || null }, bold: !!rule.bold }; continue; }
      if (rule.pill) { out.rulePill = { fill: rule.fill || '#eceae4', color: legible(rule.color || th.ink, rule.fill || '#eceae4', 4.5) }; if (rule.bold) { out.bold = true; out.weight = Math.max(600, out.weight); } continue; }
      if (rule.row || rule.accent) rowStyle = { ...(rowStyle || {}), ...Object.fromEntries(Object.entries(style).filter(([k, x]) => x && (rule.row || k === 'accent'))) };
      else {
        if (style.color) out.color = style.color;
        if (style.fill) { out.fill = style.fill; if (!rule.color) out.color = inkOn(style.fill) === '#ffffff' ? '#ffffff' : out.color; }
        if (style.bold) { out.bold = true; out.weight = Math.max(600, out.weight); }
      }
    }
    if (s.sub && !t?.no_sub && (!c.tree || m?.kind !== 'group')) {
      // one column, or several joined: "+2,5% · 11 624 000"
      if (s.subWeight) out.subWeight = s.subWeight;
      out.sub = [].concat(s.sub).map((key) => subCols[key]).filter(Boolean).map((sc) => formatCell(row[sc.j], sc, sc.s || {}, currency)).filter(Boolean).join(' · ') || undefined;
    }
    if (s.bar?.on && s.bar.only) out.text = '';
    if (s.pill?.on && out.text) {
      const pm = (s.pill.rules || []).find((r) => String(r.match ?? '').toLowerCase() === String(value).toLowerCase());
      const g = pm || guessPill(value) || { fill: s.pill.fill || '#eceae4', color: s.pill.color || '#55534e' };
      out.pill = { fill: g.fill, color: legible(g.color, g.fill, 4.5) };
    }
    if (out.rulePill && out.text) out.pill = out.rulePill;
    if (s.swatch && out.text && m?.kind !== 'group') out.icon = { type: 'swatch', shape: s.swatch, color: swatchColor(c, value) };
    if (s.icon && out.text && m?.kind !== 'group') {
      const ic = typeof s.icon === 'string' ? { default: s.icon } : s.icon;
      const name = glyphFor(ic.map, value, ic.default || '', ic.auto);
      if (name) out.icon = { type: 'glyph', name, scale: Math.max(0.3, Math.min(2, Number(ic.size) || 1)), shape: ic.shape || 'circle', fill: glyphFor(ic.fillMap, value, ic.fill || '#e9dfcc'), color: glyphFor(ic.colorMap, value, ic.color || '') || (letterText(name) ? null : '#3f382b'), stroke: ic.stroke || null, strokeWidth: ic.strokeWidth || 0.75 };
    }
    // the marker alone: the value's text hidden (a value without a marker keeps its text)
    if (s.iconOnly && out.icon && out.text) { out.text = ''; out.pill = null; out.iconOnly = true; }
    // a tag: another column's value as a small pill after the text
    if (s.tag && tagCols[s.tag] && (!c.tree || m?.kind !== 'group')) {
      const tc = tagCols[s.tag];
      const tv = row[tc.j];
      if (tv != null && tv !== '') {
        const ts2 = tc.s.pill || {};
        const pm = (ts2.rules || []).find((r) => String(r.match ?? '').toLowerCase() === String(tv).toLowerCase());
        const g = pm || guessPill(tv) || { fill: ts2.fill || '#eceae4', color: ts2.color || '#55534e' };
        out.tag = { text: String(tv), fill: g.fill, color: legible(g.color, g.fill, 4.5) };
      }
    }
    return { cell: out, rowStyle };
  }

  // best and worst rows: one or more rankings, each by its own value. A row
  // can be hit by several (best return, worst result): it shows every icon in
  // list order, and the first ranking that highlights rows decides its fill.
  const rankings = [].concat(ts.extremes || []).filter((x) => x && x.on);
  const hits = new Map();
  rankings.forEach((X, xi) => {
    const xj = (data?.columns || []).findIndex((c) => c.key === X.by);
    if (xj < 0) return;
    // a zero (cash, a position with no result) is never best or worst, whatever the rule
    const ranked = allRows.map((r, k) => [k, r[xj]]).filter(([k, v]) => leaf(k) && typeof v === 'number' && Math.abs(v) > 1e-9).sort((a, z) => z[1] - a[1]);
    // all equal: nothing stands out
    if (ranked.length < 2 || ranked[0][1] === ranked[ranked.length - 1][1]) return;
    if (X.reverse) ranked.reverse();
    const nb = X.best?.n ?? 1, nw = X.worst?.n ?? 1;
    // 'sign' (default): only gains can be best and only losses worst — all-positive rows have no worst, a zero is neither
    const gain = (v) => (X.reverse ? -v : v);
    const okBest = ([, v]) => X.rule === 'rank' || gain(v) > 0;
    const okWorst = ([, v]) => X.rule === 'rank' || gain(v) < 0;
    // place: 0 for the very best (or worst), 1 for the next…
    const add = (k, kind, place) => { if (!hits.has(k)) hits.set(k, []); hits.get(k).push({ X, xi, kind, place }); };
    ranked.slice(0, nb).filter(okBest).forEach(([k], p) => add(k, 'best', p));
    const tail = ranked.slice(Math.max(nb, ranked.length - nw)).filter(okWorst);
    tail.forEach(([k], p) => add(k, 'worst', tail.length - 1 - p));
  });
  const rankStyle = (list) => {
    const out = { marks: [] };
    const notes = [];
    for (const { X, kind, place } of list) {
      const e = X[kind] || {};
      const color = e.color || (kind === 'best' ? th.good : th.bad);
      const show = X.show || 'icon';
      // with 2–3 rows each side, the highlight fades by place: 1st strongest, then lighter
      const fade = X.fade === false ? 0 : Math.min(2, place || 0);
      const toPage = (c, k) => (fade ? mixHex(c, th.page_bg, k) : c);
      if (show !== 'row') out.marks.push({ glyph: e.icon || (kind === 'best' ? 'trend-up' : 'trend-down'), fill: e.iconFill || mixHex(color, th.page_bg, 0.8), color });
      if (show !== 'icon' && !out.fill) {
        // clear steps: about 30%, 20% and 12% of the colour over the page
        // several rows on this side: clear steps from the colour itself (30%, 20%, 12%);
        // one row keeps the design's tint
        const steps = X.fade !== false && (e.n ?? 1) > 1;
        // a design tint (the theme's soft gain / loss) leads; further places fade from it to the page
        out.fill = e.fill ? (steps ? mixHex(e.fill, th.page_bg, [0, 0.4, 0.65][fade]) : e.fill) : steps ? mixHex(color, th.page_bg, [0.7, 0.8, 0.88][fade]) : mixHex(color, th.page_bg, 0.82);
        out.accent = toPage(color, fade * 0.3);
      }
      if (e.label) notes.push(e.label);
    }
    if (notes.length) { out.note = notes.join(' · '); out.noteColor = list[0] && (list[0].X[list[0].kind]?.color || (list[0].kind === 'best' ? th.good : th.bad)); }
    return out;
  };

  // body rows
  const clipped = !t.continued && off + rows.length < allRows.length;
  const shown = clipped ? rows.slice(0, Math.max(0, rows.length - 1)) : rows;
  // a piece that starts inside a group repeats its subheader(s) on top, marked as continued
  const carry = meta ? (t.carry || []).filter((g) => meta[g]?.kind === 'group') : [];
  const CONT = { ru: 'продолжение', de: 'Fortsetzung' }[getLocale().slice(0, 2)] || 'continued';
  const entries = [...carry.map((g) => ({ r: allRows[g], g, i: -1 })), ...shown.map((r, i) => ({ r, g: off + i, i }))];
  const body = entries.map(({ r, g, i }) => {
    const m = meta ? meta[g] : undefined;
    const carried = i < 0;
    let rowStyle = null;
    const group = m?.kind === 'group';
    const cells = cols.map((c) => {
      const x = cell(c, c.tree ? null : r[c.j], r, m);
      if (x.rowStyle) rowStyle = { ...(rowStyle || {}), ...x.rowStyle };
      if (i >= 0 && t.wrapped?.[c.key]?.[i] && !c.tree) x.cell.lines = t.wrapped[c.key][i];
      if (group && !c.tree && (ts.groupValues === false || c.s.groupHide)) { x.cell.text = ''; x.cell.bar = null; x.cell.pill = null; x.cell.icon = null; }
      return x.cell;
    });
    // hidden columns can still style the row through their rules
    for (const c of ruleOnly) {
      const x = cell(c, r[c.j], r, m);
      if (x.rowStyle) rowStyle = { ...(rowStyle || {}), ...x.rowStyle };
    }
    if (!group && hits.has(g)) {
      const x = rankStyle(hits.get(g));
      rowStyle = { ...(rowStyle || {}), ...x, marks: [...(rowStyle?.mark ? [rowStyle.mark] : []), ...x.marks], fill: rowStyle?.fill || x.fill, accent: rowStyle?.accent || x.accent };
    }
    if (group) {
      const gname = String(r[(cols.find((c) => c.tree)?.dims || [])[m.level ?? 0]] ?? '');
      const gcol = ts.groupColors?.[gname] || ts.groupColor;
      for (const cl of cells) { cl.bold = true; cl.weight = Math.max(600, cl.weight); if (!cl.fill && groupFill) cl.fill = groupFill; if (gcol && !cl.pill) cl.color = gcol; }
      const tc = cols.findIndex((c) => c.tree);
      if (tc >= 0) {
        let label = cells[tc].text;
        if (ts.groupCount) label += ` · ${groupCount(g)}`;
        if (carried) label += ` (${CONT})`;
        cells[tc].text = ts.groupUpper ? label.toUpperCase() : label;
        // a repeated subheader repeats the label only, not the group's sums
        if (carried) cells.forEach((cl, k2) => { if (k2 !== tc) { cl.text = ''; cl.bar = null; cl.pill = null; cl.icon = null; } });
        if (ts.groupUpper) cells[tc].tracking = 0.06;
      }
    }
    if (rowStyle) for (const cl of cells) {
      if (rowStyle.fill && (!cl.fill || cl.fill === groupFill)) cl.fill = rowStyle.fill;
      if (rowStyle.color && !cl.pill) cl.color = rowStyle.color;
      if (rowStyle.bold) { cl.bold = true; cl.weight = Math.max(600, cl.weight); }
    }
    const marks = rowStyle?.marks || (rowStyle?.mark ? [rowStyle.mark] : []);
    if (marks.length) {
      const first = cols.findIndex((c) => c.role !== 'measure');
      cells[first >= 0 ? first : 0].marks = marks;
    }
    if (rowStyle?.note || rowStyle?.tag) {
      const first = cols.findIndex((c) => c.role !== 'measure');
      const at = first >= 0 ? first : 0;
      if (rowStyle.note) { cells[at].sub = rowStyle.note; cells[at].subColor = rowStyle.noteColor; cells[at].subBold = true; }
      if (rowStyle.tag) {
        const k2 = rowStyle.tag.at ? cols.findIndex((c) => c.key === rowStyle.tag.at || c.base === rowStyle.tag.at) : -1;
        cells[k2 >= 0 ? k2 : at].tag = rowStyle.tag;
      }
    }
    // a rank number in a dot on the highlighted row
    if (rowStyle?.pill) cols.forEach((c, k2) => { if (c.s.badge) cells[k2].badge = c.s.badge === true ? th.good : c.s.badge; });
    return { h: i >= 0 ? t.heights?.[i] ?? t.row_h : (t.heights?.[0] ?? t.row_h), cells, zebra: ts.zebra && !group && i % 2 === 1, group, groupLine: group && ts.groupLine ? ts.groupLine : null, accent: rowStyle?.accent || null, pill: rowStyle?.pill || null };
  });

  // totals: the engine's (correct for balances and ratios) unless a column asks otherwise
  const hd = ts.header || {};
  const tr = data.total_row || [];
  const totalValue = (c) => {
    const mode = c.s.total || 'auto';
    if (c.tree || mode === 'none') return null;
    if (mode === 'auto') return tr[c.j];
    const nums = stats[c.j].nums;
    if (!nums.length) return null;
    if (mode === 'sum') return nums.reduce((a, x) => a + x, 0);
    if (mode === 'avg') return nums.reduce((a, x) => a + x, 0) / nums.length;
    if (mode === 'min') return Math.min(...nums);
    if (mode === 'max') return Math.max(...nums);
    if (mode === 'count') return nums.length;
    return tr[c.j];
  };
  const totalText = cols.map((c) => {
    const v = t.totals ? totalValue(c) : null;
    return typeof v === 'number' ? formatCell(v, c, c.s.total === 'count' ? { ...c.s, format: 'number', decimals: 0, prefix: '', suffix: '' } : c.s, currency) : '';
  });
  const caps = hd.upper ?? hf.upper;
  const headText = cols.map((c) => { const l = c.s.label || c.label; return caps ? l.toUpperCase() : l; });
  const HW = (t2) => Math.max(...String(t2).split('\n').map((l) => textWidth(l, hf.size, hf.weight, hf.family, hf.italic, hf.tracking || 0)));

  // widths: content-sized, % overrides (wrapping columns always take theirs), spare room shared
  const pillPad = 12, barMin = 40;
  const iconWOf = (ic) => (ic.type === 'swatch' ? (ic.shape === 'tile' ? size * 2.6 : size * 1.25) : ic.type === 'glyph' ? glyphWidth(ic.name, size * 1.9 * (ic.scale || 1)) + size * 0.7 : size * 1.15);
  const tagW = (tg) => textWidth(tg.text.toUpperCase(), size * 0.74, 700, cf.family, false, 0.04) + 12 + 6;
  const natural = cols.map((c, k) => {
    let m = Math.max(HW(headText[k]), W(totalText[k], c, 600));
    for (const row of body) {
      const cl = row.cells[k];
      m = Math.max(m, W(cl.text, c, cl.weight) + (cl.pill ? pillPad : 0) + (cl.icon ? iconWOf(cl.icon) : 0) + (cl.indent || 0) + (cl.tag ? tagW(cl.tag) : 0) + (cl.marks ? size * (0.4 + 1.6 * cl.marks.length) : 0), cl.sub ? textWidth(cl.sub, size * 0.82, 400, fam(c), ital(c)) : 0);
    }
    if (c.s.bar?.on && !c.s.bar.pos) m = c.s.bar.only ? Math.max(60, HW(headText[k])) : m + barMin;
    if (body.some((r) => r.cells[k].spark)) m = Math.max(m, 56);
    return Math.min(m + pad * 2, b.w * 0.6);
  });
  let widths = [...natural];
  const pctOf = (c) => (c.s.width > 0 ? c.s.width : c.s.wrap ? 30 : 0);
  const fixedIdx = cols.map((c, k) => (pctOf(c) > 0 ? k : -1)).filter((k) => k >= 0);
  const fixedW = fixedIdx.reduce((a, k) => a + (b.w * Math.min(90, pctOf(cols[k]))) / 100, 0);
  fixedIdx.forEach((k) => { widths[k] = (b.w * Math.min(90, pctOf(cols[k]))) / 100; });
  // a number, a rank or a pill can't be cut short: a width set too narrow for it still fits it
  // and text keeps its icon and a few letters
  const whole = (k) => cols[k].role === 'measure' || !!cols[k].s.pill?.on;
  const mins = cols.map((c, k) => {
    if (whole(k)) return natural[k];
    const ic = body.map((r) => r.cells[k].icon).find(Boolean);
    return Math.min(natural[k], pad * 2 + (ic ? iconWOf(ic) : 0) + W('Абвг…', c, 400));
  });
  fixedIdx.forEach((k) => { widths[k] = Math.max(widths[k], mins[k]); });
  const autoIdx = cols.map((_, k) => k).filter((k) => !fixedIdx.includes(k));
  const autoSum = autoIdx.reduce((a, k) => a + natural[k], 0);
  const room = Math.max(0, b.w - fixedW);
  if (autoIdx.length) {
    if (autoSum <= room) {
      const spare = room - autoSum;
      const lead = autoIdx.find((k) => cols[k].role !== 'measure') ?? autoIdx[0];
      widths[lead] += autoIdx.length > 1 ? spare / 2 : spare;
      if (autoIdx.length > 1) autoIdx.forEach((k) => { widths[k] += (spare / 2) * (natural[k] / autoSum); });
    } else autoIdx.forEach((k) => { widths[k] = (natural[k] / autoSum) * room; });
  }
  // too wide: text columns give way first (they ellipsize), then everything shrinks alike
  let over = widths.reduce((a, x) => a + x, 0) - b.w;
  // (a wrapping column keeps the width its lines were made for)
  const soft = cols.map((_, k) => k).filter((k) => !whole(k) && !cols[k].s.wrap);
  const give = soft.reduce((a, k) => a + Math.max(0, widths[k] - mins[k]), 0);
  if (over > 0.5 && give > 0) {
    const cut = Math.min(over, give);
    soft.forEach((k) => { widths[k] -= (cut * Math.max(0, widths[k] - mins[k])) / give; });
    over -= cut;
  }
  const scale = widths.reduce((a, x) => a + x, 0) / b.w;
  if (scale > 1.001) {
    // a wrapping column keeps its width (its lines were made for it): the others give way first
    const wrapK = cols.map((c, k) => (c.s.wrap ? k : -1)).filter((k) => k >= 0);
    const wrapSum = wrapK.reduce((a, k) => a + widths[k], 0);
    const rest = widths.reduce((a, x, k) => a + (wrapK.includes(k) ? 0 : x), 0);
    const f2 = rest > 0 ? Math.max(0, b.w - wrapSum) / rest : 0;
    widths = wrapK.length && wrapSum < b.w * 0.9 && f2 > 0.3 ? widths.map((x, k) => (wrapK.includes(k) ? x : x * f2)) : widths.map((x) => x / scale);
  }

  // final text fitting (a group label may run on over the empty cells after it)
  for (const row of body) row.cells.forEach((cl, k) => {
    // wrapped text keeps its icon's lane, as on one line
    if (cl.lines) { if (cl.icon) cl.iconW = iconWOf(cl.icon); return; }
    if (row.group) {
      let room = widths[k];
      for (let q = k + 1; q < row.cells.length && !row.cells[q].text; q++) room += widths[q];
      cl.text = ellipsize(cl.text, room - pad * 2 - (cl.indent || 0), size, cl.weight, cl.family, cl.italic, cl.tracking || 0);
      return;
    }
    if (cl.icon) cl.iconW = iconWOf(cl.icon);
    cl.text = ellipsize(cl.text, widths[k] - pad * 2 - (cl.pill ? pillPad : 0) - (cl.icon ? iconWOf(cl.icon) : 0) - (cl.indent || 0) - (cl.tag ? tagW(cl.tag) : 0) - (cl.marks ? size * (0.4 + 1.6 * cl.marks.length) : 0), size, cl.weight, cl.family, cl.italic, cl.tracking || 0);
  });

  // header groups: adjacent columns sharing a group label merge into one spanning cell
  let groupRow = null;
  if (t.group_h > 0) {
    groupRow = [];
    cols.forEach((c, k) => {
      const g = c.s.group || c.group || '';
      const last = groupRow[groupRow.length - 1];
      if (g && last && last.label === g && last.to === k - 1) last.to = k;
      else groupRow.push({ label: g, from: k, to: k });
    });
  }

  const head = {
    h: t.head_h, head: true, fill: hd.fill || null,
    cells: cols.map((c, k) => ({ text: String(headText[k]).split('\n').map((l) => ellipsize(l, widths[k] - pad * 2, hf.size, hf.weight, hf.family, hf.italic, hf.tracking || 0)).join('\n'), bold: hf.weight >= 600, weight: hf.weight, size: hf.size, family: hf.family, italic: hf.italic, tracking: hf.tracking || 0, color: c.s.headColor || hd.color || (hd.fill ? inkOn(hd.fill) : hf.color), align: c.s.wrap ? 'left' : alignOf(c) })),
  };
  const out = {
    x: b.x, y: b.y + (t.group_h || 0), w: b.w, size, family: cf.family, italic: cf.italic, pad, cols: widths, keys: cols.map((c) => c.key), mins, lines: ts.lines || 'rows', zebraFill: ts.zebraFill || '#f4f3ef',
    groupRow, groupH: t.group_h || 0, groupY: b.y, headFont: hf, headCaps: caps, headColor: hd.color || hf.color,
    rows: ts.hideHead ? body : [head, ...body],
    bands: cols.map((c) => c.s.band || null), colRules: cols.map((c) => (c.s.rule ? { side: c.s.rule, color: c.s.ruleColor || th.ink, w: c.s.ruleWidth || 1.4 } : null)),
    lineColor: ts.lineColor || th.grid, headLine: ts.headLineColor || '#c3c2b7', headLineW: ts.headLineWidth ?? 0.8, totalLine: ts.totalLine || 'single',
  };
  if (clipped && t.visible > 0) {
    out.rows.push({ h: t.row_h, more: true, cells: cols.map((c, k) => ({ text: k === 0 ? `… ${allRows.length - off - shown.length} more rows` : '', color: hf.color, align: 'left', family: cf.family, italic: true, weight: cf.weight })) });
  }
  if (t.totals) {
    const cells = cols.map((c, k) => {
      const v = totalValue(c);
      let text = totalText[k];
      let color = ts.totalColor || cf.color;
      if (typeof v === 'number' && v !== 0 && (c.s.signColor || c.s.arrows)) color = v > 0 ? th.good : th.bad;
      if (typeof v === 'number' && v !== 0 && c.s.arrows) text = `${v > 0 ? '▲' : '▼'} ${text.replace(/^[−-]/, '')}`;
      const cl = { text: ellipsize(text, widths[k] - pad * 2, size, 600, fam(c), ital(c)), bold: true, weight: Math.max(600, cf.weight), color, align: alignOf(c), family: fam(c), italic: ital(c) };
      if (Array.isArray(tr[c.j]) && (c.s.total || 'auto') === 'auto') { cl.text = ''; cl.spark = { values: tr[c.j], color: c.s.color || th.palette[0] }; }
      return cl;
    });
    const firstText = cols.findIndex((c, k) => !cells[k].text && !cells[k].spark);
    const at = firstText >= 0 ? firstText : 0;
    cells[at].text = (ts.totalLabel || TOTAL[getLocale().slice(0, 2)] || 'Total').replace('{n}', String(leafCount));
    cells[at].align = 'left';
    cells[at].spark = null;
    if (ts.totalFont) {
      const tf = ts.totalFont;
      Object.assign(cells[at], { family: tf.family || cells[at].family, size: tf.size || undefined, weight: tf.weight || cells[at].weight, italic: tf.italic ?? cells[at].italic });
    }
    out.rows.push({ h: t.row_h, total: true, fill: ts.totalFill || null, cells });
  }
  return out;
}

function iconSvg(ic, x, cy, size, maxH = Infinity) {
  // an icon never grows past its row (minus a little air above and below)
  if (ic.type === 'glyph') {
    const s2 = Math.min(size * 1.9 * (ic.scale || 1), maxH);
    return glyph(ic.name, x, cy - s2 / 2, s2, { shape: ic.shape, fill: ic.fill, color: ic.color, stroke: ic.stroke, strokeWidth: ic.strokeWidth, brandFill: ic.fill && ic.fill !== '#e9dfcc' && ic.fill !== 'transparent' ? ic.fill : null });
  }
  if (ic.type === 'swatch') {
    if (ic.shape === 'tile') {
      const s2 = Math.min(size * 2.05, maxH);
      return `<rect x="${r2(x)}" y="${r2(cy - s2 / 2)}" width="${r2(s2)}" height="${r2(s2)}" rx="${r2(s2 * 0.24)}" fill="${ic.color}"/>`;
    }
    const s2 = size * 0.78;
    return ic.shape === 'dot'
      ? `<circle cx="${r2(x + s2 / 2)}" cy="${r2(cy)}" r="${r2(s2 / 2)}" fill="${ic.color}"/>`
      : `<rect x="${r2(x)}" y="${r2(cy - s2 / 2)}" width="${r2(s2)}" height="${r2(s2)}" rx="${r2(s2 * 0.22)}" fill="${ic.color}"/>`;
  }
  const r = size * 0.4;
  const cx = x + r;
  if (ic.type === 'arrows') {
    const s = r * 1.05;
    const d = ic.level === 2 ? `M${cx - s},${cy + s * 0.7}L${cx},${cy - s * 0.8}L${cx + s},${cy + s * 0.7}Z`
      : ic.level === 0 ? `M${cx - s},${cy - s * 0.7}L${cx},${cy + s * 0.8}L${cx + s},${cy - s * 0.7}Z`
      : `M${cx - s * 0.8},${cy - s}L${cx + s},${cy}L${cx - s * 0.8},${cy + s}Z`;
    return `<path d="${d}" fill="${ic.color}"/>`;
  }
  if (ic.type === 'checks') {
    const sw = Math.max(1, size * 0.14);
    const p = ic.level === 2 ? `M${cx - r * 0.75},${cy}L${cx - r * 0.15},${cy + r * 0.6}L${cx + r * 0.8},${cy - r * 0.65}`
      : ic.level === 0 ? `M${cx - r * 0.65},${cy - r * 0.65}L${cx + r * 0.65},${cy + r * 0.65}M${cx + r * 0.65},${cy - r * 0.65}L${cx - r * 0.65},${cy + r * 0.65}`
      : `M${cx},${cy - r * 0.8}L${cx},${cy + r * 0.25}M${cx},${cy + r * 0.62}L${cx},${cy + r * 0.7}`;
    return `<path d="${p}" fill="none" stroke="${ic.color}" stroke-width="${sw}" stroke-linecap="round" stroke-linejoin="round"/>`;
  }
  return `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${ic.color}"/>`;
}

function sparkSvg(sp, x, y, w, h) {
  const vals = sp.values;
  const finite = vals.filter((v) => typeof v === 'number');
  if (finite.length < 2) return '';
  const mn = Math.min(...finite), mx = Math.max(...finite);
  const n = vals.length;
  const pts = vals.map((v, i) => (typeof v === 'number' ? [x + (w * i) / (n - 1), y + h - ((v - mn) / (mx - mn || 1)) * h] : null)).filter(Boolean);
  const d = pts.map((p, i) => `${i ? 'L' : 'M'}${r2(p[0])},${r2(p[1])}`).join('');
  const last = pts[pts.length - 1];
  return `<path d="${d}" fill="none" stroke="${sp.color}" stroke-width="1.1" stroke-linejoin="round" stroke-linecap="round"/><circle cx="${r2(last[0])}" cy="${r2(last[1])}" r="1.6" fill="${sp.color}"/>`;
}

/**
 * A table laid out as facts: each row becomes a cell of a grid — its name
 * small and upper-case on top, its value large below — `ts.factCols` cells
 * across (1–4), with hairlines between them. For key → value tables such as
 * a policy's terms or a portfolio's "about".
 */
export function factsSvg(st, w, vm, data, b) {
  const th = st.doc.theme;
  const ts = w.opts?.table || {};
  const cols = displayColumns(w, data).filter((c) => !c.s.hidden && c.j >= 0);
  const key = cols.find((c) => c.role === 'dim') || cols[0];
  const vals = cols.filter((c) => c !== key);
  const rows = data?.rows || [];
  if (!key || !rows.length) return '';
  const k = Math.max(1, Math.min(4, Math.round(ts.factCols || 2)));
  const lines = Math.ceil(rows.length / k);
  const cf = vm.fonts?.table_cell || {};
  const size = cf.size || 8, fam = cf.family;
  const cw = b.w / k;
  const ch = Math.min(b.h / lines, size * 4.2);
  const pad = Math.min(10, cw * 0.06);
  const grid = th.grid || mixHex(th.ink, th.page_bg, 0.85);
  const T = (x, y, text, o) => `<text x="${r2(x)}" y="${r2(y)}" font-family="${esc(fontStack(o.family || fam))}" font-size="${r2(o.size)}" font-weight="${o.weight}" fill="${o.fill}"${o.tracking ? ` letter-spacing="${r2(o.tracking * o.size)}"` : ''}>${esc(text)}</text>`;
  let out = '';
  rows.forEach((r, i) => {
    const row = Math.floor(i / k), col = i % k;
    const x = b.x + col * cw, y = b.y + row * ch;
    // the first value that says something (text, or a number in its column's format)
    let text = '', color = th.ink;
    for (const c of vals) {
      const v = r[c.j];
      const t = formatCell(v, c, c.s, th.currency);
      if (t === '' || t == null) continue;
      text = t;
      if (c.s.color) color = c.s.color;
      if (c.s.signColor && typeof v === 'number' && v !== 0) color = v > 0 ? th.good : th.bad;
      break;
    }
    const ls = size * 0.72, vs = size * (ts.factSize || 1.18);
    const top = y + (ch - ls - vs * 1.2) / 2 + ls;
    out += T(x + pad, top, ellipsize(String(r[key.j] ?? '').toUpperCase(), cw - pad * 2, ls, 600, fam, false, 0.06), { size: ls, weight: 600, fill: th.muted, tracking: 0.06 });
    out += T(x + pad, top + vs * 1.35, ellipsize(text || '—', cw - pad * 2, vs, 700, fam), { size: vs, weight: 700, fill: color });
    if (col > 0) out += `<line x1="${r2(x)}" x2="${r2(x)}" y1="${r2(y + ch * 0.18)}" y2="${r2(y + ch * 0.82)}" stroke="${grid}" stroke-width="0.6"/>`;
    if (row < lines - 1 && col === 0) out += `<line x1="${r2(b.x)}" x2="${r2(b.x + b.w)}" y1="${r2(y + ch)}" y2="${r2(y + ch)}" stroke="${grid}" stroke-width="0.6"/>`;
  });
  return out;
}

export function tableSvg(st, tb) {
  const th = st.doc.theme;
  const T = (x, y, s, o) => `<text x="${r2(x)}" y="${r2(y)}" font-family="${esc(fontStack(o.family || tb.family))}" font-size="${o.size || tb.size}" font-weight="${o.weight}"${o.italic ? ' font-style="italic"' : ''} fill="${o.fill}"${o.anchor !== 'start' ? ` text-anchor="${o.anchor}"` : ''}${o.tracking ? ` letter-spacing="${r2(o.tracking * (o.size || tb.size))}"` : ''}>${esc(s)}</text>`;
  const line = (x1, y1, x2, y2, c, w) => `<line x1="${r2(x1)}" x2="${r2(x2)}" y1="${r2(y1)}" y2="${r2(y2)}" stroke="${c}" stroke-width="${w}"/>`;
  let out = '';
  const pad = tb.pad ?? 5;
  const colX = (k) => tb.x + tb.cols.slice(0, k).reduce((a, x) => a + x, 0);
  if (tb.groupRow) {
    const hf = tb.headFont;
    for (const g of tb.groupRow) {
      if (!g.label) continue;
      const x0 = colX(g.from) + 3, x1 = colX(g.to + 1) - 3;
      const label = ellipsize((tb.headCaps ?? hf.upper) ? g.label.toUpperCase() : g.label, x1 - x0, hf.size, hf.weight, hf.family, hf.italic);
      out += T((x0 + x1) / 2, tb.groupY + tb.groupH * 0.55, label, { family: hf.family, italic: hf.italic, size: hf.size, weight: hf.weight, fill: tb.headColor, anchor: 'middle' });
      out += line(x0, tb.groupY + tb.groupH - 2, x1, tb.groupY + tb.groupH - 2, '#c3c2b7', 0.8);
    }
  }
  let y = tb.y;
  const bottom = tb.y + tb.rows.reduce((a, r) => a + r.h, 0);
  // in a right- or centre-aligned column the icons line up: they sit before the widest text, not each its own
  const iconLane = new Map();
  const laneOf = (j) => {
    if (!iconLane.has(j)) {
      let m = 0;
      for (const r of tb.rows) {
        const c = r.cells[j];
        if (c?.icon && c.text && !r.total) m = Math.max(m, textWidth(c.text, c.size || tb.size, c.weight || (c.bold ? 600 : 400), c.family || tb.family, c.italic));
      }
      iconLane.set(j, m);
    }
    return iconLane.get(j);
  };
  // column bands behind everything
  (tb.bands || []).forEach((bf, j) => { if (bf) out += `<rect x="${r2(colX(j))}" y="${r2(tb.y)}" width="${r2(tb.cols[j])}" height="${r2(bottom - tb.y)}" fill="${bf}"/>`; });
  for (const row of tb.rows) {
    if (row.fill) out += `<rect x="${r2(tb.x)}" y="${r2(y)}" width="${r2(tb.w)}" height="${r2(row.h)}" fill="${row.fill}" rx="${row.head ? 2 : 0}"/>`;
    else if (row.zebra) out += `<rect x="${r2(tb.x)}" y="${r2(y)}" width="${r2(tb.w)}" height="${r2(row.h)}" fill="${tb.zebraFill}"/>`;
    if (row.total && tb.totalLine !== 'none') {
      out += line(tb.x, y, tb.x + tb.w, y, tb.headLine, tb.totalLine === 'thick' ? 1.6 : 0.8);
      if (tb.totalLine === 'double') out += line(tb.x, y + row.h, tb.x + tb.w, y + row.h, tb.headLine, 0.8) + line(tb.x, y + row.h + 2, tb.x + tb.w, y + row.h + 2, tb.headLine, 0.8);
    }
    if (row.accent) out += `<rect x="${r2(tb.x)}" y="${r2(y + 0.5)}" width="2.5" height="${r2(row.h - 1)}" fill="${row.accent}"/>`;
    if (row.pill) out += `<rect x="${r2(tb.x)}" y="${r2(y + 1.5)}" width="${r2(tb.w)}" height="${r2(row.h - 3)}" rx="${r2((row.h - 3) / 2)}" fill="${row.pill.fill}"${row.pill.stroke ? ` stroke="${row.pill.stroke}" stroke-width="0.8"` : ''}/>`;
    let x = tb.x;
    row.cells.forEach((c, j) => {
      const o0 = out.length;
      const w = tb.cols[j];
      const weight = c.weight || (c.bold ? 600 : 400);
      const cs = c.size || tb.size;
      const fo = { family: c.family, italic: c.italic, size: cs, tracking: c.tracking || 0 };
      const below = c.bar?.below;
      const baseline = c.sub ? y + row.h / 2 - cs * 0.12 : below ? y + row.h / 2 + cs * 0.1 : y + row.h / 2 + cs * 0.36;
      if (c.fill) out += `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(w)}" height="${r2(row.h)}" fill="${c.fill}"/>`;
      if (c.sub) {
        const sx = c.align === 'right' ? x + w - pad : x + pad + (c.indent || 0) + (c.icon && c.align !== 'center' ? c.iconW || cs * 1.15 : 0);
        const sw = c.subBold ? 700 : c.subWeight || 400, ss = c.subBold ? cs * 0.72 : cs * 0.82;
        out += T(sx, y + row.h / 2 + cs * 1.05, ellipsize(c.subBold ? c.sub.toUpperCase() : c.sub, w - pad * 2, ss, sw, c.family || tb.family, c.italic), { family: c.family, italic: c.italic, size: r2(ss), weight: sw, fill: c.subColor || th.muted, anchor: c.align === 'right' ? 'end' : 'start', tracking: c.subBold ? 0.06 : 0 });
      }
      if (c.spark) out += sparkSvg(c.spark, x + pad, y + row.h * 0.22, w - pad * 2 - 3, row.h * 0.56);
      if (c.bar && below) {
        // a thin bar under the number
        const track = w - pad * 2, bh = Math.max(1.2, cs * 0.28 * (c.bar.k || 1)), by = y + row.h / 2 + cs * 0.55;
        const bx = x + pad;
        if (c.bar.track !== false) out += `<rect x="${r2(bx)}" y="${r2(by)}" width="${r2(track)}" height="${r2(bh)}" rx="${r2(bh / 2)}" fill="${c.bar.color}" fill-opacity="0.13"/>`;
        out += `<rect x="${r2(c.align === 'right' ? bx : bx)}" y="${r2(by)}" width="${r2(Math.max(bh, track * Math.min(1, c.bar.frac)))}" height="${r2(bh)}" rx="${r2(bh / 2)}" fill="${c.bar.color}"${c.bar.stroke ? ` stroke="${c.bar.stroke}" stroke-width="${c.bar.strokeWidth}"` : ''}/>`;
      } else if (c.bar) {
        // the bar sits beside the number, sharing the cell
        const textW = textWidth(c.text, cs, weight, c.family || tb.family, c.italic);
        const track = c.text ? Math.max(0, w - pad * 2 - textW - 6) : Math.max(0, w - pad * 2);
        const bw = track * Math.min(1, c.bar.frac);
        const bh = c.bar.h || Math.min(row.h - 6, cs * 1.2 * (c.bar.k || 1));
        const bx = c.align === 'right' ? x + pad : x + w - pad - track;
        const rx = c.bar.round ? bh / 2 : Math.min(2, bh / 2);
        const by = y + (row.h - bh) / 2;
        if (!c.text && c.bar.track !== false) {
          out += c.bar.trackFill ? `<rect x="${r2(bx)}" y="${r2(by)}" width="${r2(track)}" height="${r2(bh)}" rx="${r2(rx)}" fill="${c.bar.trackFill}"/>`
            : `<rect x="${r2(bx)}" y="${r2(by)}" width="${r2(track)}" height="${r2(bh)}" rx="${r2(rx)}" fill="${c.bar.color}" fill-opacity="0.14"/>`;
          for (let q = 1; q < (c.bar.ticks || 0); q++) out += `<line x1="${r2(bx + (track * q) / c.bar.ticks)}" x2="${r2(bx + (track * q) / c.bar.ticks)}" y1="${r2(by + 1.5)}" y2="${r2(by + bh - 1.5)}" stroke="${th.ink}" stroke-opacity="0.12" stroke-width="0.6"/>`;
        }
        const line = c.bar.stroke ? ` stroke="${c.bar.stroke}" stroke-width="${c.bar.strokeWidth}"` : '';
        if (c.bar.zero != null) {
          const za = bx + track * c.bar.zero, zb = bx + track * c.bar.to;
          out += `<rect x="${r2(Math.min(za, zb))}" y="${r2(by)}" width="${r2(Math.abs(zb - za))}" height="${r2(bh)}" rx="${r2(rx)}" fill="${c.bar.color}" fill-opacity="${c.bar.opacity}"${line}/>`;
        } else out += `<rect x="${r2(bx)}" y="${r2(by)}" width="${r2(bw)}" height="${r2(bh)}" rx="${r2(rx)}" fill="${c.bar.color}" fill-opacity="${c.bar.opacity}"${line}/>`;
      }
      if (c.badge && c.text) {
        // the rank in a coloured dot
        const r = Math.min(row.h * 0.3, cs * 1.05), cx = c.align === 'right' ? x + w - pad - r : c.align === 'center' ? x + w / 2 : x + pad + r;
        out += `<circle cx="${r2(cx)}" cy="${r2(y + row.h / 2)}" r="${r2(r)}" fill="${c.badge}"/>`;
        out += T(cx, y + row.h / 2 + cs * 0.36, c.text, { ...fo, weight: 700, fill: '#ffffff', anchor: 'middle' });
      } else if (c.lines) {
        // wrapped text: its icon first (as on one line), the lines beside it
        let tx0 = x + pad + (c.indent || 0);
        if (c.icon) {
          const iw = c.iconW || (c.icon.type === 'swatch' ? (c.icon.shape === 'tile' ? cs * 2.6 : cs * 1.25) : cs * 1.15);
          out += iconSvg(c.icon, tx0, y + row.h / 2, cs, Math.max(8, row.h - 4));
          tx0 += iw;
        }
        // the lines again at the column's width as drawn (it may have given way to the others), within the
        // lines the row was made tall for; whole words move down, the last line ends in "…" if need be
        const room = Math.max(10, x + w - pad - tx0);
        const fam = c.family || tb.family, it = c.italic;
        const fits = (t) => textWidth(t, cs, weight, fam, it) <= room;
        const lines = [];
        let curL = '';
        for (const word of c.lines.join(' ').split(/\s+/).filter(Boolean)) {
          const next = curL ? `${curL} ${word}` : word;
          if (fits(next) || !curL) curL = next;
          else { lines.push(curL); curL = word; }
        }
        if (curL) lines.push(curL);
        const max = Math.max(1, c.lines.length);
        const shown = lines.slice(0, max).map((l, q) => (q === max - 1 && (lines.length > max || !fits(l)) ? ellipsize(lines.slice(q).join(' '), room, cs, weight, fam, it) : fits(l) ? l : ellipsize(l, room, cs, weight, fam, it)));
        const lh = cs * 1.3;
        let ly = y + (row.h - shown.length * lh) / 2 + cs * 0.95;
        for (const l of shown) { out += T(tx0, ly, l, { ...fo, weight, fill: c.color || th.ink, anchor: 'start' }); ly += lh; }
      } else if (row.head && c.text.includes('\n')) {
        // two-line header: the second line smaller and lighter
        const [l1, l2] = c.text.split('\n');
        const tx = c.align === 'right' ? x + w - pad : c.align === 'center' ? x + w / 2 : x + pad;
        const anchor = c.align === 'right' ? 'end' : c.align === 'center' ? 'middle' : 'start';
        out += T(tx, y + row.h / 2 - cs * 0.2, l1, { ...fo, weight, fill: c.color || th.ink, anchor });
        out += T(tx, y + row.h / 2 + cs * 1.0, l2, { ...fo, size: cs * 0.92, weight: 400, fill: c.color || th.muted, anchor });
      } else if (c.iconOnly && c.icon) {
        // the marker alone, placed as the column aligns
        const vis = c.icon.type === 'glyph' ? Math.min(glyphWidth(c.icon.name, cs * 1.9 * (c.icon.scale || 1)), glyphWidth(c.icon.name, Math.max(8, row.h - 4))) : cs * 0.78;
        const ix = c.align === 'right' ? x + w - pad - vis : c.align === 'center' ? x + (w - vis) / 2 : x + pad + (c.indent || 0);
        out += iconSvg(c.icon, ix, y + row.h / 2, cs, Math.max(8, row.h - 4));
      } else if (c.text) {
        let tx0 = x + pad + (c.indent || 0);
        if (c.icon) {
          const tw = textWidth(c.text, cs, weight, c.family || tb.family, c.italic);
          const iw = c.iconW || (c.icon.type === 'swatch' ? (c.icon.shape === 'tile' ? cs * 2.6 : cs * 1.25) : cs * 1.15);
          // with a pill, the badge and the pill go together (the pill is the text's width and a little more)
          const lane = c.pill ? tw + 12 : Math.max(tw, laneOf(j));
          const ix = c.align === 'right' ? x + w - pad - lane - iw : c.align === 'center' ? x + (w - lane - iw) / 2 : tx0;
          out += iconSvg(c.icon, ix, y + row.h / 2, cs, Math.max(8, row.h - 4));
          if (c.align === 'left') tx0 += iw;
          if (c.align === 'center') tx0 = ix + iw;
        }
        if (c.pill) {
          const pw = textWidth(c.text, cs, weight, c.family || tb.family, c.italic) + 12;
          const ph = Math.min(row.h - 3, cs * 1.7);
          // after its marker, when it has one (centred: the two together)
          const px = c.align === 'right' ? x + w - pad - pw : c.align === 'center' && !c.icon ? x + (w - pw) / 2 : tx0;
          out += `<rect x="${r2(px)}" y="${r2(y + (row.h - ph) / 2)}" width="${r2(pw)}" height="${r2(ph)}" rx="${r2(ph / 2)}" fill="${c.pill.fill}"/>`;
          out += T(px + pw / 2, baseline, c.text, { ...fo, weight: c.bold ? 600 : Math.max(500, weight), fill: c.pill.color, anchor: 'middle' });
        } else {
          const tx = c.align === 'right' ? x + w - pad : c.align === 'center' && !c.icon ? x + w / 2 : tx0;
          const anchor = c.align === 'right' ? 'end' : c.align === 'center' && !c.icon ? 'middle' : 'start';
          out += T(tx, baseline, c.text, { ...fo, weight, fill: c.color || th.ink, anchor });
          if (c.marks && anchor === 'start') {
            // one chip per ranking that hit the row, side by side
            const ms = cs * 1.45, twm = textWidth(c.text, cs, weight, c.family || tb.family, c.italic, c.tracking || 0);
            c.marks.forEach((mk, q) => { out += glyph(mk.glyph, tx + twm + cs * 0.5 + q * (ms + cs * 0.15), baseline - cs * 0.36 - ms / 2, ms, { shape: 'circle', fill: mk.fill, color: mk.color }); });
          }
          if (c.tag) {
            const tsz = cs * 0.74, tt = c.tag.text.toUpperCase();
            const tw2 = textWidth(tt, tsz, 700, c.family || tb.family, false, 0.04) + 12;
            const th2 = tsz * 2;
            const twText = textWidth(c.text, cs, weight, c.family || tb.family, c.italic, c.tracking || 0);
            const gx = anchor === 'end' ? tx - twText - 6 - tw2 : (anchor === 'middle' ? tx + twText / 2 : tx + twText) + 6;
            const gy = baseline - cs * 0.36 - th2 / 2;
            out += `<rect x="${r2(gx)}" y="${r2(gy)}" width="${r2(tw2)}" height="${r2(th2)}" rx="${r2(th2 * 0.3)}" fill="${c.tag.fill}"/>`;
            out += T(gx + tw2 / 2, gy + th2 / 2 + tsz * 0.36, tt, { family: c.family, size: tsz, weight: 700, fill: c.tag.color, anchor: 'middle', tracking: 0.04 });
          }
        }
      }
      // each cell is a part of its column: a click on it opens the column's settings
      if (tb.keys?.[j] != null && out.length > o0) out = out.slice(0, o0) + `<g data-part="col:${esc(String(tb.keys[j]))}">${out.slice(o0)}</g>`;
      x += w;
    });
    y += row.h;
    const sep = row.head ? tb.lines !== 'none' : tb.lines === 'rows' || tb.lines === 'grid';
    if (row.groupLine) out += line(tb.x, y, tb.x + tb.w, y, row.groupLine === true ? tb.headLine : row.groupLine, 1.2);
    else if (sep && !row.total) out += line(tb.x, y, tb.x + tb.w, y, row.head ? tb.headLine : tb.lineColor, row.head ? tb.headLineW : 0.5);
  }
  // column rules (e.g. between actual and forecast quarters)
  (tb.colRules || []).forEach((cr, j) => {
    if (!cr) return;
    const x = cr.side === 'right' ? colX(j + 1) : colX(j);
    const y0 = tb.y + (tb.rows[0]?.h || 0);
    out += `<rect x="${r2(x - cr.w / 2)}" y="${r2(y0)}" width="${r2(cr.w)}" height="${r2(bottom - y0 - (tb.rows[tb.rows.length - 1]?.total ? tb.rows[tb.rows.length - 1].h : 0))}" fill="${cr.color}"/>`;
  });
  if (tb.lines === 'grid') {
    let x = tb.x;
    for (let j = 0; j < tb.cols.length - 1; j++) { x += tb.cols[j]; out += line(x, tb.y, x, bottom, tb.lineColor, 0.5); }
  }
  return out;
}

/** Native PowerPoint table rows. */
export function pptxRows(tb) {
  return tb.rows.map((r) => ({
    h: r.h,
    cells: r.cells.map((c) => ({
      text: c.lines ? c.lines.join(' ') : `${' '.repeat(Math.round((c.indent || 0) / 3))}${c.text}`,
      bold: (c.weight || (c.bold ? 600 : 400)) >= 600,
      font: c.family || tb.family,
      italic: !!c.italic,
      size: c.size || tb.size,
      color: c.pill ? c.pill.color : c.color,
      fill: c.pill ? c.pill.fill : c.fill || r.fill || (r.zebra ? tb.zebraFill : null),
      align: c.align,
      border: r.head ? tb.headLine : tb.lines === 'none' || r.total ? null : tb.lineColor,
    })),
  }));
}
