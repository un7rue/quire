// The report renderer. Every widget is drawn once into "parts" (card, text
// blocks, table, vector graphic). Parts become one SVG per page for the screen
// and the PDF, and native shapes / text boxes / tables / SVG pictures for
// PowerPoint, so all three outputs match.

import { esc, ellipsize, textWidth, fontStack, setLocale, inkOn, mixHex, tokenColor } from './util.js';
import { tablePart, tableSvg, pptxRows, usesPicture, factsSvg } from './table.js';
import { listTable } from './rowlist.js';
import * as C from './charts.js';
import { glyph, setCustomIcons } from './glyphs.js';
import { needsPieces, piecesSvg, TEXT_ICONS, iconColor, toneColor } from './richtext.js';

const FONT = 'Inter';
// the theme of the widget being drawn (icon colours in text)
let TH = null;
/** Title icons against their set size (smaller everywhere; Rust leaves room for them by the same factor). */
const TITLE_ICON = 0.85;
/** A note's own colour (its bar, its avatar): the theme's dark ink, calm whatever the accent. */
export const noteInk = (th) => (th.tokens?.ink2 ? tokenColor(th, '$ink2') : th.ink);

// the chart font of the widget being drawn (its "chart" role)
export let CUR = { family: 'Inter', italic: false, size: 7, weight: 400, color: '#6b6a65' };
export const CS = (d) => Math.max(4, r2(CUR.size + d));
export const tw = (s, size, weight = 400) => textWidth(s, size, weight, CUR.family, CUR.italic);
/** The chart text of the widget being drawn. */
export const curFont = () => CUR;
// a chart's own parts' text roles for the widget being drawn: the resolved font and the settings
// the designer gave it (theme, design view or widget) — those replace how the drawing sets them
let PARTF = {};
export const PF = () => PARTF;
export const ell = (s, max, size, weight = 400) => ellipsize(s, max, size, weight, CUR.family, CUR.italic);

// `rotate`: degrees, turning up from the baseline about (x, y) — e.g. category labels at 45°
const textAttrs = (x, y, { size = 8, weight = 400, fill = '#000', anchor = 'start', opacity, family = CUR.family, italic = CUR.italic, tracking = 0, rotate = 0 } = {}) =>
  `x="${r2(x)}" y="${r2(y)}" font-family="${esc(fontStack(family))}" font-size="${size}" font-weight="${weight}"${italic ? ' font-style="italic"' : ''} fill="${fill}"${anchor !== 'start' ? ` text-anchor="${anchor}"` : ''}${opacity ? ` fill-opacity="${opacity}"` : ''}${tracking ? ` letter-spacing="${r2(tracking * size)}"` : ''}${rotate ? ` transform="rotate(${-rotate} ${r2(x)} ${r2(y)})"` : ''}`;

export const T = (x, y, s, o = {}) => `<text ${textAttrs(x, y, o)}>${esc(s)}</text>`;

/** Text made of styled pieces ({text, bold, italic, color}). */
export const TR = (x, y, runs, o = {}) => {
  const bw = (o.weight || 400) >= 600 ? 800 : 700;
  return `<text ${textAttrs(x, y, o)}>${runs.map((r) => `<tspan${r.bold ? ` font-weight="${bw}"` : ''}${r.italic ? ` font-style="${o.italic ? 'normal' : 'italic'}"` : ''}${r.color ? ` fill="${r.color}"` : ''}>${esc(r.text)}</tspan>`).join('')}</text>`;
};

export const r2 = (v) => Math.round(v * 100) / 100;

/** A colour, with `$name` references to the theme resolved (as Rust does). */
export function themeColor(th, c) {
  return tokenColor(th, c);
}

function theme(st) {
  return st.doc.theme;
}

export function seriesColor(st, w, i, name) {
  const pal = theme(st).palette;
  if (i === 0 && w.style?.color && !name) return w.style.color;
  const off = w.style?.paletteOffset || 0;
  return pal[(i + off) % pal.length] || '#888';
}

// ---------------------------------------------------------------- data shaping

/** Normalises a query result into categories × series. */
export function shape(data) {
  const cols = data?.columns || [];
  const dims = cols.filter((c) => c.role === 'dim');
  const meas = cols.filter((c) => c.role === 'measure');
  const rows = data?.rows || [];
  if (dims.length >= 2) {
    const cats = [];
    const names = [];
    const vals = new Map();
    for (const r of rows) {
      const c = String(r[0]), s = String(r[1]);
      if (!cats.includes(c)) cats.push(c);
      if (!names.includes(s)) names.push(s);
      vals.set(c + '\u0000' + s, r[dims.length]);
    }
    // colour follows the entity, so order series by name, not rank
    names.sort((a, b) => a.localeCompare(b));
    const keyOf = new Map(Array.isArray(data?.keys) ? rows.map((r, i) => [String(r[0]), data.keys[i]]) : []);
    return {
      cats,
      keys: keyOf.size ? cats.map((c) => keyOf.get(c) || '') : null,
      series: names.map((n) => ({ name: n, values: cats.map((c) => vals.get(c + '\u0000' + n) ?? null), format: meas[0]?.format })),
      named: true,
    };
  }
  if (dims.length === 1) {
    return {
      cats: rows.map((r) => String(r[0])),
      // a date category's sortable key ("2026-03", "2026-Q1"), for a chart's own date format
      keys: Array.isArray(data?.keys) ? data.keys : null,
      series: meas.map((m, j) => ({ name: m.label, values: rows.map((r) => r[1 + j]), format: m.format })),
      named: false,
    };
  }
  return { cats: ['Total'], series: meas.map((m, j) => ({ name: m.label, values: [data?.total?.[j] ?? null], format: m.format })), named: false };
}

// ---------------------------------------------------------------- shared chart pieces

export function message(st, b, text) {
  const size = CS(1);
  const words = String(text).split(/\s+/);
  const lines = [];
  let cur = '';
  for (const wd of words) {
    const c = cur ? cur + ' ' + wd : wd;
    if (tw(c, size) > b.w - 8 && cur) { lines.push(cur); cur = wd; } else cur = c;
  }
  if (cur) lines.push(cur);
  const lh = 11;
  const y0 = b.y + b.h / 2 - (lines.length * lh) / 2 + 8;
  return lines.slice(0, Math.max(1, Math.floor(b.h / lh))).map((l, i) => T(b.x + b.w / 2, y0 + i * lh, l, { size, fill: CUR.color, anchor: 'middle' })).join('');
}


// ---------------------------------------------------------------- table of contents

const ROMAN = ['I', 'II', 'III', 'IV', 'V', 'VI', 'VII', 'VIII', 'IX', 'X'];

/** A section's number in the chosen style ('' for none). */
function secNum(style, i) {
  if (style === 'roman') return ROMAN[i] || String(i + 1);
  if (style === 'arabic') return String(i + 1);
  if (style === 'alpha') return String.fromCharCode(65 + (i % 26));
  return '';
}

/** Contents settings, with each layout's defaults filled in. */
export function tocOpts(o = {}) {
  const chapters = o.style === 'sections';
  return {
    style: chapters ? 'sections' : 'list',
    show: o.show || 'all',
    cols: Math.max(1, Math.min(3, +o.cols || (chapters ? 2 : 1))),
    numbering: o.numbering || (chapters ? 'roman' : 'none'),
    label: o.label ?? (chapters ? 'Section' : ''),
    descriptions: o.descriptions ?? chapters,
    leader: o.leader || 'dots',
    pageNums: o.pageNums !== false,
    rules: o.rules ?? chapters,
    spacing: Math.max(0.6, Math.min(2, +o.spacing || 1)),
    iconShape: o.iconShape || 'circle',
    // the section icons' size against the heading (1 = the original size; 0.5 by default)
    iconSize: Math.max(0.4, Math.min(1.6, +o.iconSize || 0.5)),
    iconFill: o.iconFill || null, iconColor: o.iconColor || null, iconStroke: o.iconStroke || null, iconStrokeWidth: o.iconStrokeWidth || null,
    icon: o.icon || 'doc',
    icons: o.icons || {},
  };
}

