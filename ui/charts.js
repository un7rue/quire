// Settings-driven charts, KPIs and tiles. Every visual choice comes from one
// resolved settings object: look preset → template defaults (theme.viz.all,
// theme.viz[kind]) → the widget's own opts.viz. Older option names are read
// too, so existing templates keep their look.

import { T, TR, CS, tw, ell, shape, message, seriesColor, r2, arc, themeColor, PF, curFont } from './render.js';
import { fmtNum, niceTicks, mixHex, getLocale, textWidth, ellipsize, clamp, inkOn, legible, monthShort } from './util.js';
import { glyph, glyphFor } from './glyphs.js';

// ---------------------------------------------------------------- settings

export const BASE = {
  colorMode: 'auto', colorUp: '', colorDown: '', colorTotal: '', colorStart: '', palette: null,
  tint: 0, outline: false, outlineWidth: 0.5, radius: 2.5, roundAll: false, barWidth: 0.7, maxBar: 24, stacked: false,
  labels: 'auto', labelPos: 'auto', labelColor: 'auto', labelUnits: '', labelDecimals: null, labelSuffix: null, format: 'auto', units: '', decimals: null, signed: false, suffix: '',
  axis: true, ticks: 5, yMin: null, yMax: null, grid: true, zeroLine: true, xLabels: true,
  legend: 'auto', legendUp: '', legendDown: '', legendTotal: '', legendSwatch: 'rounded', legendSize: 8, legendOrder: 'data',
  // the category axis: every label or every Nth, turned, and dates written their own way
  xEvery: 0, xRotate: 0, xDate: 'auto',
  // a reference line (a target, the average…) and a band (a range that is fine), each with a label
  refLine: 'none', refValue: null, refLabel: '', refColor: '', refStyle: 'dashed', bandFrom: null, bandTo: null, bandLabel: '', bandColor: '',
  extremes: false, bands: true, bestLabel: '', worstLabel: '', extremesRule: 'sign',
  callout: 'none', calloutBg: '', calloutColor: '',
  lineWidth: 1.75, lineStyle: 'solid', curve: 'straight', markers: 'last', markerSize: 2.8, markerFill: 'solid', lineColor: '', areaOpacity: 0.12,
  total: true, totalLabel: '', connectors: 'dashed', bridge: false,
  rankStyle: 'bars', highlight: '', rankNumbers: true, track: true,
  inner: 0.62, mono: false, baseColor: '', sliceGap: 1.2, center: 'total', centerText: '', centerLabel: '', legendStyle: 'simple', legendAmount: false, legendShare: true, maxSlices: 7,
  tmLayout: 'squarify', chipBelow: 2, tmGap: 2, tmOutline: false, shareText: '',
  cols: 0, tileBg: '', tileBorder: true, valueColor: 'sign', markExtremes: true,
  kpiAlign: 'left', valueSize: 0, delta: true, deltaFormat: 'auto', deltaText: '', spark: 'line', sparkColor: '', sparkPos: 'auto', compact: 'auto', upIsGood: true,
  tone: 'none', arrow: false, note: '', noteColor: 'sign', breakdown: false, breakdownTone: 'sign', tileTone: 'none',
  shareGap: 1.5, shareHeight: 0, legendCols: 0,
  // donut slices and split-bar parts: their own lighten / outline (the bar ones come from the preset)
  partTint: 0, partOutline: false,
  signedTicks: false, labelBold: false, centerUpper: false,
  rankHighlight: true, markBest: false, y2Axis: true, rowLines: false, trackFill: false, trackLines: true, barBase: 'zero', subLine: false, listValues: true, barThick: 0,
  track: true, trackStyle: 'gradient', scaleMin: null, scaleMax: null, scaleTicks: '', valueLabels: true, nameLabels: true, markerStyle: 'ring',
  // the theme's shape language: gridline dash, bars and areas fading to the base, guide lines
  // from points down to the axis, value labels as plain text, in a framed box or in a pill
  gridStyle: 'solid', gridDir: 'h', barFill: 'flat', areaFill: 'none', guides: false, labelBox: 'none',
};

export const PRESETS = {
  clean: {},
  soft: {
    tint: 0.5, outline: true, radius: 4, roundAll: true, maxBar: 34, labels: 'all', labelColor: 'sign',
    legend: 'top-right', extremes: true, callout: 'last', markers: 'all', markerFill: 'hollow', lineWidth: 2,
    rankStyle: 'ranked', legendStyle: 'table', legendAmount: true, mono: true, inner: 0.56, sliceGap: 2, tmLayout: 'split', ticks: 4,
    colorMode: 'signOne', signedTicks: true, labelBold: true, centerUpper: true,
  },
};

// (an empty label unit is a choice — "no unit on the labels" — not a blank to skip)
const drop = (o) => Object.fromEntries(Object.entries(o || {}).filter(([k, v]) => v !== null && v !== undefined && (v !== '' || k === 'labelSuffix')));

/** Older option names → settings. */
function legacy(w) {
  const o = w.opts || {};
  const out = {};
  if (o.labels !== undefined) out.labels = o.labels ? 'all' : 'none';
  if (o.endLabels === false) out.labels = 'none';
  if (o.signColors !== undefined) out.colorMode = o.signColors ? 'sign' : 'series';
  if (o.stacked !== undefined) out.stacked = !!o.stacked;
  for (const k of ['units', 'highlight', 'totalLabel', 'centerLabel', 'cols', 'suffix', 'decimals', 'labelUnits', 'labelDecimals', 'labelSuffix']) if (o[k] != null && (o[k] !== '' || k === 'labelSuffix')) out[k] = o[k];
  if (o.extremes !== undefined) out.extremes = !!o.extremes;
  if (o.callout !== undefined) out.callout = o.callout ? 'last' : 'none';
  if (o.mono !== undefined) out.mono = !!o.mono;
  if (o.donut === false) out.inner = 0;
  if (o.legendValues !== undefined) { out.legendStyle = o.legendValues ? 'table' : 'simple'; out.legendAmount = !!o.legendValues; }
  if (o.center) out.center = o.center;
  if (o.total !== undefined) out.total = !!o.total;
  if (o.legend === false) out.legend = 'none';
  if (o.spark === false) out.spark = 'none';
  if (o.delta === false) out.delta = false;
  if (o.signed !== undefined) out.signed = !!o.signed;
  if (o.compact !== undefined && o.compact !== null) out.compact = o.compact ? 'on' : 'off';
  if (o.invert) out.upIsGood = false;
  if (o.highlight === false) out.markExtremes = false;
  return out;
}

/**
 * Categories each chart drew a place for an icon on, by widget id: the
 * inspector tells which icon settings can't show (folded into "Other").
 */
const SLOTS = new Map();
export const iconSlots = (id) => SLOTS.get(id);
export const clearSlots = (id) => SLOTS.delete(id);
const slots = (w) => { const set = new Set(); if (w?.id) SLOTS.set(w.id, set); return set; };

/** A chart's icon chips: shape, background, icon colour and outline from its settings. */
const chip = (S, th, shape, fill = mixHex(th.ink, th.page_bg, 0.87)) => ({
  shape: S.iconShape || shape, fill: S.iconFill || fill, color: S.iconColor || th.ink,
  stroke: S.iconStroke || null, strokeWidth: S.iconStrokeWidth || null, brandFill: S.iconFill && S.iconFill !== 'transparent' ? S.iconFill : null,
});

/**
 * The best and the worst of some values (indexes, -1 for none). `rule`:
 * 'sign' (default) — best only if it gained (above 0), worst only if it lost
 * (below 0), so all-positive values have no worst and a zero is neither;
 * 'rank' — simply the highest and the lowest (unless all are equal).
 */
export function pickExtremes(vals, rule = 'sign', reverse = false) {
  let best = -1, worst = -1;
  vals.forEach((v0, i) => {
    // a zero is never best or worst, whatever the rule
    if (typeof v0 !== 'number' || !Number.isFinite(v0) || Math.abs(v0) < 1e-9) return;
    const v = reverse ? -v0 : v0;
    const g = (k) => (reverse ? -vals[k] : vals[k]);
    if (best < 0 || v > g(best)) best = i;
    if (worst < 0 || v < g(worst)) worst = i;
  });
  if (best < 0) return { best, worst };
  const g = (k) => (reverse ? -vals[k] : vals[k]);
  if (g(best) === g(worst)) return { best: -1, worst: -1 };
  if (rule !== 'rank') {
    if (!(g(best) > 0)) best = -1;
    if (!(g(worst) < 0)) worst = -1;
  }
  return { best, worst };
}

export function resolve(st, w) {
  const th = st.doc.theme;
  const look = w.opts?.viz?.look || w.opts?.look || th.viz?.[w.kind]?.look || th.viz?.all?.look || th.chart_look || 'clean';
  const S = { ...BASE, ...(PRESETS[look] || {}), ...drop(th.viz?.all), ...drop(th.viz?.[w.kind]), ...drop(legacy(w)), ...drop(w.opts?.viz), look };
  S.colorUp = S.colorUp || th.good;
  S.colorDown = S.colorDown || th.bad;
  S.colorTotal = S.colorTotal || th.good;
  // a bridge waterfall's starting level: a quiet neutral unless set
  S.colorStart = S.colorStart || th.muted;
  // a list's names: bold as set, else as the wide-bar layout always drew them
  if (S.nameBold == null) S.nameBold = !!S.trackFill;
  // icons switched off: none, whatever the view or the template lists
  if (S.iconsOff) { S.icons = null; S.iconsAuto = false; }
  // theme colours by name ($accent, $series2…), wherever they were set (the theme's chart defaults too)
  const named = (v) => (typeof v === 'string' && v[0] === '$' ? themeColor(th, v) : v);
  for (const k of Object.keys(S)) {
    const v = S[k];
    if (typeof v === 'string') S[k] = named(v);
    else if (Array.isArray(v)) { if (v.some((x) => typeof x === 'string' && x[0] === '$')) S[k] = v.map(named); }
    else if (v && typeof v === 'object' && Object.values(v).some((x) => typeof x === 'string' && x[0] === '$')) S[k] = Object.fromEntries(Object.entries(v).map(([a, x]) => [a, named(x)]));
  }
  return S;
}

// ---- parts: the pieces of a chart a click on the canvas leads to (its legend, axis, a series…);
// the inspector opens the part of the Design tab that changes them (PART_SECTIONS in panels.js)
const attr = (x) => String(x).replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');
/** `svg` as one part of the chart, or nothing when it is empty. */
export const P = (part, svg) => (svg ? `<g data-part="${attr(part)}">${svg}</g>` : '');
/** What was added to `out` since `from`, as one part. */
const tag = (out, from, part) => out.slice(0, from) + P(part, out.slice(from));

// ---- a chart's own parts' text (axis labels, data labels, legend): drawn as always — a size
// `d` over the chart text, the weight and colour each place gives it — with what the designer set
// for the part's text role (Text tab, or the part's own section) on top
const PART_ROLE = { axis: 'axis', labels: 'labels', legend: 'legend' };
function pf(part, d, o = {}) {
  const out = { size: CS(d), ...o };
  const p = PF()[PART_ROLE[part]];
  if (!p?.f) return out;
  for (const k of p.own) {
    if (k === 'color') out.fill = p.f.color;
    else if (k in p.f) out[k] = p.f[k];
  }
  return out;
}
/** Its words as drawn (in capitals when the role says so). */
const up = (o, x) => (o.upper ? String(x).toUpperCase() : x);
/** Text in a part's font. */
const PT = (x, y, text, o) => T(x, y, up(o, text), o);
/** How wide a part's text is. */
const pw = (o, text) => textWidth(up(o, text), o.size, o.weight || 400, o.family || curFont().family, o.italic ?? curFont().italic, o.tracking || 0);

// ---- what an "auto" setting came to when the widget was last drawn (a KPI's figure size, a donut centre's,
// an axis's ends…): the inspector shows it beside "auto", and its arrows start from it
export const DRAWN = new Map();
const note = (w, k, v) => {
  if (!w?.id || v == null || !Number.isFinite(+v)) return;
  const m = DRAWN.get(w.id) || {};
  m[k] = Math.round(+v * 100) / 100;
  DRAWN.set(w.id, m);
};

const WORDS = {
  ru: { average: 'среднее', median: 'медиана', max: 'макс.', min: 'мин.', right: 'прав.', left: 'лев.', Other: 'Прочее', best: 'ЛУЧШИЙ', worst: 'ХУДШИЙ', up: 'рост', down: 'спад', total: 'итог', Total: 'Итог', share: 'портфеля', vs: 'к' },
  de: { average: 'Durchschnitt', median: 'Median', max: 'Max.', min: 'Min.', right: 'rechts', left: 'links', Other: 'Sonstige', best: 'BESTER', worst: 'SCHWÄCHSTER', up: 'Anstieg', down: 'Rückgang', total: 'gesamt', Total: 'Gesamt', share: 'des Portfolios', vs: 'vs' },
  en: { average: 'average', median: 'median', max: 'max', min: 'min', right: 'RHS', left: 'LHS', Other: 'Other', best: 'BEST', worst: 'WORST', up: 'up', down: 'down', total: 'total', Total: 'Total', share: 'of total', vs: 'vs' },
};
export const word = (k) => (WORDS[getLocale().slice(0, 2)] || WORDS.en)[k];

// ---------------------------------------------------------------- shared pieces

/** Fill and outline for a mark of base colour `c`. `strong` lightens less. */
function tone(S, c, strong = false) {
  const t = strong ? S.tint * 0.3 : S.tint;
  // the outline: its own colour, or the bar's colour clearly darker
  return { fill: t > 0 ? mixHex(c, '#ffffff', t) : c, stroke: S.outline ? S.outlineColor || mixHex(c, '#000000', strong ? 0.45 : 0.35) : null, sw: S.outlineWidth };
}

/** Fill and outline of a part of a whole (a donut slice, a split-bar segment) of colour `c`. */
function partTone(S, c) {
  return { fill: S.partTint > 0 ? mixHex(c, '#ffffff', S.partTint) : c, stroke: S.partOutline ? S.outlineColor || mixHex(c, '#000000', 0.35) : null, sw: S.outlineWidth };
}

function rect(x, y, w, h, r, t, extra = '') {
  if (!(w > 0 && h > 0)) return '';
  return `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(w)}" height="${r2(h)}" rx="${r2(Math.min(r, w / 2, h / 2))}" fill="${t.fill}"${t.stroke ? ` stroke="${t.stroke}" stroke-width="${t.sw || 1}"` : ''}${t.opacity != null ? ` fill-opacity="${t.opacity}"` : ''}${extra}/>`;
}

/**
 * A fill fading from colour `c` at the data end to a pale tint at the base
 * (`dir`: where the data end is). Returns the gradient's defs and its url.
 */
