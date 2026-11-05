// Advanced settings: a widget's own settings as JSON, to read, copy or change in one go. What is typed is
// checked against the schema the inspector is built from (the Visualization settings of its kind) before it can
// be applied: wrong types and values outside their choices are errors; names no setting has are warnings.

import { edit } from './state.js';
import { SCHEMA } from './vizschema.js';
import { BASE } from './charts.js';
import { esc, h } from './util.js';
import { icon } from './icons.js';
import { modal } from './ui.js';

// facts that turn on every group of a kind, so every setting it can have is known
const CTXS = [{}, { horizontal: true }, { share: true }, { scale: true }].map((x) => ({
  kind: '', multi: true, barSeries: 2, dims: 1, right: true, percentRight: true, percent: false, lines: true, bars: true, dateCats: true, ...x,
}));

/** Every Visualization setting of a kind, by key: its type and choices (from the schema, all its forms). */
export function vizSpec(kind) {
  const spec = {};
  if (!SCHEMA[kind]) return spec;
  for (const c of CTXS) {
    let groups;
    try { groups = SCHEMA[kind]({ ...c, kind }); } catch { continue; }
    for (const g of groups) for (const it of g.items || []) {
      if (!it.key || spec[it.key]) continue;
      spec[it.key] = { type: it.type, options: it.options?.map((o) => String(o[0])), min: it.min, max: it.max, nullable: !!it.nullable, label: it.label };
    }
  }
  return spec;
}

// settings the drawing reads that have their own editors elsewhere (Series, colours by name, icons, the view)
const OTHER_VIZ = ['seriesOpts', 'colors', 'palette', 'orientation', 'icons', 'iconsAuto', 'iconsOff', 'iconShape', 'iconFill', 'iconColor', 'iconStroke', 'iconStrokeWidth', 'valueSize', 'kpiGroup', 'highlight'];

const isColour = (v) => typeof v === 'string' && (v === '' || /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(v) || /^\$[a-z0-9_]+$/i.test(v) || v === 'transparent' || v === 'none');

/** Problems in a widget's settings: { errors, warnings }, each a list of "where: what". */
export function checkSettings(kind, value) {
  const errors = [], warnings = [];
  if (!value || typeof value !== 'object' || Array.isArray(value)) return { errors: ['The settings must be an object: { "opts": { … }, "style": { … } }.'], warnings };
  for (const k of Object.keys(value)) if (!['opts', 'style'].includes(k)) warnings.push(`${k}: only “opts” and “style” are kept; this part is ignored.`);
  const opts = value.opts ?? {}, style = value.style ?? {};
  if (typeof opts !== 'object' || Array.isArray(opts)) errors.push('opts: must be an object.');
  if (typeof style !== 'object' || Array.isArray(style)) errors.push('style: must be an object.');
  const viz = opts?.viz;
  if (viz != null) {
    if (typeof viz !== 'object' || Array.isArray(viz)) errors.push('opts.viz: must be an object.');
    else {
      const spec = vizSpec(kind);
      const known = new Set([...Object.keys(spec), ...Object.keys(BASE), ...OTHER_VIZ]);
      for (const [k, v] of Object.entries(viz)) {
        const at = `opts.viz.${k}`;
        if (v == null) continue;
        const s = spec[k];
        if (!s) {
          if (!known.has(k)) warnings.push(`${at}: no ${kind} setting has this name — it does nothing.`);
          continue;
        }
        const name = s.label ? ` (${s.label})` : '';
        switch (s.type) {
          case 'toggle': if (typeof v !== 'boolean') errors.push(`${at}${name}: true or false.`); break;
          case 'num': case 'range':
            if (typeof v !== 'number' || !Number.isFinite(v)) errors.push(`${at}${name}: a number.`);
            else if ((s.min != null && v < s.min) || (s.max != null && v > s.max)) warnings.push(`${at}${name}: ${v} is outside ${s.min ?? '…'}–${s.max ?? '…'}; the drawing may limit it.`);
            break;
          case 'seg': case 'select':
            if (s.options && !s.options.includes(String(v))) errors.push(`${at}${name}: one of ${s.options.map((o) => `"${o}"`).join(', ')}.`);
            break;
          case 'color': if (!isColour(v)) errors.push(`${at}${name}: a colour — "#4a72c2" or a theme colour like "$accent".`); break;
          case 'text': if (typeof v !== 'string') errors.push(`${at}${name}: text.`); break;
          default: break;
        }
      }
    }
  }
  return { errors, warnings };
}