/** The line between an entry and its page number. */
function leader(kind, x0, x1, y, color) {
  if (kind === 'none' || x1 <= x0) return '';
  const dots = kind === 'dots';
  return `<line x1="${r2(x0)}" x2="${r2(x1)}" y1="${r2(y)}" y2="${r2(y)}" stroke="${color}" stroke-width="${dots ? 0.8 : 0.5}"${dots ? ' stroke-dasharray="0.8 2.6" stroke-linecap="round"' : ''}/>`;
}

/** Splits rows of height `h` into `cols` columns of similar height; `keep(i)` says row i can't start a column. */
function toColumns(rows, cols, keep = () => false) {
  const per = rows.reduce((a, r) => a + r.h, 0) / cols;
  let col = 0, used = 0;
  return rows.map((r, i) => {
    if (col < cols - 1 && used > 0 && !keep(i) && used + r.h / 2 > per) { col++; used = 0; }
    used += r.h;
    return col;
  });
}

/** Contents laid out by section: numbered eyebrow, icon, serif section name, pages with descriptions. */
function tocSections(st, w, b, vm, o) {
  const th = theme(st);
  const bf = vm.fonts.body, cf = vm.fonts.caption, hf = vm.fonts.heading;
  const groups = [];
  for (const e of vm.toc || []) {
    if (e.kind === 'section') groups.push({ name: e.text, page: e.page, pages: [] });
    else if (groups.length) groups[groups.length - 1].pages.push(e);
  }
  const pagesToo = o.show !== 'sections';
  const cols = o.cols;
  const gap = 28;
  const colW = (b.w - gap * (cols - 1)) / cols;
  // a section's heading: the label, the name, a short gap before its pages
  const secH = cf.size * 1.1 + hf.size * 0.62 * 1.05 + hf.size * 0.62 * 0.5 + 8;
  // a page's row: its name, its description under it, then the rule — never through the description
  const contentH = bf.size * 1.25 + (o.descriptions ? cf.size * 1.55 : 0);
  const rowH = Math.max(contentH + (o.rules ? 6 : 1.5), (bf.size * 2.0 + (o.descriptions ? cf.size * 1.25 : 0)) * o.spacing);
  const heightOf = (g) => secH + (pagesToo ? g.pages.length * rowH : 0) + 14 * o.spacing;
  const colOf = toColumns(groups.map((g) => ({ h: heightOf(g) })), cols);
  const ic = hf.size * 0.62 * 2.1 * o.iconSize;
  const ss = hf.size * 0.62;
  let y = b.y, out = '', maxY = b.y;
  groups.forEach((g, gi) => {
    if (gi > 0 && colOf[gi] !== colOf[gi - 1]) y = b.y;
    const x0 = b.x + colOf[gi] * (colW + gap);
    const right = x0 + colW;
    let tx = x0;
    const label = `${o.label} ${secNum(o.numbering, gi)}`.trim();
    const ny = label ? y + cf.size * 1.1 + ss * 1.05 : y + ic / 2 + ss * 0.35;
    if (o.iconShape !== 'hide') {
      // the icon beside the section's name, centred on its letters
      const icon = o.icons[g.name] || o.icon;
      const iy = Math.max(y, ny - ss * 0.36 - ic / 2);
      out += `<g data-toc-icon="${esc(g.name)}">${glyph(icon, x0, iy, ic, { shape: o.iconShape, fill: o.iconFill || mixHex(th.ink, th.page_bg, 0.87), color: o.iconColor || th.ink, stroke: o.iconStroke || null, strokeWidth: o.iconStrokeWidth || 0.75 })}<rect x="${r2(x0)}" y="${r2(iy)}" width="${r2(ic)}" height="${r2(ic)}" fill="transparent"/></g>`;
      tx = x0 + ic + 10;
    }
    if (label) out += T(tx, y + cf.size * 0.9, label.toUpperCase(), { size: cf.size * 0.95, weight: 700, fill: w.style?.color || th.accent, family: cf.family, tracking: 0.12 });
    if (!pagesToo && o.pageNums) {
      const num = String(g.page);
      const nw = textWidth(num, ss, hf.weight, hf.family);
      const t = ell(g.name, right - tx - nw - 30, ss, hf.weight);
      out += T(tx, ny, t, { size: ss, weight: hf.weight, fill: hf.color, family: hf.family, italic: hf.italic });
      out += leader(o.leader, tx + textWidth(t, ss, hf.weight, hf.family) + 8, right - nw - 8, ny - 2, th.muted);
      out += T(right, ny, num, { size: ss, weight: hf.weight, fill: hf.color, family: hf.family, anchor: 'end' });
    } else {
      out += T(tx, ny, g.name, { size: ss, weight: hf.weight, fill: hf.color, family: hf.family, italic: hf.italic });
    }
    y += secH;
    if (pagesToo) for (const e of g.pages) {
      const num = o.pageNums ? String(e.page) : '';
      const nw = num ? textWidth(num, bf.size, 600, hf.family) : 0;
      const avail = colW - (tx - x0) - nw - 30;
      const t = ell(e.text, avail, bf.size, 600);
      out += T(tx, y + bf.size, t, { size: bf.size, weight: 600, fill: bf.color, family: bf.family });
      if (num) {
        const t2 = textWidth(t, bf.size, 600, bf.family);
        out += leader(o.leader, tx + t2 + 8, right - nw - 8, y + bf.size - 2, th.muted);
        out += T(right, y + bf.size, num, { size: bf.size * 1.05, weight: 600, fill: bf.color, family: hf.family, anchor: 'end' });
      }
      if (e.description && o.descriptions) out += T(tx, y + bf.size + cf.size * 1.35, ell(e.description, avail, cf.size * 0.95), { size: cf.size * 0.95, weight: 500, fill: th.muted, family: cf.family });
      if (o.rules) out += `<rect x="${r2(tx)}" y="${r2(y + contentH + 2.5)}" width="${r2(right - tx)}" height="0.6" fill="${th.grid}"/>`;
      y += rowH;
    }
    maxY = Math.max(maxY, y);
    y += 14 * o.spacing;
  });
  return { out, h: maxY - b.y };
}

