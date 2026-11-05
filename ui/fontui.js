// Font settings UI: the family picker, per-role font controls, and making
// sure every family a report uses is loaded before text is measured.

import { S, call, edit, render } from './state.js';
import { icon } from './icons.js';
import { esc, h, fontStack, resettable } from './util.js';
import { popover, closeLayer } from './ui.js';
import { hexInput } from './colorpick.js';

export const ROLE_INFO = {
  heading: 'Headings',
  heading_sub: 'Heading subtitles',
  eyebrow: 'Eyebrows (above headings)',
  title: 'Widget titles',
  subtitle: 'Widget subtitles',
  body: 'Text & comments',
  caption: 'Captions & bylines',
  kpi: 'KPI values',
  table_head: 'Table headers',
  table_cell: 'Table cells',
  chart: 'Chart text (all parts)',
  axis: 'Axis labels',
  data_label: 'Data labels',
  legend: 'Legends',
  footer: 'Page footer',
};

/** Roles that matter for each widget kind. */
export const KIND_ROLES = {
  kpi: ['title', 'subtitle', 'kpi', 'caption'],
  bar: ['title', 'subtitle', 'chart', 'axis', 'data_label', 'legend'],
  line: ['title', 'subtitle', 'chart', 'axis', 'data_label', 'legend'],
  area: ['title', 'subtitle', 'chart', 'axis', 'data_label', 'legend'],
  pie: ['title', 'subtitle', 'chart', 'legend'],
  table: ['title', 'subtitle', 'table_head', 'table_cell'],
  heading: ['eyebrow', 'heading', 'heading_sub', 'caption'],
  text: ['body'],
  comment: ['caption', 'body'],
  treemap: ['title', 'subtitle', 'chart'],
  waterfall: ['title', 'subtitle', 'chart', 'axis', 'data_label', 'legend'],
  tiles: ['title', 'subtitle', 'kpi', 'caption'],
  scale: ['title', 'subtitle', 'chart'],
  toc: ['title', 'body', 'caption'],
};

const WEIGHTS = { 100: 'Thin', 200: 'Extra light', 300: 'Light', 400: 'Regular', 500: 'Medium', 600: 'Semibold', 700: 'Bold', 800: 'Extra bold', 900: 'Black' };

let families = null;
export async function familyList(force = false) {
  if (!families || force) families = await call('list_fonts', {}, { quiet: true }).catch(() => []);
  return families;
}

// ---------------------------------------------------------------- loading

const ready = new Set(['Inter']);
const registered = new Set();

/** Loads every family the current state uses; re-renders once if any were new. */
export async function ensureFonts(st) {
  if (!st?.doc) return;
  const wanted = new Set();
  const add = (f) => f?.family && wanted.add(f.family);
  Object.values(st.fonts || {}).forEach(add);
  for (const p of st.pages || []) for (const w of p.widgets) Object.values(w.fonts || {}).forEach(add);
  for (const p of st.doc.pages) for (const w of p.widgets) for (const c of Object.values(w.opts?.table?.cols || {})) if (c.family) wanted.add(c.family);
  // fonts embedded in the document come as files
  const embedded = (st.doc.fonts || []).map((f) => f.family);
  if (embedded.some((f) => !registered.has(f))) {
    const files = await call('font_files', {}, { quiet: true }).catch(() => []);
    for (const f of files) {
      if (registered.has(f.family)) continue;
      try {
        const bytes = Uint8Array.from(atob(f.data), (c) => c.charCodeAt(0));
        const face = new FontFace(f.family, bytes.buffer);
        await face.load();
        document.fonts.add(face);
        registered.add(f.family);
      } catch (e) { console.warn('font', f.family, e); }
    }
  }
  const missing = [...wanted].filter((f) => !ready.has(f));
  if (!missing.length) return;
  await Promise.all(missing.flatMap((f) => [400, 700].map((wt) => document.fonts.load(`${wt} 12px ${fontStack(f)}`).catch(() => null))));
  missing.forEach((f) => ready.add(f));
  render();
}

// ---------------------------------------------------------------- picker

