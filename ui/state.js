// Shared UI state and the bridge to the Rust core.

import { toast } from './ui.js';

import { invoke } from './platform.js';

export const S = {
  st: null, // last state from Rust
  page: 0, // page in view
  zoom: 1,
  zoomFit: true,
  leftTab: null,
  preview: false, // designer previewing as an end user
  showHome: false, // an area's page shows while a document stays open
  lastIssues: new Map(),
  more: [], // more widgets selected with ⇧ (with `sel`, the first, whose settings the inspector shows)
};

// selecting one widget (anywhere: `S.sel = id`) ends a selection of several
let selId = null;
Object.defineProperty(S, 'sel', { get: () => selId, set: (v) => { selId = v; S.more = []; }, enumerable: true });

/** Every selected widget's id, the first (`S.sel`) first. */
export const selection = () => (selId ? [selId, ...S.more] : []);

/** ⇧-click: adds a widget to the selection, or takes it out (the next becomes the first). */
export function toggleSelected(id) {
  const ids = selection();
  const next = ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
  selId = next[0] ?? null;
  S.more = next.slice(1);
}

/**
 * An edit of the first selected widget (or of any selected one, for moving) made for all of them, as one
 * step: the same settings, the same move, removed or duplicated together. Data and words stay each one's own.
 */
export function forSelection(op) {
  const ids = selection().filter((id) => findWidget(id));
  if (ids.length < 2 || !ids.includes(op.id)) return op;
  const others = ids.filter((id) => id !== op.id);
  const many = (make) => ({ op: 'batch', edits: [op, ...others.map(make)] });
  switch (op.op) {
    case 'update_widget': {
      const { binding, title, subtitle, ...patch } = op.patch || {};
      if (patch.opts && typeof patch.opts === 'object') {
        const { text, name, follows, ...o } = patch.opts;
        patch.opts = o;
      }
      // a design view or chart type belongs to its kind: it goes only to widgets of the same kind
      const kind = findWidget(op.id).w.kind;
      const { variant, kind: k2, ...cross } = patch;
      return many((id) => ({ op: 'update_widget', id, patch: findWidget(id).w.kind === kind ? patch : cross }));
    }
    case 'remove_widget':
    case 'duplicate_widget':
      return many((id) => ({ op: op.op, id }));
    case 'set_rect': {
      // moved: all by as much; resized: all to the new size
      const r0 = findWidget(op.id).w, r = op.rect;
      const dx = r.x - r0.x, dy = r.y - r0.y, cols = S.st.page.cols;
      return many((id) => {
        const w = findWidget(id).w, spec = S.st.specs[w.kind] || { min_w: 1, min_h: 1, max_h: 99 };
        const ww = r.w !== r0.w ? Math.max(spec.min_w, Math.min(cols, r.w)) : w.w;
        const hh = r.h !== r0.h ? Math.max(spec.min_h, Math.min(spec.max_h, r.h)) : w.h;
        return { op: 'set_rect', id, rect: { x: Math.max(0, Math.min(cols - ww, w.x + dx)), y: Math.max(0, w.y + dy), w: ww, h: hh } };
      });
    }
    default:
      return op;
  }
}

const listeners = new Set();
export const onChange = (fn) => { listeners.add(fn); return () => listeners.delete(fn); };
export function render() {
  for (const fn of listeners) fn(S);
}

export const doc = () => S.st?.doc;
export const isReport = () => S.st?.doc?.kind === 'report';
/** The theme studio: pages preview the theme's design views. */
export const isTheme = () => S.st?.doc?.kind === 'theme';
/** End-user rules apply (a report, or a designer previewing one). */
export const userMode = () => isReport() || S.preview;
export const policy = () => (userMode() ? S.st.doc.policy : { add: true, remove: true, arrange: true, restyle: true, comment: true, pages: true, edit_text: true });

export function findWidget(id) {
  const d = doc();
  if (!d) return null;
  // a composite's part ("parent::part"): the part as a widget of its own, its data source shown as the
  // composite's when it has none; edits by this id reach the part
  if (typeof id === 'string' && id.includes('::')) {
    const [pid, partId] = id.split('::');
    const p = findWidget(pid);
    const k = p?.w.parts?.findIndex((x) => x.id === partId) ?? -1;
    if (!p || k < 0) return null;
    const part = p.w.parts[k];
    const pb = p.w.binding;
    const binding = part.binding ? { ...part.binding, dataset: part.binding.dataset || pb?.dataset || '' } : part.binding;
    return { pi: p.pi, wi: p.wi, w: { ...part, id, binding, locked: p.w.locked || part.locked }, vm: p.vm?.parts?.[k] || null, page: p.page, parent: p.w, part: k };
  }
  for (let pi = 0; pi < d.pages.length; pi++) {
    const wi = d.pages[pi].widgets.findIndex((w) => w.id === id);
    if (wi >= 0) return { pi, wi, w: d.pages[pi].widgets[wi], vm: S.st.pages[pi].widgets[wi], page: d.pages[pi] };
  }
  return null;
}