/** Contents as a list: section labels in capitals, pages with leaders and numbers. */
function tocList(st, w, b, vm, o) {
  const th = theme(st);
  const bf = vm.fonts.body, cf = vm.fonts.caption;
  const lh = bf.size * 2 * o.spacing;
  const rows = [];
  let si = -1;
  for (const e of vm.toc || []) {
    if (e.kind === 'section') {
      si++;
      if (o.show === 'pages') continue;
      const n = secNum(o.numbering, si);
      const pre = [o.label, n].filter(Boolean).join(' ');
      const text = pre ? `${pre}${o.label ? ' · ' : '. '}${e.text}` : e.text;
      rows.push({ e, text, sec: true, h: o.show === 'sections' ? lh : lh * 1.2 });
    } else if (o.show !== 'sections') {
      rows.push({ e, text: e.text, h: lh + (o.descriptions && e.description ? cf.size * 1.25 : 0) });
    }
  }
  const cols = o.cols, gap = 24;
  const colW = (b.w - gap * (cols - 1)) / cols;
  // a section label never ends a column
  const colOf = toColumns(rows, cols, (i) => i > 0 && rows[i - 1].sec && o.show !== 'sections');
  const ind = o.show === 'all' ? 10 : 0;
  const secColor = w.style?.color || th.bad;
  let y = b.y, out = '', maxY = b.y;
  rows.forEach((r, i) => {
    if (i > 0 && colOf[i] !== colOf[i - 1]) y = b.y;
    const x = b.x + colOf[i] * (colW + gap);
    if (r.sec && o.show !== 'sections') {
      const label = cf.upper === false ? r.text : r.text.toUpperCase();
      out += T(x, y + lh * 0.35 + cf.size, ell(label, colW, cf.size, 700), { size: cf.size, weight: 700, fill: secColor, family: cf.family, italic: cf.italic });
      y += r.h;
      maxY = Math.max(maxY, y);
      return;
    }
    // a page, or a section when only sections are listed
    const weight = r.sec ? 600 : 500;
    const num = o.pageNums ? String(r.e.page) : '';
    const nw = num ? textWidth(num, bf.size, 600, bf.family, bf.italic) : 0;
    const avail = num ? colW - ind - nw - 30 : colW - ind;
    const tw = Math.min(textWidth(r.text, bf.size, weight, bf.family, bf.italic), avail);
    out += T(x + ind, y + bf.size, ell(r.text, avail, bf.size, weight), { size: bf.size, weight, fill: r.sec ? secColor : bf.color, family: bf.family, italic: bf.italic });
    if (num) {
      out += leader(o.leader, x + ind + tw + 6, x + colW - nw - 6, y + bf.size, th.muted);
      out += T(x + colW, y + bf.size, num, { size: bf.size, weight: 600, fill: bf.color, family: bf.family, anchor: 'end' });
    }
    if (!r.sec && o.descriptions && r.e.description) out += T(x + ind, y + bf.size + cf.size * 1.35, ell(r.e.description, avail, cf.size * 0.95), { size: cf.size * 0.95, weight: 500, fill: th.muted, family: cf.family });
    y += r.h;
    maxY = Math.max(maxY, y);
    if (o.rules) out += `<rect x="${r2(x + ind)}" y="${r2(y - bf.size * 0.45)}" width="${r2(colW - ind)}" height="0.6" fill="${th.grid}"/>`;
  });
  return { out, h: maxY - b.y };
}

/** Draws the contents so it fits its box: tighter spacing first, then without the rules between pages, then without descriptions. */
function tocWidget(st, w, b, vm) {
  let o = tocOpts(w.opts?.toc);
  const draw = (x) => (x.style === 'sections' ? tocSections(st, w, b, vm, x) : tocList(st, w, b, vm, x));
  let r = draw(o);
  while (r.h > b.h + 0.5 && o.spacing > 0.62) { o = { ...o, spacing: o.spacing - 0.06 }; r = draw(o); }
  if (r.h > b.h + 0.5 && o.rules && o.descriptions) { o = { ...o, rules: false }; r = draw(o); }
  if (r.h > b.h + 0.5 && o.descriptions) { o = { ...o, descriptions: false, spacing: tocOpts(w.opts?.toc).spacing }; r = draw(o); while (r.h > b.h + 0.5 && o.spacing > 0.62) { o = { ...o, spacing: o.spacing - 0.06 }; r = draw(o); } }
  return r.out;
}

export function arc(cx, cy, r, ri, a0, a1) {
  if (a1 - a0 >= Math.PI * 2 - 1e-6) a1 = a0 + Math.PI * 2 - 1e-4;
  const p = (rad, a) => `${r2(cx + rad * Math.cos(a))},${r2(cy + rad * Math.sin(a))}`;
  const large = a1 - a0 > Math.PI ? 1 : 0;
  if (ri <= 0) return `M${r2(cx)},${r2(cy)}L${p(r, a0)}A${r2(r)},${r2(r)} 0 ${large} 1 ${p(r, a1)}Z`;
  return `M${p(r, a0)}A${r2(r)},${r2(r)} 0 ${large} 1 ${p(r, a1)}L${p(ri, a1)}A${r2(ri)},${r2(ri)} 0 ${large} 0 ${p(ri, a0)}Z`;
}

// ---------------------------------------------------------------- parts