function fade(c, dir, pale = 0.8) {
  if (!/^#[0-9a-f]{6}$/i.test(c || '')) return { defs: '', fill: c };
  const id = `qf${c.slice(1)}${dir[0]}${Math.round(pale * 100)}`;
  const [x1, y1, x2, y2] = dir === 'up' ? [0, 0, 0, 1] : dir === 'down' ? [0, 1, 0, 0] : dir === 'right' ? [1, 0, 0, 0] : [0, 0, 1, 0];
  return {
    defs: `<defs><linearGradient id="${id}" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"><stop offset="0" stop-color="${c}"/><stop offset="1" stop-color="${mixHex(c, '#ffffff', pale)}"/></linearGradient></defs>`,
    fill: `url(#${id})`,
  };
}

/** A bar: rounded all round, or only at the data end. */
function bar(S, x, y, w, h, dir, t) {
  if (!(w > 0 && h > 0)) return '';
  if (S.barFill === 'gradient' && t.fill && t.fill[0] === '#') {
    const g = fade(t.fill, dir);
    return g.defs + bar({ ...S, barFill: 'flat' }, x, y, w, h, dir, { ...t, fill: g.fill });
  }
  if (S.roundAll) return rect(x, y, w, h, S.radius, t);
  const r = Math.max(0, Math.min(S.radius, w / 2, h / 2));
  let d;
  if (dir === 'up') d = `M${r2(x)},${r2(y + h)}V${r2(y + r)}Q${r2(x)},${r2(y)} ${r2(x + r)},${r2(y)}H${r2(x + w - r)}Q${r2(x + w)},${r2(y)} ${r2(x + w)},${r2(y + r)}V${r2(y + h)}Z`;
  else if (dir === 'down') d = `M${r2(x)},${r2(y)}V${r2(y + h - r)}Q${r2(x)},${r2(y + h)} ${r2(x + r)},${r2(y + h)}H${r2(x + w - r)}Q${r2(x + w)},${r2(y + h)} ${r2(x + w)},${r2(y + h - r)}V${r2(y)}Z`;
  else if (dir === 'right') d = `M${r2(x)},${r2(y)}H${r2(x + w - r)}Q${r2(x + w)},${r2(y)} ${r2(x + w)},${r2(y + r)}V${r2(y + h - r)}Q${r2(x + w)},${r2(y + h)} ${r2(x + w - r)},${r2(y + h)}H${r2(x)}Z`;
  else d = `M${r2(x + w)},${r2(y)}H${r2(x + r)}Q${r2(x)},${r2(y)} ${r2(x)},${r2(y + r)}V${r2(y + h - r)}Q${r2(x)},${r2(y + h)} ${r2(x + r)},${r2(y + h)}H${r2(x + w)}Z`;
  return `<path d="${d}" fill="${t.fill}"${t.stroke ? ` stroke="${t.stroke}" stroke-width="${t.sw || 1}"` : ''}/>`;
}

const UNIT = { thousands: 1e3, millions: 1e6, billions: 1e9 };

/** Label text for a value under the settings. */
function labeler(st, S, fmt0, { full = false } = {}) {
  const fmt = S.format && S.format !== 'auto' ? S.format : fmt0 || 'number';
  const cur = st.doc.theme.currency;
  const u = UNIT[S.units];
  return (v, signed = S.signed) => {
    if (v == null || Number.isNaN(v)) return '';
    if (fmt === 'percent') return fmtNum(v, 'percent', { signed, decimals: S.decimals ?? (full ? 2 : Math.abs(v) >= 0.1 ? 0 : 1) }) + S.suffix;
    if (u && !full) return fmtNum(v / u, 'number', { signed, decimals: S.decimals ?? (Math.abs(v / u) >= 100 ? 0 : 1) }) + S.suffix;
    return fmtNum(v, fmt === 'number' && !full ? 'number' : fmt, { compact: S.compact !== 'off', currency: cur, signed, decimals: S.decimals }) + S.suffix;
  };
}

/**
 * Data labels' own number format: units, decimals and suffix can differ from
 * the axis ("as values" when not set; units 'none' = the full number).
 */
function labelLabeler(st, S, fmt0, opts) {
  const u = S.labelUnits;
  const L = { ...S,
    units: u === 'none' ? '' : u || S.units,
    decimals: S.labelDecimals ?? S.decimals,
    suffix: S.labelSuffix ?? S.suffix };
  return labeler(st, L, fmt0, u === 'none' ? { ...opts, full: true } : opts);
}

/** A data label's colour: 'series' (its series' or slice's own), 'sign' (up / down), 'ink', 'muted', or `auto`. */
function labelColorOf(S, th, v, seriesColor, auto) {
  switch (S.labelColor) {
    case 'series': return seriesColor || auto;
    case 'sign': return v >= 0 ? S.colorUp : S.colorDown;
    case 'ink': return th.ink;
    case 'muted': return th.muted;
    default: return auto;
  }
}

/**
 * A series' own settings (Design → Series): `axis` ('left' | 'right'), `dash`, `width`, `markers`,
 * `labels` ('none' | 'all') and `name` (in the legend). Kept by the value's id, or by name for series
 * that come from a field's values.
 */
export function seriesKey(w, sh, i) {
  return sh.named ? `n:${sh.series[i]?.name}` : w.binding?.measures?.[i]?.id || `i${i}`;
}
function seriesOpts(w, S, sh, i) {
  return (S.seriesOpts && typeof S.seriesOpts === 'object' && S.seriesOpts[seriesKey(w, sh, i)]) || {};
}

/**
 * Settings a series can have of its own (Design → Colours, Marks, Highlights and Data labels, a part per series),
 * over the chart's: kept in `seriesOpts` by the chart setting's name (a line's `width` and `dash` too, as before).
 */
export const SERIES_LABEL_KEYS = ['labels', 'labelPos', 'labelColor', 'labelBold', 'labelBox', 'labelUnits', 'labelDecimals', 'labelSuffix'];
export const SERIES_KEYS = [...SERIES_LABEL_KEYS, 'colorMode', 'colorUp', 'colorDown', 'tint', 'outline', 'outlineWidth', 'outlineColor', 'barFill',
  'lineWidth', 'lineStyle', 'curve', 'markers', 'markerSize', 'markerFill', 'areaFill', 'areaOpacity', 'extremes', 'extremesRule', 'bands', 'callout'];
function seriesLabelS(S, o) {
  return seriesS(S, o);
}
/** The chart's settings with a series' own over them (its colours as the theme has them). */
function seriesS(S, o, th = TH_CUR) {
  const own = SERIES_KEYS.filter((k) => o[k] != null && (o[k] !== '' || k === 'labelSuffix'));
  const alias = { ...(o.width != null ? { lineWidth: o.width } : {}), ...(o.dash != null ? { lineStyle: o.dash } : {}) };
  if (!own.length && !Object.keys(alias).length) return S;
  const named = (v) => (typeof v === 'string' && v[0] === '$' && th ? themeColor(th, v) : v);
  return { ...S, ...alias, ...Object.fromEntries(own.map((k) => [k, named(o[k])])) };
}
let TH_CUR = null;

/** A series' or category's colour: its own (by name), the widget palette, then the theme's. */
function seriesCol(st, w, S, i, name, key = name) {
  const th = st.doc.theme;
  if (key != null && S.colors && typeof S.colors === 'object' && S.colors[key]) return themeColor(th, S.colors[key]);
  if (Array.isArray(S.palette) && S.palette[i]) return themeColor(th, S.palette[i]);
  return seriesColor(st, w, i, name);
}

function pill(st, S, cx, cy, text, size) {
  const th = st.doc.theme;
  const wd = tw(text, size, 700) + size * 1.6;
  const h = size * 2;
  return `<rect x="${r2(cx - wd / 2)}" y="${r2(cy - h / 2)}" width="${r2(wd)}" height="${r2(h)}" rx="${r2(h / 2)}" fill="${S.calloutBg || th.ink}"/>`
    + T(cx, cy + size * 0.36, text, { size, weight: 700, fill: S.calloutColor || th.page_bg, anchor: 'middle' });
}

/**
 * A value label: plain text, or framed (`S.labelBox` 'box': a white box with a
 * hairline; 'pill': a rounded chip tinted with the mark's colour).
 */
function valueLabel(st, S, x, y, text, o, tintOf) {
  const th = st.doc.theme;
  if (S.labelBox !== 'box' && S.labelBox !== 'pill') return T(x, y, text, o);
  const size = o.size;
  const wd = tw(text, size, o.weight || 500) + size * (S.labelBox === 'pill' ? 1.3 : 0.9);
  const h = size * 1.55;
  const left = o.anchor === 'middle' ? x - wd / 2 : o.anchor === 'end' ? x - wd : x - size * 0.45;
  const top = y - size * 1.08;
  const box = S.labelBox === 'pill'
    ? `<rect x="${r2(left)}" y="${r2(top)}" width="${r2(wd)}" height="${r2(h)}" rx="${r2(h / 2)}" fill="${mixHex(tintOf || th.ink, '#ffffff', 0.84)}"/>`
    : `<rect x="${r2(left)}" y="${r2(top)}" width="${r2(wd)}" height="${r2(h)}" rx="1" fill="#ffffff" stroke="${th.ink}" stroke-width="0.6"/>`;
  return box + T(left + wd / 2, y, text, { ...o, anchor: 'middle', fill: S.labelBox === 'box' ? th.ink : o.fill });
}

const dashOf = (s) => (s === 'dashed' ? ' stroke-dasharray="5 3"' : s === 'dotted' ? ' stroke-dasharray="1 3"' : '');

function pathOf(pts, curve) {
  if (curve !== 'smooth' || pts.length < 3) return pts.map((p, j) => `${j ? 'L' : 'M'}${r2(p[0])},${r2(p[1])}`).join('');
  let d = `M${r2(pts[0][0])},${r2(pts[0][1])}`;
  for (let i = 0; i < pts.length - 1; i++) {
    const p0 = pts[i - 1] || pts[i], p1 = pts[i], p2 = pts[i + 1], p3 = pts[i + 2] || p2;
    const c1 = [p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6];
    const c2 = [p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6];
    d += `C${r2(c1[0])},${r2(c1[1])} ${r2(c2[0])},${r2(c2[1])} ${r2(p2[0])},${r2(p2[1])}`;
  }
  return d;
}

/** Legend: in the title row (top-right), at the top-left of the body, or at the bottom. */
function legend(st, S, vm, b, items0) {
  let items = items0;
  const th = st.doc.theme;
  const pos = S.legend === 'auto' ? 'top-left' : S.legend;
  if (pos === 'none' || !items.length) return { svg: '', top: 0, bottom: 0 };
  const lo = pf('legend', 0.5, { fill: th.muted });
  const size = lo.size;
  // swatches: their shape and size (a line series keeps a line)
  const ss = clamp(+S.legendSize || 8, 3, 16), shapeOf = S.legendSwatch || 'rounded';
  const sw = (it, x, y) => {
    const cy = y - size * 0.35;
    if (it.line || shapeOf === 'line') {
      const col = it.stroke || it.fill;
      return `<line x1="${r2(x)}" x2="${r2(x + Math.max(9, ss))}" y1="${r2(cy)}" y2="${r2(cy)}" stroke="${col}" stroke-width="${r2(it.line ? 2.4 : Math.max(1.6, ss * 0.3))}" stroke-linecap="round"${dashOf(it.dash)}/>`;
    }
    if (shapeOf === 'circle') return `<circle cx="${r2(x + ss / 2)}" cy="${r2(cy)}" r="${r2(ss / 2)}" fill="${it.fill}"${it.stroke ? ` stroke="${it.stroke}" stroke-width="${it.sw || 0.75}"` : ''}/>`;
    return rect(x, cy - ss / 2, ss, ss, shapeOf === 'square' ? 0 : Math.min(2, ss / 4), it);
  };
  const gapS = Math.max(12, ss + 4);
  // their order: as the data has them, reversed, or by name
  items = S.legendOrder === 'reverse' ? [...items].reverse() : S.legendOrder === 'name' ? [...items].sort((a, z) => String(a.label).localeCompare(String(z.label))) : items;
  let out = '';
  if (pos === 'top-right' && vm) {
    let x = vm.frame.x + vm.frame.w - (vm.card ? 10 : 0);
    const y = vm.title ? vm.title_y : b.y + size;
    for (const it of [...items].reverse()) {
      x -= pw(lo, it.label);
      out += PT(x, y, it.label, lo);
      x -= gapS;
      out += sw(it, x, y);
      x -= 12;
    }
    return { svg: out, top: vm.title ? 0 : size + 8, bottom: 0 };
  }
  // flowing rows
  let x = b.x, y = pos === 'bottom' ? b.y + b.h - 2 : b.y + size + 2;
  const rows = [];
  let row = [];
  for (const it of items) {
    const iw = gapS + pw(lo, it.label) + 12;
    if (x + iw > b.x + b.w && row.length) { rows.push(row); row = []; x = b.x; }
    row.push({ it, x });
    x += iw;
  }
  rows.push(row);
  if (pos === 'bottom') y -= (rows.length - 1) * (size + 5);
  rows.forEach((r, ri) => r.forEach(({ it, x: lx }) => {
    const ly = y + ri * (size + 5);
    out += sw(it, lx, ly);
    out += PT(lx + gapS, ly, it.label, lo);
  }));
  const hgt = rows.length * (size + 5) + 4;
  return { svg: out, top: pos === 'bottom' ? 0 : hgt, bottom: pos === 'bottom' ? hgt : 0 };
}

/** Text kept legible over marks: a thin outline in the page colour under it. */
const halo = (svg, th) => svg.replace('<text ', `<text stroke="${th.card_bg || th.page_bg || '#ffffff'}" stroke-width="2.4" stroke-linejoin="round" paint-order="stroke" `);

/** A category's label: a date category written the chart's way (`xDate`), else as the data has it. */
function catText(S, sh, c) {
  const cat = sh.cats[c], key = sh.keys?.[c];
  if (!key || !S.xDate || S.xDate === 'auto') return cat;
  const f = S.xDate, yy = (y) => String(y).slice(2);
  let m = /^(\d{4})-(\d{2})$/.exec(key);
  if (m) {
    const [y, mo] = [+m[1], +m[2]], mon = monthShort(mo), mm = String(mo).padStart(2, '0');
    return { m: mon, my: `${mon} ${yy(y)}`, mY: `${mon} ${y}`, n: `${mm}.${yy(y)}`, nY: `${mm}.${y}`, y: String(y) }[f] ?? cat;
  }
  m = /^(\d{4})-Q(\d)$/.exec(key);
  if (m) return { m: `Q${m[2]}`, my: `Q${m[2]} ${yy(m[1])}`, mY: `Q${m[2]} ${m[1]}`, n: `Q${m[2]} ${yy(m[1])}`, nY: `Q${m[2]} ${m[1]}`, y: m[1] }[f] ?? cat;
  m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key);
  if (m) {
    const [y, mo, d] = [m[1], +m[2], +m[3]], mon = monthShort(mo), dd = String(d).padStart(2, '0'), mm = String(mo).padStart(2, '0');
    return { m: `${d} ${mon}`, my: `${d} ${mon}`, mY: `${d} ${mon} ${y}`, n: `${dd}.${mm}`, nY: `${dd}.${mm}.${y}`, y }[f] ?? cat;
  }
  return cat;
}

/**
 * The category axis: the labels' text, and the height they take turned by `xRotate` (0, 45 or 90°); which
 * show is decided as they are drawn (every one that fits, or every Nth).
 */
function catAxis(S, sh, band, ao) {
  const rot = [45, 90].includes(+S.xRotate) ? +S.xRotate : 0, every = Math.max(0, Math.round(+S.xEvery || 0));
  // turned labels may be longer: they have the height to go down into
  const room = rot ? 90 : Math.max(band * 1.6, 30);
  const texts = sh.cats.map((_, c) => ellipsize(up(ao, catText(S, sh, c)), room, ao.size, ao.weight || 400, ao.family || curFont().family, ao.italic ?? curFont().italic, ao.tracking || 0));
  const longest = Math.max(0, ...texts.map((t) => pw(ao, t)));
  const height = rot ? longest * Math.sin(rot * Math.PI / 180) + ao.size * Math.cos(rot * Math.PI / 180) + 2 : ao.size;
  return { rot, every, texts, height };
}

const gridDash = (S) => (S.gridStyle === 'dashed' ? ' stroke-dasharray="3 2.5"' : S.gridStyle === 'dotted' ? ' stroke-dasharray="0.1 2.4" stroke-linecap="round"' : '');
const gridInk = (S, th) => S.gridColor || (S.gridStyle === 'dotted' ? mixHex(th.muted, th.page_bg, 0.3) : th.grid);
const gridW = (S) => S.gridWidth || (S.gridStyle === 'dotted' ? 1.1 : 0.6);

/**
 * Vertical gridlines (when the grid runs that way): between the categories for bars, so they
 * part the bars instead of crossing them; through the points for lines.
 */
function gridV(st, S, xs, n, y0, y1, { between = false } = {}) {
  if (!S.grid || (S.gridDir !== 'v' && S.gridDir !== 'both')) return '';
  const th = st.doc.theme;
  const at = between ? Array.from({ length: Math.max(0, n - 1) }, (_, c) => (xs(c) + xs(c + 1)) / 2) : Array.from({ length: n }, (_, c) => xs(c));
  let out = '';
  for (const x of at) out += `<line x1="${r2(x)}" x2="${r2(x)}" y1="${r2(y0)}" y2="${r2(y1)}" stroke="${gridInk(S, th)}" stroke-width="${gridW(S)}"${gridDash(S)}/>`;
  return out;
}

function axisY(st, S, ticks, lab, x0, plotW, scale) {
  const th = st.doc.theme;
  let out = '';
  const horiz = S.gridDir !== 'v';
  for (const t of ticks) {
    const y = scale(t);
    const z = t === 0 && S.zeroLine;
    const dash = z ? '' : gridDash(S);
    if ((S.grid && horiz) || z) out += `<line x1="${r2(x0)}" x2="${r2(x0 + plotW)}" y1="${r2(y)}" y2="${r2(y)}" stroke="${z ? S.zeroColor || mixHex(th.muted, th.page_bg, 0.45) : gridInk(S, th)}" stroke-width="${z ? 1 : gridW(S)}"${dash}/>`;
    if (S.axis) out += PT(x0 - 5, y + 2.6, lab(t), pf('axis', -0.5, { fill: S.axisColor || th.muted, anchor: 'end' }));
  }
  return out;
}

function scaleY(S, lo, hi, n) {
  const a = S.yMin ?? lo, z = S.yMax ?? hi;
  // a fixed step: a tick every so much, from a round start to a round end
  if (S.yStep > 0 && (z - a) / S.yStep <= 40) {
    const st = S.yStep, t0 = S.yMin ?? Math.floor(a / st + 1e-9) * st, t1 = S.yMax ?? Math.ceil(z / st - 1e-9) * st;
    const ticks = [];
    for (let v = t0; v <= t1 + st * 1e-6; v += st) ticks.push(Math.round(v / st) * st);
    if (ticks.length >= 2) return { ticks, tlo: t0, thi: t1 };
  }
  const ticks = niceTicks(a, z, Math.max(2, Math.min(8, S.ticks || n)));
  return { ticks, tlo: S.yMin ?? ticks[0], thi: S.yMax ?? ticks[ticks.length - 1] };
}

