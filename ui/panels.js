// Side panels: the gallery / widgets / data / pages rail on the left and the
// inspector on the right. What they offer depends on who is editing: the
// template designer sees everything, end users see what the template allows.

import { colorPicker, hexInput } from './colorpick.js';
import { S, api, findWidget, edit, render, userMode, isReport, isTheme, policy, canEdit, call, setState, selection } from './state.js';
import { icon } from './icons.js';
import { esc, h, clone, fmtNum, fmtDate, setLocale, getFormats, resettable, colourName, themeColours, mixHex, inkOn, contrastOf } from './util.js';
import { menu, modal, toast, popover, closeLayer } from './ui.js';
import { PILL_PRESETS, RULE_OPS, guessPill, orderColumns, displayColumns, ICON_SETS, tablePart } from './table.js';
import { rolesEditor, KIND_ROLES, pickFamily, addFontFile, familyList, fontControl } from './fontui.js';
import { fontStack } from './util.js';
import { SCHEMA, VIZ_KINDS, KIND_NAMES, vizGroups, vizParts, ordered, context as vizContext, LABELS, UNITS } from './vizschema.js';
import { resolve as resolveViz, iconSlots, seriesKey, DRAWN } from './charts.js';
import { listToTable, tableToList } from './rowlist.js';
import { workspace, editDbSource, addConnection, connWhere, gridHtml, pythonSource } from './dbx.js';
import { sqlEditor } from './sqled.js';
import { goArea } from './rail.js';
import { hasRole } from './auth.js';
import { shape, tocOpts, themeColor } from './render.js';
import { glyph, glyphFor, GLYPH_NAMES, iconLabel, customIcons, letterText, glyphWidth as glyphWidthOf } from './glyphs.js';
import { richEditor } from './richtext.js';
import { LINE, TAGS, FLAGS, BRANDS, CATEGORIES } from './iconlib.js';
import { LOGOS } from './logolib.js';
import { dragPayload, selectNewest, scrollToPage, widgetMenu, scrollToWidget } from './canvas.js';
import { studioRail, duplicateView, viewMenu } from './studio.js';
import { repeatDialog, rowMenu, rowFields, repeatText } from './repeat.js';
const scrollToWidgetSafe = (id) => requestAnimationFrame(() => scrollToWidget(id));

export const KINDS = [
  ['kpi', 'KPI card', 'kpi', 'A headline number with change and trend'],
  ['bar', 'Bar chart', 'chart-bar', 'Compare categories or periods'],
  ['line', 'Line chart', 'chart-line', 'Trends over time'],
  ['area', 'Area chart', 'chart-area', 'Volume over time'],
  ['pie', 'Donut chart', 'chart-pie', 'Share of a total'],
  ['table', 'Table', 'table', 'Exact figures by category'],
  ['heading', 'Heading', 'heading', 'Page or section title'],
  ['text', 'Text', 'paragraph', 'Free text without a card'],
  ['comment', 'Note', 'comment', 'A note to the client, signed by its author'],
  ['divider', 'Divider', 'divider', 'A thin rule between sections'],
  ['image', 'Image', 'image', 'Logo or picture'],
  ['treemap', 'Treemap', 'treemap', 'Shares as areas'],
  ['waterfall', 'Waterfall', 'waterfall', 'Steps that add up to a total'],
  ['tiles', 'Tiles', 'tiles', 'A card per category'],
  ['toc', 'Contents', 'toc', 'Table of contents with page numbers'],
  ['composite', 'Composite', 'grid', 'Several parts in one card: a figure, a mini chart, text — one data source'],
];
export const kindIcon = (k) => (KINDS.find((x) => x[0] === k) || [])[2] || 'layout';
export const kindName = (k) => (KINDS.find((x) => x[0] === k) || [])[1] || k;

/**
 * What the editor calls a widget (the inspector, the canvas, lists, fit messages): the name the
 * designer gave it (never printed), else its title, its text's first line, or for an untitled
 * figure its type and data source ("Table · Bank accounts"). `custom: false`: the default name.
 */
export function widgetName(w, { custom = true } = {}) {
  const own = custom ? String(w?.opts?.name || '').trim() : '';
  if (own) return own;
  if (w?.title) return w.title;
  const line = String(w?.opts?.text || '').split('\n')[0].replace(/[*_{}$|]/g, '').trim();
  if (line) return line.length > 32 ? `${line.slice(0, 30)}…` : line;
  const ds = S.st?.doc?.datasets?.find((d) => d.id === w?.binding?.dataset);
  return ds ? `${kindName(w.kind)} · ${ds.name}` : kindName(w?.kind);
}

const WINDOWS = [['period', 'Selected period'], ['ytd', 'Year to date'], ['trailing3', 'Last 3 months'], ['trailing6', 'Last 6 months'], ['trailing12', 'Last 12 months'], ['trailing24', 'Last 24 months'], ['all', 'All data']];
// "closing"/"opening"/"avg_daily" are the stored names; they work on any dated data
const AGGS = [['sum', 'Sum'], ['closing', 'Last value (latest date)'], ['opening', 'First value (earliest date)'], ['avg_daily', 'Average across dates'], ['avg', 'Average'], ['count', 'Count'], ['distinct', 'Distinct count'], ['min', 'Min'], ['max', 'Max']];
const FORMATS = [['number', 'Number'], ['currency', 'Currency'], ['percent', 'Percent'], ['compact', 'Compact (1.2M)']];
const GRAINS = [['month', 'Month'], ['quarter', 'Quarter'], ['year', 'Year'], ['week', 'Week'], ['day', 'Day']];
const DATA_KINDS = ['kpi', 'bar', 'line', 'area', 'pie', 'table', 'treemap', 'waterfall', 'tiles', 'scale'];

// ---------------------------------------------------------------- left rail

let left = null;

export function initLeft(el) {
  left = el;
}

function tabs() {
  return userMode()
    ? [['add', 'Add', 'plus'], ['pages', 'Pages', 'pages']]
    : [['widgets', 'Widgets', 'layout'], ['gallery', 'Gallery', 'gallery'], ['data', 'Data', 'database'], ['vars', 'Variables', 'formula'], ['pages', 'Pages', 'pages']];
}

export function renderLeft() {
  if (!left) return;
  if (!S.st?.doc) { left.innerHTML = ''; return; }
  if (isTheme()) {
    const scroll = left.querySelector('.st-rail-body')?.scrollTop || 0;
    const r = studioRail();
    left.replaceChildren(r);
    r.querySelector('.st-rail-body').scrollTop = scroll;
    return;
  }
  // the widget studio has a pane of its own: the widget, its preview size, where it's used, saving
  if (S.st.doc.widget_studio) { left.replaceChildren(widgetStudioPane()); return; }
  const t = tabs();
  if (!t.find((x) => x[0] === S.leftTab)) S.leftTab = t[0][0];
  const el = h(`<div class="rail">
    <div class="rail-tabs">${t.map(([k, label, ic]) => `<button class="rail-tab${S.leftTab === k ? ' on' : ''}" data-k="${k}">${icon(ic, 15)}<span>${label}</span></button>`).join('')}</div>
    <div class="rail-body"></div>
  </div>`);
  el.querySelectorAll('.rail-tab').forEach((b) => b.addEventListener('click', () => { S.leftTab = b.dataset.k; renderLeft(); }));
  const body = el.querySelector('.rail-body');
  if (S.leftTab === 'widgets') body.append(widgetsTab());
  if (S.leftTab === 'gallery') body.append(galleryTab());
  if (S.leftTab === 'add') body.append(addTab());
  if (S.leftTab === 'data') body.append(dataTab());
  if (S.leftTab === 'pages') body.append(pagesTab());
  if (S.leftTab === 'vars') body.append(variablesTab());
  const scroll = left.querySelector('.rail-body')?.scrollTop || 0;
  left.replaceChildren(el);
  body.scrollTop = scroll;
}

export const currentPageId = () => S.st.doc.pages[Math.min(S.page, S.st.doc.pages.length - 1)].id;

function guessBinding(kind) {
  const ds = S.st.doc.datasets[0];
  if (!ds || !DATA_KINDS.includes(kind)) return null;
  const fields = ds.fields?.length ? ds.fields : [];
  const num = fields.find((f) => f.kind === 'number');
  const text = fields.find((f) => f.kind === 'text');
  const date = fields.find((f) => f.kind === 'date') || (ds.date_field ? { name: ds.date_field } : null);
  const m = num ? [{ field: num.name, agg: ds.snapshot ? 'closing' : 'sum', label: '', format: 'number' }] : [{ field: '*', agg: 'count', label: 'Count', format: 'number' }];
  const b = { dataset: ds.id, dims: [], measures: m, window: 'period', filters: [], sort: '', limit: 0, others: false };
  if (kind === 'line' || kind === 'area') { if (date) b.dims = [`${date.name}:month`]; b.window = 'trailing12'; }
  else if (kind === 'waterfall') { if (date) b.dims = [`${date.name}:month`]; b.window = 'ytd'; }
  else if (kind !== 'kpi' && text) b.dims = [text.name];
  return b;
}

export function newWidget(kind) {
  const spec = S.st.specs[kind];
  const w = { kind, w: spec.def_w, h: spec.def_h, title: '', subtitle: '', style: {}, opts: {}, binding: guessBinding(kind) };
  if (DATA_KINDS.includes(kind)) {
    const b = w.binding;
    const m = b?.measures[0];
    const mName = m ? (m.label || (m.field === '*' ? 'Count' : m.field)) : kindName(kind);
    w.title = b?.dims[0] ? `${cap(mName)} by ${b.dims[0].split(':')[0]}` : cap(mName);
  }
  if (kind === 'heading') { w.opts = { text: 'Section heading' }; w.style = { size: 15 }; w.h = 1; }
  if (kind === 'text') w.opts = { text: 'Write your text here.' };
  if (kind === 'comment') w.opts = { text: '', signed: true, arrows: true };
  if (kind === 'bar') w.opts = { orientation: 'vertical' };
  if (kind === 'pie') w.opts = { donut: true };
  if (kind === 'table') w.opts = { totals: true };
  if (kind === 'toc') { w.title = 'Contents'; w.w = 6; w.h = 8; }
  if (kind === 'tiles') w.opts = { signColors: true };
  if (kind === 'composite') {
    // one card, one data source: a figure with a trend beside it and a line of text under them
    const ds = S.st.doc.datasets[0];
    const kb = guessBinding('kpi'), lb = guessBinding('line');
    w.binding = ds ? { dataset: ds.id, dims: [], measures: [], window: 'period', filters: [], sort: '', limit: 0, others: false } : null;
    w.opts = { grid: { cols: 12, rows: 6 } };
    w.title = kb?.measures?.[0] ? cap(kb.measures[0].label || (kb.measures[0].field === '*' ? 'Count' : kb.measures[0].field)) : 'Composite';
    const part = (id, kind, x, y, pw, ph, binding, opts = {}) => ({ id, kind, x, y, w: pw, h: ph, title: '', subtitle: '', style: { card: false }, opts, binding: binding ? { ...binding, dataset: '' } : null, variant: '' });
    w.parts = [
      part('p1', 'kpi', 0, 0, 5, 4, kb, { viz: { spark: 'none' } }),
      part('p2', 'line', 5, 0, 7, 4, lb, { viz: { axis: false, grid: false, legend: 'none' } }),
      part('p3', 'text', 0, 4, 12, 2, null, { text: 'For {{period}}.' }),
    ];
  }
  // the theme's first design view of the kind is the default look
  const view = S.st.doc.theme.styles?.[kind]?.[0];
  if (view) {
    w.variant = view.id;
    w.style = {};
    if (kind === 'bar' || kind === 'pie' || kind === 'tiles') w.opts = {};
  }
  return w;
}

// ---------------------------------------------------------------- design views (theme studio)

const SAMPLES = { kpi: [['kpi', 'Value with income and fees'], ['kpi-pct', 'A return']], table: [['categories', 'Categories'], ['grouped', 'Grouped categories']], bar: [['trend', 'Months'], ['combo', 'Months with a cumulative line'], ['ranked', 'Ranked categories'], ['categories', 'Categories'], ['signed', 'Results by category']], pie: [['categories', 'Categories'], ['signed', 'Signed contributions']], tiles: [['signed', 'Results'], ['categories', 'Values']] };

async function promptText(title, label, value) {
  const body = h(`<div class="form"><label>${esc(label)}<input name="v" value="${esc(value || '')}"></label></div>`);
  setTimeout(() => body.querySelector('input')?.focus(), 30);
  return modal({ title, body, buttons: [{ label: 'Cancel', value: null }, { label: 'OK', kind: 'primary', value: (b) => b.querySelector('[name=v]').value.trim() || null }] });
}

/** Inspector for a design view in the theme studio: edits go to the view. */
function studioInspector(f, el) {
  const w = f.w;
  const list = S.st.doc.theme.styles?.[w.kind] || [];
  const v = list.find((x) => x.id === w.variant);
  if (!v) return el;
  const idx = list.indexOf(v);
  const vset = (patch) => edit({ op: 'update_variant', kind: w.kind, id: v.id, name: null, description: null, sample: null, ...patch });
  el.querySelector('.ih-t').innerHTML = `<b>${esc(v.name)}</b><span>${esc(kindName(w.kind))} · design view${idx === 0 ? ' · <em class="ih-def">Default</em>' : ''}</span>`;
  const more = el.querySelector('#w-more');
  const acts = h(`<span class="ih-acts"></span>`);
  const btn = (ic, title, fn, disabled) => { const b = h(`<button class="tb-btn sm" title="${esc(title)}"${disabled ? ' disabled' : ''}>${icon(ic, 15)}</button>`); b.addEventListener('click', fn); acts.append(b); };
  btn('star', idx === 0 ? 'This is the default view' : 'Make default: new widgets of this type start with it', () => edit({ op: 'move_variant', kind: w.kind, id: v.id, to: 0 }), idx === 0);
  btn('duplicate', 'Duplicate', () => duplicateView(w.kind, v));
  more.replaceWith(acts);
  const m = h(`<button class="tb-btn sm" title="More">${icon('more', 16)}</button>`);
  m.addEventListener('click', (e) => viewMenu(e.currentTarget, w.kind, v, idx));
  acts.append(m);

  const d = displayControls(w, false);
  return tabbed(el, 'studio', [
    ['design', 'Design', 'brush', () => designSections(f, { ...d, view: [] })],
    ['text', 'Text', 'type', () => [textStyle(f, false)]],
    ['about', 'About', 'info', () => [section('About this view',
      row('Name', textInput('v.name', v.name, (x) => x && vset({ name: x }))),
      row('When to use it', textInput('v.desc', v.description, (x) => vset({ description: x }), { area: true, rows: 3, placeholder: 'Shown under the preview and in the template’s picker' }), 'stack'),
      SAMPLES[w.kind] ? row('Preview data', selectInput('v.sample', v.sample || SAMPLES[w.kind][0][0], SAMPLES[w.kind], (x) => vset({ sample: x }))) : null,
      h(`<div class="hint">Widgets that use this view in templates follow every change, unless they override a setting themselves. Preview data only changes what the studio shows.</div>`),
    )]],
  ]);
}

/** Kinds whose series or categories each get a colour. */
const ENTITY_KINDS = ['bar', 'line', 'area', 'pie', 'treemap', 'tiles'];

/** The colour controls that actually drive this kind of widget. */
function colourSection(f) {
  const w = f.w;
  if (w.kind === 'comment') return section('Accent', swatches(w, (c) => upd(w.id, { style: { color: c } }), !userMode()));
  if (w.kind === 'divider') return section(['Colour', { key: 'Line colour' }], row('Line', colorField('s.color', w.style?.color || null, (c) => upd(w.id, { style: { color: c } }), { title: 'Line colour' })));
  return null;
}

/**
 * One colour per series (bars, lines) or category (donuts, treemaps, tiles),
 * taken from the data on the page. Colours are kept by name, so a category
 * keeps its colour whatever the period or the order.
 */
function entityColours(f) {
  const w = f.w;
  const data = f.vm.data;
  if (!data || data.empty || !data.columns) return null;
  const sh = shape(data);
  const eff = resolveViz(S.st, drawn(f));
  const byCat = ['pie', 'treemap', 'tiles'].includes(w.kind);
  const names = (byCat ? sh.cats : sh.series.map((x) => x.name)).filter((n) => n != null && n !== '');
  if (!names.length) return null;
  const pal = S.st.doc.theme.palette;
  const alpha = [...names].sort((a, b) => a.localeCompare(b));
  const own = w.opts?.viz?.colors || {};
  const set = (name, c) => upd(w.id, { opts: { viz: { colors: { [name]: c } } } });
  const items = [];
  const ctx = vizContext(drawn(f));
  const barSeries = (w.binding?.measures || []).filter((m) => m.chart !== 'line').length;
  const sign = w.kind === 'bar'
    ? eff.colorMode === 'sign' || (!ctx.horizontal && eff.colorMode === 'signOne' && barSeries === 1)
    : ['treemap', 'tiles'].includes(w.kind) && eff.colorMode === 'sign' && sh.series.length > 1;
  // coloured by result: the up and down colours (in the chart's own colour settings) are all that apply
  if (sign) return null;
  // shades of one colour: the base colour under Colours is what applies
  if (w.kind === 'pie' && eff.mono) return null;
  const list = h(`<div class="ent-list"></div>`);
  names.slice(0, 14).forEach((name, i) => {
    const idx = byCat && w.kind !== 'tiles' ? alpha.indexOf(name) : i;
    const cur = own[name] || eff.colors?.[name] || (Array.isArray(eff.palette) && eff.palette[idx]) || pal[idx % pal.length];
    const r = h(`<div class="ent-row${own[name] ? ' set' : ''}"><span class="ent-name" title="${esc(name)}">${esc(name)}</span></div>`);
    r.prepend(colorField(`ent.${i}`, own[name] || cur, (c) => set(name, c), { none: false, title: name }));
    if (own[name]) resettable(r, r.querySelector('.ent-name'), () => set(name, null), 'the palette colour');
    list.append(r);
  });
  items.push(list);
  if (names.length > 14) items.push(h(`<div class="hint">${names.length - 14} more take palette colours in order.</div>`));
  const n = Object.keys(own).length;
  if (n) {
    const b = h(`<button class="btn sm">${icon('restart', 13)} Use the palette for all</button>`);
    b.addEventListener('click', () => upd(w.id, { opts: { viz: { colors: null } } }));
    items.push(b);
  }
  items.push(h(`<div class="hint">Colours stay with the name, so “${esc(names[0])}” keeps its colour in every period. Change the palette itself in the theme.</div>`));
  return { rows: [h(`<div class="vz-sub">${byCat ? 'Each category' : 'Each series'}</div>`), ...items], count: n };
}

/** Per-role fonts for a widget (or a design view in the studio). */
function textStyle(f, user) {
  const w = f.w;
  if (!KIND_ROLES[w.kind]) return null;
  return section('Text style', rolesEditor(
    KIND_ROLES[w.kind],
    (role) => w.style?.fonts?.[role],
    // a KPI's figure sized to fit ("auto"): the size it came to when drawn
    (role) => (role === 'kpi' && !f.vm.fonts?.kpi?.size && DRAWN.get(w.id)?.valueSize ? { ...f.vm.fonts.kpi, autoSize: DRAWN.get(w.id).valueSize } : f.vm.fonts?.[role]),
    (role, patch) => upd(w.id, { style: { fonts: { [role]: patch } } }),
    { colorField },
  ), isTheme() ? null : h(`<div class="hint">Overrides the theme for this widget only. Reset a role to follow the theme again.</div>`));
}

const cap = (s) => String(s).charAt(0).toUpperCase() + String(s).slice(1).replace(/_/g, ' ');

/** A dataset's table name in SQL (same rule as the engine): "Sales targets" → sales_targets. */
export function sqlName(name) {
  let s = String(name || '').trim().toLowerCase().replace(/[^\p{L}\p{N}]/gu, '_').replace(/_+/g, '_').replace(/^_|_$/g, '');
  if (!s || /^\d/.test(s)) s = `t_${s}`;
  return s;
}

// ---- the designer's own widget types (a library every template can use)

let TYPES_CACHE = null;
/** The library's widget types (kept until one is saved or deleted). */
export async function widgetTypes(fresh = false) {
  if (!TYPES_CACHE || fresh) {
    try { TYPES_CACHE = await call('auth', { cmd: 'widget_types', args: {} }, { quiet: true }); } catch { TYPES_CACHE = []; }
  }
  return TYPES_CACHE;
}

/** A widget made from a type, on a page (at a place, when dropped). */
export async function addTypedWidget(id, page, at = null) {
  const st = await call('auth', { cmd: 'add_typed_widget', args: { id, page, at } });
  setState(st);
  return st;
}

// ---- the widget studio's pane

/** What is known about the widget being worked on, and what can be done with it. */
function widgetStudioPane() {
  const doc = S.st.doc;
  const w = doc.pages[0]?.widgets[0];
  const mine = doc.widget_studio !== 'new' ? doc.theme.widget_types?.[doc.widget_studio] : null;
  const views = doc.theme.styles?.[w?.kind] || [];
  const viewId = w?.variant || mine?.variant || '';
  const view = views.find((v) => v.id === viewId);
  // the full size it opened at: small and medium are shares of it
  if (!S.wsBase || S.wsBase.doc !== doc.widget_studio + doc.meta.title) S.wsBase = { doc: doc.widget_studio + doc.meta.title, w: w?.w || 12, h: w?.h || 6 };
  const base = S.wsBase;
  const sizes = [['s', 'Small', 0.45], ['m', 'Medium', 0.7], ['l', 'Large', 1]];
  const curSize = sizes.find(([, , k]) => Math.round(base.w * k) === w?.w)?.[0] || '';
  const el = h(`<div class="rail ws-pane">
    <div class="ws-head">${icon(mine?.icon || 'layers', 18)}<span><b>${esc(mine?.name || view?.name || kindName(w?.kind))}</b><span>${esc(kindName(w?.kind))} · ${mine ? `yours · version ${mine.version}` : 'built-in'}</span></span></div>
    <div class="rail-body ws-body"></div>
  </div>`);
  const body = el.querySelector('.ws-body');
  const about = h(`<div class="ws-sec"><div class="sec-title">About</div></div>`);
  if (mine) {
    about.append(h(`<label class="ws-l">Name</label>`), h(`<input class="ws-in" data-k="name" value="${esc(mine.name)}" spellcheck="false">`),
      h(`<label class="ws-l">Description</label>`), h(`<textarea class="ws-in" data-k="description" rows="2" placeholder="When to use it">${esc(mine.description || '')}</textarea>`));
    about.querySelectorAll('.ws-in').forEach((i) => i.addEventListener('keydown', (e) => e.stopPropagation()));
  } else {
    about.append(h(`<div class="hint">A built-in widget${view ? ` — the design view “${esc(view.name)}”${view.description ? `: ${esc(view.description)}` : ''}` : ''}. Change it as you like, then <b>Save as my widget…</b> to keep your version; the built-in one stays as it is.</div>`));
  }
  about.append(h(`<div class="ws-kv"><span>Kind</span><b>${esc(kindName(w?.kind))}</b></div>`));
  if (view) about.append(h(`<div class="ws-kv"><span>Design view</span><b>${esc(view.name)}</b></div>`));
  body.append(about);
  // preview size: how it behaves small and large (the zoom stays, so the size shows)
  const sz = h(`<div class="ws-sec"><div class="sec-title">Preview size</div></div>`);
  sz.append(seg('ws.size', curSize, sizes.map(([k, l]) => [k, l]), (v) => {
    const k = sizes.find((x) => x[0] === v)[2];
    const nw = Math.max(3, Math.round(base.w * k)), nh = Math.max(2, Math.round(base.h * (0.55 + 0.45 * k)));
    edit({ op: 'set_rect', id: w.id, rect: { x: Math.round((24 - nw) / 2), y: 2, w: nw, h: nh } });
  }), h(`<div class="hint">Sample data: the widget is drawn on example figures. In a template it shows that template's data.</div>`));
  body.append(sz);
  // where it's used (from the Widgets tab's last look, when there was one)
  const key = mine ? `type:${mine.id}` : `${w?.kind}/${viewId}`;
  const it = (S.wgItems || []).find((x) => (x.type ? `type:${x.type}` : `${x.kind}/${x.view}`) === key);
  if (it) {
    const used = h(`<div class="ws-sec"><div class="sec-title">Used by</div></div>`);
    const few = (l, none) => (l?.length ? `${esc(l.slice(0, 5).join(', '))}${l.length > 5 ? ` and ${l.length - 5} more` : ''}` : none);
    used.append(h(`<div class="ws-kv"><span>Themes</span><b title="${esc((it.themes || []).join('\n'))}">${mine ? 'any theme' : few(it.themes, '—')}</b></div>`),
      h(`<div class="ws-kv"><span>Templates</span><b title="${esc((it.used || []).join('\n'))}">${few(it.used, 'not used yet')}</b></div>`));
    body.append(used);
  }
  // saving
  const acts = h(`<div class="ws-sec ws-acts"></div>`);
  const val = (k) => el.querySelector(`.ws-in[data-k="${k}"]`)?.value.trim();
  const f = { w, vm: null };
  if (mine) {
    const b1 = h(`<button class="btn primary">${icon('save', 14)} Save widget</button>`);
    b1.addEventListener('click', () => saveWidgetType(f, { update: true, about: { name: val('name') || mine.name, description: val('description') ?? mine.description } }));
    const b2 = h(`<button class="btn">${icon('duplicate', 14)} Save as a new widget…</button>`);
    b2.addEventListener('click', () => saveWidgetType(f));
    acts.append(b1, b2, h(`<div class="hint">Saving makes version ${mine.version + 1}: templates pick it up when opened; bankers get it when a template is published again.</div>`));
  } else {
    const b = h(`<button class="btn primary">${icon('save', 14)} Save as my widget…</button>`);
    b.addEventListener('click', () => saveWidgetType(f));
    acts.append(b);
  }
  body.append(acts);
  return el;
}

/** Saves a widget's look as a widget type (or updates the type it follows); it follows the type then. */
export async function saveWidgetType(f, { update = false, about = null } = {}) {
  let args = { widget: f.w.id, update, ...(update && about ? about : {}) };
  if (!update) {
    const ICONS = ['layers', 'kpi', 'chart-bar', 'chart-line', 'chart-pie', 'table', 'tiles', 'wallet', 'coins', 'star', 'flag', 'briefcase'];
    // a name from the title, without its placeholders ("Итого активы на {{end}}, ₽" → "Итого активы")
    const suggested = String(f.w.title || widgetName(f.w)).replace(/\{\{[^}]*\}\}/g, '').replace(/\s*(на|on|for|за)?\s*[,·:]?\s*₽?\s*$/i, '').replace(/[\s,·:]+$/, '').trim() || kindName(f.w.kind);
    const body = `<label class="dup-l">Name</label><input class="dup-name wt-name" value="${esc(suggested)}" spellcheck="false">
      <label class="dup-l">Description</label><input class="dup-name wt-desc" placeholder="When to use it (shown in the Widgets panel)" spellcheck="false">
      <label class="dup-l">Icon</label><div class="wt-icons">${ICONS.map((ic, i) => `<label class="wt-ic${i === 0 ? ' on' : ''}"><input type="radio" name="wt-ic" value="${ic}"${i === 0 ? ' checked' : ''}>${icon(ic, 18)}</label>`).join('')}</div>
      <div class="hint">The type keeps this widget's look and settings, and the data, title and size new widgets start with. It goes in the widget types library, so every template can use it. This widget follows it from now on; change it and choose <b>Update the type</b> to change every widget of the type.</div>`;
    const res = await modal({ title: 'Save as widget type', iconName: 'layers', body,
      buttons: [{ label: 'Cancel', value: null }, { label: 'Save type', kind: 'primary', value: (b) => ({ name: b.querySelector('.wt-name').value.trim(), description: b.querySelector('.wt-desc').value.trim(), icon: b.querySelector('input[name=wt-ic]:checked')?.value || 'layers' }) }],
      onOpen: (b) => {
        b.querySelectorAll('.wt-ic input').forEach((i) => i.addEventListener('change', () => b.querySelectorAll('.wt-ic').forEach((l) => l.classList.toggle('on', l.querySelector('input').checked))));
        const n = b.querySelector('.wt-name'); n.focus(); n.select();
      } });
    if (!res || !res.name) return;
    args = { ...args, ...res };
  }
  try {
    const r = await call('auth', { cmd: 'widget_type_save', args });
    setState(r.state);
    TYPES_CACHE = null;
    render();
    toast(update ? `“${r.type.name}” updated — version ${r.type.version}. Every widget of the type follows.` : `“${r.type.name}” saved as a widget type. It's in the Widgets panel of every template.`, 'info', 4000);
  } catch (e) { toast(String(e?.message || e), 'error'); }
}

function widgetsTab() {
  const el = h(`<div class="rail-sec"><div class="hint">Drag onto a page, or click to add to the current page.</div><div class="kind-grid"></div><div class="wt-sec"></div></div>`);
  const grid = el.querySelector('.kind-grid');
  // the designer's own types below the built-in kinds
  widgetTypes().then((list) => {
    const box = el.querySelector('.wt-sec');
    if (!list?.length) {
      box.innerHTML = `<div class="sec-title">Your widget types</div><div class="hint">Configure a widget the way you want it, then choose <b>Save as widget type…</b> in its ⋯ menu to reuse it in any template.</div>`;
      return;
    }
    box.innerHTML = `<div class="sec-title">Your widget types</div><div class="wt-list"></div>`;
    for (const t of list) {
      const b = h(`<div class="wt-item" draggable="true" title="${esc(t.description || t.name)}">${icon(t.icon || 'layers', 18)}<span><b>${esc(t.name)}</b><span>${esc(kindName(t.kind))} · v${t.version}${t.used_here ? ` · ${t.used_here} here` : ''}</span></span><button class="tb-btn sm" title="Delete from the library">${icon('trash', 13)}</button></div>`);
      b.addEventListener('dragstart', (e) => { dragPayload.current = { wtype: t.id, w: t.w || 4, h: t.h || 3 }; e.dataTransfer.setData('text/plain', t.id); e.dataTransfer.effectAllowed = 'copy'; });
      b.addEventListener('dragend', () => { dragPayload.current = null; });
      b.addEventListener('click', async (e) => {
        if (e.target.closest('button')) return;
        const page = currentPageId();
        const st = await addTypedWidget(t.id, page);
        selectNewest(st, page);
      });
      b.querySelector('button').addEventListener('click', async (e) => {
        e.stopPropagation();
        const ok = await modal({ title: 'Delete widget type', iconName: 'warn', iconKind: 'warn', body: `<p>Delete “${esc(t.name)}” from the library? Templates that use it keep their copy, so their widgets look the same.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
        if (!ok) return;
        try { await call('auth', { cmd: 'widget_type_delete', args: { id: t.id } }); TYPES_CACHE = null; renderLeft(); toast('Widget type deleted'); } catch (err) { toast(String(err?.message || err), 'error'); }
      });
      box.querySelector('.wt-list').append(b);
    }
  });
  for (const [k, name, ic, desc] of KINDS) {
    const b = h(`<button class="kind" data-k="${k}" draggable="true" title="${esc(desc)}">${icon(ic, 22)}<span>${name}</span></button>`);
    b.addEventListener('dragstart', (e) => {
      const w = newWidget(k);
      dragPayload.current = { widget: w, w: w.w, h: w.h };
      e.dataTransfer.setData('text/plain', k);
      e.dataTransfer.effectAllowed = 'copy';
    });
    b.addEventListener('dragend', () => { dragPayload.current = null; });
    b.addEventListener('click', async () => {
      const page = currentPageId();
      const st = await edit({ op: 'add_widget', page, widget: newWidget(k), at: null });
      selectNewest(st, page);
    });
    grid.append(b);
  }
  return el;
}

/** A report user's Add tab: a note or words of one's own, then the report's figures by page. */
function addTab() {
  const pol = policy();
  const el = h(`<div class="rail-sec"></div>`);
  const basics = [
    pol.comment && ['comment', 'Note to the client', 'comment', 'Your words, signed as from you'],
    pol.add && ['text', 'Text', 'paragraph', 'A paragraph'],
    pol.add && ['heading', 'Heading', 'heading', 'A title for a part of the page'],
    pol.add && ['divider', 'Line', 'divider', 'A rule between parts'],
    pol.add && ['image', 'Picture', 'image', 'A logo or a photo'],
  ].filter(Boolean);
  if (basics.length) {
    el.append(h(`<div class="sec-title">Notes and text</div>`));
    const grid = h(`<div class="add-grid"></div>`);
    for (const [kind, label, ic, tip] of basics) {
      const b = h(`<button class="add-tile" title="${esc(tip)}">${icon(ic, 18)}<span>${esc(label)}</span></button>`);
      b.addEventListener('click', async () => {
        const page = currentPageId();
        const st = await edit({ op: 'add_widget', page, widget: newWidget(kind), at: null });
        selectNewest(st, page);
      });
      grid.append(b);
    }
    el.append(grid);
  }
  if (pol.add && S.st.doc.catalog.length) {
    el.append(h(`<div class="sec-title">Figures of this report</div>`), h(`<div class="hint">Click + to add one to the current page, or drag it there.</div>`));
    el.append(...figureGroups());
  }
  if (!el.children.length) el.append(h(`<div class="empty-note">${icon('lock', 18)}<span>This report's template doesn't allow adding.</span></div>`));
  return el;
}

// which groups of the Add tab are open (by source page)
const ADD_OPEN = new Set();

/**
 * The report's figures, one collapsible group per page they come from. The
 * group of the page being edited comes first and open; a figure of a repeated
 * page (an account's) can only go on that page, so others' show without +.
 */
function figureGroups() {
  const doc = S.st.doc;
  const pi = Math.min(S.page, doc.pages.length - 1);
  const here = doc.pages[pi]?.repeat_of || doc.pages[pi]?.id;
  const order = [...new Set(doc.catalog.map((c) => c.page || ''))];
  order.sort((a, b) => (a === here ? -1 : b === here ? 1 : 0));
  const out = [];
  for (const pid of order) {
    const items = doc.catalog.filter((c) => (c.page || '') === pid);
    const si = doc.pages.findIndex((p) => p.id === pid);
    // the page's name as drawn (a repeated page shows the account being edited)
    const shownAt = pid === here ? pi : si;
    const name = pid ? (S.st.pages[shownAt]?.title || doc.pages[si]?.title || 'Page') : (items[0]?.category || 'Gallery');
    const open = pid === here ? !ADD_OPEN.has('-' + pid) : ADD_OPEN.has(pid);
    const g = h(`<div class="fig-group${open ? ' open' : ''}">
      <button class="fig-head">${icon('chevd', 12)}<b></b>${pid === here ? '<span class="fig-here">This page</span>' : ''}<span class="fig-n">${items.length}</span></button>
      <div class="fig-body"></div>
    </div>`);
    g.querySelector('b').textContent = String(name).replace(/\{\{[^}]*\}\}/g, '…');
    g.querySelector('.fig-head').addEventListener('click', () => {
      if (pid === here) { ADD_OPEN.has('-' + pid) ? ADD_OPEN.delete('-' + pid) : ADD_OPEN.add('-' + pid); } else if (ADD_OPEN.has(pid)) ADD_OPEN.delete(pid); else ADD_OPEN.add(pid);
      g.classList.toggle('open');
    });
    const rowed = (c) => JSON.stringify(c.widget).includes('{{row.');
    const body = g.querySelector('.fig-body');
    for (const c of items) body.append(galleryItem(c, { addable: !rowed(c) || pid === here }));
    out.push(g);
  }
  return out;
}

/** One gallery item: drag it onto a page, or + to add it to the current page. */
function galleryItem(c, { addable = true, more = false } = {}) {
  const item = h(`<div class="gal-item${addable ? '' : ' off'}" draggable="${addable}" title="${addable ? '' : 'Shows on its account’s page: open that page to add it'}">
    <span class="gi-ic">${icon(kindIcon(c.widget.kind), 18)}</span>
    <span class="gi-body"><b>${esc(String(c.name).replace(/\{\{[^}]*\}\}/g, '…'))}</b><span>${esc(c.description || kindName(c.widget.kind))}</span></span>
    ${addable ? `<button class="gi-add" title="Add to the current page">${icon('plus', 15)}</button>` : ''}
    ${more ? `<button class="gi-more" title="More">${icon('more', 15)}</button>` : ''}
  </div>`);
  if (!addable) return item;
  item.addEventListener('dragstart', (e) => {
    dragPayload.current = { catalog: c.id, w: c.widget.w, h: c.widget.h };
    e.dataTransfer.setData('text/plain', c.id);
    e.dataTransfer.effectAllowed = 'copy';
  });
  item.addEventListener('dragend', () => { dragPayload.current = null; });
  item.querySelector('.gi-add').addEventListener('click', async () => {
    const page = currentPageId();
    const st = await edit({ op: 'add_from_catalog', page, catalog: c.id, at: null });
    selectNewest(st, S.st.doc.pages[Math.min(S.page, S.st.doc.pages.length - 1)].repeat_of || page);
  });
  item.querySelector('.gi-more')?.addEventListener('click', (e) => menu(e.currentTarget, [
    { label: 'Rename…', icon: 'rename', run: () => editCatalogItem(c) },
    { label: 'Remove from gallery', icon: 'trash', danger: true, run: () => edit({ op: 'remove_catalog', id: c.id }) },
  ], { align: 'end' }));
  return item;
}

function galleryTab({ bare = false } = {}) {
  const doc = S.st.doc;
  const user = userMode();
  const pol = policy();
  const el = h(`<div class="rail-sec"></div>`);
  if (user && !pol.add) {
    el.append(h(`<div class="empty-note">${icon('lock', 18)}<span>This template doesn't allow adding widgets.</span></div>`));
    return el;
  }
  if (!bare) el.append(h(`<div class="hint">${user ? 'Widgets prepared for this report. Drag one onto a page, or click + to add it.' : 'What end users can add. Build a widget on a page, then right-click → Save to gallery.'}</div>`));
  if (!doc.catalog.length) el.append(h(`<div class="empty-note">${icon('gallery', 18)}<span>The gallery is empty.</span></div>`));
  const cats = [...new Set(doc.catalog.map((c) => c.category || 'Other'))];
  for (const cat of cats) {
    el.append(h(`<div class="sec-title">${esc(cat)}</div>`));
    for (const c of doc.catalog.filter((x) => (x.category || 'Other') === cat)) el.append(galleryItem(c, { more: !user }));
  }
  return el;
}

async function editCatalogItem(c) {
  const body = h(`<div class="form">
    <label>Name<input name="name" value="${esc(c.name)}"></label>
    <label>Category<input name="category" value="${esc(c.category)}" placeholder="KPIs, Charts, Tables…"></label>
    <label>Description<input name="description" value="${esc(c.description)}"></label>
  </div>`);
  const v = await modal({ title: 'Gallery item', body, buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: (b) => Object.fromEntries([...b.querySelectorAll('input')].map((i) => [i.name, i.value])) }] });
  if (v) edit({ op: 'upsert_catalog', item: { ...c, ...v } });
}

export async function saveToGallery(id) {
  const f = findWidget(id);
  if (!f) return;
  const cat = { kpi: 'KPIs', table: 'Tables', text: 'Text', comment: 'Text', heading: 'Text' }[f.w.kind] || 'Charts';
  const body = h(`<div class="form">
    <label>Name<input name="name" value="${esc(widgetName(f.w))}"></label>
    <label>Category<input name="category" value="${cat}"></label>
    <label>Description<input name="description" placeholder="What end users will see in the gallery"></label>
  </div>`);
  const v = await modal({ title: 'Save to gallery', body, buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: (b) => Object.fromEntries([...b.querySelectorAll('input')].map((i) => [i.name, i.value])) }] });
  if (v) { await edit({ op: 'save_to_catalog', id, ...v }); toast('Added to the gallery'); }
}

export function replaceMenu(anchor, f, { types = false, gallery = true } = {}) {
  const items = [];
  // the same data as another kind of chart
  if (types && CHART_TYPES.some((t) => t[1] === f.w.kind)) {
    const cur = chartTypeOf(f.w);
    items.push({ title: 'The same data as' });
    for (const t of CHART_TYPES) {
      if (t[0] === cur) continue;
      const why = typeOff(f.w, t);
      items.push({ label: t[3], icon: t[4], desc: why || null, disabled: !!why, run: () => setChartType(f.w, t[0]) });
    }
  }
  if (gallery && S.st.doc.catalog.length) {
    items.push(...(items.length ? [{ sep: true }] : []), { title: 'A widget from the gallery' });
    for (const c of S.st.doc.catalog) {
      items.push({ label: c.name, icon: kindIcon(c.widget.kind), desc: c.description, run: () => edit({ op: 'replace_widget', id: f.w.id, catalog: c.id }) });
    }
  }
  if (items.length) menu(anchor, items);
}

export function moveToPageItems(f) {
  const items = S.st.doc.pages.map((p, i) => ({
    label: `${i + 1}. ${p.title}`,
    icon: 'page',
    disabled: i === f.pi,
    run: () => edit({ op: 'move_to_page', id: f.w.id, page: p.id }),
  }));
  if (policy().pages) items.push({ sep: true }, { label: 'New page', icon: 'plus', run: () => edit({ op: 'move_to_page', id: f.w.id, page: null }) });
  return items;
}

// ---------------------------------------------------------------- data tab

const SOURCES = [
  ['csv', 'CSV file', 'table'],
  ['excel', 'Excel workbook', 'table'],
  ['sqlite', 'SQLite database', 'database'],
  ['postgres', 'PostgreSQL', 'database'],
  ['clickhouse', 'ClickHouse', 'database'],
  ['duckdb', 'DuckDB', 'database'],
  ['python', 'Python script', 'code'],
  ['inline', 'Paste data', 'paste'],
  ['query', 'SQL over other sources', 'code'],
];

/** A small tag for what a source reads: SQL, PY, CSV, XLS or DATA (pasted). */
export function srcTag(src) {
  const t = src?.type;
  const [k, l] = ['db', 'duckdb', 'sqlite', 'postgres', 'clickhouse', 'query'].includes(t) ? ['sql', 'SQL'] : t === 'python' ? ['py', 'PY'] : t === 'csv' ? ['csv', 'CSV'] : t === 'excel' ? ['xls', 'XLS'] : ['data', 'DATA'];
  return `<i class="src-tag t-${k}" title="${{ sql: 'An SQL query', py: 'A Python script', csv: 'A CSV file', xls: 'An Excel workbook', data: 'Pasted data' }[k]}">${l}</i>`;
}

export function sourceLabel(src, shared = false) {
  if (src.type === 'db') {
    const c = (S.st.doc?.connections || []).find((x) => x.id === src.connection) || (S.sharedConns || []).find((x) => x.id === src.connection);
    return c ? `SQL on ${c.name}` : shared ? 'SQL' : 'SQL · connection removed';
  }
  const s = SOURCES.find((x) => x[0] === src.type);
  const where = src.path ? src.path.split('/').pop() : src.type === 'query' ? 'joins, CTEs, windows' : src.type === 'duckdb' ? 'in-memory' : src.type === 'inline' ? `${src.rows?.length ?? 0} rows` : src.type === 'postgres' || src.type === 'clickhouse' ? (src.url || '').replace(/:[^:@/]*@/, ':•••@') : '';
  return `${s ? s[1] : src.type}${where ? ' · ' + where : ''}`;
}

/** Opens the Data area on a connection, data source or the parameters. */
export function openData(sel) {
  S.dataSel = sel;
  goArea('data');
}

/** Kinds of data source a template can keep as its own (SQL on its connections, files, scripts…). */
export function ownSourceKinds() {
  const doc = S.st.doc;
  return [
    ...(doc.connections || []).map((c) => ({ label: `SQL on ${c.name}`, icon: 'database', desc: 'Browse its tables, write the query', run: () => workspace(c) })),
    ...SOURCES.filter(([k]) => k !== 'python' || !S.st?.python_off).map(([k, label, ic]) => ({ label, icon: ic, run: () => datasetEditor({ id: '', name: '', source: defaultSource(k), date_field: null, fields: [] }) })),
  ];
}

/** Kinds of shared source (Data area): SQL on a shared connection, or a file or script. */
export function sharedSourceKinds(conns, onSave) {
  return [
    ...conns.map((c) => ({ label: `SQL on ${c.name}`, icon: 'database', desc: 'Browse its tables, write the query', run: () => workspace(c, { global: true, saveSource: onSave }) })),
    ...SOURCES.filter(([k]) => k !== 'query' && (k !== 'python' || !S.st?.python_off)).map(([k, label, ic]) => ({ label, icon: ic, run: () => datasetEditor({ id: '', name: '', source: defaultSource(k), date_field: null, fields: [], shared: true }, false, { onSave }) })),
  ];
}

/** The template's "Add": a shared source from the Data area, or a new one (made in the Data area). */
export async function addSourceMenu(anchor) {
  let lib = { datasets: [] };
  try { lib = await call('auth', { cmd: 'library' }, { quiet: true }); } catch { /* none */ }
  const have = new Set(S.st.doc.datasets.map((d) => d.id));
  menu(anchor, [
    { title: 'From the Data area' },
    ...lib.datasets.map((d) => ({ label: d.name, icon: 'table', desc: have.has(d.id) ? 'already in this template' : sourceLabel(d.source, true), disabled: have.has(d.id), run: () => call('auth', { cmd: 'use_source', args: { id: d.id } }).then((st) => { setState(st); toast(`“${d.name}” added — use it in any widget’s Data tab.`, 'info'); }) })),
    !lib.datasets.length && { label: 'No shared data sources yet', icon: 'info', disabled: true, run: () => {} },
    { sep: true },
    { label: 'Create a data source…', icon: 'plus', desc: 'In the Data area — shared, or only for this template', run: () => openData({ kind: 'new' }) },
  ]);
}

/** The template's data at a glance; each item opens in the Data area, where the work happens. */
function dataTab() {
  const doc = S.st.doc;
  const el = h(`<div class="rail-sec dtab">
    <button class="btn dtab-open">${icon('database', 15)} Open in the Data area ${icon('chevr', 13)}</button>
    <div class="sec-title row">Data sources<button class="btn sm" id="add-src">${icon('plus', 14)} Add</button></div>
    <div class="ds-list"></div>
    <div class="sec-title row">Connections</div>
    <div class="cn-list"></div>
    <div class="sec-title row">Report parameters</div>
    <div class="param-list"></div>
    <div class="hint">Click any of them to work on it in the <b>Data</b> area. In a widget, <code>{{name}}</code> follows a parameter; in SQL, <code>:name</code>, <code>:period_start</code> and <code>:period_end</code>.</div>
  </div>`);
  el.querySelector('.dtab-open').addEventListener('click', () => openData(null));
  const item = (ic, title, sub, sel, tag = '') => {
    const it = h(`<button class="ds-item dtab-item"><span class="gi-ic">${icon(ic, 16)}</span><span class="gi-body"><b>${esc(title)}</b><span>${esc(sub)}</span></span>${tag}${icon('chevr', 14)}</button>`);
    it.addEventListener('click', () => openData(sel));
    return it;
  };
  const list = el.querySelector('.ds-list');
  if (!doc.datasets.length) list.append(h(`<div class="empty-note">${icon('database', 18)}<span>No data yet. Add a source.</span></div>`));
  for (const ds of doc.datasets) list.append(item(ds.shared ? 'link' : 'table', ds.name || ds.id, `${ds.shared ? 'Shared · ' : ''}${sourceLabel(ds.source)}`, { kind: ds.shared ? 'lib' : 'ds', id: ds.id }, srcTag(ds.source)));
  const cl = el.querySelector('.cn-list');
  if (!(doc.connections || []).length) cl.append(h(`<div class="empty-note small"><span>None — add one from the Data area.</span></div>`));
  for (const c of doc.connections || []) cl.append(item('database', c.name, connWhere(c), { kind: 'conn', id: c.id }));
  const pl = el.querySelector('.param-list');
  if (!doc.param_defs.length) pl.append(h(`<div class="empty-note small"><span>No parameters. The reporting period is always available.</span></div>`));
  else pl.append(item('sliders', doc.param_defs.map((p) => p.label || p.name).join(', '), `${doc.param_defs.length} parameter${doc.param_defs.length > 1 ? 's' : ''}`, { kind: 'params' }));
  el.querySelector('#add-src').addEventListener('click', (e) => addSourceMenu(e.currentTarget));
  return el;
}

function defaultSource(k) {
  return {
    csv: { type: 'csv', path: '' },
    excel: { type: 'excel', path: '', sheet: '' },
    sqlite: { type: 'sqlite', path: '', query: 'select * from sales\nwhere date between :data_start and :data_end' },
    postgres: { type: 'postgres', url: 'postgresql://user:password@localhost:5432/db', query: 'select * from sales\nwhere date between :data_start and :data_end' },
    clickhouse: { type: 'clickhouse', url: 'clickhouse://default@localhost:8123/default', query: 'select * from sales\nwhere date between :data_start and :data_end' },
    duckdb: { type: 'duckdb', path: '', query: "-- Query a DuckDB file, or leave the database empty and read files directly:\n-- select * from 'sales.parquet'  ·  read_csv('data/*.csv')\nselect * from 'sales.parquet'" },
    python: { type: 'python', code: '# `params` holds data_start, data_end (the dates to load) and report parameters.\nimport pandas as pd\n\ndf = pd.DataFrame({\n    "date": pd.date_range(params["data_start"], params["data_end"], freq="D"),\n})\ndf["value"] = range(len(df))\ndf' },
    inline: { type: 'inline', columns: [], rows: [] },
    query: { type: 'query', sql: `-- Combine sources with CTEs, joins and window functions.\n-- :period_start and :period_end are the report period.\nwith p as (\n  select *\n  from ${sqlName(S.st.doc?.datasets[0]?.name || 'sales')}\n  where ${S.st.doc?.datasets[0]?.date_field ? qid(S.st.doc.datasets[0].date_field) : 'true'} between :period_start and :period_end\n)\nselect * from p` },
  }[k];
}

/**
 * Edits a data source. `onSave(ds)` saves it elsewhere than the open
 * template (a shared source of the Data area); `conn` is its connection then.
 */
export async function datasetEditor(ds, previewNow = false, { onSave = null, conn = null } = {}) {
  if (ds.source?.type === 'db') return editDbSource(ds, { saveSource: onSave, conn });
  // a Python source is edited in a Python tab of the Data area, like an SQL source
  if (ds.source?.type === 'python') { if (!S.showHome && S.st?.doc) goArea('data'); return pythonSource(ds, { saveSource: onSave }); }
  ds = clone(ds);
  const src = ds.source;
  const type = src.type;
  const body = h(`<div class="ds-editor">
    <div class="form two">
      <label>Name<input name="name" value="${esc(ds.name)}" placeholder="Sales"></label>
      <label>Date field <span class="muted">(for periods)</span><select name="date_field"><option value="">None</option></select></label>
    </div>
    <div class="form src"></div>
    <details class="sql-sec" open>
      <summary>${icon('code', 14)} Shape with SQL <span class="muted">— calculated fields, filters, joins</span></summary>
      <div class="sql-body">
        <div class="sec-title">Calculated fields</div>
        <div class="calc-list"></div>
        <button class="btn sm" id="calc-add" type="button">${icon('plus', 13)} Add field</button>
        <div class="hint">Any SQL expression over the columns: <code>amount / 1e6</code>, <code>CASE WHEN amount &gt; 1e6 THEN 'Large' ELSE 'Small' END</code>, <code>date_trunc('quarter', report_dt)</code>, <code>coalesce(region, 'Unknown')</code>.</div>
        <div class="tf-part"${type === 'query' ? ' hidden' : ''}><div class="sec-title">Transform</div>
        <textarea name="transform" class="code" rows="6" placeholder="select * from data where amount > 0">${esc(ds.transform || '')}</textarea>
        <div class="hint">Your rows are the table <code>data</code>. Join other sources by name: ${(S.st.doc?.datasets || []).filter((d) => d.id !== ds.id).map((d) => `<code>${esc(sqlName(d.name))}</code>`).join(', ') || '<em>none yet</em>'}. Runs in DuckDB, sandboxed — no file or network access. <code>:period_start</code>, <code>:period_end</code> and parameters work here.</div></div>
      </div>
    </details>
    <label class="tgl snap-tgl"><input type="checkbox" name="snapshot"${ds.snapshot ? ' checked' : ''}><span class="track"></span><span><b>Values are snapshots per date</b> (balances, headcount, prices) — they don't add up over time, so new values default to <b>Last value</b>: the value at the latest date in the period.</span></label>
    <div class="ds-actions"><button class="btn" id="pv">${icon('play', 14)} Load preview</button><span class="pv-status muted"></span></div>
    <div class="pv-table"></div>
  </div>`);
  const srcEl = body.querySelector('.src');
  const calcs = clone(ds.calc_fields || []);
  const calcList = body.querySelector('.calc-list');
  const drawCalcs = () => {
    calcList.replaceChildren(...calcs.map((c, i) => {
      const r = h(`<div class="calc-row"><input class="cn" placeholder="name" value="${esc(c.name)}"><span class="muted">=</span><input class="ce code" placeholder="SQL expression" value="${esc(c.expr)}"><button class="tb-btn sm" type="button">${icon('x', 13)}</button></div>`);
      r.querySelector('.cn').addEventListener('input', (e) => { c.name = e.target.value; });
      r.querySelector('.ce').addEventListener('input', (e) => { c.expr = e.target.value; });
      r.querySelectorAll('input').forEach((x) => x.addEventListener('keydown', (e) => e.stopPropagation()));
      r.querySelector('button').addEventListener('click', () => { calcs.splice(i, 1); drawCalcs(); });
      return r;
    }));
  };
  drawCalcs();
  body.querySelector('#calc-add').addEventListener('click', () => { calcs.push({ name: '', expr: '' }); drawCalcs(); calcList.querySelector('.calc-row:last-child .cn')?.focus(); });
  const pathRow = (kind) => {
    const r = h(`<label>File<div class="path-row"><input name="path" value="${esc(src.path || '')}" placeholder="/path/to/file"><button class="btn" type="button">Browse…</button></div></label>`);
    r.querySelector('button').addEventListener('click', async () => {
      const p = await call('pick_data_file', { kind });
      if (p) {
        r.querySelector('input').value = p;
        if (kind === 'excel') fillSheets(p);
        if (!body.querySelector('[name=name]').value) body.querySelector('[name=name]').value = p.split('/').pop().replace(/\.[^.]+$/, '');
      }
    });
    return r;
  };
  if (type === 'csv') srcEl.append(pathRow('csv'));
  if (type === 'excel') {
    srcEl.append(pathRow('excel'));
    srcEl.append(h(`<label>Sheet<select name="sheet"><option value="${esc(src.sheet || '')}">${esc(src.sheet || 'First sheet')}</option></select></label>`));
  }
  if (type === 'sqlite') { srcEl.append(pathRow('sqlite')); srcEl.append(h(`<label>Query<textarea name="query" class="code" rows="6">${esc(src.query || '')}</textarea></label>`)); }
  if (type === 'duckdb') {
    const r = pathRow('duckdb');
    r.querySelector('input').placeholder = 'Optional: a .duckdb file. Empty = in-memory';
    srcEl.append(r);
    srcEl.append(h(`<label>Query <span class="muted">— <code>:data_start</code>, <code>:data_end</code> (the dates to load), the period and parameters work here; file names are relative to the document</span><textarea name="query" class="code" rows="7">${esc(src.query || '')}</textarea></label>`));
  }
  if (type === 'postgres' || type === 'clickhouse') {
    srcEl.append(h(`<label>Connection<input name="url" value="${esc(src.url || '')}" placeholder="${type === 'clickhouse' ? 'clickhouse://user:password@host:8123/database' : 'postgresql://user:password@host:5432/db'}"></label>`));
    srcEl.append(h(`<label>Query<textarea name="query" class="code" rows="6">${esc(src.query || '')}</textarea></label>`));
  }
  if (type === 'query') {
    const others = (S.st.doc?.datasets || []).filter((d) => d.id !== ds.id);
    srcEl.append(h(`<label>SQL <span class="muted">— DuckDB, over the other sources by name</span><textarea name="sql" class="code" rows="12">${esc(src.sql || '')}</textarea></label>`));
    srcEl.append(h(`<div class="hint">Sources: ${others.map((d) => `<code>${esc(sqlName(d.name))}</code>`).join(', ') || '<em>none yet</em>'}. Report values: <code>:period_start</code>, <code>:period_end</code> (the period), <code>:prev_start</code>, <code>:prev_end</code> (the comparison)${(S.st.doc?.param_defs || []).map((p) => `, <code>:${esc(p.name)}</code>`).join('')}; a parameter with several values becomes a list, so write <code>client in (:client)</code>. CTEs, joins and window functions all work. Runs sandboxed, with no file or network access.</div>`));
  }
  if (type === 'python') {
    // the Python editor: highlighting, and completion of pd., sources["…"] (other sources) and params["…"]
    const lab = h(`<label class="py-src">Script <span class="muted">— leave a DataFrame in <code>df</code> or as the last line; <code>sources["name"]</code> reads another data source, <code>params</code> the report's values</span></label>`);
    const ped = sqlEditor({ lang: 'python', value: src.code || '', placeholder: '# e.g.\ndf = sources["Позиции на дату"]\ndf.groupby("instr_type_ru")["amt_rub"].sum().reset_index()',
      sources: () => (S.st.doc?.datasets || []).filter((d) => d.id !== ds.id).map((d) => d.name), params: () => (S.st.doc?.param_defs || []).map((p) => ({ name: p.name, value: S.st.doc.params.values?.[p.name] ?? '' })) });
    ped.ta.name = 'code';
    ped.ta.addEventListener('keydown', (e) => e.stopPropagation());
    lab.append(ped.el);
    srcEl.append(lab);
  }
  if (type === 'inline') {
    const csv = [src.columns.join(','), ...src.rows.slice(0, 2000).map((r) => r.map((v) => (v == null ? '' : /[,"\n]/.test(String(v)) ? `"${String(v).replace(/"/g, '""')}"` : v)).join(','))].join('\n');
    srcEl.append(h(`<label>Data <span class="muted">— paste CSV or tab-separated rows with a header line</span><textarea name="inline" class="code" rows="10">${esc(src.columns.length ? csv : '')}</textarea></label>`));
  }
  async function fillSheets(path) {
    try {
      const sheets = await call('excel_sheets', { path }, { quiet: true });
      const sel = body.querySelector('[name=sheet]');
      sel.innerHTML = sheets.map((s) => `<option${s === src.sheet ? ' selected' : ''}>${esc(s)}</option>`).join('');
    } catch { /* not readable yet */ }
  }
  if (type === 'excel' && src.path) fillSheets(src.path);

  const read = () => {
    const v = (n) => body.querySelector(`[name=${n}]`)?.value ?? '';
    const s = { type };
    if (type === 'csv') s.path = v('path');
    if (type === 'excel') { s.path = v('path'); s.sheet = v('sheet'); }
    if (type === 'sqlite') { s.path = v('path'); s.query = v('query'); }
    if (type === 'postgres' || type === 'clickhouse') { s.url = v('url'); s.query = v('query'); }
    if (type === 'duckdb') { s.path = v('path'); s.query = v('query'); }
    if (type === 'python') s.code = v('code');
    if (type === 'inline') Object.assign(s, parseDelimited(v('inline')));
    if (type === 'query') s.sql = v('sql');
    return {
      ...ds, name: v('name') || 'Data', source: s, date_field: v('date_field') || null, snapshot: body.querySelector('[name=snapshot]').checked,
      transform: body.querySelector('[name=transform]').value, calc_fields: calcs.filter((c) => c.name.trim() && c.expr.trim()),
    };
  };
  const dateSel = body.querySelector('[name=date_field]');
  body.querySelector('[name=snapshot]').addEventListener('change', (e) => { e.target.dataset.touched = '1'; });
  dateSel.addEventListener('change', () => { if (!ds.id) preview(); });
  const setFields = (fields) => {
    ds.fields = fields;
    const cur = dateSel.value || ds.date_field || '';
    dateSel.innerHTML = '<option value="">None</option>' + fields.map((f) => `<option value="${esc(f.name)}"${f.name === cur ? ' selected' : ''}>${esc(f.name)}${f.kind === 'date' ? ' (date)' : ''}</option>`).join('');
    if (!cur) { const d = fields.find((f) => f.kind === 'date'); if (d) dateSel.value = d.name; }
  };
  setFields(ds.fields || []);
  if (ds.date_field && !ds.fields?.length) dateSel.innerHTML += `<option selected>${esc(ds.date_field)}</option>`;
  const status = body.querySelector('.pv-status');
  const preview = async () => {
    status.textContent = 'Loading…';
    try {
      const p = await call('preview_dataset', { dataset: read() }, { quiet: true });
      setFields(p.fields);
      status.textContent = `${p.total.toLocaleString()} rows · ${p.columns.length} columns`;
      const snapBox = body.querySelector('[name=snapshot]');
      if (p.snapshot_hint && !ds.id && !snapBox.dataset.touched) {
        snapBox.checked = true;
        status.textContent += ' · looks like daily snapshots';
      }
      body.querySelector('.pv-table').innerHTML = gridHtml(p.fields, p.rows.slice(0, 40));
      return p;
    } catch (e) {
      status.textContent = '';
      body.querySelector('.pv-table').innerHTML = `<div class="err-box">${esc(e)}</div>`;
      return null;
    }
  };
  body.querySelector('#pv').addEventListener('click', preview);
  const res = await modal({
    title: ds.id ? `Data source · ${ds.name}` : `New data source · ${(SOURCES.find((x) => x[0] === type) || [])[1]}`,
    body, wide: true,
    onOpen: () => { if (previewNow || ds.id) preview(); },
    buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: () => 'save' }],
  });
  if (res !== 'save') return;
  const out = read();
  if (!out.fields?.length) {
    // load once so widgets know the fields
    const p = await preview().catch(() => null);
    if (p) out.fields = p.fields;
  }
  out.fields = ds.fields;
  if (onSave) await onSave(out);
  else await edit({ op: 'upsert_dataset', dataset: out });
}

function parseDelimited(text) {
  const lines = text.replace(/\r\n?/g, '\n').split('\n').filter((l) => l.trim());
  if (!lines.length) return { columns: [], rows: [] };
  const d = (lines[0].match(/\t/g) || []).length > (lines[0].match(/,/g) || []).length ? '\t' : (lines[0].match(/;/g) || []).length > (lines[0].match(/,/g) || []).length ? ';' : ',';
  const split = (l) => {
    const out = []; let f = '', q = false;
    for (let i = 0; i < l.length; i++) {
      const c = l[i];
      if (q) { if (c === '"') { if (l[i + 1] === '"') { f += '"'; i++; } else q = false; } else f += c; }
      else if (c === '"' && !f) q = true;
      else if (c === d) { out.push(f); f = ''; }
      else f += c;
    }
    out.push(f);
    return out;
  };
  const columns = split(lines[0]).map((s) => s.trim());
  const rows = lines.slice(1).map((l) => split(l).map((v) => { const t = v.trim(); if (t === '') return null; const n = Number(t.replace(/,/g, '')); return /^-?[\d,]*\.?\d+$/.test(t) && isFinite(n) ? n : t; }));
  return { columns, rows };
}

/**
 * The template's parameters. `addNew`: only a new, empty parameter; `only`:
 * just that parameter (by name). The others are kept as they are.
 */
export async function paramEditor({ addNew = false, only = null } = {}) {
  const doc = S.st.doc;
  // a stored parameter may leave its type empty: one with choices (typed or from the data) is a list
  const all = clone(doc.param_defs).map((d) => ({ multi: false, field: '', datasets: [], source: null, columns: {}, ...d, kind: d.kind || (d.options?.length || d.source ? 'list' : 'text') }));
  const focus = addNew || only != null;
  const defs = addNew ? [] : only != null ? all.filter((d) => d.name === only) : all;
  const kept = focus ? all.filter((d) => !defs.includes(d)) : [];
  const allFields = [...new Set(doc.datasets.flatMap((d) => (d.fields || []).map((f) => f.name)))].sort();
  const body = h(`<div class="param-editor"><div class="pe-list"></div><button class="btn sm" id="pe-add"${focus ? ' hidden' : ''}>${icon('plus', 14)} Add parameter</button>
    <div class="hint">Report users pick parameter values in the toolbar, and the whole report redraws. A parameter can filter every source that has its column, appear in text as <code>{{name}}</code>, in widget filters as <code>{{name}}</code>, and in SQL as <code>:name</code>.</div></div>`);
  const list = body.querySelector('.pe-list');
  const draw = () => {
    list.replaceChildren(...defs.map((d, i) => {
      const card = h(`<div class="pe-card">
        <div class="pe-top"><input class="pe-label" placeholder="Label, e.g. Client" value="${esc(d.label)}"><input class="pe-name code" placeholder="name" value="${esc(d.name)}"><button class="tb-btn sm pe-del" title="Remove">${icon('trash', 14)}</button></div>
        <div class="pe-grid">
          <label>Type<select class="pe-kind">${[['list', 'Choice list'], ['text', 'Text'], ['number', 'Number'], ['date', 'Date']].map(([k, l]) => `<option value="${k}"${d.kind === k ? ' selected' : ''}>${l}</option>`).join('')}</select></label>
          <label class="pe-list-only">Choices<select class="pe-src"><option value="typed"${d.source ? '' : ' selected'}>Typed in</option><option value="data"${d.source ? ' selected' : ''}>From the data</option></select></label>
          <label class="pe-typed">Values<input class="pe-opts" placeholder="North, South, East" value="${esc((d.options || []).join(', '))}"></label>
          <label class="pe-from">Source<select class="pe-ds">${doc.datasets.map((x) => `<option value="${esc(x.id)}"${d.source?.dataset === x.id ? ' selected' : ''}>${esc(x.name)}</option>`).join('')}</select></label>
          <label class="pe-from">Column<select class="pe-col"></select></label>
          <label class="pe-from">Shown as<select class="pe-lab"></select></label>
          <label class="pe-list-only pe-multi"><span>Several at once</span><span class="tgl"><input type="checkbox" class="pe-m"${d.multi ? ' checked' : ''}><span class="track"></span></span></label>
          <label>Default<input class="pe-def" placeholder="All" value="${esc(d.default || '')}"></label>
          <label>Filters column<select class="pe-field"><option value="">Nothing automatically</option>${allFields.map((f) => `<option${d.field === f ? ' selected' : ''}>${esc(f)}</option>`).join('')}</select></label>
        </div>
        <div class="pe-sources"></div>
      </div>`);
      const $ = (c) => card.querySelector(c);
      const refresh = () => {
        const list2 = d.kind === 'list';
        card.querySelectorAll('.pe-list-only').forEach((x) => (x.hidden = !list2));
        $('.pe-typed').hidden = !list2 || !!d.source;
        card.querySelectorAll('.pe-from').forEach((x) => (x.hidden = !list2 || !d.source));
        const ds = doc.datasets.find((x) => x.id === d.source?.dataset) || doc.datasets[0];
        $('.pe-col').innerHTML = (ds?.fields || []).map((f) => `<option${d.source?.field === f.name ? ' selected' : ''}>${esc(f.name)}</option>`).join('');
        // the text users see for each choice (e.g. a name for a code); empty: the value itself
        $('.pe-lab').innerHTML = `<option value="">The value itself</option>${(ds?.fields || []).map((f) => `<option${d.source?.label === f.name ? ' selected' : ''}>${esc(f.name)}</option>`).join('')}`;
        // which column it filters in each source: found by name, chosen, or none
        const box = $('.pe-sources');
        box.replaceChildren();
        const has = (x, n) => (x.fields || []).find((f) => f.name.toLowerCase() === String(n || '').toLowerCase())?.name;
        const anyOn = d.field || Object.values(d.columns || {}).some(Boolean);
        if (!anyOn) {
          box.append(h(`<div class="hint">Not applied automatically: choose the column it filters above, or use <code>{{${esc(d.name || 'name')}}}</code> in widget filters and <code>:${esc(d.name || 'name')}</code> in SQL.</div>`));
          return;
        }
        box.append(h(`<div class="pe-src-head">Filters these sources</div>`));
        let off = 0;
        for (const x of doc.datasets) {
          const set = d.columns?.[x.id];
          const auto = has(x, d.field);
          const cur = set === undefined ? '__auto' : set;
          const on = set === undefined ? !!auto : !!set;
          if (!on) off++;
          const r = h(`<div class="pe-src${on ? '' : ' off'}"><span class="pe-src-n">${icon(on ? 'check' : 'minus', 12)} ${esc(x.name)}</span></div>`);
          const sel = selectInput(`pe.${i}.${x.id}`, cur, [['__auto', auto ? `Auto: ${auto}` : 'Auto: no such column'], ...(x.fields || []).map((f) => [f.name, f.name]), ['', 'Not filtered']], (v) => {
            d.columns = { ...(d.columns || {}) };
            if (v === '__auto') delete d.columns[x.id]; else d.columns[x.id] = v;
            refresh();
          });
          r.append(sel);
          box.append(r);
        }
        if (off) box.append(h(`<div class="hint warn-hint">${off} source${off > 1 ? 's are' : ' is'} not filtered: widgets on ${off > 1 ? 'them' : 'it'} show every ${esc((d.label || d.name || 'value').toLowerCase())}. Pick its column if it has one under another name.</div>`));
      };
      const on = (c, ev, fn) => { const x = $(c); x.addEventListener(ev, () => { fn(x); refresh(); }); x.addEventListener('keydown', (e) => e.stopPropagation()); };
      on('.pe-label', 'input', (x) => { d.label = x.value; if (!d._named) { d.name = x.value.trim().toLowerCase().replace(/[^a-zа-я0-9]+/gi, '_').replace(/^_|_$/g, ''); $('.pe-name').value = d.name; } });
      on('.pe-name', 'input', (x) => { d.name = x.value; d._named = true; });
      on('.pe-kind', 'change', (x) => { d.kind = x.value; });
      on('.pe-src', 'change', (x) => { d.source = x.value === 'data' ? { dataset: doc.datasets[0]?.id || '', field: '' } : null; });
      on('.pe-ds', 'change', (x) => { d.source = { dataset: x.value, field: '' }; });
      on('.pe-col', 'change', (x) => { d.source = { ...(d.source || {}), field: x.value }; if (!d.field) { d.field = x.value; $('.pe-field').value = x.value; } });
      on('.pe-lab', 'change', (x) => { d.source = { ...(d.source || {}), label: x.value }; });
      on('.pe-opts', 'input', (x) => { d.options = x.value.split(',').map((t) => t.trim()).filter(Boolean); });
      on('.pe-m', 'change', (x) => { d.multi = x.checked; });
      on('.pe-def', 'input', (x) => { d.default = x.value; });
      on('.pe-field', 'change', (x) => { d.field = x.value; });
      $('.pe-del').addEventListener('click', () => { defs.splice(i, 1); draw(); });
      if (d.source && !d.source.field) d.source.field = $('.pe-col').value;
      refresh();
      if (d.source && !d.source.field) { d.source.field = $('.pe-col').value; }
      d._named = !!d.name;
      return card;
    }));
    if (!defs.length) list.append(h(`<div class="empty-note small"><span>No parameters yet. The reporting period is always available.</span></div>`));
  };
  draw();
  body.querySelector('#pe-add').addEventListener('click', () => { defs.push({ name: '', label: '', kind: 'list', options: [], default: 'All', multi: false, field: '', datasets: [], source: null }); draw(); list.querySelector('.pe-card:last-child .pe-label')?.focus(); });
  if (addNew) setTimeout(() => body.querySelector('#pe-add').click(), 60);
  const title = addNew ? 'New parameter' : only != null ? `Parameter “${defs[0]?.label || only}”` : 'Report parameters';
  let ok;
  for (;;) {
    ok = await modal({ title, iconName: 'sliders', body, wide: true, buttons: [{ label: 'Cancel', value: null }, { label: addNew ? 'Add' : 'Save', kind: 'primary', value: true }] });
    if (!ok) return;
    // a name is used once
    const names = [...kept, ...defs].map((d) => d.name.trim().replace(/\s+/g, '_')).filter(Boolean);
    const twice = names.find((n, i) => names.indexOf(n) !== i);
    if (twice) { toast(`There is already a parameter “${twice}” — choose another name.`, 'error', 3200); continue; }
    if (addNew && !defs.some((d) => d.name.trim())) { toast('Give the parameter a name (used as :name in SQL).', 'error', 3000); continue; }
    break;
  }
  const clean = defs.filter((d) => d.name.trim()).map(({ _named, ...d }) => ({ ...d, name: d.name.trim().replace(/\s+/g, '_'), default: d.default || 'All', source: d.kind === 'list' && d.source?.dataset && d.source?.field ? d.source : null, multi: d.kind === 'list' && !!d.multi, columns: d.columns || {} }));
  // the ones not shown stay where they were
  const out = only != null ? all.map((d) => (d.name === only ? clean[0] : d)).filter(Boolean) : [...kept, ...clean];
  edit({ op: 'set_param_defs', defs: out });
}

// ---------------------------------------------------------------- pages tab

function pagesTab() {
  const doc = S.st.doc;
  const pol = policy();
  const el = h(`<div class="rail-sec">
    <div class="sec-title row">Pages${pol.pages ? `<button class="btn sm" id="add-page">${icon('plus', 14)} Add</button>` : ''}</div>
    <div class="page-list"></div>
  </div>`);
  const list = el.querySelector('.page-list');
  doc.pages.forEach((p, i) => {
    const issues = S.st.pages[i].issues.filter((x) => x.severity !== 'info');
    const item = h(`<div class="pg-item${i === S.page ? ' on' : ''}" data-pi="${i}">
      <span class="pg-num">${i + 1}</span>
      <span class="gi-body"><b>${p.repeat_of ? '↳ ' : ''}${esc(S.st.pages[i]?.title || p.title)}</b><span>${p.repeat_of ? `copy ${(p.copy || 0) + 1} of ${p.copies} · ` : p.repeat?.dataset ? `repeats ×${p.copies || 0} · ` : ''}${S.st.pages[i]?.hidden ? 'hidden for this data · ' : ''}${p.section ? esc(p.section) + ' · ' : ''}${p.auto ? 'continuation · ' : ''}${p.widgets.length} widget${p.widgets.length === 1 ? '' : 's'}</span></span>
      <span class="pg-state ${issues.length ? 'warn' : 'ok'}">${icon(issues.length ? 'warn' : 'ok', 14)}</span>
    </div>`);
    item.addEventListener('click', () => { S.page = i; scrollToPage(i); renderLeft(); });
    item.addEventListener('dblclick', () => pol.pages && renamePage(i));
    list.append(item);
  });
  el.querySelector('#add-page')?.addEventListener('click', async () => {
    const st = await edit({ op: 'add_page', after: doc.pages[S.page].id });
    if (st) { S.page = Math.min(S.page + 1, st.doc.pages.length - 1); render(); scrollToPage(S.page); }
  });
  return el;
}

export async function renamePage(i) {
  const p = S.st.doc.pages[i];
  const sections = [...new Set(S.st.doc.pages.map((x) => x.section).filter(Boolean))];
  const body = h(`<div class="form"><label>Page title<input name="t" value="${esc(p.title)}"></label>
    <label>Section <span class="muted">— groups pages in the section tabs and the table of contents</span><input name="s" list="sec-list" value="${esc(p.section || '')}" placeholder="e.g. Portfolio"><datalist id="sec-list">${sections.map((x) => `<option value="${esc(x)}">`).join('')}</datalist></label>
    <label>Description <span class="muted">— shown under the page in the table of contents</span><input name="d" value="${esc(p.description || '')}" placeholder="e.g. Assets, results and structure"></label>
    <label class="tgl"><input type="checkbox" name="plain"${p.plain ? ' checked' : ''}><span class="track"></span><span>Cover page — no section tabs or footer</span></label>
    <label>Page group <span class="muted">— pages in one group link to each other in a heading's page navigation (e.g. one account)</span><input name="g" value="${esc(p.group || '')}" placeholder="e.g. BROK"></label>
    <label>Navigation label <span class="muted">— short name in that navigation</span><input name="l" value="${esc(p.label || '')}" placeholder="${esc(p.title)}"></label>
    <div class="rep-set"><span><b>Repeat</b> <span class="muted">— ${p.repeat_of ? 'this page is a copy made for a repeated page' : p.repeat?.dataset ? `for ${esc(repeatText(p.repeat))}` : 'one copy per row of a data source (e.g. per account)'}</span></span><button class="btn sm" type="button" data-rep>${icon('refresh', 13)} ${p.repeat?.dataset || p.repeat_of ? 'Change…' : 'Set up…'}</button></div>
    <label>Show only when <span class="muted">— leave empty to always show; with a value such as <code>{{data:Source.field}}</code> the page shows only when it comes out non-empty and not 0 (e.g. accounts a client has). Hidden pages aren’t exported or numbered.</span><input name="si" value="${esc(p.show_if || '')}" placeholder="e.g. {{data:port.has_du}}"></label>
    <label>Frame <span class="muted">— a thin line around the page (cover pages)</span><span class="m-row"><input type="color" name="fc" value="${esc(p.frame || '#ddccae')}"><label class="tgl"><input type="checkbox" name="f"${p.frame ? ' checked' : ''}><span class="track"></span><span>Show</span></label></span></label></div>`);
  body.querySelector('[data-rep]')?.addEventListener('click', () => { document.querySelector('.modal-back:last-of-type .btn')?.click(); setTimeout(() => repeatDialog(i), 50); });
  const v = await modal({ title: 'Page', body, buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: (b) => ({ t: b.querySelector('[name=t]').value, s: b.querySelector('[name=s]').value, plain: b.querySelector('[name=plain]').checked, g: b.querySelector('[name=g]').value, l: b.querySelector('[name=l]').value, d: b.querySelector('[name=d]').value, si: b.querySelector('[name=si]').value, f: b.querySelector('[name=f]').checked ? b.querySelector('[name=fc]').value : '' }) }] });
  if (!v) return;
  if (v.t.trim() && v.t.trim() !== p.title) await edit({ op: 'rename_page', page: p.id, title: v.t.trim() });
  if ((v.s || '').trim() !== (p.section || '')) await edit({ op: 'set_page_section', page: p.id, section: v.s.trim() });
  if (!!v.plain !== !!p.plain) await edit({ op: 'set_page_plain', page: p.id, plain: v.plain });
  if (v.g.trim() !== (p.group || '') || v.l.trim() !== (p.label || '') || v.f !== (p.frame || '') || v.d.trim() !== (p.description || '') || v.si.trim() !== (p.show_if || '')) await edit({ op: 'set_page_props', page: p.id, group: v.g, label: v.l, frame: v.f, description: v.d.trim(), show_if: v.si.trim() });
}

// ---------------------------------------------------------------- inspector

let right = null;
export function initRight(el) { right = el; }

/** Field builders. Each commits through `edit` when the value changes. */
/** A row whose setting is made here (`has`): double-click its name to put it back (`reset`). */
function mark(r, has, reset, what = 'as designed') {
  if (has) resettable(r, r.querySelector(':scope > label'), reset, what);
  return r;
}

function row(label, control, cls = '') {
  const r = h(`<div class="f-row ${cls}"><label>${esc(label)}</label><div class="f-ctl"></div></div>`);
  r.querySelector('.f-ctl').append(control);
  return r;
}

function textInput(key, value, onCommit, { placeholder = '', type = 'text', min, max, step, area = false, rows = 4 } = {}) {
  const el = area
    ? h(`<textarea data-key="${key}" rows="${rows}" placeholder="${esc(placeholder)}">${esc(value ?? '')}</textarea>`)
    : h(`<input data-key="${key}" type="${type}" value="${esc(value ?? '')}" placeholder="${esc(placeholder)}"${min != null ? ` min="${min}"` : ''}${max != null ? ` max="${max}"` : ''}${step != null ? ` step="${step}"` : ''}>`);
  let last = String(value ?? '');
  const commit = () => {
    // a "Mixed" field (several widgets selected): any value typed is new for some of them; empty changes nothing
    if (el.classList.contains('mixed') ? el.value === '' : el.value === last) return;
    el.classList.remove('mixed');
    last = el.value;
    onCommit(type === 'number' ? (el.value === '' ? null : +el.value) : el.value);
  };
  el.addEventListener('change', commit);
  el.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Enter' && !area) { commit(); el.blur(); }
    if (e.key === 'Enter' && area && (e.metaKey || e.ctrlKey)) { commit(); el.blur(); }
  });
  return el;
}

function selectInput(key, value, options, onChange) {
  const el = h(`<select data-key="${key}">${options.map(([v, l]) => `<option value="${esc(v)}"${String(v) === String(value ?? '') ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`);
  el.addEventListener('change', () => onChange(el.value));
  return el;
}

function toggle(key, checked, onChange, label) {
  const el = h(`<label class="tgl" data-key="${key}"><input type="checkbox"${checked ? ' checked' : ''}><span class="track"></span>${label ? `<span>${esc(label)}</span>` : ''}</label>`);
  el.querySelector('input').addEventListener('change', (e) => onChange(e.target.checked));
  return el;
}

function seg(key, value, options, onChange) {
  // long labels don't fit side by side in the panel: offer them as a list
  // (and more than four choices crowd it whatever their length)
  if (!options.some((o) => o[2]) && (options.length > 4 || options.reduce((a, o) => a + String(o[1]).length, 0) > 17)) return selectInput(key, value, options, onChange);
  const el = h(`<div class="seg" data-key="${key}">${options.map(([v, l, ic]) => `<button data-v="${esc(v)}" class="${String(v) === String(value) ? 'on' : ''}" title="${esc(l)}">${ic ? icon(ic, 14) : ''}${ic ? '' : esc(l)}</button>`).join('')}</div>`);
  el.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => onChange(b.dataset.v)));
  return el;
}

/** Icons that tell the inspector's sections apart at a glance. */
export const SECTION_ICONS = {
  Content: 'paragraph', Data: 'database', 'Time & rows': 'calendar', 'Design view': 'palette', 'Chart type': 'chart-column', 'Card & frame': 'card',
  Style: 'brush', Look: 'brush', 'Colours & marks': 'drop', Colours: 'drop', Colour: 'drop', Accent: 'drop', 'Data labels': 'tag', Values: 'numbers', Value: 'numbers',
  'Axis & grid': 'axis', Legend: 'legend', Highlights: 'star', Lines: 'chart-line', Bar: 'chart-bar', 'Ranked list': 'toc', Scale: 'ruler', Centre: 'target',
  Layout: 'tiles', Card: 'card', 'Note & breakdown': 'paragraph', Change: 'arrow-up', Trend: 'chart-line', Points: 'marks', 'Text style': 'type',
  'Position on the grid': 'move', Template: 'layout', Report: 'page', 'Fit to page': 'autofit', Page: 'page', Header: 'headings', Footer: 'headings',
  Theme: 'palette', Typography: 'type', 'Chart defaults': 'chart-bar', 'What end users can do': 'shield', 'Theme colours': 'drop', 'Series colours': 'drop',
  'Cards & charts': 'card', 'Language & currency': 'globe', 'Rows & pages': 'table', 'Header row': 'headings', 'Lines & rows': 'grid', 'Totals & groups': 'sigma',
  Columns: 'pivot', 'About this view': 'info', 'Preview': 'eye', 'Type & view': 'palette', Series: 'chart-line', Marks: 'brush', Numbers: 'numbers',
  'Axes & grid': 'axis', 'Card & title': 'card', List: 'toc', Icons: 'star',
};
// open/closed per section, remembered; sections without a choice use their default
const FOLD = (() => { try { return JSON.parse(localStorage.getItem('quire.sections') || '{}') || {}; } catch { return {}; } })();
const saveFold = () => { try { localStorage.setItem('quire.sections', JSON.stringify(FOLD)); } catch { /* per-viewer nicety */ } };

/** A collapsible inspector section with an icon. `title` may be [title, { icon, badge, key }]. */
function section(title, ...children) {
  const [name, o] = Array.isArray(title) ? title : [title, {}];
  const key = o.key || name;
  const closed = key in FOLD ? !FOLD[key] : !!o.closed;
  const s = h(`<section class="insp-sec${closed ? ' closed' : ''}" data-sec="${esc(name)}">
    <button class="insp-title"><span class="is-ic">${icon(o.icon || SECTION_ICONS[name] || 'sliders', 15)}</span><span class="is-name" data-i18n="sec">${esc(name)}</span>${o.badge ? `<span class="is-badge" title="Settings changed here">${o.badge}</span>` : ''}<span class="is-chev">${icon('chevd', 13)}</span></button>
    <div class="insp-body"></div>
  </section>`);
  s.querySelector('.insp-title').addEventListener('click', () => {
    FOLD[key] = s.classList.toggle('closed') === false;
    saveFold();
  });
  const body = s.querySelector('.insp-body');
  for (const c of children.flat()) if (c) body.append(c);
  return s;
}

/**
 * Splits the inspector into categories. `tabs`: [key, label, icon, () => elements].
 * Empty categories are left out; the choice is remembered per context.
 */
/** What the last `tabbed` built: each tab's parts (for several selected widgets, to find their common settings). */
let BUILT = null;
/** Several widgets selected: per tab, the setting keys they all have (other settings are left out). */
let COMMON = null;

/** Every setting key in a tab's parts. */
function keysIn(parts) {
  const out = new Set();
  for (const p of parts) {
    if (p.dataset?.key) out.add(p.dataset.key);
    p.querySelectorAll?.('[data-key]').forEach((n) => out.add(n.dataset.key));
  }
  return out;
}

/** A tab's parts with only the common settings: other controls, then rows and sections left empty, go. */
function onlyCommon(parts, keys) {
  return parts.filter((p) => {
    p.querySelectorAll?.('[data-key]').forEach((n) => { if (!keys.has(n.dataset.key)) n.remove(); });
    if (p.dataset?.key) return keys.has(p.dataset.key);
    const empty = (n) => !n.querySelector('[data-key]');
    // rows, then the groups and cards holding them, then sections
    p.querySelectorAll?.('.f-row, .m-row, .xywh > label, .ser-card, .insp-sec').forEach((n) => { if (empty(n)) n.remove(); });
    return !empty(p);
  });
}

function tabbed(el, ctx, tabs) {
  let built = tabs.map(([k, label, ic, make]) => [k, label, ic, (make() || []).flat().filter(Boolean)]).filter((t) => t[3].length);
  BUILT = built;
  if (COMMON) built = built.filter((t) => COMMON.has(t[0])).map(([k, l, ic, parts]) => [k, l, ic, onlyCommon(parts, COMMON.get(k))]).filter((t) => t[3].length);
  if (!built.length) return el;
  S.inspTab ||= {};
  const cur = built.find((t) => t[0] === S.inspTab[ctx]) || built[0];
  const q = (S.inspQuery || '').trim();
  el.append(searchBar(q));
  if (q) {
    el.append(searchResults(built, q, ctx));
    return el;
  }
  if (built.length > 1) {
    const strip = h(`<div class="insp-tabs" data-i18n="insp">${built.map(([k, label, ic]) => `<button class="it${k === cur[0] ? ' on' : ''}" data-k="${k}" data-cat="${k}" title="${esc(label)}">${icon(ic, 16)}<span>${esc(label)}</span></button>`).join('')}</div>`);
    strip.querySelectorAll('.it').forEach((b) => b.addEventListener('click', () => { S.inspTab[ctx] = b.dataset.k; renderRight(); right.querySelector('.insp')?.scrollTo({ top: 0 }); }));
    el.append(strip);
  }
  const body = h(`<div class="insp-pane" data-cat="${cur[0]}"></div>`);
  for (const c of cur[3]) body.append(c);
  el.append(body);
  return el;
}

// ---------------------------------------------------------------- settings search

/** The search box above the inspector's tabs (⌘F). */
function searchBar(q) {
  const bar = h(`<div class="insp-search">${icon('search', 14)}<input data-key="insp.search" placeholder="Find a setting" value="${esc(q)}" spellcheck="false"><kbd>⌘F</kbd></div>`);
  const inp = bar.querySelector('input');
  inp.addEventListener('input', () => { S.inspQuery = inp.value; renderRight(); });
  inp.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Escape') { S.inspQuery = ''; renderRight(); }
    // Enter: go to the first result's control
    if (e.key === 'Enter') right.querySelector('.sr-group input:not([type=checkbox]), .sr-group select, .sr-group button, .sr-group .tgl input')?.focus();
  });
  if (q) {
    const x = h(`<button class="tb-btn sm sr-clear" title="Clear (Esc)">${icon('x', 13)}</button>`);
    x.addEventListener('click', () => { S.inspQuery = ''; renderRight(); });
    bar.querySelector('kbd').replaceWith(x);
  }
  return bar;
}

// both spellings, and a few words people use for the same thing
const SYNONYMS = [['colour', 'color'], ['centre', 'center'], ['grey', 'gray'], ['font', 'type text typography'], ['size', 'height width'], ['border', 'outline'], ['spacing', 'padding gap'], ['hide', 'show'], ['number', 'format decimals'], ['legend', 'key']];
const norm = (s) => String(s || '').toLowerCase().replace(/ё/g, 'е');
function terms(q) {
  return norm(q).split(/\s+/).filter(Boolean).map((t) => {
    const alts = new Set([t]);
    for (const group of SYNONYMS) {
      const words = group.join(' ').split(' ');
      if (words.some((w) => w.startsWith(t) || t.startsWith(w))) words.forEach((w) => alts.add(w));
    }
    return [...alts];
  });
}

/** Text a row is found by: its label, then the choices inside it. */
function rowText(r) {
  // a text role (Text tab): by its name and what it is set to, e.g. "KPI value Inter · 13 pt · Bold"
  if (r.matches('details.role')) {
    const name = r.querySelector('.ro-name')?.textContent || '';
    return { label: norm(name), all: norm(`${name} ${r.querySelector('.ro-meta')?.textContent || ''} text font`) };
  }
  const label = r.querySelector(':scope > label')?.textContent || '';
  const extra = [...r.querySelectorAll('option, .seg button, [title], [placeholder]')].map((x) => `${x.textContent} ${x.title || ''} ${x.placeholder || ''} ${x.value || x.dataset?.v || ''}`).join(' ');
  return { label: norm(label), all: norm(label + ' ' + extra) };
}

/**
 * Every setting in every tab that matches, as live controls grouped by
 * "Tab › Section". A section whose name matches comes whole.
 */
function searchResults(built, q, ctx) {
  const tq = terms(q);
  const hit = (text) => tq.every((alts) => alts.some((a) => text.includes(a)));
  const out = h(`<div class="insp-pane sr-pane"></div>`);
  let n = 0;
  for (const [k, label, ic, items] of built) {
    for (const item of items) {
      const secs = item.matches?.('.insp-sec') ? [item] : [...(item.querySelectorAll?.('.insp-sec') || [])];
      // content that isn't in a section: search its rows directly
      const groups = secs.length ? secs.map((sec) => [sec.querySelector('.is-name')?.textContent || '', sec.querySelector('.insp-body'), sec.querySelector('.is-ic')?.innerHTML || '']) : [['', item, '']];
      for (const [name, body, secIcon] of groups) {
        if (!body) continue;
        const whole = name && hit(norm(name));
        const rows = [...body.querySelectorAll('.f-row, details.role')];
        let picked = whole ? [...body.children] : rows.filter((r) => hit(rowText(r).all));
        // a matching row brings its hint along
        if (!whole) picked = picked.flatMap((r) => (r.nextElementSibling?.classList.contains('hint') ? [r, r.nextElementSibling] : [r]));
        if (!picked.length) continue;
        const g = h(`<section class="sr-group"><button class="sr-head" title="Open this section in its tab"><span class="sr-ic">${secIcon || icon(ic, 14)}</span><span class="sr-path"><span>${esc(label)}</span>${name ? ` › <b>${esc(name)}</b>` : ''}</span>${icon('chevr', 12)}</button><div class="sr-body"></div></section>`);
        g.querySelector('.sr-head').addEventListener('click', () => {
          S.inspTab[ctx] = k; S.inspQuery = '';
          if (name) { FOLD[name] = true; saveFold(); }
          renderRight();
          setTimeout(() => [...right.querySelectorAll('.is-name')].find((x) => x.textContent === name)?.closest('.insp-sec')?.scrollIntoView({ block: 'start' }), 0);
        });
        const gb = g.querySelector('.sr-body');
        for (const el of picked) {
          if (!whole) { const lab = el.querySelector?.(':scope > label') || el.querySelector?.('.ro-name'); if (lab) lab.innerHTML = highlight(lab.textContent, tq); }
          // a matching text role opens, its settings ready
          if (el.matches?.('details.role')) el.open = true;
          gb.append(el);
        }
        n += whole ? 1 : picked.filter((x) => x.matches?.('.f-row, details.role')).length;
        out.append(g);
      }
    }
  }
  if (!n) out.append(h(`<div class="sr-empty">${icon('search', 18)}<b>No settings match “${esc(q)}”</b><span>${COMMON ? 'Only the settings all the selected widgets have are searched: select fewer to find the others.' : 'Try another word, e.g. “colour”, “labels”, “legend”, “padding”.'}</span></div>`));
  else out.prepend(h(`<div class="sr-count">${n} match${n === 1 ? '' : 'es'} · change them here, or open a section</div>`));
  return out;
}

/** The label with the matched words marked. */
function highlight(text, tq) {
  let html = esc(text);
  const words = tq.flat().filter((w) => w.length > 1).sort((a, z) => z.length - a.length);
  for (const w of words) {
    const re = new RegExp(`(${w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'ig');
    if (re.test(norm(text))) { html = html.replace(new RegExp(`(?![^<]*>)(${w.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')})`, 'i'), '<mark>$1</mark>'); break; }
  }
  return html;
}

function swatches(w, onPick, custom) {
  const pal = S.st.doc.theme.palette;
  const cur = w.style?.color || null;
  const el = h(`<div class="swatches-row"></div>`);
  const auto = h(`<button class="sw auto${!cur ? ' on' : ''}" title="Theme default">A</button>`);
  auto.addEventListener('click', () => onPick(null));
  el.append(auto);
  // the theme's colours by name, so the note follows the theme
  const th = S.st.doc.theme;
  for (const [v, n, c] of [...themeColours(th).main.filter(([x]) => ['$accent', '$good', '$bad', '$ink'].includes(x)), ...themeColours(th).series]) {
    const b = h(`<button class="sw${cur && (cur === v || cur.toLowerCase() === c.toLowerCase()) ? ' on' : ''}" style="background:${c}" title="${esc(n)}"></button>`);
    b.addEventListener('click', () => onPick(v));
    el.append(b);
  }
  if (custom) {
    const more = h(`<button class="sw custom" title="Custom colour">${icon('plus', 12)}</button>`);
    more.addEventListener('click', (e) => {
      e.stopPropagation();
      const pop = h(`<div class="menu cf-pop"></div>`);
      pop.append(colorPicker(/^#[0-9a-f]{6}$/i.test(cur || '') ? cur : pal[0], (c) => { closeLayer(); onPick(c); }));
      popover(more, pop);
    });
    el.append(more);
  }
  return el;
}

const upd = (id, patch) => edit({ op: 'update_widget', id, patch });

export function renderRight() {
  if (!right) return;
  if (!S.st?.doc) { right.innerHTML = ''; return; }
  // keep focus and caret across re-renders
  const a = document.activeElement;
  const key = right.contains(a) ? a.dataset?.key : null;
  const sel = key && 'selectionStart' in a ? [a.selectionStart, a.selectionEnd] : null;
  const scroll = right.querySelector('.insp')?.scrollTop || 0;
  const f = S.sel ? findWidget(S.sel) : null;
  if (S.colEdit && S.colEdit.id !== S.sel) S.colEdit = null;
  const el = f && S.more.length ? multiInspector(selection().map(findWidget).filter(Boolean)) : f ? widgetInspector(f) : docInspector();
  right.replaceChildren(el);
  el.scrollTop = scroll;
  if (key) {
    const n = right.querySelector(`[data-key="${CSS.escape(key)}"]`);
    if (n) { n.focus({ preventScroll: true }); if (sel && 'setSelectionRange' in n) try { n.setSelectionRange(...sel); } catch { /* not a text input */ } }
  }
}

// ---------------------------------------------------------------- contents

/** Text as a page shows it, roughly: markup removed, one line; null if it's only placeholders. */
function plainText(s) {
  const t = String(s || '').replace(/\{\$[a-z_]+\|([^}]*)\}/gi, '$1').replace(/\{#[0-9a-f]+\|([^}]*)\}/gi, '$1').replace(/\*+/g, '').replace(/^- /gm, '').replace(/\s+/g, ' ').trim();
  return t.replace(/\{\{[^}]*\}\}/g, '').replace(/[\s·,–—-]+/g, '') ? t : null;
}

/** The first sentence of a page's text or comment, as a description. */
function firstSentence(p) {
  const w = [...p.widgets].sort((a, b) => a.y - b.y || a.x - b.x).find((x) => (x.kind === 'text' || x.kind === 'comment') && plainText(x.opts?.text));
  if (!w) return '';
  const t = plainText(w.opts.text) || '';
  const m = t.match(/^(.{20,110}?[.!?])(\s|$)/);
  return m ? m[1] : t.length > 110 ? `${t.slice(0, 107).trim()}…` : t;
}

/** Pages a contents can list: not continuation pages, not the contents page. */
const tocListable = (p) => !p.auto && !p.widgets.some((w) => w.kind === 'toc');

/** Contents settings and entries: where titles come from, which pages are in, and a helper that fills gaps. */
function tocSource(f) {
  const w = f.w;
  const doc = S.st.doc;
  const o = drawn(f).opts?.toc || {};
  const set = (k, v) => upd(w.id, { opts: { toc: { [k]: v } } });
  const all = o.include === 'all';
  const out = [];

  out.push(section(['Entries', { icon: 'toc' }],
    row('Titles from', seg('toc.titles', o.titles || 'pages', [['pages', 'Page titles'], ['headings', 'Headings']], (v) => set('titles', v))),
    row('Descriptions', seg('toc.descFrom', o.descFrom || 'pages', [['pages', 'Page notes'], ['headings', 'Subtitles']], (v) => set('descFrom', v))),
    row('Pages', seg('toc.include', all ? 'all' : 'sections', [['sections', 'In a section'], ['all', 'All']], (v) => set('include', v))),
    h(`<div class="hint">${o.titles === 'headings' ? 'Each entry reads the page’s main heading, so the contents stays up to date as you edit the pages.' : 'Entries use the page titles you set. Choose <b>Headings</b> to follow each page’s main heading instead.'} Page numbers always follow the pages.</div>`),
  ));

  // the entries, then what is left out and why
  const list = h(`<div class="toc-entries"></div>`);
  const listed = new Set();
  for (const e of f.vm.toc || []) {
    const at = e.index ?? e.page - 1;
    if (e.kind === 'page') listed.add(at);
    const b = h(`<div class="toc-entry ${e.kind}"><button class="te-main" title="${e.kind === 'page' ? 'Edit this page’s title, section and description' : 'Edit the first page of this section'}"><span class="te-t">${esc(e.text)}</span>${e.kind === 'page' && e.description ? `<span class="te-d">${esc(e.description)}</span>` : ''}</button><span class="te-n">${e.page}</span></div>`);
    b.querySelector('.te-main').addEventListener('click', () => renamePage(at));
    if (e.kind === 'page') {
      const x = h(`<button class="tb-btn sm te-x" title="Leave this page out of the contents">${icon('eye', 13)}</button>`);
      x.addEventListener('click', () => edit({ op: 'set_page_props', page: doc.pages[at].id, group: null, label: null, frame: null, toc_hidden: true }));
      b.append(x);
    }
    list.append(b);
  }
  const left = doc.pages.map((p, i) => [p, i]).filter(([p, i]) => tocListable(p) && !listed.has(i));
  const why = (p) => (p.toc_hidden ? 'hidden' : p.plain ? 'cover page' : !p.section ? 'no section' : '');
  const items = [];
  if (list.children.length) items.push(list);
  else items.push(h(`<div class="hint">No pages are listed yet.</div>`));
  if (left.length) {
    const lo = h(`<div class="toc-left"><div class="tl-title">Left out · ${left.length}</div></div>`);
    for (const [p, i] of left) {
      const r = h(`<div class="toc-entry out"><button class="te-main" title="Edit this page"><span class="te-t">${esc(p.title)}</span><span class="te-d">${why(p)}</span></button><span class="te-n">${i + 1}</span></div>`);
      r.querySelector('.te-main').addEventListener('click', () => renamePage(i));
      if (!p.plain) {
        const add = h(`<button class="tb-btn sm te-x" title="Add to the contents">${icon('plus', 13)}</button>`);
        add.addEventListener('click', () => {
          if (p.toc_hidden) return edit({ op: 'set_page_props', page: p.id, group: null, label: null, frame: null, toc_hidden: false });
          if (all) return;
          // a page needs a section: take the one before it
          const prev = doc.pages.slice(0, i).reverse().find((x) => x.section)?.section || doc.pages.slice(i + 1).find((x) => x.section)?.section || 'Contents';
          edit({ op: 'set_page_section', page: p.id, section: prev });
        });
        r.append(add);
      }
      lo.append(r);
    }
    items.push(lo);
  }
  const fill = h(`<button class="btn sm">${icon('wand', 13)} Fill in from the report…</button>`);
  fill.addEventListener('click', () => tocAssistant(f));
  items.push(fill, h(`<div class="hint">Finds missing sections, titles and descriptions in the pages’ headings and text, and lets you review them before applying.</div>`));
  out.push(section(['Pages in the contents', { icon: 'page', badge: left.length || null }], items));
  return out;
}

/** Proposes each page's section, title and description from its content, for review. */
async function tocAssistant(f) {
  const doc = S.st.doc;
  const rows = [];
  let section = '';
  doc.pages.forEach((p, i) => {
    if (!tocListable(p) || p.plain) return;
    const head = S.st.pages[i]?.heading || null;
    const sec = p.section || section;
    section = sec;
    const title = head && head[0] && head[0] !== p.title && p.title.match(/^Page \d+$|^Страница \d+$|^$/) ? head[0] : p.title;
    const altTitle = head?.[0] && head[0] !== p.title ? head[0] : null;
    const sub = head?.[1] && /\p{L}{3}/u.test(head[1]) ? head[1] : '';
    const desc = p.description || sub || firstSentence(p);
    rows.push({ p, i, sec, title, altTitle, desc, changed: sec !== p.section || title !== p.title || desc !== (p.description || '') || !!p.toc_hidden });
  });
  const body = h(`<div class="toc-assist">
    <div class="ta-top"><span class="hint"></span><label class="tgl"><input type="checkbox" id="ta-all"><span class="track"></span><span>All pages</span></label></div>
    <div class="ta-head"><span></span><span>#</span><span>Section</span><span>Title</span><span>Description</span></div>
    <div class="ta-rows"></div>
    <datalist id="ta-secs">${[...new Set(doc.pages.map((p) => p.section).filter(Boolean))].map((x) => `<option value="${esc(x)}">`).join('')}</datalist>
  </div>`);
  const n = rows.filter((r) => r.changed).length;
  body.querySelector('.hint').textContent = n ? `${n} page${n > 1 ? 's' : ''} can be filled in. Suggestions are highlighted; edit anything before applying.` : 'Every page already has a section, title and description. Turn on “All pages” to review them.';
  const host = body.querySelector('.ta-rows');
  const draw = (showAll) => {
    host.replaceChildren();
    for (const r of rows) {
      if (!showAll && !r.changed) continue;
      const el = h(`<div class="ta-row">
        <input type="checkbox" class="ta-on"${r.changed ? ' checked' : ''}>
        <span class="ta-n">${r.i + 1}</span>
        <input class="ta-sec${r.sec !== r.p.section ? ' sugg' : ''}" list="ta-secs" value="${esc(r.sec)}" placeholder="Section">
        <span class="ta-tw"><input class="ta-title${r.title !== r.p.title ? ' sugg' : ''}" value="${esc(r.title)}"></span>
        <input class="ta-desc${r.desc !== (r.p.description || '') ? ' sugg' : ''}" value="${esc(r.desc)}" placeholder="One line about the page">
      </div>`);
      if (r.altTitle && r.altTitle !== r.title) {
        const use = h(`<button class="ta-alt" title="Use the page’s heading">↳ ${esc(r.altTitle)}</button>`);
        use.addEventListener('click', () => { el.querySelector('.ta-title').value = r.altTitle; el.querySelector('.ta-title').classList.add('sugg'); el.querySelector('.ta-on').checked = true; });
        el.querySelector('.ta-tw').append(use);
      }
      el.querySelectorAll('input:not(.ta-on)').forEach((x) => {
        x.addEventListener('keydown', (e) => e.stopPropagation());
        x.addEventListener('input', () => { el.querySelector('.ta-on').checked = true; });
      });
      el._r = r;
      host.append(el);
    }
    if (!host.children.length) host.append(h(`<div class="hint" style="padding:12px">Nothing to fill in.</div>`));
  };
  draw(false);
  body.querySelector('#ta-all').addEventListener('change', (e) => draw(e.target.checked));
  const res = await modal({ title: 'Fill in the contents from the report', iconName: 'wand', body, wide: true, buttons: [{ label: 'Cancel', value: null }, { label: 'Apply', kind: 'primary', value: (b) => [...b.querySelectorAll('.ta-row')].filter((el) => el.querySelector('.ta-on').checked).map((el) => ({ r: el._r, sec: el.querySelector('.ta-sec').value.trim(), title: el.querySelector('.ta-title').value.trim(), desc: el.querySelector('.ta-desc').value.trim() })) }] });
  if (!res || !res.length) return;
  const edits = [];
  for (const { r, sec, title, desc } of res) {
    if (title && title !== r.p.title) edits.push({ op: 'rename_page', page: r.p.id, title });
    if (sec !== (r.p.section || '')) edits.push({ op: 'set_page_section', page: r.p.id, section: sec });
    if (desc !== (r.p.description || '') || r.p.toc_hidden) edits.push({ op: 'set_page_props', page: r.p.id, group: null, label: null, frame: null, description: desc, toc_hidden: false });
  }
  if (!edits.length) return;
  if (await edit({ op: 'batch', edits })) toast(`Contents updated: ${res.length} page${res.length > 1 ? 's' : ''}. Undo reverts it in one step.`);
}

/** The colours a chart's icon chips are drawn in when none are set (as `chip` in charts.js and `glyph` draw them). */
function chipAuto(kind, eff) {
  const th = S.st.doc.theme;
  const color = themeColor(th, eff.iconColor) || th.ink;
  const fill = kind === 'tiles' ? 'transparent' : mixHex(th.ink, th.page_bg, 0.87);
  const back = themeColor(th, eff.iconFill) || fill;
  // the outline: the icon colour, dimmed towards the chip (or, on a see-through chip, faint)
  const stroke = /^#[0-9a-f]{6}$/i.test(back) ? mixHex(color, back, 0.62) : mixHex(color, th.page_bg, 0.62);
  return { fill, color, stroke };
}

/** Chip shapes behind an icon. */
const CHIP_SHAPES = [['circle', 'Circle'], ['square', 'Rounded'], ['sharp', 'Square'], ['none', 'None']];

/**
 * The chip around an icon, the same everywhere: shape, background, icon
 * colour and an outline. `cur` holds the values under the names in `keys`;
 * `set(patch)` writes them (null resets one).
 */
function chipRows(key, cur, set, { keys = {}, shape = 'circle', withShape = true, auto = null } = {}) {
  const K = { shape: 'shape', fill: 'fill', color: 'color', stroke: 'stroke', width: 'strokeWidth', ...keys };
  const sh = cur[K.shape] || shape;
  const put = (k, v) => set({ [K[k]]: v });
  const rows = [];
  if (withShape) rows.push(row('Chip', seg(`${key}.shape`, sh, CHIP_SHAPES, (v) => put('shape', v))));
  if (sh !== 'none') rows.push(row('Chip background', colorField(`${key}.fill`, cur[K.fill] || null, (v) => put('fill', v), { title: 'Chip background', clear: true, auto: auto?.fill })));
  rows.push(row('Icon colour', colorField(`${key}.color`, cur[K.color] || null, (v) => put('color', v), { title: 'Icon colour (Auto: the text colour)', auto: auto?.color })));
  if (sh !== 'none' && auto) {
    // the chip's quiet outline, in a dimmed icon colour, is drawn unless switched off ("none")
    const on = cur[K.stroke] !== 'none';
    rows.push(row('Chip outline', toggle(`${key}.stroke`, on, (v) => set({ [K.stroke]: v ? null : 'none', [K.width]: null }))));
    if (on) rows.push(row('Outline', pair2(
      textInput(`${key}.width`, cur[K.width] ?? '', (v) => put('width', v == null ? null : Math.max(0.25, Math.min(4, v))), { type: 'number', min: 0.25, max: 4, placeholder: 'auto' }),
      colorField(`${key}.scol`, cur[K.stroke] || null, (v) => put('stroke', v || null), { title: 'Outline colour', auto: auto.stroke }))));
  } else if (sh !== 'none') {
    const on = !!cur[K.stroke];
    rows.push(row('Chip outline', toggle(`${key}.stroke`, on, (v) => set({ [K.stroke]: v ? S.st.doc.theme.muted : null, [K.width]: v ? cur[K.width] ?? null : null }))));
    if (on) rows.push(row('Outline', pair2(
      textInput(`${key}.width`, cur[K.width] ?? '', (v) => put('width', v == null ? null : Math.max(0.25, Math.min(4, v))), { type: 'number', min: 0.25, max: 4, placeholder: '0.75' }),
      colorField(`${key}.scol`, cur[K.stroke], (v) => put('stroke', v || S.st.doc.theme.muted), { none: false, title: 'Outline colour' }))));
  }
  return rows;
}

/**
 * Lines over a column chart: any value can be drawn as a line (a running
 * total, a target), or a new line added from the data.
 */
/** Charts whose series each get their own settings (Design → Series). */
const SERIES_KINDS = ['bar', 'line', 'area'];

/**
 * Each series of a column, line or area chart: its name in the legend, colour, columns or line, the
 * axis it reads against (a right axis for a second scale), its line and markers, and its labels.
 */
function seriesSection(f) {
  const w = f.w;
  if (!SERIES_KINDS.includes(w.kind) || !w.binding?.dataset) return null;
  const d = drawn(f);
  const orient = d.opts?.orientation || d.opts?.viz?.orientation || 'vertical';
  if (w.kind === 'bar' && orient !== 'vertical') return null;
  const data = f.vm.data;
  if (!data || data.empty || !data.columns) return null;
  const sh = shape(data);
  if (!sh.series.length) return null;
  const b = w.binding;
  const ms = b.measures || [];
  const eff = resolveViz(S.st, d);
  const own = w.opts?.viz?.seriesOpts || {};
  const pal = S.st.doc.theme.palette;
  const setMs = (n) => upd(w.id, { binding: { ...b, measures: n } });
  const isLine = (i) => w.kind !== 'bar' || (!sh.named && ms[i]?.chart === 'line');
  const barsLeft = sh.series.filter((_, i) => !isLine(i)).length;
  const many = sh.series.length > 1;
  const rows = [];
  let count = 0;
  sh.series.forEach((x, i) => {
    const key = seriesKey(w, sh, i);
    const o = own[key] || {};
    const so = { ...(eff.seriesOpts?.[key] || {}), ...o };
    const sset = (k, v) => upd(w.id, { opts: { viz: { seriesOpts: { [key]: { [k]: v } } } } });
    const line = isLine(i);
    count += Object.values(o).filter((v) => v != null && v !== '').length + (ms[i]?.chart === 'line' ? 1 : 0);
    // the colour as drawn (a theme reference such as "$ink" resolved)
    const cur = themeColor(S.st.doc.theme, eff.colors?.[x.name] || (!sh.named && ms[i]?.color) || (line && w.kind === 'bar' && eff.lineColor) || (Array.isArray(eff.palette) && eff.palette[i]) || pal[i % pal.length]) || (line && w.kind === 'bar' && barsLeft ? S.st.doc.theme.ink : pal[i % pal.length]);
    const card = h(`<div class="ser-card" data-series="${esc(x.name)}"><div class="ser-head"></div><div class="ser-body"></div></div>`);
    const head = card.querySelector('.ser-head');
    // its colour (set under Colours), then the name the legend shows
    const cm = line ? 'auto' : so.colorMode ?? (barsLeft === 1 ? eff.colorMode : eff.colorMode === 'sign' ? 'sign' : 'auto');
    const sgn = !line && (cm === 'sign' || (cm === 'signOne' && barsLeft === 1));
    const th0 = S.st.doc.theme;
    const dotBg = sgn ? `linear-gradient(135deg, ${themeColor(th0, so.colorUp) || eff.colorUp} 50%, ${themeColor(th0, so.colorDown) || eff.colorDown} 50%)` : cur || '';
    head.append(h(`<i class="ser-dot" style="background:${esc(dotBg)}" title="Colours: under Colours"></i>`));
    head.append(textInput(`ser.${i}.name`, o.name ?? '', (v) => sset('name', v ? String(v).trim() || null : null), { placeholder: x.name }));
    const body = card.querySelector('.ser-body');
    if (w.kind === 'bar' && !sh.named) {
      const r = row('Show as', seg(`ser.${i}.as`, line ? 'line' : 'bar', [['bar', 'Columns'], ['line', 'Line']], (v) => { const n = clone(ms); n[i].chart = v === 'line' ? 'line' : null; setMs(n); }));
      // at least one value stays as columns
      if (!line && barsLeft <= 1) { r.querySelectorAll('button').forEach((x2) => { x2.disabled = true; }); r.title = 'The only columns: keep at least one value as columns'; }
      body.append(mark(r, ms[i]?.chart === 'line', () => { const n = clone(ms); n[i].chart = null; setMs(n); }, 'columns'));
    }
    if (many) body.append(mark(row('Axis', seg(`ser.${i}.ax`, so.axis === 'right' ? 'right' : 'left', [['left', 'Left'], ['right', 'Right']], (v) => sset('axis', v === 'right' ? 'right' : null))), o.axis != null, () => sset('axis', null), 'the left axis'));
    rows.push(card);
  });
  if (sh.series.some((_, i) => (own[seriesKey(w, sh, i)] || {}).axis === 'right')) {
    rows.push(h(`<div class="hint">The right axis has its own scale: its labels take the colour of the series on it, and the legend marks it. Its minimum, maximum and units are under <b>Axes & grid</b>. Two scales on one chart can suggest a link that isn't there: use them for related measures (e.g. an amount and its return).</div>`));
  }
  if (w.kind === 'bar' && !sh.named) {
    const nums = fieldsOf(b.dataset).filter((x) => x.kind === 'number');
    if (nums.length) {
      const add = h(`<button class="btn sm">${icon('plus', 13)} Add a line</button>`);
      add.addEventListener('click', (e) => menu(e.currentTarget, nums.map((x) => ({ label: x.name, run: () => setMs([...ms, { id: nextMeasureId(ms), field: x.name, agg: 'sum', label: cap(x.name), format: ms[0]?.format || 'number', chart: 'line' }]) }))));
      rows.push(add);
    }
  }
  return { rows, count };
}

/** Settings a chart with two or more series has a part of for each series (the rest of their sections stay common). */
const PER_SERIES = ['colorMode', 'colorUp', 'colorDown', 'tint', 'outline', 'outlineWidth', 'outlineColor', 'barFill', 'lineColor', 'lineWidth',
  'lineStyle', 'curve', 'markers', 'markerSize', 'markerFill', 'areaFill', 'areaOpacity', 'extremes', 'extremesRule', 'bands', 'callout',
  'labels', 'labelPos', 'labelColor', 'labelBold', 'labelBox', 'labelUnits', 'labelDecimals', 'labelSuffix'];

/**
 * A chart with two or more series: in Colours, Marks, Highlights and Data labels a part for each series under its
 * name — as Axes & grid has one per axis — with what that series can have of its own. Unset, a series looks as
 * the chart drew it. Stacked columns are labelled with their totals and mark no best or worst: those stay common.
 */
function seriesParts(f) {
  const w = f.w;
  if (!SERIES_KINDS.includes(w.kind) || !w.binding?.dataset) return null;
  const d = drawn(f);
  const orient = d.opts?.orientation || d.opts?.viz?.orientation || 'vertical';
  if (w.kind === 'bar' && orient !== 'vertical') return null;
  const data = f.vm.data;
  if (!data || data.empty || !data.columns) return null;
  const sh = shape(data);
  if (sh.series.length < 2) return null;
  const eff = resolveViz(S.st, d);
  const own = w.opts?.viz?.seriesOpts || {};
  const b = w.binding;
  const ms = b.measures || [];
  const th = S.st.doc.theme;
  const pal = th.palette;
  const isLine = (i) => w.kind !== 'bar' || (!sh.named && ms[i]?.chart === 'line');
  const nBars = sh.series.filter((_, i) => !isLine(i)).length, nLines = sh.series.length - nBars;
  const stacked = !!eff.stacked && nBars > 1;
  const out = { Colours: [], Marks: [], Highlights: [], 'Data labels': [] };
  const count = { Colours: 0, Marks: 0, Highlights: 0, 'Data labels': 0 };
  let anyLabels = false;
  const range = (key, v, min, max, step, fmt, onSet) => {
    const el = h(`<div class="vz-range"><input type="range" data-key="${key}" min="${min}" max="${max}" step="${step}" value="${v}"><span>${esc(fmt(+v))}</span></div>`);
    const inp = el.querySelector('input'), lab = el.querySelector('span');
    inp.addEventListener('input', () => { lab.textContent = fmt(+inp.value); });
    inp.addEventListener('change', () => onSet(+inp.value));
    return el;
  };
  const pct = (v) => `${Math.round(v * 100)}%`;
  sh.series.forEach((x, i) => {
    const key = seriesKey(w, sh, i);
    const o = own[key] || {};
    const so = { ...(eff.seriesOpts?.[key] || {}), ...o };
    const sset = (k, v) => upd(w.id, { opts: { viz: { seriesOpts: { [key]: { [k]: v } } } } });
    const line = isLine(i);
    const m0 = so.colorMode ?? (nBars === 1 ? eff.colorMode : eff.colorMode === 'sign' ? 'sign' : 'auto');
    const bySign0 = !line && (m0 === 'sign' || (m0 === 'signOne' && nBars === 1));
    const col = themeColor(th, eff.colors?.[x.name] || (!sh.named && ms[i]?.color) || (line && w.kind === 'bar' && eff.lineColor) || pal[i % pal.length]) || pal[i % pal.length];
    const dot = () => (bySign0 ? `linear-gradient(135deg, ${so.colorUp ? themeColor(th, so.colorUp) : eff.colorUp} 50%, ${so.colorDown ? themeColor(th, so.colorDown) : eff.colorDown} 50%)` : col);
    const head = () => h(`<div class="vz-sub ser-sub"><i style="background:${esc(dot())}"></i>${esc(so.name || x.name)}<span class="ser-kind">${line ? 'line' : 'columns'}</span></div>`);
    for (const t of Object.keys(out)) out[t].push(head());
    // a row of this series: its own value, else the chart's; a set one is marked and resets on a double-click
    const put = (sec, label, k, ctl) => {
      const r = row(label, ctl);
      if (o[k] != null && o[k] !== '') { count[sec]++; resettable(r, r.querySelector('label'), () => sset(k, null)); }
      out[sec].push(r);
    };
    const val = (k, fb = eff[k]) => so[k] ?? fb;
    const k_ = (n) => `sr.${i}.${n}`;

    // ---- colours
    const ownCol = w.opts?.viz?.colors?.[x.name];
    // columns: by their own colour, or up / down by sign (then those two are its colours)
    const mode = line ? 'auto' : val('colorMode', nBars === 1 ? eff.colorMode : eff.colorMode === 'sign' ? 'sign' : 'auto');
    const bySign = !line && (mode === 'sign' || (mode === 'signOne' && nBars === 1));
    if (!line) put('Colours', 'Colour by', 'colorMode', seg(k_('cm'), bySign ? 'sign' : 'auto', [['auto', 'Its colour'], ['sign', 'Up / down']], (v) => sset('colorMode', v)));
    if (bySign) {
      put('Colours', 'Up', 'colorUp', colorField(k_('cu'), so.colorUp || eff.colorUp, (c) => sset('colorUp', c), { none: false, title: 'Up / positive' }));
      put('Colours', 'Down', 'colorDown', colorField(k_('cd'), so.colorDown || eff.colorDown, (c) => sset('colorDown', c), { none: false, title: 'Down / negative' }));
    } else {
      out.Colours.push(row('Colour', colorField(k_('c'), typeof ownCol === 'string' && ownCol[0] === '$' ? ownCol : col, (c) => upd(w.id, { opts: { viz: { colors: { [x.name]: c } } }, ...(!sh.named && ms[i]?.color ? { binding: { ...b, measures: ms.map((m, j) => (j === i ? { ...m, color: null } : m)) } } : {}) }), { none: false, title: x.name })));
    }

    // ---- marks
    if (!line) {
      put('Marks', 'Lighten fills', 'tint', range(k_('tint'), val('tint') ?? 0, 0, 0.9, 0.05, pct, (v) => sset('tint', v)));
      const ol = !!val('outline');
      put('Marks', 'Outline', 'outline', toggle(k_('ol'), ol, (v) => sset('outline', v)));
      if (ol) {
        put('Marks', 'Outline width', 'outlineWidth', textInput(k_('olw'), o.outlineWidth ?? '', (v) => sset('outlineWidth', v == null ? null : Math.max(0.25, Math.min(6, v))), { type: 'number', min: 0.25, max: 6, step: 0.25, placeholder: String(eff.outlineWidth ?? 0.5) }));
        put('Marks', 'Outline colour', 'outlineColor', colorField(k_('olc'), so.outlineColor || null, (c) => sset('outlineColor', c), { title: 'Outline colour', auto: eff.outlineColor || mixHex(bySign ? eff.colorUp : col, '#000000', 0.35) }));
      }
      put('Marks', 'Fill', 'barFill', seg(k_('bf'), val('barFill') || 'flat', [['flat', 'Solid'], ['gradient', 'Fade to axis']], (v) => sset('barFill', v)));
    } else {
      // a line's width and style keep their old names (width, dash)
      const lr = (label, k, ctl) => { const r = row(label, ctl); if (o[k] != null) { count.Marks++; resettable(r, r.querySelector('label'), () => sset(k, null)); } out.Marks.push(r); };
      lr('Width', 'width', textInput(k_('lw'), o.width ?? '', (v) => sset('width', v == null || v === '' ? null : Math.max(0.5, Math.min(8, +v))), { type: 'number', min: 0.5, max: 8, step: 0.25, placeholder: String(eff.lineWidth ?? 2) }));
      lr('Style', 'dash', seg(k_('ls'), so.dash || eff.lineStyle || 'solid', [['solid', 'Solid'], ['dashed', 'Dashed'], ['dotted', 'Dotted']], (v) => sset('dash', v)));
      put('Marks', 'Shape', 'curve', seg(k_('cv'), val('curve') || 'straight', [['straight', 'Straight'], ['smooth', 'Smooth']], (v) => sset('curve', v)));
      const mk = val('markers') || 'none';
      put('Marks', 'Markers', 'markers', seg(k_('mk'), mk, [['none', 'None'], ['last', 'Last'], ['all', 'All']], (v) => sset('markers', v)));
      if (mk !== 'none') {
        put('Marks', 'Marker size', 'markerSize', textInput(k_('ms'), o.markerSize ?? '', (v) => sset('markerSize', v == null ? null : Math.max(1, Math.min(8, v))), { type: 'number', min: 1, max: 8, step: 0.2, placeholder: String(eff.markerSize ?? 2.8) }));
        put('Marks', 'Marker fill', 'markerFill', seg(k_('mf'), val('markerFill') || 'solid', [['solid', 'Solid'], ['hollow', 'Hollow']], (v) => sset('markerFill', v)));
      }
      if (w.kind !== 'area') {
        const area = o.areaFill ?? (nLines === 1 && !nBars ? eff.areaFill : 'none') ?? 'none';
        put('Marks', 'Area under', 'areaFill', seg(k_('af'), area || 'none', [['none', 'None'], ['flat', 'Flat'], ['gradient', 'Fading']], (v) => sset('areaFill', v)));
        if (area && area !== 'none') put('Marks', 'Area opacity', 'areaOpacity', range(k_('ao'), val('areaOpacity') ?? 0.12, 0, 0.8, 0.02, pct, (v) => sset('areaOpacity', v)));
      }
    }

    // ---- highlights: best & worst (side-by-side columns), a callout
    if (!line && !stacked) {
      const ext = !!(o.extremes ?? (nBars === 1 ? eff.extremes : false));
      put('Highlights', 'Mark best & worst', 'extremes', toggle(k_('ex'), ext, (v) => sset('extremes', v)));
      if (ext) {
        put('Highlights', 'Best / worst', 'extremesRule', selectInput(k_('er'), val('extremesRule') || 'sign', [['sign', 'Gains & losses only'], ['rank', 'Always highest & lowest']], (v) => sset('extremesRule', v)));
        put('Highlights', 'Background bands', 'bands', toggle(k_('bd'), !!val('bands'), (v) => sset('bands', v)));
      }
    }
    if (!(stacked && !line)) {
      const call = o.callout ?? (line ? (nLines === 1 && eff.callout === 'last' ? 'last' : 'none') : (nBars === 1 && eff.callout === 'max' ? 'max' : 'none'));
      put('Highlights', 'Callout', 'callout', seg(k_('co'), call, [['none', 'None'], ['last', 'Last'], ['max', 'Largest']], (v) => sset('callout', v)));
    }

    // ---- data labels (stacked columns: their totals, common)
    if (stacked && !line) { out['Data labels'].pop(); return; }
    const shown = val('labels') || 'auto';
    if (shown !== 'none') anyLabels = true;
    put('Data labels', 'Show', 'labels', selectInput(k_('lb'), shown, LABELS, (v) => sset('labels', v)));
    if (shown === 'none') return;
    if (line) put('Data labels', 'Position', 'labelPos', seg(k_('lp'), ['inside', 'below'].includes(val('labelPos')) ? 'below' : 'outside', [['outside', 'Above'], ['below', 'Below']], (v) => sset('labelPos', v)));
    else put('Data labels', 'Position', 'labelPos', selectInput(k_('lp'), val('labelPos') || 'auto', [['auto', 'Auto'], ['outside', 'Outside'], ['inside', 'Inside'], ['below', 'Next to axis']], (v) => sset('labelPos', v)));
    put('Data labels', 'Colour', 'labelColor', selectInput(k_('lc'), val('labelColor') || 'auto', [['auto', 'Auto'], ['series', 'Series colour'], ['sign', 'Up / down'], ['ink', 'Text'], ['muted', 'Muted']], (v) => sset('labelColor', v)));
    put('Data labels', 'Bold', 'labelBold', toggle(k_('lbo'), !!val('labelBold'), (v) => sset('labelBold', v)));
    put('Data labels', 'Frame', 'labelBox', seg(k_('lx'), val('labelBox') || 'none', [['none', 'Plain'], ['box', 'Box'], ['pill', 'Pill']], (v) => sset('labelBox', v)));
    if (!(!sh.named && ms[i]?.format === 'percent')) put('Data labels', 'Units', 'labelUnits', selectInput(k_('lu'), val('labelUnits') ?? '', [['', 'As values'], ['none', 'Full number'], ...UNITS.slice(1)], (v) => sset('labelUnits', v || null)));
    put('Data labels', 'Decimals', 'labelDecimals', selectInput(k_('ld'), val('labelDecimals') ?? '', [['', 'As values'], ['0', '0'], ['1', '1'], ['2', '2'], ['3', '3']], (v) => sset('labelDecimals', v === '' ? null : +v)));
    put('Data labels', 'Suffix', 'labelSuffix', textInput(k_('ls2'), o.labelSuffix ?? '', (v) => sset('labelSuffix', v === '' ? null : v), { placeholder: val('labelSuffix') ?? 'as values' }));
  });
  // a section where no series has anything of its own (only headings) has no parts
  for (const t of Object.keys(out)) if (!out[t].some((r) => r.classList?.contains('f-row'))) delete out[t];
  return { out, count, stacked, anyLabels };
}

/** A section's common rows: those of the chart's settings no series has its own of (and their headings, if any are left). */
function commonRows(rows) {
  const keep = rows.filter((r) => !PER_SERIES.some((k) => r.querySelector?.(`[data-key="viz.${k}"]`) || r.dataset?.key === `viz.${k}`));
  // a heading with nothing under it goes
  return keep.filter((r, i) => !r.classList?.contains('vz-sub') || (keep[i + 1] && !keep[i + 1].classList?.contains('vz-sub')));
}

/**
 * Icons a chart puts beside its categories (legends, tiles, treemaps): one per
 * category, found by name (flags, brands) or picked, with the chip's shape.
 */
function chartIcons(f) {
  const w = f.w;
  if (!['pie', 'treemap', 'bar', 'tiles'].includes(w.kind)) return null;
  const eff = resolveViz(S.st, drawn(f));
  const map = eff.icons && typeof eff.icons === 'object' ? eff.icons : {};
  const set = (patch) => upd(w.id, { opts: { viz: patch } });
  // only charts that draw icons in this form (a plain bar chart has none)
  const shown = iconSlots(w.id);
  if (!shown) return null;
  const cats = [...new Set((f.vm.data?.rows || []).map((r) => r[0]).filter((x) => x != null && x !== ''))].slice(0, 30).map(String);
  const off = !!eff.iconsOff;
  const show = row('Show icons', toggle('vz.ioff', !off, (v) => set({ iconsOff: v ? null : true })));
  if (off) return { rows: [show], count: 1 };
  const items = [
    show,
    row('Flags & logos', toggle('vz.iauto', !!eff.iconsAuto, (v) => set({ iconsAuto: v || null }), 'Find them by name')),
    ...chipRows('vz.i', eff, set, { keys: { shape: 'iconShape', fill: 'iconFill', color: 'iconColor', stroke: 'iconStroke', width: 'iconStrokeWidth' }, auto: chipAuto(w.kind, eff) }),
  ];
  if (cats.length) {
    items.push(h(`<div class="hint">Icon for each category</div>`));
    for (const c of cats) {
      const cur = glyphFor(map, c, '', false);
      items.push(row(c, iconPick(`vz.icon.${c}`, cur, (g) => set({ icons: { [c]: g || null } }), { none: true, title: `Icon for “${c}”` })));
    }
    const hidden = cats.filter((c) => !shown.has(c));
    if (hidden.length) items.push(h(`<div class="hint"><span>Not on the chart now, so their icons don't show. They are folded into “Other” for lack of room; give the chart more height to show them.</span> <b>${esc(hidden.join(', '))}</b></div>`));
  }
  return { rows: items, count: Object.keys(w.opts?.viz?.icons || {}).length + (w.opts?.viz?.iconsAuto != null ? 1 : 0) };
}

/** An icon button that opens the icon library: icons, flags, brands and the document's own logos. */
function iconPick(key, value, onChange, { none = false, title = 'Icon', chip = null } = {}) {
  const ic = (name, s = 16, shape = 'none') => { const wd = letterText(name) ? glyphWidthOf(name, s) : s; return `<svg viewBox="0 0 ${wd} ${s}" width="${wd}" height="${s}">${glyph(name, 0, 0, s, { shape, color: 'currentColor', fill: 'transparent' })}</svg>`; };
  const chipOf = (n) => (n.startsWith('flag-') || n.startsWith('brand-') || n.startsWith('up-') ? 'circle' : 'none');
  // the button shows the marker as drawn, in its chip's colours (`chip`: { fill, color }) where they're known
  const shown = (n) => { if (!chip || !letterText(n)) return ic(n, 16, chipOf(n)); const wd = glyphWidthOf(n, 16); return `<svg viewBox="0 0 ${wd} 16" width="${wd}" height="16">${glyph(n, 0, 0, 16, { shape: 'circle', fill: chip.fill, color: chip.color })}</svg>`; };
  const b = h(`<button class="icon-pick" data-key="${key}" title="${esc(title)}${value ? ` · ${esc(iconLabel(value))}` : ''}">${value ? shown(value) : '<span class="dim">Auto</span>'}${icon('chevd', 11)}</button>`);
  b.addEventListener('click', (e) => {
    e.stopPropagation();
    const el = h(`<div class="menu ip-pop">
      <input class="ip-search" placeholder="Search ${Object.keys(LINE).length + Object.keys(FLAGS).length + Object.keys(BRANDS).length + Object.keys(LOGOS).length} icons, flags, brands and companies">
      <div class="ip-tabs"></div>
      <div class="ip-cats"></div>
      <div class="ip-grid"></div>
      <div class="ip-foot"></div>
    </div>`);
    const grid = el.querySelector('.ip-grid'), tabsEl = el.querySelector('.ip-tabs'), cats = el.querySelector('.ip-cats'), foot = el.querySelector('.ip-foot');
    const q = el.querySelector('.ip-search');
    const st = { tab: letterText(value) ? 'letters' : value?.startsWith('flag-') ? 'flags' : value?.startsWith('brand-') ? 'brands' : value?.startsWith('up-') ? 'mine' : 'icons', cat: 'Report' };
    const pick = (n) => { closeLayer(); onChange(n || null); };
    const cell = (n, shape = chipOf(n)) => {
      // a letter badge is wider than its height: its own width
      const c = h(`<button class="ip-cell${n === (value || '') || (letterText(n) && letterText(n) === letterText(value)) ? ' on' : ''}" title="${esc(iconLabel(n))}">${ic(n, 20, shape)}</button>`);
      c.addEventListener('click', () => pick(n));
      return c;
    };
    const lists = () => {
      const words = norm(q.value).split(/\s+/).filter(Boolean);
      const hit = (text) => words.every((w) => norm(text).includes(w));
      if (words.length) {
        // search everything, best kinds first
        return [
          ...Object.entries(FLAGS).filter(([c, f]) => hit(`${f.name} ${f.words} ${c}`)).map(([c]) => `flag-${c}`),
          ...Object.entries(LOGOS).filter(([s, x]) => hit(`${x.name} ${x.words} ${s}`)).map(([s]) => `logo-${s}`),
          ...Object.entries(BRANDS).filter(([s, x]) => hit(`${x.name} ${s}`)).map(([s]) => `brand-${s}`),
          ...customIcons().filter((x) => hit(x.name)).map((x) => `up-${x.id}`),
          ...GLYPH_NAMES.filter((n) => hit(n)),
          ...Object.keys(LINE).filter((n) => !GLYPH_NAMES.includes(n) && hit(`${n} ${TAGS[n] || ''}`)),
        ].slice(0, 240);
      }
      if (st.tab === 'flags') return Object.keys(FLAGS).map((c) => `flag-${c}`);
      if (st.tab === 'brands') return Object.keys(BRANDS).map((s) => `brand-${s}`);
      if (st.tab === 'logos') return Object.keys(LOGOS).map((s) => `logo-${s}`);
      if (st.tab === 'mine') return customIcons().map((x) => `up-${x.id}`);
      if (st.cat === 'Report') return GLYPH_NAMES;
      if (st.cat === 'All') return Object.keys(LINE);
      return (CATEGORIES.find(([c]) => c === st.cat) || [, []])[1];
    };
    const draw = () => {
      const searching = !!q.value.trim();
      tabsEl.replaceChildren(...[['icons', 'Icons'], ['letters', 'Letters'], ['flags', `Flags · ${Object.keys(FLAGS).length}`], ['brands', `Brands · ${Object.keys(BRANDS).length}`], ['logos', `Companies · ${Object.keys(LOGOS).length}`], ['mine', 'My logos']].map(([k, l]) => {
        const t = h(`<button class="${!searching && st.tab === k ? 'on' : ''}">${esc(l)}</button>`);
        t.addEventListener('click', () => { st.tab = k; q.value = ''; draw(); });
        return t;
      }));
      cats.hidden = searching || st.tab !== 'icons';
      cats.replaceChildren(...['Report', ...CATEGORIES.map(([c]) => c), 'All'].map((c) => {
        const t = h(`<button class="chip${st.cat === c ? ' on' : ''}">${esc(c === 'All' ? `All ${Object.keys(LINE).length}` : c)}</button>`);
        t.addEventListener('click', () => { st.cat = c; draw(); });
        return t;
      }));
      // letters: any 1–8 typed, or one of the usual ones
      if (st.tab === 'letters' && !searching) {
        const form = h(`<div class="ip-letters"><div class="ip-lrow"><input class="ip-ltext" maxlength="8" placeholder="B, S, IN, OUT…" value="${esc(letterText(value) || '')}"><span class="ip-lprev"></span><button class="btn sm primary">Use</button></div><div class="hint">Up to 8. One letter makes a round badge, more a rounded one. Its colours are the chip's: set per value in the column's <b>Each value</b>.</div><div class="ip-lpicks"></div></div>`);
        const inp = form.querySelector('input'), prev = form.querySelector('.ip-lprev');
        const show = () => { const t = inp.value.trim(); prev.innerHTML = t ? ic(`txt:${t}`, 18) : ''; };
        inp.addEventListener('input', show);
        inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter' && inp.value.trim()) pick(`txt:${inp.value.trim()}`); });
        form.querySelector('.btn').addEventListener('click', () => inp.value.trim() && pick(`txt:${inp.value.trim()}`));
        // each as wide as its badge
        form.querySelector('.ip-lpicks').append(...['B', 'S', 'IN', 'OUT', 'BUY', 'SELL', '+', '−', 'Y', 'N', 'A', 'P', 'NEW', 'CLOSED'].map((t) => { const c = cell(`txt:${t}`); c.style.width = `${Math.ceil(glyphWidthOf(`txt:${t}`, 20)) + 12}px`; return c; }));
        show();
        grid.replaceChildren(form);
        foot.replaceChildren();
        if (none) { const x = h(`<button class="btn sm">Auto / none</button>`); x.addEventListener('click', () => pick('')); foot.append(x); }
        setTimeout(() => inp.focus(), 0);
        return;
      }
      const items = lists();
      grid.replaceChildren(...items.slice(0, st.cat === 'All' && !searching ? 400 : 300).map((n) => cell(n)));
      if (!items.length) grid.append(h(`<div class="ip-empty">${searching ? 'Nothing found. Try another word, a country, a currency code (AED) or a brand.' : st.tab === 'mine' ? 'No logos yet. Upload an SVG or a PNG: it travels inside the document.' : ''}</div>`));
      foot.replaceChildren();
      if (none) { const x = h(`<button class="btn sm">Auto / none</button>`); x.addEventListener('click', () => pick('')); foot.append(x); }
      foot.append(h('<span class="grow"></span>'));
      if (!userMode()) {
        const up = h(`<button class="btn sm">${icon('upload', 13)} Upload logo…</button>`);
        up.addEventListener('click', () => uploadIcon((id) => pick(`up-${id}`)));
        foot.append(up);
      }
    };
    q.addEventListener('input', draw);
    q.addEventListener('keydown', (e2) => e2.stopPropagation());
    draw();
    popover(b, el);
    setTimeout(() => q.focus(), 0);
  });
  return b;
}

/** Adds an SVG or picture to the document's own icons. */
function uploadIcon(done) {
  const inp = h('<input type="file" accept=".svg,.png,.jpg,.jpeg,.webp,image/svg+xml,image/png,image/jpeg,image/webp" hidden>');
  document.body.append(inp);
  inp.addEventListener('change', () => {
    const f = inp.files?.[0];
    inp.remove();
    if (!f) return;
    if (f.size > 1_500_000) { toast('That file is over 1.5 MB. Use a smaller logo.', 'error'); return; }
    const r = new FileReader();
    r.onload = async () => {
      const id = `i${Date.now().toString(36)}`;
      const name = f.name.replace(/\.[^.]+$/, '');
      const icons = [...(S.st.doc.icons || []), { id, name, href: String(r.result) }];
      if (await edit({ op: 'set_icons', icons })) { toast(`“${name}” added to this document’s logos.`); done(id); }
    };
    r.readAsDataURL(f);
  });
  inp.click();
}

/** Design settings of a contents widget: layout, page entries, section labels, icons. */
function tocDesign(f) {
  const w = f.w;
  const o = tocOpts(drawn(f).opts?.toc);
  const set = (k, v) => upd(w.id, { opts: { toc: { [k]: v } } });
  const chapters = o.style === 'sections';
  const layout = [
    row('Layout', seg('toc.style', o.style, [['list', 'List'], ['sections', 'Chapters']], (v) => {
      // explicit values: a design view underneath may hold the other layout's settings
      const ch = v === 'sections';
      upd(w.id, { opts: { toc: { style: v, show: 'all', cols: ch ? 2 : 1, numbering: ch ? 'roman' : 'none', label: ch ? null : '', descriptions: ch, rules: ch } } });
    })),
    h(`<div class="hint">${chapters ? 'Each section as a block with its number, icon and name, with its pages listed under it.' : 'Section labels in small capitals with their pages underneath.'}</div>`),
    row('Show', selectInput('toc.show', o.show, [['all', 'Sections and pages'], ['sections', 'Sections only'], ...(chapters ? [] : [['pages', 'Pages only']])], (v) => set('show', v))),
    row('Columns', seg('toc.cols', String(o.cols), [['1', '1'], ['2', '2'], ['3', '3']], (v) => set('cols', +v))),
    row('Spacing', seg('toc.spacing', String(o.spacing), [['0.85', 'Tight'], ['1', 'Normal'], ['1.25', 'Airy']], (v) => set('spacing', +v))),
  ];
  const pages = [
    row('Page numbers', toggle('toc.pageNums', o.pageNums, (v) => set('pageNums', v))),
    o.pageNums && row('Leader', seg('toc.leader', o.leader, [['dots', 'Dots'], ['line', 'Line'], ['none', 'None']], (v) => set('leader', v))),
    o.show !== 'sections' && row('Descriptions', toggle('toc.desc', o.descriptions, (v) => set('descriptions', v))),
    o.show !== 'sections' && row('Lines between', toggle('toc.rules', o.rules, (v) => set('rules', v))),
  ];
  const secs = o.show === 'pages' ? [] : [
    row('Numbering', selectInput('toc.numbering', o.numbering, [['none', 'None'], ['roman', 'I, II, III'], ['arabic', '1, 2, 3'], ['alpha', 'A, B, C']], (v) => set('numbering', v))),
    row('Label', textInput('toc.label', o.label, (v) => set('label', v), { placeholder: 'e.g. Section, Chapter' })),
    row('Colour', colorField('s.color', w.style?.color || null, (c) => upd(w.id, { style: { color: c } }), { title: 'Section colour' })),
  ];
  if (!chapters) layout.push(h(`<div class="hint">Section icons appear in the Chapters layout.</div>`));
  const out = [section(['Contents', { icon: 'toc' }], layout)];
  if (chapters) {
    const icons = [
      row('Icon chip', selectInput('toc.iconShape', o.iconShape, [...CHIP_SHAPES, ['hide', 'No icons']], (v) => set('iconShape', v))),
    ];
    if (o.iconShape !== 'hide') {
      icons.push(row('Size', selectInput('toc.iconSize', String(o.iconSize), [['0.5', '50%'], ['0.6', '60%'], ['0.75', '75%'], ['0.9', '90%'], ['1', '100%'], ['1.2', '120%'], ['1.4', '140%']], (v) => set('iconSize', +v))));
      icons.push(...chipRows('toc.i', o, (p) => upd(w.id, { opts: { toc: p } }), { withShape: false, keys: { shape: 'iconShape', fill: 'iconFill', color: 'iconColor', stroke: 'iconStroke', width: 'iconStrokeWidth' } }));
      icons.push(row('Default icon', iconPick('toc.icon', o.icon, (v) => set('icon', v))));
      // one icon per section of this report (a theme has no sections of its own)
      const names = [...new Set((f.vm.toc || []).filter((e) => e.kind === 'section').map((e) => e.text))];
      if (!isTheme() && names.length) {
        icons.push(h(`<div class="hint">For each section</div>`));
        for (const n of names) icons.push(row(n, iconPick(`toc.icons.${n}`, o.icons[n] || '', (v) => set('icons', { [n]: v }), { none: true, title: `Icon for “${n}”` })));
        if (names.every((n) => o.icons[n])) icons.splice(1, 1, row('Default icon', iconPick('toc.icon', o.icon, (v) => set('icon', v))), h(`<div class="hint">Used for sections without their own icon. Every section has one now; choose Auto / none for a section to use the default.</div>`));
      }
    }
    icons.push(h(`<div class="hint">Tip: click a section's icon on the page to change it.</div>`));
    out.push(section(['Icons', { icon: 'star' }], icons));
  }
  out.push(section(['Page entries', { icon: 'page' }], pages), secs.length && section(['Section labels', { icon: 'tag' }], secs));
  return out;
}

function widgetInspector(f) {
  const w = f.w;
  const user = userMode();
  const pol = policy();
  const locked = user && w.locked;
  const el = h(`<div class="insp">
    <div class="insp-head">
      <span class="ih-ic">${icon(kindIcon(w.kind), 18)}</span>
      <span class="ih-t"><b>${esc(widgetName(w))}</b><span>${kindName(w.kind)}${w.catalog ? ' · from gallery' : ''}</span></span>
      <button class="tb-btn sm" id="w-more">${icon('more', 16)}</button>
    </div>
  </div>`);
  el.querySelector('#w-more').addEventListener('click', (e) => widgetMenu(e.currentTarget, f));
  const src = w.opts?.table?.continues;
  if (src) {
    const n = h(`<div class="note">${icon('table', 15)}<div><span>This is a continuation of a long table. Its rows, look and data follow the original.</span><div class="note-acts"><button class="btn sm primary">Edit the original table</button></div></div></div>`);
    n.querySelector('button').addEventListener('click', () => { S.sel = src; render(); scrollToWidgetSafe(src); });
    el.append(n);
    return el;
  }
  if (isTheme()) return studioInspector(f, el);
  if (S.colEdit?.id === w.id && w.kind === 'table' && !user) {
    const r = columnInspector(f, el);
    if (r) return r;
  }
  if (locked) {
    el.append(h(`<div class="note">${icon('lock', 15)}<span>This part of the template is locked. It updates with the period but can't be changed.</span></div>`));
    return el;
  }
  const canStyle = !user || pol.restyle;
  const canText = !user || pol.edit_text || (w.kind === 'comment' && pol.comment);
  const empty = f.vm.data?.empty;
  if (empty) {
    const n = h(`<div class="note warn">${icon('warn', 15)}<div><span>${esc(empty.message)}</span><div class="note-acts"></div></div></div>`);
    const acts = n.querySelector('.note-acts');
    if (empty.reason === 'period' && empty.latest) {
      const bt = h(`<button class="btn sm primary">Go to the latest data</button>`);
      bt.addEventListener('click', () => edit({ op: 'set_params', params: { ...S.st.doc.params, date: empty.latest } }));
      acts.append(bt);
    }
    if ((empty.reason === 'period' || empty.reason === 'dates') && w.binding && (!user || true)) {
      const bt = h(`<button class="btn sm">Show all data</button>`);
      bt.addEventListener('click', () => upd(w.id, { binding: { window: 'all' } }));
      acts.append(bt);
    }
    el.append(n);
  }

  // ---- content
  const content = [];
  if (!['heading', 'divider', 'image', 'text'].includes(w.kind) && (canStyle || canText)) {
    if (w.kind !== 'comment') {
      content.push(row('Title', textInput('title', w.title, (v) => upd(w.id, { title: v }))));
      content.push(row('Subtitle', textInput('subtitle', w.subtitle, (v) => upd(w.id, { subtitle: v }), { placeholder: 'Optional' })));
      if (w.subtitle) content.push(row('Subtitle place', seg('opts.subSide', drawn(f).opts?.subSide ? 'side' : 'below', [['below', 'Below the title'], ['side', 'Right of it']], (v) => upd(w.id, { opts: { subSide: v === 'side' } }))));
    }
  }
  if (['heading', 'text', 'comment'].includes(w.kind) && canText) {
    // text and notes: their window (the words, icons, a note's background and mark), or the box below
    if (w.kind !== 'heading') {
      const open = h(`<button class="btn sm primary tw-open">${icon('rename', 14)} ${w.kind === 'comment' ? 'Edit the note…' : 'Edit the text…'}</button>`);
      open.addEventListener('click', () => import('./canvas.js').then((m) => m.startInlineEdit(f)));
      content.push(open);
    }
    // text and notes: bold, italic, colours, sizes and icons word by word
    content.push(row(w.kind === 'heading' ? 'Heading' : 'Text', w.kind === 'heading'
      ? textInput('opts.text', w.opts?.text, (v) => upd(w.id, { opts: { text: v } }), { area: true, rows: 2 })
      : richEditor(w.opts?.text ?? '', S.st.doc.theme, (v) => upd(w.id, { opts: { text: v } }), { placeholder: w.kind === 'comment' ? 'Write to your client…' : 'Write here…' }), 'stack'));
    if (w.kind === 'comment' || w.kind === 'text') {
      content.push(row('Arrows', toggle('opts.arrows', !!w.opts?.arrows, (v) => upd(w.id, { opts: { arrows: v || null } }), '▲ +6,48%  ▼ −2,1%')));
    }
    if (w.kind === 'comment') {
      content.push(row('Signed note', toggle('opts.signed', !!w.opts?.signed, (v) => upd(w.id, { opts: { signed: v || null } }), 'From you, with your name')));
    }
    if (w.kind === 'heading') {
      const o = w.opts || {};
      const oset = (k, v) => upd(w.id, { opts: { [k]: v === '' ? null : v } });
      content.push(row('Subtitle', textInput('opts.subtitle', o.subtitle, (v) => oset('subtitle', v), { placeholder: '{{period}}' }), 'stack'));
      content.push(row('Eyebrow', textInput('opts.eyebrow', o.eyebrow, (v) => oset('eyebrow', v), { placeholder: 'Small capitals above' })));
      if (!user) content.push(row('Badge', textInput('opts.badge', o.badge, (v) => oset('badge', v), { placeholder: 'e.g. № 400PX0F' })));
      if (!user && o.badge) {
        const bg = h(`<div class="m-row"></div>`);
        bg.append(selectInput('opts.badgeStyle', o.badgeStyle || 'solid', [['solid', 'Filled'], ['outline', 'Outline'], ['text', 'Text only']], (v) => oset('badgeStyle', v === 'solid' ? '' : v)),
          colorField('opts.badgeFill', o.badgeFill || null, (c) => oset('badgeFill', c || '')));
        content.push(row('Badge style', bg));
      }
      if (!user) content.push(row('Note on the right', textInput('opts.meta', o.meta, (v) => oset('meta', v), { area: true, rows: 2, placeholder: 'e.g. 23 accounts · on {{end}}' }), 'stack'));
      if (!user) content.push(row('Page navigation', toggle('opts.nav', !!o.nav, (v) => oset('nav', v))));
      const ru = h(`<div class="m-row"></div>`);
      ru.append(textInput('opts.rule', o.rule ?? '', (v) => oset('rule', v || null), { type: 'number', min: 0, max: 6, placeholder: 'none' }), colorField('opts.ruleColor', o.ruleColor || null, (c) => oset('ruleColor', c)));
      if (!user) content.push(row('Rule under (pt)', ru));
    }
    if (w.kind === 'heading' || w.kind === 'text') content.push(row('Align', seg('opts.align', w.opts?.align || 'left', [['left', 'Left', 'align-left'], ['center', 'Centre', 'align-center'], ['right', 'Right', 'align-right']], (v) => upd(w.id, { opts: { align: v === 'left' ? null : v } }))));
    if (!user) content.push(h(`<div class="hint">Placeholders: <code>{{period}}</code> <code>{{compare}}</code> <code>{{title}}</code>${S.st.doc.param_defs.map((p) => ` <code>{{${esc(p.name)}}}</code>`).join('')}. Start a line with “- ” for a bullet; <code>*italic*</code>, <code>**bold**</code> and <code>{$accent|colour}</code> style words.</div>`));
    if (w.kind === 'comment') content.push(row('Author', textInput('opts.author', w.opts?.author, (v) => upd(w.id, { opts: { author: v } }))));
  }
  if (w.kind === 'image' && canStyle) {
    const b = h(`<button class="btn">${icon('image', 14)} Choose image…</button>`);
    b.addEventListener('click', async () => {
      const p = await call('pick_data_file', { kind: 'image' });
      if (!p) return;
      const src = await call('read_image', { path: p });
      upd(w.id, { opts: { src } });
    });
    content.push(row('Image', b));
  }

  // ---- data
  const data = [];
  if (DATA_KINDS.includes(w.kind) && !user) data.push(dataSection(w));
  if (w.kind === 'composite' && !user) data.push(...compositeSections(f));
  if (DATA_KINDS.includes(w.kind) && user && w.binding) {
    const b = w.binding;
    // "Time" picks a slice of dated data: a source without a date column (SQL that returns the
    // period itself) shows the same whatever is picked, so it isn't offered
    const dated = (S.st.doc.datasets || []).some((d) => d.id === b.dataset && d.date_field);
    const items = dated ? [row('Time', selectInput('b.window', b.window, WINDOWS, (v) => upd(w.id, { binding: { window: v } })))] : [];
    if (['table', 'bar', 'pie'].includes(w.kind) && b.dims.length === 1) {
      // how many rows (the best or the worst, as the table sorts); an "Other" row stays as designed
      items.push(row('Rows', selectInput('b.limit', b.limit || 0, [[0, 'All'], [3, '3'], [5, '5'], [8, '8'], [10, '10'], [15, '15']], (v) => upd(w.id, { binding: { limit: +v } }))));
    }
    if (items.length) data.push(section(dated ? 'Time & rows' : 'Rows', items));
  }

  if (user) return simpleInspector(f, el, { content, data, canStyle, pol });

  // ---- design
  const d = displayControls(w, user);
  const design = canStyle ? designSections(f, d) : [];

  // ---- layout
  const layout = [];
  // the designer's own name for it: in the editor only, never printed
  if (!user) {
    const def = widgetName(w, { custom: false });
    const own = String(w.opts?.name || '');
    const nm = h(`<div class="m-row"></div>`);
    nm.append(textInput('opts.name', own, (v) => upd(w.id, { opts: { name: String(v || '').trim() || null } }), { placeholder: def }));
    const nameRow = row('Name', nm);
    if (own) resettable(nameRow, nameRow.querySelector('label'), () => upd(w.id, { opts: { name: null } }), `the default name, ${def}`);
    layout.push(section(['Name', { icon: 'rename' }], nameRow,
      h(`<div class="hint">Only in the editor, never printed: the canvas, the inspector, lists such as “Keep with the end of” and the fit messages show it. Leave it empty for the default.</div>`)));
  }
  if (!user || pol.arrange) {
    const spec = S.st.specs[w.kind];
    // several selected: the number typed goes to each of them as it is (one column or height lines them up)
    const setRect = (k, x, min, max) => {
      const ws = selection().map((id) => findWidget(id)?.w).filter(Boolean);
      const one = (o) => ({ op: 'set_rect', id: o.id, rect: { x: o.x, y: o.y, w: o.w, h: o.h, [k]: Math.max(min, Math.min(max, Math.round(x))) } });
      return edit(ws.length > 1 ? { op: 'batch', edits: ws.map(one) } : one(w));
    };
    const n = (k, v, min, max) => textInput('r.' + k, v, (x) => x != null && setRect(k, x, min, max), { type: 'number', min, max });
    const grid = h(`<div class="xywh"></div>`);
    const pg = S.st.page;
    // a part: on its composite's grid
    const [gc, gr] = f.parent ? [f.parent.opts?.grid?.cols || 12, f.parent.opts?.grid?.rows || 6] : [pg.cols, 99];
    [['x', 'Column', 0, gc - 1], ['y', 'Row', 0, gr - 1], ['w', 'Width', f.parent ? 1 : spec.min_w, gc], ['h', 'Height', f.parent ? 1 : spec.min_h, f.parent ? gr : spec.max_h]].forEach(([k, l, mn, mx]) => {
      const c = h(`<label><span>${l}</span></label>`);
      c.append(n(k, w[k], mn, mx));
      grid.append(c);
    });
    const items = [grid, h(`<div class="hint">${f.parent ? `In the composite’s cells: it has ${gc} columns and ${gr} rows (Layout of the composite).` : `In grid cells. The page has ${pg.cols} columns and ${pg.rows} rows.`}</div>`)];
    if (w.kind === 'composite') {
      const gg = h(`<div class="xywh"></div>`);
      [['cols', 'Columns', 12], ['rows', 'Rows', 6]].forEach(([k, l, d]) => {
        const c = h(`<label><span>${l}</span></label>`);
        c.append(textInput('grid.' + k, w.opts?.grid?.[k] ?? d, (x) => x != null && upd(w.id, { opts: { grid: { [k]: Math.max(1, Math.min(48, Math.round(x))) } } }), { type: 'number', min: 1, max: 48 }));
        gg.append(c);
      });
      layout.push(section(['Grid inside', { icon: 'grid' }], gg, h(`<div class="hint">The cells its parts are placed on (drag a part on the canvas, or set its place in its Layout tab).</div>`)));
    }
    if (!user && !f.parent) items.push(row('Locked for users', toggle('locked', w.locked, (v) => upd(w.id, { locked: v }))));
    layout.push(section('Position on the grid', items));
    // kept with the end of a table above it: while that table continues, this goes after its last rows
    if (!user && w.kind !== 'table' && !f.parent) {
      const page = S.st.doc.pages[f.pi];
      const tables = page.widgets.filter((t) => t.kind === 'table' && t.y + t.h <= w.y && t.x < w.x + w.w && t.x + t.w > w.x);
      // a continuation piece is linked through its table
      const srcOf = (t) => t.opts?.table?.continues || t.id;
      // an untitled table goes by its data source; "grid rows" tells two tables apart on the page
      const nameOf = (t) => widgetName(findWidget(srcOf(t))?.w || t);
      // where it sits, only to tell apart tables that would have the same name
      const label = (t) => `${nameOf(t)}${t.opts?.table?.continues ? ' (continued)' : ''}`;
      const where = (t) => {
        const same = tables.filter((o) => label(o) === label(t));
        if (same.length < 2) return '';
        const sameRow = same.every((o) => o.y === t.y);
        if (sameRow) return t.x === Math.min(...same.map((o) => o.x)) ? ' · left' : t.x === Math.max(...same.map((o) => o.x)) ? ' · right' : ' · middle';
        return t.y === Math.min(...same.map((o) => o.y)) ? ' · upper' : t.y === Math.max(...same.map((o) => o.y)) ? ' · lower' : ' · middle';
      };
      const opts2 = tables.map((t) => [srcOf(t), label(t) + where(t)]);
      const cur = w.opts?.follows || '';
      const items2 = tables.length
        ? [row('Keep with the end of', selectInput('follows', cur, [['', 'Nothing — stays where it is'], ...opts2], (v) => upd(w.id, { opts: { follows: v || null } }))),
          h(`<div class="hint">While that table continues onto more pages, this goes right after its last rows, keeping its distance from the table. The table takes its place here.${f.vm?.moved ? ' <b>It does now.</b>' : ''}</div>`)]
        : [h(`<div class="hint">Put it under a table to keep it with the table's end — e.g. a total or a chart that belongs after the table's last rows.</div>`)];
      layout.push(section(['Linked to a table', { icon: 'link' }], items2));
    }
  }

  // a widget on data: its title and subtitle sit at the top of the Data tab (what it shows and what it's called)
  const onData = (DATA_KINDS.includes(w.kind) || w.kind === 'composite') && data.length;
  // a composite's part: which composite it is in, and the way back to it
  if (f.parent) {
    const n = h(`<div class="note part-of">${icon('grid', 15)}<div><span>A part of <b>${esc(widgetName(f.parent))}</b>: its data source and filters are the composite’s unless set here; it moves on the composite’s grid.</span><div class="note-acts"><button class="btn sm">${icon('arrow-up', 13)} The composite</button></div></div></div>`);
    n.querySelector('button').addEventListener('click', () => { S.sel = f.parent.id; render(); });
    el.append(n);
  }
  const titleSec = content.length ? section(['Title', { icon: 'heading' }], content) : null;
  tabbed(el, user ? 'user' : 'widget', [
    // one first tab for every object, "Content": a chart's title and its data; a text's, heading's or
    // note's words; a contents list's entries
    ['data', 'Content', 'paragraph', () => (onData ? [titleSec, ...data]
      : [content.length && (DATA_KINDS.includes(w.kind) ? titleSec : section('Content', content)), ...(w.kind === 'toc' && !user ? tocSource(f) : []), ...data])],
    ['design', 'Design', 'brush', () => design],
    ['text', 'Text', 'type', () => (canStyle ? [textStyle(f, user)] : [])],
    ['layout', 'Layout', 'move', () => layout],
  ]);

  // ---- actions
  const acts = h(`<div class="insp-foot"></div>`);
  if (S.st.doc.catalog.length && (!user || (pol.add && pol.remove)) && !f.parent) {
    const b = h(`<button class="btn sm">${icon('swap', 14)} Replace…</button>`);
    b.addEventListener('click', () => replaceMenu(b, f));
    acts.append(b);
  }
  if (!user && !f.parent) {
    const b = h(`<button class="btn sm">${icon('gallery', 14)} Save to gallery</button>`);
    b.addEventListener('click', () => saveToGallery(w.id));
    acts.append(b);
  }
  if (!user || pol.remove || (w.kind === 'comment' && pol.comment)) {
    const b = h(`<button class="btn sm danger-ghost" title="Delete (⌫)">${icon('trash', 14)} Delete</button>`);
    b.addEventListener('click', () => edit({ op: 'remove_widget', id: w.id }));
    acts.append(h('<span class="grow"></span>'), b);
  }
  if (acts.children.length) el.append(acts);
  return el;
}

/** What a control is set to, as text to compare (null for a group whose own controls are compared). */
function controlValue(n) {
  if (n.matches('.tgl')) return String(!!n.querySelector('input')?.checked);
  if (n.matches('input[type=checkbox]')) return String(n.checked);
  if (n.matches('input, textarea, select')) return n.value;
  if (n.matches('.seg')) return n.querySelector('button.on')?.dataset.v ?? '';
  if (n.matches('.color-field')) { const sw = n.querySelector('.cf-sw'); return `${sw?.className} ${sw?.style.background || ''} ${n.title}`; }
  if (n.querySelector('[data-key]')) return null;
  return n.textContent.trim();
}

/** Every setting's value in a widget's inspector parts, by key. */
function valuesIn(parts) {
  const out = new Map();
  for (const p of parts) {
    for (const n of [...(p.dataset?.key ? [p] : []), ...(p.querySelectorAll?.('[data-key]') || [])]) {
      if (!out.has(n.dataset.key)) { const v = controlValue(n); if (v != null) out.set(n.dataset.key, v); }
    }
  }
  return out;
}

/** A control the selected widgets have set differently: it reads "Mixed" until one value is picked for all. */
function showMixed(n) {
  const tip = 'Mixed: the selected widgets have this set differently. A value set here applies to all of them.';
  if (n.matches('input[type=checkbox], .tgl')) { const c = n.matches('input') ? n : n.querySelector('input'); if (c) c.indeterminate = true; }
  else if (n.matches('input, textarea')) { n.value = ''; n.placeholder = 'Mixed'; }
  else if (n.matches('select')) n.prepend(h(`<option value="" disabled selected hidden>Mixed</option>`)), (n.value = '');
  else if (n.matches('.seg')) n.querySelectorAll('button.on').forEach((b) => b.classList.remove('on'));
  else if (n.matches('.color-field')) { const sw = n.querySelector('.cf-sw'); if (sw) { sw.className = 'cf-sw mixed'; sw.style.background = ''; } }
  else return;
  n.classList.add('mixed');
  n.title = tip;
  // the row's name says it too
  const lab = n.closest('.f-row')?.querySelector(':scope > label');
  if (lab && !lab.querySelector('.mixed-tag')) lab.append(h(`<span class="mixed-tag" title="${tip}">Mixed</span>`));
}

/** Settings that belong to each widget alone, even when several have them: its name and what it is kept with. */
const OWN_KEYS = new Set(['opts.name', 'follows', 'title', 'subtitle', 'opts.subSide']);

/**
 * Several widgets selected (⇧-click): the settings they all have, in the first one's tabs and with its values.
 * A change goes to all of them in one step (`forSelection` in state.js); their data and words stay their own.
 */
function multiInspector(fs) {
  // each widget's inspector, built aside (whole: a search would take its matches out), says which settings
  // it has and what each is set to
  const q = S.inspQuery;
  S.inspQuery = '';
  const vals = [];
  let per;
  try {
    per = fs.map((f) => {
      BUILT = null;
      widgetInspector(f);
      vals.push(valuesIn((BUILT || []).flatMap((t) => t[3])));
      return new Map((BUILT || []).map(([k, , , parts]) => [k, keysIn(parts)]));
    });
  } finally { S.inspQuery = q; }
  const common = new Map();
  for (const [k, keys] of per[0]) {
    if (k === 'data') continue;
    const all = [...keys].filter((x) => !OWN_KEYS.has(x) && per.every((m) => m.get(k)?.has(x)));
    if (all.length) common.set(k, new Set(all));
  }
  COMMON = common;
  let el;
  try { el = widgetInspector(fs[0]); } finally { COMMON = null; }
  // the first one's head and actions become the selection's
  const counts = {};
  for (const f of fs) counts[kindName(f.w.kind)] = (counts[kindName(f.w.kind)] || 0) + 1;
  el.querySelector('.insp-head')?.replaceWith(h(`<div class="insp-head"><span class="ih-ic">${icon('copy', 18)}</span><span class="ih-t"><b>${fs.length} widgets selected</b><span>${esc(Object.entries(counts).map(([k, n]) => (n > 1 ? `${k} ×${n}` : k)).join(' · '))}</span></span></div>`));
  el.querySelector('.insp-foot')?.remove();
  // a setting the widgets have set differently reads "Mixed"
  const mixed = new Set([...common.values()].flatMap((ks) => [...ks]).filter((k) => vals.some((v) => v.get(k) !== vals[0].get(k))));
  el.querySelectorAll('[data-key]').forEach((n) => { if (mixed.has(n.dataset.key)) showMixed(n); });
  const note = h(`<div class="note">${icon('info', 15)}<span>${common.size ? `The settings they all have. A change applies to all of them${mixed.size ? '; <b>Mixed</b>: set differently' : ''}.` : 'These widgets have no settings in common. They can still be moved, resized, duplicated or deleted together.'} ⇧-click a widget to add or remove it.</span></div>`);
  el.querySelector('.insp-head').after(note);
  const acts = h(`<div class="insp-foot"></div>`);
  const dup = h(`<button class="btn sm" title="Duplicate (⌘D)">${icon('duplicate', 14)} Duplicate</button>`);
  dup.addEventListener('click', () => edit({ op: 'duplicate_widget', id: fs[0].w.id }));
  const del = h(`<button class="btn sm danger-ghost" title="Delete (⌫)">${icon('trash', 14)} Delete ${fs.length}</button>`);
  del.addEventListener('click', () => edit({ op: 'remove_widget', id: fs[0].w.id }));
  acts.append(dup, h('<span class="grow"></span>'), del);
  el.append(acts);
  return el;
}

/**
 * A report user's inspector: no tabs, only what a report needs — the look
 * (the theme's prepared styles and the colours), the texts, the period and
 * rows, and replacing or deleting the widget.
 */
function simpleInspector(f, el, { content, data, canStyle, pol }) {
  const w = f.w;
  const views = S.st.doc.theme.styles?.[w.kind] || [];
  const body = h(`<div class="insp-pane simple"></div>`);
  const types = canStyle && CHART_TYPES.some((t) => t[1] === w.kind) && !!w.binding;
  const secs = [
    types && section(['Chart type', { icon: 'chart-column' }], typeGrid(f)),
    canStyle && views.length && section(['Look', { icon: 'brush' }], lookList(f, views)),
    ...(canStyle ? basicLook(f) : []),
    canStyle && (colourSection(f) || userColours(f)),
    content.length && section(['Text', { icon: 'type' }], content),
    ...data,
  ];
  for (const x of secs) if (x) body.append(x);
  if (!body.children.length) body.append(h(`<div class="hint pad">Nothing to change here: the report's template keeps this part as it is.</div>`));
  el.append(body);
  const acts = h(`<div class="insp-foot"></div>`);
  const gallery = !!S.st.doc.catalog.length && pol.add && pol.remove;
  if (gallery) {
    const b = h(`<button class="btn sm" title="Put another widget of this report here">${icon('swap', 14)} Replace…</button>`);
    b.addEventListener('click', () => replaceMenu(b, f, { types: false, gallery }));
    acts.append(b);
  }
  if (pol.remove || (w.kind === 'comment' && pol.comment)) {
    const b = h(`<button class="btn sm danger-ghost" title="Delete (⌫)">${icon('trash', 14)} Delete</button>`);
    b.addEventListener('click', () => edit({ op: 'remove_widget', id: w.id }));
    acts.append(h('<span class="grow"></span>'), b);
  }
  if (acts.children.length) el.append(acts);
  return el;
}

/** Whether the widget's data has numbers big enough to shorten (1 000 and more). */
function hasBig(data) {
  let big = false;
  const walk = (x) => { if (big) return; if (typeof x === 'number') big = Math.abs(x) >= 1000; else if (x && typeof x === 'object') for (const v of Object.values(x)) walk(v); };
  walk(data);
  return big;
}

/** The title as designed, bold (the theme's title), dimmed (small and grey) or off — its text is kept. */
function titleStyleRow(f) {
  const w = f.w;
  const st = drawn(f).style || {};
  const tf = f.vm?.fonts?.title;
  // "Default" is the title as the template's look draws it
  const cur = w.style?.hideTitle ? 'hide' : w.style?.titleStyle || '';
  return mark(row('Title', seg('u.tstyle', cur, [['', 'Default'], ['strong', 'Bold'], ['dim', 'Dim'], ['hide', 'Off']], (v) => upd(w.id, { style: { hideTitle: v === 'hide' ? true : null, titleStyle: v === 'hide' || !v ? null : v } }))), !!cur, () => upd(w.id, { style: { hideTitle: null, titleStyle: null } }));
}

const NUMBER_KINDS = ['kpi', 'bar', 'line', 'area', 'pie', 'treemap', 'tiles', 'waterfall'];
const LABEL_KINDS = ['bar', 'line', 'area', 'waterfall'];
const LEGEND_KINDS = ['bar', 'line', 'area', 'pie'];

/**
 * A report user's basic look controls: fit (title, spacing, text size), numbers
 * (full or short, decimals) and the chart's or table's parts. Values are as
 * drawn (the design view under the widget's own settings).
 */
function basicLook(f) {
  const w = f.w;
  const d = drawn(f);
  const st = d.style || {};
  const sset = (k, v) => upd(w.id, { style: { [k]: v } });
  const out = [];

  // ---- fit
  const fit = [];
  if (w.title && !['heading', 'divider'].includes(w.kind)) fit.push(titleStyleRow(f));
  if (w.kind !== 'divider') {
    const framed = !!(f.vm?.card || f.vm?.title);
    const padNow = st.pad ?? (framed ? 10 : 0);
    // a card keeps at least 6 pt inside it: no "None" there
    const steps = f.vm?.card ? [['6', 'S'], ['10', 'M'], ['16', 'L']] : [['0', 'None'], ['4', 'S'], ['10', 'M'], ['16', 'L']];
    const shownPad = f.vm?.card ? Math.max(6, padNow) : padNow;
    fit.push(mark(row('Spacing', seg('u.pad', steps.some(([v]) => +v === +shownPad) ? String(shownPad) : '', steps, (v) => sset('pad', +v))), w.style?.pad != null, () => sset('pad', null)));
    const k = st.textScale ?? 1;
    fit.push(mark(row('Text size', seg('u.ts', k < 0.95 ? 's' : k > 1.05 ? 'l' : 'm', [['s', 'Small'], ['m', 'Normal'], ['l', 'Large']], (v) => sset('textScale', v === 's' ? 0.85 : v === 'l' ? 1.15 : null))), w.style?.textScale != null, () => sset('textScale', null)));
  }
  if (!['heading', 'divider', 'text', 'image'].includes(w.kind)) {
    const on = f.vm?.card ?? !!st.card;
    fit.push(mark(row('Card', toggle('u.card', on, (v) => sset('card', v), 'Frame')), w.style?.card != null, () => sset('card', null)));
  }
  if (fit.length) out.push(section(['Fit', { icon: 'autofit' }], fit));

  // ---- numbers
  const vz = (k, v) => upd(w.id, { opts: { viz: { [k]: v } } });
  const nums = [];
  if (NUMBER_KINDS.includes(w.kind) && w.binding) {
    const own = w.opts?.viz || {};
    // a donut shows amounts only as its total or in the legend
    const amounts = w.kind !== 'pie' || (() => { const e = resolveViz(S.st, d); return (e.center === 'total' && e.inner > 0.2) || (e.legendStyle !== 'none' && e.legendAmount); })();
    if (amounts && hasBig(f.vm?.data)) nums.push(mark(row('Numbers', seg('u.compact', own.compact === 'on' ? 'on' : own.compact === 'off' ? 'off' : '', [['', 'As designed'], ['off', '1 234 567'], ['on', '1,2 M']], (v) => vz('compact', v || null))), own.compact != null, () => vz('compact', null)));
    nums.push(mark(row('Decimals', selectInput('u.dec', own.decimals ?? '', [['', 'As designed'], ['0', '0'], ['1', '1'], ['2', '2']], (v) => vz('decimals', v === '' ? null : +v))), own.decimals != null, () => vz('decimals', null)));
  }
  if (w.kind === 'table' && w.binding) {
    // every number column at once
    const keys = (w.binding.measures || []).filter((m) => m.agg !== 'text' && m.format !== 'percent').map((m) => m.id);
    const cols = d.opts?.table?.cols || {};
    if (keys.length) {
      const setCols = (patch) => upd(w.id, { opts: { table: { cols: Object.fromEntries(keys.map((k) => [k, patch])) } } });
      const ownCols = w.opts?.table?.cols || {};
      if (hasBig(f.vm?.data)) nums.push(mark(row('Short numbers', toggle('u.tcompact', keys.every((k) => cols[k]?.compact), (v) => setCols({ compact: v || null }), '1,2 M')), keys.some((k) => ownCols[k]?.compact != null), () => setCols({ compact: null })));
      const decs = [...new Set(keys.map((k) => cols[k]?.decimals ?? ''))];
      nums.push(mark(row('Decimals', selectInput('u.tdec', decs.length === 1 ? String(decs[0]) : '', [['', decs.length > 1 ? 'Mixed' : 'As designed'], ['0', '0'], ['1', '1'], ['2', '2']], (v) => setCols({ decimals: v === '' ? null : +v }))), keys.some((k) => ownCols[k]?.decimals != null), () => setCols({ decimals: null })));
    }
  }
  if (nums.length) out.push(section(['Numbers', { icon: 'numbers', closed: true }], nums));

  // ---- the chart's or the table's parts
  const parts = [];
  const eff = SCHEMA[w.kind] ? resolveViz(S.st, d) : {};
  // a 100% bar writes its values inside the parts; a bar list always shows them; a range has points
  const orient = w.kind === 'bar' ? CHART_TYPES.find((t) => t[0] === chartTypeOf(w))?.[2] : null;
  const ov = w.opts?.viz || {};
  if (orient === 'share') parts.push(mark(row('Values on the chart', toggle('u.labels', !!eff.insideLabels, (v) => vz('insideLabels', v))), ov.insideLabels != null, () => vz('insideLabels', null)));
  else if (LABEL_KINDS.includes(w.kind) && !isList(f) && orient !== 'scale') parts.push(mark(row('Values on the chart', toggle('u.labels', eff.labels !== 'none', (v) => vz('labels', v ? 'auto' : 'none'))), ov.labels != null, () => vz('labels', null)));
  if (LEGEND_KINDS.includes(w.kind)) {
    const key = w.kind === 'pie' ? 'legendStyle' : 'legend';
    parts.push(mark(row('Legend', toggle('u.legend', eff[key] !== 'none', (v) => vz(key, v ? (w.kind === 'pie' ? 'simple' : 'auto') : 'none'))), ov[key] != null, () => vz(key, null)));
  }
  if (w.kind === 'kpi') {
    const signed = eff.valueColor === 'sign';
    // one number only: values on a line or category names keep their colour
    const single = !eff.valueJoin && !['name', 'shares'].includes(eff.kpiGroup) && typeof f.vm?.data?.kpi?.value === 'number';
    if (single) parts.push(mark(row('Colour by result', toggle('u.sign', signed, (v) => upd(w.id, { opts: { viz: { valueColor: v ? 'sign' : 'font', signed: v } } }), 'Green or red')), ov.valueColor != null, () => upd(w.id, { opts: { viz: { valueColor: null, signed: null } } })));
    if (f.vm?.card ?? st.card) parts.push(mark(row('Card colour', seg('u.tone', ['sign', 'solid'].includes(eff.tone) ? eff.tone : 'none', [['none', 'Plain'], ['sign', 'Tinted'], ['solid', 'Solid']], (v) => upd(w.id, { opts: { viz: { tone: v, arrow: v !== 'none' } } }))), ov.tone != null, () => upd(w.id, { opts: { viz: { tone: null, arrow: null } } })));
    parts.push(mark(row('Align', seg('u.align', eff.kpiAlign === 'right' ? 'right' : eff.kpiAlign === 'center' ? 'center' : 'left', [['left', 'Left', 'align-left'], ['center', 'Centre', 'align-center'], ['right', 'Right', 'align-right']], (v) => upd(w.id, { opts: { align: v === 'left' ? null : v, viz: { kpiAlign: v } } }))), ov.kpiAlign != null || w.opts?.align != null, () => upd(w.id, { opts: { align: null, viz: { kpiAlign: null } } })));
  }
  if (w.kind === 'pie' && eff.inner > 0.2) {
    parts.push(mark(row('Centre shows', selectInput('u.center', eff.center || 'total', [['total', 'The total'], ['top', 'The largest share'], ['none', 'Nothing']], (v) => vz('center', v))), ov.center != null, () => vz('center', null)));
  }
  if (w.kind === 'table') {
    parts.push(mark(row('Totals row', toggle('u.totals', d.opts?.totals !== false, (v) => upd(w.id, { opts: { totals: v } }))), w.opts?.totals != null, () => upd(w.id, { opts: { totals: null } })));
    // the facts grid sizes its own cells, so stretching rows would do nothing there
    if (d.opts?.table?.layout !== 'facts') parts.push(mark(row('Fill height', toggle('u.fill', !!d.opts?.table?.fill, (v) => upd(w.id, { opts: { table: { fill: v } } }), 'Stretch rows')), w.opts?.table?.fill != null, () => upd(w.id, { opts: { table: { fill: null } } })));
    parts.push(mark(row('Header row', toggle('u.head', !d.opts?.table?.hideHead, (v) => upd(w.id, { opts: { table: { hideHead: v ? null : true } } }))), w.opts?.table?.hideHead != null, () => upd(w.id, { opts: { table: { hideHead: null } } })));
    // any column can be hidden (at least one stays)
    const all = displayColumns(d, f.vm?.data);
    // columns that only feed another (a tag beside a name, a colouring rule) stay as they are
    const cols = all.filter((c) => !all.some((o) => o.s.tag === c.key) && !(c.s.hidden && (c.s.rules || []).length));
    const shown = cols.filter((c) => !c.s.hidden);
    if (cols.length > 1) {
      const list = h(`<div class="col-vis"></div>`);
      for (const c of cols) {
        const label = c.s.label || c.label || c.key;
        const last = !c.s.hidden && shown.length === 1;
        const t = toggle(`u.col.${c.key}`, !c.s.hidden, (v) => upd(w.id, { opts: { table: { cols: { [c.key]: { hidden: v ? null : true } } } } }), String(label).trim() || '(no name)');
        if (last) t.querySelector('input').disabled = true;
        list.append(t);
      }
      parts.push(row('Columns', list, 'stack'));
    }
  }
  if (parts.length) out.push(section([w.kind === 'table' ? 'Table' : 'Chart', { icon: w.kind === 'table' ? 'table' : 'chart-bar', closed: true }], parts));
  if (w.kind === 'table') { const x = userRanking(f, d.opts?.table || {}); if (x) out.push(x); }
  return out;
}

/**
 * Best & worst for a report user: which value ranks the rows, how many best and
 * worst to mark and how; several rankings may sit side by side (a return and a
 * result). The icons, colours and rules stay as the designer set them.
 */
function userRanking(f, ts) {
  const w = f.w;
  const nums = (f.vm.data?.columns || []).filter((c) => c.role === 'measure' && !ts.cols?.[c.key]?.bar?.only);
  if (!nums.length) return null;
  const list = [].concat(ts.extremes || []).filter(Boolean).map((x) => ({ ...x }));
  const save = (next) => upd(w.id, { opts: { table: { extremes: next.length ? next : null } } });
  const label = (c) => (ts.cols?.[c.key]?.label || c.label || c.key).replace(/\s+/g, ' ').trim();
  const pct = (key) => nums.find((c) => c.key === key)?.format === 'percent';
  // the icon a ranking suggests: % for a return, coins for an amount, arrows for a price change —
  // one no other ranking of the table uses
  const colOf = (key) => nums.find((c) => c.key === key) || {};
  const kindOf = (key) => { const c = colOf(key); return /изм|chg|change/i.test(`${c.key} ${label(c)} ${c.field || ''}`) ? 'chg' : pct(key) ? 'ret' : 'amt'; };
  const SUGGEST = { ret: [['percent', 'percent'], ['star', 'star'], ['award', 'award']], amt: [['coins', 'coins'], ['medal', 'medal'], ['crown', 'crown']], chg: [['trend-up', 'trend-down'], ['arrow-up', 'arrow-down'], ['thumbs-up', 'thumbs-down']] };
  const suggest = (key, except = -1) => {
    const used = new Set(list.filter((_, k) => k !== except).flatMap((x) => [x.best?.icon, x.worst?.icon]));
    const opts = SUGGEST[kindOf(key)];
    return opts.find(([b, w]) => !used.has(b) && !used.has(w)) || opts[0];
  };
  // a return marks gains and losses; an amount (all positive) its highest and lowest
  const fresh = (key) => { const [bi, wi] = suggest(key); return { on: true, by: key, show: 'both', ...(pct(key) ? {} : { rule: 'rank' }), best: { n: 1, icon: bi }, worst: { n: 1, icon: wi } }; };
  const items = [];
  list.forEach((X, xi) => {
    const set = (patch) => { const n = list.map((x) => ({ ...x })); n[xi] = { ...n[xi], ...patch }; save(n); };
    const card = h(`<div class="rank-card${X.on ? '' : ' off'}"><div class="rk-head"></div><div class="rk-body"></div></div>`);
    const head = card.querySelector('.rk-head');
    head.append(selectInput(`u.rk${xi}.by`, X.by, nums.map((c) => [c.key, label(c)]), (v) => {
      // icons the user didn't pick follow the new value
      const [ob, ow] = suggest(X.by, xi), [nb, nw] = suggest(v, xi);
      const keep = (cur, old, nu) => (!cur || cur === old || Object.values(SUGGEST).flat().flat().includes(cur) ? nu : cur);
      set({ by: v, ...(pct(v) ? { rule: null } : { rule: 'rank' }), best: { ...X.best, icon: keep(X.best?.icon, ob, nb) }, worst: { ...X.worst, icon: keep(X.worst?.icon, ow, nw) } });
    }));
    head.append(toggle(`u.rk${xi}.on`, !!X.on, (v) => set({ on: v })));
    const del = h(`<button class="tb-btn sm" title="Remove this ranking">${icon('trash', 13)}</button>`);
    del.addEventListener('click', () => save(list.filter((_, k) => k !== xi)));
    head.append(del);
    const body = card.querySelector('.rk-body');
    const n = [['0', '0'], ['1', '1'], ['2', '2'], ['3', '3']];
    // how many, and the icon each side shows (when marked with an icon)
    const [sb, sw] = suggest(X.by, xi);
    for (const [kind, lab, def] of [['best', 'Best', sb], ['worst', 'Worst', sw]]) {
      const e = X[kind] || {};
      const r = h(`<div class="m-row"></div>`);
      r.append(seg(`u.rk${xi}.${kind[0]}`, String(e.n ?? 1), n, (v) => set({ [kind]: { ...e, n: +v } })));
      if ((X.show || 'icon') !== 'row') r.append(iconPick(`u.rk${xi}.${kind}i`, e.icon || def, (v) => set({ [kind]: { ...e, icon: v || def } }), { title: `${lab} icon` }));
      body.append(row(lab, r));
    }
    body.append(row('Mark with', seg(`u.rk${xi}.show`, X.show || 'icon', [['icon', 'Icon'], ['row', 'Row'], ['both', 'Both']], (v) => set({ show: v }))));
    items.push(card);
  });
  const used = new Set(list.map((x) => x.by));
  const next = nums.find((c) => !used.has(c.key) && pct(c.key)) || nums.find((c) => !used.has(c.key));
  if (next) {
    const add = h(`<button class="btn sm">${icon('plus', 13)} ${list.length ? 'Add another ranking' : 'Mark the best & worst rows'}</button>`);
    add.addEventListener('click', () => save([...list, fresh(next.key)]));
    items.push(add);
  }
  items.push(h(`<div class="hint">Rows ranked by the value you choose, within the period shown. For a return, best only if it gained and worst only if it lost; for an amount, the highest and the lowest.</div>`));
  const on = list.filter((x) => x.on).length;
  return section(['Highlights', { icon: 'star', badge: on || null }], items);
}

/** Every chart type as a picture to click; types this data can't draw are greyed out. */
function typeGrid(f) {
  const w = f.w;
  const cur = chartTypeOf(w);
  const el = h(`<div class="ct-grid"></div>`);
  for (const t of CHART_TYPES) {
    const [id, , , label, ic] = t;
    const why = typeOff(w, t);
    const b = h(`<button class="ct${id === cur ? ' on' : ''}" title="${esc(why || label)}"${why ? ' disabled' : ''}>${icon(ic, 20, 1.5)}<span>${esc(label)}</span></button>`);
    b.addEventListener('click', () => { if (id !== cur) setChartType(w, id); });
    el.append(b);
  }
  return el;
}

/** The theme's prepared looks for this widget, one click each. */
function lookList(f, views) {
  const w = f.w;
  // a bar's looks belong to one bar type (columns, bars, 100%, range)
  const orientOf = (v) => v.opts?.orientation || v.opts?.viz?.orientation || 'vertical';
  const or = w.kind === 'bar' ? CHART_TYPES.find((t) => t[0] === chartTypeOf(w))?.[2] : null;
  const list = or ? views.filter((v) => orientOf(v) === or) : views;
  const box = h(`<div class="lk-list"></div>`);
  // a long list starts short: the first few and the one in use
  const all = LOOKS_OPEN.has(w.id) || list.length <= 6;
  const shown = all ? list : list.filter((v, i) => i < 5 || v.id === w.variant);
  for (const v of shown) {
    const b = h(`<button class="lk${v.id === w.variant ? ' on' : ''}"><i class="lk-dot"></i><span><b></b><span></span></span></button>`);
    b.querySelector('b').textContent = v.name;
    b.querySelector('span span').textContent = v.description || '';
    b.addEventListener('click', () => v.id !== w.variant && upd(w.id, { variant: v.id }));
    box.append(b);
  }
  if (shown.length < list.length) {
    const more = h(`<button class="btn sm ghost">${icon('chevd', 13)} ${list.length - shown.length} more looks</button>`);
    more.addEventListener('click', () => { LOOKS_OPEN.add(w.id); renderRight(); });
    box.append(more);
  }
  return box;
}
const LOOKS_OPEN = new Set();

/** A report user's theme choice: the themes that have every look this report uses. */
function themePicker() {
  const list = h(`<div class="th-list"><div class="hint">Loading themes…</div></div>`);
  api.themes().then((all) => {
    const ok = all.filter((t) => t.fits || t.current);
    list.innerHTML = '';
    for (const t of ok) {
      const b = h(`<button class="th-pick${t.current ? ' on' : ''}"><span class="tcd-sw">${(t.colors || []).map((c) => `<i style="background:${esc(c)}"></i>`).join('')}</span><b></b>${t.current ? '<span class="th-cur">In use</span>' : ''}</button>`);
      b.querySelector('b').textContent = t.name;
      b.addEventListener('click', () => !t.current && api.applyTheme(t.id));
      list.append(b);
    }
    if (ok.length < 2) list.append(h(`<div class="hint">No other theme has this report's looks.</div>`));
  });
  return section(['Theme', { icon: 'palette' }], list);
}

/** Chart types a chart can switch between: [id, kind, orientation, label, icon]. */
/** A bar chart in its List layout (drawn by the table engine). */
const isList = (f) => {
  if (!f || f.w.kind !== 'bar') return false;
  const d = drawn(f);
  return (d.opts?.orientation || d.opts?.viz?.orientation) === 'horizontal' && resolveViz(S.st, d).rankStyle === 'ranked';
};

const CHART_TYPES = [
  ['kpi', 'kpi', null, 'Number', 'kpi'],
  ['table', 'table', null, 'Table', 'table'],
  ['columns', 'bar', 'vertical', 'Columns', 'chart-column'],
  ['bars', 'bar', 'horizontal', 'Bars', 'chart-hbar'],
  ['share', 'bar', 'share', '100% bar', 'chart-share'],
  ['scale', 'bar', 'scale', 'Range', 'scale'],
  ['line', 'line', null, 'Line', 'chart-line'],
  ['area', 'area', null, 'Area', 'chart-area'],
  ['pie', 'pie', null, 'Donut', 'chart-pie'],
  ['treemap', 'treemap', null, 'Treemap', 'treemap'],
  ['tiles', 'tiles', null, 'Tiles', 'tiles'],
  ['waterfall', 'waterfall', null, 'Waterfall', 'waterfall'],
];

/**
 * The chart type as a gallery of icons. Switching keeps the data and takes the
 * new type's default design view. In the studio a view keeps its type, so only
 * a bar view's orientation can change.
 */
function chartTypes(w, user) {
  const cur = chartTypeOf(w);
  const list = isTheme() || user ? CHART_TYPES.filter((t) => t[1] === w.kind) : CHART_TYPES;
  if (list.length < 2) return null;
  const el = h(`<div class="ct-grid"></div>`);
  for (const t of list) {
    const [id, , , label, ic] = t;
    const why = isTheme() ? '' : typeOff(w, t);
    const b = h(`<button class="ct${id === cur ? ' on' : ''}" title="${esc(why ? `${why}: add one in Data` : label)}"${why ? ' disabled' : ''}>${icon(ic, 20, 1.5)}<span>${esc(label)}</span></button>`);
    b.addEventListener('click', () => { if (id !== cur) setChartType(w, id); });
    el.append(b);
  }
  return el;
}

/** Whether a chart type can draw this widget's data, and if not why. */
function typeOff(w, [id, , , label]) {
  const dims = w.binding?.dims?.length ?? 0;
  // donuts, treemaps, tiles and waterfalls draw one series over one set of categories
  const needsOne = ['pie', 'treemap', 'tiles', 'waterfall', 'share', 'scale'].includes(id);
  // charts draw categories: a single number (no "Group by") has none to draw
  const needsSome = !['kpi', 'table'].includes(id);
  if (needsOne && dims !== 1) return `${label} needs exactly one “Group by” field`;
  if (needsSome && dims === 0) return `${label} needs a “Group by” field`;
  return '';
}

/** The chart type a widget is drawn as now (an id of CHART_TYPES). */
function chartTypeOf(w) {
  const f0 = findWidget(w.id);
  const o = (f0 ? drawn(f0) : w).opts || {};
  const orient = o.orientation || o.viz?.orientation || 'vertical';
  return w.kind === 'bar' ? CHART_TYPES.find((t) => t[2] === orient)?.[0] || 'columns' : w.kind;
}

/** Switches a widget to another chart type, keeping its data. */
function setChartType(w, id) {
  const [, kind, or, label] = CHART_TYPES.find((t) => t[0] === id);
  // a bar list and a table share one engine: switching keeps the look
  const f1 = findWidget(w.id);
  if (id === 'table' && isList(f1)) {
    const p = listToTable(S.st, f1.w, resolveViz(S.st, drawn(f1)), f1.vm, drawn(f1).style);
    if (p) { upd(w.id, p); toast('Now a table with the same look: add columns in Content. Undo with ⌘Z.', 'info'); return; }
  }
  if (id === 'bars' && w.kind === 'table') {
    const p = tableToList(drawn(f1), f1.vm?.data);
    if (p) { upd(w.id, p); toast('Now a bar chart (List layout) with the same look. Undo with ⌘Z.', 'info'); return; }
  }
  if (kind === w.kind) {
    // a bar's design view belongs to one bar type: take the first view of the new type
    const orientOf = (v) => v.opts?.orientation || v.opts?.viz?.orientation || 'vertical';
    const views = S.st.doc.theme.styles?.bar || [];
    const keep = views.find((v) => v.id === w.variant && orientOf(v) === or);
    const view = keep || views.find((v) => orientOf(v) === or);
    return upd(w.id, { variant: view?.id || '', opts: { orientation: or } });
  }
  // a bar's design view belongs to one bar type (columns, bars, 100%, range)
  const orientOf = (v) => v.opts?.orientation || v.opts?.viz?.orientation || 'vertical';
  const list = S.st.doc.theme.styles?.[kind] || [];
  const view = kind === 'bar' ? list.find((v) => orientOf(v) === or) : list[0];
  upd(w.id, { kind, variant: view?.id || '', opts: { orientation: or, viz: null } });
  toast(`Now shown as ${label.toLowerCase()}${view ? `, “${view.name}” look` : ''}. Undo with ⌘Z.`, 'info');
}

/** The design view as a button that opens the theme's views for this kind. */
function viewPicker(w, views, user) {
  // a widget of a widget type uses the type's design view unless it picks its own
  const typeView = w.type && S.st.doc.theme.widget_types?.[w.type]?.variant;
  const cur = views.find((v) => v.id === (w.variant || typeView));
  const b = h(`<button class="vp-btn"><span class="vp-sw">${icon(cur ? 'palette' : 'brush', 14)}</span><span class="vp-t"><b>${esc(cur ? cur.name : 'Own look')}</b>${cur?.description ? `<span>${esc(cur.description)}</span>` : ''}</span>${icon('chevd', 13)}</button>`);
  b.addEventListener('click', () => menu(b, [
    { title: `${kindName(w.kind)} views in “${S.st.doc.theme.name || 'this theme'}”` },
    ...views.map((v, i) => ({ label: v.name, desc: [i === 0 ? 'Default' : '', v.description].filter(Boolean).join(' · '), checked: v.id === w.variant, run: () => upd(w.id, { variant: v.id }) })),
    { sep: true },
    { label: 'Own look', desc: 'Keep this widget’s settings without a design view', checked: !cur, run: () => upd(w.id, { variant: '' }) },
    !user && { label: 'Edit views in the theme studio', icon: 'palette', run: () => api.themeStudio('current') },
  ]));
  return b;
}

/** Look controls: the design view, the chart type and the card. */
function displayControls(w, user) {
  const o = w.opts || {};
  const out = { view: [], type: [], card: [] };
  const opt = (k, v) => upd(w.id, { opts: { [k]: v } });
  const views = S.st.doc.theme.styles?.[w.kind] || [];
  if (!isTheme() && views.length && (!user || policy().restyle)) {
    const dv = h(`<div class="m-row"></div>`);
    dv.append(viewPicker(w, views, user));
    if (!user) {
      const save = h(`<button class="btn sm icon" title="Save this widget’s look as a new design view in the theme">${icon('plus', 14)}</button>`);
      save.addEventListener('click', async () => {
        const name = await promptText('Save as a design view', 'Name', `${kindName(w.kind)} ${views.length + 1}`);
        if (name) { await edit({ op: 'add_variant', kind: w.kind, name, from: null, widget: w.id }); toast(`“${name}” added to the theme. This widget now uses it.`); }
      });
      dv.append(save);
    }
    out.view.push(dv);
    const cur = views.find((v) => v.id === (w.variant || (w.type && S.st.doc.theme.widget_types?.[w.type]?.variant)));
    out.view.push(h(`<div class="hint">${cur ? `${cur.description ? esc(cur.description) + '. ' : ''}From the theme “${esc(S.st.doc.theme.name || 'Untitled')}”. Settings you change below apply to this widget only.` : 'This widget has its own look. Pick a design view to follow the theme.'}</div>`));
  }
  if (CHART_TYPES.some((t) => t[1] === w.kind)) out.type.push(chartTypes(w, user));
  // values as drawn: the design view's style under the widget's own
  const st = drawn(findWidget(w.id) || { w }).style || {};
  const sset = (k, v) => upd(w.id, { style: { [k]: v } });
  // a vertical divider is drawn in the gap before its column (taking no room) or in the middle of its own column
  if (w.kind === 'divider') out.card.push(row('Vertical', toggle('o.vertical', !!o.vertical, (v) => opt('vertical', v || null))));
  if (w.kind === 'divider' && o.vertical) out.card.push(row('Place', seg('o.gap', o.gap ? 'gap' : 'column', [['gap', 'In the gap'], ['column', 'In its column']], (v) => opt('gap', v === 'gap' || null))),
    h(`<div class="hint">${o.gap ? 'Drawn in the gap at the left edge of its column, between the widgets either side: it takes no room, so they can meet it.' : 'Drawn in the middle of a column of its own.'}</div>`));
  if (['kpi', 'bar', 'line', 'area', 'pie', 'table', 'text', 'treemap', 'waterfall', 'tiles', 'scale', 'toc', 'heading'].includes(w.kind) && !user) {
    const os = w.style || {};
    out.card.push(mark(row('Card', toggle('s.card', st.card ?? (!['text', 'heading'].includes(w.kind) && S.st.doc.theme.card !== 'plain'), (v) => sset('card', v))), os.card != null, () => sset('card', null), 'the design view'));
    const bd = h(`<div class="m-row"></div>`);
    bd.append(colorField('s.border', st.border || null, (c) => sset('border', c), { title: 'Outline colour' }), textInput('s.borderWidth', st.borderWidth ?? '', (v) => sset('borderWidth', v), { type: 'number', min: 0, max: 6, placeholder: 'width' }));
    out.card.push(mark(row('Outline', bd), os.border != null || os.borderWidth != null, () => upd(w.id, { style: { border: null, borderWidth: null } }), 'the design view'));
    out.card.push(mark(row('Fill', colorField('s.bg', st.background || null, (c) => sset('background', c), { title: 'Card fill' })), os.background != null, () => sset('background', null), 'the design view'));
    // this card's corners; empty follows the theme (Theme → Cards & charts → Corner radius)
    out.card.push(mark(row('Corners (pt)', textInput('s.radius', st.radius ?? '', (v) => sset('radius', v === '' || v == null ? null : Math.max(0, Math.min(40, +v))),
      { type: 'number', min: 0, max: 40, placeholder: `theme · ${S.st.doc.theme.radius ?? 6}` })), os.radius != null, () => sset('radius', null), 'the theme'));
    // inner padding: the space between the widget's edge and its content — S unless set, None for any widget
    const f0 = findWidget(w.id);
    const roomy = !!f0?.vm?.card && (S.st.doc.theme.card_pad || 0) > 10;
    const padNow = st.pad ?? (roomy ? S.st.doc.theme.card_pad : 6);
    const steps = [['0', 'None', 'border-none'], ['6', 'S'], ['10', 'M'], ['16', 'L']];
    const pr = h(`<div class="m-row"></div>`);
    pr.append(seg('s.padp', steps.some(([v]) => +v === +padNow) ? String(padNow) : '', steps, (v) => sset('pad', +v)),
      textInput('s.pad', st.pad ?? '', (v) => sset('pad', v == null ? null : Math.max(0, Math.min(30, v))), { type: 'number', min: 0, max: 30, placeholder: String(padNow) }));
    out.card.push(mark(row('Padding', pr), os.pad != null, () => sset('pad', null), 'the default (S)'));
    const sides = [].concat(st.rule || []);
    const sd = h(`<div class="seg multi"></div>`);
    for (const [k, l] of [['left', 'Left'], ['right', 'Right'], ['top', 'Top'], ['bottom', 'Under']]) {
      const b = h(`<button class="${sides.includes(k) ? 'on' : ''}">${l}</button>`);
      b.addEventListener('click', () => { const n = sides.includes(k) ? sides.filter((x) => x !== k) : [...sides, k]; sset('rule', n.length ? n : null); });
      sd.append(b);
    }
    out.card.push(mark(row('Divider lines', sd), os.rule != null, () => sset('rule', null), 'the design view'));
    if (w.title) out.card.push(titleStyleRow(findWidget(w.id) || { w, vm: null }));
    // the title's alignment: left unless set (a view aligned right takes its title along)
    if (w.title) {
      const ta = st.titleAlign || (o.align === 'right' ? 'right' : 'left');
      out.card.push(mark(row('Title align', seg('s.talign', ta, [['left', 'Left', 'align-left'], ['center', 'Centre', 'align-center'], ['right', 'Right', 'align-right']], (v) => sset('titleAlign', v))), os.titleAlign != null, () => sset('titleAlign', null), 'the design view'));
    }
    if (w.title) out.card.push(mark(row('Space under title (pt)', textInput('s.titleGap', st.titleGap ?? '', (v) => sset('titleGap', v == null || v === '' ? null : Math.max(0, Math.min(40, +v))), { type: 'number', min: 0, max: 40, placeholder: '4' })), os.titleGap != null, () => sset('titleGap', null)));
    if (w.title) {
      // an icon in a chip before the title; "off" hides one a design view adds
      const ti = st.titleIcon && !st.titleIcon.off ? st.titleIcon : {};
      const tset = (patch) => sset('titleIcon', { off: null, ...patch });
      out.card.push(row('Title icon', iconPick('s.ti', ti.icon || '', (v) => (v ? tset({ icon: v, glyph: null }) : sset('titleIcon', { off: true })), { none: true, title: 'Icon before the title' })));
      if (ti.icon) {
        out.card.push(...chipRows('s.ti', ti, tset, { shape: 'square' }));
      } else if (ti.glyph) {
        out.card.push(h(`<div class="hint">This title has a “${esc(ti.glyph)}” badge. Pick an icon to replace it.</div>`));
      }
    }
  }
  return out;
}

const VIZ_UI = { row, seg, toggle, selectInput, textInput, colorField: (...a) => colorField(...a), group: (title, count, rows, key, first) => section([title, { badge: count || null, key: `vz.${key}`, closed: !first }], rows) };

/** The widget's visualization settings (opts.viz), one section per group. */
/** The widget as drawn: its design view merged under its own settings. */
const drawn = (f) => (f.vm?.eff ? { ...f.w, opts: f.vm.eff.opts || {}, style: f.vm.eff.style || {} } : f.w);

function vizSections(f) {
  const w = f.w;
  if (!SCHEMA[w.kind]) return { tools: null, parts: new Map() };
  const own = w.opts?.viz || {};
  let eff = resolveViz(S.st, drawn(f));
  // with a right axis the legend shows (it says which axis is whose) unless hidden on the widget itself:
  // its setting reads as drawn
  if (vizContext(drawn(f)).right && eff.legend === 'none' && own.legend == null) eff = { ...eff, legend: 'auto', legendForced: true };
  const set = (k, v) => upd(w.id, { opts: { viz: { [k]: v } } });
  // facts from the data: a KPI without a series or a comparison has no trend or change to show
  const k = f.vm.data?.kpi;
  const ctx = { ...vizContext(drawn(f)), drawn: DRAWN.get(w.id), noSpark: w.kind === 'kpi' && (k?.spark || []).filter((p) => p.v != null).length <= 2, noDelta: w.kind === 'kpi' && k?.pct == null && k?.delta == null };
  // where a value not set here comes from: the widget's design view, or the template's chart defaults
  const th = S.st.doc.theme;
  const view = (th.styles?.[w.kind] || []).find((v) => v.id === w.variant);
  const viewViz = isTheme() ? {} : view?.opts?.viz || {}, themeViz = th.viz?.[w.kind] || {};
  const has = (o, k) => o[k] != null && o[k] !== '';
  const srcOf = (k) => (has(viewViz, k) ? { kind: 'view', what: `the design view “${view.name || view.id}”` } : has(themeViz, k) ? { kind: 'theme', what: 'the template’s chart defaults (Theme → Chart defaults)' } : null);
  const parts = vizParts(w.kind, ctx, own, eff, set, { ...VIZ_UI, srcOf });
  if (eff.legendForced && parts.has('Legend')) parts.get('Legend').rows.push(h(`<div class="hint">Shown: the chart has a right axis, and the legend says which axis each series reads against (LHS / RHS). Choose <b>Hidden</b> to hide it anyway.</div>`));
  const n = Object.values(own).filter((v) => v != null && v !== '').length;
  if (!n) return { tools: null, parts };
  const tools = h(`<div class="vz-bar"><span>${icon('sliders', 14)} ${n} change${n > 1 ? 's' : ''} ${isTheme() ? 'in this view' : 'here'}</span><span class="grow"></span></div>`);
  const rs = h(`<button class="btn sm" title="Back to the defaults">${icon('restart', 13)} Reset</button>`);
  rs.addEventListener('click', () => upd(w.id, { opts: { viz: Object.fromEntries(Object.keys(own).map((k) => [k, null])) } }));
  tools.append(rs);
  if (!userMode() && !isTheme()) {
    const save = h(`<button class="btn sm" title="Make these the template default for every ${esc(KIND_NAMES[w.kind] || w.kind)}">${icon('upload', 13)} Use as default</button>`);
    save.addEventListener('click', async () => {
      await edit({ op: 'set_theme', patch: { viz: { [w.kind]: own } } });
      await upd(w.id, { opts: { viz: Object.fromEntries(Object.keys(own).map((k) => [k, null])) } });
      toast(`Saved as the default for ${KIND_NAMES[w.kind] || w.kind}.`);
    });
    tools.append(save);
  }
  return { tools, parts };
}

/**
 * The Design tab: every part of the visual in one order and under one name, whatever the kind
 * (SECTION_ORDER) — the schema's options, each series' or category's colour, values drawn as
 * lines, icons and the card, merged into the part they change.
 */
function designSections(f, d) {
  const w = f.w;
  const { tools, parts } = vizSections(f);
  const add = (title, rows, count = 0, where = 'end') => {
    if (!rows?.length) return;
    const p = parts.get(title) || { count: 0, rows: [] };
    p.rows = where === 'start' ? [...rows, ...p.rows] : [...p.rows, ...rows];
    p.count += count;
    parts.set(title, p);
  };
  add('Type & view', [...d.view, ...d.type], 0, 'start');
  // a widget of the designer's own type: which, and what can be done with it
  const wt = w.type && S.st.doc.theme.widget_types?.[w.type];
  if (wt) {
    const r = h(`<div class="wt-row">${icon(wt.icon || 'layers', 16)}<span><b>${esc(wt.name)}</b><span>Widget type · version ${wt.version}. Settings changed here apply to this widget; <b>Update the type</b> passes them to every widget of the type.</span></span></div>`);
    const acts = h(`<div class="m-row"></div>`);
    const up = h(`<button class="btn sm">${icon('upload', 13)} Update the type</button>`);
    up.addEventListener('click', () => saveWidgetType(f, { update: true }));
    const de = h(`<button class="btn sm">${icon('link', 13)} Detach</button>`);
    de.addEventListener('click', () => edit({ op: 'detach_widget_type', id: w.id }));
    acts.append(up, de);
    add('Type & view', [r, acts], 0, 'start');
  }
  const ser = seriesSection(f);
  if (ser) add('Series', ser.rows, ser.count, 'start');
  // two or more series: in Colours, Marks, Highlights and Data labels a part per series after what they share
  // (stacked columns keep their labels common: those are the stacks' totals)
  const sp = seriesParts(f);
  if (sp) {
    for (const title of ['Colours', 'Marks', 'Highlights', 'Data labels']) {
      const p = parts.get(title) || { count: 0, rows: [] };
      if (title === 'Data labels' && sp.stacked) continue;
      const common = commonRows(p.rows);
      const mine = sp.out[title] || [];
      if (!common.length && !mine.length) { parts.delete(title); continue; }
      const head = common.some((r) => r.classList?.contains('f-row')) && mine.length ? [h(`<div class="vz-sub">All series</div>`)] : [];
      parts.set(title, { count: (p.count || 0) + (sp.count[title] || 0), rows: [...head, ...common.filter((r) => !r.classList?.contains('vz-sub') || head.length), ...mine] });
    }
  }
  const sl = sp && !sp.stacked ? { any: sp.anyLabels } : null;
  // each series' colour sits on its card under Series; categories' colours (donuts, treemaps, tiles) here
  if (ENTITY_KINDS.includes(w.kind) && !ser) { const ec = entityColours(f); if (ec) add('Colours', ec.rows, ec.count); }

  const ic = chartIcons(f);
  if (ic) add('Icons', ic.rows, ic.count);
  // the KPI value's size is its text style's ("KPI values"): one setting, two doors
  if (w.kind === 'kpi') add('Value', [kpiSizeRow(f)], w.opts?.viz?.valueSize ? 1 : 0, 'start');
  // the card first, then how a KPI colours it
  add('Card & title', d.card, 0, 'start');
  // a chart part's text, where its other settings are (the same roles as in the Text tab)
  if (KIND_ROLES[w.kind]) {
    const eff = resolveViz(S.st, drawn(f));
    // only while the part is drawn: a hidden legend's or label's text has nothing to change
    const shown = { axis: eff.axis || eff.xLabels, data_label: (sl ? sl.any : eff.labels !== 'none') || eff.insideLabels || eff.valueLabels || eff.nameLabels,
      legend: w.kind === 'pie' ? eff.legendStyle !== 'none' : eff.legend !== 'none' };
    for (const [title, role] of [['Axes & grid', 'axis'], ['Data labels', 'data_label'], ['Legend', 'legend']]) {
      if (!parts.has(title) || !KIND_ROLES[w.kind].includes(role) || !shown[role]) continue;
      const own = Object.values(w.style?.fonts?.[role] || {}).some((v) => v != null);
      // with a part per series, their shared text comes first
      const shared = sl && role === 'data_label';
      add(title, [h(`<div class="vz-sub">${shared ? 'Text · all series' : 'Text'}</div>`), rolesEditor([role], (r) => w.style?.fonts?.[r], (r) => f.vm.fonts?.[r], (r, patch) => upd(w.id, { style: { fonts: { [r]: patch } } }), { colorField })], own ? 1 : 0, shared ? 'start' : 'end');
    }
  }
  const out = tools ? [tools] : [];
  ordered(parts).forEach(([title, p], i) => {
    out.push(section([title, { badge: p.count || null, key: `vz.${w.kind}.${title}`, closed: !(i === 0 || title === 'Layout' || title === 'Value' || p.count > 0) }], p.rows));
    // a table's and a contents list's own parts go after the visual's, before its card
    if (title === 'Type & view') {
      if (w.kind === 'table') out.push(...tableDesign(f));
      if (w.kind === 'toc') out.push(...tocDesign(f));
    }
  });
  if (!parts.has('Type & view')) {
    if (w.kind === 'table') out.unshift(...tableDesign(f));
    if (w.kind === 'toc') out.unshift(...tocDesign(f));
  }
  const cs = colourSection(f);
  if (cs) out.push(cs);
  return changedOnly(out);
}

/**
 * "Changed only": the Design tab cut down to what is set on this widget (the rows marked as set here), with a
 * switch at the top while anything is.
 */
function changedOnly(out) {
  const set = out.flatMap((el) => [...(el.querySelectorAll?.('.f-row.vz-set') || [])]).length;
  if (!set) return out;
  const bar = h(`<label class="vz-only"><input type="checkbox"${S.onlySet ? ' checked' : ''}> Changed only <span class="muted">· ${set} set here</span></label>`);
  bar.querySelector('input').addEventListener('change', (e) => { S.onlySet = e.target.checked; renderRight(); });
  if (S.onlySet) {
    for (const el of out) {
      if (!el.classList?.contains('insp-sec')) continue;
      el.querySelectorAll('.f-row:not(.vz-set)').forEach((r) => { r.hidden = true; });
      // hints, buttons and cards (a series' card) with nothing set in them
      el.querySelectorAll(':scope > .insp-body > :not(.f-row):not(.vz-sub)').forEach((c) => { if (!c.querySelector('.vz-set')) c.hidden = true; });
      // headings with nothing set under them go too, and parts with nothing set
      el.querySelectorAll('.vz-sub').forEach((sub) => {
        let n = sub.nextElementSibling, any = false;
        while (n && !n.classList.contains('vz-sub')) { if (n.classList.contains('vz-set')) any = true; n = n.nextElementSibling; }
        if (!any) sub.hidden = true;
      });
      if (!el.querySelector('.f-row.vz-set')) el.hidden = true;
      else el.classList.remove('closed');
    }
  }
  const tools = out[0]?.classList?.contains('vz-bar') ? out[0] : null;
  if (tools) tools.querySelector('.grow')?.before(bar); else out.unshift(bar);
  return out;
}

/** A report user's colours for each series or category, as a section of its own. */
function userColours(f) {
  if (!ENTITY_KINDS.includes(f.w.kind)) return null;
  const ec = entityColours(f);
  return ec ? section(['Colours', { icon: 'drop', badge: ec.count || null }], ec.rows) : null;
}

/** The KPI value's size: the widget's "KPI values" text size (an older own size is moved there). */
function kpiSizeRow(f) {
  const w = f.w;
  const legacy = w.opts?.viz?.valueSize;
  const own = legacy || w.style?.fonts?.kpi?.size;
  const now = legacy || f.vm.fonts?.kpi?.size;
  return row('Size (pt)', textInput('viz.kpiSize', own ?? '', (v) => upd(w.id, { opts: { viz: { valueSize: null } }, style: { fonts: { kpi: { size: v == null || v === '' ? null : Math.max(4, Math.min(96, +v)) } } } }),
    { type: 'number', min: 4, max: 96, placeholder: now ? String(now) : DRAWN.get(w.id)?.valueSize ? `auto · ${DRAWN.get(w.id).valueSize}` : 'auto' }));
}

/** Template-wide chart defaults (theme.viz[kind]). */
let DEFAULTS_KIND = 'bar';
function chartDefaults() {
  const th = S.st.doc.theme;
  const kind = DEFAULTS_KIND;
  const own = th.viz?.[kind] || {};
  const eff = resolveViz(S.st, { kind, opts: {} });
  const set = (k, v) => edit({ op: 'set_theme', patch: { viz: { [kind]: { [k]: v } } } });
  const pick = selectInput('vd.kind', kind, VIZ_KINDS.map((k) => [k, `${KIND_NAMES[k]}${Object.keys(th.viz?.[k] || {}).length ? ' •' : ''}`]), (v) => { DEFAULTS_KIND = v; render(); });
  const fake = { kind, opts: { viz: own }, binding: { measures: [{}, {}], dims: [] } };
  const ctx = { ...vizContext(fake), bars: ['bar', 'waterfall'].includes(kind), lines: ['line', 'area', 'bar'].includes(kind), multi: true, barSeries: 2 };
  const parts = [];
  vizGroups(kind, ctx, own, eff, set, { ...VIZ_UI, group: (title, count, rows) => { const g = h(`<div class="cd-group"><div class="cd-title">${icon(SECTION_ICONS[title] || 'sliders', 13)} ${esc(title)}${count ? `<span class="is-badge">${count}</span>` : ''}</div></div>`); g.append(...rows); parts.push(g); return g; } });
  return section('Chart defaults', row('For', pick), parts,
    h(`<div class="hint">Every ${esc((KIND_NAMES[kind] || kind).toLowerCase())} in the template starts from these. A widget's own settings still win.</div>`));
}

function fieldsOf(dsId) {
  return S.st.doc.datasets.find((d) => d.id === dsId || d.name === dsId)?.fields || [];
}

/** Where a KPI's other values can show. */
const KPI_ROLES = [['beside', 'Beside the main value'], ['note', 'In the note line'], ['chip', 'Chip at the right'], ['rows', 'Rows under the value'], ['none', 'Not shown']];

/**
 * A KPI's values by role. The first measure is the main value; the others are
 * placed by the KPI's settings (aside, note {n}, chip, breakdown rows), which
 * this reads and writes so each value says plainly where it goes.
 */
function kpiRoles(w) {
  const f = findWidget(w.id);
  const S2 = f ? resolveViz(S.st, drawn(f)) : {};
  const note = String(S2.note || '');
  const role = (i) => {
    if (S2.breakdown) return 'rows';
    if (Number(S2.aside) === i) return 'beside';
    if (Number(S2.chip) === i) return 'chip';
    if (note.includes(`{${i}}`)) return 'note';
    return 'none';
  };
  const dropFromNote = (t, i) => t.replace(new RegExp(`\\s*[·,]?\\s*\\{${i}\\}`), '').replace(/^\s*[·,]\s*/, '').trim();
  const set = (i, r) => {
    const p = {};
    if (r !== 'rows' && S2.breakdown) p.breakdown = false;
    if (r === 'rows') { p.breakdown = true; return upd(w.id, { opts: { viz: p } }); }
    p.aside = r === 'beside' ? i : Number(S2.aside) === i ? 0 : S2.aside ?? null;
    p.chip = r === 'chip' ? String(i) : Number(S2.chip) === i ? null : S2.chip ?? null;
    let t = note;
    if (r === 'note' && !t.includes(`{${i}}`)) t = t ? `${t} · {${i}}` : `{${i}}`;
    if (r !== 'note' && t.includes(`{${i}}`)) t = dropFromNote(t, i);
    if (t !== note) p.note = t || null;
    return upd(w.id, { opts: { viz: p } });
  };
  // swapping the main value with another keeps every {n}, aside and chip pointing at the same value
  const makeMain = (i) => {
    const ms = clone(w.binding.measures);
    [ms[0], ms[i]] = [ms[i], ms[0]];
    const swap = (n) => (n === i ? 0 : n === 0 ? i : n);
    const t = note.replace(/\{(\d+)\}/g, (_, n) => `{${swap(+n)}}`);
    const p = {};
    if (S2.aside != null && S2.aside !== '') p.aside = swap(Number(S2.aside)) || null;
    if (S2.chip != null && S2.chip !== '') p.chip = String(swap(Number(S2.chip)));
    if (t !== note) p.note = t;
    return upd(w.id, { binding: { measures: ms }, opts: { viz: p } });
  };
  return { role, set, makeMain };
}

/** A widget's filters and SQL filter (a composite's: shared by its parts). */
function filterRows(w, b, setB, fieldOpts) {
  const doc = S.st.doc;
  const out = [];
  const fbox = h(`<div class="filters"></div>`);
  b.filters.forEach((flt, i) => {
    const r = h(`<div class="m-row"></div>`);
    r.append(selectInput(`b.f${i}.field`, flt.field, fieldOpts(), (v) => { const n = clone(b.filters); n[i].field = v; setB({ filters: n }); }));
    r.append(selectInput(`b.f${i}.op`, flt.op || '=', [['=', '='], ['!=', '≠'], ['>', '>'], ['>=', '≥'], ['<', '<'], ['<=', '≤'], ['in', 'in'], ['contains', '∋']], (v) => { const n = clone(b.filters); n[i].op = v; setB({ filters: n }); }));
    r.append(textInput(`b.f${i}.value`, flt.value, (v) => { const n = clone(b.filters); n[i].value = v; setB({ filters: n }); }, { placeholder: rowFields(findWidget(w.id)?.pi ?? -1).length ? 'value, {{param}} or {{row.field}}' : 'value or {{param}}' }));
    const wpi = findWidget(w.id)?.pi ?? -1;
    if (rowFields(wpi).length) {
      const rb = h(`<button class="tb-btn sm row-pick" title="The value of this page’s row (the page repeats)">{{row}}</button>`);
      rb.addEventListener('click', () => rowMenu(rb, wpi, (tok) => { const n = clone(b.filters); n[i].value = tok; setB({ filters: n }); }));
      r.append(rb);
    }
    const x = h(`<button class="tb-btn sm">${icon('x', 13)}</button>`);
    x.addEventListener('click', () => { const n = clone(b.filters); n.splice(i, 1); setB({ filters: n }); });
    r.append(x);
    fbox.append(r);
  });
  const addF = h(`<button class="btn sm">${icon('filter', 13)} Add filter</button>`);
  addF.addEventListener('click', () => setB({ filters: [...b.filters, { field: '', op: '=', value: doc.param_defs[0] ? `{{${doc.param_defs[0].name}}}` : '' }] }));
  fbox.append(addF);
  out.push(row('Filters', fbox, 'stack'));
  const sqlBox = textInput('b.where', b.where_sql || '', (v) => setB({ where_sql: v.trim() }), { area: true, rows: 2, placeholder: "e.g. amount > 1e6 AND product_group <> 'Banking'" });
  sqlBox.classList.add('code');
  out.push(row('SQL filter', sqlBox, 'stack'));
  return out;
}

// what a composite can hold, in its Add menu
const PART_KINDS = [['kpi', 'Figure', 5, 3], ['bar', 'Bar chart', 6, 4], ['line', 'Line', 6, 3], ['area', 'Area', 6, 3], ['pie', 'Donut', 4, 4],
  ['table', 'Table rows', 12, 3], ['tiles', 'Tiles', 12, 2], ['text', 'Text', 12, 1], ['heading', 'Heading', 12, 1], ['divider', 'Line', 12, 1], ['image', 'Image or logo', 2, 2]];

/** A composite's Content: the data source and filters its parts share, and its parts. */
function compositeSections(f) {
  const w = f.w;
  const doc = S.st.doc;
  const b = w.binding || { dataset: '', dims: [], measures: [], window: 'period', filters: [], sort: '', limit: 0, others: false };
  const setB = (patch) => upd(w.id, { binding: { ...b, ...patch } });
  const fields = fieldsOf(b.dataset);
  const fieldOpts = (filter) => [['', '—'], ...fields.filter(filter || (() => true)).map((x) => [x.name, `${x.name}${x.kind === 'date' ? ' 📅' : x.kind === 'number' ? ' #' : ''}`])];
  const items = [row('Dataset', selectInput('b.dataset', b.dataset, [['', 'Choose…'], ...doc.datasets.map((d) => [d.id, d.name || d.id])], (v) => setB({ dataset: v }))),
    h(`<div class="hint">Every part reads this source with these filters, unless a part picks another source; its own filters add to them.</div>`)];
  if (b.dataset) items.push(...filterRows(w, b, setB, fieldOpts));
  const list = h(`<div class="cp-list"></div>`);
  (w.parts || []).forEach((p) => {
    const pid = `${w.id}::${p.id}`;
    const r = h(`<div class="cp-row"><button class="cp-open">${icon(kindIcon(p.kind), 14)}<span><b>${esc(widgetName({ ...p, id: pid }))}</b><span>${esc(kindName(p.kind))} · ${p.w}×${p.h} at ${p.x + 1}, ${p.y + 1}</span></span></button><button class="tb-btn sm" title="Remove this part">${icon('x', 13)}</button></div>`);
    r.querySelector('.cp-open').addEventListener('click', () => { S.sel = pid; render(); });
    r.querySelector('.tb-btn').addEventListener('click', () => edit({ op: 'remove_widget', id: pid }));
    list.append(r);
  });
  if (!(w.parts || []).length) list.append(h(`<div class="hint">No parts yet.</div>`));
  const add = h(`<button class="btn sm">${icon('plus', 13)} Add a part</button>`);
  add.addEventListener('click', () => menu(add, PART_KINDS.map(([k, label, pw, ph]) => ({ label, icon: kindIcon(k), run: async () => {
    const nw = newWidget(k);
    const part = { kind: k, x: 0, y: 0, w: pw, h: ph, title: '', subtitle: '', style: { card: false }, opts: nw.opts || {}, binding: nw.binding ? { ...nw.binding, dataset: '' } : null, variant: nw.variant || '' };
    if (k === 'text') part.opts = { text: 'Write here.' };
    if (k === 'heading') part.opts = { text: 'Heading' };
    const st = await edit({ op: 'add_part', id: w.id, part });
    const added = st && findWidget(w.id)?.w.parts?.slice(-1)[0];
    if (added) { S.sel = `${w.id}::${added.id}`; render(); }
  } }))));
  return [section(['Data', { icon: 'database' }], items), section(['Parts', { icon: 'grid', badge: (w.parts || []).length || null }], list, add,
    h(`<div class="hint">Click a part on the canvas (with the composite selected) to edit it with its full inspector; drag it to move it, its handles to resize it.</div>`))];
}

function dataSection(w) {
  const b = w.binding || { dataset: '', dims: [], measures: [], window: 'period', filters: [], sort: '', limit: 0, others: false };
  const doc = S.st.doc;
  const fields = fieldsOf(b.dataset);
  const setB = (patch) => upd(w.id, { binding: { ...b, ...patch } });
  const items = [];
  if (!doc.datasets.length) {
    items.push(h(`<div class="note">${icon('database', 15)}<span>Add a data source in the Data tab first.</span></div>`));
    return section('Data', items);
  }
  const dsRow = h(`<div class="m-row"></div>`);
  dsRow.append(selectInput('b.dataset', b.dataset, [['', 'Choose…'], ...doc.datasets.map((d) => [d.id, d.name || d.id])], (v) => setB({ dataset: v })));
  const curDs = doc.datasets.find((d) => d.id === b.dataset || d.name === b.dataset);
  if (curDs) {
    const fx = h(`<button class="btn sm" title="New calculated field (SQL)"><span class="fx">fx</span></button>`);
    fx.addEventListener('click', () => calcFieldEditor(curDs, null));
    dsRow.append(fx);
  }
  items.push(row('Dataset', dsRow));
  const fieldOpts = (filter) => [['', '—'], ...fields.filter(filter || (() => true)).map((f) => [f.name, `${f.name}${f.kind === 'date' ? ' 📅' : f.kind === 'number' ? ' #' : ''}`])];
  const detail = w.kind === 'table' && b.mode === 'rows';
  if (w.kind === 'table') {
    items.push(row('Rows', seg('b.mode', b.mode || '', [['', 'Summary'], ['rows', 'Detail rows']], (v) => {
      const patch = { mode: v };
      if (v === 'rows' && !b.columns?.length) patch.columns = [...b.dims.map((d) => d.split(':')[0]), ...b.measures.map((m) => m.field).filter((x) => x && x !== '*')];
      if (!patch.columns?.length && v === 'rows') patch.columns = fields.slice(0, 5).map((f) => f.name);
      setB(patch);
    })));
  }
  if (detail) {
    const cbox = h(`<div class="measures"></div>`);
    const colsList = b.columns || [];
    colsList.forEach((c, i) => {
      const r = h(`<div class="m-row col-pick"><span class="grip">${icon('grip2', 13)}</span></div>`);
      r.append(selectInput(`b.c${i}`, c, fieldOpts(), (v) => { const n = [...colsList]; if (v) n[i] = v; else n.splice(i, 1); setB({ columns: n }); }));
      const up = h(`<button class="tb-btn sm" title="Move up"${i === 0 ? ' disabled' : ''}>${icon('chevl', 13)}</button>`);
      up.style.transform = 'rotate(90deg)';
      up.addEventListener('click', () => { const n = [...colsList]; [n[i - 1], n[i]] = [n[i], n[i - 1]]; setB({ columns: n }); });
      const x = h(`<button class="tb-btn sm" title="Remove">${icon('x', 13)}</button>`);
      x.addEventListener('click', () => { const n = [...colsList]; n.splice(i, 1); setB({ columns: n }); });
      r.append(up, x);
      cbox.append(r);
    });
    const add = selectInput('b.cadd', '', [['', '+ Add column'], ...fields.filter((f) => !colsList.includes(f.name)).map((f) => [f.name, f.name])], (v) => v && setB({ columns: [...colsList, v] }));
    cbox.append(add);
    items.push(row('Columns', cbox, 'stack'));
  }

  // dimensions
  if (w.kind !== 'kpi' && !detail) {
    const dims = [...b.dims];
    const maxDims = w.kind === 'table' ? 3 : ['bar', 'line', 'area'].includes(w.kind) ? 2 : 1;
    for (let i = 0; i < Math.min(maxDims, dims.length + 1); i++) {
      const [field, grain] = (dims[i] || '').split(':');
      const isDate = fields.find((f) => f.name === field)?.kind === 'date';
      const r = h(`<div class="dim-row"></div>`);
      r.append(selectInput(`b.dim${i}`, field || '', fieldOpts(), (v) => {
        const fd = fields.find((f) => f.name === v);
        const nd = [...dims];
        if (!v) nd.splice(i, 1); else nd[i] = fd?.kind === 'date' ? `${v}:${grain || 'month'}` : v;
        setB({ dims: nd.filter(Boolean) });
      }));
      if (isDate) r.append(selectInput(`b.grain${i}`, grain || 'month', GRAINS, (v) => { const nd = [...dims]; nd[i] = `${field}:${v}`; setB({ dims: nd }); }));
      items.push(row(i === 0 ? (w.kind === 'line' || w.kind === 'area' ? 'X axis' : 'Group by') : w.kind === 'table' ? 'Then by' : 'Split by', r));
    }
    if (w.kind === 'table') {
      const [pf, pg] = (b.pivot || '').split(':');
      const isDate = fields.find((x) => x.name === pf)?.kind === 'date';
      const r = h(`<div class="dim-row"></div>`);
      r.append(selectInput('b.pivot', pf || '', [['', 'None'], ...fields.filter((x) => !b.dims.some((d) => d.split(':')[0] === x.name)).map((x) => [x.name, `${x.name}${x.kind === 'date' ? ' 📅' : ''}`])], (v) => {
        const fd = fields.find((x) => x.name === v);
        setB({ pivot: !v ? '' : fd?.kind === 'date' ? `${v}:${pg || 'month'}` : v, sort: b.sort && /^\d+:/.test(b.sort) ? '' : b.sort });
      }));
      if (isDate) r.append(selectInput('b.pgrain', pg || 'month', GRAINS, (v) => setB({ pivot: `${pf}:${v}` })));
      items.push(row('Columns by', r));
    }
  }
  // measures
  const ms = b.measures.length ? b.measures : [];
  const mbox = h(`<div class="measures"></div>`);
  const kpi = w.kind === 'kpi' ? kpiRoles(w) : null;
  if (kpi) mbox.append(h(`<div class="hint">The <b>main value</b> is the big number. Every other value shows where you choose: beside it, in the note line, in a chip or as rows under it.</div>`));
  ms.forEach((m, i) => {
    const card = h(`<div class="measure${kpi ? (i === 0 ? ' kpi-main' : ' kpi-other') : ''}"></div>`);
    if (kpi) {
      // a KPI's values by role: the big number, then where each other value goes
      const head = h(`<div class="m-head kpi-role"></div>`);
      if (i === 0) head.append(h(`<span class="role-main">${icon('kpi', 13)} Main value</span>`));
      else {
        head.append(h(`<span class="role-n">Value ${i + 1}</span>`));
        head.append(selectInput(`b.m${i}.role`, kpi.role(i), KPI_ROLES, (v) => kpi.set(i, v)));
        const mk = h(`<button class="tb-btn sm" title="Make this the main value">${icon('star', 13)}</button>`);
        mk.addEventListener('click', () => kpi.makeMain(i));
        head.append(mk);
      }
      card.append(head);
    } else if (ms.length > 1 || w.kind === 'table') {
      const head = h(`<div class="m-head"><span>${w.kind === 'table' ? 'Column' : 'Series'} ${i + 1}</span></div>`);
      const move = (d) => {
        if (w.kind === 'table') {
          const f2 = findWidget(w.id);
          const cols = tableColumns(f2);
          const at = cols.findIndex((c) => c.key === (m.id || `m${i}`));
          if (at >= 0) moveColumn(f2, cols[at].key, at + d);
        } else {
          const n = clone(ms);
          const j = i + d;
          if (j < 0 || j >= n.length) return;
          [n[i], n[j]] = [n[j], n[i]];
          setB({ measures: n });
        }
      };
      const up = h(`<button class="tb-btn sm" title="${w.kind === 'table' ? 'Move column left' : 'Move up'}">${icon('arrow-up', 13)}</button>`);
      const dn = h(`<button class="tb-btn sm" title="${w.kind === 'table' ? 'Move column right' : 'Move down'}">${icon('arrow-down', 13)}</button>`);
      up.addEventListener('click', () => move(-1));
      dn.addEventListener('click', () => move(1));
      head.append(up, dn);
      card.append(head);
    }
    const r1 = h(`<div class="m-row"></div>`);
    r1.append(selectInput(`b.m${i}.agg`, m.agg, AGGS, (v) => { const n = clone(ms); n[i].agg = v; setB({ measures: n }); }));
    r1.append(selectInput(`b.m${i}.field`, m.field, [['*', 'Rows'], ...fields.map((f) => [f.name, f.name])], (v) => { const n = clone(ms); n[i].field = v; setB({ measures: n }); }));
    const x = h(`<button class="tb-btn sm" title="Remove">${icon('x', 13)}</button>`);
    x.addEventListener('click', () => { const n = clone(ms); n.splice(i, 1); setB({ measures: n }); });
    r1.append(x);
    const r2 = h(`<div class="m-row"></div>`);
    r2.append(textInput(`b.m${i}.label`, m.label, (v) => { const n = clone(ms); n[i].label = v; setB({ measures: n }); }, { placeholder: 'Label' }));
    r2.append(selectInput(`b.m${i}.format`, m.format || 'number', FORMATS, (v) => { const n = clone(ms); n[i].format = v; setB({ measures: n }); }));
    const r3 = h(`<div class="m-row ratio"><span>÷</span></div>`);
    r3.append(selectInput(`b.m${i}.div`, m.divide_by || '', [['', 'No ratio'], ...fields.filter((f) => f.kind === 'number').map((f) => [f.name, `sum of ${f.name}`])], (v) => { const n = clone(ms); n[i].divide_by = v || null; if (v && n[i].format === 'number') n[i].format = 'percent'; setB({ measures: n }); }));
    card.append(r1, r2, r3);
    if (w.kind === 'bar' && ms.length > 1) {
      const r5 = h(`<div class="m-row"></div>`);
      r5.append(toggle(`b.m${i}.line`, m.chart === 'line', (v) => { const n = clone(ms); n[i].chart = v ? 'line' : null; setB({ measures: n }); }, 'Draw as a line'));
      card.append(r5);
    }
    if (w.kind !== 'kpi') {
      const r4 = h(`<div class="m-row ratio"><span>Δ</span></div>`);
      const calcs = [['', 'This period'], ['prev', 'Comparison period'], ['delta', 'Change vs comparison'], ['pct', '% change vs comparison']];
      if (w.kind === 'table') calcs.push(['share', 'Share of total'], ['rank', 'Rank'], ['running', 'Running total'], ['trend', 'Trend (12-month sparkline)']);
      r4.append(selectInput(`b.m${i}.calc`, m.calc || '', calcs, (v) => {
        const n = clone(ms);
        n[i].calc = v;
        if (v === 'pct' || v === 'share') n[i].format = 'percent';
        const suffix = { prev: ' (prev.)', delta: ' Δ', pct: ' Δ%', share: ' share', rank: ' rank', running: ' (running)', trend: ' trend' }[v] || '';
        if (!n[i].label || /( \(prev\.\)| Δ%?| share| rank| \(running\)| trend)$/.test(n[i].label)) n[i].label = `${m.field === '*' ? 'Count' : cap(m.field)}${suffix}`;
        setB({ measures: n });
      }));
      card.append(r4);
    }
    mbox.append(card);
  });
  const add = h(`<button class="btn sm">${icon('plus', 13)} Add value</button>`);
  add.addEventListener('click', async () => {
    const num = fields.find((f) => f.kind === 'number');
    const snap = S.st.doc.datasets.find((d) => d.id === b.dataset)?.snapshot;
    await setB({ measures: [...ms, { id: nextMeasureId(ms), field: num?.name || '*', agg: num ? (snap ? 'closing' : 'sum') : 'count', label: '', format: 'number' }] });
    // a new KPI value shows straight away: beside the main one, or in the note
    if (kpi && ms.length >= 1) kpiRoles(findWidget(w.id)?.w || w).set(ms.length, kpi.role(1) === 'beside' || ms.length > 1 ? 'note' : 'beside');
  });
  if (!(w.kind === 'pie' && ms.length >= 1)) mbox.append(add);
  if (!detail) items.push(row('Values', mbox, 'stack'));
  // a slice of dated data: only a source with a date column has one to pick
  if (S.st.doc.datasets.find((d) => d.id === b.dataset)?.date_field) items.push(row('Time', selectInput('b.window', b.window, WINDOWS, (v) => setB({ window: v }))));
  // filters
  items.push(...filterRows(w, b, setB, fieldOpts));
  items.push(row('If empty, show', textInput('o.emptyText', w.opts?.emptyText || '', (v) => upd(w.id, { opts: { emptyText: v.trim() || null } }), { placeholder: 'e.g. No losses this period' })));
  if (w.kind !== 'kpi') {
    const colNames = detail ? (b.columns || []).map(cap) : [...b.dims.map((d) => cap(d.split(':')[0])), ...b.measures.map((m) => m.label || (m.field === '*' ? 'Count' : cap(m.field)))];
    const sortOpts = w.kind === 'table' && b.pivot && !detail
      ? [['', 'Automatic'], ['value_desc', 'Largest total first'], ['value_asc', 'Smallest total first'], ['dim_asc', 'A → Z / oldest first'], ['dim_desc', 'Z → A / newest first'], ['data', 'As in the data']]
      : w.kind === 'table'
      ? [['', detail ? 'As in the data' : 'Automatic'], ...(detail ? [] : [['data', 'As in the data']]), ...colNames.flatMap((n, i) => [[`${i}:desc`, `${n} ↓`], [`${i}:asc`, `${n} ↑`]])]
      : [['', 'Automatic'], ['value_desc', 'Largest first'], ['value_asc', 'Smallest first'], ['dim_asc', 'A → Z / oldest first'], ['dim_desc', 'Z → A / newest first'], ['data', 'As in the data']];
    items.push(row('Sort', selectInput('b.sort', b.sort, sortOpts, (v) => setB({ sort: v }))));
    const lim = h(`<div class="m-row"></div>`);
    lim.append(textInput('b.limit', b.limit || '', (v) => setB({ limit: Math.max(0, +v || 0) }), { type: 'number', min: 0, placeholder: 'All' }));
    if (!detail) lim.append(toggle('b.others', b.others, (v) => setB({ others: v }), '“Other” row'));
    items.push(row(detail ? 'Max rows' : 'Top N', lim));
  }
  return section('Data', items);
}

// ---------------------------------------------------------------- variables

/**
 * Kinds of {{placeholder}}, each with its own colour wherever a token is
 * shown: the designer's own variables, the period, document settings, report
 * parameters, page numbers, and values read from data or SQL.
 */
export const VAR_KINDS = [
  { id: 'mine', label: 'My variables', group: 'My variables' },
  { id: 'period', label: 'Period & dates', group: 'Period' },
  { id: 'doc', label: 'Document', group: 'Document' },
  { id: 'param', label: 'Parameters', group: 'Parameters' },
  { id: 'page', label: 'Page', group: 'Page' },
  { id: 'data', label: 'Data & SQL', group: 'From the data' },
];
const PERIOD_VARS = ['period', 'start', 'end', 'compare', 'today', 'data_start', 'data_end', 'period_start', 'period_end', 'prev_start', 'prev_end'];
const DOC_VARS = ['title', 'subtitle', 'organization', 'author'];
const PAGE_VARS = ['page', 'pages', 'section'];

/** The kind of a token such as `{{end}}`, `{{data:…}}` or `:period_start` (SQL), or null. */
export function varKind(text) {
  const t = String(text || '').trim();
  let m = t.match(/^\{\{\s*([^}|]*?)\s*(?:\|[^}]*)?\}\}$/);
  const sql = !m && t.match(/^:([A-Za-z_]\w*)$/);
  const name = m ? m[1] : sql ? sql[1] : null;
  if (name == null) return null;
  if (/^(data|sql)\s*:/i.test(name)) return 'data';
  const doc = S.st?.doc;
  if (doc?.variables?.some((v) => v.name.trim() === name)) return 'mine';
  if (doc?.param_defs?.some((p) => p.name === name)) return 'param';
  if (PERIOD_VARS.includes(name)) return 'period';
  if (DOC_VARS.includes(name)) return 'doc';
  if (PAGE_VARS.includes(name)) return 'page';
  return name === 'name' || name === 'param' ? 'param' : null;
}

/** Colours every <code> token under `root` by its kind. */
export function tintTokens(root) {
  if (!root?.querySelectorAll) return;
  const list = root.matches?.('code') ? [root] : root.querySelectorAll('code');
  for (const c of list) {
    const k = varKind(c.textContent);
    if (k) c.dataset.vk = k;
    else delete c.dataset.vk;
  }
}
if (typeof MutationObserver !== 'undefined' && typeof document !== 'undefined') {
  // tokens appear in hints and notes all over the interface
  new MutationObserver((list) => {
    for (const m of list) for (const n of m.addedNodes) if (n.nodeType === 1) tintTokens(n);
  }).observe(document.documentElement, { childList: true, subtree: true });
}

/** Every {{placeholder}}: what it shows now and how it is filled; the designer's own ones can be edited. */
function variablesTab() {
  const vars = S.st.variables || [];
  const el = h(`<div class="rail-sec vars">
    <div class="hint">Use these in any text: titles, headings, notes, headers and footers. Click a token to copy it. The colour tells you where a token's value comes from.</div>
    <div class="var-legend">${VAR_KINDS.map((k) => `<span data-vk="${k.id}"><i></i>${esc(k.label)}</span>`).join('')}</div>
    <div class="sec-title row" data-vk="mine"><i class="vk-dot"></i>My variables<button class="btn sm" id="var-add">${icon('plus', 14)} Add</button></div>
    <div class="var-mine"></div>
    <div class="var-groups"></div>
  </div>`);
  const row = (v) => {
    const at = v.custom ? null : varSource(v);
    // a description: the designer's own, or how a built-in is filled (unless "where" already says it)
    const desc = v.custom ? v.description || v.how : at?.go && /^Document →/.test(v.how) ? '' : v.how;
    // name · code · current value · description, one under the other
    const r = h(`<div class="var-row${v.custom ? '' : ' fixed'}">
      <div class="var-head"><b class="var-name">${esc(v.label)}</b></div>
      <button class="var-tok" title="Copy ${esc(v.token)}"><code>${esc(v.token)}</code>${icon('copy', 12)}</button>
      <button class="var-body" title="${esc(v.custom ? 'Edit this variable' : at.go ? 'Show where it is set' : at.where)}">
        <span class="var-val"><span class="var-k">Value</span>${v.value ? esc(v.value) : `<em>${v.group === 'Page' ? 'depends on the page' : v.token === '{{compare}}' ? 'no comparison' : 'not set'}</em>`}</span>
        ${desc ? `<span class="var-how">${esc(desc)}</span>` : ''}
        ${at ? `<span class="var-where">${icon(at.go ? 'lock' : 'info', 11)}<span>${esc(at.where)}</span></span>` : ''}</button>
    </div>`);
    r.querySelector('.var-tok').addEventListener('click', async () => {
      try { await navigator.clipboard.writeText(v.token); toast(`${v.token} copied. Paste it into any text.`); } catch { toast(v.token, 'info'); }
    });
    r.querySelector('.var-body').addEventListener('click', () => (v.custom ? variableEditor(v.custom) : at.go ? at.go() : toast(at.where, 'info')));
    if (v.custom) {
      const more = h(`<button class="tb-btn sm gi-more" title="Edit">${icon('more', 15)}</button>`);
      more.addEventListener('click', (e) => menu(e.currentTarget, [
        { label: 'Edit…', icon: 'rename', run: () => variableEditor(v.custom) },
        { label: 'Delete', icon: 'trash', danger: true, run: () => edit({ op: 'set_variables', variables: (S.st.doc.variables || []).filter((x) => x.name !== v.custom) }) },
      ], { align: 'end' }));

      r.querySelector('.var-head').append(more);
    }
    return r;
  };
  const mine = vars.filter((v) => v.custom);
  const mineEl = el.querySelector('.var-mine');
  if (mine.length) mine.forEach((v) => mineEl.append(row(v)));
  else mineEl.append(h(`<div class="empty-note small"><span>None yet. A variable is a name for text, an SQL query or a data value you use in many places, e.g. <code>{{bank_name}}</code> or <code>{{aum}}</code>.</span></div>`));
  const groups = el.querySelector('.var-groups');
  for (const g of ['Period', 'Document', 'Parameters', 'Page']) {
    const list = vars.filter((v) => v.group === g);
    if (!list.length) continue;
    const k = VAR_KINDS.find((x) => x.group === g);
    groups.append(h(`<div class="sec-title" data-vk="${k.id}"><i class="vk-dot"></i>${esc(g)}</div>`));
    list.forEach((v) => groups.append(row(v)));
  }
  groups.append(h(`<div class="sec-title" data-vk="data"><i class="vk-dot"></i>From the data</div>`));
  groups.append(h(`<div class="var-row static"><code class="var-tok">{{data:Source.field|agg}}</code><div class="var-body"><span class="var-how">A value from a dataset in the period: <code>last</code>, <code>first</code>, <code>sum</code>, <code>avg</code>, <code>min</code>, <code>max</code>, <code>count</code>, <code>list</code>. A column named like a parameter follows it.</span></div></div>`));
  groups.append(h(`<div class="var-row static"><code class="var-tok">{{sql: select …}}</code><div class="var-body"><span class="var-how">The first cell of an SQL query over your sources; <code>:period_start</code>, <code>:period_end</code> and parameters work in it.</span></div></div>`));
  el.querySelector('#var-add').addEventListener('click', () => variableEditor(null));
  return el;
}

/** Opens the icon picker of one Contents section (clicked on the page). */
export function pickTocIcon(name) {
  const ctx = userMode() ? 'user' : 'widget';
  S.inspTab = { ...(S.inspTab || {}), [ctx]: 'design' };
  S.inspQuery = '';
  render();
  setTimeout(() => {
    const b = right.querySelector(`[data-key="toc.icons.${CSS.escape(name)}"]`) || right.querySelector('[data-key="toc.icon"]');
    if (!b) return;
    flash(b);
    setTimeout(() => b.click(), 350);
  }, 60);
}

/** Draws the eye to a control: scrolls it into view and flashes it. */
function flash(el) {
  if (!el) return;
  el.scrollIntoView({ block: 'center', behavior: 'smooth' });
  el.classList.remove('locate');
  void el.offsetWidth;
  el.classList.add('locate');
  setTimeout(() => el.classList.remove('locate'), 2400);
  if (el.matches?.('input, select, textarea')) el.focus({ preventScroll: true });
}

/**
 * Where a built-in variable gets its value, and a way to go there: built-in
 * values are read-only on the Variables tab and are set in the toolbar, the
 * document's settings, the page or a parameter.
 */
function varSource(v) {
  const t = v.token;
  const docField = (key, label) => ({ where: `Set in Document → ${label}`, go: () => { S.sel = null; S.colEdit = null; S.inspTab = { ...(S.inspTab || {}), doc: 'doc' }; render(); setTimeout(() => flash(right.querySelector(`[data-key="${key}"]`)), 60); } });
  const toolbar = (sel, label) => ({ where: `Set in the toolbar · ${label}`, go: () => flash(document.querySelector(sel)) });
  if (['{{period}}', '{{start}}', '{{end}}'].includes(t)) return toolbar('#plabel', 'period');
  if (t === '{{compare}}') return toolbar('#compare', 'comparison');
  if (t === '{{today}}') return { where: 'Filled automatically: the day the report is opened or exported' };
  if (t === '{{page}}' || t === '{{pages}}') return { where: 'Filled automatically on each page (headers and footers)' };
  if (t === '{{section}}') return { where: 'Set per page: page settings → Section', go: () => renamePage(S.page) };
  if (t === '{{title}}') return docField('m.title', 'Title');
  if (t === '{{subtitle}}') return docField('m.subtitle', 'Subtitle');
  if (t === '{{organization}}') return docField('m.org', 'Organization');
  if (t === '{{author}}') return docField('m.author', 'Author');
  if (v.group === 'Parameters') {
    const name = t.slice(2, -2);
    return { where: `Set in the toolbar · ${v.label} · defined in Data → Report parameters`, go: () => {
      const c = document.querySelector(`#toolbar [data-param="${CSS.escape(name)}"]`);
      if (c) flash(c);
      if (!userMode()) toast(`Report users pick “${v.label}” in the toolbar. To change its choices, edit the parameter.`, 'info');
      if (!c && !userMode()) paramEditor();
    } };
  }
  return { where: v.how };
}

/** Adds or edits one of the designer's variables, with a live test. */
async function variableEditor(name) {
  const list = clone(S.st.doc.variables || []);
  const cur = list.find((v) => v.name === name) || { name: '', label: '', kind: 'text', value: '', format: '', description: '' };
  const sources = S.st.doc.datasets;
  const body = h(`<div class="form var-editor">
    <div class="form two"><label>Name <span class="muted">— used as <code>{{name}}</code></span><input name="name" value="${esc(cur.name)}" placeholder="aum"></label>
    <label>Label<input name="label" value="${esc(cur.label)}" placeholder="Assets under management"></label></div>
    <label>Filled from<div class="seg var-kind">${[['text', 'Text'], ['sql', 'SQL query'], ['data', 'Data value']].map(([k, l]) => `<button type="button" data-v="${k}" class="${cur.kind === k ? 'on' : ''}">${l}</button>`).join('')}</div></label>
    <label class="var-value"></label>
    <div class="form two"><label>Format<select name="format">${[['', 'As it comes'], ['number', 'Number'], ['currency', 'Currency'], ['percent', 'Percent'], ['date', 'Date']].map(([k, l]) => `<option value="${k}"${cur.format === k ? ' selected' : ''}>${l}</option>`).join('')}</select></label>
    <label>Description<input name="desc" value="${esc(cur.description || '')}" placeholder="What it is, for other designers"></label></div>
    <div class="ds-actions"><button class="btn" type="button" id="var-test">${icon('play', 14)} Test</button><span class="var-result muted"></span></div>
  </div>`);
  let kind = cur.kind || 'text';
  const slot = body.querySelector('.var-value');
  const drawValue = () => {
    const v = slot.querySelector('textarea, input')?.value ?? cur.value;
    if (kind === 'text') slot.innerHTML = `Text <span class="muted">— may hold other variables, e.g. “as of {{end}}”</span><textarea name="value" rows="2"></textarea>`;
    else if (kind === 'sql') slot.innerHTML = `SQL <span class="muted">— the first cell of the result; sources by name (${sources.map((d) => `<code>${esc(sqlName(d.name))}</code>`).join(', ') || 'none'}); <code>:period_start</code>, <code>:period_end</code>, <code>:prev_start</code>, <code>:prev_end</code>, parameters</span><textarea name="value" class="code" rows="5" placeholder="select sum(value) from portfolio where date = :end"></textarea>`;
    else slot.innerHTML = `Data value <span class="muted">— Source.field|agg, e.g. ${esc(sources[0] ? sources[0].name : 'Sales')}.${esc(sources[0]?.fields?.find((f) => f.kind === 'number')?.name || 'value')}|sum</span><input name="value" class="code" placeholder="Source.field|last">`;
    const inp = slot.querySelector('textarea, input');
    inp.value = v;
    inp.addEventListener('keydown', (e) => e.stopPropagation());
  };
  drawValue();
  body.querySelectorAll('.var-kind button').forEach((b) => b.addEventListener('click', () => {
    kind = b.dataset.v;
    body.querySelectorAll('.var-kind button').forEach((x) => x.classList.toggle('on', x === b));
    drawValue();
  }));
  body.querySelectorAll('input, textarea').forEach((x) => x.addEventListener('keydown', (e) => e.stopPropagation()));
  const read = () => ({
    name: body.querySelector('[name=name]').value.trim().replace(/[^\p{L}\p{N}_]+/gu, '_'),
    label: body.querySelector('[name=label]').value.trim(),
    kind, value: slot.querySelector('textarea, input').value,
    format: body.querySelector('[name=format]').value,
    description: body.querySelector('[name=desc]').value.trim(),
  });
  // a test runs the variable through the engine without keeping it
  body.querySelector('#var-test').addEventListener('click', async () => {
    const v = read();
    const res = body.querySelector('.var-result');
    if (!v.name) { res.textContent = 'Give it a name first.'; return; }
    res.textContent = 'Testing…';
    const others = list.filter((x) => x.name !== name && x.name !== v.name);
    const st = await edit({ op: 'set_variables', variables: [...others, v] }, { quiet: true });
    const got = st?.variables?.find((x) => x.custom === v.name)?.value;
    await api.undo();
    res.innerHTML = got == null ? 'Could not run it.' : /^\[.*\]$/.test(got) ? `<span class="err">${esc(got)}</span>` : `${icon('check', 13)} <b>${esc(got)}</b>`;
  });
  const ok = await modal({ title: name ? `Variable · {{${name}}}` : 'New variable', iconName: 'formula', body, wide: true, buttons: [{ label: 'Cancel', value: null }, { label: name ? 'Save' : 'Add variable', kind: 'primary', value: true }] });
  if (!ok) return;
  const v = read();
  if (!v.name) { toast('A variable needs a name.', 'error'); return; }
  const builtin = (S.st.variables || []).some((x) => !x.custom && x.token === `{{${v.name}}}`);
  if (builtin) { toast(`{{${v.name}}} is built in. Choose another name.`, 'error'); return; }
  const next = list.filter((x) => x.name !== name && x.name !== v.name);
  next.push(v);
  if (await edit({ op: 'set_variables', variables: next })) toast(`{{${v.name}}} is ready. Use it in any text.`);
}

// ---- header & footer

const SLOT_NAMES = { left: 'Left', center: 'Centre', right: 'Right' };
const BAND_OPEN = new Set(['footer.left']);

/** Inserts text at the caret of an input and commits it. */
function insertAt(inp, text) {
  const a = inp.selectionStart ?? inp.value.length, b = inp.selectionEnd ?? a;
  inp.value = inp.value.slice(0, a) + text + inp.value.slice(b);
  inp.focus();
  inp.setSelectionRange(a + text.length, a + text.length);
  inp.dispatchEvent(new Event('change'));
}

function tokenMenu(anchor, inp) {
  const doc = S.st.doc;
  const items = [
    { title: 'Page' },
    { label: 'Page number', hint: '{{page}}', run: () => insertAt(inp, '{{page}}') },
    { label: 'Page count', hint: '{{pages}}', run: () => insertAt(inp, '{{pages}}') },
    { label: 'Section', hint: '{{section}}', run: () => insertAt(inp, '{{section}}') },
    ...(rowFields(S.sel ? (findWidget(S.sel)?.pi ?? S.page) : S.page).map(([k, v], i) => [...(i === 0 ? [{ title: 'This page’s row (repeated page)' }] : []), { label: k, hint: String(v).slice(0, 24), run: () => insertAt(inp, `{{row.${k}}}`) }]).flat()),
    ...((S.st.doc.variables || []).length ? [{ title: 'My variables' }, ...S.st.doc.variables.map((v) => ({ label: v.label || v.name, hint: `{{${v.name}}}`, run: () => insertAt(inp, `{{${v.name}}}`) }))] : []),
    { title: 'Report' },
    { label: 'Report title', hint: '{{title}}', run: () => insertAt(inp, '{{title}}') },
    { label: 'Period', hint: '{{period}}', run: () => insertAt(inp, '{{period}}') },
    { label: 'Comparison period', hint: '{{compare}}', run: () => insertAt(inp, '{{compare}}') },
    { label: 'Period start / end', hint: '{{start}}', run: () => insertAt(inp, '{{start}} – {{end}}') },
    { label: 'Today', hint: '{{today}}', run: () => insertAt(inp, '{{today}}') },
    ...doc.param_defs.map((p) => ({ label: `Parameter: ${p.label || p.name}`, hint: `{{${p.name}}}`, run: () => insertAt(inp, `{{${p.name}}}`) })),
    { sep: true },
    ...(doc.datasets.length ? [{ label: 'A value from the data…', icon: 'database', desc: 'Any field or calculated field, latest value in the period', run: () => dataTokenMenu(anchor, inp) }] : []),
    { label: 'An SQL query…', icon: 'code', desc: 'First cell of a query over your datasets', run: () => { const d0 = doc.datasets[0]; insertAt(inp, `{{sql: select count(*) from ${d0 ? sqlName(d0.name) : 'dataset'}${d0?.date_field ? ` where "${d0.date_field}" between :period_start and :period_end` : ''}}}`); } },
  ];
  menu(anchor, items);
}


function dataTokenMenu(anchor, inp) {
  const doc = S.st.doc;
  const items = [];
  for (const ds of doc.datasets) {
    items.push({ title: ds.name });
    const names = [...new Set([...(ds.fields || []).map((f) => f.name), ...(ds.calc_fields || []).map((c) => c.name)])];
    for (const f of names.slice(0, 40)) items.push({ label: f, hint: (ds.calc_fields || []).some((c) => c.name === f) ? 'calculated' : '', run: () => aggMenu(anchor, inp, ds.name, f) });
  }
  menu(anchor, items);
}

function aggMenu(anchor, inp, ds, field) {
  const aggs = [['last', 'Latest value'], ['first', 'Earliest value'], ['sum', 'Sum'], ['avg', 'Average'], ['min', 'Minimum'], ['max', 'Maximum'], ['count', 'Count'], ['distinct', 'Distinct count'], ['list', 'List of values']];
  menu(anchor, [{ title: `${ds}.${field}` }, ...aggs.map(([a, l]) => ({ label: l, run: () => insertAt(inp, `{{data:${ds}.${field}${a === 'last' ? '' : '|' + a}}}`) }))]);
}

function bandEditor(which) {
  const doc = S.st.doc;
  const band = doc.page[which];
  const res = S.st[which] || {};
  const set = (patch) => edit({ op: 'set_page_setup', patch: { [which]: patch } });
  const title = which === 'header' ? 'Header' : 'Footer';
  const items = [row(`Show ${title.toLowerCase()}`, toggle(`${which}.show`, band.show, (v) => set({ show: v })))];
  if (band.show) {
    items.push(
      row('Height (pt)', textInput(`${which}.h`, band.height, (v) => v != null && set({ height: Math.max(8, Math.min(160, v)) }), { type: 'number', min: 8, max: 160 })),
      row('Background', colorField(`${which}.fill`, band.fill || null, (c) => set({ fill: c || '' }))),
      row('Divider', (() => {
        const g = h(`<div class="m-row"></div>`);
        g.append(selectInput(`${which}.line`, band.line, [['none', 'None'], ['top', 'Above'], ['bottom', 'Below']], (v) => set({ line: v })),
          colorField(`${which}.lc`, band.line_color || null, (c) => set({ line_color: c || '' })));
        return g;
      })()),
      row('Section tabs', selectInput(`${which}.tabs`, band.tabs, [['none', 'None'], ['left', 'Left'], ['center', 'Centre'], ['right', 'Right'], ['full', 'Full width']], (v) => set({ tabs: v }))),
    );
    // how the current tab is marked: the theme's, so every page and template with the theme follows
    if (band.tabs && band.tabs !== 'none') {
      const ts = doc.theme.tabs || {};
      const tset = (patch) => edit({ op: 'set_theme', patch: { tabs: patch } });
      const g = h(`<div class="m-row"></div>`);
      g.append(selectInput(`${which}.tstyle`, ts.style || 'pill', [['pill', 'Pill'], ['box', 'Box'], ['underline', 'Underline'], ['text', 'Bold text']], (v) => tset({ style: v === 'pill' ? null : v })),
        colorField(`${which}.tfill`, ts.fill || null, (c) => tset({ fill: c || null })));
      items.push(row('Tab style', g), row('Tab capitals', toggle(`${which}.tupper`, !!ts.upper, (v) => tset({ upper: v || null }))));
    }
    for (const pos of ['left', 'center', 'right']) {
      if (band.tabs === pos || band.tabs === 'full') continue;
      const sl = band[pos] || {};
      const key = `${which}.${pos}`;
      const filled = !!(sl.text || sl.image);
      const det = h(`<details class="role slot${filled ? ' set' : ''}"><summary><span class="ro-name">${SLOT_NAMES[pos]}</span><span class="ro-meta">${esc(sl.image ? '🖼 ' : '')}${esc((sl.text || '').split('\n')[0] || (sl.image ? 'Image' : 'Empty'))}</span></summary><div class="slot-body"></div></details>`);
      if (BAND_OPEN.has(key)) det.open = true;
      det.addEventListener('toggle', () => (det.open ? BAND_OPEN.add(key) : BAND_OPEN.delete(key)));
      const body = det.querySelector('.slot-body');
      const ta = textInput(`${key}.text`, sl.text, (v) => set({ [pos]: { text: v } }), { area: true, rows: 2, placeholder: 'Text, placeholders, data…' });
      const ins = h(`<button class="btn sm">${icon('plus', 13)} Insert</button>`);
      ins.addEventListener('click', () => tokenMenu(ins, ta));
      const shown = res[pos]?.text;
      body.append(ta, h(`<div class="m-row slot-tools"></div>`));
      const tools = body.querySelector('.slot-tools');
      tools.append(ins);
      const img = h(`<button class="btn sm">${icon('image', 13)} ${sl.image ? 'Replace logo' : 'Logo…'}</button>`);
      img.addEventListener('click', async () => {
        const p = await call('pick_data_file', { kind: 'image' });
        if (!p) return;
        const src = await call('read_image', { path: p });
        const ratio = await new Promise((ok) => { const im = new Image(); im.onload = () => ok(im.naturalWidth / (im.naturalHeight || 1)); im.onerror = () => ok(3); im.src = src; });
        set({ [pos]: { image: src, image_ratio: ratio, image_h: sl.image_h || Math.max(10, band.height - 12) } });
      });
      tools.append(img);
      if (sl.image) {
        tools.append(textInput(`${key}.ih`, sl.image_h || '', (v) => set({ [pos]: { image_h: v || 0 } }), { type: 'number', min: 4, max: 160, placeholder: 'height' }));
        const rm = h(`<button class="tb-btn sm" title="Remove logo">${icon('trash', 13)}</button>`);
        rm.addEventListener('click', () => set({ [pos]: { image: '', image_ratio: 0 } }));
        tools.append(rm);
      }
      if (shown && shown !== sl.text) body.append(h(`<div class="hint slot-preview">Shows: ${esc(shown.replace(/\n/g, ' ⏎ '))}</div>`));
      if (res[pos]?.font) body.append(fontControl(`${key}.font`, sl.font, res[pos].font, (patch) => set({ [pos]: { font: patch === null ? Object.fromEntries(Object.keys(sl.font || {}).map((k) => [k, null])) : patch } }), { colorField }));
      items.push(det);
    }
    items.push(h(`<div class="hint">Use <b>Insert</b> for page numbers, the period, report parameters, any field or calculated field from your data (<code>{{data:Dataset.field|sum}}</code>) or an SQL query (<code>{{sql: select …}}</code>). Line breaks start a new line. The default font comes from <b>Page footer</b> in the theme fonts.</div>`));
  }
  return section(title, items);
}

/** The theme's settings, by category: colours, cards & charts, typography, language. */
function themeParts(doc) {
  const th = doc.theme;
  const tset = (k, v) => edit({ op: 'set_theme', patch: { [k]: v } });
  const col = (label, key, value, fallback, title) => {
    const r = h(`<div class="tc-row"><span class="tc-name">${esc(label)}</span></div>`);
    r.prepend(colorField(`t.${key}`, value, (c) => tset(key, c || fallback), { none: false, title, named: false }));
    r.append(hexInput(value, (c) => tset(key, c)));
    return r;
  };
  const colours = section('Theme colours',
    col('Page', 'page_bg', th.page_bg, '#ffffff', 'Page background'),
    col('Cards', 'card_bg', th.card_bg, '#fcfcfb', 'Card background'),
    col('Text', 'ink', th.ink, '#0b0b0b', 'Main text'),
    col('Muted text', 'muted', th.muted, '#6b6a65', 'Labels and axes'),
    col('Accent', 'accent', th.accent, '#2a78d6', 'Eyebrows, chips and highlights'),
    col('Positive', 'good', th.good, '#1f7a4d', 'Up is good'),
    col('Negative', 'bad', th.bad, '#c0392b', 'Down is bad'),
    col('Lines & grid', 'grid', th.grid, '#e4e3dd', 'Grid lines and rules'),
    col('Card outline', 'card_border', th.card_border, '#e4e3dd', 'Card outline'),
  );
  const pal = h(`<div class="pal-edit"></div>`);
  th.palette.forEach((c, i) => {
    const set = (v) => { const p = [...th.palette]; p[i] = v || c; tset('palette', p); };
    const cell = h(`<div class="pal-cell"></div>`);
    cell.append(colorField(`t.pal${i}`, c, set, { none: false, title: `Series ${i + 1}`, named: false }), hexInput(c, set));
    pal.append(cell);
  });
  const series = section('Series colours', pal, h(`<div class="hint">Charts colour series in this order. The default order is checked for colour-vision deficiencies; keep the first three distinct if you change them.</div>`));
  const cards = section('Cards & charts',
    row('Chart preset', seg('t.look', th.chart_look || 'clean', [['clean', 'Clean'], ['soft', 'Soft']], (v) => tset('chart_look', v))),
    h(`<div class="hint">${(th.chart_look || 'clean') === 'soft' ? '<b>Soft</b>: tinted, outlined and rounded marks, every value labelled, best and worst marked, the last value as a pill, donuts in shades of one colour.' : '<b>Clean</b>: solid marks with few labels and no highlights.'} The starting point for every chart in the theme; any setting in a design view or widget overrides it.</div>`),
    row('Cards', seg('t.card', th.card, [['outlined', 'Outlined'], ['filled', 'Filled'], ['shadow', 'Raised'], ['plain', 'Plain']], (v) => tset('card', v))),
    row('Corner radius', textInput('t.radius', th.radius, (v) => v != null && tset('radius', v), { type: 'number', min: 0, max: 20 })),
    row('Text size', seg('t.scale', th.font_scale, [[0.9, '90%'], [1, '100%'], [1.1, '110%'], [1.2, '120%']], (v) => tset('font_scale', +v))),
  );
  const lang = section('Language & currency',
    row('Language', selectInput('t.locale', th.locale || 'en-US', [['en-US', 'English'], ['ru-RU', 'Русский'], ['de-DE', 'Deutsch']], (v) => tset('locale', v))),
    row('Currency', textInput('t.cur', th.currency, (v) => tset('currency', v), { placeholder: '$' })),
  );

  const base = h(`<button class="fc-family"><span style="font-family:${esc(fontStack(th.font_family || 'Inter'))}">${esc(th.font_family || 'Inter')}</span>${icon('chevd', 12)}</button>`);
  base.addEventListener('click', (e) => pickFamily(e.currentTarget, th.font_family || 'Inter', (v) => tset('font_family', v)));
  const embedded = h(`<div class="emb-fonts"></div>`);
  for (const ef of doc.fonts || []) {
    const r = h(`<div class="emb-font"><span style="font-family:${esc(fontStack(ef.family))}">${esc(ef.family)}</span><span class="muted">${esc(ef.file)}</span></div>`);
    const x = h(`<button class="tb-btn sm" title="Remove">${icon('x', 13)}</button>`);
    x.addEventListener('click', () => edit({ op: 'remove_font', family: ef.family }));
    r.append(x);
    embedded.append(r);
  }
  const addF = h(`<button class="btn sm">${icon('upload', 13)} Add font file…</button>`);
  addF.addEventListener('click', async () => { const fam = await addFontFile(); if (fam) toast(`Added “${fam}”. Pick it in any font setting.`); });
  embedded.append(addF);
  familyList();
  const typography = [
    section('Typography',
      row('Base font', base),
      rolesEditor(Object.keys(S.st.fonts || {}), (role) => th.fonts?.[role], (role) => S.st.fonts?.[role], (role, patch) => edit({ op: 'set_theme', patch: { fonts: { [role]: patch } } }), { colorField }),
      h(`<div class="hint">Every role inherits the base font. Widgets can override any role under <b>Text</b>.</div>`)),
    section(['Font files', { icon: 'upload' }], embedded, h(`<div class="hint">Fonts added here travel with the ${isTheme() ? 'theme' : 'template'}, so pages measure, fit and export the same on any computer.</div>`)),
  ];
  return { colours: [colours, series], charts: [cards, chartDefaults()], typography, lang: [lang] };
}

// ---- document inspector

function docInspector() {
  const doc = S.st.doc;
  const user = userMode();
  const el = h(`<div class="insp"><div class="insp-head"><span class="ih-ic">${icon(isReport() ? 'page' : 'layout', 18)}</span><span class="ih-t"><b>${esc(doc.meta.title)}</b><span>${isReport() ? `Report${doc.meta.template_title ? ' · from “' + esc(doc.meta.template_title) + '”' : ''}` : 'Template'}</span></span></div></div>`);
  const meta = (k, v) => edit({ op: 'set_meta', patch: { [k]: v } });
  if (isTheme()) {
    el.querySelector('.ih-ic').innerHTML = icon('palette', 18);
    el.querySelector('.ih-t').innerHTML = `<b>${esc(doc.theme.name)}</b><span>Theme · select a design view to edit it</span>`;
    const t = themeParts(doc);
    return tabbed(el, 'theme', [
      ['colours', 'Colours', 'drop', () => t.colours],
      ['type', 'Type', 'type', () => t.typography],
      ['charts', 'Charts', 'chart-bar', () => t.charts],
      ['general', 'General', 'sliders', () => [section(['Theme', { icon: 'palette' }], row('Name', textInput('t.name', doc.theme.name, (v) => v && edit({ op: 'set_theme', patch: { name: v } }))),
        h(`<div class="hint">Saving adds the theme to your library. Templates load it from <b>Theme</b> in the top bar.</div>`)), ...t.lang]],
    ]);
  }
  const docSecs = [
    section(isReport() ? 'Report' : 'Template',
      row('Title', textInput('m.title', doc.meta.title, (v) => meta('title', v))),
      row('Subtitle', textInput('m.subtitle', doc.meta.subtitle, (v) => meta('subtitle', v))),
      row('Author', textInput('m.author', doc.meta.author, (v) => meta('author', v), { placeholder: S.st.user })),
      !user && row('Organization', textInput('m.org', doc.meta.organization, (v) => meta('organization', v))),
      !user && row('Description', textInput('m.desc', doc.meta.description, (v) => meta('description', v), { area: true, rows: 3 }), 'stack'),
    ),
  ];
  const arr = (k, v) => edit({ op: 'set_arrange', patch: { [k]: v } });
  const fitSec = () => section('Fit to page',
    row('Auto-arrange', toggle('a.auto', doc.arrange.auto, (v) => arr('auto', v))),
    row('Fill empty space', toggle('a.fill', doc.arrange.fill, (v) => arr('fill', v))),
    h(`<div class="hint">When you add, remove or resize a widget, Quire keeps the grid tidy: it closes gaps, makes room by shrinking flexible charts, and suggests fixes for anything it can't solve on its own. A widget you move or resize stays where you put it.</div>`));
  if (user) {
    const body = h(`<div class="insp-pane simple"></div>`);
    if (doc.policy.restyle && isReport()) body.append(themePicker());
    body.append(...docSecs);
    if (isReport()) body.append(fitSec());
    el.append(body);
    return el;
  }
  docSecs.push(fitSec());
  if (!user) {
    const f = doc.formats || {};
    docSecs.push(section(['Formats', { icon: 'numbers', closed: true }],
      ...formatRows(f, (patch) => edit({ op: 'set_formats', formats: { ...f, ...patch } }), 'fmt', 'Follow app settings'),
      formatSample(S.st.formats || f, doc.theme.locale, doc.theme.currency),
      Object.values(f).some(Boolean) && (() => {
        const b = h(`<button class="btn sm">${icon('undo', 13)} Follow app settings</button>`);
        b.addEventListener('click', () => edit({ op: 'set_formats', formats: {} }));
        return b;
      })(),
      h(`<div class="hint">Choices left on <b>Follow app settings</b> take your settings (⌘,), then the language (${esc(doc.theme.locale || 'en-US')}). Set one here to fix it for this document on any computer.</div>`),
    ));
  }

  const ps = (k, v) => edit({ op: 'set_page_setup', patch: { [k]: v } });
  const pg = doc.page;
  const pageSecs = [section('Page',
    row('Size', selectInput('p.size', pg.size, [['A4', 'A4'], ['A3', 'A3'], ['Letter', 'US Letter'], ['Slide', 'Slide 16:9']], (v) => ps('size', v))),
    row('Orientation', seg('p.land', pg.landscape ? '1' : '0', [['1', 'Landscape'], ['0', 'Portrait']], (v) => ps('landscape', v === '1'))),
    row('Grid', (() => {
      const g = h(`<div class="m-row"></div>`);
      g.append(textInput('p.cols', pg.cols, (v) => v && ps('cols', v), { type: 'number', min: 1, max: 48 }), h('<span class="x">×</span>'), textInput('p.rows', pg.rows, (v) => v && ps('rows', v), { type: 'number', min: 1, max: 64 }));
      return g;
    })()),
    row('Margin (pt)', textInput('p.margin', pg.margin, (v) => v != null && ps('margin', v), { type: 'number', min: 0, max: 120 })),
    row('Gutter (pt)', textInput('p.gap', pg.gap, (v) => v != null && ps('gap', v), { type: 'number', min: 0, max: 40 })),
  ), bandEditor('header'), bandEditor('footer')];

  const tsec = h(`<div class="btn-pair"></div>`);
  const ed = h(`<button class="btn sm" title="Edit this theme's colours, fonts and design views">${icon('palette', 13)} Theme studio</button>`);
  ed.addEventListener('click', () => api.themeStudio('current'));
  const ap = h(`<button class="btn sm">${icon('swap', 13)} Change theme…</button>`);
  ap.addEventListener('click', (e) => import('./app.js').then((m) => m.themeMenu(e.currentTarget)));
  tsec.append(ed, ap);
  const nviews = Object.values(doc.theme.styles || {}).reduce((a, l) => a + l.length, 0);
  const t = themeParts(doc);
  const themeSecs = [
    section('Theme', h(`<div class="theme-card"><span class="tcd-sw">${[doc.theme.page_bg, doc.theme.ink, doc.theme.accent, ...doc.theme.palette.slice(0, 4)].map((c) => `<i style="background:${esc(c)}"></i>`).join('')}</span><span><b>${esc(doc.theme.name || 'Unnamed')}</b><span>${nviews} design views</span></span></div>`), tsec),
    ...t.colours, ...t.charts, ...t.typography, ...t.lang,
  ];

  const pol = doc.policy;
  const pset = (k, v) => edit({ op: 'set_policy', patch: { [k]: v } });
  const access = [...(isReport() || isTheme() ? [] : [section(['Who can use this report', { icon: 'users' }], publishBox())]), section('What end users can do',
    row('Add from gallery', toggle('po.add', pol.add, (v) => pset('add', v))),
    row('Remove widgets', toggle('po.remove', pol.remove, (v) => pset('remove', v))),
    row('Move & resize', toggle('po.arrange', pol.arrange, (v) => pset('arrange', v))),
    row('Change look', toggle('po.restyle', pol.restyle, (v) => pset('restyle', v))),
    row('Edit text', toggle('po.edit_text', pol.edit_text, (v) => pset('edit_text', v))),
    row('Add comments', toggle('po.comment', pol.comment, (v) => pset('comment', v))),
    row('Add & remove pages', toggle('po.pages', pol.pages, (v) => pset('pages', v))),
    h(`<div class="hint">Lock individual widgets (right-click → Lock) to keep headers, logos or mandatory figures fixed.</div>`),
  )];
  // which filters report users see in the toolbar, and whether they may change them
  const ACCESS = [['change', 'Can change'], ['show', 'Show only'], ['hide', 'Hidden']];
  const filterRows = [
    row('Period', seg('po.period', pol.period || 'change', ACCESS, (v) => pset('period', v))),
    row('Comparison', seg('po.compare', pol.compare || 'change', ACCESS, (v) => pset('compare', v))),
    ...doc.param_defs.map((d, i) => row(d.label || d.name, seg(`po.param.${d.name}`, d.user || 'change', ACCESS, (v) => {
      const defs = clone(doc.param_defs);
      defs[i].user = v === 'change' ? '' : v;
      edit({ op: 'set_param_defs', defs });
    }))),
  ];
  if (!doc.param_defs.length) filterRows.push(h(`<div class="hint">No parameters yet. Add them in Data → Report parameters.</div>`));
  else {
    const b = h(`<button class="btn sm">${icon('sliders', 13)} Edit parameters…</button>`);
    b.addEventListener('click', paramEditor);
    filterRows.push(b);
  }
  filterRows.push(h(`<div class="hint"><b>Show only</b> keeps the value visible in the report’s toolbar but fixed; <b>Hidden</b> removes it. Fixed values are the template’s current ones.</div>`));
  access.push(section(['Filters for report users', { icon: 'sliders' }], filterRows));
  return tabbed(el, 'doc', [
    ['doc', 'Document', 'page', () => docSecs],
    ['page', 'Page', 'layout', () => pageSecs],
    ['theme', 'Theme', 'palette', () => themeSecs],
    ['access', 'Access', 'shield', () => access],
  ]);
}

// ---------------------------------------------------------------- publishing

/**
 * The template's place in the Reports area: published or a draft, and who may
 * use it — everyone with the Reports role, or chosen groups and people.
 * Before the first publish the choice waits and goes along with it.
 */
/**
 * Publishing in one place: what it does, who gets the report, and the button. A template is the
 * designer's working copy; publishing puts a copy of it in the Reports area for the people chosen.
 * Changes reach them when it is published again.
 */
export function publishDialog() {
  const doc = S.st?.doc;
  if (!doc || doc.kind !== 'template') return;
  const body = h(`<div class="modal-body pub-dlg">
    <div class="pub-how">
      <div><b>${icon('layout', 14)} Template</b><span>Your working copy. Change it as much as you like: report users don't see it.</span></div>
      <span class="pub-arrow">${icon('chevr', 16)}</span>
      <div><b>${icon('upload', 14)} Publish</b><span>Puts a copy in the Reports area for the people you choose below.</span></div>
      <span class="pub-arrow">${icon('chevr', 16)}</span>
      <div><b>${icon('users', 14)} Report users</b><span>Make reports from it for their clients. To change what they get, edit the template and publish again.</span></div>
    </div>
  </div>`);
  body.append(publishBox());
  modal({ title: `Publish “${doc.meta.title}”`, iconName: 'upload', wide: true, body, buttons: [{ label: 'Done', value: true, kind: 'primary' }] }).then(() => render());
}

function publishBox() {
  const box = h(`<div class="pub-box"><div class="hint">Loading…</div></div>`);
  const doc = S.st.doc;
  let pending = { everyone: true, groups: [], users: [] };
  (async () => {
    let g, list;
    try {
      [g, list] = await Promise.all([call('auth', { cmd: 'groups' }, { quiet: true }), call('auth', { cmd: 'reports' }, { quiet: true })]);
    } catch (e) { box.innerHTML = `<div class="hint">${esc(String(e))}</div>`; return; }
    let rep = list.find((r) => r.id === doc.meta.report_id) || null;
    const people = g.people.filter((p) => p.user);
    const draw = () => {
      const a = rep ? rep.audience : pending;
      const reach = rep ? rep.reach : people.filter((p) => a.everyone || a.users.includes(p.login) || g.groups.some((x) => a.groups.includes(x.id) && x.members.includes(p.login))).map((p) => p.login);
      const nameOf = (l) => g.people.find((p) => p.login === l)?.name || l;
      const state = !rep ? ['draft', 'Not published yet', 'Report users don’t see it. Choose who may use it, then publish.']
        : rep.published ? ['live', `Published · version ${rep.version}`, `${rep.by ? `by ${rep.by}` : ''}${rep.at ? ` · ${rep.at}` : ''}. Changes to the pages reach report users when you publish again.`]
          : ['off', 'Unpublished (a draft)', 'Kept with its audience, but nobody sees it in Reports.'];
      box.innerHTML = `<div class="pub-state ${state[0]}"><span class="dot"></span><div><b>${esc(state[1])}</b><span>${esc(state[2])}</span></div></div>
        <div class="seg pub-scope"><button data-v="all" class="${a.everyone ? 'on' : ''}">Everyone with Reports</button><button data-v="some" class="${a.everyone ? '' : 'on'}">Chosen groups & people</button></div>
        ${a.everyone ? '' : `<div class="pub-sub">Groups</div>
          <div class="pub-list">${g.groups.map((x) => `<label class="pub-item"><input type="checkbox" data-k="groups" value="${esc(x.id)}"${a.groups.includes(x.id) ? ' checked' : ''}><span class="pub-ic">${icon('users', 13)}</span><span class="pub-t"><b>${esc(x.name)}</b><span>${x.members.length} member${x.members.length === 1 ? '' : 's'}${x.description ? ' · ' + esc(x.description) : ''}</span></span></label>`).join('') || '<div class="hint">No groups yet — an administrator makes them in Admin → Groups.</div>'}</div>
          <div class="pub-sub">People</div>
          <div class="pub-list">${people.map((p) => `<label class="pub-item"><input type="checkbox" data-k="users" value="${esc(p.login)}"${a.users.includes(p.login) ? ' checked' : ''}><span class="pub-av">${esc((p.name || p.login).split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase())}</span><span class="pub-t"><b>${esc(p.name || p.login)}</b><span>${esc(p.login)}</span></span></label>`).join('') || '<div class="hint">No one has the Reports role yet.</div>'}</div>`}
        <div class="pub-reach">${icon('users', 13)}<span>${reach.length ? `Reaches <b>${reach.length}</b> ${reach.length === 1 ? 'person' : 'people'}: ${reach.slice(0, 6).map((l) => esc(nameOf(l))).join(', ')}${reach.length > 6 ? '…' : ''}` : 'Reaches nobody yet.'}</span></div>
        <div class="pub-acts">
          <button class="btn sm primary" data-a="publish">${icon('upload', 13)} ${rep ? 'Publish changes' : 'Publish to Reports'}</button>
          ${rep ? `<button class="btn sm" data-a="toggle">${rep.published ? 'Unpublish' : 'Publish again'}</button>` : ''}
        </div>
        <div class="hint">Designers and administrators always see it. Report users also see only the clients and products in their own access.</div>`;
      const setA = async (next) => {
        if (!rep) { pending = next; draw(); return; }
        try { const r = await call('auth', { cmd: 'set_audience', args: { id: rep.id, audience: next } }); rep = { ...r.report, reach: r.reach }; draw(); } catch (e) { toast(String(e), 'error'); }
      };
      box.querySelectorAll('.pub-scope button').forEach((b) => b.addEventListener('click', () => setA({ ...a, everyone: b.dataset.v === 'all' })));
      box.querySelectorAll('input[data-k]').forEach((x) => x.addEventListener('change', () => {
        const k = x.dataset.k;
        setA({ ...a, [k]: x.checked ? [...new Set([...a[k], x.value])] : a[k].filter((v) => v !== x.value) });
      }));
      box.querySelector('[data-a="publish"]').addEventListener('click', async () => {
        try {
          const r = await call('auth', { cmd: 'publish', args: rep ? {} : { audience: pending } });
          rep = { ...r.report, reach: r.reach };
          setState(r.state);
          toast(`Published “${r.report.title}” — version ${r.report.version}. Report users get it now; the template is saved with it.`, 'info', 4200);
        } catch (e) { toast(String(e), 'error', 3600); }
      });
      box.querySelector('[data-a="toggle"]')?.addEventListener('click', async () => {
        const r = await call('auth', { cmd: 'set_audience', args: { id: rep.id, published: !rep.published } });
        rep = { ...r.report, reach: r.reach };
        draw();
        toast(rep.published ? 'Back in the Reports area.' : 'Unpublished — report users no longer see it.', 'info');
      });
    };
    draw();
  })();
  return box;
}

// ---------------------------------------------------------------- table design

const tpatch = (w, patch) => upd(w.id, { opts: { table: patch } });

/**
 * A compact colour control: swatch button → palette popover. The theme's colours are offered by name
 * (Accent, Text, Series 2…) and kept as `$name`, so the colour follows the theme; `named: false` for the
 * theme's own colour definitions.
 */
export function colorField(key, value, onChange, { none = true, title = 'Colour', clear = false, named = true, auto = null } = {}) {
  // `clear`: "None" means truly transparent, and "Auto" goes back to the default
  // `auto`: the colour drawn when none is set, shown as it is (not as "none")
  const th0 = S.st.doc.theme;
  const isAuto = !value && !!auto;
  const v0 = isAuto ? auto : value;
  const shown = themeColor(th0, v0);
  const see = v0 && v0 !== 'transparent';
  const nm = colourName(value);
  const tip = isAuto ? ` · Auto (${auto === 'transparent' ? 'transparent' : esc(shown)})` : value === 'transparent' ? ' · transparent' : nm ? ` · ${esc(nm)} (theme colour)` : '';
  const b = h(`<button class="color-field${nm ? ' named' : ''}${isAuto ? ' auto' : ''}" data-key="${key}" title="${esc(title)}${tip}"><span class="cf-sw${see ? '' : v0 === 'transparent' ? ' clear' : ' none'}" style="${see ? `background:${shown}` : ''}"></span></button>`);
  b.addEventListener('click', (e) => {
    e.stopPropagation();
    const th = S.st.doc.theme;
    const el = h(`<div class="menu cf-pop"></div>`);
    const pick = (c) => { closeLayer(); onChange(c); };
    const sw = (v, c, label) => {
      const x = h(`<button class="sw${v === value ? ' on' : ''}" style="background:${c}" title="${esc(label ? `${label} · ${c}` : c)}"></button>`);
      x.addEventListener('click', () => pick(v));
      return x;
    };
    const title2 = (t) => h(`<div class="menu-title" style="padding-left:2px">${t}</div>`);
    if (named) {
      // the theme's colours by name: picking one keeps the name, so it follows the theme
      const { main, series, more } = themeColours(th);
      el.append(title2('Theme colours'));
      const list = h(`<div class="cf-named"></div>`);
      for (const [v, n, c] of main) {
        const x = h(`<button class="cf-nm${v === value ? ' on' : ''}" title="${esc(c)}"><span class="sw" style="background:${c}"></span><span>${esc(n)}</span></button>`);
        x.addEventListener('click', () => pick(v));
        list.append(x);
      }
      el.append(list);
      el.append(title2('Series'));
      const row = h(`<div class="swatches-row"></div>`);
      series.forEach(([v, n, c]) => row.append(sw(v, c, n)));
      el.append(row);
      if (more.length) {
        const det = h(`<details class="cf-more"${more.some(([v]) => v === value) ? ' open' : ''}><summary>More theme colours</summary></details>`);
        const ml = h(`<div class="cf-named"></div>`);
        for (const [v, n, c] of more) {
          const x = h(`<button class="cf-nm${v === value ? ' on' : ''}" title="${esc(c)}"><span class="sw" style="background:${c}"></span><span>${esc(n)}</span></button>`);
          x.addEventListener('click', () => pick(v));
          ml.append(x);
        }
        det.append(ml);
        el.append(det);
      }
    } else {
      el.append(title2('Series'));
      const row = h(`<div class="swatches-row"></div>`);
      (th.palette || []).forEach((c) => row.append(sw(c, c)));
      el.append(row);
    }
    if (named) {
      // custom colours: transparent, then the colours this template already uses (most used first), so one
      // picked once is a click away; they stay as they are whatever the theme does
      el.append(title2('Custom colours'));
      const own = h(`<div class="swatches-row"></div>`);
      const tr = h(`<button class="sw transparent${value === 'transparent' ? ' on' : ''}" title="Transparent: what is behind shows through"></button>`);
      tr.addEventListener('click', () => pick('transparent'));
      own.append(tr);
      for (const c of usedColours()) own.append(sw(c, c));
      el.append(own);
      if (own.children.length === 1) el.append(h(`<div class="hint cf-hint">Colours you pick below show here, for this template.</div>`));
    } else {
      el.append(title2('Soft'));
      const soft = h(`<div class="swatches-row"></div>`);
      for (const c of ['#e3f4e7', '#fde6e4', '#fff1d6', '#e1ecfb', '#ece9fb', '#eceae4', '#f4f3ef', '#ffffff']) soft.append(sw(c, c));
      el.append(soft);
    }
    // a colour of its own, picked inside the popover (the system colour window would close it)
    el.append(title2(named ? 'Pick a colour' : 'Custom'));
    el.append(colorPicker(see && /^#[0-9a-f]{6}$/i.test(shown || '') ? shown : '#2a78d6', (c) => pick(c)));
    if (clear || none) {
      const foot = h(`<div class="cf-foot">${clear ? '<button class="btn sm" data-v="auto">Auto</button><button class="btn sm" data-v="clear">None</button>' : '<button class="btn sm" data-v="none">None</button>'}</div>`);
      foot.querySelectorAll('.btn').forEach((x) => x.addEventListener('click', () => pick(x.dataset.v === 'clear' ? 'transparent' : null)));
      el.append(foot);
    }
    popover(b, el);
  });
  return b;
}

/** The colours (#rrggbb) the open template's widgets and page bands use, most used first (up to 16). */
const USED = new WeakMap();
function usedColours() {
  const st = S.st;
  if (USED.has(st)) return USED.get(st);
  const n = new Map();
  const walk = (v) => {
    if (typeof v === 'string') { if (/^#[0-9a-f]{6}$/i.test(v)) n.set(v.toLowerCase(), (n.get(v.toLowerCase()) || 0) + 1); }
    else if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') Object.values(v).forEach(walk);
  };
  for (const p of st.doc.pages) for (const w of p.widgets) { walk(w.opts); walk(w.style); }
  walk(st.doc.page?.header); walk(st.doc.page?.footer);
  const out = [...n.entries()].sort((a, b) => b[1] - a[1]).slice(0, 16).map(([c]) => c);
  USED.set(st, out);
  return out;
}

function tableColumns(f) {
  const out = [];
  const seen = new Set();
  for (const c of displayColumns(f.w, f.vm.data)) {
    const pivoted = c.base && c.base !== c.key;
    const key = pivoted ? c.base : c.key;
    if (seen.has(key)) continue;
    seen.add(key);
    const m = pivoted ? (f.w.binding?.measures || []).find((x) => x.id === c.base) : null;
    out.push({ key, label: pivoted ? `${m?.label || cap(m?.field || 'Value')} · each column` : c.label, role: c.role, format: c.format, tree: c.tree });
  }
  return out;
}

/** Moves a table column to a new index in the display order. */
function moveColumn(f, key, to) {
  const keys = tableColumns(f).map((c) => c.key);
  const from = keys.indexOf(key);
  if (from < 0) return;
  to = Math.max(0, Math.min(keys.length - 1, to));
  if (to === from) return;
  keys.splice(to, 0, ...keys.splice(from, 1));
  tpatch(f.w, { order: keys });
}

/** A fresh, unused measure id. */
function nextMeasureId(ms) {
  let n = ms.length;
  while (ms.some((m) => m.id === `m${n}`)) n++;
  return `m${n}`;
}

const r1 = (v) => (v ? Math.round(v * 10) / 10 : 'auto');

function tableDesign(f) {
  const w = f.w;
  // values as drawn: the design view's, under the widget's own
  const ts = drawn(f).opts?.table || {};
  const hd = ts.header || {};
  const pages = [], headerRow = [], lines = [], totals = [], items = [];
  pages.push(row('Continue on next pages', toggle('t.paginate', ts.paginate !== false, (v) => tpatch(w, { paginate: v }))));
  pages.push(h(`<div class="hint">${ts.paginate !== false ? "Rows that don't fit carry on to new pages, with the header repeated and the totals at the end. Pages are added and removed as the data changes." : 'Off: rows that don’t fit are cut, and the page fit advisor suggests fixes.'}</div>`));
  const head = h(`<div class="m-row"></div>`);
  head.append(colorField('t.hfill', hd.fill, (c) => tpatch(w, { header: { fill: c } }), { title: 'Header fill' }));
  head.append(colorField('t.hcolor', hd.color, (c) => tpatch(w, { header: { color: c } }), { title: 'Header text' }));
  head.append(toggle('t.hupper', hd.upper ?? !!f.vm.fonts?.table_head?.upper, (v) => tpatch(w, { header: { upper: v } }), 'CAPS'));
  headerRow.push(row('Colours', head));
  const zebra = h(`<div class="m-row"></div>`);
  zebra.append(toggle('t.zebra', !!ts.zebra, (v) => tpatch(w, { zebra: v })));
  if (ts.zebra) zebra.append(colorField('t.zfill', ts.zebraFill || '#f4f3ef', (c) => tpatch(w, { zebraFill: c }), { none: false, title: 'Stripe colour' }));
  lines.push(row('Striped rows', zebra));
  headerRow.unshift(row('Show', toggle('t.hidehead', !ts.hideHead, (v) => tpatch(w, { hideHead: v ? null : true }))));
  lines.unshift(row('Lines', seg('t.lines', ts.lines || 'rows', [['rows', 'Rows'], ['grid', 'Grid'], ['header', 'Header'], ['none', 'None']], (v) => tpatch(w, { lines: v }))));
  const lc = h(`<div class="m-row"></div>`);
  lc.append(colorField('t.lc', ts.lineColor || null, (c) => tpatch(w, { lineColor: c }), { title: 'Row lines' }), colorField('t.hlc', ts.headLineColor || null, (c) => tpatch(w, { headLineColor: c }), { title: 'Header line' }),
    textInput('t.hlw', ts.headLineWidth ?? '', (v) => tpatch(w, { headLineWidth: v }), { type: 'number', min: 0, max: 4, placeholder: '0.8' }));
  lines.push(row('Line colours', lc));
  // row height: presets from the cell font, or exact points
  const cell = f.vm.fonts?.table_cell?.size || 8;
  const presets = [['compact', 'Compact', 1.87], ['normal', 'Normal', 2.125], ['roomy', 'Roomy', 2.6]].map(([k, l, x]) => [k, l, Math.round(cell * x * 2) / 2]);
  const rh = f.vm.table?.row_h || 0;
  const curP = presets.find((p) => Math.abs(p[2] - rh) < 0.3)?.[0] || 'custom';
  lines.push(row('Row height', seg('t.rowp', curP, [...presets.map(([k, l]) => [k, l]), ['custom', 'Custom']], (v) => {
    const p = presets.find((x) => x[0] === v);
    if (p) tpatch(w, { row_h: p[2], head_h: Math.max(p[2], Math.round((f.vm.fonts?.table_head?.size || cell) * (p[2] / cell) * 2) / 2) + 3 });
  })));
  // exact heights; never below what a line of the cells' or the header's text needs
  const minRow = Math.ceil(cell * 1.3), minHead = Math.ceil((f.vm.fonts?.table_head?.size || cell) * 1.3);
  const clampH = (v, lo) => (v == null ? null : Math.max(lo, Math.min(80, v)));
  lines.push(row('Rows (pt)', textInput('t.rowh', ts.row_h ?? '', (v) => tpatch(w, { row_h: clampH(v, minRow) }), { type: 'number', min: minRow, max: 80, placeholder: `auto · ${r1(f.vm.table?.row_h)}` })));
  lines.push(row('Cell padding (pt)', textInput('t.cellpad', ts.cellPad ?? '', (v) => tpatch(w, { cellPad: v == null ? null : Math.max(0, Math.min(20, v)) }), { type: 'number', min: 0, max: 20, step: 0.5, placeholder: '5' })));
  lines.push(row('Header row (pt)', textInput('t.headh', ts.head_h ?? '', (v) => tpatch(w, { head_h: clampH(v, minHead) }), { type: 'number', min: minHead, max: 80, placeholder: `auto · ${r1(f.vm.table?.head_h)}` })));
  if (ts.layout !== 'facts') lines.push(row('Fill height', toggle('t.fill', !!ts.fill, (v) => tpatch(w, { fill: v }), 'Stretch rows to the bottom')));
  // totals on or off, then how they look: all in one place
  totals.push(row('Totals row', toggle('o.totals', drawn(f).opts?.totals !== false, (v) => upd(w.id, { opts: { totals: v } }))));
  if (drawn(f).opts?.totals !== false) {
    const tot = h(`<div class="m-row"></div>`);
    tot.append(colorField('t.tfill', ts.totalFill || null, (c) => tpatch(w, { totalFill: c }), { title: 'Totals fill' }), colorField('t.tcolor', ts.totalColor || null, (c) => tpatch(w, { totalColor: c }), { title: 'Totals text' }),
      selectInput('t.tline', ts.totalLine || 'single', [['single', 'Line above'], ['thick', 'Thick line'], ['double', 'Double underline'], ['none', 'No line']], (v) => tpatch(w, { totalLine: v })));
    totals.push(row('Totals look', tot));
  }
  const b = w.binding || {};
  if ((b.dims || []).length >= 2 && b.mode !== 'rows') {
    totals.push(row('Subtotals', toggle('t.sub', !!ts.subtotals, (v) => tpatch(w, { subtotals: v }))));
    totals.push(row('Only group totals', toggle('t.collapse', !!ts.collapse, (v) => tpatch(w, { collapse: v }))));
    if (ts.subtotals || ts.collapse) {
      const gs = h(`<div class="m-row wrap"></div>`);
      gs.append(toggle('t.gup', !!ts.groupUpper, (v) => tpatch(w, { groupUpper: v || null }), 'CAPS'), toggle('t.gcount', !!ts.groupCount, (v) => tpatch(w, { groupCount: v || null }), 'Count'),
        toggle('t.gvals', ts.groupValues !== false, (v) => tpatch(w, { groupValues: v ? null : false }), 'Totals'), toggle('t.gind', ts.indent !== false, (v) => tpatch(w, { indent: v ? null : false }), 'Indent'));
      totals.push(row('Group rows show', gs));
      const gl = h(`<div class="m-row"></div>`);
      gl.append(toggle('t.gline', !!ts.groupLine, (v) => tpatch(w, { groupLine: v ? S.st.doc.theme.ink : null })), ...(ts.groupLine ? [colorField('t.glc', ts.groupLine === true ? null : ts.groupLine, (c) => tpatch(w, { groupLine: c || true }))] : []),
        toggle('t.gnofill', ts.groupFill === 'none', (v) => tpatch(w, { groupFill: v ? 'none' : null }), 'No fill'));
      totals.push(row('Line under groups', gl));
      const g = h(`<div class="m-row"></div>`);
      g.append(colorField('t.gfill', ts.groupFill || '#f1f0eb', (c) => tpatch(w, { groupFill: c }), { none: false, title: 'Group row fill' }), colorField('t.gcolor', ts.groupColor || null, (c) => tpatch(w, { groupColor: c }), { title: 'Group row text' }));
      totals.push(row('Group rows', g));
    }
  }
  if (b.pivot) totals.push(row('Row totals', toggle('t.rowtot', ts.rowTotals !== false, (v) => tpatch(w, { rowTotals: v }))));

  if (w.opts?.totals !== false) totals.push(row('Totals label', textInput('t.tlabel', ts.totalLabel || '', (v) => tpatch(w, { totalLabel: v || null }), { placeholder: 'Total' })));
  if (!userMode()) pages.push(row('PowerPoint', selectInput('t.pptx', ts.pptx || 'auto', [['auto', 'Automatic'], ['native', 'Editable table'], ['picture', 'Exact picture']], (v) => tpatch(w, { pptx: v }))));

  const list = h(`<div class="col-list"></div>`);
  const cols = tableColumns(f);
  // columns come from the binding: values (measures) and grouping fields (dims)
  const bnd = w.binding || {};
  const setBinding = (patch) => upd(w.id, { binding: { ...bnd, ...patch } });
  const isMeasure = (key) => (bnd.measures || []).some((m) => m.id === key);
  const isDim = (key) => (bnd.dims || []).includes(key);
  const removable = (key) => !isTheme() && cols.length > 1 && (isMeasure(key) || (isDim(key) && (bnd.dims || []).length > 1));
  const removeCol = (key) => {
    if (isMeasure(key)) setBinding({ measures: bnd.measures.filter((m) => m.id !== key) });
    else if (isDim(key)) setBinding({ dims: bnd.dims.filter((d) => d !== key) });
  };
  if (!cols.length) list.append(h(`<div class="hint">Columns appear once the table has data.</div>`));
  for (const c of cols) {
    const cs = ts.cols?.[c.key] || {};
    const badges = [cs.pill?.on && 'pills', cs.bar?.on && 'bars', cs.scale?.on && 'scale', cs.arrows && 'arrows', (cs.rules || []).length && `${cs.rules.length} rule${cs.rules.length > 1 ? 's' : ''}`].filter(Boolean);
    const r = h(`<div class="col-item${cs.hidden ? ' hidden' : ''}" draggable="true" data-key="${esc(c.key)}" title="Drag to reorder">
      <span class="ord"><button class="tb-btn sm up" title="Move left"${cols[0].key === c.key ? ' disabled' : ''}>${icon('chevl', 11)}</button><button class="tb-btn sm down" title="Move right"${cols[cols.length - 1].key === c.key ? ' disabled' : ''}>${icon('chevr', 11)}</button></span>
      <button class="tb-btn sm eye" title="${cs.hidden ? 'Show' : 'Hide'} column">${icon(cs.hidden ? 'minus' : 'eye', 14)}</button>
      <span class="ci-body"><b>${esc(cs.label || c.label)}</b><span>${badges.length ? esc(badges.join(' · ')) : c.role === 'measure' && (f.w.binding?.measures || []).find((m) => m.id === c.key)?.agg !== 'text' ? 'number' : 'text'}</span></span>
      <label class="ci-w" title="Width as a % of the table — empty is automatic. Or drag the column borders on the page."><input type="number" min="3" max="90" step="1" placeholder="auto" value="${cs.width || ''}">%</label>
      <button class="btn sm" title="Format, bars, icons and rules">Edit ${icon('chevr', 12)}</button>
      ${removable(c.key) ? `<button class="tb-btn sm del" title="Remove this column">${icon('trash', 13)}</button>` : ''}
    </div>`);
    r.querySelector('.del')?.addEventListener('click', (e) => { e.stopPropagation(); removeCol(c.key); toast(`“${cs.label || c.label}” removed. Undo with ⌘Z.`, 'info'); });
    r.querySelector('.eye').addEventListener('click', () => tpatch(w, { cols: { [c.key]: { hidden: !cs.hidden } } }));
    const wi = r.querySelector('.ci-w input');
    ['pointerdown', 'mousedown', 'click'].forEach((t) => wi.addEventListener(t, (e) => e.stopPropagation()));
    r.querySelector('.ci-w').addEventListener('dragstart', (e) => e.preventDefault());
    wi.addEventListener('change', () => { const v = parseFloat(wi.value); tpatch(w, { cols: { [c.key]: { width: v > 0 ? Math.max(3, Math.min(90, v)) : null } } }); });
    const idx = cols.findIndex((x) => x.key === c.key);
    r.querySelector('.up').addEventListener('click', () => moveColumn(f, c.key, idx - 1));
    r.querySelector('.down').addEventListener('click', () => moveColumn(f, c.key, idx + 1));
    r.addEventListener('dragstart', (e) => { e.dataTransfer.setData('text/x-quire-col', c.key); e.dataTransfer.effectAllowed = 'move'; r.classList.add('dragging'); });
    r.addEventListener('dragend', () => r.classList.remove('dragging'));
    r.addEventListener('dragover', (e) => {
      if (!e.dataTransfer.types.includes('text/x-quire-col')) return;
      e.preventDefault();
      const below = e.offsetY > r.offsetHeight / 2;
      r.classList.toggle('drop-below', below);
      r.classList.toggle('drop-above', !below);
    });
    r.addEventListener('dragleave', () => r.classList.remove('drop-above', 'drop-below'));
    r.addEventListener('drop', (e) => {
      e.preventDefault();
      const below = r.classList.contains('drop-below');
      r.classList.remove('drop-above', 'drop-below');
      const key = e.dataTransfer.getData('text/x-quire-col');
      if (!key || key === c.key) return;
      const from = cols.findIndex((x) => x.key === key);
      let to = idx + (below ? 1 : 0);
      if (from < to) to -= 1;
      moveColumn(f, key, to);
    });
    r.querySelector('.btn').addEventListener('click', () => openColumn(f, c.key));
    r.querySelector('.ci-body').addEventListener('click', () => openColumn(f, c.key));
    list.append(r);
  }
  items.push(list);
  if (cols.some((c) => ts.cols?.[c.key]?.width && !ts.cols[c.key].wrap)) {
    const auto = h(`<button class="btn sm">${icon('undo', 13)} Automatic widths</button>`);
    auto.addEventListener('click', () => tpatch(w, { cols: Object.fromEntries(cols.map((c) => [c.key, { width: null }])) }));
    items.push(auto);
  }
  if (!isTheme() && bnd.dataset) {
    // add a column from the table's data: a number as a total, text as it is
    const add = h(`<button class="btn sm">${icon('plus', 13)} Add column</button>`);
    add.addEventListener('click', (e) => {
      const ms = bnd.measures || [];
      const used = new Set([...ms.map((m) => m.field), ...(bnd.dims || [])]);
      const fields = fieldsOf(bnd.dataset);
      const item = (x) => ({ label: x.name, desc: x.kind === 'number' ? 'Number' + (used.has(x.name) ? ' · already shown' : '') : 'Text', run: () => {
        const id = nextMeasureId(ms);
        // a field already shown keeps how it is calculated (e.g. a return: max, percent)
        const like = ms.find((m) => m.field === x.name);
        const agg = like?.agg || (x.kind === 'number' ? 'sum' : 'text');
        setBinding({ measures: [...ms, { id, field: x.name, agg, label: cap(x.name), format: like?.format || (x.kind === 'number' ? 'number' : '') }] });
        toast(`“${cap(x.name)}” added at the end. Click it to format.`, 'info');
      } });
      const nums = fields.filter((x) => x.kind === 'number'), texts = fields.filter((x) => x.kind !== 'number');
      menu(e.currentTarget, [
        ...(nums.length ? [{ title: 'Numbers' }, ...nums.map(item)] : []),
        ...(texts.length ? [{ title: 'Text' }, ...texts.filter((x) => !(bnd.dims || []).includes(x.name)).map(item)] : []),
      ]);
    });
    items.push(add);
  }
  items.push(h(`<div class="hint">Drag to reorder. Click a column for its format, bars, icons and highlight rules. The bin removes a column; the eye only hides it. <b>Width</b> is a % of the table (empty = automatic); on the page, drag the borders between a selected table’s columns.</div>`));
  // a table that is a ranked bar list can be the bar chart it looks like
  const asList = !isTheme() && !userMode() && tableToList(drawn(f), f.vm?.data);
  let listNote = null;
  if (asList) {
    listNote = h(`<div class="note">${icon('chart-hbar', 15)}<div><span>This table is a ranked bar list. As a bar chart (List layout) it keeps this look and gets the chart's settings.</span><div class="note-acts"><button class="btn sm">Show as a bar chart</button></div></div></div>`);
    listNote.querySelector('.btn').addEventListener('click', () => { upd(w.id, asList); toast('Now a bar chart (List layout) with the same look. Undo with ⌘Z.', 'info'); });
  }
  // a design view keeps no per-column styles (columns belong to each table's data)
  // layout: rows, or a grid of facts (name above, value below) for key → value tables
  const layoutItems = [row('Show as', seg('t.layout', ts.layout === 'facts' ? 'facts' : 'rows', [['rows', 'Rows'], ['facts', 'Facts']], (v) => tpatch(w, { layout: v === 'facts' ? 'facts' : null })))];
  if (ts.layout === 'facts') {
    layoutItems.push(row('Across', seg('t.factCols', String(ts.factCols || 2), [['1', '1'], ['2', '2'], ['3', '3'], ['4', '4']], (v) => tpatch(w, { factCols: +v }))));
    layoutItems.push(row('Value size', textInput('t.factSize', String(ts.factSize ?? 1.18), (v) => tpatch(w, { factSize: +v || null }), { type: 'number', min: 0.8, max: 2.5 })));
    layoutItems.push(h(`<div class="hint">Each row becomes a fact: its name small on top, its first value large below — for terms, “about” and other key → value lists.</div>`));
  }
  return [listNote, section('Layout', layoutItems), !isTheme() && section('Columns', items), section('Header row', headerRow), section('Lines & rows', lines), totals.length ? section('Totals & groups', totals) : null, !isTheme() && extremesSection(f, ts), section(['Rows & pages', { closed: true }], pages)].filter(Boolean);
}

/** Controls side by side in one row. */
function pair2(...xs) { const r = h(`<div class="m-row"></div>`); r.append(...xs.filter(Boolean)); return r; }

/**
 * Best and worst rows: one or more rankings, each by its own value (shown or
 * hidden). A row hit by several shows each ranking's icon; the first ranking
 * that highlights rows decides the row's fill.
 */
function extremesSection(f, ts) {
  const w = f.w;
  const th = S.st.doc.theme;
  const nums = (f.vm.data?.columns || []).filter((c) => c.role === 'measure');
  if (!nums.length) return null;
  const list = [].concat(ts.extremes || []).filter(Boolean).map((x) => ({ ...x }));
  const save = (next) => tpatch(w, { extremes: next.length ? next : null });
  const colOf = (key) => nums.find((c) => c.key === key);
  // icons say which value a mark is about: arrows for returns, coins for amounts
  const defaults = (key) => (colOf(key)?.format === 'percent' ? ['trend-up', 'trend-down'] : ['coins', 'coins']);
  const fresh = (key) => { const [bi, wi] = defaults(key); return { on: true, by: key, show: 'icon', best: { n: 1, icon: bi }, worst: { n: 1, icon: wi } }; };
  const items = [];
  if (!list.length) {
    const by = (nums.find((c) => c.format === 'percent') || nums[nums.length - 1]).key;
    const b = h(`<button class="btn sm">${icon('star', 13)} Mark best & worst rows</button>`);
    b.addEventListener('click', () => save([fresh(by)]));
    items.push(b, h(`<div class="hint">Put an icon after the best and worst rows’ names, highlight the rows, or both, ranked by any value.</div>`));
  }
  list.forEach((X, xi) => {
    const set = (patch) => { const n = list.map((x) => ({ ...x })); n[xi] = { ...n[xi], ...patch }; save(n); };
    const card = h(`<div class="rank-card${X.on ? '' : ' off'}"><div class="rk-head"><span class="rk-n">${xi + 1}</span></div><div class="rk-body"></div></div>`);
    const head = card.querySelector('.rk-head');
    head.append(selectInput(`t.ext${xi}.by`, X.by, nums.map((c) => [c.key, `${c.label?.trim() || c.key}${ts.cols?.[c.key]?.hidden ? ' (hidden)' : ''}`]), (v) => {
      // a new value keeps the user's icons unless they were the old defaults
      const [ob, ow] = defaults(X.by), [nb, nw] = defaults(v);
      set({ by: v, best: { ...X.best, icon: !X.best?.icon || X.best.icon === ob ? nb : X.best.icon }, worst: { ...X.worst, icon: !X.worst?.icon || X.worst.icon === ow ? nw : X.worst.icon } });
    }));
    head.append(toggle(`t.ext${xi}.on`, !!X.on, (v) => set({ on: v })));
    const del = h(`<button class="tb-btn sm" title="Remove this ranking">${icon('trash', 13)}</button>`);
    del.addEventListener('click', () => save(list.filter((_, k) => k !== xi)));
    head.append(del);
    const body = card.querySelector('.rk-body');
    body.append(row('Mark with', seg(`t.ext${xi}.show`, X.show || 'icon', [['icon', 'Icon'], ['row', 'Row'], ['both', 'Both']], (v) => set({ show: v }))));
    for (const [kind, label, k0] of [['best', 'Best', 0], ['worst', 'Worst', 1]]) {
      const e = X[kind] || {};
      const def = { icon: defaults(X.by)[k0], color: kind === 'best' ? th.good : th.bad };
      const sk = (patch) => set({ [kind]: { ...e, ...patch } });
      const r = h(`<div class="m-row"></div>`);
      r.append(seg(`t.ext${xi}${kind}n`, String(e.n ?? 1), [['0', '0'], ['1', '1'], ['2', '2'], ['3', '3']], (v) => sk({ n: +v })));
      if ((X.show || 'icon') !== 'row') r.append(iconPick(`t.ext${xi}${kind}i`, e.icon || def.icon, (v) => sk({ icon: v || null }), { title: `${label} icon` }));
      r.append(colorField(`t.ext${xi}${kind}c`, e.color || def.color, (v) => sk({ color: v }), { title: `${label} colour` }));
      body.append(row(label, r));
      body.append(row(`${label} label`, textInput(`t.ext${xi}${kind}l`, e.label || '', (v) => sk({ label: v || null }), { placeholder: kind === 'best' ? 'e.g. best return' : 'e.g. worst return' })));
    }
    // several rows each side: 1st strongest, 2nd and 3rd lighter
    if ((X.show || 'icon') !== 'icon' && ((X.best?.n ?? 1) > 1 || (X.worst?.n ?? 1) > 1)) body.append(row('Fade by place', toggle(`t.ext${xi}.fade`, X.fade !== false, (v) => set({ fade: v ? null : false }), '1st strongest')));
    body.append(row('Lower is better', toggle(`t.ext${xi}.rev`, !!X.reverse, (v) => set({ reverse: v || null }))));
    body.append(row('Mark', seg(`t.ext${xi}.rule`, X.rule === 'rank' ? 'rank' : 'sign', [['sign', 'Gains & losses'], ['rank', 'Highest & lowest']], (v) => set({ rule: v === 'rank' ? 'rank' : null }))));
    body.append(h(`<div class="hint">${X.rule === 'rank' ? 'The highest and the lowest rows are marked whatever their sign.' : 'Best only if the value gained (above 0), worst only if it lost (below 0); a zero is neither. All positive → no worst.'}</div>`));
    items.push(card);
  });
  if (list.length) {
    const used = new Set(list.map((x) => x.by));
    // suggest a different kind of value: an amount next to a return, and the other way round
    const usedPct = list.some((x) => colOf(x.by)?.format === 'percent');
    const free = nums.filter((c) => !used.has(c.key) && !ts.cols?.[c.key]?.bar?.only);
    const next = free.find((c) => (c.format === 'percent') !== usedPct) || free[0] || nums.find((c) => !used.has(c.key));
    const add = h(`<button class="btn sm">${icon('plus', 13)} Add another ranking</button>`);
    add.disabled = !next;
    add.addEventListener('click', () => save([...list, fresh(next.key)]));
    items.push(add);
    if (list.length > 1) items.push(h(`<div class="hint">A row marked by several rankings shows each one’s icon, in this order. Only one fill can show: the first ranking set to <b>Row</b> or <b>Both</b> decides it. Labels (optional) appear under the name, joined with “·”.</div>`));
    else items.push(h(`<div class="hint">Rows are ranked within the period shown; group rows are skipped.</div>`));
  }
  const on = list.filter((x) => x.on).length;
  return section(['Highlights', { icon: 'star', badge: on || null }], items);
}

// ---- a part of a visual clicked on the canvas: the part of the Design tab that changes it

/** The Design tab's part for each part of a visual (charts.js tags them: data-part). */
const PART_SECTIONS = { legend: 'Legend', axis: 'Axes & grid', labels: 'Data labels', highlights: 'Highlights', centre: 'Centre', title: 'Card & title',
  card: 'Card & title', icons: 'Icons', value: 'Value', note: 'Note & breakdown', change: 'Change', trend: 'Trend', marks: 'Colours', connectors: 'Marks', list: 'List' };
/** What a part is called on the canvas, as it is hovered. */
export function partName(part) {
  if (part.startsWith('series:')) return part.slice(7);
  if (part.startsWith('col:')) return 'Column';
  return { legend: 'Legend', axis: 'Axis & grid', labels: 'Data labels', highlights: 'Highlight', centre: 'Centre', title: 'Title', card: 'Card', icons: 'Icons',
    value: 'Value', note: 'Note', change: 'Change', trend: 'Trend', marks: 'Marks', connectors: 'Connectors' }[part] || part;
}

/**
 * Opens the settings of a part of a widget: a table's column, or the Design tab's part that
 * changes it (a series opens Colours at its own colour), unfolded and pointed at.
 */
export function focusPart(id, part) {
  const f = findWidget(id);
  if (!f) return;
  S.sel = id;
  if (part.startsWith('col:')) {
    if (f.w.kind === 'table') { S.colEdit = { id, key: part.slice(4) }; render(); return; }
    // a ranked list's rows are drawn as a table: its list settings
    part = 'list';
  }
  const series = part.startsWith('series:') ? part.slice(7) : null;
  const hasSeries = series && SERIES_KINDS.includes(f.w.kind) && !(f.w.kind === 'bar' && ['horizontal', 'share', 'scale'].includes(drawn(f).opts?.orientation || drawn(f).opts?.viz?.orientation));
  const sec = series ? (hasSeries ? 'Series' : 'Colours') : PART_SECTIONS[part];
  S.colEdit = null;
  S.inspTab = { ...(S.inspTab || {}), widget: 'design', studio: 'design' };
  S.inspQuery = '';
  render();
  requestAnimationFrame(() => {
    const el = right.querySelector(`.insp-sec[data-sec="${sec}"]`) || right.querySelector('.insp-pane .insp-sec');
    if (!el) return;
    if (el.classList.contains('closed')) el.querySelector('.insp-title').click();
    const row = series && ([...el.querySelectorAll('.ser-card')].find((r) => r.dataset.series === series) || [...el.querySelectorAll('.ent-row')].find((r) => r.querySelector('.ent-name')?.getAttribute('title') === series));
    flash(row || el);
  });
}

/** Opens a table column's settings in the inspector. */
function openColumn(f, key) {
  S.colEdit = { id: f.w.id, key };
  renderRight();
}

/** Rule conditions that make sense for a column's values. */
const NUM_OPS = RULE_OPS.filter(([k]) => !['contains'].includes(k));
const TEXT_OPS = RULE_OPS.filter(([k]) => ['=', '!=', 'contains', 'empty'].includes(k));

/**
 * One table column's settings, drilled into from the Columns list. Only the
 * settings that can show for this column are offered: a text column gets
 * icons and pills, a number column its format, bar and colouring; a column
 * shown as a bar alone has no number to format.
 */
/** A badge for the usual words of a direction: a purchase or an inflow on green, a sale or an outflow on red. */
function guessMarker(v) {
  const x = String(v).trim().toLowerCase();
  if (/^(buy|purchase|покупка|купить)$/.test(x)) return { icon: 'txt:B', fill: '$gainMid' };
  if (/^(sell|sale|продажа|продать)$/.test(x)) return { icon: 'txt:S', fill: '$lossMid' };
  if (/^(in|inflow|deposit|ввод|пополнение|зачисление)$/.test(x)) return { icon: 'txt:IN', fill: '$gainMid' };
  if (/^(out|outflow|withdrawal|вывод|снятие|списание)$/.test(x)) return { icon: 'txt:OUT', fill: '$lossMid' };
  return null;
}

/** Every value of a dataset's field, by "dataset|field" (asked of the engine once per field). */
const FIELD_VALUES = new Map();

function columnInspector(f, el) {
  const w = f.w;
  const cols = tableColumns(f);
  const c = cols.find((x) => x.key === S.colEdit.key);
  if (!c) { S.colEdit = null; return null; }
  const own = w.opts?.table?.cols?.[c.key] || {};
  const cs = drawn(f).opts?.table?.cols?.[c.key] || {};
  const save = (patch) => upd(w.id, { opts: { table: { cols: { [c.key]: patch } } } });
  const th = S.st.doc.theme;
  // a value shown as text (a measure aggregated as "text": a direction, a type) is a text column
  const numeric = c.role === 'measure' && (f.w.binding?.measures || []).find((m) => m.id === c.key)?.agg !== 'text';
  const data = f.vm.data || {};
  const j = (data.columns || []).findIndex((x) => x.key === c.key);
  const values = j < 0 ? [] : [...new Set((data.rows || []).map((r) => r[j]).filter((v) => v != null && v !== ''))];
  // every value the column's field can take, from the whole source (not only what this period shows): asked once
  const fieldOf = (f.w.binding?.measures || []).find((m) => m.id === c.key)?.field || (c.role !== 'measure' ? String(c.key).split(':')[0] : '');
  const allKey = `${f.w.binding?.dataset}|${fieldOf}`;
  const allVals = FIELD_VALUES.get(allKey);
  if (!numeric && fieldOf && f.w.binding?.dataset && !allVals) {
    FIELD_VALUES.set(allKey, []);
    call('field_values', { dataset: f.w.binding.dataset, field: fieldOf }, { quiet: true })
      .then((v) => { FIELD_VALUES.set(allKey, Array.isArray(v) ? v.map(String) : []); if (S.colEdit?.key === c.key) renderRight(); }).catch(() => {});
  }
  const others = (data.columns || []).filter((x) => x.key !== c.key);
  const label = cs.label || c.label;

  // ---- header: back to the table, the column's name, hide and reset
  const head = el.querySelector('.insp-head');
  head.innerHTML = '';
  const back = h(`<button class="tb-btn sm ih-back" title="Back to the table">${icon('chevl', 15)}</button>`);
  back.addEventListener('click', () => { S.colEdit = null; renderRight(); });
  const idx = cols.indexOf(c);
  head.append(back, h(`<span class="ih-t"><b>${esc(label)}</b><span>${esc(w.title || 'Table')} · ${numeric ? 'number' : 'text'} column ${idx + 1} of ${cols.length}</span></span>`));
  const acts = h(`<span class="ih-acts"></span>`);
  const btn = (ic, title, fn, disabled) => { const b = h(`<button class="tb-btn sm" title="${esc(title)}"${disabled ? ' disabled' : ''}>${icon(ic, 15)}</button>`); b.addEventListener('click', fn); acts.append(b); };
  btn('chevl', 'Previous column', () => openColumn(f, cols[idx - 1].key), idx === 0);
  btn('chevr', 'Next column', () => openColumn(f, cols[idx + 1].key), idx === cols.length - 1);
  btn(cs.hidden ? 'eye-off' : 'eye', cs.hidden ? 'Hidden — show the column' : 'Hide the column', () => save({ hidden: !cs.hidden || null }));
  btn('undo', 'Reset this column’s own settings', () => upd(w.id, { opts: { table: { cols: { [c.key]: null } } } }), !Object.keys(own).length);
  head.append(acts);

  const sec = (name, ic, ...rows) => section([name, { icon: ic, key: `col:${name}` }], ...rows);
  const pair = (...xs) => { const r = h(`<div class="m-row"></div>`); r.append(...xs.filter(Boolean)); return r; };
  const out = [];

  // ---- what the column shows
  const mode = !numeric ? (cs.pill?.on ? 'pills' : 'text') : cs.bar?.on ? (cs.bar.only ? 'bar' : 'both') : 'number';
  const barOnly = mode === 'bar';
  const display = [row('Header', textInput('ce.label', cs.label || '', (v) => save({ label: v || null }), { placeholder: c.label }))];
  if (numeric) {
    display.push(row('Show as', seg('ce.mode', mode, [['number', 'Number'], ['bar', 'Bar'], ['both', 'Both']], (v) => save({ bar: v === 'number' ? { on: false } : { on: true, only: v === 'bar' } }))));
    display.push(h(`<div class="hint">${barOnly ? 'A bar alone, sized against the largest value. Good for shares.' : mode === 'both' ? 'The number with a bar beside or under it.' : 'The formatted number.'}</div>`));
  } else {
    display.push(row('Show as', seg('ce.mode', mode, [['text', 'Text'], ['pills', 'Pills']], (v) => {
      const p = { on: v === 'pills' };
      if (p.on && !(cs.pill?.rules || []).length && values.length && values.length <= 12) p.rules = values.slice(0, 12).map((x, i) => { const g = guessPill(x) || PILL_PRESETS[[2, 4, 3, 5][i % 4]]; return { match: String(x), fill: g.fill, color: g.color }; });
      save({ pill: p });
    })));
  }
  out.push(sec('Display', 'eye', display));

  // ---- the number
  if (numeric && !barOnly) {
    out.push(sec('Number format', 'numbers',
      row('Format', selectInput('ce.format', cs.format || 'auto', [['auto', 'As the data'], ['number', 'Number'], ['currency', 'Currency'], ['percent', 'Percent'], ['compact', 'Compact (1.2M)']], (v) => save({ format: v === 'auto' ? null : v }))),
      row('Decimals', seg('ce.dec', cs.decimals ?? '', [['', 'Auto'], ['0', '0'], ['1', '1'], ['2', '2']], (v) => save({ decimals: v === '' ? null : +v }))),
      row('Before · after', pair(textInput('ce.prefix', cs.prefix || '', (v) => save({ prefix: v || null }), { placeholder: 'e.g. €' }), textInput('ce.suffix', cs.suffix || '', (v) => save({ suffix: v || null }), { placeholder: 'e.g. pts' }))),
      row('Plus sign', toggle('ce.signed', !!cs.signed, (v) => save({ signed: v }), '+5.2')),
      row('Zero as dash', toggle('ce.zero', cs.zero === 'dash', (v) => save({ zero: v ? 'dash' : null }), '—')),
    ));
  }

  // ---- the bar
  if (numeric && cs.bar?.on) {
    const bar = cs.bar;
    const items = [
      row('Colour', colorField('ce.barc', bar.color || th.palette[0], (v) => save({ bar: { color: v } }), { none: false, title: 'Bar colour' })),
      row('Negative', colorField('ce.barn', bar.negColor || null, (v) => save({ bar: { negColor: v } }), { title: 'Colour of negative bars (the theme’s negative colour if none)' })),
    ];
    items.push(row('Thickness', seg('ce.barsize', bar.size || 'medium', [['thin', 'Thin'], ['medium', 'Medium'], ['thick', 'Thick']], (v) => save({ bar: { size: v === 'medium' ? null : v } }))));
    // an outline around each bar, as on bar charts
    items.push(row('Outline', toggle('ce.barline', !!bar.stroke, (v) => save({ bar: { stroke: v ? 'auto' : null } }))));
    if (bar.stroke) items.push(row('Outline width · colour', pair2(
      textInput('ce.barlw', bar.strokeWidth ?? '', (v) => save({ bar: { strokeWidth: v == null ? null : Math.max(0.25, Math.min(4, v)) } }), { type: 'number', min: 0.25, max: 4, placeholder: '0.5' }),
      colorField('ce.barlc', bar.stroke === 'auto' ? null : bar.stroke, (v) => save({ bar: { stroke: v || 'auto' } }), { title: 'Outline colour (none: the bar’s colour, darker)' }))));
    if (barOnly || bar.pos !== 'below') items.push(row('Ends', seg('ce.barround', bar.round ? 'round' : 'square', [['square', 'Square'], ['round', 'Round']], (v) => save({ bar: { round: v === 'round' || null } }))));
    if (!barOnly) items.push(row('Position', seg('ce.barpos', bar.pos || '', [['', 'Beside'], ['below', 'Under']], (v) => save({ bar: { pos: v || null } }))));
    if (barOnly || bar.pos === 'below') items.push(row('Background', toggle('ce.track', bar.track !== false, (v) => save({ bar: { track: v ? null : false } }), 'Track behind the bar')));
    // the same bar options as a bar chart's List layout (one engine draws both)
    if (barOnly && !bar.pos) {
      items.push(row('Bar length', seg('ce.barbase', bar.base === 'zero' ? 'zero' : 'size', [['zero', 'From zero'], ['size', 'By size']], (v) => save({ bar: { base: v === 'zero' ? 'zero' : null } }))));
      if (bar.track !== false) items.push(row('Track divisions', toggle('ce.barticks', !!bar.ticks, (v) => save({ bar: { ticks: v ? 8 : null } }))));
    }
    if (w.binding?.pivot) items.push(row('Length', seg('ce.barrow', bar.scope === 'row' ? 'row' : '', [['', 'Largest value'], ['row', 'Share of row']], (v) => save({ bar: { scope: v || null } }))));
    out.push(sec('Bar', 'chart-hbar', items));
  }

  // ---- colour by value (numbers shown as numbers)
  if (numeric && !barOnly) {
    const ic = cs.icons || {};
    const items = [
      row('By sign', toggle('ce.sign', !!cs.signColor, (v) => save({ signColor: v }), 'Green / red')),
      row('Arrows', toggle('ce.arrows', !!cs.arrows, (v) => save({ arrows: v }), '▲ ▼')),
      row('Colour scale', pair(toggle('ce.scale', !!cs.scale?.on, (v) => save({ scale: { on: v, from: cs.scale?.from || '#ffffff', to: cs.scale?.to || '#86b6ef' } })),
        cs.scale?.on && colorField('ce.sfrom', cs.scale.from || '#ffffff', (v) => save({ scale: { from: v } }), { none: false, title: 'Lowest value' }),
        cs.scale?.on && h('<span class="muted">→</span>'),
        cs.scale?.on && colorField('ce.sto', cs.scale.to || '#86b6ef', (v) => save({ scale: { to: v } }), { none: false, title: 'Highest value' }))),
      row('Icon set', pair(toggle('ce.icons', !!ic.on, (v) => save({ icons: { on: v, type: ic.type || 'traffic', hi: ic.hi ?? '', lo: ic.lo ?? '' } })),
        ic.on && selectInput('ce.itype', ic.type || 'traffic', ICON_SETS, (v) => save({ icons: { type: v } })))),
    ];
    if (ic.on) {
      items.push(row('Good from', textInput('ce.ihi', ic.hi ?? '', (v) => save({ icons: { hi: v } }), { placeholder: 'e.g. 0.4' })));
      items.push(row('Warning from', textInput('ce.ilo', ic.lo ?? '', (v) => save({ icons: { lo: v } }), { placeholder: 'e.g. 0.3' })));
      items.push(row('Lower is better', toggle('ce.irev', !!ic.reverse, (v) => save({ icons: { reverse: v } }))));
      items.push(h(`<div class="hint">Percentages are fractions: 40% = 0.4.</div>`));
    }
    out.push(sec('Colour by value', 'drop', items));
  }

  // ---- icons and swatches before text values
  if (!numeric) {
    const icon0 = typeof cs.icon === 'string' ? { default: cs.icon } : cs.icon || null;
    const marker = icon0 ? 'icon' : cs.swatch ? 'swatch' : 'none';
    const items = [row('Marker', seg('ce.marker', marker, [['none', 'None'], ['icon', 'Icon'], ['swatch', 'Swatch']], (v) => {
      // the usual words get their badge straight away (B on green for a purchase…); the rest are set under Each value
      if (v === 'icon') {
        const map = {}, fillMap = {};
        for (const x of [...values, ...(FIELD_VALUES.get(allKey) || [])]) { const g = guessMarker(x); if (g) { map[x] = g.icon; fillMap[x] = g.fill; } }
        save({ swatch: null, icon: { map, fillMap, default: '', shape: 'circle' } });
      }
      else if (v === 'swatch') save({ icon: null, swatch: 'dot' });
      else save({ icon: null, swatch: null });
    }))];
    if (marker === 'swatch') {
      items.push(row('Shape', seg('ce.swatch', cs.swatch, [['dot', 'Dot'], ['square', 'Square'], ['tile', 'Tile']], (v) => save({ swatch: v }))));
      items.push(h(`<div class="hint">Each value gets its series colour, as in the charts.</div>`));
    }
    if (icon0) {
      const ip = (patch) => save({ icon: typeof cs.icon === 'string' ? { default: cs.icon, ...patch } : patch });
      // flags and logos found from the value itself; its explanation right under it
      items.push(row('Flags & logos', toggle('ce.iauto', !!icon0.auto, (v) => ip({ auto: v || null }), 'Find them by name')));
      items.push(h(`<div class="hint">On: a value without an icon of its own gets a country’s flag (from its name, currency code or currency name, e.g. “AED”, “UAE dirham”, “Дирхам”) or a company’s or brand’s logo (“Сбербанк”, “SBER-20280425”).</div>`));
      items.push(row('Size', selectInput('ce.isize', String(icon0.size ?? 1), [['0.5', '50%'], ['0.6', '60%'], ['0.7', '70%'], ['0.85', '85%'], ['1', '100% · as the text'], ['1.2', '120%'], ['1.4', '140%']], (v) => ip({ size: +v === 1 ? null : +v }))));
      items.push(...chipRows('ce.i', icon0, ip));
      items.push(row('Other values', iconPick('ce.idef', icon0.default || '', (v) => ip({ default: v || '' }), { none: true, title: 'Marker for values without one of their own (Each value)' })));
      // the marker alone: the value's text hidden (values without a marker keep theirs)
      items.push(row('Text', seg('ce.ionly', cs.iconOnly ? 'hide' : 'show', [['show', 'Show'], ['hide', 'Marker only']], (v) => save({ iconOnly: v === 'hide' || null }))));
      if (cs.iconOnly) items.push(h(`<div class="hint">Only the marker shows; a value without one keeps its text. Pills don't show with the text hidden.</div>`));
    }
    out.push(sec('Icons', 'star', items));
  }

  // ---- pills: the colours of values without their own (each value's are under Each value)
  if (!numeric && cs.pill?.on) {
    out.push(sec('Pills', 'tag', [
      row('Other values', pair(colorField('ce.pdf', cs.pill.fill || '#eceae4', (v) => save({ pill: { fill: v } }), { none: false, title: 'Pill colour' }), colorField('ce.pdc', cs.pill.color || '#55534e', (v) => save({ pill: { color: v } }), { none: false, title: 'Pill text' }))),
      h(`<div class="hint">Each value's own pill colours are under <b>Each value</b>.</div>`),
    ]));
  }

  // ---- each value: its pill colours and its marker (an icon or letters), as the table shows it
  const icon1 = !numeric ? (typeof cs.icon === 'string' ? { default: cs.icon } : cs.icon || null) : null;
  if (!numeric && (cs.pill?.on || icon1) && (values.length || (allVals || []).length)) {
    const rules = cs.pill?.rules || [];
    const ruleOf = (v) => rules.find((r) => String(r.match ?? '').toLowerCase() === String(v).toLowerCase());
    const setPill = (v, patch) => {
      const n = clone(rules);
      let r = n.find((x) => String(x.match ?? '').toLowerCase() === String(v).toLowerCase());
      if (!r) { const g = guessPill(v) || { fill: cs.pill?.fill || '#eceae4', color: cs.pill?.color || '#55534e' }; r = { match: String(v), fill: g.fill, color: g.color }; n.push(r); }
      Object.assign(r, patch);
      save({ pill: { rules: n } });
    };
    const ip = (patch) => save({ icon: typeof cs.icon === 'string' ? { default: cs.icon, ...patch } : patch });
    const th = S.st.doc.theme;
    const what = [cs.pill?.on && 'its own pill colours (background and text)', icon1 && 'its own marker — an icon or letters such as B, S or IN — with the chip’s and the mark’s colours'].filter(Boolean).join(', and ');
    const items = [h(`<div class="hint">Each value can have ${what}. A value with nothing set takes the column's, above.</div>`)];
    // the values shown now, then the source's others (they may show in another period), then those set up but no
    // longer in the source (their settings are kept)
    const low = (x) => String(x).toLowerCase();
    const seen = new Set();
    const listAll = [];
    const add = (v, note) => { if (v == null || v === '' || seen.has(low(v))) return; seen.add(low(v)); listAll.push([String(v), note]); };
    values.forEach((v) => add(v, ''));
    (allVals || []).forEach((v) => add(v, 'not in the data now'));
    [...rules.map((r) => r.match), ...Object.keys(icon1?.map || {}), ...Object.keys(icon1?.fillMap || {})].forEach((v) => add(v, 'set up, not in the source'));
    const list = listAll.slice(0, 60);
    for (const [v, note] of list) {
      const pr = h(`<div class="m-row ev-row"></div>`);
      if (cs.pill?.on) {
        const r = ruleOf(v), g = r || guessPill(v) || { fill: cs.pill.fill || '#eceae4', color: cs.pill.color || '#55534e' };
        pr.append(h(`<span class="pill-prev" style="background:${esc(themeColor(th, g.fill) || '')};color:${esc(themeColor(th, g.color) || '')}">${esc(String(v))}</span>`),
          colorField(`ce.ev.${v}.pf`, r?.fill || null, (c) => setPill(v, { fill: c || g.fill }), { none: false, title: `Pill for “${v}”`, auto: themeColor(th, g.fill) }),
          colorField(`ce.ev.${v}.pc`, r?.color || null, (c) => setPill(v, { color: c || g.color }), { none: false, title: `Pill text for “${v}”`, auto: themeColor(th, g.color) }));
      }
      if (icon1) {
        const cur = glyphFor(icon1.map || {}, v, '');
        // its chip and mark as drawn: a letter badge's letters, unset, are white or ink — whichever reads on the chip
        const chipFill = themeColor(th, (icon1.fillMap || {})[v] || icon1.fill) || '#e9dfcc';
        const ownMark = themeColor(th, (icon1.colorMap || {})[v] || icon1.color || '');
        const hex = (x) => /^#[0-9a-f]{6}$/i.test(x || '');
        const markDrawn = letterText(cur) ? (ownMark && hex(ownMark) && hex(chipFill) && contrastOf(ownMark, chipFill) >= 4.5 ? ownMark : hex(chipFill) ? inkOn(chipFill) : '#ffffff') : ownMark || th.ink;
        pr.append(iconPick(`ce.ev.${v}.ic`, cur, (n) => ip({ map: { [v]: n || null } }), { none: true, title: `Marker for “${v}”`, chip: { fill: chipFill, color: markDrawn } }));
        if ((icon1.shape || 'circle') !== 'none' || letterText(cur)) {
          pr.append(colorField(`ce.ev.${v}.if`, (icon1.fillMap || {})[v] || null, (c) => ip({ fillMap: { [v]: c || null } }), { title: `Chip for “${v}”`, auto: themeColor(th, icon1.fill) || '#e9dfcc' }));
        }
        pr.append(colorField(`ce.ev.${v}.icc`, (icon1.colorMap || {})[v] || null, (c) => ip({ colorMap: { [v]: c || null } }), { title: `Mark for “${v}” (Auto: as reads on the chip)`, auto: markDrawn }));
      }
      const r0 = row(String(v), pr, 'stack');
      if (note) r0.querySelector(':scope > label').append(h(`<span class="ev-note">${esc(note)}</span>`));
      items.push(r0);
    }
    if (listAll.length > list.length) items.push(h(`<div class="hint">The first ${list.length} of ${listAll.length} values; the rest take the column's.</div>`));
    out.push(sec('Each value', 'layers', items));
  }

  // ---- text style (a bar alone has no text)
  if (!barOnly) {
    const fam = cs.family || f.vm.fonts?.table_cell?.family || 'Inter';
    const fb = h(`<button class="fc-family" style="flex:1"><span style="font-family:${esc(fontStack(fam))}">${esc(fam)}</span>${cs.family ? '' : '<em>table font</em>'}${icon('chevd', 12)}</button>`);
    fb.addEventListener('click', (e) => pickFamily(e.currentTarget, fam, (v) => save({ family: v })));
    const signed = numeric && (cs.signColor || cs.arrows);
    const items = [
      row('Font', pair(fb, cs.family && (() => { const x = h(`<button class="tb-btn sm" title="Use the table font">${icon('x', 13)}</button>`); x.addEventListener('click', () => save({ family: null })); return x; })())),
      row('Style', pair(toggle('ce.bold', !!cs.bold, (v) => save({ bold: v }), 'Bold'), toggle('ce.italic', !!cs.italic, (v) => save({ italic: v }), 'Italic'))),
      row('Text colour', signed ? h(`<span class="muted sm">Set by sign (green / red)</span>`) : colorField('ce.color', cs.color || null, (v) => save({ color: v }), { title: 'Text colour' })),
      row('Align', seg('ce.align', cs.align || 'auto', [['auto', 'Auto'], ['left', 'Left', 'align-left'], ['center', 'Centre', 'align-center'], ['right', 'Right', 'align-right']], (v) => save({ align: v === 'auto' ? null : v }))),
    ];
    // wrap: long text breaks onto more lines instead of "…", at the width the column has now
    // (a grouped table's first column can't wrap: it isn't offered there)
    const colPct = () => {
      try {
        const tb = tablePart(S.st, drawn(f), f.vm, f.vm.data);
        const k = tb?.keys?.indexOf(c.key) ?? -1;
        return k >= 0 && tb.w > 0 ? Math.round((tb.cols[k] / tb.w) * 1000) / 10 : 30;
      } catch { return 30; }
    };
    if (!numeric && c.key !== '__tree') items.push(row('Wrap', pair(toggle('ce.wrap', !!cs.wrap, (v) => save(v ? { wrap: true, ...(cs.width ? {} : { width: colPct(), wrapAuto: true }) } : { wrap: null, ...(cs.wrapAuto ? { width: null, wrapAuto: null } : {}) })),
      cs.wrap && selectInput('ce.maxl', cs.maxLines || 4, [[2, 'up to 2 lines'], [3, 'up to 3 lines'], [4, 'up to 4 lines'], [6, 'up to 6 lines']], (v) => save({ maxLines: +v })))));
    items.push(row('Second line', selectInput('ce.sub', cs.sub || '', [['', 'None'], ...others.map((x) => [x.key, x.label])], (v) => save({ sub: v || null }))));
    items.push(row('Tag after it', selectInput('ce.tag', cs.tag || '', [['', 'None'], ...others.map((x) => [x.key, x.label])], (v) => save({ tag: v || null }))));
    out.push(sec('Text', 'type', items));
  }

  // ---- highlights and conditional rules
  {
    const rules = cs.rules || [];
    const items = [];
    if (numeric) {
      const XS = [].concat(drawn(f).opts?.table?.extremes || []).filter(Boolean);
      if (XS.some((x) => x.on && x.by === c.key)) items.push(h(`<div class="hint">${icon('star', 12)} Best and worst rows are ranked by this column (table settings → Best & worst).</div>`));
      else {
        const mark = h(`<button class="btn sm wrap">${icon('star', 13)}<span>Mark best &amp; worst rows by this column</span></button>`);
        const pct = c.format === 'percent' || cs.format === 'percent';
        mark.addEventListener('click', () => upd(w.id, { opts: { table: { extremes: [...XS, { on: true, by: c.key, show: 'icon', best: { n: 1, icon: pct ? 'trend-up' : 'coins' }, worst: { n: 1, icon: pct ? 'trend-down' : 'coins' } }] } } }));
        items.push(mark);
      }
    }
    rules.forEach((r, i) => items.push(ruleCard(r, i, rules, save, numeric, values, others)));
    const add = h(`<button class="btn sm">${icon('plus', 13)} Add rule</button>`);
    add.addEventListener('click', () => save({ rules: [...rules, numeric ? { op: '<', value: '0', color: th.bad } : { op: '=', value: values[0] ?? '', fill: '#fff1d6', row: true }] }));
    items.push(add);
    items.push(h(`<div class="hint">${rules.length ? 'Later rules win. ' : ''}Colour cells or whole rows, or put an icon after the row’s name, when a value meets a condition.</div>`));
    out.push(sec('Highlights & rules', 'highlight', items));
  }

  // ---- totals
  if (numeric && w.opts?.totals !== false) {
    out.push(sec('Total', 'sigma', row('Total row', selectInput('ce.total', cs.total || 'auto', [['auto', 'Automatic'], ['sum', 'Sum'], ['avg', 'Average'], ['min', 'Minimum'], ['max', 'Maximum'], ['count', 'Count'], ['none', 'Empty']], (v) => save({ total: v === 'auto' ? null : v })))));
  }

  // ---- the column's place in the table
  out.push(sec('Column', 'pivot',
    row('Width', textInput('ce.width', cs.width || '', (v) => save({ width: v ? Math.max(3, Math.min(90, +v)) : null }), { type: 'number', min: 3, max: 90, placeholder: 'Auto (% of table)' })),
    row('Header group', textInput('ce.group', cs.group || '', (v) => save({ group: v || null }), { placeholder: 'e.g. This period' })),
    row('Cell fill', colorField('ce.fill', cs.fill || null, (v) => save({ fill: v }), { title: 'Fill of every cell' })),
    row('Band', colorField('ce.band', cs.band || null, (v) => save({ band: v }), { title: 'A band of colour behind the whole column, header included' })),
    row('Divider', seg('ce.rule', cs.rule || '', [['', 'None'], ['left', 'Left'], ['right', 'Right']], (v) => save({ rule: v || null }))),
    !c.tree && (w.binding?.dims || []).length >= 2 && row('In group rows', toggle('ce.ghide', !cs.groupHide, (v) => save({ groupHide: v ? null : true }), 'Show')),
  ));

  const body = h(`<div class="insp-pane" data-cat="design"></div>`);
  for (const s of out) if (s) body.append(s);
  el.append(body);
  return el;
}

/** One conditional rule: the condition, then what it does. */
function ruleCard(r, i, rules, save, numeric, values, cols = []) {
  const set = (patch) => { const n = clone(rules); Object.assign(n[i], patch); save({ rules: n }); };
  const el = h(`<div class="rule-card"><div class="rc-when"><span class="rc-k">When</span></div><div class="rc-then"></div></div>`);
  const when = el.querySelector('.rc-when');
  const ops = numeric ? NUM_OPS : TEXT_OPS;
  when.append(selectInput(`ce.r${i}.op`, r.op, ops.some(([k]) => k === r.op) ? ops : [...ops, RULE_OPS.find(([k]) => k === r.op) || [r.op, r.op]], (v) => set({ op: v })));
  if (!['positive', 'negative', 'empty'].includes(r.op)) {
    const list = !numeric && values.length && values.length <= 40 ? `rv-${i}` : null;
    const inp = textInput(`ce.r${i}.v`, r.value ?? '', (v) => set({ value: v }), { placeholder: ['top', 'bottom'].includes(r.op) ? 'N' : 'value' });
    if (list) { inp.setAttribute('list', list); when.append(h(`<datalist id="${list}">${values.map((v) => `<option value="${esc(v)}">`).join('')}</datalist>`)); }
    when.append(inp);
  }
  if (r.op === 'between') when.append(h('<span class="rc-k">and</span>'), textInput(`ce.r${i}.v2`, r.value2 ?? '', (v) => set({ value2: v }), { placeholder: 'value' }));
  const x = h(`<button class="tb-btn sm rc-x" title="Remove rule">${icon('trash', 13)}</button>`);
  x.addEventListener('click', () => { const n = clone(rules); n.splice(i, 1); save({ rules: n }); });
  when.append(x);
  const then = el.querySelector('.rc-then');
  const line = (k, ...xs) => { const d = h(`<div class="rc-line"><span class="rc-k">${k}</span></div>`); d.append(...xs.filter(Boolean)); then.append(d); };
  line('Colour', colorField(`ce.r${i}.c`, r.color || null, (v) => set({ color: v }), { title: 'Text colour' }), colorField(`ce.r${i}.f`, r.fill || null, (v) => set({ fill: v }), { title: 'Fill' }),
    toggle(`ce.r${i}.b`, !!r.bold, (v) => set({ bold: v || null }), 'Bold'));
  line('Apply to', seg(`ce.r${i}.scope`, r.pill ? 'pill' : r.row ? 'row' : 'cell', [['cell', 'Cell'], ['row', 'Whole row'], ['pill', 'Pill']],
    (v) => set({ row: v === 'row' || null, pill: v === 'pill' || null })));
  line('Stripe', colorField(`ce.r${i}.acc`, typeof r.accent === 'string' ? r.accent : r.accent ? '#c0392b' : null, (v) => set({ accent: v || null }), { title: 'A coloured stripe on the row’s left edge', clear: true }),
    h(`<span class="muted sm">on the row’s left edge</span>`));
  line('Icon', iconPick(`ce.r${i}.icon`, r.icon || '', (v) => set({ icon: v || null }), { none: true, title: 'An icon after the row’s name' }),
    r.icon && colorField(`ce.r${i}.ic`, r.iconColor || null, (v) => set({ iconColor: v }), { title: 'Icon colour' }),
    r.icon && colorField(`ce.r${i}.if`, r.iconFill || null, (v) => set({ iconFill: v }), { title: 'Chip colour (Auto: a tint of the icon colour)', clear: true }),
    h(`<span class="muted sm">after the row’s name</span>`));
  line('Label', textInput(`ce.r${i}.tag`, r.tag || '', (v) => set({ tag: v || null }), { placeholder: 'a badge, e.g. {v} days ({v} is the value)' }));
  if (r.tag) {
    line('Label in', selectInput(`ce.r${i}.tagAt`, r.tagAt || '', [['', 'after the row’s name'], ...cols.map((x) => [x.key, `the column “${x.label.replace(/\n/g, ' ')}”`])], (v) => set({ tagAt: v || null })),
      colorField(`ce.r${i}.tf`, r.tagFill || null, (v) => set({ tagFill: v }), { title: 'Badge colour' }),
      colorField(`ce.r${i}.tc`, r.tagColor || null, (v) => set({ tagColor: v }), { title: 'Badge text' }));
  }
  line('Note', textInput(`ce.r${i}.note`, r.note || '', (v) => set({ note: v || null }), { placeholder: 'under the name, e.g. ▲ best result' }));
  return el;
}

// ---------------------------------------------------------------- calculated fields

/** Adds or edits one SQL calculated field, with a live test against the data. */
// ---------------------------------------------------------------- formats

export const FORMAT_CHOICES = {
  date: [['', 'As the language'], ['dd.mm.yyyy', '31.12.2026'], ['dd/mm/yyyy', '31/12/2026'], ['mm/dd/yyyy', '12/31/2026'], ['yyyy-mm-dd', '2026-12-31'], ['d mmm yyyy', '31 Dec 2026'], ['mmm d, yyyy', 'Dec 31, 2026']],
  thousands: [['', 'As the language'], ['space', '1 234 567'], ['thin', '1 234 567 (thin)'], [',', '1,234,567'], ['.', '1.234.567'], ["'", '1’234’567'], ['none', '1234567']],
  decimal: [['', 'As the language'], ['.', '0.5'], [',', '0,5']],
  currency_pos: [['', 'As the language'], ['before', '$100'], ['after', '100 $']],
  negative: [['', '−1 234'], ['paren', '(1 234)']],
};

/** Rows that edit a set of formats; `set(patch)` saves one choice. */
export function formatRows(f, set, prefix = 'fmt', empty = 'As the language') {
  const ch = (k) => FORMAT_CHOICES[k].map(([v, l]) => [v, v === '' && k !== 'negative' ? empty : l]);
  return [
    row('Dates', selectInput(`${prefix}.date`, f.date || '', ch('date'), (v) => set({ date: v }))),
    row('Thousands', selectInput(`${prefix}.th`, f.thousands || '', ch('thousands'), (v) => set({ thousands: v }))),
    row('Decimals', selectInput(`${prefix}.dec`, f.decimal || '', ch('decimal'), (v) => set({ decimal: v }))),
    row('Negatives', selectInput(`${prefix}.neg`, f.negative || '', FORMAT_CHOICES.negative, (v) => set({ negative: v }))),
    row('Currency symbol', textInput(`${prefix}.cur`, f.currency || '', (v) => set({ currency: v }), { placeholder: empty === 'As the language' ? 'theme’s, e.g. AED' : 'app setting' })),
    row('Symbol goes', selectInput(`${prefix}.cpos`, f.currency_pos || '', ch('currency_pos'), (v) => set({ currency_pos: v }))),
  ];
}

/** A line showing the formats on sample values. */
export function formatSample(f, locale, currency) {
  const prev = getFormats();
  const loc = S.st?.doc?.theme?.locale;
  setLocale(locale || 'en-US', f);
  const t = `${fmtNum(1234567.891, 'number', { decimals: 2 })} · ${fmtNum(-25400, 'currency', { currency: currency || '$' })} · ${fmtNum(0.125, 'percent')} · ${fmtDate('2026-12-31')}`;
  setLocale(loc, prev);
  return h(`<div class="fmt-sample">${icon('eye', 13)}<span>${esc(t)}</span></div>`);
}

/** SQL identifier, quoted when needed. */
const qid = (n) => (/^[a-z_][a-z0-9_]*$/.test(n) ? n : `"${String(n).replace(/"/g, '""')}"`);

/**
 * Level of detail, the SQL way: an aggregate over a group of rows chosen by
 * the user, kept on every row (a window function). The builder writes plain
 * SQL into the expression, so it stays visible and editable.
 */
function lodBuilder(ds, use) {
  const fields = ds.fields || [];
  const nums = fields.filter((f) => f.kind === 'number');
  const dims = fields.filter((f) => f.kind !== 'number');
  const el = h(`<details class="lod">
    <summary>${icon('layers', 14)} Level of detail <span class="muted">— a value computed for a group of rows (Tableau’s FIXED / INCLUDE), as SQL</span></summary>
    <div class="lod-body">
      <div class="lod-grid">
        <label>Compute<select name="agg">${[['sum', 'Sum'], ['avg', 'Average'], ['min', 'Minimum'], ['max', 'Maximum'], ['count', 'Count'], ['count_distinct', 'Count distinct']].map(([v, l]) => `<option value="${v}">${l}</option>`).join('')}</select></label>
        <label>Of<select name="of">${nums.map((f) => `<option>${esc(f.name)}</option>`).join('')}${dims.map((f) => `<option>${esc(f.name)}</option>`).join('')}<option value="__expr">Expression…</option></select></label>
        <label class="lod-expr" hidden>Expression<input name="ofexpr" class="code" placeholder="revenue - cost"></label>
        <label>Rows<select name="when">
          <option value="all">All rows</option>
          ${ds.date_field ? `<option value="period">Report period (:period_start – :period_end)</option><option value="prev">Comparison period</option><option value="ytd">Year to date</option>` : ''}
        </select></label>
        <label>Result<select name="mode">
          <option value="fixed">The group’s value, on every row (FIXED)</option>
          <option value="once">Once per group, to average across groups (INCLUDE)</option>
          <option value="share">Each row’s share of its group</option>
        </select></label>
      </div>
      <div class="lod-by"><span class="muted">Fixed by</span>${dims.map((f) => `<button type="button" class="chip" data-d="${esc(f.name)}">${esc(f.name)}</button>`).join('')}<span class="muted lod-none">none = the whole dataset</span></div>
      <pre class="lod-sql code"></pre>
      <div class="lod-tip hint"></div>
      <div class="ds-actions"><button class="btn sm primary" type="button" id="lod-use">${icon('check', 13)} Use this SQL</button></div>
    </div>
  </details>`);
  const v = (n) => el.querySelector(`[name=${n}]`).value;
  const by = new Set();
  const gen = () => {
    const of = v('of') === '__expr' ? `(${v('ofexpr') || '1'})` : qid(v('of'));
    const d = ds.date_field ? qid(ds.date_field) : null;
    const range = { period: [':period_start', ':period_end'], prev: [':prev_start', ':prev_end'], ytd: ["date_trunc('year', CAST(:period_end AS DATE))", ':period_end'] }[v('when')];
    const inner = range && d ? `CASE WHEN ${d} BETWEEN ${range[0]} AND ${range[1]} THEN ${of} END` : of;
    const agg = v('agg') === 'count_distinct' ? `count(DISTINCT ${inner})` : `${v('agg')}(${inner})`;
    const part = by.size ? `PARTITION BY ${[...by].map(qid).join(', ')}` : '';
    const win = `OVER (${part})`;
    if (v('mode') === 'once') return `CASE WHEN row_number() OVER (${part}) = 1 THEN ${agg} ${win} END`;
    if (v('mode') === 'share') return `${inner} / NULLIF(sum(${inner}) ${win}, 0)`;
    return `${agg} ${win}`;
  };
  const slug = (x) => String(x).toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');
  const name = () => `${v('mode') === 'share' ? 'share' : v('agg')}_${v('of') === '__expr' ? 'value' : slug(v('of'))}${by.size ? '_by_' + [...by].map(slug).join('_') : '_total'}`;
  const draw = () => {
    el.querySelector('.lod-expr').hidden = v('of') !== '__expr';
    el.querySelector('.lod-none').hidden = by.size > 0;
    el.querySelector('.lod-sql').textContent = gen();
    el.querySelector('.lod-tip').innerHTML = {
      fixed: 'Every row carries its group’s total. In a widget, use <b>Maximum</b> or <b>Average</b> of this field (a <b>Sum</b> would count the group once per row).',
      once: 'The value sits on one row per group; the others are empty. In a widget, the <b>Average</b> of this field is the average across groups, e.g. average revenue per customer.',
      share: 'Each row’s part of its group (0–1). <b>Sum</b> it in a widget and show it as a percent.',
    }[v('mode')];
  };
  el.querySelectorAll('.lod-by .chip').forEach((b) => b.addEventListener('click', () => { by.has(b.dataset.d) ? by.delete(b.dataset.d) : by.add(b.dataset.d); b.classList.toggle('on'); draw(); }));
  el.querySelectorAll('select, input').forEach((x) => { x.addEventListener('change', draw); x.addEventListener('input', draw); x.addEventListener('keydown', (e) => e.stopPropagation()); });
  el.querySelector('#lod-use').addEventListener('click', () => use(gen(), name()));
  draw();
  return el;
}

export async function calcFieldEditor(ds, index, { onSave = null } = {}) {
  const save = (next) => (onSave ? onSave(next) : edit({ op: 'upsert_dataset', dataset: next }));
  const existing = index != null ? ds.calc_fields[index] : null;
  const fields = ds.fields || [];
  const body = h(`<div class="form calc-editor">
    <label>Name<input name="name" value="${esc(existing?.name || '')}" placeholder="e.g. amount_m"></label>
    <label>SQL expression<textarea name="expr" class="code" rows="4" placeholder="amount / 1e6">${esc(existing?.expr || '')}</textarea></label>
    <div class="calc-fields"><span class="muted">Fields:</span>${fields.map((f) => `<button type="button" class="chip" data-f="${esc(f.name)}">${esc(f.name)}</button>`).join('')}</div>
    <div class="hint">Examples: <code>amount / 1e6</code> · <code>CASE WHEN amount &gt;= 1e6 THEN 'Large' ELSE 'Small' END</code> · <code>date_trunc('quarter', report_dt)</code> · <code>product_group || ' / ' || product_name</code> · <code>coalesce(region, 'Unknown')</code>. Report values: <code>:period_start</code>, <code>:period_end</code>, <code>:prev_start</code>, <code>:prev_end</code>${S.st.doc.param_defs.map((p) => `, <code>:${esc(p.name)}</code>`).join('')}.</div>
    <div class="lod-slot"></div>
    <div class="ds-actions"><button class="btn" type="button" id="calc-test">${icon('play', 14)} Test</button><span class="calc-status muted"></span></div>
    <div class="calc-sample"></div>
  </div>`);
  const nameEl = body.querySelector('[name=name]');
  const exprEl = body.querySelector('[name=expr]');
  [nameEl, exprEl].forEach((x) => x.addEventListener('keydown', (e) => e.stopPropagation()));
  body.querySelector('.lod-slot').replaceWith(lodBuilder(ds, (sql, name) => { exprEl.value = sql; if (!nameEl.value.trim()) nameEl.value = name; }));
  body.querySelectorAll('.chip').forEach((b) => b.addEventListener('click', () => {
    const at = exprEl.selectionStart ?? exprEl.value.length;
    const f = /^[a-z_][a-z0-9_]*$/.test(b.dataset.f) ? b.dataset.f : `"${b.dataset.f}"`;
    exprEl.value = exprEl.value.slice(0, at) + f + exprEl.value.slice(exprEl.selectionEnd ?? at);
    exprEl.focus();
  }));
  const build = () => {
    const calcs = clone(ds.calc_fields || []);
    const c = { name: nameEl.value.trim(), expr: exprEl.value.trim() };
    if (index != null) calcs[index] = c; else calcs.push(c);
    return { ...ds, calc_fields: calcs };
  };
  const status = body.querySelector('.calc-status');
  const sample = body.querySelector('.calc-sample');
  const test = async () => {
    const name = nameEl.value.trim();
    if (!name || !exprEl.value.trim()) { status.textContent = 'Give the field a name and an expression.'; return false; }
    status.textContent = 'Testing…';
    try {
      const p = await call('preview_dataset', { dataset: build() }, { quiet: true });
      const j = p.columns.indexOf(name);
      const kind = p.fields.find((f) => f.name === name)?.kind || 'text';
      const vals = [...new Set(p.rows.map((r) => r[j]))].slice(0, 8);
      status.textContent = `Works · ${kind}`;
      sample.innerHTML = `<div class="calc-vals">${vals.map((v) => `<span class="chip">${esc(v ?? 'null')}</span>`).join('')}</div>`;
      return true;
    } catch (e) {
      status.textContent = '';
      sample.innerHTML = `<div class="err-box">${esc(e)}</div>`;
      return false;
    }
  };
  body.querySelector('#calc-test').addEventListener('click', test);
  const buttons = [{ label: 'Cancel', value: null }];
  if (existing) buttons.unshift({ label: 'Delete field', value: 'delete' });
  buttons.push({ label: existing ? 'Save' : 'Add field', kind: 'primary', value: () => 'save' });
  const res = await modal({ title: existing ? `Calculated field · ${existing.name}` : `New calculated field · ${ds.name}`, body, wide: true, buttons });
  if (res === 'delete') {
    const calcs = clone(ds.calc_fields || []);
    calcs.splice(index, 1);
    await save({ ...ds, calc_fields: calcs });
    return;
  }
  if (res !== 'save') return;
  const next = build();
  if (!next.calc_fields.every((c) => c.name && c.expr)) { toast('A calculated field needs a name and an expression.', 'error'); return; }
  const st = await save(next);
  if (st) toast(`“${nameEl.value.trim()}” is ready — pick it in any field list.`);
}
