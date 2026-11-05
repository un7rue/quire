// The theme studio's workspace: a rail of object types on the left and, in
// the middle, either the theme overview (colours and type) or one gallery of
// design views for the chosen type. Each view is previewed on its own with
// the theme's sample data, cropped to what it draws.

import { S, edit, render } from './state.js';
import { widgetSvg } from './render.js';
import { icon } from './icons.js';
import { esc, h, fontStack } from './util.js';
import { menu, modal, toast } from './ui.js';
import { ROLE_INFO } from './fontui.js';

/** Object types by family: [kind, name, icon, what the views are for]. */
export const STUDIO_GROUPS = [
  ['Figures', [
    ['kpi', 'KPI cards', 'kpi', 'Headline figures: a value with its change, a note, a breakdown or a trend.'],
    ['table', 'Tables', 'table', 'Exact figures by category, with grouping, totals and per-column styling.'],
  ]],
  ['Charts', [
    ['bar', 'Bar charts', 'chart-bar', 'Compare categories or periods, rank items, split a total into parts, or place points on one range.'],
    ['line', 'Line charts', 'chart-line', 'Trends over time.'],
    ['area', 'Area charts', 'chart-area', 'Volume over time.'],
    ['pie', 'Donut charts', 'chart-pie', 'Shares of a total.'],
    ['waterfall', 'Waterfalls', 'waterfall', 'Steps that add up to a total.'],
    ['treemap', 'Treemaps', 'treemap', 'Shares shown as areas.'],
    ['tiles', 'Tiles', 'tiles', 'A card for each category.'],
  ]],
  ['Text & structure', [
    ['heading', 'Headings', 'heading', 'Page and section titles, with eyebrows, badges and rules.'],
    ['text', 'Text', 'paragraph', 'Paragraphs and notes without a card.'],
    ['comment', 'Comments', 'comment', 'Commentary with an author and an accent bar.'],
    ['divider', 'Dividers', 'divider', 'Thin rules between sections.'],
    ['toc', 'Contents', 'toc', 'Tables of contents built from the report’s sections and pages.'],
  ]],
];
const KIND_INFO = Object.fromEntries(STUDIO_GROUPS.flatMap(([, ks]) => ks.map((k) => [k[0], k])));
/** Singular names, for buttons and dialogs. */
const ONE = { kpi: 'KPI card', table: 'table', bar: 'bar chart', line: 'line chart', area: 'area chart', pie: 'donut chart', waterfall: 'waterfall', treemap: 'treemap', scale: 'dot scale', tiles: 'tiles view', heading: 'heading', text: 'text block', comment: 'comment', divider: 'divider', toc: 'contents style' };

/** The type the studio shows: "overview" or a widget kind. */
export function studioKind() {
  const sel = S.sel?.match(/^sv_([a-z]+)_/)?.[1];
  if (sel && KIND_INFO[sel]) S.studioKind = sel;
  if (!S.studioKind || (S.studioKind !== 'overview' && !KIND_INFO[S.studioKind])) S.studioKind = 'overview';
  return S.studioKind;
}

const views = (kind) => S.st.doc.theme.styles?.[kind] || [];

/** Where each preview sits in the studio document. */
function previewIndex() {
  const out = new Map();
  S.st.doc.pages.forEach((p, pi) => p.widgets.forEach((w, wi) => out.set(w.id, [pi, wi])));
  return out;
}

/** Back to the theme overview, where it was left. */
export function backToOverview() {
  S.studioKind = 'overview';
  S.sel = null;
  render();
  const top = S.studioOverviewTop || 0;
  requestAnimationFrame(() => document.querySelector('#canvas')?.scrollTo({ top }));
}

