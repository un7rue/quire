// Repeating pages: a page (or the pages of its group) repeats once per row
// of a data source — one set of account pages per account the client has.
// Here: the designer's dialog to set it up, the badges on pages, and the
// row's fields for texts and filters.

import { S, edit, call } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { modal, toast, menu } from './ui.js';

/** The page that defines the repeat for page `pi` (itself, or the page a copy copies). */
export function repeatSource(pi) {
  const pages = S.st.doc.pages;
  const p = pages[pi];
  if (!p) return null;
  if (p.repeat_of) return pages.find((x) => x.id === p.repeat_of) || null;
  if (p.repeat?.dataset) return p;
  // a continuation page of a repeated table carries the row but not the repeat
  return null;
}

/** The row fields available on page `pi`: [name, value] (empty when the page doesn't repeat). */
export function rowFields(pi) {
  const p = S.st.doc.pages[pi];
  return p?.row ? Object.entries(p.row) : [];
}

/** A short description of a repeat: "each row of «Accounts» where kind = BROK". */
export function repeatText(rep) {
  const ds = S.st.doc.datasets.find((d) => d.id === rep.dataset);
  const name = ds ? ds.name : rep.dataset;
  const where = rep.field && rep.values?.length ? ` where ${rep.field} = ${rep.values.join(' or ')}` : '';
  return `each row of «${name}»${where}`;
}

/** The badge in a page's header: on a repeated page, how many copies; on a copy, which one. */
export function repeatBadge(page, vm, pi) {
  if (page.repeat_of) {
    const src = S.st.doc.pages.findIndex((x) => x.id === page.repeat_of);
    return `<button class="rep-badge copy" data-src="${src}" title="Made from page ${src + 1} for this row. Edit the page it copies — every copy follows.">${icon('duplicate', 12)} Copy ${(page.copy || 0) + 1} of ${page.copies} · ${esc(vm?.copy_label || '')} <span class="go">edit on page ${S.st.pages[src]?.number || src + 1}</span></button>`;
  }
  if (page.repeat?.dataset) {
    const n = page.copies || 0;
    return `<button class="rep-badge" data-rep="${pi}" title="Repeats for ${esc(repeatText(page.repeat))}. Click to change.">${icon('refresh', 12)} ${n ? `Repeats ×${n}` : 'Repeats · no rows now'}${n ? ` · ${esc(vm?.copy_label || '')}` : ''}</button>`;
  }
  return '';
}

/** Wires the badges inside `host` (after the canvas renders). */
export function wireBadges(host, scrollToPage) {
  host.querySelectorAll('.rep-badge[data-rep]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); repeatDialog(+b.dataset.rep); }));
  host.querySelectorAll('.rep-badge.copy').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); const i = +b.dataset.src; S.page = i; scrollToPage(i); }));
}