export async function pickFamily(anchor, current, onPick) {
  const list = await familyList();
  const el = h(`<div class="menu font-pop">
    <input class="font-search" placeholder="Search ${list.length} fonts…" spellcheck="false">
    <div class="font-list"></div>
    <div class="font-foot"></div>
  </div>`);
  const box = el.querySelector('.font-list');
  const draw = (q) => {
    const ql = q.trim().toLowerCase();
    const items = list.filter((f) => !ql || f.family.toLowerCase().includes(ql));
    box.replaceChildren(...items.slice(0, 400).map((f) => {
      const b = h(`<button class="font-item${f.family === current ? ' on' : ''}" title="${esc(f.family)}">
        <span class="fi-name" style="font-family:${esc(fontStack(f.family))}">${esc(f.family)}</span>
        <span class="fi-meta">${f.embedded ? 'in document · ' : ''}${f.weights.length > 1 ? `${f.weights.length} weights` : WEIGHTS[f.weights[0]] || ''}${f.italic ? ' · italic' : ''}</span>
      </button>`);
      b.addEventListener('click', () => { closeLayer(); onPick(f.family); });
      return b;
    }));
    if (!items.length) box.append(h(`<div class="hint" style="padding:10px">No font matches “${esc(q)}”.</div>`));
  };
  draw('');
  const inp = el.querySelector('.font-search');
  inp.addEventListener('input', () => draw(inp.value));
  inp.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter') box.querySelector('.font-item')?.click();
    if (e.key === 'Escape') closeLayer();
  });
  if (!S.st.doc.kind || S.st.doc.kind === 'template') {
    const add = h(`<button class="btn sm">${icon('upload', 13)} Add a font file…</button>`);
    add.addEventListener('click', async () => {
      closeLayer();
      const fam = await addFontFile();
      if (fam) onPick(fam);
    });
    el.querySelector('.font-foot').append(add, h(`<span class="hint">Embedded in the template so it renders the same anywhere.</span>`));
  }
  popover(anchor, el);
  setTimeout(() => { inp.focus(); el.querySelector('.font-item.on')?.scrollIntoView({ block: 'center' }); });
}

export async function addFontFile() {
  const p = await call('pick_data_file', { kind: 'font' });
  if (!p) return null;
  const before = new Set((S.st.doc.fonts || []).map((f) => f.family));
  const st = await edit({ op: 'add_font', path: p });
  if (!st) return null;
  await familyList(true);
  const added = (st.doc.fonts || []).find((f) => !before.has(f.family)) || st.doc.fonts?.[st.doc.fonts.length - 1];
  return added?.family || null;
}

// ---------------------------------------------------------------- controls

/**
 * One role's font settings. `spec` holds only what is set here; `resolved`
 * is the effective font (shown as placeholders). onChange(patch) gets a
 * partial spec, or null to reset.
 */
