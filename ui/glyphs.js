// Small line icons and flags for reports (drawn as SVG paths, so they export
// to PDF and PowerPoint exactly). Each icon is drawn on a 24-unit grid.

const r2 = (v) => Math.round(v * 100) / 100;

const PATHS = {
  cash: '<rect x="3" y="6" width="18" height="12" rx="2"/><circle cx="12" cy="12" r="2.6"/><path d="M6.5 9.5v5M17.5 9.5v5"/>',
  'trend-up': '<path d="M4 16l5-5 4 3 6-7"/><path d="M14 7h5v5"/>',
  'trend-down': '<path d="M4 8l5 5 4-3 6 7"/><path d="M14 17h5v-5"/>',
  layers: '<path d="M12 4l8 4-8 4-8-4z"/><path d="M4 12l8 4 8-4"/><path d="M4 16l8 4 8-4"/>',
  diamond: '<path d="M7 5h10l3 4-8 10L4 9z"/><path d="M4 9h16M9.5 5L12 19l2.5-14"/>',
  briefcase: '<rect x="3.5" y="7.5" width="17" height="11.5" rx="2"/><path d="M9 7.5V5.5h6v2M3.5 12.5h17"/>',
  percent: '<path d="M6 18L18 6"/><circle cx="7.5" cy="7.5" r="2.2"/><circle cx="16.5" cy="16.5" r="2.2"/>',
  grid: '<rect x="4.5" y="4.5" width="6" height="6" rx="1.2"/><rect x="13.5" y="4.5" width="6" height="6" rx="1.2"/><rect x="4.5" y="13.5" width="6" height="6" rx="1.2"/><rect x="13.5" y="13.5" width="6" height="6" rx="1.2"/>',
  doc: '<path d="M7 3.5h7l4 4V20a.5.5 0 0 1-.5.5h-10A.5.5 0 0 1 7 20z"/><path d="M14 3.5V8h4M9.5 12h6M9.5 15.5h6"/>',
  bars: '<path d="M5 19V12M10 19V7M15 19v-5M20 19V9"/>',
  swap: '<path d="M5 9h13l-3.5-3.5M19 15H6l3.5 3.5"/>',
  globe: '<circle cx="12" cy="12" r="9.5"/><path d="M2.5 12h19M12 2.5c3 3 4.2 6 4.2 9.5s-1.2 6.5-4.2 9.5c-3-3-4.2-6-4.2-9.5s1.2-6.5 4.2-9.5"/>',
  pie: '<path d="M12 4a8 8 0 1 0 8 8h-8z"/><path d="M14.5 2.5a7.5 7.5 0 0 1 7 7h-7z"/>',
  safe: '<rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="12" cy="12" r="3.5"/><path d="M12 8.5v1.2M12 14.3v1.2M8.5 12h1.2M14.3 12h1.2"/>',
  calendar: '<rect x="4" y="5.5" width="16" height="14" rx="2"/><path d="M4 10h16M8.5 3.5v4M15.5 3.5v4"/>',
  chart: '<path d="M4 19h16"/><path d="M5 15l4-4 3 2 6-6"/>',
  star: '<path d="M12 4l2.4 5 5.4.6-4 3.7 1.1 5.4L12 16l-4.9 2.7 1.1-5.4-4-3.7 5.4-.6z"/>',
  flag: '<path d="M6 20V4.5M6 5h11l-2 3.5 2 3.5H6"/>',
  coins: '<ellipse cx="12" cy="7" rx="6.5" ry="2.6"/><path d="M5.5 7v5c0 1.4 2.9 2.6 6.5 2.6s6.5-1.2 6.5-2.6V7M5.5 12v5c0 1.4 2.9 2.6 6.5 2.6s6.5-1.2 6.5-2.6v-5"/>',
  arrow: '<path d="M5 12h13M13 7l5 5-5 5"/>',
  dots: '<circle cx="8" cy="8" r="1.4"/><circle cx="16" cy="8" r="1.4"/><circle cx="8" cy="16" r="1.4"/><circle cx="16" cy="16" r="1.4"/>',
  wallet: '<path d="M4 7.5A2.5 2.5 0 0 1 6.5 5H18v3"/><rect x="4" y="7.5" width="16" height="11.5" rx="2"/><path d="M15.5 13.2h2"/>',
};

