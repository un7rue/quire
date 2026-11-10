// Small shared helpers.

export const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

export function esc(s) {
  return String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

export function h(html) {
  const t = document.createElement('template');
  t.innerHTML = html.trim();
  return t.content.firstElementChild;
}

export const isMac = typeof navigator !== 'undefined' && (navigator.platform.toLowerCase().includes('mac') || navigator.userAgent.includes('Mac'));
export const mod = (e) => (isMac ? e.metaKey : e.ctrlKey);

export function debounce(fn, ms) {
  let t = null;
  const d = (...a) => {
    clearTimeout(t);
    t = setTimeout(() => { t = null; fn(...a); }, ms);
  };
  d.flush = (...a) => { if (t) { clearTimeout(t); t = null; fn(...a); } };
  return d;
}

export const clone = (v) => JSON.parse(JSON.stringify(v));

// ---------------------------------------------------------------- text measurement (Inter, same files as Rust)

// without a browser (the server drawing pages for the report API) the core measures text
const measure = globalThis.quireMeasure;
let ctx = null;

/** CSS / SVG font-family list for a report font, falling back to Inter. */
export const fontStack = (family) => (!family || family === 'Inter' ? 'Inter' : `'${String(family).replace(/['"\\]/g, '')}', Inter`);

/** Width in points; `tracking` is letter spacing in em. */
export function textWidth(s, size, weight = 400, family = 'Inter', italic = false, tracking = 0) {
  s = String(s);
  let w;
  if (measure) w = measure(s, +size, +weight || 400, String(family || 'Inter'), !!italic);
  else {
    ctx ??= document.createElement('canvas').getContext('2d');
    ctx.font = `${italic ? 'italic ' : ''}${weight} ${size}px ${fontStack(family)}`;
    w = ctx.measureText(s).width;
  }
  return w + (tracking ? tracking * size * [...s].length : 0);
}

export function ellipsize(s, max, size, weight = 400, family = 'Inter', italic = false, tracking = 0) {
  s = String(s ?? '');
  if (textWidth(s, size, weight, family, italic, tracking) <= max) return s;
  let lo = 0, hi = s.length;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (textWidth(s.slice(0, mid) + '…', size, weight, family, italic, tracking) <= max) lo = mid;
    else hi = mid - 1;
  }
  return lo ? s.slice(0, lo).trimEnd() + '…' : '';
}

// ---------------------------------------------------------------- number formatting

// the report's locale (set from the theme before each render)
let LOC = 'en-US';
/** The document's language and its format choices (empty ones follow the language). */
let FMT = {};
export function setLocale(l, formats) { LOC = l || 'en-US'; FMT = formats || {}; }
export const getFormats = () => FMT;

const SEPS = { space: '\u00a0', thin: '\u202f', ',': ',', '.': '.', "'": '’', none: '' };
const locSeps = {};
/** A number as the language writes it, with the document's separators swapped in. */
function seps(str) {
  if (!FMT.thousands && !FMT.decimal) return str;
  if (!locSeps[LOC]) {
    const parts = new Intl.NumberFormat(LOC).formatToParts(12345.6);
    locSeps[LOC] = [parts.find((p) => p.type === 'group')?.value ?? ',', parts.find((p) => p.type === 'decimal')?.value ?? '.'];
  }
  const [g, d] = locSeps[LOC];
  const G = FMT.thousands ? SEPS[FMT.thousands] ?? FMT.thousands : g;
  const D = FMT.decimal || d;
  let out = '';
  for (const ch of str) out += ch === g ? '\u0000' : ch === d ? '\u0001' : ch;
  return out.replace(/\u0000/g, G).replace(/\u0001/g, D);
}
/** Negative numbers with a minus, or in brackets when the document asks for it. */
const negWrap = (neg, body) => (neg && FMT.negative === 'paren' ? `(${body})` : `${neg ? '−' : ''}${body}`);
export const getLocale = () => LOC;
const SUFFIX_CURRENCY = /^(ru|uk|be|kk|de|fr|es|it|pl|cs|sk|fi|sv|nb|da|pt|nl|hu|ro|tr)/;
const COMPACT = {
  ru: [[1e12, ' трлн'], [1e9, ' млрд'], [1e6, ' млн'], [1e3, ' тыс']],
  de: [[1e12, ' Bio.'], [1e9, ' Mrd.'], [1e6, ' Mio.'], [1e3, ' Tsd.']],
  en: [[1e12, 'T'], [1e9, 'B'], [1e6, 'M'], [1e3, 'K']],
};