// ---------------------------------------------------------------- columns, combos and lines

export function columns(st, w, b, data, vm, { allLines = false } = {}) {
  const S = resolve(st, w);
  const th = st.doc.theme;
  const sh = shape(data);
  const n = sh.cats.length;
  if (!n) return message(st, b, 'No data for this period.');
  const fmt = sh.series[0]?.format || 'number';
  const lab = labeler(st, S, fmt);
  const isLine = (i) => allLines || (!sh.named && w.binding?.measures?.[i]?.chart === 'line');
  const bars = sh.series.map((s, i) => ({ s, i })).filter(({ i }) => !isLine(i));
  const lines = sh.series.map((s, i) => ({ s, i })).filter(({ i }) => isLine(i));
  const stacked = S.stacked && bars.length > 1;
  // each series' own settings: its axis and legend name (Series), and its colours, marks, highlights and labels
  // (a part each in Colours, Marks, Highlights and Data labels) over the chart's
  TH_CUR = th;
  const so = (i) => seriesOpts(w, S, sh, i);
  const Ss = (i) => seriesS(S, so(i), th);
  const bySignOf = (i) => { const m = Ss(i).colorMode; return m === 'sign' || (m === 'signOne' && bars.length === 1); };
  const bySign = bars.length > 0 && bars.every(({ i }) => bySignOf(i));
  // best & worst, and a callout: the chart's for a lone series (as before), else only what a series sets itself
  const extOf = (i) => !stacked && !isLine(i) && n >= 3 && !!(so(i).extremes ?? (bars.length === 1 ? S.extremes : false));
  const callOf = (i) => so(i).callout ?? (isLine(i) ? (lines.length === 1 && S.callout === 'last' ? 'last' : 'none') : (bars.length === 1 && S.callout === 'max' ? 'max' : 'none'));
  const callouts = sh.series.some((_, i) => callOf(i) !== 'none');
  // a right axis: series put on it get their own scale (stacked columns stay on the left)
  const onRight = (i) => sh.series.length > 1 && so(i).axis === 'right' && !(stacked && !isLine(i));
  const anyRight = sh.series.some((_, i) => onRight(i));
  const nameOf = (i, x) => so(i).name || x.name;
  const colOf = (i, s) => seriesCol(st, w, S, i, sh.named ? s.name : null, s.name);
  // a value's own colour may be a theme token ("$ink"), as in the theme's design views
  const lineCol = (i, s) => themeColor(th, w.binding?.measures?.[i]?.color || S.colors?.[s.name] || S.lineColor) || (lines.length === 1 && bars.length ? th.ink : colOf(i, s));
  // legend: with a right axis, each series says which axis it reads against (LHS / RHS)
  const axisTag = (i) => (anyRight ? ` (${word(onRight(i) ? 'right' : 'left')})` : '');
  const items = [];
  // a lone column series by sign: its up / down colours, named so (with a right axis: its name and axis)
  if (bars.length && bySign && bars.length === 1 && !anyRight) {
    const S0 = Ss(bars[0].i);
    items.push({ label: S.legendUp || word('up'), ...tone(S0, S0.colorUp) });
    if (bars.some(({ s }) => s.values.some((v) => v < 0))) items.push({ label: S.legendDown || word('down'), ...tone(S0, S0.colorDown) });
  } else bars.forEach(({ s, i }) => items.push({ label: nameOf(i, s) + axisTag(i), ...tone(Ss(i), bySignOf(i) ? Ss(i).colorUp : colOf(i, s)) }));
  lines.forEach(({ s, i }) => items.push({ label: nameOf(i, s) + axisTag(i), line: true, stroke: lineCol(i, s), dash: Ss(i).lineStyle }));
  const showLegend = items.length > 1 || (lines.length && bars.length);
  // with a right axis the legend says which axis is whose: it shows even where the design view hides it (a
  // widget that hides it itself keeps it hidden)
  // (the widget as stored: `w` here has its design view merged in)
  const raw = st.doc.pages.flatMap((p) => p.widgets).find((x) => x.id === String(w.id).split('~')[0]);
  const ownLegend = raw?.opts?.viz?.legend;
  const LG = anyRight && S.legend === 'none' && ownLegend !== 'none' ? { ...S, legend: 'auto' } : S;
  const lg = showLegend ? legend(st, LG, vm, b, items) : { svg: '', top: 0, bottom: 0 };
  // extent
  let lo = 0, hi = 0, loR = 0, hiR = 0;
  for (let c = 0; c < n; c++) {
    if (stacked) {
      let p = 0, q = 0;
      for (const { s } of bars) { const v = s.values[c] || 0; if (v >= 0) p += v; else q += v; }
      hi = Math.max(hi, p); lo = Math.min(lo, q);
    } else for (const { s, i } of bars) { const v = s.values[c]; if (v != null) { if (onRight(i)) { hiR = Math.max(hiR, v); loR = Math.min(loR, v); } else { hi = Math.max(hi, v); lo = Math.min(lo, v); } } }
    for (const { s, i } of lines) { const v = s.values[c]; if (v != null) { if (onRight(i)) { hiR = Math.max(hiR, v); loR = Math.min(loR, v); } else { hi = Math.max(hi, v); lo = Math.min(lo, v); } } }
  }
  // a reference line and a band are on the left scale: it reaches them
  const firstVals = (bars[0] || lines[0])?.s.values.filter((v) => v != null) || [];
  const sorted = [...firstVals].sort((a, z) => a - z), mid = (sorted.length - 1) / 2;
  const refV = S.refLine === 'value' ? (S.refValue == null || S.refValue === '' ? null : +S.refValue)
    : !firstVals.length ? null
      : { average: firstVals.reduce((a, v) => a + v, 0) / firstVals.length, median: (sorted[Math.floor(mid)] + sorted[Math.ceil(mid)]) / 2,
        max: sorted[sorted.length - 1], min: sorted[0] }[S.refLine] ?? null;
  const bandOn = S.bandFrom != null && S.bandFrom !== '' && S.bandTo != null && S.bandTo !== '';
  for (const v of [refV, ...(bandOn ? [+S.bandFrom, +S.bandTo] : [])]) if (v != null && Number.isFinite(v)) { hi = Math.max(hi, v); lo = Math.min(lo, v); }
  const extremes = bars.some(({ i }) => extOf(i));
  // labels drawn: the chart's, or any series' own (room is kept above the marks for them)
  const labelsOn = S.labels === 'all' || (S.labels === 'auto' && (bars.length === 1 && n <= 14)) || S.labels === 'last' || S.labels === 'extremes'
    || sh.series.some((_, i) => ['all', 'last', 'extremes'].includes(so(i).labels));
  const { ticks, tlo, thi } = scaleY(S, lo < 0 ? lo * 1.15 : lo, hi * (labelsOn ? 1.1 : 1.02), 4);
  const axisLab = (t) => (fmt === 'percent' && S.format === 'auto' ? fmtNum(t, 'percent', { signed: S.signedTicks && t !== 0, decimals: 0 }) : lab(t, false));
  const axisW = S.axis ? Math.max(...ticks.map((t) => pw(pf('axis', -0.5), axisLab(t)))) + 7 : 2;
  // the right axis: its own scale and number format (the first series on it)
  const rFirst = sh.series.findIndex((_, i) => onRight(i));
  const fmtR = rFirst >= 0 ? sh.series[rFirst].format || 'number' : fmt;
  const SR = { ...S, yMin: S.y2Min, yMax: S.y2Max, ticks: S.y2Ticks || S.ticks, yStep: S.y2Step };
  const showR = S.y2Axis !== false;
  const R = anyRight ? scaleY(SR, loR < 0 ? loR * 1.15 : loR, hiR * (labelsOn ? 1.1 : 1.02), 4) : null;
  const labR = labeler(st, { ...S, units: S.y2Units ?? S.units }, fmtR);
  const axisLabR = (t) => (fmtR === 'percent' ? fmtNum(t, 'percent', { signed: (S.y2Signed ?? S.signedTicks) && t !== 0, decimals: 0 }) : labR(t, false));
  const axisWR = R && showR ? Math.max(...R.ticks.map((t) => pw(pf('axis', -0.5), axisLabR(t)))) + 7 : 0;
  const bandsOn = bars.some(({ i }) => extOf(i) && Ss(i).bands);
  const x0 = b.x + axisW, plotW = Math.max(10, b.w - axisW - axisWR - (lines.some(({ i }) => callOf(i) === 'last') && !anyRight ? 8 : 2));
  const cax = catAxis(S, sh, plotW / n, pf('axis', 0.5, { fill: th.muted, anchor: 'middle' }));
  const yb = b.y + b.h - lg.bottom - (S.xLabels ? cax.height + 7 : 3);
  const yt = b.y + lg.top + (bandsOn ? CS(0.5) + 12 : labelsOn ? pf('labels', 0.5).size + 4 : 4) + (callouts ? CS(1) + 6 : 0);
  const scaleL = (v) => yb - ((v - tlo) / (thi - tlo || 1)) * (yb - yt);
  note(w, 'yMin', tlo); note(w, 'yMax', thi); note(w, 'gridWidth', gridW(S));
  if (R) { note(w, 'y2Min', R.tlo); note(w, 'y2Max', R.thi); }
  const scaleR = R ? (v) => yb - ((v - R.tlo) / (R.thi - R.tlo || 1)) * (yb - yt) : scaleL;
  const scale = scaleL;
  // a series' own scale and zero line
  const scaleOf = (i) => (onRight(i) ? scaleR : scaleL);
  const zeroOf = (i) => (onRight(i) && R ? scaleR(Math.max(R.tlo, Math.min(R.thi, 0))) : scaleL(Math.max(tlo, Math.min(thi, 0))));
  const band = plotW / n;
  const xs = (c) => x0 + band * c + band / 2;
  let out = P('legend', lg.svg);
  // best / worst: per series that marks them; background bands for the first that asks for them
  const ext = new Map();
  let banded = false;
  for (const { s, i } of bars) {
    if (!extOf(i)) continue;
    const Si = Ss(i);
    const bw0 = pickExtremes(s.values, Si.extremesRule);
    ext.set(i, bw0);
    if (Si.bands && !banded) {
      banded = true;
      for (const [c, good] of [[bw0.best, true], [bw0.worst, false]]) {
        if (c < 0) continue;
        const col = good ? Si.colorUp : Si.colorDown;
        out += P('highlights', rect(xs(c) - band * 0.4, yt - 8, band * 0.8, yb - yt + 20, 5, { fill: col, opacity: 0.08 }));
        out += P('highlights', T(xs(c), yt - 13, (good ? S.bestLabel : S.worstLabel) || word(good ? 'best' : 'worst'), { size: CS(0.5), weight: 700, fill: col, anchor: 'middle' }));
      }
    }
  }
  // the band behind the marks
  if (bandOn) {
    const [ba, bb] = [scaleL(Math.max(+S.bandFrom, +S.bandTo)), scaleL(Math.min(+S.bandFrom, +S.bandTo))];
    const bc = themeColor(th, S.bandColor) || th.good || th.accent;
    out += P('highlights', `<rect x="${r2(x0)}" y="${r2(ba)}" width="${r2(plotW)}" height="${r2(Math.max(0.5, bb - ba))}" fill="${bc}" fill-opacity="0.1"/>`);
  }
  out += P('axis', axisY(st, S, ticks, axisLab, x0, plotW, scale));
  out += P('axis', gridV(st, S, xs, n, yt, yb, { between: bars.length > 0 }));
  if (R && showR) {
    // its labels in its series' colour (one series on it), so it's clear which axis a line reads against
    // its labels as the left axis's (the legend says which series is on which), unless set
    let ro = '';
    for (const t of R.ticks) ro += PT(x0 + plotW + 5, scaleR(t) + 2.6, axisLabR(t), pf('axis', -0.5, { fill: S.y2Color || S.axisColor || th.muted, anchor: 'start' }));
    out += P('axis', ro);
  }
  const zero = scale(Math.max(tlo, Math.min(thi, 0)));
  const k = stacked ? 1 : Math.max(1, bars.length);
  const groupW = Math.min(band * S.barWidth, S.maxBar * k);
  const bw = groupW / k;
  const labV = labelLabeler(st, S, fmt);
  // the data labels drawn so far, as boxes: a line's labels move or give way where they would land on one
  const taken = [];
  const boxAt = (x, base, text, font, anchor, framed) => {
    const wd = pw(font, text) + (framed ? font.size * 1.1 : 0), sz = font.size;
    const l = anchor === 'middle' ? x - wd / 2 : anchor === 'end' ? x - wd : x;
    return { l, r: l + wd, t: base - sz * 0.95 - (framed ? 2 : 0), b: base + sz * 0.25 + (framed ? 2 : 0) };
  };
  const clash = (q) => taken.some((o) => q.l < o.r + 1 && o.l < q.r + 1 && q.t < o.b + 1 && o.t < q.b + 1);
  // the lines' points: a column's label steps clear of them (above the point, or under it)
  const dots = lines.flatMap(({ s, i }) => s.values.map((v, c) => (v == null ? null : { x: xs(c), y: scaleOf(i)(v) })).filter(Boolean));
  const onDot = (q) => dots.find((d) => d.x > q.l - 4 && d.x < q.r + 4 && d.y > q.t - 5 && d.y < q.b + 5);
  // which of a series' values get a label, by its own Show (else the chart's)
  const showLabel = (c, v, vals, L = S, lab = labV) => {
    const on = L.labels === 'all' || (L.labels === 'auto' && bars.length === 1 && n <= 14) || L.labels === 'last' || L.labels === 'extremes';
    if (L.labels === 'none' || !on) return false;
    if (L.labels === 'last') return c === n - 1;
    if (L.labels === 'extremes') return v === Math.max(...vals.filter((x) => x != null)) || v === Math.min(...vals.filter((x) => x != null));
    return pw(pf('labels', -0.5), lab(v)) < band + 8;
  };
  if (stacked) {
    for (let c = 0; c < n; c++) {
      let pos = 0, neg = 0;
      bars.forEach(({ s, i }) => {
        const v = s.values[c] || 0;
        if (!v) return;
        const from = v >= 0 ? pos : neg;
        const to = from + v;
        const ya = scale(Math.max(from, to)), yb2 = scale(Math.min(from, to));
        out += P(`series:${s.name}`, rect(xs(c) - bw / 2, ya + (from ? 1 : 0), bw, yb2 - ya - (from ? 1 : 0), S.roundAll ? S.radius : 0, tone(Ss(i), bySignOf(i) ? (v >= 0 ? Ss(i).colorUp : Ss(i).colorDown) : colOf(i, s))));
        if (v >= 0) pos = to; else neg = to;
      });
      if (labelsOn && S.labels !== 'none' && pw(pf('labels', -0.5), labV(pos)) < band) out += P('labels', PT(xs(c), scale(pos) - 4, labV(pos), pf('labels', -0.5, { fill: labelColorOf(S, th, pos, th.ink, th.muted), anchor: 'middle' })));
    }
  } else {
    bars.forEach(({ s, i }, bi) => {
      s.values.forEach((v, c) => {
        if (v == null) return;
        const x = xs(c) - groupW / 2 + bw * bi + (k > 1 ? 1 : 0);
        const sc = scaleOf(i), z0 = zeroOf(i);
        const ya = Math.min(z0, sc(v)), yb2 = Math.max(z0, sc(v));
        const Si = Ss(i);
        const base = bySignOf(i) ? (v >= 0 ? Si.colorUp : Si.colorDown) : colOf(i, s);
        const e = ext.get(i);
        const strong = !!e && (c === e.best || (c === e.worst && v < 0));
        out += P(`series:${s.name}`, bar(Si, x, ya, bw - (k > 1 ? 2 : 0), Math.max(1.2, yb2 - ya), v >= 0 ? 'up' : 'down', tone(Si, base, strong)));
        // this series' own label settings over the chart's
        const L = seriesLabelS(S, so(i)), labL = L === S ? labV : labelLabeler(st, L, fmt);
        if (showLabel(c, v, s.values, L, labL)) {
          const pos = L.labelPos === 'auto' ? (lines.length && lo < 0 ? (v >= 0 ? 'below' : 'outside') : 'outside') : L.labelPos;
          const ls = pf('labels', 0.5).size;
          // next to the axis: on the side away from the bar (under it for a rise, over it for a fall), so the
          // labels run in one row along the axis
          const ly = pos === 'inside' ? (v >= 0 ? ya + ls - 0.5 + 3 : yb2 - 4) : pos === 'below' ? (v >= 0 ? z0 + ls + 6 : z0 - 5) : v >= 0 ? ya - 5 : yb2 + ls + 4;
          const fill = pos === 'inside' && Si.tint < 0.3 ? '#ffffff' : labelColorOf(L, th, v, colOf(i, s), bySignOf(i) ? (v >= 0 ? Si.colorUp : Si.colorDown) : CUR_muted(th));
          if (callOf(i) === 'max' && v === Math.max(...s.values.filter((q) => q != null))) return;
          let lyy = ly - (L.labelBox !== 'none' && pos === 'outside' && v >= 0 ? 2 : 0);
          const ltxt = up(pf('labels', 0.5), labL(v)), framed = L.labelBox !== 'none';
          // a line's point in the way: over it (or under it, where there is no room above)
          const d0 = onDot(boxAt(x + bw / 2, lyy, ltxt, pf('labels', 0.5), 'middle', framed));
          if (d0) {
            const upY = d0.y - 7;
            // no room over the point: the label would land on its own column, hard to read — it's left out
            if (upY - ls <= yt - 2) return;
            lyy = upY;
          }
          taken.push(boxAt(x + bw / 2, lyy, ltxt, pf('labels', 0.5), 'middle', framed));
          out += P('labels', valueLabel(st, L, x + bw / 2, lyy, ltxt, pf('labels', 0.5, { weight: L.labelBold ? 700 : 500, fill, anchor: 'middle' }), base));
        }
      });
    });
  }
  // lines over the bars, on the same axis
  lines.forEach(({ s, i }) => {
    const col = lineCol(i, s);
    const o = so(i), sc = scaleOf(i), Si = Ss(i);
    const lw = Si.lineWidth, mk = Si.markers;
    const pts = s.values.map((v, c) => (v == null ? null : [xs(c), sc(v), v])).filter(Boolean);
    if (!pts.length) return;
    const d = pathOf(pts, Si.curve);
    // the area under: always on an area chart, on a lone line when the theme fills under lines, or where a
    // series asks for one
    const under = w.kind === 'area' || (Si.areaFill !== 'none' && ((lines.length === 1 && !bars.length) || o.areaFill != null));
    if (under) {
      const g = Si.areaFill === 'gradient' ? fade(col, 'up', 1) : null;
      out += P(`series:${s.name}`, (g ? g.defs : '') + `<path d="${d}L${r2(pts[pts.length - 1][0])},${r2(zeroOf(i))}L${r2(pts[0][0])},${r2(zeroOf(i))}Z" fill="${g ? g.fill : col}" fill-opacity="${g ? Math.max(0.3, Si.areaOpacity * 2.5) : Si.areaOpacity}"/>`);
    }
    // guide lines from each point down to the axis (pure line charts: in a combo they would cross the bars)
    if (S.guides && !bars.length) for (const p of pts) out += P(`series:${s.name}`, `<line x1="${r2(p[0])}" x2="${r2(p[0])}" y1="${r2(p[1])}" y2="${r2(yb)}" stroke="${th.grid}" stroke-width="0.75"/>`);
    out += P(`series:${s.name}`, `<path d="${d}" fill="none" stroke="${col}" stroke-width="${lw}" stroke-linejoin="round" stroke-linecap="round"${dashOf(Si.lineStyle)}/>`);
    pts.forEach((p, j) => {
      const last = j === pts.length - 1;
      if (mk === 'all' || (mk === 'last' && last)) {
        const hollow = Si.markerFill === 'hollow' && !last;
        out += P(`series:${s.name}`, `<circle cx="${r2(p[0])}" cy="${r2(p[1])}" r="${r2(last ? Si.markerSize + 0.6 : Si.markerSize)}" fill="${hollow ? th.page_bg : col}" stroke="${hollow ? col : th.card_bg}" stroke-width="${hollow ? 2 : 1.5}"/>`);
      }
      // this series' own label settings over the chart's (a line's labels sit above its points)
      const L = seriesLabelS(S, o), labL = L === S ? labV : labelLabeler(st, L, fmt);
      const ext = L.labels === 'extremes' && (p[2] === Math.max(...pts.map((q) => q[2])) || p[2] === Math.min(...pts.map((q) => q[2])));
      const wantLabel = L.labels === 'none' ? false : L.labels === 'all' || ext || (L.labels === 'auto' && lines.length <= 2 && last) || (L.labels === 'last' && last);
      if (wantLabel && !(last && callOf(i) === 'last') && !(callOf(i) === 'max' && p[2] === Math.max(...pts.map((q) => q[2])))) {
        const below = L.labelPos === 'inside' || L.labelPos === 'below';
        const lf = pf('labels', 0.5), side = last && !bars.length;
        const lx = side ? p[0] + 6 : p[0], anchor = side ? 'start' : 'middle', ltxt = up(lf, labL(p[2]));
        // its place, or the other side of the point where a column's label (or another line's) is in the way;
        // where both are taken, it gives way
        const above = p[1] - 7, under = p[1] + lf.size + 5;
        const tries = side ? [p[1] + 3, above, under] : below ? [under, above] : [above, under];
        const ly = tries.find((y) => !clash(boxAt(lx, y, ltxt, lf, anchor, L.labelBox !== 'none')));
        if (ly == null) return;
        taken.push(boxAt(lx, ly, ltxt, lf, anchor, L.labelBox !== 'none'));
        out += P('labels', valueLabel(st, L, lx, ly, ltxt, pf('labels', 0.5, { weight: o.labelBold != null ? (L.labelBold ? 700 : 500) : 600, fill: labelColorOf(L, th, p[2], col, th.ink), anchor }), col));
      }
    });
    // this series' callout: its last value or its largest, in a pill
    const co = callOf(i);
    if (co !== 'none') {
      const p = co === 'max' ? pts.reduce((a2, q) => (q[2] > a2[2] ? q : a2)) : pts[pts.length - 1];
      const text = labeler(st, S, s.format || fmt, { full: true })(p[2], true);
      const size = CS(1.5);
      const cx = Math.min(p[0], x0 + plotW - tw(text, size, 700) / 2 - size);
      out += P('highlights', pill(st, S, cx, p[1] - size * 1.9, text, size));
    }
  });
  // the reference line over the marks, its label at the right end above it
  if (refV != null && Number.isFinite(refV)) {
    const ry = scaleL(refV), rc = themeColor(th, S.refColor) || th.ink;
    out += P('highlights', `<line x1="${r2(x0)}" x2="${r2(x0 + plotW)}" y1="${r2(ry)}" y2="${r2(ry)}" stroke="${rc}" stroke-width="1"${dashOf(S.refStyle)}/>`);
    const named = { average: word('average'), median: word('median'), max: word('max'), min: word('min') }[S.refLine];
    const text = S.refLabel ? S.refLabel.replace('{value}', lab(refV, false)) : named ? `${named} ${lab(refV, false)}` : lab(refV, false);
    out += P('highlights', halo(T(x0 + 2, ry - 3, text, { size: CS(-0.5), weight: 600, fill: rc }), th));
  }
  // the band's label, over the marks: inside its top edge, at the right (the reference line's is at the left)
  if (bandOn && S.bandLabel) {
    const bc = themeColor(th, S.bandColor) || th.good || th.accent;
    out += P('highlights', halo(T(x0 + plotW - 2, scaleL(Math.max(+S.bandFrom, +S.bandTo)) + CS(-0.5) + 2, S.bandLabel, { size: CS(-0.5), weight: 600, fill: bc, anchor: 'end' }), th));
  }
  // a column series' callout: on its largest (or last) column
  bars.forEach(({ s, i }, bi) => {
    const co = callOf(i);
    if (co === 'none' || stacked) return;
    const vals = s.values;
    const c = co === 'last' ? vals.length - 1 - [...vals].reverse().findIndex((v) => v != null) : vals.indexOf(Math.max(...vals.filter((v) => v != null)));
    if (c < 0 || vals[c] == null) return;
    const x = k > 1 ? xs(c) - groupW / 2 + bw * bi + bw / 2 : xs(c);
    out += P('highlights', pill(st, S, x, scaleOf(i)(vals[c]) - CS(1) * 2.2, labeler(st, S, s.format || fmt, { full: true })(vals[c]), CS(0.5)));
  });
  if (S.xLabels) {
    // every label that fits (or every Nth); turned ones hang from the axis, ending under their category
    let lastRight = -Infinity;
    const ao = pf('axis', 0.5, { fill: th.muted, anchor: cax.rot ? 'end' : 'middle' });
    cax.texts.forEach((t, c) => {
      if (cax.every > 1 && c % cax.every) return;
      if (cax.rot) {
        const gap = ao.size / Math.sin(cax.rot * Math.PI / 180);
        if (cax.every < 2 && xs(c) < lastRight + gap) return;
        out += P('axis', T(xs(c) + (cax.rot === 90 ? ao.size * 0.35 : ao.size * 0.25), yb + 6 + ao.size * 0.6, t, { ...ao, rotate: cax.rot }));
        lastRight = xs(c);
        return;
      }
      const lw2 = pw(ao, t);
      if (cax.every < 2 && xs(c) - lw2 / 2 < lastRight + 4) return;
      out += P('axis', T(xs(c), b.y + b.h - lg.bottom - 3, t, ao));
      lastRight = xs(c) + lw2 / 2;
    });
  }
  return out;
}
const CUR_muted = (th) => th.muted;