export const canEdit = (w) => !userMode() || !w.locked;

export async function call(cmd, args = {}, { quiet = false } = {}) {
  try {
    return await invoke(cmd, args);
  } catch (e) {
    if (!quiet) toast(String(e), 'error', 4200);
    throw e;
  }
}

/** Replaces the state and re-renders. */
export function setState(st) {
  if (!st) return;
  const before = S.st;
  S.st = st;
  if (S.st.doc) {
    S.page = Math.min(S.page, S.st.doc.pages.length - 1);
    // widgets removed (undo, another edit) leave the selection
    const ids = selection().filter((id) => findWidget(id));
    if (ids.length !== selection().length) { selId = ids[0] ?? null; S.more = ids.slice(1); }
  } else S.sel = null;
  render();
  if (st.notice) toast(st.notice, 'info', 5000);
  return before;
}

export async function edit(op, { quiet = false } = {}) {
  try {
    const before = S.st;
    op = forSelection(op);
    const st = await invoke('edit', { op });
    setState(st);
    reportAdjustments(before, st, op);
    return st;
  } catch (e) {
    if (!quiet) toast(String(e), 'error', 4200);
    return null;
  }
}

/** Tells the user when the fit engine moved or resized other widgets. */
function reportAdjustments(before, after, op) {
  if (!before?.doc || !after?.doc || !after.doc.arrange?.auto) return;
  if (['apply_ops', 'auto_arrange', 'set_params'].includes(op.op)) return;
  const pinned = op.id || op.widget?.id;
  const old = new Map();
  before.doc.pages.forEach((p) => p.widgets.forEach((w) => old.set(w.id, w)));
  let resized = 0, moved = 0;
  after.doc.pages.forEach((p) => p.widgets.forEach((w) => {
    const o = old.get(w.id);
    if (!o || w.id === pinned) return;
    if (o.w !== w.w || o.h !== w.h) resized++;
    else if (o.x !== w.x || o.y !== w.y) moved++;
  }));
  if (resized) toast(`Made room: adjusted ${resized} widget${resized === 1 ? '' : 's'} to keep the page fitting.`, 'info', 2600);
}

/**
 * Opening a document replaces the open one: `S.beforeReplace()` (set by the
 * app) asks first when it has unsaved changes. The document then shows.
 */
const opening = (fn, asks = () => true) => async (...a) => {
  if (asks(...a) && S.beforeReplace && !(await S.beforeReplace())) return null;
  const st = await fn(...a);
  if (st) { S.showHome = false; S.sel = null; document.dispatchEvent(new Event('doc-replaced')); setState(st); }
  return st;
};

export const api = {
  // `client`: a report made for that client from the start
  newDoc: opening((kind, client = null) => call('new_doc', { kind, client })),
  open: opening(() => call('open_dialog')),
  openPath: opening((path) => call('open_path', { path })),
  // from the open template: its caller asks (the template is what gets replaced)
  fromTemplate: opening((current = false) => call('report_from_template', { current }), (current) => !current),
  save: (saveAs = false) => call('save', { saveAs }).then((st) => { if (st) { setState(st); toast('Saved'); } return st; }),
  undo: () => call('undo').then(setState),
  redo: () => call('redo').then(setState),
  refresh: () => call('refresh').then(setState),
  // the open template's own theme sets the template aside instead of closing it
  themeStudio: opening((source) => call('theme_studio_open', { source }), (source) => source !== 'current'),
  closeStudio: (apply) => call('theme_studio_close', { apply }).then(setState),
  // the studio and its template or report, both kept open: switch between them
  swapStudio: () => call('theme_studio_swap').then(setState),
  // several documents open at once (up to 10): show one, or close one
  activateDoc: (id) => call('doc_activate', { id }).then((st) => { S.sel = null; document.dispatchEvent(new Event('doc-replaced')); setState(st); }),
  closeDoc: (id) => call('doc_close', { id }).then((st) => { S.sel = null; if (!st?.doc) S.showHome = true; setState(st); }),
  applyTheme: (id) => call('apply_theme', { id }).then((st) => { setState(st); toast('Theme applied'); }),
  themes: () => call('list_themes', {}, { quiet: true }).catch(() => []),
  themeOp: (op, id, name = null) => call('theme_library_op', { op, id, name }),
};
