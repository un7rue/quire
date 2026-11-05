// Rich text in comments and text blocks: the icons a line can carry, how a
// line with sizes and icons is drawn, and the editor that writes the markup
// Rust lays out:
//   **bold**  *italic*  {#rrggbb|colour}  {$good|theme colour}
//   {+|larger}  {++|much larger}  {-|smaller}  {!up} an icon
// Spans nest: {$good|{!up} **+6,48%**}.

import { esc, textWidth, fontStack, tokenColor } from './util.js';
import { icon } from './icons.js';

const r2 = (v) => Math.round(v * 100) / 100;

/**
 * Icons for a line of text: drawn in a 24×24 box (the screen and the PDF), a
 * character for PowerPoint, and the colour they take when nothing colours them.
 */
export const TEXT_ICONS = {
  up: { label: 'Up', ch: '▲', tone: 'good', d: '<path d="M12 5 21 19H3z" fill="currentColor"/>' },
  down: { label: 'Down', ch: '▼', tone: 'bad', d: '<path d="M12 19 3 5h18z" fill="currentColor"/>' },
  'trend-up': { label: 'Rising', ch: '↗', tone: 'good', d: '<path d="M4 18 10 12l4 4 6-8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M14.5 6.5H21V13" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>' },
  'trend-down': { label: 'Falling', ch: '↘', tone: 'bad', d: '<path d="M4 6l6 6 4-4 6 8" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M14.5 17.5H21V11" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>' },
  right: { label: 'Arrow', ch: '▶', tone: '', d: '<path d="M6 4 20 12 6 20z" fill="currentColor"/>' },
  check: { label: 'Done', ch: '✓', tone: 'good', d: '<path d="M4 12.5 9.5 18 20 6" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>' },
  cross: { label: 'No', ch: '✕', tone: 'bad', d: '<path d="M6 6l12 12M18 6 6 18" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>' },
  warn: { label: 'Warning', ch: '⚠', tone: 'warn', d: '<path d="M12 3 22.5 21h-21z" fill="currentColor"/><path d="M12 9.5v5M12 17.6v.4" stroke="#fff" stroke-width="2.4" stroke-linecap="round"/>' },
  star: { label: 'Star', ch: '★', tone: 'accent', d: '<path d="m12 3 2.7 6 6.3.6-4.8 4.3 1.4 6.4L12 17l-5.6 3.3 1.4-6.4L3 9.6 9.3 9z" fill="currentColor"/>' },
  dot: { label: 'Dot', ch: '●', tone: '', d: '<circle cx="12" cy="12" r="6.5" fill="currentColor"/>' },
  info: { label: 'Note', ch: 'ⓘ', tone: '', d: '<circle cx="12" cy="12" r="10" fill="currentColor"/><path d="M12 11v6M12 7.4v.4" stroke="#fff" stroke-width="2.4" stroke-linecap="round"/>' },
  question: { label: 'Question', ch: '?', tone: 'question', d: '<circle cx="12" cy="12" r="10" fill="currentColor"/><path d="M9.2 9.3a2.9 2.9 0 1 1 4.2 2.6c-.9.5-1.4 1-1.4 2v.4M12 17.4v.3" fill="none" stroke="#fff" stroke-width="2.3" stroke-linecap="round"/>' },
  important: { label: 'Important', ch: '!', tone: 'accent', d: '<circle cx="12" cy="12" r="10" fill="currentColor"/><path d="M12 6.8v6.4M12 16.8v.4" stroke="#fff" stroke-width="2.6" stroke-linecap="round"/>' },
  comment: { label: 'Comment', ch: '💬', tone: 'ink', d: '<path d="M4 5.5A2.5 2.5 0 0 1 6.5 3h11A2.5 2.5 0 0 1 20 5.5v8a2.5 2.5 0 0 1-2.5 2.5H10l-4.5 4v-4h0A1.5 1.5 0 0 1 4 14.5z" fill="currentColor"/><path d="M8 8.2h8M8 11.4h5" stroke="#fff" stroke-width="1.8" stroke-linecap="round"/>' },
};

/** The marks a note can carry on its badge: what kind of note it is. */
export const NOTE_MARKS = [['', 'None'], ['question', 'Question'], ['important', 'Important'], ['warn', 'Warning'], ['comment', 'Comment']];