// ---------------------------------------------------------------- horizontal bars and ranking

export function hbars(st, w, b, data, vm) {
  const S = resolve(st, w);
  const th = st.doc.theme;
  const sh = shape(data);
  const n = sh.cats.length;
  if (!n) return message(st, b, 'No data for this period.');
  const fmt = sh.series[0]?.format || 'number';
  const lab = labelLabeler(st, S, fmt);
  const bySign = S.colorMode === 'sign';
  const stacked = S.stacked && sh.series.length > 1;
  const items = sh.series.map((s, i) => ({ label: s.name, ...tone(S, seriesCol(st, w, S, i, sh.named ? s.name : null, s.name)) }));
  const lg = sh.series.length > 1 ? legend(st, S, vm, b, items) : { svg: '', top: 0, bottom: 0 };
  let lo = 0, hi = 0;
  for (let c = 0; c < n; c++) {
    if (stacked) { let p = 0; for (const s of sh.series) p += Math.max(0, s.values[c] || 0); hi = Math.max(hi, p); }
    else for (const s of sh.series) { const v = s.values[c] || 0; hi = Math.max(hi, v); lo = Math.min(lo, v); }
  }
  if (S.yMin != null) lo = S.yMin;
  if (S.yMax != null) hi = S.yMax;
  const labelsOn = S.labels !== 'none';
  const top = b.y + lg.top;
  const catO = pf('axis', 0.5, { fill: th.ink, anchor: 'end' });
  const labelW = Math.min(b.w * 0.4, Math.max(...sh.cats.map((c) => pw(catO, c))) + 8);
  const valW = labelsOn ? Math.max(...sh.series.flatMap((s) => s.values.map((v) => pw(pf('labels', 0), lab(v ?? 0))))) + 8 : 4;
  const x0 = b.x + labelW, plotW = Math.max(10, b.w - labelW - valW);
  const bandH = (b.y + b.h - lg.bottom - top) / n;
  const scale = (v) => x0 + ((v - lo) / (hi - lo || 1)) * plotW;
  const zero = scale(0);
  const thick = Math.min(bandH * S.barWidth, S.maxBar);
  let out = P('legend', lg.svg);
  for (let c = 0; c < n; c++) {
    const cy = top + bandH * c + bandH / 2;
    out += P('axis', T(x0 - 6, cy + 2.6, ellipsize(up(catO, sh.cats[c]), labelW - 6, catO.size, catO.weight || 400, catO.family || curFont().family, catO.italic ?? curFont().italic, catO.tracking || 0), catO));
    if (stacked) {
      let acc = 0;
      sh.series.forEach((s, i) => {
        const v = s.values[c] || 0;
        if (v <= 0) return;
        out += P(`series:${s.name}`, rect(scale(acc) + (acc ? 1 : 0), cy - thick / 2, scale(acc + v) - scale(acc) - (acc ? 1 : 0), thick, S.roundAll ? S.radius : 0, tone(S, seriesCol(st, w, S, i, sh.named ? s.name : null, s.name))));
        acc += v;
      });
      if (labelsOn) out += P('labels', PT(scale(acc) + 4, cy + 2.6, lab(acc), pf('labels', 0, { fill: labelColorOf(S, th, acc, th.ink, th.muted) })));
    } else {
      const k = sh.series.length;
      const sub = thick / k;
      sh.series.forEach((s, i) => {
        const v = s.values[c];
        if (v == null) return;
        const y = cy - thick / 2 + sub * i;
        const xa = Math.min(zero, scale(v)), xb = Math.max(zero, scale(v));
        const base = bySign ? (v >= 0 ? S.colorUp : S.colorDown) : seriesCol(st, w, S, i, sh.named ? s.name : null, s.name);
        const hl = S.highlight && sh.cats[c] === S.highlight;
        out += P(`series:${s.name}`, bar(S, xa, y + (k > 1 ? 0.5 : 0), xb - xa, sub - (k > 1 ? 1 : 0), v >= 0 ? 'right' : 'left', tone(S, hl ? th.ink : base, hl)));
        if (labelsOn) out += P('labels', PT(v >= 0 ? xb + 4 : zero + 4, y + sub / 2 + 2.6, lab(v), pf('labels', 0, { weight: bySign ? 600 : 400, fill: labelColorOf(S, th, v, base, bySign ? base : th.muted), anchor: 'start' })));
      });
    }
  }
  out += P('axis', `<line x1="${r2(zero)}" x2="${r2(zero)}" y1="${r2(top)}" y2="${r2(b.y + b.h - lg.bottom)}" stroke="${mixHex(th.muted, th.page_bg, 0.45)}" stroke-width="0.8"/>`);
  return out;
}


// ---------------------------------------------------------------- waterfall

export function waterfall(st, w, b, data, vm) {
  const S = resolve(st, w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0];
  if (!s0 || !sh.cats.length) return message(st, b, 'No data for this period.');
  const fmt = s0.format || 'number';
  const lab = labeler(st, S, fmt);
  let run = 0;
  // a bridge: the first value is a level (where it starts), the rest the changes to it
  const bridge = !!S.bridge && sh.cats.length > 1;
  const steps = sh.cats.map((c, i) => { const v = s0.values[i] || 0; const from = run; run += v; return { name: c, v, from, to: run, total: bridge && i === 0 }; });
  const bars = S.total ? [...steps, { name: S.totalLabel || word('Total'), v: run, from: 0, to: run, total: true }] : steps;
  // best and worst: as on a bar chart — a tinted band with its label above, the bar in its strong tone
  const marks = S.extremes && steps.length >= 3;
  const { best, worst } = marks ? pickExtremes(steps.map((x) => x.v), S.extremesRule) : { best: -1, worst: -1 };
  const items = [{ label: S.legendUp || word('up'), ...tone(S, S.colorUp) }];
  if (steps.some((x) => x.v < 0)) items.push({ label: S.legendDown || word('down'), ...tone(S, S.colorDown) });
  if (S.total) items.push({ label: S.legendTotal || word('total'), ...tone(S, S.colorTotal, true) });
  const lg = legend(st, S, vm, b, items);
  let lo = Math.min(0, ...bars.map((x) => Math.min(x.from, x.to)));
  const hi = Math.max(0, ...bars.map((x) => Math.max(x.from, x.to)));
  if (bridge && S.yMin == null) {
    // the axis starts below the lowest level the changes reach, so small changes beside large levels show
    const levels = bars.flatMap((x) => (x.total ? [x.to] : [x.from, x.to]));
    const low = Math.min(...levels), span = Math.max(1e-9, hi - low);
    lo = Math.max(0, low - span * 0.9);
  }
  const { ticks, tlo, thi } = scaleY(S, lo, hi * (bridge ? 1.03 : 1.12), 3);
  const axisLab = (t) => lab(t, false);
  const axisW = S.axis ? Math.max(...ticks.map((t) => pw(pf('axis', -0.5), axisLab(t)))) + 8 : 2;
  const x0 = b.x + axisW, plotW = Math.max(10, b.w - axisW);
  const yb = b.y + b.h - lg.bottom - (S.xLabels ? pf('axis', 0.5).size + 8 : 3), yt = b.y + lg.top + CS(1.5) * 2.4;
  // levels below the axis's start stop at the axis (a bridge's start and end bars)
  const scale = (v) => Math.min(yb, yb - ((v - tlo) / (thi - tlo || 1)) * (yb - yt));
  let out = P('legend', lg.svg);
  const band = plotW / bars.length;
  if (S.bands) for (const [i, good] of [[best, true], [worst, false]]) {
    if (i < 0) continue;
    const col = good ? S.colorUp : S.colorDown;
    const cx = x0 + band * i + band / 2;
    out += P('highlights', rect(cx - band * 0.4, yt - 8, band * 0.8, yb - yt + 20, 5, { fill: col, opacity: 0.08 }));
    out += P('highlights', T(cx, yt - 13, (good ? S.bestLabel : S.worstLabel) || word(good ? 'best' : 'worst'), { size: CS(0.5), weight: 700, fill: col, anchor: 'middle' }));
  }
  out += P('axis', axisY(st, S, ticks, axisLab, x0, plotW, scale));
  // vertical gridlines between the steps, behind the bars (as on a bar chart)
  out += P('axis', gridV(st, S, (i) => x0 + band * i + band / 2, bars.length, yt, yb, { between: true }));
  const bw = Math.min(band * S.barWidth, S.maxBar * 2);
  const conn = mixHex(th.muted, th.page_bg, 0.35);
  bars.forEach((x, i) => {
    const cx = x0 + band * i + band / 2;
    const y0 = scale(Math.max(x.from, x.to)), y1 = scale(Math.min(x.from, x.to));
    const t = x.total ? tone(S, bridge && i === 0 ? S.colorStart : S.colorTotal, true) : tone(S, x.v >= 0 ? S.colorUp : S.colorDown, i === best || i === worst);
    // only the totals stand on the axis: they fade to it, the steps in between stay solid
    out += P('marks', bar({ ...S, barFill: x.total ? S.barFill : 'flat' }, cx - bw / 2, y0, bw, Math.max(1.2, y1 - y0), x.v >= 0 || x.total ? 'up' : 'down', t));
    if ((!x.total || (bridge && i === 0)) && i < bars.length - 1 && S.connectors !== 'none') {
      out += P('connectors', `<line x1="${r2(cx + bw / 2)}" x2="${r2(cx + band - bw / 2)}" y1="${r2(scale(x.to))}" y2="${r2(scale(x.to))}" stroke="${conn}" stroke-width="1"${S.connectors === 'dashed' ? ' stroke-dasharray="3 3"' : ''}/>`);
    }
    const col = x.v >= 0 ? S.colorUp : S.colorDown;
    if (x.total && bridge && i === 0) {
      out += P('labels', PT(cx, y0 - 4, labeler(st, S, fmt, { full: true })(x.v), pf('labels', 1, { weight: 700, fill: th.muted, anchor: 'middle' })));
    } else if (x.total) {
      out += S.callout !== 'none' ? P('highlights', pill(st, S, cx, y0 - CS(1) - 6, labeler(st, S, fmt, { full: true })(x.v, true), CS(0.5)))
        : P('labels', PT(cx, y0 - 4, labeler(st, S, fmt, { full: true })(x.v), pf('labels', 1, { weight: 700, fill: th.ink, anchor: 'middle' })));
    } else if (S.labels !== 'none') {
      const signed = labelLabeler(st, S, fmt)(x.v, true);
      const lc = labelColorOf(S, th, x.v, col, col);
      if (x.v >= 0) {
        out += P('labels', PT(cx, y0 - 5, signed, pf('labels', 0.5, { weight: 700, fill: lc, anchor: 'middle' })));
      } else {
        out += P('labels', PT(cx, y1 + pf('labels', 0.5).size + 4, signed, pf('labels', 0.5, { weight: 700, fill: lc, anchor: 'middle' })));
      }
    }
    if (S.xLabels) { const ao = pf('axis', 0.5, { weight: x.total ? 700 : 400, fill: x.total ? th.ink : th.muted, anchor: 'middle' }); out += P('axis', T(cx, b.y + b.h - lg.bottom - 3, ellipsize(up(ao, x.name), band - 2, ao.size, ao.weight, ao.family || curFont().family, ao.italic ?? curFont().italic, ao.tracking || 0), ao)); }
  });
  return out;
}