/** The dialog: repeat this page (and its group) for each row of a data source. */
export async function repeatDialog(pi) {
  const doc = S.st.doc;
  let page = doc.pages[pi];
  if (page.repeat_of) page = doc.pages.find((x) => x.id === page.repeat_of) || page;
  const cur = { dataset: '', field: '', values: [], label: '', ...(page.repeat || {}) };
  const group = page.group || '';
  const groupPages = group ? doc.pages.filter((p) => p.group === group && !p.repeat_of && !p.auto) : [page];
  const titleOf = (p) => (S.st.pages[doc.pages.indexOf(p)]?.title || p.title);
  const body = h(`<div class="modal-body rep-dlg">
    <p class="lead">Make one copy of ${groupPages.length > 1 ? `these ${groupPages.length} pages` : 'this page'} for <b>each row</b> of a data source — for example a set of account pages for every account the client has. The pages you design show the first row; the copies follow them.</p>
    <label>For each row of<select name="ds"><option value="">— Don’t repeat —</option>${doc.datasets.map((d) => `<option value="${esc(d.id)}"${d.id === cur.dataset ? ' selected' : ''}>${esc(d.name)}</option>`).join('')}</select></label>
    <div class="rep-more">
      <div class="rep-where">
        <label>Only rows where<select name="field"><option value="">— every row —</option></select></label>
        <div class="rep-vals"></div>
      </div>
      <label>Name each copy by<select name="label"></select><span class="muted">— in the contents, the page navigation and the copy’s badge</span></label>
      ${groupPages.length > 1 ? `<label class="tgl"><input type="checkbox" name="whole" checked><span class="track"></span><span>Repeat the whole group «${esc(group)}» together: ${groupPages.map((p) => `<i>${esc(titleOf(p))}</i>`).join(', ')}</span></label>` : ''}
      <div class="rep-preview"></div>
      <div class="rep-fields"></div>
      <div class="hint">${icon('info', 13)} To make each copy show <b>its own</b> data, filter its widgets by the row: in a widget’s <b>Data</b> tab add a filter such as <code>acct = {{row.acct_cd}}</code> (the <code>{{row}}</code> button picks a field). Titles, headings and texts take <code>{{row.field}}</code> too — e.g. the page title <code>{{row.title}}</code>.</div>
    </div>
  </div>`);
  const $ = (n) => body.querySelector(`[name="${n}"]`);
  let info = null;
  const read = () => {
    const values = [...body.querySelectorAll('.rep-vals input:checked')].map((x) => x.value);
    return { dataset: $('ds').value, field: $('field').value, values: $('field').value ? values : [], label: $('label').value };
  };
  const fillSelects = () => {
    const cols = info?.columns || [];
    const f = $('field'), l = $('label');
    const fv = f.value || cur.field, lv = l.value || cur.label;
    f.innerHTML = `<option value="">— every row —</option>${cols.map((c) => `<option${c === fv ? ' selected' : ''}>${esc(c)}</option>`).join('')}`;
    l.innerHTML = cols.map((c) => `<option${c === lv ? ' selected' : ''}>${esc(c)}</option>`).join('');
    if (!lv && cols.length) l.value = cols.find((c) => /numb|name|title|label/i.test(c)) || cols[0];
  };
  const fillValues = () => {
    const box = body.querySelector('.rep-vals');
    const field = $('field').value;
    if (!field || !info) { box.replaceChildren(); return; }
    const chosen = new Set(field === cur.field ? cur.values : []);
    const vals = info.values[field] || [];
    box.innerHTML = `<span class="muted">is one of</span>${vals.map((v) => `<label class="rep-chip"><input type="checkbox" value="${esc(v)}"${chosen.has(v) ? ' checked' : ''}><span>${esc(v || '(empty)')}</span></label>`).join('')}${vals.length >= 60 ? '<span class="muted">… the first 60 values</span>' : ''}`;
  };
  const showPreview = async () => {
    const rep = read();
    const pv = body.querySelector('.rep-preview');
    const fl = body.querySelector('.rep-fields');
    body.querySelector('.rep-more').hidden = !rep.dataset;
    if (!rep.dataset) { pv.replaceChildren(); fl.replaceChildren(); return; }
    try {
      const r = await call('repeat_preview', { repeat: rep }, { quiet: true });
      const n = r.copies.length;
      pv.className = `rep-preview${n ? '' : ' none'}`;
      pv.innerHTML = n
        ? `${icon('duplicate', 14)}<span><b>${n} cop${n === 1 ? 'y' : 'ies'}</b> for the report’s current data${rep.field && rep.values.length ? '' : ` (all ${r.rows} rows)`}: ${r.copies.slice(0, 12).map((c) => `<i>${esc(c)}</i>`).join(' · ')}${n > 12 ? ' …' : ''}</span>`
        : `${icon('eye-off', 14)}<span><b>No rows</b> match now, so ${groupPages.length > 1 ? 'these pages are' : 'this page is'} left out of the report (and come back when the data has rows).</span>`;
      const first = r.first || {};
      fl.innerHTML = `<div class="sec-title">Use the row on the page <span class="muted">— values of the first copy</span></div>
        <div class="rep-fl">${r.columns.map((c) => `<button class="rep-f" data-c="${esc(c)}" title="Copy {{row.${esc(c)}}}"><code>{{row.${esc(c)}}}</code><span>${esc(first[c] ?? '')}</span></button>`).join('')}</div>`;
      fl.querySelectorAll('.rep-f').forEach((b) => b.addEventListener('click', () => {
        navigator.clipboard?.writeText(`{{row.${b.dataset.c}}}`).then(() => toast(`Copied {{row.${b.dataset.c}}} — paste it into a title, text or filter.`, 'info', 1800)).catch(() => {});
      }));
    } catch (e) {
      pv.className = 'rep-preview none';
      pv.innerHTML = `${icon('warn', 14)}<span>${esc(String(e))}</span>`;
    }
  };
  const load = async () => {
    const ds = $('ds').value;
    info = null;
    if (ds) {
      try { info = await call('repeat_preview', { repeat: { dataset: ds, field: '', values: [], label: '' } }, { quiet: true }); } catch { info = null; }
    }
    fillSelects();
    fillValues();
    showPreview();
  };
  $('ds').addEventListener('change', () => { cur.field = ''; cur.values = []; cur.label = ''; load(); });
  $('field').addEventListener('change', () => { fillValues(); showPreview(); });
  $('label').addEventListener('change', showPreview);
  body.querySelector('.rep-vals').addEventListener('change', showPreview);
  load();
  const on = !!page.repeat?.dataset;
  const v = await modal({ title: on ? 'Repeated pages' : 'Repeat for each…', iconName: 'refresh', wide: true, body,
    buttons: [
      ...(on ? [{ label: 'Stop repeating', value: 'stop', kind: 'danger' }] : []),
      { label: 'Cancel', value: null },
      { label: on ? 'Save' : 'Repeat', kind: 'primary', value: () => ({ rep: read(), whole: !!body.querySelector('[name=whole]')?.checked }) },
    ] });
  if (!v) return;
  const whole = groupPages.length > 1;
  if (v === 'stop') {
    await edit({ op: 'set_page_repeat', page: page.id, repeat: { dataset: '', field: '', values: [], label: '' }, whole_group: whole });
    toast('The pages no longer repeat.', 'info');
    return;
  }
  if (!v.rep.dataset) {
    if (on) await edit({ op: 'set_page_repeat', page: page.id, repeat: v.rep, whole_group: whole });
    return;
  }
  const st = await edit({ op: 'set_page_repeat', page: page.id, repeat: v.rep, whole_group: v.whole });
  if (st) {
    const n = st.doc.pages.find((p) => p.id === page.id)?.copies || 0;
    toast(n ? `Repeats for ${n} row${n === 1 ? '' : 's'}. Filter widgets by {{row.…}} so each copy shows its own data.` : 'Set up — no rows match right now, so the pages are left out.', 'info', 3600);
  }
}

/** A menu of the row's fields for a filter value or a text, at `anchor`; `pick(token)` inserts it. */
export function rowMenu(anchor, pi, pick) {
  const fields = rowFields(pi);
  if (!fields.length) {
    toast('This page doesn’t repeat. Page menu → Repeat for each… sets it up.', 'info', 2400);
    return;
  }
  menu(anchor, [{ title: 'This page’s row' }, ...fields.map(([k, v]) => ({ label: k, hint: String(v).slice(0, 28), run: () => pick(`{{row.${k}}}`) }))]);
}