/** Soft backgrounds for a note: neutral (the theme's card) first. */
export const NOTE_BACKGROUNDS = [['', 'Neutral'], ['#f7f0e1', 'Warm'], ['#e9f1ea', 'Mint'], ['#e8eff6', 'Sky'], ['#efecf6', 'Lavender'], ['#ffffff', 'White']];

/** An icon's colour: its run's, else its own tone in the theme, else the text's. */
export function iconColor(th, name, runColor, textColor) {
  if (runColor) return runColor;
  const t = TEXT_ICONS[name]?.tone;
  return toneColor(th, t) || textColor;
}

/** An icon tone in the theme ("question" is a calm blue whatever the theme). */
export function toneColor(th, t) {
  if (t === 'question') return '#4f7ea8';
  if (t === 'warn') return '#c08a1e';
  return (t && th?.[t]) || null;
}

/** One icon of a line, its box `size` points high, with its baseline at `y`. */
export function iconSvg(name, x, y, size, color) {
  const ic = TEXT_ICONS[name] || TEXT_ICONS.dot;
  const s = size * 0.82;
  const top = y - size * 0.74;
  return `<svg x="${r2(x + (size * 1.1 - s) / 2)}" y="${r2(top + (size * 0.8 - s) / 2)}" width="${r2(s)}" height="${r2(s)}" viewBox="0 0 24 24" color="${esc(color)}" style="overflow:visible">${ic.d}</svg>`;
}

/** Whether a line's runs need drawing piece by piece (an icon or another size). */
export const needsPieces = (runs) => runs.some((r) => r.icon || (r.size && r.size !== 1));

/**
 * A line of styled runs drawn piece by piece from `x` on the baseline `y`.
 * `o`: the text's font (size, weight, family, italic, fill, tracking); `th` for icon colours.
 */
export function piecesSvg(x, y, runs, o, th) {
  let out = '';
  let cx = x;
  const bw = (o.weight || 400) >= 600 ? 800 : 700;
  for (const r of runs) {
    const size = o.size * (r.size || 1);
    if (r.icon) {
      out += iconSvg(r.icon, cx, y, size, iconColor(th, r.icon, r.color, o.fill));
      cx += size * 1.1;
      continue;
    }
    const weight = r.bold ? bw : o.weight || 400;
    const italic = r.italic ? !o.italic : !!o.italic;
    out += `<text x="${r2(cx)}" y="${r2(y)}" font-family="${esc(fontStack(o.family || 'Inter'))}" font-size="${r2(size)}" font-weight="${weight}"${italic ? ' font-style="italic"' : ''} fill="${r.color || o.fill}" xml:space="preserve"${o.tracking ? ` letter-spacing="${r2(o.tracking * size)}"` : ''}>${esc(r.text)}</text>`;
    cx += textWidth(r.text, size, weight, o.family || 'Inter', italic, o.tracking || 0);
  }
  return out;
}

// ---------------------------------------------------------------- the editor

/** The colours offered in the editor: the theme's, as `$name` so they follow the theme. */
export function editorColours(th) {
  const named = [['$ink', 'Text'], ['$good', 'Positive'], ['$bad', 'Negative'], ['$accent', 'Accent'], ['$muted', 'Muted text']];
  const seen = new Set();
  // one swatch per colour (a theme's accent is often its red)
  return [...named.map(([k, l]) => [k, l, th[k.slice(1)]]), ...(th.palette || []).slice(0, 6).map((c, i) => [`$series${i + 1}`, `Series ${i + 1}`, c])]
    .filter(([, , c]) => c && !seen.has(String(c).toLowerCase()) && seen.add(String(c).toLowerCase()));
}

const ICON_ORDER = ['question', 'important', 'warn', 'comment', 'info', 'check', 'cross', 'up', 'down', 'trend-up', 'trend-down', 'star', 'dot', 'right'];

const SIZES = { '+': 5, '++': 6, '-': 2 };
const SIZE_OF = { 5: '+', 6: '++', 2: '-', 1: '-' };