export function widgetParts(st, w, vm) {
  const th = theme(st);
  // the widget's design view from the theme, merged by Rust
  if (vm.eff) w = { ...w, opts: vm.eff.opts || {}, style: vm.eff.style || {} };
  C.clearSlots(w.id);
  CUR = vm.fonts?.chart || { family: 'Inter', italic: false, size: 7, weight: 400, color: th.muted };
  const ownKeys = (role) => { const a = { ...(st.doc.theme.fonts?.[role] || {}), ...(w.style?.fonts?.[role] || {}) }; return Object.keys(a).filter((k) => a[k] != null && a[k] !== ''); };
  PARTF = Object.fromEntries([['axis', 'axis'], ['labels', 'data_label'], ['legend', 'legend']].map(([k, r]) => [k, { f: vm.fonts?.[r], own: ownKeys(r) }]));
  const parts = { card: null, texts: [], table: null, graphic: null, accent: null };
  const f = vm.frame;
  TH = th;
  // a note signed by the banker: tinted with the theme's accent
  const note = w.kind === 'comment' && w.opts?.signed;
  if (vm.card) {
    const outlined = th.card === 'outlined' || w.kind === 'comment';
    parts.card = {
      x: f.x, y: f.y, w: f.w, h: f.h,
      // a note sits on the theme's own card colour unless its author picked another
      fill: w.style?.background || th.card_bg,
      stroke: w.style?.border || (outlined ? th.card_border || '#e4e3dd' : null),
      sw: w.style?.borderWidth || th.card_border_w || 0.75,
      radius: cardRadius(w.style?.radius ?? th.radius, f),
      // raised cards: no frame, a soft shadow under them
      shadow: th.card === 'shadow' && !w.style?.border && w.kind !== 'comment' ? mixHex(th.ink, th.page_bg, 0.9) : null,
    };
    if (parts.card.shadow) parts.card.stroke = null;
  } else if (w.style?.border || w.style?.background) {
    // no card, but an outline or a fill of its own: draw just that
    parts.card = {
      x: f.x, y: f.y, w: f.w, h: f.h,
      fill: w.style.background || 'none',
      stroke: w.style.border || null,
      sw: w.style.borderWidth || 0.75,
      radius: cardRadius(w.style?.radius ?? th.radius, f),
    };
  }
  // KPI cards can take the colour of their result
  let tone = null;
  if (w.kind === 'kpi' && vm.data?.kpi) {
    const S = C.resolve(st, w);
    const v = vm.data.kpi.value ?? 0;
    const good = (v >= 0) === S.upIsGood;
    const col = { sign: good ? S.colorUp : S.colorDown, good: S.colorUp, bad: S.colorDown, accent: th.accent, solid: good ? S.colorUp : S.colorDown, ink: th.ink }[S.tone];
    if (col) {
      tone = { col, solid: S.tone === 'solid' || S.tone === 'ink' };
      const card = parts.card || (parts.card = { x: f.x, y: f.y, w: f.w, h: f.h, radius: cardRadius(w.style?.radius ?? th.radius, f), sw: 0.75 });
      card.fill = tone.solid ? col : mixHex(col, th.page_bg, 0.86);
      card.stroke = tone.solid ? null : mixHex(col, th.page_bg, 0.62);
    }
  }
  // hairlines along chosen sides (column dividers)
  const sides = [].concat(w.style?.rule || []);
  if (sides.length) {
    const gap = (st.doc.page.gap || 8) / 2;
    const c = w.style?.ruleColor || th.grid;
    const lw = w.style?.ruleWidth || 0.75;
    parts.rules = sides.map((sd) => (sd === 'left' ? { x: f.x - gap, y: f.y, w: lw, h: f.h } : sd === 'right' ? { x: f.x + f.w + gap - lw, y: f.y, w: lw, h: f.h } : sd === 'top' ? { x: f.x, y: f.y - gap, w: f.w, h: lw } : { x: f.x, y: f.y + f.h + gap - lw, w: f.w, h: lw })).map((r) => ({ ...r, fill: c }));
  }
  if (tone?.solid) {
    const on = inkOn(tone.col);
    vm = { ...vm, fonts: { ...vm.fonts, title: { ...vm.fonts.title, color: on }, subtitle: { ...vm.fonts.subtitle, color: on }, kpi: { ...vm.fonts.kpi, color: on }, caption: { ...vm.fonts.caption, color: on } } };
    w = { ...w, opts: { ...w.opts, viz: { ...(w.opts?.viz || {}), valueColor: 'font', colorUp: on, colorDown: on } } };
  }
  if (vm.viz_text) w = { ...w, opts: { ...w.opts, viz: { ...(w.opts?.viz || {}), ...vm.viz_text } } };
  parts.deco = vm.deco || [];
  if (w.kind === 'comment') {
    parts.accent = { x: f.x + (vm.card ? 0 : 0), y: f.y + (vm.card ? th.radius * 0.6 : 0), w: 3, h: f.h - (vm.card ? th.radius * 1.2 : 0), fill: w.style?.color || (note ? noteInk(th) : '#d9a400') };
    // the note's mark (a question, something important…) as a badge on the avatar, or in the corner
    const mark = w.opts?.mark && TEXT_ICONS[w.opts.mark];
    if (mark) {
      const av = note && (vm.deco || []).find((d) => d.kind === 'rect' && d.radius > 0);
      const s2 = av ? av.w * 0.5 : 14;
      const bx = av ? av.x + av.w - s2 * 0.7 : f.x + f.w - s2 - 8, by = av ? av.y + av.h - s2 * 0.7 : f.y + 8;
      const col = toneColor(th, mark.tone) || th.ink;
      const body = `<circle cx="${r2(bx + s2 / 2)}" cy="${r2(by + s2 / 2)}" r="${r2(s2 / 2 + 1)}" fill="${th.page_bg}"/><svg x="${r2(bx)}" y="${r2(by)}" width="${r2(s2)}" height="${r2(s2)}" viewBox="0 0 24 24" color="${col}">${mark.d}</svg>`;
      parts.svgs = [...(parts.svgs || []), { x: bx - 1, y: by - 1, w: s2 + 2, h: s2 + 2, body }];
    }
  }
  const tf = vm.fonts.title, sf = vm.fonts.subtitle;
  // the title starts at the body's inset as the core worked it out (a card always has one, even when the
  // widget asks for none — otherwise its title and icon would sit on the card's edge)
  const inset = vm.card || vm.title ? (vm.body ? Math.max(0, vm.body.x - f.x - (w.kind === 'comment' ? 4 : 0)) : w.style?.pad ?? 10) : 0;
  let tx = f.x + inset;
  const ti = w.style?.titleIcon?.off ? null : w.style?.titleIcon;
  if (ti?.icon && vm.title) {
    // a line icon in a chip before the title
    const s2 = tf.size * (ti.size || 2.1) * TITLE_ICON;
    const cy = vm.title_y - tf.size * 0.36;
    const body = glyph(ti.icon, tx, cy - s2 / 2, s2, { shape: ti.shape || 'square', fill: ti.fill || mixHex(th.ink, th.page_bg, 0.88), color: ti.color || th.ink, stroke: ti.stroke || null, strokeWidth: ti.strokeWidth || 0.75, brandFill: ti.fill && ti.fill !== 'transparent' ? ti.fill : null });
    parts.svgs = [...(parts.svgs || []), { x: tx, y: cy - s2 / 2, w: s2, h: s2, body }];
    tx += s2 + tf.size * 0.7;
  } else if (ti && vm.title) {
    // a small coloured chip before the title (▲ leaders, ▼ laggards…)
    const s2 = tf.size * 1.35;
    const cy = vm.title_y - tf.size * 0.36;
    parts.deco = [...(parts.deco || []),
      { kind: 'rect', x: tx, y: cy - s2 / 2, w: s2, h: s2, fill: ti.color || th.accent, radius: s2 * 0.22 },
      { kind: 'text', x: tx + s2 / 2, y: cy + tf.size * 0.3, w: s2, text: ti.glyph || '▲', font: { ...tf, size: tf.size * 0.8, color: ti.ink || '#ffffff', tracking: 0, weight: 700 }, anchor: 'middle' }];
    tx += s2 + tf.size * 0.6;
  }
  // the title's alignment (Card & title → Title align); a view aligned right (a figure at the page's right
  // edge) takes its title along
  const talign = w.style?.titleAlign || (w.opts?.align === 'right' && !['heading', 'text'].includes(w.kind) ? 'right' : 'left');
  const widthOf = (txt, ff) => textWidth(txt, ff.size, ff.weight, ff.family, ff.italic, ff.tracking || 0);
  const placeX = (txt, ff) => (talign === 'right' ? f.x + f.w - inset - widthOf(txt, ff) : f.x + f.w / 2 - widthOf(txt, ff) / 2);
  let tlines = vm.title ? vm.title.split('\n').map((t, i) => ({ text: t, x: 0, y: tf.size * 0.96 + i * tf.size * 1.2 })) : [];
  if (talign !== 'left' && vm.title) {
    const xs = tlines.map((l) => placeX(l.text, tf));
    tx = Math.min(...xs);
    tlines = tlines.map((l, i) => ({ ...l, x: xs[i] - tx }));
  }
  if (vm.title) parts.texts.push({ name: 'Title', x: tx, y: vm.title_y - tf.size * 0.96, block: true, size: tf.size, weight: tf.weight, family: tf.family, italic: tf.italic, tracking: tf.tracking, color: tf.color, lines: tlines, lineH: tf.size * 1.2 });
  // a subtitle beside the title: right-aligned on the title's baseline; under it, aligned as the title
  const side = w.opts?.subSide && vm.title && vm.subtitle;
  const sx = side ? f.x + f.w - inset - widthOf(vm.subtitle, sf) : talign !== 'left' && vm.subtitle ? placeX(vm.subtitle, sf) : tx;
  if (vm.subtitle) parts.texts.push({ name: 'Subtitle', x: sx, y: vm.sub_y, size: sf.size, weight: sf.weight, family: sf.family, italic: sf.italic, tracking: sf.tracking, color: sf.color, lines: [{ text: vm.subtitle, x: 0, y: 0 }], lineH: sf.size * 1.375 });
  for (const t of vm.texts) {
    // Rust-wrapped blocks; line.y is the baseline offset from the block top
    parts.texts.push({
      name: w.kind === 'heading' ? 'Heading' : 'Text',
      x: t.x, y: t.y, size: t.block.size, weight: t.block.weight, family: t.block.family, italic: t.block.italic, tracking: t.block.tracking || 0,
      color: t.color || th.ink, lineH: t.block.line_h,
      lines: t.block.lines, block: true,
    });
  }
  if ((w.kind === 'comment' || w.kind === 'text') && !(w.opts?.text || '').trim()) {
    parts.placeholder = message(st, vm.body, w.kind === 'comment' ? 'Double-click to write your commentary' : 'Double-click to add text');
  }
  const data = vm.data;
  const b = vm.body;
  const needsData = ['kpi', 'bar', 'line', 'area', 'pie', 'table', 'treemap', 'waterfall', 'tiles', 'scale'].includes(w.kind);
  if (needsData) {
    if (!w.binding || !w.binding.dataset) parts.graphic = { ...b, body: message(st, b, 'Choose the data for this widget in the inspector.') };
    else if (!data) parts.graphic = { ...b, body: message(st, b, 'Loading…') };
    else if (data.error) parts.graphic = { ...b, body: message(st, b, '⚠ ' + data.error) };
    // the designer's own words when there is nothing to show (e.g. "no losses this period")
    else if (data.empty && w.kind !== 'kpi') parts.graphic = { ...b, body: message(st, b, w.opts?.emptyText || data.empty.message) };
    else if (w.kind === 'table' && w.opts?.table?.layout === 'facts' && !data.row_meta) {
      // key → value rows as a grid of facts
      parts.graphic = { ...b, body: factsSvg(st, w, vm, data, b) };
    }
    else if (w.kind === 'table') {
      parts.table = tablePart(st, w, vm, data);
      parts.tablePicture = usesPicture(w);
    }
    else if (w.kind === 'bar' && (w.opts?.viz?.orientation || w.opts?.orientation) === 'horizontal' && C.resolve(st, w).rankStyle === 'ranked') {
      // a bar chart's List layout is drawn by the table engine
      const L = listTable(st, C.resolve(st, w), data, b, CUR);
      if (L) {
        parts.table = tablePart(st, { ...w, kind: 'table', opts: { ...w.opts, table: L.table } }, { ...vm, table: L.t, fonts: { ...vm.fonts, table_cell: L.cell, table_head: L.cell } }, L.data);
        parts.tablePicture = true;
      } else parts.graphic = { ...b, body: message(st, b, 'A list needs one “Group by” field and a value.') };
    }
    else {
      const orient = w.opts?.viz?.orientation || w.opts?.orientation;
      const body = w.kind === 'kpi' ? C.kpi(st, w, b, data, vm)
        : w.kind === 'bar' ? (orient === 'horizontal' ? C.hbars(st, w, b, data, vm) : orient === 'share' ? C.sharebar(st, w, b, data, vm) : orient === 'scale' ? C.scale(st, w, b, data, vm) : C.columns(st, w, b, data, vm))
        : w.kind === 'scale' ? C.scale(st, w, b, data, vm)
        : w.kind === 'line' || w.kind === 'area' ? C.columns(st, w, b, data, vm, { allLines: true })
        : w.kind === 'treemap' ? C.treemap(st, w, b, data)
        : w.kind === 'waterfall' ? C.waterfall(st, w, b, data, vm)
        : w.kind === 'tiles' ? C.tiles(st, w, b, data, vm)
        : C.donut(st, w, b, data);
      parts.graphic = { ...b, body };
    }
  } else if (w.kind === 'composite') {
    // each part drawn as a widget of its own in its place (the core laid them out)
    parts.children = (w.parts || []).map((p, k) => {
      const pv = vm.parts?.[k];
      if (!pv) return null;
      const pw = { ...p, id: `${w.id}::${p.id}`, binding: p.binding ? { ...p.binding, dataset: p.binding.dataset || w.binding?.dataset || '' } : p.binding };
      return { id: pw.id, w: pw, vm: pv, parts: widgetParts(st, pw, pv) };
    }).filter(Boolean);
    if (!parts.children.length) parts.placeholder = message(st, b, 'Add parts in the inspector: a figure, a mini chart, text, an icon…');
  } else if (w.kind === 'toc') {
    parts.graphic = { ...b, body: tocWidget(st, w, b, vm) };
  } else if (w.kind === 'divider') {
    const c = w.style?.color || '#c3c2b7', lw = w.style?.width || 0.8;
    // drawn as a filled rectangle so PowerPoint gets a native shape
    // in the gap: centred in the gap before its column, between the widgets either side
    const gx = w.opts?.gap ? f.x - (st.page?.gap ?? 6) / 2 : f.x + f.w / 2;
    parts.rules = [...(parts.rules || []), w.opts?.vertical
      ? { x: gx - lw / 2, y: f.y, w: lw, h: f.h, fill: c }
      : { x: f.x, y: f.y + f.h / 2 - lw / 2, w: f.w, h: lw, fill: c }];
  } else if (w.kind === 'image') {
    const src = w.opts?.src;
    parts.graphic = src
      ? { x: f.x, y: f.y, w: f.w, h: f.h, body: `<image href="${esc(src)}" x="${r2(f.x)}" y="${r2(f.y)}" width="${r2(f.w)}" height="${r2(f.h)}" preserveAspectRatio="xMidYMid meet"/>` }
      : { x: f.x, y: f.y, w: f.w, h: f.h, body: message(st, f, 'Choose an image in the inspector.') };
  }
  return parts;
}

