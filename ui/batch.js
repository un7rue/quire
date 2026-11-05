// Batch reports: one published report for many clients at once, each saved as a PDF (or
// PowerPoint) into a folder. Each report is made apart from the open documents (they stay as
// they are), drawn here page by page, and written by the core; a panel in the corner shows
// the progress while you keep working, and flags reports whose pages need a look.

import { WEB } from './platform.js';
import { S, api, call, edit } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { toast, modal } from './ui.js';
import { pageSvg, slide } from './render.js';

const KEY = 'quire.batch';
const load = () => { try { return JSON.parse(localStorage.getItem(KEY) || '{}'); } catch { return {}; } };
const keep = (v) => { try { localStorage.setItem(KEY, JSON.stringify({ ...load(), ...v })); } catch {} };

const iso = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
/** A period by its name: [start, end] as yyyy-mm-dd, or nulls for the report's own. */
function periodOf(kind, from, to) {
  const t = new Date();
  const y = t.getFullYear(), m = t.getMonth();
  switch (kind) {
    case 'ytd': return [iso(new Date(y, 0, 1)), iso(t)];
    case 'month': return [iso(new Date(y, m - 1, 1)), iso(new Date(y, m, 0))];
    case 'quarter': { const q = Math.floor(m / 3); return [iso(new Date(y, (q - 1) * 3, 1)), iso(new Date(y, q * 3, 0))]; }
    case 'year': return [iso(new Date(y - 1, 0, 1)), iso(new Date(y - 1, 11, 31))];
    case 'custom': return [from || null, to || null];
    default: return [null, null];
  }
}
const PERIODS = [['report', 'The report’s own period'], ['ytd', 'This year so far'], ['quarter', 'Last quarter'], ['month', 'Last month'], ['year', 'Last year'], ['custom', 'Dates…']];

let running = null;