export function selectView(kind, id) {
  S.studioKind = kind;
  S.sel = id ? `sv_${kind}_${id}` : null;
  render();
  if (id) requestAnimationFrame(() => document.querySelector(`.sv-card[data-id="${CSS.escape(S.sel)}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' }));
}

// ---------------------------------------------------------------- left rail

export function studioRail() {
  const kind = studioKind();
  const total = Object.values(S.st.doc.theme.styles || {}).reduce((a, l) => a + l.length, 0);
  const el = h(`<div class="st-rail">
    <div class="st-rail-head"><span>Theme</span><span class="st-total">${total} design views</span></div>
    <div class="st-rail-body"></div>
  </div>`);
  const body = el.querySelector('.st-rail-body');
  const item = (k, name, ic, sub, count) => {
    const b = h(`<button class="st-item${kind === k ? ' on' : ''}" data-k="${k}">
      <span class="st-ic">${icon(ic, 16)}</span>
      <span class="st-lbl"><b>${esc(name)}</b>${sub ? `<span>${esc(sub)}</span>` : ''}</span>
      ${count != null ? `<span class="st-count">${count}</span>` : ''}
    </button>`);
    b.addEventListener('click', () => { if (S.studioKind !== k || S.sel) { S.studioKind = k; S.sel = null; render(); document.querySelector('#canvas')?.scrollTo({ top: 0 }); } });
    return b;
  };
  body.append(item('overview', 'Overview', 'palette', 'Colours and typography'));
  for (const [group, kinds] of STUDIO_GROUPS) {
    body.append(h(`<div class="st-group">${esc(group)}</div>`));
    for (const [k, name, ic] of kinds) body.append(item(k, name, ic, null, views(k).length));
  }
  return el;
}

// ---------------------------------------------------------------- workspace

export function renderStudio(host) {
  const kind = studioKind();
  const keep = host.dataset.studio === kind ? host.scrollTop : 0;
  host.dataset.studio = kind;
  host.classList.add('studio');
  const page = kind === 'overview' ? overviewPage() : kindPage(kind);
  host.replaceChildren(page);
  cropPreviews(page);
  host.scrollTop = keep;
}

/** Fits each preview's height to what it draws; widths stay true to scale. */
function cropPreviews(root) {
  root.querySelectorAll('svg[data-frame]').forEach((svg) => {
    const g = svg.querySelector('.pv-content');
    if (!g) return;
    let bb;
    try { bb = g.getBBox(); } catch { return; }
    if (!bb || !bb.height) return;
    const [fx, fy, fw, fh] = svg.dataset.frame.split(' ').map(Number);
    const pad = +svg.dataset.pad || 0;
    const y0 = Math.max(fy, bb.y), y1 = Math.min(fy + fh, bb.y + bb.height);
    if (y1 <= y0) return;
    svg.setAttribute('viewBox', `${fx - pad} ${y0 - pad} ${fw + pad * 2} ${y1 - y0 + pad * 2}`);
    svg.querySelector(':scope > rect')?.setAttribute('y', y0 - pad);
    svg.querySelector(':scope > rect')?.setAttribute('height', y1 - y0 + pad * 2);
  });
}

function preview(id, idx, pad = 10) {
  const at = idx.get(id);
  if (!at) return '';
  try { return widgetSvg(S.st, at[0], at[1], { pad }).svg; } catch (e) { console.error(e); return ''; }
}

function kindPage(kind) {
  const [, name, ic, about] = KIND_INFO[kind];
  const list = views(kind);
  const idx = previewIndex();
  const el = h(`<div class="st-page">
    <button class="st-back" title="Back to the theme overview (Backspace)">${icon('chevl', 15)} Overview</button>
    <header class="st-head">
      <span class="st-hic">${icon(ic, 22)}</span>
      <div class="st-ht"><h1>${esc(name)}</h1><p>${esc(about)} <span class="st-dim">${list.length} design view${list.length === 1 ? '' : 's'} · the first is the default for new ${esc(name.toLowerCase())}.</span></p></div>
      <button class="btn primary st-new">${icon('plus', 15)} New design view</button>
    </header>
    <div class="st-grid k-${kind}"></div>
  </div>`);
  el.querySelector('.st-new').addEventListener('click', () => newViewDialog(kind));
  el.querySelector('.st-back').addEventListener('click', backToOverview);
  const grid = el.querySelector('.st-grid');
  list.forEach((v, i) => {
    const id = `sv_${kind}_${v.id}`;
    const card = h(`<article class="sv-card${S.sel === id ? ' on' : ''}" data-id="${esc(id)}" tabindex="0">
      <div class="sv-prev" style="background:${esc(S.st.doc.theme.page_bg)}">${preview(id, idx)}</div>
      <div class="sv-meta">
        <div class="sv-row"><span class="sv-name">${esc(v.name)}</span>${i === 0 ? `<span class="sv-badge">${icon('star', 11)} Default</span>` : ''}
          <span class="grow"></span>
          <button class="sv-btn dup" title="Duplicate">${icon('duplicate', 15)}</button>
          <button class="sv-btn more" title="More">${icon('more', 15)}</button>
        </div>
        ${v.description ? `<div class="sv-desc">${esc(v.description)}</div>` : ''}
      </div>
    </article>`);
    card.addEventListener('click', (e) => { if (e.target.closest('.sv-btn')) return; if (S.sel !== id) { S.sel = id; render(); } });
    card.addEventListener('keydown', (e) => { if (e.key === 'Enter') { S.sel = id; render(); } });
    card.querySelector('.dup').addEventListener('click', () => duplicateView(kind, v));
    card.querySelector('.more').addEventListener('click', (e) => viewMenu(e.currentTarget, kind, v, i));
    card.addEventListener('contextmenu', (e) => { e.preventDefault(); viewMenu({ x: e.clientX, y: e.clientY }, kind, v, i); });
    grid.append(card);
  });
  const add = h(`<button class="sv-card sv-add"><span class="sv-add-ic">${icon('plus', 20)}</span><b>New ${esc(ONE[kind])} design</b><span>Start from scratch or from one of the views above</span></button>`);
  add.addEventListener('click', () => newViewDialog(kind));
  grid.append(add);
  return el;
}

// ---------------------------------------------------------------- view actions

export async function duplicateView(kind, v) {
  const st = await edit({ op: 'add_variant', kind, name: `${v.name} copy`, from: v.id, widget: null });
  const nv = st?.doc.theme.styles?.[kind]?.slice(-1)[0];
  if (nv) { selectView(kind, nv.id); toast(`Duplicated as “${nv.name}”`); }
}

export async function deleteView(kind, v) {
  if (views(kind).length < 2) { toast('A type needs at least one design view.', 'info'); return; }
  const ok = await modal({ title: 'Delete design view', iconName: 'warn', iconKind: 'warn', body: `<p>Delete “${esc(v.name)}”? Templates that use it keep their own copy of its settings.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
  if (!ok) return;
  if (S.sel === `sv_${kind}_${v.id}`) S.sel = null;
  await edit({ op: 'remove_variant', kind, id: v.id });
}

export async function renameView(kind, v) {
  const name = await askText('Rename design view', 'Name', v.name);
  if (name && name !== v.name) await edit({ op: 'update_variant', kind, id: v.id, name, description: null, sample: null });
}

export function viewMenu(anchor, kind, v, i) {
  const n = views(kind).length;
  menu(anchor, [
    { label: 'Edit', icon: 'sliders', run: () => selectView(kind, v.id) },
    { label: 'Rename…', icon: 'rename', run: () => renameView(kind, v) },
    { label: 'Duplicate', icon: 'duplicate', run: () => duplicateView(kind, v) },
    { sep: true },
    { label: 'Make default', icon: 'star', desc: 'New widgets of this type start with it', disabled: i === 0, run: () => edit({ op: 'move_variant', kind, id: v.id, to: 0 }) },
    { label: 'Move earlier', icon: 'chevl', disabled: i === 0, run: () => edit({ op: 'move_variant', kind, id: v.id, to: i - 1 }) },
    { label: 'Move later', icon: 'chevr', disabled: i >= n - 1, run: () => edit({ op: 'move_variant', kind, id: v.id, to: i + 1 }) },
    { sep: true },
    { label: 'Delete', icon: 'trash', danger: true, disabled: n < 2, run: () => deleteView(kind, v) },
  ]);
}

function askText(title, label, value) {
  const f = h(`<div class="form"><label>${esc(label)}<input name="v" value="${esc(value || '')}"></label></div>`);
  setTimeout(() => { const i = f.querySelector('input'); i?.focus(); i?.select(); }, 30);
  return modal({ title, body: f, buttons: [{ label: 'Cancel', value: null }, { label: 'OK', kind: 'primary', value: (b) => b.querySelector('[name=v]').value.trim() || null }] });
}

/** New design view: a name, what it's for, and what it starts from. */
export async function newViewDialog(kind, from) {
  const list = views(kind);
  const idx = previewIndex();
  let pick = from === undefined ? (list[0]?.id ?? '') : from || '';
  const body = h(`<div class="nv">
    <div class="nv-fields">
      <label>Name<input name="name" placeholder="e.g. Hero figure" autocomplete="off"></label>
      <label>When to use it<input name="desc" placeholder="Optional — shown under the preview" autocomplete="off"></label>
    </div>
    <div class="nv-label">Start from</div>
    <div class="nv-grid k-${kind}"></div>
    <div class="nv-hint">${icon('info', 14)}<span>The new view copies every setting of the one you pick. Change anything afterwards in the panel on the right; widgets in templates pick it under <b>Design view</b>.</span></div>
  </div>`);
  const grid = body.querySelector('.nv-grid');
  const opt = (id, label, inner, sub) => {
    const b = h(`<button class="nv-opt${pick === id ? ' on' : ''}" data-id="${esc(id)}"><span class="nv-prev" style="background:${esc(S.st.doc.theme.page_bg)}">${inner}</span><span class="nv-name">${esc(label)}</span>${sub ? `<span class="nv-sub">${esc(sub)}</span>` : ''}</button>`);
    b.addEventListener('click', () => { pick = id; grid.querySelectorAll('.nv-opt').forEach((x) => x.classList.toggle('on', x === b)); });
    b.addEventListener('dblclick', () => body.closest('.modal')?.querySelector('.modal-foot .primary')?.click());
    grid.append(b);
  };
  opt('', 'Blank', `<span class="nv-blank">${icon(KIND_INFO[kind][2], 26)}</span>`, 'The theme’s plain look');
  list.forEach((v, i) => opt(v.id, v.name, preview(`sv_${kind}_${v.id}`, idx, 6), i === 0 ? 'Default' : ''));
  setTimeout(() => { body.querySelector('[name=name]')?.focus(); cropPreviews(body); }, 30);
  const res = await modal({
    title: `New ${ONE[kind]} design view`, iconName: KIND_INFO[kind][2], wide: true, body,
    buttons: [{ label: 'Cancel', value: null }, { label: 'Create design view', kind: 'primary', value: (b) => ({ name: b.querySelector('[name=name]').value.trim(), desc: b.querySelector('[name=desc]').value.trim() }) }],
  });
  if (!res) return;
  const src = list.find((v) => v.id === pick);
  const name = res.name || (src ? `${src.name} copy` : `New ${ONE[kind]}`);
  const st = await edit({ op: 'add_variant', kind, name, from: pick || null, widget: null });
  const nv = st?.doc.theme.styles?.[kind]?.slice(-1)[0];
  if (!nv) return;
  // a copy describes itself, not its source
  if (res.desc || nv.description) await edit({ op: 'update_variant', kind, id: nv.id, name: null, description: res.desc, sample: null });
  selectView(kind, nv.id);
  toast(`“${nv.name}” added. Adjust it in the panel on the right.`);
}

// ---------------------------------------------------------------- overview

const SAMPLE_TEXT = {
  eyebrow: 'Personal report', heading: 'Portfolio overview', heading_sub: 'as of 30 June 2026',
  title: 'Result by category', subtitle: 'Six months to June', kpi: '354 266 000',
  body: 'Returns were driven by deposits, while equities and metals lost value over the period.',
  caption: 'Source: custodian statements', table_head: 'Value · Result · Return', table_cell: 'Deposits   273 674 000   +3.9%',
  chart: 'Jan  Feb  Mar  Apr  ·  +7.28%', footer: 'Test Private Banking · page 3 of 18',
};

function overviewPage() {
  const th = S.st.doc.theme;
  const fonts = S.st.fonts || {};
  const head = fonts.heading || fonts.title || {};
  const el = h(`<div class="st-page ov">
    <header class="st-head">
      <span class="st-hic">${icon('palette', 22)}</span>
      <div class="st-ht"><h1>${esc(th.name || 'Untitled theme')}</h1><p>Colours, typography and design views that templates load. Edit colours and fonts in the panel on the right; pick an object type on the left to work on its design views.</p></div>
    </header>
    <div class="ov-hero" style="background:${esc(th.page_bg)}">
      <div class="ov-eyebrow" style="${fontCss(fonts.eyebrow, th.accent)}">${esc(SAMPLE_TEXT.eyebrow)}</div>
      <div class="ov-title" style="font-family:${esc(fontStack(head.family || th.font_family))};font-weight:${head.weight || 400};color:${esc(head.color || th.ink)}">Report on assets <i>and returns</i></div>
      <div class="ov-rule" style="background:${esc(th.accent)}"></div>
      <div class="ov-body" style="${fontCss(fonts.body, th.muted)}">${esc(SAMPLE_TEXT.body)}</div>
      <div class="ov-chips">${[th.ink, th.accent, th.good, th.bad, ...th.palette.slice(0, 6)].map((c) => `<i style="background:${esc(c)}"></i>`).join('')}</div>
    </div>
    <section class="ov-sec"><h2>${icon('drop', 15)} Colours</h2><div class="ov-colors"></div>
      <h3>Series</h3><div class="ov-series">${th.palette.map((c, i) => `<div class="ov-ser"><i style="background:${esc(c)}"></i><span>${i + 1}</span><code>${esc(c)}</code></div>`).join('')}</div>
    </section>
    <section class="ov-sec"><h2>${icon('type', 15)} Typography</h2><div class="ov-type" style="background:${esc(th.page_bg)}"></div></section>
    <section class="ov-sec"><h2>${icon('gallery', 15)} Object types</h2><div class="ov-kinds"></div></section>
  </div>`);
  const colours = [['Page', th.page_bg], ['Cards', th.card_bg], ['Text', th.ink], ['Muted text', th.muted], ['Accent', th.accent], ['Positive', th.good], ['Negative', th.bad], ['Lines & grid', th.grid], ['Card outline', th.card_border]];
  const cg = el.querySelector('.ov-colors');
  for (const [name, c] of colours) if (c) cg.append(h(`<div class="ov-col"><i style="background:${esc(c)}"></i><span><b>${esc(name)}</b><code>${esc(c)}</code></span></div>`));
  const tg = el.querySelector('.ov-type');
  for (const role of ['eyebrow', 'heading', 'heading_sub', 'title', 'subtitle', 'kpi', 'body', 'table_head', 'table_cell', 'chart', 'caption', 'footer']) {
    const f = fonts[role];
    if (!f) continue;
    const size = Math.min(40, Math.max(10, (f.size || 8) * 1.5));
    let text = SAMPLE_TEXT[role] || role;
    if (role === 'kpi') text += ` ${th.currency || ''}`;
    tg.append(h(`<div class="ov-trow"><span class="ov-role">${esc(ROLE_INFO[role] || role)}</span>
      <span class="ov-sample" style="${fontCss(f, th.ink)};font-size:${size}px">${esc(f.upper ? text.toUpperCase() : text)}</span>
      <span class="ov-meta">${esc(f.family)} · ${+(+f.size || 0).toFixed(1)} pt · ${f.weight}${f.italic ? ' italic' : ''}</span></div>`));
  }
  const kg = el.querySelector('.ov-kinds');
  const idx = previewIndex();
  for (const [, kinds] of STUDIO_GROUPS) {
    for (const [k, name, ic] of kinds) {
      const list = views(k);
      if (!list.length) continue;
      const t = h(`<button class="ov-kind"><span class="ov-kprev" style="background:${esc(S.st.doc.theme.page_bg)}">${preview(`sv_${k}_${list[0].id}`, idx, 8)}</span><span class="ov-kname">${icon(ic, 14)} ${esc(name)}<span>${list.length}</span></span></button>`);
      t.addEventListener('click', () => {
        // remember where the overview was, to come back to the same place
        S.studioOverviewTop = document.querySelector('#canvas')?.scrollTop || 0;
        S.studioKind = k; S.sel = null; render(); document.querySelector('#canvas')?.scrollTo({ top: 0 });
      });
      kg.append(t);
    }
  }
  return el;
}

function fontCss(f, fallback) {
  if (!f) return `color:${fallback}`;
  return `font-family:${esc(fontStack(f.family))};font-weight:${f.weight || 400};${f.italic ? 'font-style:italic;' : ''}color:${esc(f.color || fallback)};${f.tracking ? `letter-spacing:${f.tracking}em;` : ''}${f.upper ? 'text-transform:uppercase;' : ''}`;
}