function textSvg(t) {
  let out = '';
  for (const l of t.lines) {
    const bx = t.x + (l.x || 0);
    const by = t.block ? t.y + l.y : t.y;
    const o = { size: t.size, weight: t.weight, fill: t.color, family: t.family || 'Inter', italic: !!t.italic, tracking: t.tracking || 0 };
    if (l.bullet) out += T(t.x + t.size * 0.15, by, '•', o);
    // icons and larger or smaller words are placed piece by piece
    out += l.runs ? (needsPieces(l.runs) ? piecesSvg(bx, by, l.runs, o, TH) : TR(bx, by, l.runs, o)) : T(bx, by, l.text, o);
  }
  return out;
}

function decoSvg(d) {
  if (d.kind === 'text') {
    const f = d.font;
    return T(d.x, d.y, d.text, { size: f.size, weight: f.weight, fill: f.color, family: f.family, italic: f.italic, tracking: f.tracking, anchor: d.anchor || 'start' });
  }
  return `<rect x="${r2(d.x)}" y="${r2(d.y)}" width="${r2(d.w)}" height="${r2(d.h)}"${d.radius ? ` rx="${r2(d.radius)}"` : ''} fill="${d.fill}"/>`;
}

/** A card's corners: never more than a quarter of its height or width (a short card stays a rectangle, not a pill). */
const cardRadius = (r, f) => Math.max(0, Math.min(r, f.h / 4, f.w / 4));

