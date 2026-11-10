/**
 * Admin → API: the systems that ask Quire for reports over its API (a CRM, a client portal, a
 * scheduler), each with its own key, clients and products, and reports; what each published report
 * accepts from them (which parameters a request may set, and their defaults); how long finished
 * files are kept. Keys are shown once, when made (src/api_accounts.rs).
 */
import { call } from './state.js';
import { icon } from './icons.js';
import { esc } from './util.js';
import { toast, modal } from './ui.js';
import { areaHead, accessEditor, accessText } from './areas.js';

const fmtDay = (iso) => (iso ? new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' }) : '');

/** The key, once: copy it now. */
async function showKey(key, name) {
  await modal({ title: `The key for “${name}”`, iconName: 'lock', wide: true,
    body: `<p>Copy it now and give it to the system's team: it isn't shown again. Quire keeps only a fingerprint of it.</p>
      <div class="api-key"><code>${esc(key)}</code><button class="btn sm" data-a="copy">${icon('copy', 13)} Copy</button></div>
      <p class="muted">Requests carry it in the Authorization header, after “Bearer”. A new key replaces this one at once.</p>`,
    buttons: [{ label: 'Done', value: true, kind: 'primary' }],
    onOpen: (box) => box.querySelector('[data-a="copy"]').addEventListener('click', async () => {
      try { await navigator.clipboard.writeText(key); toast('Key copied'); } catch { toast('Select the key and copy it', 'info'); }
    }) });
}

export async function apiAdminPage(el) {
  el.innerHTML = `${areaHead('admin', 'Systems that ask Quire for reports — a CRM, a client portal, a scheduler. Each has its own key and sees only its clients, products and reports. Below: what each report accepts from them.',
    `<button class="btn primary" data-a="add">${icon('plus', 14)} Add API account</button>`, 'API')}
    <div class="ad-split">
      <section class="ar-card">
        <div class="ar-card-t">${icon('link', 15)} API accounts <span class="muted api-n"></span></div>
        <div class="ad-list api-list"><div class="ar-note">Loading…</div></div>
      </section>
      <section class="ar-card ad-edit api-edit"><div class="ar-note">Pick an API account to change it, or add one.</div></section>
    </div>
    <section class="ar-card api-reps"></section>
    <section class="ar-card api-files"></section>`;
  let api, opts;
  try {
    [api, opts] = await Promise.all([call('auth', { cmd: 'api' }, { quiet: true }), call('auth', { cmd: 'options' }, { quiet: true }).catch(() => null)]);
  } catch (e) { el.querySelector('.api-list').innerHTML = `<div class="ar-note bad">${icon('warn', 14)} ${esc(String(e))}</div>`; return; }
  let sel = null; // the account being changed ('' = a new one)
  const listEl = el.querySelector('.api-list'), editEl = el.querySelector('.api-edit');
  const title = (id) => api.reports.find((r) => r.id === id)?.title || id;

  const drawList = () => {
    el.querySelector('.api-n').textContent = String(api.clients.length);
    listEl.innerHTML = api.clients.map((c) => `<div class="ad-row api-row${c.id === sel ? ' on' : ''}${c.active ? '' : ' off'}" data-id="${esc(c.id)}">
        <span class="av">${icon('link', 14)}</span>
        <span class="ad-who"><b class="no-i18n">${esc(c.name)}</b><span><span class="no-i18n">${esc(c.id)}${c.key ? ` · …${esc(c.key.hint)}` : ''}</span>${c.active ? '' : ' · <span>switched off</span>'}</span></span>
        <span class="ad-acc no-i18n">${esc(accessText({ roles: {}, access: c.access }, opts))}</span>
        <span class="ad-acc">${c.reports.length ? `<span class="no-i18n">${esc(c.reports.map(title).join(', '))}</span>` : 'Every published report'}</span>
      </div>`).join('') || '<div class="ar-note">No API accounts yet. Add one for each system that will ask for reports.</div>';
    listEl.querySelectorAll('.api-row').forEach((r) => r.addEventListener('click', () => { sel = r.dataset.id; drawList(); drawEditor(); }));
  };

  const drawEditor = () => {
    const isNew = sel === '';
    const c0 = isNew ? { id: '', name: '', description: '', active: true, access: { all: false, coverage: [], sources: [], clients: [] }, reports: [] } : api.clients.find((c) => c.id === sel);
    if (!c0) { editEl.innerHTML = '<div class="ar-note">Pick an API account to change it, or add one.</div>'; return; }
    const c = JSON.parse(JSON.stringify(c0));
    c.access = { all: true, coverage: [], sources: [], clients: [], ...(c.access || {}) };
    editEl.innerHTML = `<div class="ad-form">
      <div class="ad-title"><span class="av lg">${icon('link', 18)}</span><div>${isNew ? '<h3>New API account</h3><span>A system that will ask for reports</span>' : `<h3 class="no-i18n">${esc(c.name)}</h3><span class="no-i18n">${esc(c.id)}</span>`}</div></div>
      <div class="ad-sec">Account</div>
      <div class="ad-grid">
        <label>Name<input name="name" value="${esc(c.name)}" placeholder="e.g. Client portal"></label>
        <label>Id <input name="id" value="${esc(c.id)}" placeholder="from the name" autocapitalize="off" spellcheck="false"></label>
        <label class="api-wide">What it is<input name="description" value="${esc(c.description)}" placeholder="optional: the system and who runs it"></label>
      </div>
      <div class="ad-toggles"><label class="tgl"><input type="checkbox" name="active"${c.active ? ' checked' : ''}><span class="track"></span><span>Its key works</span></label></div>
      ${isNew ? '' : `<div class="ad-sec">Key</div>
      <div class="api-keyrow">${c.key ? `${icon('lock', 14)} <span>The key ends in</span> <code>…${esc(c.key.hint)}</code> <span class="muted">·</span> <span>Made</span> <span class="no-i18n">${esc(fmtDay(c.key.at))}</span>` : `<span class="muted">No key</span>`}<span class="grow"></span><button class="btn" data-a="key">${icon('refresh', 14)} New key…</button></div>`}
      <div class="ad-sec">Client access <span class="muted">— the clients and products its reports may cover</span></div>
      <div class="ad-access"></div>
      <div class="ad-sec">Reports <span class="muted">— none ticked: every published report</span></div>
      <div class="ad-reps">${api.reports.map((r) => `<label class="ad-rep${c.reports.includes(r.id) ? ' on' : ''}"><input type="checkbox" value="${esc(r.id)}"${c.reports.includes(r.id) ? ' checked' : ''}><span class="ad-rbox">${icon('check', 13, 2.4)}</span><span class="ad-rtx"><b>${esc(r.title)}</b><span>${r.off ? 'kept off the API' : r.published ? 'published' : 'not published'}</span></span></label>`).join('') || '<div class="ar-note">No reports published yet.</div>'}</div>
      <div class="ad-err" hidden></div>
      <div class="ad-acts">
        ${isNew ? '' : `<button class="btn danger-ghost" data-a="del">${icon('trash', 14)} Delete</button>`}
        <span class="grow"></span>
        <button class="btn" data-a="cancel">${isNew ? 'Cancel' : 'Revert'}</button>
        <button class="btn primary" data-a="save">${icon('save', 14)} ${isNew ? 'Add and make its key' : 'Save'}</button>
      </div>
    </div>`;
    const $ = (n) => editEl.querySelector(`[name="${n}"]`);
    accessEditor(editEl.querySelector('.ad-access'), c, opts);
    editEl.querySelectorAll('.ad-rep input').forEach((x) => x.addEventListener('change', () => x.closest('.ad-rep').classList.toggle('on', x.checked)));
    const err = editEl.querySelector('.ad-err');
    editEl.querySelector('[data-a="cancel"]').addEventListener('click', () => { if (isNew) sel = null; drawList(); drawEditor(); });
    editEl.querySelector('[data-a="save"]').addEventListener('click', async () => {
      const client = { was: isNew ? '' : c0.id, id: $('id').value.trim(), name: $('name').value.trim(), description: $('description').value.trim(), active: $('active').checked,
        access: c.access, reports: [...editEl.querySelectorAll('.ad-rep input:checked')].map((x) => x.value) };
      if (!isNew && client.id !== c0.id && !await modal({ title: 'Change the id?', iconName: 'warn', iconKind: 'warn',
        body: '<p>The key carries the id: the account gets a new key, and the old one stops working.</p>',
        buttons: [{ label: 'Cancel', value: false }, { label: 'Change and make a new key', kind: 'primary', value: true }] })) return;
      try {
        const r = await call('auth', { cmd: 'api_save_client', args: { client } }, { quiet: true });
        api = r.api;
        sel = api.clients.find((x) => x.name === client.name && (!client.id || x.id === client.id))?.id ?? null;
        drawList(); drawEditor(); drawReports();
        if (r.key) await showKey(r.key, client.name); else toast(`Saved “${client.name}”`);
      } catch (e) { err.hidden = false; err.textContent = String(e); }
    });
    editEl.querySelector('[data-a="key"]')?.addEventListener('click', async () => {
      if (!await modal({ title: `A new key for “${c0.name}”?`, iconName: 'warn', iconKind: 'warn',
        body: '<p>The current key stops working at once: the system can’t ask for reports until its team puts the new key in.</p>',
        buttons: [{ label: 'Cancel', value: false }, { label: 'Make a new key', kind: 'danger', value: true }] })) return;
      try {
        const r = await call('auth', { cmd: 'api_new_key', args: { id: c0.id } }, { quiet: true });
        api = r.api; drawList(); drawEditor();
        await showKey(r.key, c0.name);
      } catch (e) { toast(String(e), 'error'); }
    });
    editEl.querySelector('[data-a="del"]')?.addEventListener('click', async () => {
      if (!await modal({ title: `Delete “${c0.name}”?`, iconName: 'trash', iconKind: 'warn',
        body: '<p>Its key stops working at once. Its requests stay in Activity.</p>',
        buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] })) return;
      try { api = await call('auth', { cmd: 'api_delete_client', args: { id: c0.id } }, { quiet: true }); sel = null; drawList(); drawEditor(); toast(`Deleted “${c0.name}”`); } catch (e) { toast(String(e), 'error'); }
    });
  };

  // ---- what each report accepts
  const repEl = el.querySelector('.api-reps');
  const defaultText = (v) => (v == null ? '' : Array.isArray(v) ? v.join(', ') : String(v));
  const drawReports = () => {
    repEl.innerHTML = `<div class="ar-card-t">${icon('page', 15)} What each report accepts</div>
      <p class="muted api-lead">A request always gives the client, by the bank's id, and the period: two dates, or this year so far, last month, last quarter or last year. Tick the other parameters a request may set. A default here is used when a request leaves one out; empty, the template's own applies.</p>
      <div class="hint api-lead"><code>client_id</code> · <code>period: { start, end }</code> · <code>period: { preset: ytd | last_month | last_quarter | last_year }</code> · <code>template</code> · <code>params</code></div>
      ${api.reports.map((r) => `<div class="api-rep" data-id="${esc(r.id)}">
        <div class="api-rep-h"><b>${esc(r.title)}</b> <code>${esc(r.id)}</code>${r.published ? '' : ' <span class="muted">· not published</span>'}<span class="grow"></span>
          <label class="tgl"><input type="checkbox" data-k="on"${r.off ? '' : ' checked'}><span class="track"></span><span>Offered over the API</span></label></div>
        ${r.params.length ? `<table class="ac-t api-pt"><thead><tr><th>Parameter</th><th>A request may set it</th><th>Default for API requests</th><th>Template's default</th></tr></thead><tbody>
          ${r.params.map((p) => `<tr data-p="${esc(p.name)}">
            <td><b class="no-i18n">${esc(p.label || p.name)}</b> <code>${esc(p.name)}</code>${p.multi ? ' <span class="muted">· several values</span>' : ''}</td>
            <td><label class="tgl"><input type="checkbox" data-k="accept"${p.accept ? ' checked' : ''}><span class="track"></span></label></td>
            <td><input data-k="default" value="${esc(defaultText(p.api_default))}" placeholder="${p.choices?.length ? 'e.g. ' + esc(p.choices.slice(0, 2).join(', ')) : 'the template’s'}" spellcheck="false">${p.choices?.length ? `<div class="hint"><span class="no-i18n">${esc(p.choices.join(', '))}</span>${p.multi ? ' <span>— several by comma</span>' : ''}</div>` : ''}</td>
            <td class="muted no-i18n">${esc(p.default || '—')}</td>
          </tr>`).join('')}</tbody></table>` : '<div class="ar-note">Only the client and the period.</div>'}
        <div class="api-rep-f"><span class="ad-err" hidden></span><span class="grow"></span><button class="btn sm" data-a="save-rep">${icon('save', 13)} Save</button></div>
      </div>`).join('') || '<div class="ar-note">No reports published yet.</div>'}`;
    repEl.querySelectorAll('.api-rep').forEach((box) => {
      box.querySelector('[data-a="save-rep"]').addEventListener('click', async () => {
        const params = {};
        box.querySelectorAll('tr[data-p]').forEach((tr) => {
          const v = tr.querySelector('[data-k="default"]').value.trim();
          params[tr.dataset.p] = { accept: tr.querySelector('[data-k="accept"]').checked, default: v || null };
        });
        const settings = { off: !box.querySelector('[data-k="on"]').checked, params };
        const err = box.querySelector('.ad-err');
        try {
          api = await call('auth', { cmd: 'api_save_report', args: { id: box.dataset.id, settings } }, { quiet: true });
          drawReports(); drawList();
          toast(`Saved what “${title(box.dataset.id)}” accepts`);
        } catch (e) { err.hidden = false; err.textContent = String(e); }
      });
    });
  };

  // ---- files
  const filesEl = el.querySelector('.api-files');
  filesEl.innerHTML = `<div class="ar-card-t">${icon('clock', 15)} Finished files</div>
    <div class="api-keep"><span>Keep each report for download, in days:</span><input type="number" min="1" max="365" value="${api.retention_days}"><span>then delete it.</span><button class="btn sm" data-a="keep">${icon('save', 13)} Save</button></div>
    <p class="muted">Copies a system asked to be put in a shared folder or bucket aren't deleted.</p>`;
  filesEl.querySelector('[data-a="keep"]').addEventListener('click', async () => {
    const days = +filesEl.querySelector('input').value;
    try { api = await call('auth', { cmd: 'api_retention', args: { days } }, { quiet: true }); toast(`Files are kept ${days} days`); } catch (e) { toast(String(e), 'error'); }
  });

  el.querySelector('[data-a="add"]').addEventListener('click', () => { sel = ''; drawList(); drawEditor(); editEl.querySelector('[name="name"]')?.focus(); });
  drawList();
  drawEditor();
  drawReports();
}