/** Markup → HTML for a contenteditable box. */
export function toHtml(src, th) {
  const col = (c) => tokenColor(th, c);
  const out = [];
  // spans and icons
  const rich = (s) => {
    let html = '';
    let i = 0;
    while (i < s.length) {
      const at = s.indexOf('{', i);
      if (at < 0) { html += emph(s.slice(i)); break; }
      html += emph(s.slice(i, at));
      const icon = /^\{!([a-z0-9-]{1,24})\}/.exec(s.slice(at));
      if (icon) {
        html += chip(icon[1]);
        i = at + icon[0].length;
        continue;
      }
      const m = /^\{(#[0-9a-fA-F]{3,8}|\$[A-Za-z0-9_]+)?(\+\+|\+|-)?\|/.exec(s.slice(at));
      const end = m && (m[1] || m[2]) ? closing(s, at + m[0].length) : -1;
      if (end < 0) { html += esc('{'); i = at + 1; continue; }
      let inner = rich(s.slice(at + m[0].length, end));
      if (m[2]) inner = `<font size="${SIZES[m[2]]}">${inner}</font>`;
      if (m[1]) inner = `<font color="${esc(col(m[1]))}" data-c="${esc(m[1])}">${inner}</font>`;
      html += inner;
      i = end + 1;
    }
    return html;
  };
  const emph = (s) => {
    if (!s) return '';
    const marks = (s.match(/\*/g) || []).length;
    if (marks < 2) return esc(s);
    let html = esc(s).replace(/\*\*(.+?)\*\*/g, '<b>$1</b>').replace(/(^|[^*])\*([^*]+?)\*(?!\*)/g, '$1<i>$2</i>');
    return html;
  };
  for (const line of String(src || '').replace(/\r\n/g, '\n').split('\n')) out.push(rich(line) || '<br>');
  return out.map((l) => `<div>${l}</div>`).join('');
}

function closing(s, from) {
  let depth = 0;
  for (let k = from; k < s.length; k++) {
    if (s[k] === '{') depth++;
    else if (s[k] === '}') { if (!depth) return k; depth--; }
  }
  return -1;
}

/** An icon as it shows in the editor. */
export function chip(name) {
  const ic = TEXT_ICONS[name] || TEXT_ICONS.dot;
  return `<span class="rt-ic" contenteditable="false" data-i="${esc(name)}" data-tone="${esc(ic.tone)}">${esc(ic.ch)}</span>`;
}

/** The editor's HTML → markup. Theme colours stay `$name`; others become #hex. */
export function toMarkup(root, th) {
  const hexOf = (c) => {
    if (!c) return null;
    if (c.startsWith('#')) return c.toLowerCase();
    const m = /rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(c);
    return m ? '#' + [m[1], m[2], m[3]].map((x) => (+x).toString(16).padStart(2, '0')).join('') : null;
  };
  const named = Object.fromEntries(['ink', 'good', 'bad', 'accent', 'muted'].map((k) => [String(th[k] || '').toLowerCase(), '$' + k]));
  const walk = (n) => {
    if (n.nodeType === 3) return n.nodeValue.replace(/ /g, ' ').replace(/\*/g, '\\*');
    if (n.nodeType !== 1) return '';
    if (n.classList?.contains('rt-ic')) return `{!${n.dataset.i}}`;
    const tag = n.tagName.toLowerCase();
    if (tag === 'br') return '\n';
    let inner = [...n.childNodes].map(walk).join('');
    if (!inner) return tag === 'div' || tag === 'p' ? '\n' : '';
    if (tag === 'b' || tag === 'strong' || (n.style?.fontWeight && +n.style.fontWeight >= 600) || n.style?.fontWeight === 'bold') inner = wrapEmph(inner, '**');
    if (tag === 'i' || tag === 'em' || n.style?.fontStyle === 'italic') inner = wrapEmph(inner, '*');
    const size = tag === 'font' ? SIZE_OF[n.getAttribute('size')] : null;
    if (size) inner = `{${size}|${inner}}`;
    const c = n.dataset?.c || (tag === 'font' ? n.getAttribute('color') : null) || n.style?.color;
    if (c) {
      const hex = c.startsWith('$') ? c : hexOf(c);
      const k = hex && (c.startsWith('$') ? c : named[hex] || hex);
      if (k && k !== '$ink') inner = `{${k}|${inner}}`;
    }
    if ((tag === 'div' || tag === 'p') && n.previousSibling) inner = '\n' + inner;
    return inner;
  };
  return [...root.childNodes].map(walk).join('').replace(/\n{3,}/g, '\n\n').replace(/^\n+|\n+$/g, '');
}

// bold or italic around the text, its edge spaces kept outside ("** a **" isn't bold)
function wrapEmph(s, m) {
  const lead = s.match(/^\s*/)[0], trail = s.match(/\s*$/)[0];
  const core = s.slice(lead.length, s.length - trail.length);
  return core ? `${lead}${m}${core}${m}${trail}` : s;
}

/**
 * A rich text editor: a toolbar (bold, italic, colour, size, icon) over an
 * editable box. `onCommit(markup)` runs when the text changes and the box
 * loses focus (or ⌘↵).
 */
export function richEditor(value, th, onCommit, { placeholder = '', minH = 90, iconsOpen = false } = {}) {
  const el = document.createElement('div');
  el.className = 'rt';
  const cols = editorColours(th);
  el.innerHTML = `<div class="rt-bar">
      <button data-cmd="bold" title="Bold (⌘B)"><b>B</b></button>
      <button data-cmd="italic" title="Italic (⌘I)"><i>I</i></button>
      <span class="rt-sep"></span>
      <span class="rt-cols">${cols.map(([k, l, c]) => `<button data-col="${esc(k)}" title="${esc(l)}"><i style="background:${esc(c)}"></i></button>`).join('')}</span>
      <span class="rt-sep"></span>
      <button data-size="2" title="Smaller">A−</button>
      <button data-size="3" title="Normal size">A</button>
      <button data-size="5" title="Larger">A+</button>
      <span class="rt-sep"></span>
      ${iconsOpen ? '' : `<button data-a="icons" title="Insert an icon">${icon('sparkles', 14)}</button>`}
    </div>
    <div class="rt-icons"${iconsOpen ? '' : ' hidden'}>${iconsOpen ? '<span class="rt-lab">Icons</span>' : ''}${ICON_ORDER.map((k) => [k, TEXT_ICONS[k]]).map(([k, v]) => `<button data-icon="${k}" title="${esc(v.label)}"><svg viewBox="0 0 24 24" width="15" height="15" color="${esc(toneColor(th, v.tone) || th.ink)}">${v.d}</svg></button>`).join('')}</div>
    <div class="rt-box" contenteditable="true" spellcheck="true" data-ph="${esc(placeholder)}" style="min-height:${minH}px"></div>`;
  const box = el.querySelector('.rt-box');
  box.innerHTML = toHtml(value, th);
  // icons shown in their colour
  const tint = () => box.querySelectorAll('.rt-ic').forEach((n) => { if (!n.closest('font[color]')) n.style.color = toneColor(th, n.dataset.tone) || ''; });
  tint();
  let last = String(value ?? '');
  const commit = () => {
    const v = toMarkup(box, th);
    if (v === last) return;
    last = v;
    onCommit(v);
  };
  const keep = (e) => e.preventDefault(); // keep the selection when clicking the bar
  el.querySelector('.rt-bar').addEventListener('mousedown', keep);
  el.querySelector('.rt-icons').addEventListener('mousedown', keep);
  const exec = (cmd, arg) => { box.focus(); document.execCommand('styleWithCSS', false, false); document.execCommand(cmd, false, arg); tint(); };
  el.querySelectorAll('[data-cmd]').forEach((b) => b.addEventListener('click', () => exec(b.dataset.cmd)));
  el.querySelectorAll('[data-size]').forEach((b) => b.addEventListener('click', () => exec('fontSize', b.dataset.size)));
  el.querySelectorAll('[data-col]').forEach((b) => b.addEventListener('click', () => {
    const k = b.dataset.col;
    const c = tokenColor(th, k);
    exec('foreColor', c);
    // remember theme colours by name, so they follow the theme
    if (k.startsWith('$')) box.querySelectorAll('font[color]').forEach((f) => { if ((f.getAttribute('color') || '').toLowerCase() === String(c).toLowerCase()) f.dataset.c = k; });
  }));
  const pop = el.querySelector('.rt-icons');
  el.querySelector('[data-a="icons"]')?.addEventListener('click', () => { pop.hidden = !pop.hidden; });
  el.querySelectorAll('[data-icon]').forEach((b) => b.addEventListener('click', () => {
    box.focus();
    document.execCommand('insertHTML', false, chip(b.dataset.icon) + '&nbsp;');
    tint();
    if (!iconsOpen) pop.hidden = true;
  }));
  box.addEventListener('blur', () => setTimeout(() => { if (!el.contains(document.activeElement)) commit(); }, 0));
  box.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); commit(); box.blur(); }
  });
  el.commit = commit;
  el.markup = () => toMarkup(box, th);
  el.focusEnd = () => {
    box.focus();
    const r = document.createRange();
    r.selectNodeContents(box);
    r.collapse(false);
    const s = getSelection();
    s.removeAllRanges();
    s.addRange(r);
  };
  return el;
}