function partsSvg(st, p) {
  let out = '';
  if (p.card?.shadow) {
    const c = p.card;
    out += `<rect x="${r2(c.x - 0.6)}" y="${r2(c.y + 0.4)}" width="${r2(c.w + 1.2)}" height="${r2(c.h + 1.6)}" rx="${r2(c.radius + 0.6)}" fill="${c.shadow}" fill-opacity="0.45"/>`
      + `<rect x="${r2(c.x)}" y="${r2(c.y + 1.1)}" width="${r2(c.w)}" height="${r2(c.h)}" rx="${c.radius}" fill="${c.shadow}" fill-opacity="0.7"/>`;
  }
  // the card, the title (with its icon) are parts a click on the canvas leads to
  if (p.card) {
    out += C.P('card', `<rect x="${r2(p.card.x)}" y="${r2(p.card.y)}" width="${r2(p.card.w)}" height="${r2(p.card.h)}" rx="${p.card.radius}" fill="${p.card.fill}"${p.card.stroke ? ` stroke="${p.card.stroke}" stroke-width="${p.card.sw || 0.75}"` : ''}/>`);
  }
  for (const d of p.deco || []) out += C.P('title', decoSvg(d));
  for (const g of p.svgs || []) out += C.P('title', g.body);
  if (p.accent) out += `<rect x="${r2(p.accent.x)}" y="${r2(p.accent.y)}" width="${p.accent.w}" height="${r2(p.accent.h)}" rx="1.5" fill="${p.accent.fill}"/>`;
  for (const t of p.texts) out += t.name === 'Title' || t.name === 'Subtitle' ? C.P('title', textSvg(t)) : textSvg(t);
  if (p.table) out += tableSvg(st, p.table);
  if (p.graphic) out += p.graphic.body;
  // a composite's parts, each tagged with its own id (a click on one finds it)
  for (const c of p.children || []) {
    const rules = (c.parts.rules || []).map((r) => `<rect x="${r2(r.x)}" y="${r2(r.y)}" width="${r2(r.w)}" height="${r2(r.h)}" fill="${r.fill}"/>`).join('');
    out += `<g data-w="${esc(c.id)}">${rules}${partsSvg(st, c.parts)}</g>`;
  }
  return out;
}

/** Page tokens that change per page. */
export function pageTokens(st, pi, text) {
  const sec = st.doc.pages[pi].section || '';
  // numbers count only the pages shown (some hide when their data is empty)
  const num = st.pages?.[pi]?.number || pi + 1;
  const total = st.pages ? st.pages.filter((p) => !p.hidden).length : st.doc.pages.length;
  return String(text || '').replaceAll('{{page}}', String(num)).replaceAll('{{pages}}', String(total)).replaceAll('{{section}}', sec);
}

/**
 * Section tabs laid out from x, the current one marked in the theme's tab style
 * (theme.tabs.style): a filled pill (default), a box, a rule under it, or bold text alone.
 */
function tabsAt(st, pi, x0, y, maxW, align, f, bg) {
  const th = theme(st);
  const ts = st.doc.theme.tabs || {};
  const style = ['box', 'underline', 'text'].includes(ts.style) ? ts.style : 'pill';
  const weight = ts.weight || 600;
  const upper = !!ts.upper;
  const secs = [];
  for (const p of st.doc.pages) if (p.section && !secs.includes(p.section)) secs.push(p.section);
  if (!secs.length) return [];
  const cur = st.doc.pages[pi].section;
  const size = Math.max(6, f.size + 0.5);
  const h = size * 2.4;
  const full = align === 'full';
  const label = (s) => (upper ? s.toUpperCase() : s);
  const padX = style === 'text' || style === 'underline' ? 10 : 22;
  const widths = secs.map((s) => (full ? maxW / secs.length - 4 : textWidth(label(s), size, weight, f.family, f.italic) + padX));
  const total = widths.reduce((a, w) => a + w + 4, -4);
  let x = align === 'center' ? x0 + (maxW - total) / 2 : align === 'right' ? x0 + maxW - total : x0;
  return secs.map((s, i) => {
    const on = s === cur;
    const mark = themeColor(th, ts.fill) || (bg ? f.color || inkOn(bg) : th.ink);
    const shape = style === 'text' ? 'none' : style;
    const filled = shape === 'pill' || shape === 'box';
    const color = on ? (filled ? themeColor(th, ts.color) || bg || th.page_bg : themeColor(th, ts.color) || mark) : f.color || th.muted;
    const tw = textWidth(label(s), size, weight, f.family, f.italic);
    const tab = { x, y: y - h / 2, w: widths[i], h, text: label(s), on, size, weight, family: f.family, italic: f.italic, shape, radius: shape === 'box' ? Math.min(2, h / 4) : h / 2,
      fill: on && filled ? mark : 'none', color,
      // the rule under the current tab, as wide as its words
      under: on && shape === 'underline' ? { x: x + (widths[i] - tw) / 2, y: y + size * 0.85, w: tw, h: 1.4, fill: mark } : null };
    x += widths[i] + 4;
    return tab;
  });
}

/**
 * The header or footer of a page as plain pieces (rects, lines, texts,
 * images, tabs) so the screen, the PDF and PowerPoint draw the same thing.
 */
export function bandPieces(st, pi, which) {
  const band = st[which];
  if (!band?.show || st.doc.pages[pi].plain) return [];
  const pg = st.page;
  const th = theme(st);
  const c = pg.content;
  const gap = 6;
  const top = which === 'header' ? pg.margin : c.y + c.h + gap;
  const hgt = Math.max(8, band.height) - gap;
  const mid = top + hgt / 2;
  const out = [];
  if (band.fill) {
    // a filled band bleeds to the page edge
    const y0 = which === 'header' ? 0 : top - gap / 2;
    const y1 = which === 'header' ? top + hgt + gap / 2 : pg.h;
    out.push({ kind: 'rect', x: 0, y: y0, w: pg.w, h: y1 - y0, fill: themeColor(th, band.fill) });
  }
  if (band.line && band.line !== 'none') {
    const ly = which === 'header' ? (band.line === 'top' ? top - 2 : top + hgt + 2) : band.line === 'bottom' ? top + hgt + 2 : top - 2;
    out.push({ kind: 'line', x: c.x, y: ly, w: c.w, color: themeColor(th, band.line_color) || th.grid });
  }
  const third = c.w / 3;
  const area = { left: [c.x, third * 1.45], center: [c.x + third * 0.5, third * 2], right: [c.x + c.w - third * 1.45, third * 1.45] };
  const tabs = band.tabs && band.tabs !== 'none' ? band.tabs : null;
  if (tabs) {
    const f = band[tabs === 'full' ? 'left' : tabs]?.font || st.fonts?.caption || { size: 7, family: 'Inter' };
    const [ax, aw] = tabs === 'full' ? [c.x, c.w] : area[tabs];
    for (const t of tabsAt(st, pi, ax, mid, aw, tabs === 'full' ? 'full' : tabs, f, themeColor(th, band.fill) || null)) out.push({ kind: 'tab', ...t });
  }
  for (const pos of ['left', 'center', 'right']) {
    if (tabs === pos || tabs === 'full') continue;
    const s = band[pos];
    if (!s) continue;
    const f = s.font;
    const lines = pageTokens(st, pi, s.text).split('\n').filter((l, i, a) => l.trim() || a.length === 1).map((l) => (f.upper ? l.toUpperCase() : l)).filter((l) => l !== '');
    const [ax, aw] = area[pos];
    const imgH = s.image ? Math.min(s.image_h || hgt, hgt) : 0;
    const imgW = s.image ? imgH * (s.image_ratio || 3) : 0;
    const lh = f.size * 1.25;
    const textW = lines.length ? Math.max(...lines.map((l) => textWidth(l, f.size, f.weight, f.family, f.italic))) : 0;
    const totalW = imgW + (imgW && textW ? 6 : 0) + textW;
    let x = pos === 'left' ? ax : pos === 'right' ? ax + aw - totalW : ax + (aw - totalW) / 2;
    if (s.image) {
      out.push({ kind: 'image', x, y: mid - imgH / 2, w: imgW, h: imgH, src: s.image, align: pos });
      x += imgW + 6;
    }
    if (lines.length) {
      const y0 = mid - (lh * lines.length) / 2;
      out.push({ kind: 'text', x, y: y0, w: Math.min(textW + 4, aw), h: lh * lines.length, lh, lines: lines.map((l) => ellipsize(l, aw - imgW, f.size, f.weight, f.family, f.italic)), font: f, align: pos === 'center' ? 'middle' : pos === 'right' ? 'end' : 'start', anchorX: pos === 'left' ? x : pos === 'right' ? ax + aw : x + textW / 2 });
    }
  }
  return out;
}