import { LINE, FLAGS, BRANDS } from './iconlib.js';
import { LOGOS } from './logolib.js';
import { legible, inkOn, contrastOf, mixHex } from './util.js';

/** Icons a picker offers first (the hand-drawn report set). */
export const GLYPH_NAMES = [...Object.keys(PATHS), 'buy', 'sell', 'in', 'out'];

/** Uploaded icons (logos) the document carries: id → { name, href }. */
let CUSTOM = {};
export function setCustomIcons(list) { CUSTOM = Object.fromEntries((list || []).map((c) => [c.id, c])); }
export const customIcons = () => Object.values(CUSTOM);

let seq = 0;
/** A clip path for the chip shape, and the matching outline. */
function chipClip(x, y, s, shape, stroke = null, sw = 0.5) {
  const id = `ic${++seq}`;
  const r = shape === 'circle' ? s / 2 : shape === 'sharp' ? 0 : s * 0.22;
  const geo = shape === 'circle' ? `<circle cx="${r2(x + s / 2)}" cy="${r2(y + s / 2)}" r="${r2(s / 2)}"/>` : `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(s)}" height="${r2(s)}" rx="${r2(r)}"/>`;
  return { id, def: `<clipPath id="${id}">${geo}</clipPath>`, ring: geo.replace('/>', ` fill="none" stroke="${stroke || '#00000024'}" stroke-width="${stroke ? sw : 0.5}"/>`) };
}

/** A country flag (or a company logo: `set` LOGOS), square, clipped to the chip shape (a soft square without one). */
function flag(code, x, y, s, shape, stroke = null, sw = 0.75, set = FLAGS) {
  const f = set[code];
  if (!f) return '';
  const [mx, my, w, hgt] = f.vb.split(/\s+/).map(Number);
  if (shape === 'none') {
    // no chip: a 4:3 flag, centred, without a frame (the square artwork cropped top and bottom)
    const fw = s, fh = s * 0.75, rx = x, ry = y + (s - fh) / 2;
    const c0 = chipClip(rx, ry, 0, 'sharp');
    const k0 = fw / w;
    return `<g><clipPath id="${c0.id}"><rect x="${r2(rx)}" y="${r2(ry)}" width="${r2(fw)}" height="${r2(fh)}"/></clipPath><g clip-path="url(#${c0.id})"><g transform="translate(${r2(rx - mx * k0)} ${r2(ry + (fh - hgt * k0) / 2 - my * k0)}) scale(${r2(k0 * 10000) / 10000})">${f.svg.replace(/__ID__/g, `${c0.id}_`)}</g></g></g>`;
  }
  const k = s / Math.max(w, hgt);
  const c = chipClip(x, y, s, shape, stroke, sw);
  const body = f.svg.replace(/__ID__/g, `${c.id}_`);
  return `<g>${c.def}<g clip-path="url(#${c.id})"><g transform="translate(${r2(x - mx * k)} ${r2(y - my * k)}) scale(${r2(k * 10000) / 10000})">${body}</g></g>${c.ring}</g>`;
}

/** A brand mark: white on its colour in a chip, or in its colour on its own. */
function brand(slug, x, y, s, shape, color, fill = null, stroke = null, sw = 0.75) {
  const b = BRANDS[slug];
  if (!b) return '';
  const chip = shape !== 'none';
  const inner = chip ? s * 0.58 : s;
  const k = inner / 24, ox = x + (s - inner) / 2, oy = y + (s - inner) / 2;
  // the brand's colour fills the chip unless the chip has its own background
  const bgc = fill || b.hex, ink = fill ? (color && color !== '#3f382b' ? color : b.hex) : '#ffffff';
  const line = stroke ? ` stroke="${stroke}" stroke-width="${sw}"` : '';
  const bg = chip ? (shape === 'circle' ? `<circle cx="${r2(x + s / 2)}" cy="${r2(y + s / 2)}" r="${r2(s / 2)}" fill="${bgc}"${line}/>` : `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(s)}" height="${r2(s)}" rx="${r2(shape === 'sharp' ? 0 : s * 0.22)}" fill="${bgc}"${line}/>`) : '';
  return `${bg}<path transform="translate(${r2(ox)} ${r2(oy)}) scale(${r2(k * 1000) / 1000})" d="${b.path}" fill="${chip ? ink : color && color !== '#3f382b' ? color : b.hex}"/>`;
}