// ---------------------------------------------------------------- the note window

/**
 * The editor window for a note or a text block: the words with their toolbar,
 * and for a note its background, its mark and whether it is signed.
 * Resolves to a patch for the widget, or null when cancelled.
 */
export async function textWindow(w, th, { modal, canStyle = true } = {}) {
  const note = w.kind === 'comment';
  const o = w.opts || {};
  const body = document.createElement('div');
  body.className = 'modal-body tw';
  const ed = richEditor(o.text ?? '', th, () => {}, { minH: 190, iconsOpen: true, placeholder: note ? 'Write to your client…' : 'Write here…' });
  body.append(ed);
  const state = { mark: o.mark || '', background: w.style?.background || '', signed: !!o.signed, arrows: !!o.arrows };
  const side = document.createElement('div');
  side.className = 'tw-side';
  const draw = () => {
    side.innerHTML = `
      ${note && canStyle ? `<div class="tw-row"><label>Background</label><div class="tw-bgs">${NOTE_BACKGROUNDS.map(([c, l]) => `<button data-bg="${c}" class="${c === state.background ? 'on' : ''}" title="${l}"><i style="background:${c || th.card_bg}"></i><span>${l}</span></button>`).join('')}</div></div>` : ''}
      ${note ? `<div class="tw-row"><label>Mark</label><div class="tw-marks">${NOTE_MARKS.map(([k, l]) => `<button data-mark="${k}" class="${k === state.mark ? 'on' : ''}" title="${l}">${k ? `<svg viewBox="0 0 24 24" width="16" height="16" color="${toneColor(th, TEXT_ICONS[k].tone) || th.ink}">${TEXT_ICONS[k].d}</svg>` : '<b>—</b>'}<span>${l}</span></button>`).join('')}</div></div>` : ''}
      <div class="tw-row tw-checks">
        ${note ? `<label class="tw-check"><input type="checkbox" data-k="signed"${state.signed ? ' checked' : ''}><span class="tw-ct"><b>Signed by you</b><small>Your initials, name and the date</small></span></label>` : ''}
        <label class="tw-check"><input type="checkbox" data-k="arrows"${state.arrows ? ' checked' : ''}><span class="tw-ct"><b>Arrows on numbers</b><small>▲ +6,48%  ▼ −2,1%, coloured</small></span></label>
      </div>`;
    side.querySelectorAll('[data-bg]').forEach((b) => b.addEventListener('click', () => { state.background = b.dataset.bg; draw(); }));
    side.querySelectorAll('[data-mark]').forEach((b) => b.addEventListener('click', () => { state.mark = b.dataset.mark; draw(); }));
    side.querySelectorAll('[data-k]').forEach((b) => b.addEventListener('change', () => { state[b.dataset.k] = b.checked; }));
  };
  draw();
  body.append(side);
  setTimeout(() => ed.focusEnd(), 30);
  const ok = await modal({ title: note ? 'Note to the client' : 'Text', iconName: note ? 'comment' : 'paragraph', body, wide: true,
    buttons: [{ label: 'Cancel', value: false }, { label: 'Save', kind: 'primary', value: true }] });
  if (!ok) return null;
  const patch = { opts: { text: ed.markup(), arrows: state.arrows || null } };
  if (note) {
    patch.opts.signed = state.signed || null;
    patch.opts.mark = state.mark || null;
    if (canStyle) patch.style = { background: state.background || null };
  }
  return patch;
}