export function fmtNum(v, format = 'number', { compact = false, currency = '$', signed = false, decimals = null } = {}) {
  if (v === null || v === undefined || Number.isNaN(v)) return '—';
  if (typeof v !== 'number') return String(v);
  const sign = signed && v > 0 ? '+' : '';
  const fixed = decimals != null && decimals !== '' ? { minimumFractionDigits: +decimals, maximumFractionDigits: +decimals } : null;
  if (format === 'percent') {
    const p = v * 100;
    const d = Math.abs(p) >= 100 ? 0 : compact ? (Math.abs(p) >= 10 ? 0 : 1) : 1;
    return sign + negWrap(v < 0, seps(Math.abs(p).toLocaleString(LOC, fixed || { minimumFractionDigits: d, maximumFractionDigits: d })) + '%');
  }
  const a = Math.abs(v);
  let body;
  if (compact || format === 'compact') {
    const units = COMPACT[LOC.slice(0, 2)] || COMPACT.en;
    const u = units.find(([n]) => a >= n);
    if (u) {
      const x = a / u[0];
      body = x.toLocaleString(LOC, fixed || { maximumFractionDigits: x >= 100 ? 0 : x >= 10 ? 1 : 2, minimumFractionDigits: LOC.startsWith('ru') && x < 100 ? 1 : 0 }) + u[1];
    } else body = a.toLocaleString(LOC, fixed || { maximumFractionDigits: a >= 100 ? 0 : 2 });
  } else {
    const d = a >= 1000 || Number.isInteger(v) ? 0 : 2;
    body = a.toLocaleString(LOC, fixed || { minimumFractionDigits: format === 'currency' && d ? 2 : 0, maximumFractionDigits: d });
  }
  body = seps(body);
  const cur = FMT.currency || currency;
  const after = FMT.currency_pos ? FMT.currency_pos === 'after' : SUFFIX_CURRENCY.test(LOC);
  const money = format !== 'currency' ? body : after ? `${body} ${cur}` : `${cur}${body}`;
  return v < 0 ? negWrap(true, money) : `${sign}${money}`;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
export const isIsoDate = (v) => typeof v === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(v);
const MONTHS_RU = ['янв', 'фев', 'мар', 'апр', 'май', 'июн', 'июл', 'авг', 'сен', 'окт', 'ноя', 'дек'];
const MONTHS_DE = ['Jan', 'Feb', 'Mär', 'Apr', 'Mai', 'Jun', 'Jul', 'Aug', 'Sep', 'Okt', 'Nov', 'Dez'];
/** A month's short name in the report's language (1–12). */
export const monthShort = (m) => (LOC.startsWith('ru') ? MONTHS_RU : LOC.startsWith('de') ? MONTHS_DE : MONTHS)[m - 1] || '';

export function fmtDate(v) {
  if (!isIsoDate(v)) return String(v ?? '');
  const [y, m, d] = v.split('-').map(Number);
  const p2 = (n) => String(n).padStart(2, '0');
  const mon = (LOC.startsWith('ru') ? MONTHS_RU : LOC.startsWith('de') ? MONTHS_DE : MONTHS)[m - 1];
  switch (FMT.date) {
    case 'dd.mm.yyyy': return `${p2(d)}.${p2(m)}.${y}`;
    case 'dd/mm/yyyy': return `${p2(d)}/${p2(m)}/${y}`;
    case 'mm/dd/yyyy': return `${p2(m)}/${p2(d)}/${y}`;
    case 'yyyy-mm-dd': return v;
    case 'd mmm yyyy': return `${d} ${mon} ${y}`;
    case 'mmm d, yyyy': return `${mon} ${d}, ${y}`;
    default: break;
  }
  if (!LOC.startsWith('en')) return `${String(d).padStart(2, '0')}.${String(m).padStart(2, '0')}.${y}`;
  return `${d} ${MONTHS[m - 1]} ${y}`;
}

/** Text vs. dark ink on a fill colour. */
export function inkOn(hex) {
  const n = parseInt(String(hex || '#ffffff').replace('#', '').padEnd(6, 'f').slice(0, 6), 16);
  const r = (n >> 16) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
  const L = 0.2126 * r ** 2.2 + 0.7152 * g ** 2.2 + 0.0722 * b ** 2.2;
  return L > 0.36 ? '#0b0b0b' : '#ffffff';
}

const lum = (hex) => {
  const n = parseInt(hex.slice(1), 16);
  const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
  return 0.2126 * f(n >> 16) + 0.7152 * f((n >> 8) & 255) + 0.0722 * f(n & 255);
};
const isHex = (c) => /^#[0-9a-f]{6}$/i.test(String(c || ''));

/** Contrast ratio of two #rrggbb colours (1–21), or null when either isn't one. */
export function contrastOf(a, b) {
  if (!isHex(a) || !isHex(b)) return null;
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/** `fg` when it reads on `bg` (at least `min`:1), else black or white, whichever reads better. */
export function legible(fg, bg, min = 3) {
  const c = contrastOf(fg, bg);
  if (c == null || c >= min) return fg;
  return contrastOf('#000000', bg) >= contrastOf('#ffffff', bg) ? '#000000' : '#ffffff';
}

/**
 * A setting made here: its name is marked, and a double-click on the name puts it back to what it
 * follows (the design view, the theme, the palette…) — no reset button.
 */
export function resettable(row, nameEl, reset, what = 'the default') {
  if (!nameEl) return;
  row?.classList.add('vz-set');
  nameEl.classList.add('dbl-reset');
  nameEl.title = `${nameEl.title ? `${nameEl.title} · ` : ''}Set here — double-click to go back to ${what}`;
  nameEl.addEventListener('dblclick', (e) => { e.preventDefault(); e.stopPropagation(); window.getSelection?.()?.removeAllRanges(); reset(); });
}

/** Linear blend of two hex colours, t in 0..1. */
export function mixHex(a, b, t) {
  const p = (h) => { const n = parseInt(String(h).replace('#', '').slice(0, 6), 16); return [n >> 16, (n >> 8) & 255, n & 255]; };
  const x = p(a), y = p(b);
  return '#' + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, '0')).join('');
}