/** An uploaded picture (a logo), fitted into the chip. */
function custom(id, x, y, s, shape, stroke = null, sw = 0.75) {
  const c = CUSTOM[id];
  if (!c) return '';
  const k = chipClip(x, y, s, shape === 'none' ? 'sharp' : shape, stroke, sw);
  return `<g>${k.def}<image href="${c.href}" x="${r2(x)}" y="${r2(y)}" width="${r2(s)}" height="${r2(s)}" preserveAspectRatio="xMidYMid ${shape === 'none' ? 'meet' : 'slice'}" clip-path="url(#${k.id})"/>${stroke && shape !== 'none' ? k.ring : ''}</g>`;
}

/** What a name stands for, for pickers and labels. */
export function iconLabel(name) {
  if (!name) return '';
  if (name.startsWith('flag-')) return FLAGS[name.slice(5)]?.name || name;
  if (name.startsWith('brand-')) return BRANDS[name.slice(6)]?.name || name;
  if (name.startsWith('logo-')) return LOGOS[name.slice(5)]?.name || name;
  if (name.startsWith('up-')) return CUSTOM[name.slice(3)]?.name || 'Uploaded';
  if (letterText(name)) return `Letters “${letterText(name)}”`;
  return name.replace(/-/g, ' ');
}

/**
 * The flag or brand a value names, if any: "AED", "UAE dirham", "Дирхам ОАЭ",
 * "Visa"… — whole words, in English or Russian. Used when a value has no icon
 * of its own and the column (or chart) finds icons by name.
 */
// each flag's words, as stems so Russian endings match ("рубля", "доллара")
const stem = (w) => (w.length > 5 ? w.slice(0, -2) : w.length > 3 ? w.slice(0, -1) : w);
// short words (codes such as AED, США) match whole; longer ones by their stem
const FLAG_WORDS = Object.entries(FLAGS).map(([code, f]) => [code, [...new Map(f.words.split(/\s+/).filter((w) => w.length > 1).map((w) => [stem(w), w.length <= 3])).entries()]]);
// each logo's words: names and tickers, comma-separated — a single word matches a whole token, a phrase anywhere
const LOGO_WORDS = Object.entries(LOGOS).map(([slug, l]) => [slug, [l.name, ...l.words.split(',')].map((w) => w.trim().toLowerCase().replace(/ё/g, 'е')).filter((w) => w.length > 1)]);
export function autoIcon(value) {
  const v = String(value ?? '').toLowerCase().replace(/ё/g, 'е');
  if (!v) return '';
  const tokens = v.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
  if (!tokens.length) return '';
  // a government bond named by its country: "RU-20290723-6.9", "US-20300115-4"
  if (tokens.length > 1 && tokens[0].length === 2 && /^\d/.test(tokens[1]) && FLAGS[tokens[0]]) return `flag-${tokens[0]}`;
  // a company: its name or ticker, in Russian or English ("Банк ВТБ", "SBER-20280425-13")
  for (const [slug, words] of LOGO_WORDS) if (words.some((w) => (/[^\p{L}\p{N}]/u.test(w) ? v.includes(w) : tokens.includes(w)))) return `logo-${slug}`;
  for (const [slug, b] of Object.entries(BRANDS)) { const n = b.name.toLowerCase(); if (v === n || tokens.includes(n)) return `brand-${slug}`; }
  // the flag whose words the value names most often wins: "Canadian dollar" is Canada, not the US
  let best = '', score = 0;
  for (const [code, stems] of FLAG_WORDS) {
    let n = 0;
    for (const [st, exact] of stems) if (tokens.some((t) => (exact ? t === st : t.startsWith(st)))) n += 1;
    if (n > score) { score = n; best = code; }
  }
  return best ? `flag-${best}` : '';
}