// ---------------------------------------------------------------- donut / pie

export function donut(st, w, b, data) {
  const S = resolve(st, w);
  const drawn = slots(w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0];
  if (!s0) return message(st, b, 'No data for this period.');
  // contributions can be negative: slices use the size, the centre the signed sum
  const raw = sh.cats.map((c, i) => ({ name: c, raw: s0.values[i] || 0 }));
  const signedMode = raw.some((x) => x.raw < 0);
  let items = raw.map((x) => ({ ...x, v: signedMode ? Math.abs(x.raw) : Math.max(0, x.raw) })).filter((x) => x.v > 0);
  if (S.sortSlices !== false) items.sort((a, z) => z.v - a.v);
  if (!items.length) return message(st, b, 'No data for this period.');
  if (items.length > S.maxSlices) {
    const restItems = items.slice(S.maxSlices - 1);
    items = [...items.slice(0, S.maxSlices - 1), { name: word('Other') || 'Other', v: restItems.reduce((a, x) => a + x.v, 0), raw: restItems.reduce((a, x) => a + x.raw, 0), other: true }];
  }
  const total = items.reduce((a, x) => a + x.v, 0);
  const signedTotal = items.reduce((a, x) => a + x.raw, 0);
  const fmt = s0.format || 'number';
  // the legend's amounts are its data labels: their own format when set
  const money = S.labelUnits || S.labelDecimals != null || S.labelSuffix != null
    ? (v) => labelLabeler(st, S, fmt)(v, S.signed || signedMode)
    : (v) => fmtNum(v, fmt, { currency: th.currency, compact: S.compact === 'on', decimals: S.decimals, signed: S.signed || signedMode }) + S.suffix;
  const order = [...items].map((x) => x.name).sort((a, z) => a.localeCompare(z));
  const base = S.baseColor || (signedMode && S.mono ? (signedTotal >= 0 ? S.colorUp : S.colorDown) : '') || w.style?.color || '#a98a6e';
  const color = (it, i) => (it.other ? '#b9b8b2' : S.mono ? mixHex(base, th.page_bg, Math.min(0.8, i * (0.8 / Math.max(1, items.length)))) : seriesCol(st, w, S, order.indexOf(it.name), it.name));
  const legendStyle = S.legendStyle;
  const side = legendStyle !== 'none' && b.w > b.h * 1.3;
  // a side legend takes what its names, amounts and shares need — the ring gives way, down to a third of the height
  const lgo = pf('legend', legendStyle === 'table' ? 1.5 : 0.5, { weight: legendStyle === 'table' ? 600 : 400, fill: th.ink });
  const lsize = lgo.size;
  const needW = 22 + Math.max(...items.map((it) => pw(lgo, it.name)))
    + (S.legendAmount ? Math.max(...items.map((it) => tw(money(it.raw), lsize, 700))) + 10 : 0) + (S.legendShare ? 26 : 0);
  // a ring with a number in its middle keeps room for it: at least half the height
  const ringMin = S.center !== 'none' ? Math.min(b.w * 0.4, Math.max(50, b.h * 0.42)) : Math.max(28, b.h * 0.28);
  const legendW = legendStyle === 'none' ? 0 : side ? Math.min(b.w - ringMin - 20, Math.max(b.w * (legendStyle === 'table' || S.legendAmount ? 0.6 : 0.45), Math.min(needW, b.w - ringMin - 20))) : b.w;
  const legendH = legendStyle === 'none' || side ? 0 : items.length * (lsize + 6) + 6;
  const rad = Math.max(8, Math.min(side ? (b.w - legendW - 16) / 2 : b.w / 2, (b.h - legendH) / 2) - 2);
  const cx = side ? b.x + rad + 2 : b.x + b.w / 2;
  const cy = side ? b.y + b.h / 2 : b.y + rad + 2;
  const inner = rad * clamp(S.inner, 0, 0.92);
  let out = '';
  let a0 = -Math.PI / 2;
  items.forEach((it, i) => {
    const a1 = a0 + (it.v / total) * Math.PI * 2;
    const pt = partTone(S, color(it, i));
    out += P(`series:${it.name}`, `<path d="${arc(cx, cy, rad, inner, a0, a1)}" fill="${pt.fill}" stroke="${th.page_bg}" stroke-width="${S.sliceGap}" stroke-linejoin="round"/>`);
    if (pt.stroke) out += P(`series:${it.name}`, `<path d="${arc(cx, cy, rad, inner, a0, a1)}" fill="none" stroke="${pt.stroke}" stroke-width="${pt.sw || 0.5}" stroke-linejoin="round"/>`);
    a0 = a1;
  });
  if (inner > 9 && S.center !== 'none') {
    // the number and its label as one block in the middle of the ring; a unit that would make the
    // number too small to read goes under it as the label ("+1,40" over "П.П.")
    let big = S.center === 'text' ? S.centerText : S.center === 'top' ? fmtNum(items[0].v / total, 'percent', { decimals: 0 })
      : fmtNum(signedTotal, fmt, { compact: S.compact !== 'off', currency: th.currency, signed: S.signed || signedMode, decimals: S.decimals });
    // a blank label (" ") means "no label": the design hides the word "Total"
    const set = S.centerLabel != null && S.centerLabel !== '';
    const lbl = String(S.centerLabel || '').trim();
    let sub = lbl || (S.center === 'top' ? items[0].name : '');
    const unit = S.center === 'total' || !S.center ? String(S.suffix || '').trim() : '';
    const fits = (t) => Math.min(inner * 0.5, S.centerSize || 22, (inner * 1.7) / Math.max(1, String(t).length * 0.55));
    if (unit && !lbl && fits(big + S.suffix) < Math.min(inner * 0.5, 12)) sub = unit;
    else if (unit) big += S.suffix;
    if (!sub && !set && S.center !== 'text') sub = word('Total');
    const bs = fits(big);
    note(w, 'centerSize', Math.round(bs));
    const ss = Math.min(CS(-0.5), Math.max(5, inner * 0.28));
    // cap heights: the pair centred together, a small gap between
    const gap = Math.max(2, bs * 0.18);
    const blockH = bs * 0.72 + (sub ? gap + ss * 0.72 : 0);
    const top = cy - blockH / 2;
    out += P('centre', T(cx, top + bs * 0.72, big, { size: bs, weight: 700, fill: th.ink, anchor: 'middle' }));
    if (sub) out += P('centre', T(cx, top + bs * 0.72 + gap + ss * 0.72, S.centerUpper ? sub.toUpperCase() : sub, { size: ss, weight: 600, fill: th.muted, anchor: 'middle' }));
  }
  if (legendStyle === 'none') return out;
  const lx = side ? cx + rad + 20 : b.x;
  const lw = side ? b.x + b.w - lx : b.w;
  const rowH = legendStyle === 'table' ? Math.min((side ? b.h : legendH) / items.length, 26) : lsize + 6;
  const ly0 = side ? cy - (rowH * items.length) / 2 : cy + rad + 10;
  items.forEach((it, i) => {
    const y = ly0 + rowH * i, mid = y + rowH / 2;
    const size = lsize;
    out += P('legend', rect(lx, mid - 4, 8, 8, 2, partTone(S, color(it, i))));
    drawn.add(it.name);
    const dg = (S.icons && typeof S.icons === 'object') || S.iconsAuto ? glyphFor(S.icons, it.name, '', S.iconsAuto) : '';
    if (dg) out += P('icons', glyph(dg, lx + 13, mid - size * 0.75, size * 1.5, chip(S, th, 'circle')));
    const dgx = dg ? size * 1.9 : 0;
    const amt = S.legendAmount ? money(it.raw) : '';
    const share = S.legendShare ? fmtNum(it.v / total, 'percent', { decimals: 0 }) : '';
    const shareW = share ? 26 : 0;
    const amtW = amt ? tw(amt, size, 700) + 10 : 0;
    out += P('legend', T(lx + 14 + dgx, mid + size * 0.36, ellipsize(up(lgo, it.name), lw - 20 - dgx - shareW - amtW, size, lgo.weight, lgo.family || curFont().family, lgo.italic ?? curFont().italic, lgo.tracking || 0), lgo));
    if (amt) out += P('legend', T(lx + lw - shareW - 4, mid + size * 0.36, amt, { size, weight: 700, family: lgo.family, italic: lgo.italic, fill: labelColorOf(S, th, it.raw, color(it, i), th.ink), anchor: 'end' }));
    if (share) out += P('legend', T(lx + lw, mid + size * 0.36, share, { size: size - 0.5, family: lgo.family, italic: lgo.italic, fill: th.muted, anchor: 'end' }));
    if (legendStyle === 'table') out += P('legend', `<line x1="${r2(lx)}" x2="${r2(lx + lw)}" y1="${r2(y + rowH)}" y2="${r2(y + rowH)}" stroke="${th.grid}" stroke-width="0.6"/>`);
  });
  return out;
}

// ---------------------------------------------------------------- treemap

/**
 * A treemap: each tile's area is its value's share of the box, tiles fill it
 * with thin paper gaps, and what a tile says follows its size (icon, name,
 * value and share or result; a line; or colour only). Colour: by category, by
 * result (green / red, stronger the larger the result) or by value (one hue,
 * darker the larger). Categories below "group small below" fold into one tile.
 */