/** The batch dialog: report, period, clients, format and folder; then runs it. */
export async function batchDialog(preselect = null) {
  if (running) { running.panel.classList.remove('min'); toast('A batch is already running.', 'info'); return; }
  let reports = [], clients = [];
  try {
    [reports, clients] = await Promise.all([call('auth', { cmd: 'reports' }, { quiet: true }), call('auth', { cmd: 'my_clients' }, { quiet: true })]);
  } catch (e) { toast(String(e), 'error'); return; }
  reports = reports.filter((r) => r.published);
  if (!reports.length) { toast('No report is published to you yet.', 'info'); return; }
  if (!clients.length) { toast('No clients are in your access yet.', 'info'); return; }
  const last = load();
  const picked = new Set(preselect || last.clients?.filter((c) => clients.some((x) => x.name === c)) || clients.map((c) => c.name));
  if (!picked.size) clients.forEach((c) => picked.add(c.name));
  const body = h(`<div class="modal-body bt-dlg" data-i18n="batch">
      <p class="muted">One report for each client you pick, saved into a folder. Your open documents stay as they are; you can keep working while it runs.</p>
      <div class="bt-grid">
        <label>Report</label>
        <select class="bt-rep">${reports.map((r) => `<option value="pub:${esc(r.id)}"${last.kind === `pub:${r.id}` ? ' selected' : ''}>${esc(r.title)}</option>`).join('')}</select>
        <label>Period</label>
        <div class="bt-per"><select class="bt-period">${PERIODS.map(([k, l]) => `<option value="${k}"${(last.period || 'report') === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>
          <span class="bt-dates"><input type="date" class="bt-from" value="${esc(last.from || '')}"> – <input type="date" class="bt-to" value="${esc(last.to || '')}"></span>
          <span class="muted bt-span"></span></div>
        <label>Format</label>
        <div class="bt-fmt"><label><input type="radio" name="bt-fmt" value="pdf"${last.format !== 'pptx' ? ' checked' : ''}> PDF</label><label><input type="radio" name="bt-fmt" value="pptx"${last.format === 'pptx' ? ' checked' : ''}> PowerPoint</label></div>
        <label>Folder</label>
        <div class="bt-folder"><button class="btn sm" data-a="folder">${icon('folder', 13)} Choose…</button><span class="bt-path ${last.folder ? '' : 'muted'}">${esc(last.folder || 'No folder chosen')}</span></div>
      </div>
      <div class="bt-cl-h"><b>Clients</b> <span class="muted bt-n"></span><span class="grow"></span>
        <button class="btn sm" data-a="all">All</button><button class="btn sm" data-a="none">None</button><input class="bt-find" placeholder="Find a client"></div>
      <div class="bt-list">${clients.map((c) => `<label class="bt-cl" data-name="${esc(c.name)}"><input type="checkbox" value="${esc(c.name)}"${picked.has(c.name) ? ' checked' : ''}><span><b>${esc(c.name)}</b><span>${esc(c.id)}${c.banker ? ` · ${esc(c.banker)}` : ''}</span></span></label>`).join('')}</div>
    </div>`);
  let folder = last.folder || '';
  const $ = (s) => body.querySelector(s);
  const count = () => { const n = body.querySelectorAll('.bt-cl input:checked').length; $('.bt-n').textContent = `${n} of ${clients.length}`; };
  const showDates = () => {
    const k = $('.bt-period').value;
    $('.bt-dates').hidden = k !== 'custom';
    const [a, b] = periodOf(k, $('.bt-from').value, $('.bt-to').value);
    $('.bt-span').textContent = k === 'custom' || !a ? '' : `${a} – ${b}`;
  };
  $('.bt-period').addEventListener('change', showDates);
  body.querySelectorAll('.bt-cl input').forEach((i) => i.addEventListener('change', count));
  $('[data-a="all"]').addEventListener('click', () => { body.querySelectorAll('.bt-cl:not([hidden]) input').forEach((i) => { i.checked = true; }); count(); });
  $('[data-a="none"]').addEventListener('click', () => { body.querySelectorAll('.bt-cl:not([hidden]) input').forEach((i) => { i.checked = false; }); count(); });
  $('.bt-find').addEventListener('input', (e) => { const q = e.target.value.trim().toLowerCase(); body.querySelectorAll('.bt-cl').forEach((l) => { l.hidden = !!q && !l.textContent.toLowerCase().includes(q); }); });
  $('[data-a="folder"]').addEventListener('click', async () => {
    const f = await call('pick_folder', {}, { quiet: true }).catch(() => null);
    if (f) { folder = f; $('.bt-path').textContent = f; $('.bt-path').classList.remove('muted'); }
  });
  count();
  showDates();
  const go = await modal({ title: 'Batch reports', iconName: 'layers', wide: true, body,
    buttons: [{ label: 'Cancel', value: null }, { label: 'Make the reports', kind: 'primary', value: (box) => ({
      kind: box.querySelector('.bt-rep').value,
      period: box.querySelector('.bt-period').value, from: box.querySelector('.bt-from').value, to: box.querySelector('.bt-to').value,
      format: box.querySelector('[name=bt-fmt]:checked').value,
      clients: [...box.querySelectorAll('.bt-cl input:checked')].map((i) => i.value),
    }) }] });
  if (!go) return;
  if (!go.clients.length) { toast('Pick at least one client.', 'info'); return batchDialog(); }
  if (!folder) { toast('Choose a folder for the files.', 'info'); return batchDialog(go.clients); }
  const [start, end] = periodOf(go.period, go.from, go.to);
  if (go.period === 'custom' && (!start || !end || start > end)) { toast('Pick a start and an end date.', 'info'); return batchDialog(go.clients); }
  keep({ kind: go.kind, period: go.period, from: go.from, to: go.to, format: go.format, folder, clients: go.clients });
  run({ ...go, start, end, folder, title: reports.find((r) => `pub:${r.id}` === go.kind)?.title || '' });
}

const STATUS = { wait: ['clock', 'Waiting'], work: ['spinner', 'Making…'], done: ['check', 'Saved'], warn: ['warn', 'Saved · pages need a look'], fail: ['alert', 'Failed'], skip: ['x', 'Cancelled'] };

/** Runs a batch: one report after another, with the panel in the corner. */
async function run(job) {
  const items = job.clients.map((name) => ({ name, status: 'wait' }));
  const panel = h(`<div class="bt-panel" role="status" data-i18n="batch">
      <div class="bt-ph">${icon('layers', 14)}<b>Batch reports</b><span class="muted bt-pc"></span><span class="grow"></span>
        <button class="tb-btn" data-a="min" title="Minimise">${icon('chevd', 14)}</button><button class="tb-btn" data-a="close" title="Close" hidden>${icon('x', 14)}</button></div>
      <div class="bt-bar"><i></i></div>
      <div class="bt-sub muted"></div>
      <div class="bt-items"></div>
      <div class="bt-pf"><button class="btn sm" data-a="stop">Stop</button><button class="btn sm" data-a="reveal" hidden>${icon('folder', 13)} Show folder</button></div>
    </div>`);
  document.body.append(panel);
  running = { panel, stop: false };
  const draw = () => {
    const done = items.filter((i) => !['wait', 'work'].includes(i.status)).length;
    panel.querySelector('.bt-pc').textContent = `${done} of ${items.length}`;
    panel.querySelector('.bt-bar i').style.width = `${(done / items.length) * 100}%`;
    panel.querySelector('.bt-sub').textContent = `${job.title} · ${job.format === 'pptx' ? 'PowerPoint' : 'PDF'} · ${job.folder}`;
    panel.querySelector('.bt-items').innerHTML = items.map((it, k) => {
      const [ic, label] = STATUS[it.status];
      return `<div class="bt-it" data-s="${it.status}" data-k="${k}"><span class="bt-ic">${icon(ic, 13)}</span>
        <span class="bt-tx"><b>${esc(it.name)}</b><span>${esc(it.note || label)}</span></span>
        ${it.path && !WEB ? `<button class="tb-btn" data-a="file" title="Open the file">${icon('page', 13)}</button>` : ''}
        ${it.status === 'warn' || it.status === 'done' ? `<button class="tb-btn" data-a="open" title="Open this report in Quire">${icon('external', 13)}</button>` : ''}</div>`;
    }).join('');
  };
  panel.addEventListener('click', async (e) => {
    const a = e.target.closest('[data-a]')?.dataset.a;
    const it = items[+e.target.closest('.bt-it')?.dataset.k];
    if (a === 'min') panel.classList.toggle('min');
    if (a === 'close') { panel.remove(); }
    if (a === 'stop') { running.stop = true; e.target.disabled = true; }
    if (a === 'reveal') call('reveal', { path: items.find((i) => i.path)?.path || job.folder, open: false }).catch(() => {});
    if (a === 'file' && it?.path) call('reveal', { path: it.path, open: true }).catch(() => {});
    if (a === 'open' && it) openOne(job, it.name);
  });
  draw();
  for (const it of items) {
    if (running.stop) { it.status = 'skip'; continue; }
    it.status = 'work';
    draw();
    try {
      const st = await call('batch_report', { kind: job.kind, client: it.name, start: job.start, end: job.end }, { quiet: true });
      const shown = st.doc.pages.map((_, i) => i).filter((i) => !st.pages[i]?.hidden);
      // pages that need a look: content cut off or running past the page
      const issues = shown.reduce((n, i) => n + (st.pages[i].issues || []).filter((x) => x.severity !== 'info').length, 0);
      const pages = job.format === 'pptx' ? shown.map((i) => slide(st, i)) : shown.map((i) => pageSvg(st, i, { exportMode: true }));
      const name = `${it.name} — ${st.doc.meta.title || job.title} — ${st.period?.label || ''}`.replace(/ — $/, '');
      it.path = await call('batch_save', { folder: job.folder, name, format: job.format, pages, width: st.page.w, height: st.page.h, client: it.name, title: st.doc.meta.title || job.title }, { quiet: true });
      it.status = issues ? 'warn' : 'done';
      it.note = issues ? `Saved · ${issues} layout issue${issues === 1 ? '' : 's'} — open it to check` : `Saved · ${shown.length} pages`;
    } catch (e) {
      it.status = 'fail';
      it.note = String(e);
    }
    draw();
    // let the app breathe between reports
    await new Promise((r) => setTimeout(r, 30));
  }
  const ok = items.filter((i) => i.path).length, warn = items.filter((i) => i.status === 'warn').length, fail = items.filter((i) => i.status === 'fail').length;
  panel.querySelector('[data-a="stop"]').hidden = true;
  panel.querySelector('[data-a="reveal"]').hidden = !ok || WEB;
  panel.querySelector('[data-a="close"]').hidden = false;
  panel.classList.add('finished');
  panel.querySelector('.bt-sub').innerHTML = `<span>${ok} saved</span>${warn ? ` · <span>${warn} to check</span>` : ''}${fail ? ` · <span>${fail} failed</span>` : ''} · <span class="no-i18n">${esc(job.folder)}</span>`;
  running = null;
  // the client list shows the new reports ("Last report")
  document.dispatchEvent(new CustomEvent('quire:batch-done'));
  toast(`Batch done: ${ok} of ${items.length} reports saved${warn ? `, ${warn} to check` : ''}.`, fail ? 'error' : 'ok', 5000);
}

/** Opens one report of the batch in the app, for that client and the batch's period. */
async function openOne(job, client) {
  await api.newDoc(job.kind, client);
  if (job.start && job.end && S.st?.doc) {
    await edit({ op: 'set_params', params: { ...S.st.doc.params, grain: 'custom', start: job.start, date: job.end } });
  }
  toast(`Report for ${client} opened.`, 'info', 2400);
}