/** Round axis ticks: 0 / 1,000 / 2,000 … */
export function niceTicks(min, max, count = 5) {
  if (!(isFinite(min) && isFinite(max))) return [0, 1];
  if (min === max) { max = min === 0 ? 1 : min * 1.2; if (min > 0) min = 0; }
  const span = max - min;
  const raw = span / Math.max(1, count);
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => span / s <= count) || 10 * mag;
  const lo = Math.floor(min / step) * step;
  const hi = Math.ceil(max / step) * step;
  const out = [];
  for (let v = lo; v <= hi + step * 1e-9; v += step) out.push(Math.round(v / step) * step);
  return out;
}

// ---------------------------------------------------------------- the theme's named colours

/** The theme's main colours by the names its Colours section gives them; a value `$name` follows the theme. */
export const NAMED_COLOURS = [['$accent', 'Accent'], ['$ink', 'Text'], ['$muted', 'Muted text'], ['$good', 'Positive'], ['$bad', 'Negative'],
  ['$page', 'Page'], ['$card', 'Cards'], ['$grid', 'Lines & grid'], ['$cardBorder', 'Card outline']];

/** Names for the theme's other colours (its design views use them). */
const TOKEN_NAMES = { card2: 'Second card', ink2: 'Text 2', body: 'Body text', accentLine: 'Accent line', gainTint: 'Gain tint', lossTint: 'Loss tint',
  gainMid: 'Gain, medium', lossMid: 'Loss, medium', received: 'Received', iconInk: 'Icon', chip: 'Icon chip', share: 'Share bar', forecast: 'Forecast',
  interest: 'Interest', dividend: 'Dividends', coupon: 'Coupons', income: 'Income', principal: 'Principal', dividendSoft: 'Dividends, soft',
  couponSoft: 'Coupons, soft', incomeSoft: 'Income, soft', donut: 'Donut', frame: 'Frame', rule: 'Rules', coverFill: 'Cover', backFill: 'Closing page',
  headFill: 'Table header', headInk: 'Table header text', zebra: 'Striped rows' };

/** A `$name` colour as the theme has it (anything else unchanged). */
export function tokenColor(th, c) {
  if (typeof c !== 'string' || !c.startsWith('$')) return c;
  const k = c.slice(1);
  const base = { good: th.good, bad: th.bad, accent: th.accent, ink: th.ink, muted: th.muted, grid: th.grid, page: th.page_bg, card: th.card_bg, cardBorder: th.card_border || th.grid };
  const series = /^series(\d+)$/.exec(k);
  if (series) return th.palette?.[+series[1] - 1] || '#888888';
  if (base[k]) return base[k];
  const t = th.tokens?.[k];
  // a derived colour may be a main colour by name, which it follows
  if (typeof t === 'string' && t.startsWith('$') && !th.tokens[t.slice(1)]) return tokenColor(th, t);
  return t || '#888888';
}

/** What a colour value is called: "Accent", "Series 2", "Gain tint"… (null: not a theme colour). */
export function colourName(c) {
  if (typeof c !== 'string' || !c.startsWith('$')) return null;
  const k = c.slice(1);
  const main = NAMED_COLOURS.find(([v]) => v === c);
  if (main) return main[1];
  const series = /^series(\d+)$/.exec(k);
  if (series) return `Series ${series[1]}`;
  const m = /^(class|block)(\d+)$/.exec(k);
  if (m) return `${m[1] === 'class' ? 'Asset class' : 'Block'} ${m[2]}`;
  return TOKEN_NAMES[k] || k;
}

/** Every colour the theme names: [value, name, colour] groups — main, series and the theme's others. */
export function themeColours(th) {
  const main = NAMED_COLOURS.map(([v, n]) => [v, n, tokenColor(th, v)]);
  const series = (th.palette || []).map((c, i) => [`$series${i + 1}`, `Series ${i + 1}`, c]);
  const more = Object.keys(th.tokens || {}).map((k) => [`$${k}`, colourName(`$${k}`), th.tokens[k] ? tokenColor(th, `$${k}`) : ''])
    .filter(([, , c]) => /^#[0-9a-f]{6}$/i.test(c));
  return { main, series, more };
}