export function treemap(st, w, b, data) {
  const S = resolve(st, w);
  const drawn = slots(w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0];
  if (!s0) return message(st, b, 'No data for this period.');
  const items = sh.cats.map((c, i) => ({ name: c, v: Math.max(0, s0.values[i] || 0) })).filter((x) => x.v > 0).sort((a, z) => z.v - a.v);
  if (!items.length) return message(st, b, 'No data for this period.');
  const total = items.reduce((a, x) => a + x.v, 0);
  const order = [...items].map((x) => x.name).sort((a, z) => a.localeCompare(z));
  const fmt = s0.format || 'currency';
  const money = (v) => fmtNum(v, fmt === 'percent' ? 'number' : fmt, { currency: th.currency, compact: S.compact === 'on', decimals: S.decimals });
  const s1 = sh.series[1];
  const second = s1 ? Object.fromEntries(sh.cats.map((c, i) => [c, s1.values[i]])) : {};
  const secondText = (name) => (s1 && second[name] != null ? fmtNum(second[name], s1.format || 'number', { signed: true, currency: th.currency, decimals: s1.format === 'percent' ? (S.decimals ?? 1) : null }) : '');
  const icons = S.icons && typeof S.icons === 'object' ? S.icons : S.iconsAuto ? {} : null;
  const shareTxt = S.shareText || word('share');

  // the smallest categories fold into one tile
  let tiles = items;
  const small = items.filter((x) => (x.v / total) * 100 < (S.chipBelow || 0));
  if (small.length > 1) {
    const sum = small.reduce((a, x) => a + x.v, 0);
    const r = s1 ? small.reduce((a, x) => a + (second[x.name] ?? 0) * x.v, 0) / sum : null;
    tiles = [...items.filter((x) => !small.includes(x)), { name: `${word('Other')} · ${small.length}`, v: sum, other: true, r }];
  }

  // colour: what the tile's colour says
  const bySign = S.colorMode === 'sign' && s1;
  const byValue = S.colorMode === 'value';
  const rOf = (it) => (it.other ? it.r : second[it.name]) ?? 0;
  const maxAbs = Math.max(1e-9, ...tiles.map((it) => Math.abs(rOf(it))));
  const maxV = tiles[0].v;
  const base = S.baseColor || th.palette?.[0] || th.ink;
  const paint = (it) => {
    if (bySign) {
      const r = rOf(it);
      if (Math.abs(r) < maxAbs * 0.02) return { col: th.muted, k: 0.18 };
      return { col: r > 0 ? S.colorUp : S.colorDown, k: 0.18 + 0.5 * Math.min(1, Math.abs(r) / maxAbs) };
    }
    if (byValue) return { col: base, k: 0.12 + 0.6 * (it.v / maxV) };
    if (it.other) return { col: th.muted, k: 0.16 };
    return { col: seriesCol(st, w, S, order.indexOf(it.name), it.name), k: S.tmFill != null ? +S.tmFill : 0.24 };
  };

  // where the tiles go: squarified, or the largest as a full-height column of its share
  const squarify = (list, bx0) => {
    const out = [];
    let rest = [...list], bx = { ...bx0 };
    let remaining = list.reduce((a, x) => a + x.v, 0);
    while (rest.length) {
      const horiz = bx.w >= bx.h, side = horiz ? bx.h : bx.w;
      let row = [], bestR = Infinity;
      for (const it of rest) {
        const trial = [...row, it];
        const sum = trial.reduce((a, x) => a + x.v, 0);
        const len = (sum / remaining) * (horiz ? bx.w : bx.h);
        const worstR = Math.max(...trial.map((x) => { const l = (x.v / sum) * side; return Math.max(len / l, l / len); }));
        if (worstR > bestR) break;
        bestR = worstR; row = trial;
      }
      const sum = row.reduce((a, x) => a + x.v, 0);
      const len = (sum / remaining) * (horiz ? bx.w : bx.h);
      let off = 0;
      for (const it of row) { const l = (it.v / sum) * side; out.push(horiz ? { it, x: bx.x, y: bx.y + off, w: len, h: l } : { it, x: bx.x + off, y: bx.y, w: l, h: len }); off += l; }
      bx = horiz ? { x: bx.x + len, y: bx.y, w: bx.w - len, h: bx.h } : { x: bx.x, y: bx.y + len, w: bx.w, h: bx.h - len };
      remaining -= sum;
      rest = rest.slice(row.length);
    }
    return out;
  };
  const gap = S.tmGap ?? 2;
  // the outer edges meet the box; the gaps are only between tiles
  const box = { x: b.x - gap / 2, y: b.y - gap / 2, w: b.w + gap, h: b.h + gap };
  const layout = S.tmLayout === 'auto' ? (tiles[0].v / total > 0.4 ? 'split' : 'squarify') : S.tmLayout;
  let rects;
  if (layout === 'split' && tiles.length > 1) {
    // the largest a full-height column of its share; the others bands across the rest, each the height of its share
    // a dominant category leaves the others room for a name and a value (about 120 pt)
    const lw = Math.min(box.w * (tiles[0].v / total), Math.max(box.w * 0.6, box.w - 120));
    const rest = tiles.slice(1), restSum = rest.reduce((a, x) => a + x.v, 0);
    let yy = box.y;
    rects = [{ it: tiles[0], x: box.x, y: box.y, w: lw, h: box.h }, ...rest.map((it) => { const h = box.h * (it.v / restSum); const r = { it, x: box.x + lw, y: yy, w: box.w - lw, h }; yy += h; return r; })];
  } else rects = squarify(tiles, box);

  const radius = Math.min(4, th.radius ?? 4);
  let out = '';
  for (const r of rects) {
    const x = r.x + gap / 2, y = r.y + gap / 2, wd = r.w - gap, ht = r.h - gap;
    if (wd < 1 || ht < 1) continue;
    const it = r.it;
    if (!it.other) drawn.add(it.name);
    const { col, k } = paint(it);
    const fill = mixHex(col, th.page_bg, 1 - k);
    const ink = inkOn(fill) === '#ffffff' ? '#ffffff' : th.ink;
    const soft = ink === '#ffffff' ? 'rgba(255,255,255,.82)' : th.muted;
    // the gaps separate the tiles; an outline only when asked for (not the chart preset's)
    const o0 = out.length;
    out += rect(x, y, wd, ht, radius, { fill, stroke: S.tmOutline ? S.outlineColor || mixHex(col, th.page_bg, 0.45) : null, sw: S.outlineWidth });
    const note = bySign || (s1 && it.other) ? secondText(it.name) || (it.other && it.r != null ? fmtNum(it.r, s1?.format || 'number', { signed: true, decimals: s1?.format === 'percent' ? (S.decimals ?? 1) : null }) : '') : fmtNum(it.v / total, 'percent', { decimals: 0 });
    const noteTail = bySign ? '' : shareTxt;
    const pad = clamp(Math.min(wd, ht) * 0.07, 5, 12);
    const ns = clamp(Math.min(wd * 0.075, ht * 0.2), CS(0), CS(2.2));
    const g = icons && !it.other ? glyphFor(icons, it.name, '', S.iconsAuto) : '';
    const gw = g ? ns * 1.45 : 0;
    const vs = clamp(Math.min(ht * 0.2, wd * 0.12), CS(1.5), 28);
    // the value in full, or short when the tile is narrow
    const full = money(it.v);
    const value = tw(full, CS(0.5), 700) > wd - pad * 2 - 40 ? fmtNum(it.v, fmt === 'percent' ? 'number' : fmt, { currency: th.currency, compact: true }) : full;
    const nameLine = (yb, size) => {
      if (g) out += P('icons', glyph(g, x + pad, yb - size * 0.95, size * 1.15, chip(S, th, 'square')));
      out += T(x + pad + gw, yb, ell(it.name, wd - pad * 2 - gw, size, 600), { size, weight: 600, fill: ink });
    };
    // together in the top left: the name, then the value and its share in the same size under it
    const lh = CS(0.5) * 1.25;
    if (S.tmLabels === 'top' && ht >= pad * 2 + lh * 2 && wd >= 40) {
      const s = CS(0.5);
      nameLine(y + pad + s * 0.95, s);
      const withNote = ht >= pad * 2 + lh * 3;
      out += T(x + pad, y + pad + s * 0.95 + lh, ell(value, wd - pad * 2, s, 600), { size: s, weight: 600, fill: ink });
      if (withNote) out += T(x + pad, y + pad + s * 0.95 + lh * 2, ell(`${note}${noteTail && !it.other ? ' ' + noteTail : ''}`, wd - pad * 2, s - 0.5, 500), { size: s - 0.5, weight: 500, fill: soft });
    } else if (S.tmLabels === 'top' && ht >= CS(0) * 1.7 && wd >= 40) {
      // a thin tile: the name and its value on one line, as far as they fit
      const s = Math.min(CS(0), ht * 0.55), yb = y + ht / 2 + s * 0.36, room = wd - pad * 2;
      const nw = tw(it.name, s, 600), vw = tw(value, s, 600);
      if (nw + 5 + vw <= room) {
        out += T(x + pad, yb, it.name, { size: s, weight: 600, fill: ink });
        out += T(x + pad + nw + 5, yb, value, { size: s, weight: 500, fill: ink });
      } else out += T(x + pad, yb, ell(it.name, room, s, 600), { size: s, weight: 600, fill: ink });
    } else if (S.tmLabels === 'top') {
      // too small for words
    } else if (ht >= pad * 2 + ns * 1.3 + vs * 1.15 + CS(0.5) * 1.5 && wd >= 90) {
      // a large tile: name on top, the value large at the bottom, its share or result under it
      nameLine(y + pad + ns * 0.95, ns);
      out += T(x + pad, y + ht - pad - CS(0.5) * 1.5, ell(value, wd - pad * 2, vs, 700), { size: vs, weight: 700, fill: ink });
      out += T(x + pad, y + ht - pad, note, { size: CS(0.5), weight: 700, fill: bySign ? ink : soft });
      if (noteTail && !it.other) out += T(x + pad + tw(note, CS(0.5), 700) + 4, y + ht - pad, noteTail, { size: CS(0.5), fill: soft });
    } else if (ht >= pad * 2 + CS(0.5) * 2.6 + CS(0.5) && wd >= 64) {
      // a middle tile: the name, then the value and its share on one line
      const s = CS(0.5);
      nameLine(y + pad + s * 0.95, s);
      out += T(x + pad, y + ht - pad, ell(value, wd - pad * 2 - tw(note, s - 0.5, 700) - 6, s, 700), { size: s, weight: 700, fill: ink });
      out += T(x + wd - pad, y + ht - pad, note, { size: s - 0.5, weight: 600, fill: soft, anchor: 'end' });
    } else if (ht >= CS(0) * 1.7 && wd >= 40) {
      // a small tile: one line, the value at the right when it fits
      const s = Math.min(CS(0), ht * 0.55), yb = y + ht / 2 + s * 0.36;
      // coloured by result: its result beside the value when there is room
      const withNote = bySign && note && tw(`${value} · ${note}`, s, 700) + tw(it.name, s, 600) + 16 < wd - pad * 2;
      const shown = withNote ? `${value} · ${note}` : value;
      const vw = tw(shown, s, 700);
      const room = wd - pad * 2;
      if (room > vw + 30) {
        out += T(x + pad, yb, ell(it.name, room - vw - 6, s, 600), { size: s, weight: 600, fill: ink });
        out += T(x + wd - pad, yb, shown, { size: s, weight: 700, fill: ink, anchor: 'end' });
      } else out += T(x + pad, yb, ell(it.name, room, s, 600), { size: s, weight: 600, fill: ink });
    }
    out = tag(out, o0, it.other ? 'marks' : `series:${it.name}`);
  }
  return out;
}

// ---------------------------------------------------------------- tiles

export function tiles(st, w, b, data, vm) {
  const S = resolve(st, w);
  const drawn = slots(w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0], s1 = sh.series[1];
  if (!s0 || !sh.cats.length) return message(st, b, 'No data for this period.');
  const n = sh.cats.length;
  const cols = Math.max(1, Math.min(n, S.cols || Math.ceil(n / Math.max(1, Math.round(b.h / 52)))));
  note(w, 'cols', cols);
  const rows = Math.ceil(n / cols);
  const gap = S.tileGap ?? 6;
  const cw = (b.w - gap * (cols - 1)) / cols, chh = (b.h - gap * (rows - 1)) / rows;
  const tf = vm.fonts.kpi, cf = vm.fonts.caption;
  const vals = s0.values.filter((v) => typeof v === 'number');
  const ext = pickExtremes(s0.values, S.extremesRule);
  const best = ext.best >= 0 ? s0.values[ext.best] : NaN, worst = ext.worst >= 0 ? s0.values[ext.worst] : NaN;
  const lab0 = labeler(st, S, s0.format || 'number', { full: true });
  let out = '';
  sh.cats.forEach((cat, i) => {
    const x = b.x + (i % cols) * (cw + gap), y = b.y + Math.floor(i / cols) * (chh + gap);
    const v = s0.values[i];
    const sgn = (s1 ? s1.values[i] : v) ?? 0;
    const mark = S.markExtremes && n > 2 && (v === best || v === worst);
    const edge = mark ? (v === best ? S.colorUp : S.colorDown) : S.tileBorder ? mixHex(th.ink, th.page_bg, 0.86) : 'none';
    const toneCol = S.tileTone === 'sign' && sgn !== 0 ? (sgn > 0 ? S.colorUp : S.colorDown) : null;
    const fillT = toneCol ? mixHex(toneCol, th.page_bg, 0.86) : S.tileBg || w.style?.background || th.card_bg;
    const edgeT = mark ? edge : toneCol ? mixHex(toneCol, th.page_bg, 0.62) : edge;
    const o0 = out.length;
    out += `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(cw)}" height="${r2(chh)}" rx="${S.radius ?? 5}" fill="${fillT}" stroke="${edgeT}" stroke-width="${mark ? 1.2 : 0.7}"/>`;
    const pad = S.tilePad ?? 7;
    const capText = cf.upper ? String(cat).toUpperCase() : String(cat);
    const capO = { size: cf.size, weight: cf.weight || 600, fill: legible(cf.color || th.ink, fillT, 4.5), family: cf.family, italic: cf.italic, tracking: cf.tracking || 0 };
    const capW = (t) => textWidth(t, cf.size, capO.weight, cf.family, cf.italic, capO.tracking);
    // a long label takes a second line when the tile is tall enough
    let capLines = [capText];
    if (capW(capText) > cw - pad * 2 && chh > cf.size * 5.5 && capText.includes(' ')) {
      const words = capText.split(' ');
      let k = words.length - 1;
      while (k > 1 && capW(words.slice(0, k).join(' ')) > cw - pad * 2) k--;
      capLines = [words.slice(0, k).join(' '), words.slice(k).join(' ')];
    }
    drawn.add(cat);
    const tg = (S.icons && typeof S.icons === 'object') || S.iconsAuto ? glyphFor(S.icons, cat, '', S.iconsAuto) : '';
    const gx = tg ? cf.size * 2.6 : 0;
    if (tg) out += P('icons', glyph(tg, x + pad, y + pad - cf.size * 0.35, cf.size * 2.1, chip(S, th, tg.startsWith('flag-') ? 'circle' : 'square', 'transparent')));
    capLines.forEach((l, li) => { out += T(x + pad + gx, y + pad + cf.size + li * cf.size * 1.25, ellipsize(l, cw - pad * 2 - gx, cf.size, capO.weight, cf.family, cf.italic, capO.tracking), capO); });
    // the figure always reads on its tile (a dark tile turns dark ink white)
    const color = legible(S.valueColor === 'sign' ? (sgn > 0 ? S.colorUp : sgn < 0 ? S.colorDown : tf.color) : S.valueColor === 'ink' ? th.ink : tf.color, fillT, 3);
    // the caption's height: its lines, or the icon beside them when that is taller
    const capH = Math.max(cf.size + (capLines.length - 1) * cf.size * 1.25, tg ? cf.size * 1.75 : 0);
    const s1Text = s1 ? fmtNum(s1.values[i], s1.format || 'number', { currency: th.currency, signed: true, decimals: S.decimals }) : '';
    // a short tile: the value and the second figure share the bottom line (nothing overlaps the caption)
    const tight = s1 && chh - pad * 2 - capH - 4 < (cf.size + 4) + Math.max(9, cf.size * 1.4);
    const room = chh - pad * 2 - capH - 4 - (s1 && !tight ? cf.size + 4 : 0);
    const vsz = Math.max(7, Math.min(tf.size > 0 ? tf.size : 16, room * 0.95));
    const s1W = tight ? textWidth(s1Text, cf.size, 500, cf.family) + 8 : 0;
    const vy = tight ? y + chh - pad : Math.min(y + pad + capH + 4 + vsz * 0.92 + Math.max(0, room - vsz) * 0.6, y + chh - pad - (s1 ? cf.size + 4 : 0));
    out += T(x + pad, vy, ell(lab0(v), cw - pad * 2 - s1W, vsz, 700), { size: vsz, weight: tf.weight >= 600 ? tf.weight : 700, fill: color, family: tf.family, italic: tf.italic });
    if (s1) out += T(tight ? x + cw - pad : x + pad, y + chh - pad, s1Text, { size: cf.size, weight: 500, fill: color, family: cf.family, anchor: tight ? 'end' : 'start' });
    out = tag(out, o0, `series:${cat}`);
  });
  return out;
}

// ---------------------------------------------------------------- KPI