/** A letter badge (B = buy, S = sell, IN / OUT for cash flows). */
/**
 * Letters in a badge instead of an icon: any 1–8 (`txt:B`, `txt:IN`, `txt:CONVERT`), and the old names of the four the reports
 * started with (buy → B, sell → S, in → IN, out → OUT).
 */
const LETTERS = { buy: 'B', sell: 'S', in: 'IN', out: 'OUT' };
export function letterText(name) {
  if (typeof name !== 'string') return null;
  if (name.startsWith('txt:')) return [...name.slice(4)].slice(0, 8).join('') || null;
  return LETTERS[name] || null;
}

/** A letter badge: a round chip for one letter, a rounded bar for more; its letters in `ink` where it reads, else white or ink. */
function letter(t, x, y, s, bg, ink) {
  const n = [...t].length;
  // past four letters the type gets smaller and the letters closer, so a word stays a badge
  const per = n > 4 ? 0.36 : 0.42;
  const w = n > 1 ? s * (0.55 + per * n) : s;
  const fs = n > 4 ? s * 0.44 : n > 1 ? s * 0.5 : s * 0.62;
  // unset (or unreadable), on a dark chip white; on a light one a deep shade of the chip's own colour, as a tonal badge
  const hex = (c) => /^#[0-9a-f]{6}$/i.test(c || '');
  const tonal = (c) => { for (let t = 0.45; t <= 0.9; t += 0.05) { const d = mixHex(c, '#000000', t); if (contrastOf(d, c) >= 4.5) return d; } return '#000000'; };
  const fg = ink && hex(ink) && hex(bg) && contrastOf(ink, bg) >= 4.5 ? ink : hex(bg) ? (inkOn(bg) === '#ffffff' ? '#ffffff' : tonal(bg)) : '#ffffff';
  const esc2 = (v) => v.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
  // a plus or a minus is drawn, not set: as text it sits on the baseline, off the badge's middle
  if (n === 1 && /^[+\-−–]$/.test(t)) {
    const cx = x + w / 2, cy = y + s / 2, len = s * 0.46, th = Math.max(0.8, s * 0.12);
    const bar = (bx, by, bw, bh) => `<rect x="${r2(bx)}" y="${r2(by)}" width="${r2(bw)}" height="${r2(bh)}" rx="${r2(Math.min(bw, bh) / 2)}" fill="${fg}"/>`;
    return `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(w)}" height="${r2(s)}" rx="${r2(s / 2)}" fill="${bg}"/>${bar(cx - len / 2, cy - th / 2, len, th)}${t === '+' ? bar(cx - th / 2, cy - len / 2, th, len) : ''}`;
  }
  return `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(w)}" height="${r2(s)}" rx="${r2(n > 1 ? s * 0.22 : s / 2)}" fill="${bg}"/><text x="${r2(x + w / 2)}" y="${r2(y + s / 2 + fs * 0.36)}" font-family="Inter, Manrope" font-size="${r2(fs)}" font-weight="800" fill="${fg}" text-anchor="middle">${esc2(t)}</text>`;
}

/** Width a glyph takes at size `s` (letter badges of more than one letter are wider). */
export function glyphWidth(name, s) {
  const t = letterText(name);
  const n = t ? [...t].length : 1;
  return n > 1 ? s * (0.55 + (n > 4 ? 0.36 : 0.42) * n) : s;
}

/**
 * An icon at (x, y) of size s. `shape`: 'none' | 'circle' | 'square' (a
 * chip behind the icon, filled with `fill`). `color` is the stroke colour.
 */