function chromeSvg(st, pi) {
  let out = '';
  for (const which of ['header', 'footer']) for (const p of bandPieces(st, pi, which)) {
    if (p.kind === 'rect') out += `<rect x="${r2(p.x)}" y="${r2(p.y)}" width="${r2(p.w)}" height="${r2(p.h)}" fill="${p.fill}"/>`;
    else if (p.kind === 'line') out += `<line x1="${r2(p.x)}" x2="${r2(p.x + p.w)}" y1="${r2(p.y)}" y2="${r2(p.y)}" stroke="${p.color}" stroke-width="0.6"/>`;
    else if (p.kind === 'image') out += `<image href="${esc(p.src)}" x="${r2(p.x)}" y="${r2(p.y)}" width="${r2(p.w)}" height="${r2(p.h)}" preserveAspectRatio="${p.align === 'right' ? 'xMaxYMid' : p.align === 'center' ? 'xMidYMid' : 'xMinYMid'} meet"/>`;
    else if (p.kind === 'tab') {
      if (p.on && p.fill !== 'none') out += `<rect x="${r2(p.x)}" y="${r2(p.y)}" width="${r2(p.w)}" height="${r2(p.h)}" rx="${r2(p.radius)}" fill="${p.fill}"/>`;
      if (p.under) out += `<rect x="${r2(p.under.x)}" y="${r2(p.under.y)}" width="${r2(p.under.w)}" height="${r2(p.under.h)}" fill="${p.under.fill}"/>`;
      const wt = p.on && p.shape === 'none' ? Math.max(p.weight, 700) : p.weight;
      out += T(p.x + p.w / 2, p.y + p.h / 2 + p.size * 0.36, ellipsize(p.text, p.w - 8, p.size, wt, p.family, p.italic), { size: p.size, weight: wt, fill: p.color, family: p.family, italic: p.italic, anchor: 'middle' });
    } else if (p.kind === 'text') {
      const f = p.font;
      p.lines.forEach((l, i) => { out += T(p.anchorX, p.y + p.lh * i + f.size * 0.98, l, { size: f.size, weight: f.weight, fill: f.color, family: f.family, italic: f.italic, anchor: p.align }); });
    }
  }
  return out;
}

/** The whole page as one SVG document (screen and PDF). */
export function pageSvg(st, pi, { exportMode = false, only = null } = {}) {
  setLocale(st.doc.theme.locale, st.formats || st.doc.formats);
  setCustomIcons(st.doc.icons);
  const pg = st.page;
  const th = theme(st);
  const doc = st.doc.pages[pi];
  const vms = st.pages[pi].widgets;
  // the cover and the closing page may have a paper of their own (the theme's)
  let body = `<rect x="0" y="0" width="${pg.w}" height="${pg.h}" fill="${st.pages[pi]?.fill || th.page_bg}"/>`;
  doc.widgets.forEach((w, i) => {
    if (only && !only.has(w.id)) return;
    const vm = vms[i];
    // a linked widget shows after its table's last rows (its mirror) while the table continues
    if (vm.moved) return;
    const p = widgetParts(st, w, vm);
    const content = partsSvg(st, p) + (p.placeholder && !exportMode ? p.placeholder : '');
    // clip each widget to its frame so nothing spills onto neighbours
    const f = vm.frame;
    const rules = (p.rules || []).map((r) => `<rect x="${r2(r.x)}" y="${r2(r.y)}" width="${r2(r.w)}" height="${r2(r.h)}" fill="${r.fill}"/>`).join('');
    body += `<g data-w="${w.id}">${rules}<clipPath id="c${w.id}"><rect x="${r2(f.x - 1)}" y="${r2(f.y - 1)}" width="${r2(f.w + 2)}" height="${r2(f.h + 2)}"/></clipPath><g clip-path="url(#c${w.id})">${content}</g></g>`;
  });
  body += chromeSvg(st, pi);
  if (doc.frame) {
    const inset = Math.min(16, pg.margin * 0.45);
    body += `<rect x="${inset}" y="${inset}" width="${r2(pg.w - inset * 2)}" height="${r2(pg.h - inset * 2)}" fill="none" stroke="${themeColor(th, doc.frame)}" stroke-width="0.75"/>`;
  }
  const size = exportMode ? `width="${pg.w}" height="${pg.h}"` : `width="100%" height="100%"`;
  return `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 ${pg.w} ${pg.h}" ${size} font-family="Inter">${body}</svg>`;
}

/**
 * One widget on its own, cropped to its frame with `pad` points around it
 * (the theme studio's previews). Returns the SVG and the frame.
 */
export function widgetSvg(st, pi, wi = 0, { pad = 0 } = {}) {
  setLocale(st.doc.theme.locale, st.formats || st.doc.formats);
  setCustomIcons(st.doc.icons);
  const th = theme(st);
  const w = st.doc.pages[pi].widgets[wi];
  const vm = st.pages[pi].widgets[wi];
  const p = widgetParts(st, w, vm);
  const f = vm.frame;
  const rules = (p.rules || []).map((r) => `<rect x="${r2(r.x)}" y="${r2(r.y)}" width="${r2(r.w)}" height="${r2(r.h)}" fill="${r.fill}"/>`).join('');
  const id = `pv${w.id.replace(/[^\w-]/g, '_')}`;
  const vb = [f.x - pad, f.y - pad, f.w + pad * 2, f.h + pad * 2].map(r2);
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${vb.join(' ')}" width="100%" font-family="Inter" data-frame="${[f.x, f.y, f.w, f.h].map(r2).join(' ')}" data-pad="${pad}">`
    + `<rect x="${vb[0]}" y="${vb[1]}" width="${vb[2]}" height="${vb[3]}" fill="${th.page_bg}"/>`
    + `<g class="pv-content">${rules}<clipPath id="${id}"><rect x="${r2(f.x - 1)}" y="${r2(f.y - 1)}" width="${r2(f.w + 2)}" height="${r2(f.h + 2)}"/></clipPath><g clip-path="url(#${id})">${partsSvg(st, p)}</g></g></svg>`;
  return { svg, frame: f };
}