export function kpi(st, w, b, data, vm) {
  const S = resolve(st, w);
  const th = st.doc.theme;
  const k = data?.kpi;
  if (!k) return message(st, b, 'No data.');
  const col = (data.columns || []).find((c) => c.role === 'measure');
  const fmt = S.format && S.format !== 'auto' ? S.format : col?.format || 'number';
  const compact = S.compact === 'on' || (S.compact === 'auto' && Math.abs(k.value ?? 0) >= 1e7);
  const mcols = (data.columns || []).filter((c) => c.role === 'measure');
  // with a "Group by": the top category's name, its value, or every category's share
  const cats = Array.isArray(k.cats) ? k.cats : null;
  const mode = cats ? S.kpiGroup || 'name' : null;
  const shown = mode === 'shares' ? cats.filter((c) => Math.round((c.share || 0) * 100) > 0) : cats || [];
  // one value as written: text as it is; whole numbers before the last (counts) without decimals
  const lastNum = (k.values || []).reduce((a, v, i) => (typeof v === 'number' ? i : a), -1);
  const one = (v, i) => (v == null ? '' : typeof v === 'string' ? v
    : fmtNum(v, mcols[i]?.format || 'number', { currency: th.currency, compact: S.compact === 'on', decimals: Number.isInteger(v) && i !== lastNum && mcols[i]?.format !== 'percent' ? 0 : S.decimals }));
  // a text value (e.g. the date of the next event) is shown as it is
  let value = mode === 'shares' ? shown.map((c) => Math.round(c.share * 100)).join(' · ') + ' %'
    : mode === 'name' ? k.name || '—'
    : typeof k.value === 'string' ? k.value : fmtNum(k.value, fmt, { currency: th.currency, compact, signed: S.signed, decimals: S.decimals }) + S.suffix;
  // every number on one line: "24 · 52,8 млн", "Сен · 3,3" (text values go in the note)
  if (S.valueJoin && mode !== 'shares') value = [mode === 'name' ? k.name : null, ...(k.values || [k.value]).map((v, i) => (typeof v === 'string' ? '' : one(v, i)))].filter(Boolean).join(' · ') + S.suffix;
  const plainValue = mode === 'name' || mode === 'shares' || S.valueJoin || typeof k.value !== 'number';
  const kf = vm.fonts.kpi, cf = vm.fonts.caption;
  const sparkVals = (k.spark || []).filter((p) => p.v != null);
  const wantSpark = S.spark !== 'none' && sparkVals.length > 2;
  const sparkPos = S.sparkPos === 'auto' ? (b.w >= 150 ? 'right' : 'bottom') : S.sparkPos;
  const side = wantSpark && sparkPos === 'right';
  const center = S.kpiAlign === 'center';
  const right = S.kpiAlign === 'right';
  const textW = side ? b.w - Math.min(b.w * 0.36, 130) - 10 : b.w;
  let size = S.valueSize > 0 ? S.valueSize : kf.size > 0 ? kf.size : clamp(b.h * 0.44, 13, 30);
  while (size > 8 && textWidth(value, size, kf.weight, kf.family, kf.italic) > textW) size -= 1;
  const good = (v) => (v > 0) === S.upIsGood;
  const vmode = w.opts?.viz?.valueColor || st.doc.theme.viz?.kpi?.valueColor || (S.signed ? 'sign' : 'font');
  const vcolor = vmode === 'sign' && !plainValue && k.value !== 0 ? (good(k.value) ? S.colorUp : S.colorDown) : vmode === 'ink' ? th.ink : kf.color;
  // an arrow glyph before the value: ↗ up, ↘ down
  const arrow = S.arrow && (k.value ?? 0) !== 0 ? ((k.value ?? 0) > 0 ? '↗' : '↘') : '';
  const aSize = size * 0.62, aW = arrow ? textWidth(arrow, aSize, 700, kf.family) + size * 0.25 : 0;
  while (arrow && size > 8 && textWidth(value, size, kf.weight, kf.family, kf.italic) + aW > textW) size -= 1;
  const tx = right ? b.x + b.w : center && !side ? b.x + b.w / 2 : b.x;
  const anchor = right ? 'end' : center && !side ? 'middle' : 'start';
  // a second value beside the main one (e.g. the return next to the result)
  const asideShare = S.shareAside && cats?.[0]?.share != null;
  const asideIdx = asideShare ? 0 : Number(S.aside) || 0;
  const asideVal = asideShare ? cats[0].share : asideIdx ? k.values?.[asideIdx] : null;
  const asideCol = asideIdx ? mcols[asideIdx] : null;
  const asideText = asideShare ? fmtNum(asideVal, 'percent', { decimals: S.asideDecimals ?? 0 })
    : asideVal != null ? fmtNum(asideVal, asideCol?.format || 'number', { currency: th.currency, signed: true, decimals: S.asideDecimals ?? 2 }) : '';
  const asideW = () => (asideText ? textWidth(asideText, size * (asideShare ? 0.72 : 0.38), 700, kf.family) + size * 0.3 : 0);
  while (asideText && size > 8 && textWidth(value, size, kf.weight, kf.family, kf.italic) + aW + asideW() > textW) size -= 1;
  // fit the height too: the value gives way so the lines under it stay inside the widget
  const nVals = (k.values || [k.value]).length;
  const below = S.breakdown && nVals > 1 ? (nVals - 1) * Math.max(cf.size * 2.1, 12) + cf.size + 8
    : (S.note ? cf.size + 6 : 0) + (S.delta && (k.pct != null || k.delta != null) ? cf.size + 6 : 0);
  // the value is the point of a KPI: it gives way only so far (75% of its size);
  // past that the lines below are cut and the canvas offers to make the card taller
  const floor = Math.max(8, size * 0.75);
  if (!S.inline) while (size > floor && size * 1.1 + below > b.h) size -= 1;
  const asideSize = asideShare ? size * 0.72 : size * 0.38;
  note(w, 'valueSize', Math.round(size));
  if (S.inline && vm.title) {
    // the value sits on the title's line, at the right
    const ix = b.x + b.w;
    return P('value', T(ix, vm.title_y + size * 0.15, value, { size, weight: kf.weight, fill: vcolor, family: kf.family, italic: kf.italic, anchor: 'end' }));
  }
  let out = '';
  if (arrow) out += P('value', T(tx - (anchor === 'middle' ? textWidth(value, size, kf.weight, kf.family, kf.italic) / 2 + aW / 2 : 0), b.y + size * 0.86, arrow, { size: aSize, weight: 700, fill: vcolor, family: kf.family, anchor: 'start' }));
  out += P('value', T(tx + (anchor === 'middle' ? aW / 2 : anchor === 'end' ? 0 : aW), b.y + size * 0.92, value, { size, weight: kf.weight, fill: vcolor, family: kf.family, italic: kf.italic, anchor, tracking: kf.tracking }));
  if (asideText && anchor === 'start') {
    const vx = tx + aW + textWidth(value, size, kf.weight, kf.family, kf.italic) + size * 0.25;
    out += P('value', T(vx, b.y + size * 0.92, asideText, { size: asideSize, weight: 700, fill: asideShare || asideVal === 0 ? th.muted : good(asideVal) ? S.colorUp : S.colorDown, family: kf.family }));
  }
  let y = b.y + size * 0.92 + cf.size + 5.5;
  const cols = (data.columns || []).filter((c) => c.role === 'measure');
  const mv = k.values || [k.value];
  const fmtOf = (i, signed) => typeof mv[i] === 'string' ? mv[i] : fmtNum(mv[i], cols[i]?.format || 'number', { currency: th.currency, signed, compact: S.compact === 'on', decimals: cols[i]?.format === 'percent' ? S.noteDecimals ?? S.decimals : S.noteDecimals ?? null });
  // a note under the value: "{1} profit since January" — {n} is the n-th value, coloured by sign
  if (S.note) {
    const runs = [];
    let bold = false;
    // {name} the top category, {names} the categories shown, {share} the top one's share
    const words = { name: k.name || '', names: shown.map((c) => c.name).join(' · '), share: cats?.[0]?.share != null ? fmtNum(cats[0].share, 'percent', { decimals: 0 }) : '' };
    String(S.note).split(/(\{\d+\}|\{name\}|\{names\}|\{share\}|\*\*)/).forEach((piece) => {
      if (piece === '**') { bold = !bold; return; }
      const wm = piece.match(/^\{(name|names|share)\}$/);
      if (wm) { if (words[wm[1]]) runs.push({ text: words[wm[1]], bold, color: bold && S.noteBoldColor ? S.noteBoldColor : null }); return; }
      const m = piece.match(/^\{(\d+)\}$/);
      if (!m) { if (piece) runs.push({ text: piece, bold, color: bold && S.noteBoldColor ? S.noteBoldColor : null }); return; }
      const i = +m[1], v = mv[i];
      if (v == null) return;
      const color = typeof v === 'string' ? null : S.noteColor === 'sign' ? (v === 0 ? th.muted : good(v) ? S.colorUp : S.colorDown) : S.noteColor === 'ink' ? th.ink : null;
      runs.push({ text: fmtOf(i, S.noteColor === 'sign'), bold: true, color });
    });
    const nx = right ? b.x + b.w : center && !side ? b.x + b.w / 2 : b.x;
    // the note keeps inside the card: a little smaller type first (to 85%), then the end gives way (…)
    const nw = cf.weight < 600 ? cf.weight : 500;
    const widthAt = (sz) => runs.reduce((a, r) => a + textWidth(r.text, sz, r.bold ? 700 : nw, cf.family, cf.italic), 0);
    let nsz = cf.size;
    while (nsz > cf.size * 0.85 && widthAt(nsz) > b.w) nsz = +(nsz - 0.2).toFixed(2);
    if (widthAt(nsz) > b.w) {
      let room = b.w - textWidth('…', nsz, nw, cf.family, cf.italic);
      for (let i = 0; i < runs.length; i++) {
        const rw = textWidth(runs[i].text, nsz, runs[i].bold ? 700 : nw, cf.family, cf.italic);
        if (rw <= room) { room -= rw; continue; }
        runs[i] = { ...runs[i], text: ellipsize(runs[i].text, Math.max(0, room), nsz, runs[i].bold ? 700 : nw, cf.family, cf.italic, 0) };
        runs.length = i + 1;
        break;
      }
    }
    out += P('note', TR(nx, y, runs, { size: nsz, weight: nw, fill: cf.color, family: cf.family, italic: cf.italic, anchor: right ? 'end' : center && !side ? 'middle' : 'start' }));
    y += cf.size + 6;
  }
  // a chip with another value at the right of the note line ("+2,1")
  if (S.chip && mv[Number(S.chip)] != null) {
    const i = Number(S.chip), v = mv[i];
    const t = fmtOf(i, true);
    const cs = cf.size * 0.95, cw = textWidth(t, cs, 700, cf.family) + cs * 1.4, chh = cs * 1.8;
    const cy = y - cf.size - 6 + cf.size * 0.1 - chh / 2 - cs * 0.1;
    const col = v === 0 ? th.muted : good(v) ? S.colorUp : S.colorDown;
    out += P('note', rect(b.x + b.w - cw, cy, cw, chh, chh / 2, { fill: mixHex(col, th.page_bg, 0.84) }));
    out += P('note', T(b.x + b.w - cw / 2, cy + chh / 2 + cs * 0.36, t, { size: cs, weight: 700, fill: col, family: cf.family, anchor: 'middle' }));
  }
  // breakdown: the other values as rows (e.g. top-ups and withdrawals)
  if (S.breakdown && mv.length > 1) {
    const rowH = Math.max(cf.size * 2.1, 12);
    const rows = mv.length - 1;
    let ry = Math.max(y + 2, b.y + b.h - rows * rowH);
    out += P('note', `<rect x="${r2(b.x)}" y="${r2(ry - 2)}" width="${r2(b.w)}" height="0.6" fill="${th.grid}"/>`);
    for (let i = 1; i < mv.length; i++) {
      const v = mv[i];
      // parts of a whole (buys and sells) are neither good nor bad: a series colour, the value in ink
      const neutral = S.breakdownTone === 'neutral';
      const c = neutral ? seriesCol(st, w, S, i - 1) : v == null || v === 0 ? th.muted : good(v) ? S.colorUp : S.colorDown;
      const cy = ry + rowH / 2;
      out += P('note', `<rect x="${r2(b.x)}" y="${r2(cy - 4.5)}" width="9" height="9" rx="2" fill="${c}"/>`);
      if (!neutral) out += P('note', T(b.x + 4.5, cy + 2.3, v > 0 ? '+' : v < 0 ? '−' : '·', { size: 7, weight: 800, fill: '#ffffff', family: kf.family, anchor: 'middle' }));
      const val = v == null ? '—' : fmtOf(i, !neutral);
      const vw = textWidth(val, cf.size, 700, cf.family);
      // on a colour block (figures in the block's text colour) the rows take it too; the +/− chips keep the sign
      const onBlock = S.valueColor === 'font';
      const ink = onBlock ? cf.color || th.ink : th.ink;
      out += P('note', T(b.x + 14, cy + cf.size * 0.36, ellipsize(cols[i]?.label || '', b.w - vw - 22, cf.size, 500, cf.family), { size: cf.size, weight: 500, fill: ink, family: cf.family }));
      out += P('note', T(b.x + b.w, cy + cf.size * 0.36, val, { size: cf.size, weight: 700, fill: neutral || onBlock ? ink : c, family: cf.family, anchor: 'end' }));
      ry += rowH;
    }
    return out;
  }
  if (S.delta && (k.pct != null || k.delta != null)) {
    const up = (k.delta ?? 0) >= 0;
    const flat = Math.abs(k.pct ?? 0) < 0.0005 && Math.abs(k.delta ?? 0) < 1e-9;
    const color = flat ? th.muted : good(k.delta) ? S.colorUp : S.colorDown;
    const df = S.deltaFormat === 'auto' ? (fmt === 'percent' ? 'pts' : 'pct') : S.deltaFormat;
    const amount = df === 'pts' ? `${fmtNum(Math.abs(k.delta * 100), 'number', { decimals: 1 })} ${getLocale().startsWith('ru') ? 'п.п.' : 'pts'}`
      : df === 'abs' ? fmtNum(Math.abs(k.delta), fmt, { currency: th.currency, compact: true })
      : fmtNum(Math.abs(k.pct ?? 0), 'percent');
    const deltaText = flat ? '■ 0' : `${up ? '▲' : '▼'} ${amount}`;
    const cap = { size: cf.size, family: cf.family, italic: cf.italic };
    const cmpText = (S.deltaText || (st.period.compare ? `${word('vs')} {{compare}}` : '')).replace('{{compare}}', st.period.compare || '');
    const dw = textWidth(deltaText, cf.size, cf.weight, cf.family, cf.italic);
    const cw2 = cmpText ? textWidth(cmpText, cf.size, 400, cf.family, cf.italic) + 5 : 0;
    const dx = center && !side ? b.x + b.w / 2 - (dw + cw2) / 2 : b.x;
    out += P('change', T(dx, y, deltaText, { ...cap, weight: cf.weight, fill: color }));
    if (cmpText) out += P('change', T(dx + dw + 5, y, ellipsize(cmpText, textW - dw - 5, cf.size, 400, cf.family, cf.italic), { ...cap, fill: cf.color }));
    y += 6;
  }
  if (!wantSpark) return out;
  let box;
  if (side) {
    const sw = Math.min(b.w * 0.36, 130);
    box = { x: b.x + b.w - sw, y: b.y + 3, w: sw, h: Math.min(b.h - 4, 44) };
  } else {
    const room = b.y + b.h - y;
    if (room < 14) return out;
    const sh = Math.min(room - 2, 34);
    box = { x: b.x, y: b.y + b.h - sh, w: b.w - 4, h: sh - 1 };
  }
  const vals = (k.spark || []).map((p) => p.v);
  const finite = vals.filter((v) => v != null);
  const mn = Math.min(...finite), mx = Math.max(...finite);
  const n = vals.length;
  const accent = S.sparkColor || w.style?.color || th.palette[0];
  if (S.spark === 'bars') {
    const bw = (box.w / n) * 0.7;
    vals.forEach((v, i) => {
      if (v == null) return;
      const hgt = Math.max(1, ((v - Math.min(0, mn)) / (mx - Math.min(0, mn) || 1)) * box.h);
      out += P('trend', rect(box.x + (box.w / n) * i, box.y + box.h - hgt, bw, hgt, 1.5, { fill: i === n - 1 ? accent : mixHex(th.muted, th.page_bg, 0.45) }));
    });
    return out;
  }
  const pts = vals.map((v, i) => (v == null ? null : [box.x + 3 + ((box.w - 6) * i) / (n - 1), box.y + box.h - 3 - ((v - mn) / (mx - mn || 1)) * (box.h - 6)]));
  const d = pts.filter(Boolean).map((p, i) => `${i ? 'L' : 'M'}${r2(p[0])},${r2(p[1])}`).join('');
  out += P('trend', `<path d="${d}" fill="none" stroke="${S.sparkColor ? S.sparkColor : '#b9b8b2'}" stroke-width="1.2" stroke-linejoin="round" stroke-linecap="round"/>`);
  const last = pts.filter(Boolean).pop();
  if (last) out += P('trend', `<circle cx="${r2(last[0])}" cy="${r2(last[1])}" r="2.4" fill="${accent}" stroke="${th.card_bg}" stroke-width="1.2"/>`);
  return out;
}

export { textWidth };

// ---------------------------------------------------------------- 100% share bar