export function fontControl(key, spec, resolved, onChange, { colorField, allowReset = true } = {}) {
  spec = spec || {};
  // labelled rows: every setting visible and named, none squeezed into one line
  const el = h(`<div class="font-ctl" data-key="${key}">
    <label class="fc-l">Font</label>
    <button class="fc-family" title="Font family"><span style="font-family:${esc(fontStack(resolved.family))}">${esc(resolved.family)}</span>${spec.family ? '' : '<em>from theme</em>'}${icon('chevd', 12)}</button>
    <label class="fc-l">Size</label>
    <div class="fc-row"><span class="fc-unit"><input class="fc-size" data-key="${key}.size" type="number" min="4" max="120" step="1" value="${spec.size ?? ''}" placeholder="${resolved.size ? +resolved.size.toFixed(1) : resolved.autoSize ? `auto · ${resolved.autoSize}` : 'auto'}" title="Size"><i>pt</i></span>
      <select class="fc-weight" title="Weight"></select></div>
    <label class="fc-l">Style</label>
    <div class="fc-row">
      <button class="fc-tog${(spec.italic ?? resolved.italic) ? ' on' : ''}" data-k="italic" title="Italic"><i>I</i> Italic</button>
      <button class="fc-tog${(spec.upper ?? resolved.upper) ? ' on' : ''}" data-k="upper" title="All capitals">AA Capitals</button>
    </div>
    <label class="fc-l">Spacing</label>
    <div class="fc-row"><span class="fc-unit"><input class="fc-track" data-key="${key}.track" type="number" min="-0.1" max="0.5" step="0.01" value="${spec.tracking ?? ''}" placeholder="${resolved.tracking ? +resolved.tracking.toFixed(2) : '0'}" title="Letter spacing"><i>em</i></span><span class="fc-hint">between letters</span></div>
    <label class="fc-l">Colour</label>
    <div class="fc-row fc-color"></div>
  </div>`);
  el.querySelector('.fc-family').addEventListener('click', (e) => pickFamily(e.currentTarget, resolved.family, (f) => onChange({ family: f })));
  const size = el.querySelector('.fc-size');
  size.addEventListener('change', () => onChange({ size: size.value === '' ? null : Math.max(4, Math.min(120, +size.value)) }));
  size.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') size.blur(); });
  const track = el.querySelector('.fc-track');
  track.addEventListener('change', () => onChange({ tracking: track.value === '' ? null : Math.max(-0.1, Math.min(0.5, +track.value)) }));
  track.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') track.blur(); });
  const weight = el.querySelector('.fc-weight');
  familyList().then((list) => {
    const fam = list.find((f) => f.family === resolved.family);
    const avail = fam?.weights?.length ? fam.weights : [400, 700];
    const ws = [...new Set([...avail, resolved.weight])].sort((a, b) => a - b);
    weight.innerHTML = ws.map((w) => `<option value="${w}"${w === resolved.weight ? ' selected' : ''}>${WEIGHTS[w] || w}${avail.includes(w) ? '' : ' (synthetic)'}</option>`).join('');
  });
  weight.addEventListener('change', () => onChange({ weight: +weight.value }));
  el.querySelectorAll('.fc-tog').forEach((b) => b.addEventListener('click', () => {
    const k = b.dataset.k;
    onChange({ [k]: !(spec[k] ?? resolved[k]) });
  }));
  if (colorField) {
    el.querySelector('.fc-color').append(colorField(`${key}.color`, spec.color || null, (c) => onChange({ color: c }), { title: `Colour (now ${resolved.color})` }),
      hexInput(spec.color || '', (c) => onChange({ color: c })), h(`<span class="fc-hint">${spec.color ? '' : `theme: ${esc(resolved.color || '')}`}</span>`));
  }
  // a setting made here: double-click its name to follow the theme again
  if (allowReset) {
    const keysOf = { Font: ['family'], Size: ['size', 'weight'], Style: ['italic', 'upper'], Spacing: ['tracking'], Colour: ['color'] };
    el.querySelectorAll('.fc-l').forEach((l) => {
      const ks = keysOf[l.textContent.trim()] || [];
      if (ks.some((k) => spec[k] != null)) resettable(null, l, () => onChange(Object.fromEntries(ks.map((k) => [k, null]))), 'the theme');
    });
  }
  return el;
}

/** A compact list of roles; each expands into its control. */
export function rolesEditor(roles, getSpec, getResolved, onChange, opts) {
  const wrap = h(`<div class="roles"></div>`);
  for (const role of roles) {
    const r = getResolved(role);
    if (!r) continue;
    const spec = getSpec(role) || {};
    const set = Object.keys(spec).some((k) => spec[k] != null);
    const item = h(`<details class="role${set ? ' set' : ''}" data-role="${role}">
      <summary><span class="ro-sample" style="font-family:${esc(fontStack(r.family))};font-weight:${r.weight};${r.italic ? 'font-style:italic;' : ''}color:${esc(r.color)}">${esc(r.upper ? 'AG' : 'Ag')}</span>
        <span class="ro-txt"><span class="ro-name">${esc(ROLE_INFO[role] || role)}</span>
        <span class="ro-meta">${esc(r.family)} · ${r.size ? +r.size.toFixed(1) : 'auto'} pt · ${WEIGHTS[r.weight] || r.weight}${r.upper ? ' · capitals' : ''}${r.italic ? ' · italic' : ''}</span></span>
        ${set ? '<span class="ro-own" title="Set for this widget">own</span>' : ''}${icon('chevd', 12)}</summary>
    </details>`);
    if (OPEN.has(role)) item.open = true;
    item.addEventListener('toggle', () => (item.open ? OPEN.add(role) : OPEN.delete(role)));
    // the whole role set here: double-click its name to follow the theme again
    if (set) resettable(null, item.querySelector('.ro-name'), () => onChange(role, null), 'the theme');
    item.append(fontControl(`role.${role}`, spec, r, (patch) => onChange(role, patch), opts));
    wrap.append(item);
  }
  return wrap;
}
const OPEN = new Set();