/** Native PowerPoint elements for a page. */
export function slide(st, pi) {
  setLocale(st.doc.theme.locale, st.formats || st.doc.formats);
  setCustomIcons(st.doc.icons);
  const th = theme(st);
  const doc = st.doc.pages[pi];
  const vms = st.pages[pi].widgets;
  const elements = [];
  doc.widgets.forEach((w, i) => {
    const vm = vms[i];
    if (vm.moved) return;
    put(w, vm, widgetParts(st, w, vm));
  });
  // one widget's native elements (a composite's parts after its card, each as a widget of its own)
  function put(w, vm, p) {
    if (p.card?.shadow) elements.push({ kind: 'shape', x: p.card.x, y: p.card.y + 1.1, w: p.card.w, h: p.card.h, fill: mixHex(p.card.shadow, st.doc.theme.page_bg, 0.3), radius: p.card.radius });
    if (p.card) elements.push({ kind: 'shape', x: p.card.x, y: p.card.y, w: p.card.w, h: p.card.h, fill: ['none', 'transparent'].includes(p.card.fill) ? null : p.card.fill, stroke: p.card.stroke, stroke_w: p.card.sw || 0, radius: p.card.radius });
    for (const r of p.rules || []) elements.push({ kind: 'shape', x: r.x, y: r.y, w: r.w, h: r.h, fill: r.fill });
    for (const g of p.svgs || []) elements.push({ kind: 'svg', x: g.x, y: g.y, w: g.w, h: g.h, name: 'Icon', svg: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${r2(g.x)} ${r2(g.y)} ${r2(g.w)} ${r2(g.h)}" width="${r2(g.w)}" height="${r2(g.h)}">${g.body}</svg>` });
    for (const d of p.deco || []) {
      if (d.kind !== 'text') { elements.push({ kind: 'shape', x: d.x, y: d.y, w: d.w, h: d.h, fill: d.fill, radius: d.radius || 0 }); continue; }
      const f = d.font;
      const x = d.anchor === 'end' ? d.x - d.w : d.anchor === 'middle' ? d.x - d.w / 2 : d.x;
      elements.push({ kind: 'text', x, y: d.y - f.size * 1.02, w: d.w + 4, h: f.size * 1.4, name: 'Label', paras: [{ align: d.anchor === 'end' ? 'right' : d.anchor === 'middle' ? 'center' : undefined, runs: [{ text: d.text, size: f.size, color: f.color, bold: f.weight >= 600, font: f.family, italic: !!f.italic, spacing: (f.tracking || 0) * f.size }] }] });
    }
    if (p.accent) elements.push({ kind: 'shape', x: p.accent.x, y: p.accent.y, w: p.accent.w, h: p.accent.h, fill: p.accent.fill, radius: 1.5 });
    for (const t of p.texts) elements.push(textElement(t, vm, w));
    if (p.table && (p.tablePicture || p.table.groupRow)) {
      // pills and data bars need the exact drawing
      const tb = p.table;
      const hgt = tb.rows.reduce((a, r) => a + r.h, 0) + (tb.groupH || 0);
      p.graphic = { x: tb.x, y: tb.y - (tb.groupH || 0), w: tb.w, h: hgt, body: tableSvg(st, tb) };
    } else if (p.table) {
      elements.push({ kind: 'table', x: p.table.x, y: p.table.y, w: p.table.w, cols: p.table.cols, size: p.table.size, pad: p.table.pad ?? 5, name: w.title || 'Table', rows: pptxRows(p.table) });
    }
    if (p.graphic) {
      const g = p.graphic;
      const pad = 2;
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="${r2(g.x - pad)} ${r2(g.y - pad)} ${r2(g.w + pad * 2)} ${r2(g.h + pad * 2)}" width="${r2(g.w + pad * 2)}" height="${r2(g.h + pad * 2)}" font-family="${FONT}">${g.body}</svg>`;
      elements.push({ kind: 'svg', x: g.x - pad, y: g.y - pad, w: g.w + pad * 2, h: g.h + pad * 2, svg, name: w.title || w.kind });
    }
    for (const c of p.children || []) put(c.w, c.vm, c.parts);
  }
  // page chrome as native elements
  for (const which of ['header', 'footer']) for (const p of bandPieces(st, pi, which)) {
    if (p.kind === 'rect') elements.unshift({ kind: 'shape', x: p.x, y: p.y, w: p.w, h: p.h, fill: p.fill });
    else if (p.kind === 'line') elements.push({ kind: 'shape', x: p.x, y: p.y - 0.3, w: p.w, h: 0.6, fill: p.color });
    else if (p.kind === 'image') elements.push({ kind: 'svg', x: p.x, y: p.y, w: p.w, h: p.h, name: 'Logo', svg: `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 ${r2(p.w)} ${r2(p.h)}" width="${r2(p.w)}" height="${r2(p.h)}"><image href="${esc(p.src)}" x="0" y="0" width="${r2(p.w)}" height="${r2(p.h)}" preserveAspectRatio="xMidYMid meet"/></svg>` });
    else if (p.kind === 'tab') {
      if (p.on && p.fill !== 'none') elements.push({ kind: 'shape', x: p.x, y: p.y, w: p.w, h: p.h, fill: p.fill, radius: p.radius });
      if (p.under) elements.push({ kind: 'shape', x: p.under.x, y: p.under.y, w: p.under.w, h: p.under.h, fill: p.under.fill });
      elements.push({ kind: 'text', x: p.x, y: p.y + (p.h - p.size * 1.3) / 2, w: p.w, h: p.size * 1.4, name: 'Section tab', paras: [{ align: 'center', runs: [{ text: p.text, size: p.size, color: p.color, bold: p.weight >= 600 || (p.on && p.shape === 'none'), font: p.family }] }] });
    } else if (p.kind === 'text') {
      const f = p.font;
      const x = p.align === 'end' ? p.anchorX - p.w : p.align === 'middle' ? p.anchorX - p.w / 2 : p.x;
      elements.push({ kind: 'text', x, y: p.y, w: p.w + 2, h: p.h + 2, name: which === 'header' ? 'Header' : 'Footer', paras: p.lines.map((l) => ({ align: p.align === 'end' ? 'right' : p.align === 'middle' ? 'center' : undefined, line_h: p.lh, runs: [{ text: l, size: f.size, color: f.color, bold: f.weight >= 600, font: f.family, italic: !!f.italic }] })) });
    }
  }
  if (doc.frame) {
    const inset = Math.min(16, st.page.margin * 0.45);
    elements.unshift({ kind: 'shape', x: inset, y: inset, w: st.page.w - inset * 2, h: st.page.h - inset * 2, stroke: themeColor(th, doc.frame), stroke_w: 0.75 });
  }
  return { background: st.pages[pi]?.fill || th.page_bg, elements };
}

function textElement(t, vm, w) {
  // merge Rust lines back into paragraphs so the text stays editable
  const paras = [];
  let cur = null;
  let prevY = null;
  for (const l of t.lines) {
    const gap = prevY == null ? 0 : l.y - prevY;
    const newPara = !cur || l.bullet || gap > t.lineH * 1.05 || !t.block;
    const pieces = l.runs ? l.runs.map((r) => ({ ...r })) : [{ text: l.text }];
    if (newPara) {
      cur = { pieces, bullet: !!l.bullet, before: cur ? Math.max(0, gap - t.lineH) : 0 };
      paras.push(cur);
    } else {
      cur.pieces.push({ text: ' ' }, ...pieces);
    }
    prevY = l.y;
  }
  const top = t.block ? t.y : t.y - t.size * 0.96;
  const width = t.block ? (w.kind === 'heading' ? vm.frame.w : vm.body.w) : vm.frame.w - 20;
  const height = t.block ? (t.lines.length ? t.lines[t.lines.length - 1].y + t.size * 0.4 : t.size) : t.lineH;
  // an icon becomes its character in its colour
  const run = (r) => ({ text: r.icon ? (TEXT_ICONS[r.icon]?.ch || '•') : r.text, size: t.size * (r.size || 1), bold: !!r.bold || t.weight >= 600, color: r.icon ? iconColor(TH, r.icon, r.color, t.color) : r.color || t.color, font: t.family || 'Inter', italic: r.italic ? !t.italic : !!t.italic, spacing: (t.tracking || 0) * t.size });
  return {
    kind: 'text', x: t.x, y: top, w: Math.min(width, vm.frame.x + vm.frame.w - t.x), h: height, name: t.name,
    paras: paras.map((p) => ({ bullet: p.bullet, line_h: t.lineH, before: p.before || null, runs: p.pieces.map(run) })),
  };
}