/** One bar split into each category's share, with a legend beneath. */
export function sharebar(st, w, b, data, vm) {
  const S = resolve(st, w);
  const drawn = slots(w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0], s1 = sh.series[1];
  if (!s0) return message(st, b, 'No data for this period.');
  let items = sh.cats.map((c, i) => ({ name: c, v: Math.max(0, s0.values[i] || 0), r: s1 ? s1.values[i] : null, i })).filter((x) => x.v > 0);
  if (!items.length) return message(st, b, 'No data for this period.');
  const total = items.reduce((a, x) => a + x.v, 0);
  const fmt = s0.format || 'number', fmt1 = s1?.format || 'number';
  // colour: the series palette, or the sign of the second value (a result)
  const signed = S.colorMode === 'sign' && s1;
  const maxAbs = signed ? Math.max(...items.map((x) => Math.abs(x.r || 0))) || 1 : 1;
  const colorOf = (it) => {
    if (it.other) return mixHex(th.muted, th.page_bg, 0.45);
    if (!signed) return seriesCol(st, w, S, it.i, it.name);
    const base = (it.r || 0) >= 0 ? S.colorUp : S.colorDown;
    return mixHex(base, th.page_bg, 0.15 + 0.5 * (1 - Math.min(1, Math.abs(it.r || 0) / maxAbs)));
  };
  const size = CS(1);
  const legendOn = S.legend !== 'none';
  const rowsLegend = S.legendStyle === 'rows';
  const legendH = !legendOn ? 0 : rowsLegend ? items.length * size * 2.2 + 4 : size * 2.6;
  const barH = S.shareHeight > 0 ? S.shareHeight : Math.max(6, Math.min(b.h - legendH, 18));
  note(w, 'shareHeight', Math.round(barH));
  const gap = S.shareGap;
  let out = '';
  let x = b.x;
  const r = Math.min(S.radius, barH / 2);
  // amounts: the label decimals; percentages: the legend's decimals
  const money = (v, f = fmt) => fmtNum(v, f, { currency: th.currency, compact: S.compact === 'on', decimals: f === 'percent' ? S.decimals : S.labelDecimals });
  items.forEach((it, k) => {
    const wd = (it.v / total) * (b.w - gap * (items.length - 1));
    const t = partTone(S, colorOf(it));
    const col = t.fill;
    const first = k === 0, last = k === items.length - 1;
    if (first && last) out += P(`series:${it.name}`, rect(x, b.y, wd, barH, r, t));
    else if (first) out += P(`series:${it.name}`, bar({ ...S, roundAll: false, radius: r, barFill: 'flat' }, x, b.y, wd, barH, 'left', t));
    else if (last) out += P(`series:${it.name}`, bar({ ...S, roundAll: false, radius: r, barFill: 'flat' }, x, b.y, wd, barH, 'right', t));
    else out += P(`series:${it.name}`, rect(x, b.y, wd, barH, 0, t));
    // a label inside a wide enough segment: name, then the value (or the result)
    if (S.insideLabels && barH < size * 2.6 && barH >= size * 1.3) {
      // a thin bar: "name · value" on one line
      const val = S.insideValue === 'second' && it.r != null ? money(it.r, fmt1) : money(it.v);
      const t = `${it.name} · ${val}`, ls = Math.min(size * 0.85, barH * 0.62);
      const lo = pf('labels', 0, { size: ls, weight: 700, fill: inkOn(col), anchor: 'middle' });
      if (wd > pw(lo, t) + 10) out += P('labels', PT(x + wd / 2, b.y + barH / 2 + lo.size * 0.36, t, lo));
    }
    if (S.insideLabels && barH >= size * 2.6) {
      const ink = inkOn(col);
      const val = S.insideValue === 'second' && it.r != null ? money(it.r, fmt1) : S.insideValue === 'share' ? fmtNum(it.v / total, 'percent', { decimals: 1 }) : money(it.v);
      const ls = size * 0.82, vs = size * 1.05;
      const need = Math.max(tw(it.name, ls, 600), tw(val, vs, 800)) + 12;
      if (wd > need) {
        out += P('labels', PT(x + 7, b.y + barH / 2 - vs * 0.25, ell(it.name, wd - 12, ls, 600), pf('labels', 0, { size: ls, weight: 600, fill: ink, opacity: 0.85 })));
        out += P('labels', PT(x + 7, b.y + barH / 2 + vs * 0.95, val, pf('labels', 0, { size: vs, weight: 800, fill: ink })));
      }
    }
    x += wd + gap;
  });
  if (!legendOn) return out;
  const shareOf = (it) => fmtNum(it.v / total, 'percent', { decimals: S.decimals ?? (it.v / total < 0.1 ? 1 : 0) });
  // the legend text after the name: "{value} · {share} · рез. {2}"
  const legendText = (it) => {
    if (S.legendText) return String(S.legendText).replace('{value}', money(it.v)).replace('{share}', shareOf(it)).replace('{2}', it.r != null ? fmtNum(it.r, fmt1, { currency: th.currency, signed: true }) : '—');
    return [S.legendShare ? shareOf(it) : '', S.legendAmount ? money(it.v) : ''].filter(Boolean).join('  ');
  };
  const icons = S.icons && typeof S.icons === 'object' ? S.icons : S.iconsAuto ? {} : null;
  if (rowsLegend) {
    // fit the rows into the height left: tighter rows, then smaller text,
    // then the smallest rows folded into one "Other" row
    const avail = b.y + b.h - (b.y + barH) - size * 0.4;
    let fs = size, step = size * 2.2, rows = items;
    if (rows.length * step > avail) {
      step = Math.max(size * 1.6, avail / rows.length);
      if (rows.length * step > avail) { fs = Math.max(size * 0.8, avail / rows.length / 1.6); step = fs * 1.6; }
      if (rows.length * step > avail) {
        const k = Math.max(1, Math.floor(avail / step) - 1);
        const rest = items.slice(k);
        rows = [...items.slice(0, k), { name: word('Other'), v: rest.reduce((a, x) => a + x.v, 0), r: null, i: -1, other: true }];
      }
    }
    const size0 = size;
    let ly = b.y + barH + Math.min(size0 * 2, step * 0.95);
    for (const it of rows) {
      const size = fs;
      let lx = b.x;
      out += P('legend', rect(lx, ly - size * 0.8, size * 0.85, size * 0.85, 2, partTone(S, colorOf(it))));
      lx += size * 1.5;
      if (!it.other) drawn.add(it.name);
      const g = icons ? glyphFor(icons, it.name, '', S.iconsAuto) : '';
      if (g) { out += P('icons', glyph(g, lx, ly - size * 1.0, size * 1.25, chip(S, th, 'circle'))); lx += size * 1.7; }
      out += P('legend', PT(lx, ly, it.name, pf('legend', 1, { size, weight: 700, fill: th.ink })));
      lx += pw(pf('legend', 1, { size, weight: 700 }), it.name) + size * 0.8;
      const rest = legendText(it);
      out += P('legend', T(lx, ly, ell(rest, b.x + b.w - lx, size * 0.92), { ...pf('legend', 1, { size: size * 0.92, fill: th.muted }), size: size * 0.92 }));
      if (signed && it.r != null) out += P('legend', T(b.x + b.w, ly, fmtNum(it.r, fmt1, { currency: th.currency, signed: true, decimals: fmt1 === 'percent' ? 1 : null }), { size: size * 0.92, weight: 700, fill: it.r >= 0 ? S.colorUp : S.colorDown, anchor: 'end' }));
      ly += step;
    }
    return out;
  }
  const ly = b.y + barH + size * 1.9;
  let lx = b.x;
  const cols = S.legendCols > 0 ? S.legendCols : 0;
  const colW = cols ? b.w / cols : 0;
  items.forEach((it, k) => {
    const rest = legendText(it);
    const nameW = pw(pf('legend', 1, { size, weight: 600 }), it.name), restW = rest ? pw({ ...pf('legend', 1, { size }), size, upper: false }, rest) + 5 : 0;
    const g = icons ? glyphFor(icons, it.name, '', S.iconsAuto) : '';
    const iw = 12 + (g ? size * 1.6 : 0) + nameW + 5 + restW;
    let px = cols ? b.x + colW * (k % cols) : lx;
    const py = cols ? ly + Math.floor(k / cols) * (size * 1.9) : ly;
    if (!cols && k > 0 && px + iw > b.x + b.w + 1) return;
    drawn.add(it.name);
    out += P('legend', rect(px, py - size * 0.78, size * 0.85, size * 0.85, 2, { fill: colorOf(it) }));
    px += 12;
    if (g) { out += P('icons', glyph(g, px, py - size * 0.95, size * 1.2, chip(S, th, 'circle'))); px += size * 1.6; }
    out += P('legend', PT(px, py, it.name, pf('legend', 1, { size, weight: 600, fill: th.ink })));
    px += nameW + 5;
    if (rest) out += P('legend', T(px, py, rest, { ...pf('legend', 1, { size, fill: th.muted }), size }));
    lx += iw + 16;
  });
  return out;
}

// ---------------------------------------------------------------- dot scale

/** Points on one horizontal scale: benchmarks against the portfolio, or a percentile. */
export function scale(st, w, b, data, vm) {
  const S = resolve(st, w);
  const th = st.doc.theme;
  const sh = shape(data);
  const s0 = sh.series[0];
  if (!s0 || !sh.cats.length) return message(st, b, 'No data for this period.');
  const fmt = s0.format || 'number';
  const lab = labeler(st, S, fmt, { full: true });
  const signed = S.signed || fmt === 'percent';
  const hlLab = S.hlDecimals != null ? labeler(st, { ...S, decimals: S.hlDecimals }, fmt, { full: true }) : lab;
  const pts = sh.cats.map((c, i) => ({ name: c, v: s0.values[i] })).filter((p) => p.v != null);
  const order = [...pts].sort((a, z) => a.v - z.v);
  const hl = S.highlight || (pts.length === 1 ? pts[0].name : '');
  const dots = S.markerStyle === 'dots';
  const size = CS(0.5);
  const th2 = S.shareHeight > 0 ? S.shareHeight : 5;
  // the lowest and highest points can close the scale: values outside its ends —
  // unless the highlighted point is one of them: it then stays a marker on the track ("you" are never a cap)
  const hlAtEnd = hl && (order[0]?.name === hl || order[order.length - 1]?.name === hl);
  const ends = S.scaleEnds && order.length >= 3 && !hlAtEnd ? [order[0], order[order.length - 1]] : null;
  const inner = ends ? order.slice(1, -1) : order;
  const capOf = (p) => (dots ? (p.v >= 0 ? S.colorUp : S.colorDown) : th.ink);
  const vals = pts.map((p) => p.v);
  let lo = S.scaleMin ?? Math.min(...vals), hi = S.scaleMax ?? Math.max(...vals);
  const span = hi - lo || 1;
  if (!ends) {
    if (S.scaleMin == null) lo -= span * 0.06;
    if (S.scaleMax == null) hi += span * 0.06;
  }
  // room for the end values (and their "min"/"max" captions) beside the track
  const endTxt = ends ? ends.map((p) => lab(p.v, signed)) : [];
  const endSize = size + 1.5;
  const endW = ends ? ends.map((p, i) => Math.max(tw(endTxt[i], endSize, 800), tw(p.name.toUpperCase(), size - 1.5, 700)) + 8) : [0, 0];
  const x0 = b.x + endW[0] + (ends ? th2 * 0.5 : 6), x1 = b.x + b.w - endW[1] - (ends ? th2 * 0.5 : 6);
  const X = (v) => x0 + ((v - lo) / (hi - lo || 1)) * (x1 - x0);
  // ticks: typed labels spread evenly, or round steps of the scale
  let ticks = String(S.scaleTicks || '').split('|').map((t) => t.trim()).filter(Boolean).map((t, i, a) => ({ t, x: a.length === 1 ? (x0 + x1) / 2 : x0 + ((x1 - x0) * i) / (a.length - 1), edge: i === 0 ? 'start' : i === a.length - 1 ? 'end' : 'middle' }));
  if (!ticks.length && S.axisTicks) {
    const raw = (hi - lo) / 8;
    const mag = 10 ** Math.floor(Math.log10(raw || 1));
    const step = [1, 2, 2.5, 5, 10].map((k) => k * mag).find((k) => k >= raw) || raw;
    for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-12; v += step) {
      const x = X(v);
      if (Math.abs(v) < step / 1e6 && S.scaleZero) continue;
      if (x - x0 < th2 * 0.8 || x1 - x < th2 * 0.8) continue;
      ticks.push({ t: lab(v, signed), x, edge: 'middle', line: true });
    }
  }
  const zero = S.scaleZero && lo < 0 && hi > 0;
  const big = dots ? th2 * 0.62 : 4.6, small = dots ? th2 * 0.32 : 3.6, endR = dots ? th2 * 0.42 : 3.6;
  const below = (ticks.length ? size * 1.7 : 0) + (S.nameLabels || zero ? size * 1.7 : 0);
  const above = S.valueLabels ? size * 2.8 + Math.max(0, big - th2 / 2) : 4;
  const ty = b.y + Math.max(above, (b.h - below - above) / 2 + above);
  const ring = th.page_bg;
  let out = '';
  if (S.track) {
    const gid = `g${String(w.id || 'x').replace(/[^\w-]/g, '')}`;
    const tx0 = ends ? x0 : b.x, tw0 = ends ? x1 - x0 : b.w;
    const down = mixHex(S.colorDown, th.page_bg, dots ? 0.5 : 0.35), up = mixHex(S.colorUp, th.page_bg, dots ? 0.5 : 0.35);
    let fill;
    if (S.trackStyle === 'gradient') {
      // the down colour below zero and the up colour above it, each paler towards zero;
      // a scale all above (or all below) zero is all up (or all down)
      const at = (v) => clamp((X(v) - tx0) / (tw0 || 1), 0, 1);
      const paleDown = mixHex(S.colorDown, th.page_bg, 0.72), paleUp = mixHex(S.colorUp, th.page_bg, 0.72);
      // (the air added at the ends doesn't count: only the data or a set end crosses zero)
      const neg = Math.min(...vals) < 0 || (S.scaleMin != null && S.scaleMin < 0), pos = Math.max(...vals) > 0 || (S.scaleMax != null && S.scaleMax > 0);
      const stops = neg && pos && lo < 0 && hi > 0
        ? [[0, down], [Math.max(0, at(0) - 0.012), paleDown], [Math.min(1, at(0) + 0.012), paleUp], [1, up]]
        : !pos ? [[0, down], [1, paleDown]] : [[0, paleUp], [1, up]];
      out += `<defs><linearGradient id="${gid}" x1="0" x2="1" y1="0" y2="0">${stops.map(([o, c]) => `<stop offset="${r2(o)}" stop-color="${c}"/>`).join('')}</linearGradient></defs>`;
      fill = `url(#${gid})`;
    }
    if (fill) out += P('axis', `<rect x="${r2(tx0)}" y="${r2(ty - th2 / 2)}" width="${r2(tw0)}" height="${r2(th2)}" rx="${r2(th2 / 2)}" fill="${fill}"${dots ? ' stroke="#00000014" stroke-width="0.6"' : ''}/>`);
    else if (S.trackStyle === 'split' && lo < 0 && hi > 0) {
      out += P('axis', rect(tx0, ty - th2 / 2, X(0) - tx0, th2, th2 / 2, { fill: mixHex(S.colorDown, th.page_bg, 0.55) }));
      out += P('axis', rect(X(0), ty - th2 / 2, tx0 + tw0 - X(0), th2, th2 / 2, { fill: mixHex(S.colorUp, th.page_bg, 0.55) }));
    } else out += P('axis', rect(tx0, ty - th2 / 2, tw0, th2, th2 / 2, { fill: mixHex(th.ink, th.page_bg, 0.86) }));
  }
  // ticks: thin page-coloured cuts through the track, labels just under it
  for (const k of ticks) {
    if (k.line) out += P('axis', `<rect x="${r2(k.x - 0.4)}" y="${r2(ty - th2 / 2)}" width="0.8" height="${r2(th2)}" fill="${th.page_bg}" opacity="0.9"/>`);
    else if (k.edge === 'middle') out += P('axis', `<rect x="${r2(k.x - 0.4)}" y="${r2(ty - th2 / 2 - 2)}" width="0.8" height="${r2(th2 + 4)}" fill="${th.muted}"/>`);
    out += P('axis', PT(k.x, ty + th2 / 2 + size * 1.35, k.t, pf('axis', -0.5, { weight: 700, fill: mixHex(th.muted, th.page_bg, 0.25), anchor: k.edge })));
  }
  const nameY = ty + th2 / 2 + (ticks.length ? size * 1.7 : 0) + size * 1.35;
  if (zero) {
    const zc = mixHex(th.ink, th.page_bg, 0.45);
    out += P('axis', `<rect x="${r2(X(0) - 0.7)}" y="${r2(ty - th2 / 2 - 3)}" width="1.4" height="${r2(th2 + 6)}" fill="${zc}"/>`);
    const zt = labeler(st, { ...S, decimals: 0 }, fmt, { full: true })(0, false);
    const zw = tw(zt, size - 0.5, 700) / 2 + 4;
    // its label only where no point's name sits
    const clash = S.nameLabels && inner.some((p) => Math.abs(X(p.v) - X(0)) < zw + tw(p.name, size) / 2);
    if (!clash) out += P('axis', T(X(0), nameY, zt, { size: size - 0.5, weight: 700, fill: zc, anchor: 'middle' }));
  }
  if (ends) {
    ends.forEach((p, i) => {
      const x = i ? x1 : x0, col = capOf(p);
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r2(endR)}" fill="${dots ? mixHex(col, th.page_bg, 0.5) : th.page_bg}" stroke="${dots ? ring : th.ink}" stroke-width="${dots ? 1.6 : 1.4}"/>`);
      const ex = i ? x1 + endR + 5 : x0 - endR - 5, anchor = i ? 'start' : 'end';
      out += P('labels', PT(ex, ty + endSize * 0.36, endTxt[i], pf('labels', 0, { size: endSize, weight: 800, fill: col, anchor })));
      if (S.nameLabels) out += P('labels', T(i ? b.x + b.w : ex, ty + endSize * 0.36 + size * 1.05, p.name.toUpperCase(), { size: size - 1.5, weight: 700, fill: mixHex(th.muted, th.page_bg, 0.2), anchor: 'end', tracking: 0.08 }));
    });
  }
  // labels alternate up and down a level when neighbours collide
  let lastRight = -Infinity, level = 0;
  for (const p of inner) {
    const x = X(p.v), on = p.name === hl;
    const vt = S.valueLabels ? (on ? hlLab : lab)(p.v, signed) : '';
    const vsize = on ? size + 1.5 : size;
    const wv = Math.max(tw(vt, vsize, 800), S.nameLabels ? tw(p.name, size) : 0);
    level = x - wv / 2 < lastRight + 3 ? (level + 1) % 2 : 0;
    lastRight = x + wv / 2;
    const r = on ? big : small;
    const col = on ? capOf(p) : dots ? mixHex(th.ink, th.page_bg, 0.45) : th.ink;
    if (dots) {
      // the highlighted point lifts off the track with a soft shadow
      const sid = `s${String(w.id || 'x').replace(/[^\w-]/g, '')}`;
      if (on) out += P(`series:${p.name}`, `<defs><filter id="${sid}" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="0" dy="1.2" stdDeviation="1.4" flood-color="#000" flood-opacity="0.3"/></filter></defs>`);
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r2(r)}" fill="${col}" stroke="${ring}" stroke-width="${on ? 2.4 : 1.6}"${on ? ` filter="url(#${sid})"` : ''}/>`);
    } else if (on) {
      // the client's own value: a dot in its result's colour inside a ring of paper, lifted off the track
      const c = p.v > 0 ? S.colorUp : p.v < 0 ? S.colorDown : th.ink;
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r2(r + 3)}" fill="${th.page_bg}"/>`);
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r2(r + 3)}" fill="none" stroke="${mixHex(c, th.page_bg, 0.75)}" stroke-width="0.8"/>`);
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r2(r)}" fill="${c}"/>`);
    } else {
      out += P(`series:${p.name}`, `<circle cx="${r2(x)}" cy="${r2(ty)}" r="${r}" fill="${S.markerStyle === 'solid' ? th.ink : th.page_bg}" stroke="${th.ink}" stroke-width="1.4"/>`);
    }
    const labCol = dots ? (on ? col : mixHex(th.ink, th.page_bg, 0.45)) : th.ink;
    if (vt) out += P('labels', PT(x, ty - Math.max(r, th2 / 2) - 4 - level * size * 1.2, vt, pf('labels', 0, { size: vsize, weight: 800, fill: labCol, anchor: 'middle' })));
    if (S.nameLabels) out += P('labels', T(x, (dots || zero ? nameY : ty + r + size * 1.25) + level * size * 1.2, p.name, { size: on ? size + 0.3 : size, weight: on ? 800 : dots ? 700 : 400, fill: dots ? labCol : on ? th.ink : th.muted, anchor: 'middle' }));
  }
  return out;
}