/** The patch that turns `old` into `nw` through a merging update: what is gone is set to null. */
export function replacePatch(old, nw) {
  const out = {};
  const o = old && typeof old === 'object' && !Array.isArray(old) ? old : {};
  const n = nw && typeof nw === 'object' && !Array.isArray(nw) ? nw : {};
  for (const k of Object.keys(o)) if (!(k in n)) out[k] = null;
  for (const [k, v] of Object.entries(n)) {
    const was = o[k];
    out[k] = v && typeof v === 'object' && !Array.isArray(v) && was && typeof was === 'object' && !Array.isArray(was) ? replacePatch(was, v) : v;
  }
  return out;
}

/** The dialog: the widget's own settings, checked as they are typed; Apply is one undo step. */
export async function advancedSettings(f) {
  const w = f.w;
  const own = { opts: w.opts || {}, style: w.style || {} };
  const drawn = f.vm?.eff ? { opts: f.vm.eff.opts || {}, style: f.vm.eff.style || {} } : own;
  const body = h(`<div class="modal-body adv">
    <div class="adv-head"><span class="muted">Only what this widget sets itself; the rest comes from its design view and the theme.</span>
      <span class="seg adv-view"><button data-v="own" class="on">This widget</button><button data-v="drawn">As drawn</button></span></div>
    <textarea class="code adv-text" spellcheck="false" data-own-esc></textarea>
    <div class="adv-check"></div>
  </div>`);
  const ta = body.querySelector('.adv-text'), out = body.querySelector('.adv-check');
  ta.value = JSON.stringify(own, null, 2);
  let state = { ok: true, value: own };
  const check = () => {
    let v;
    try { v = JSON.parse(ta.value); } catch (e) { state = { ok: false }; out.innerHTML = `<div class="adv-err">${icon('warn', 13)} Not valid JSON: ${esc(e.message)}</div>`; return; }
    const { errors, warnings } = checkSettings(w.kind, v);
    state = { ok: !errors.length, value: v };
    out.innerHTML = errors.map((x) => `<div class="adv-err">${icon('warn', 13)} ${esc(x)}</div>`).join('')
      + warnings.map((x) => `<div class="adv-warn">${icon('info', 13)} ${esc(x)}</div>`).join('')
      || `<div class="adv-ok">${icon('ok', 13)} Checked: every setting is one this ${esc(w.kind)} has, with a value it takes.</div>`;
    const apply = document.querySelector('.modal .adv-apply');
    if (apply) apply.disabled = !state.ok || body.dataset.view === 'drawn';
  };
  ta.addEventListener('input', check);
  ta.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Tab') { e.preventDefault(); const s = ta.selectionStart; ta.setRangeText('  ', s, ta.selectionEnd, 'end'); check(); }
  });
  // "As drawn": everything merged, to read (and copy from) — applied only from "This widget"
  body.querySelectorAll('.adv-view button').forEach((b) => b.addEventListener('click', () => {
    body.querySelectorAll('.adv-view button').forEach((x) => x.classList.toggle('on', x === b));
    body.dataset.view = b.dataset.v;
    ta.value = JSON.stringify(b.dataset.v === 'drawn' ? drawn : own, null, 2);
    ta.readOnly = b.dataset.v === 'drawn';
    check();
  }));
  const res = await modal({ title: `Advanced settings · ${w.title || w.kind}`, iconName: 'code', wide: true, body,
    onOpen: (box) => { box.querySelector('.modal-foot .btn.primary')?.classList.add('adv-apply'); check(); },
    buttons: [{ label: 'Cancel', value: null }, { label: 'Apply', kind: 'primary', value: () => (state.ok && body.dataset.view !== 'drawn' ? state.value : null) }] });
  if (!res) return;
  const patch = {};
  if ('opts' in res || own.opts) patch.opts = replacePatch(own.opts, res.opts || {});
  if ('style' in res || own.style) patch.style = replacePatch(own.style, res.style || {});
  await edit({ op: 'update_widget', id: w.id, patch });
}