export function glyph(name, x, y, s, { color = '#3f382b', fill = '#e9dfcc', shape = 'none', stroke = null, strokeWidth = null, brandFill = null } = {}) {
  if (!name) return '';
  // an outline given without a width: a hairline (the quiet automatic one sizes itself below)
  const sw0 = strokeWidth;
  strokeWidth = strokeWidth ?? 0.75;
  if (name.startsWith('flag-')) return flag(name.slice(5), x, y, s, shape, stroke, strokeWidth);
  if (name.startsWith('logo-')) return flag(name.slice(5), x, y, s, shape === 'none' ? 'circle' : shape, stroke, strokeWidth, LOGOS);
  if (name.startsWith('brand-')) return brand(name.slice(6), x, y, s, shape, color, brandFill, stroke, strokeWidth);
  if (name.startsWith('up-')) return custom(name.slice(3), x, y, s, shape, stroke, strokeWidth);
  // letters: on the chip's colour, in the mark's where it reads on it (as the column's settings show them); a
  // see-through chip leaves the mark's colour as the badge
  const lt = letterText(name);
  if (lt) return fill === 'transparent' || fill === 'none' ? letter(lt, x, y, s, color || '#3f382b', null) : letter(lt, x, y, s, fill, color);
  const p = PATHS[name] || LINE[name];
  if (!p) return '';
  // a see-through chip: whatever is behind shows, so the icon keeps its colour and the outline is a faint one of it
  const clear = fill === 'transparent' || fill === 'none';
  // an icon in a chip always shows on it (a theme may give both the same colour)
  if (shape !== 'none' && !clear) color = legible(color, fill);
  // a line icon in a chip has a quiet border of its own colour (flags and logos keep theirs);
  // "none" or false leaves it out
  let faint = '';
  if (stroke == null && shape !== 'none' && clear && /^#[0-9a-f]{6}$/i.test(color)) { stroke = color; faint = ' stroke-opacity="0.38"'; strokeWidth = sw0 ?? Math.max(0.5, s * 0.045); }
  if (stroke == null && shape !== 'none' && /^#[0-9a-f]{6}$/i.test(color) && /^#[0-9a-f]{6}$/i.test(fill)) { stroke = blend(color, fill, 0.62); strokeWidth = sw0 ?? Math.max(0.5, s * 0.045); }
  if (stroke === 'none' || stroke === false) stroke = null;
  const edge = stroke ? ` stroke="${stroke}" stroke-width="${strokeWidth}"${faint}` : '';
  const bg = clear ? 'none' : fill;
  let out = '';
  if (shape === 'circle') out += `<circle cx="${r2(x + s / 2)}" cy="${r2(y + s / 2)}" r="${r2(s / 2)}" fill="${bg}"${edge}/>`;
  else if (shape === 'square') out += `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(s)}" height="${r2(s)}" rx="${r2(s * 0.26)}" fill="${bg}"${edge}/>`;
  else if (shape === 'sharp') out += `<rect x="${r2(x)}" y="${r2(y)}" width="${r2(s)}" height="${r2(s)}" fill="${bg}"${edge}/>`;
  // in a chip the icon takes two-thirds of it, so it reads beside flags and logos that fill theirs
  const inner = shape === 'none' ? s : s * 0.76;
  const k = inner / 24;
  const ox = x + (s - inner) / 2, oy = y + (s - inner) / 2;
  out += `<g transform="translate(${r2(ox)} ${r2(oy)}) scale(${r2(k * 1000) / 1000})" fill="none" stroke="${color}" stroke-width="${r2(Math.max(1.5, (shape === 'none' ? 0.7 : 0.5) / k))}" stroke-linecap="round" stroke-linejoin="round">${p}</g>`;
  return out;
}

/** The icon a value maps to: exact match, then case-insensitive, then a default. */
export function glyphFor(map, value, fallback = '', auto = false) {
  if (!map) return (auto && autoIcon(value)) || fallback;
  const v = String(value ?? '');
  if (map[v]) return map[v];
  const lv = v.toLowerCase();
  for (const [k, g] of Object.entries(map)) if (k.toLowerCase() === lv) return g;
  for (const [k, g] of Object.entries(map)) if (k && lv.includes(k.toLowerCase())) return g;
  return (auto && autoIcon(value)) || fallback;
}

/** `a` moved toward `b` by `k` (0 = a, 1 = b), for #rrggbb colours. */
function blend(a, b, k) {
  const p = (c) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return '#' + x.map((v, i) => Math.round(v + (y[i] - v) * k).toString(16).padStart(2, '0')).join('');
}
