// Quire — app shell: title bar, period & parameter controls, home screen,
// shortcuts and export.

import { WEB } from './platform.js';
import { S, api, call, edit, setState, onChange, render, isReport, isTheme, userMode, findWidget, policy, canEdit } from './state.js';
import { initCanvas, renderCanvas, scrollToPage, fitZoom, startInlineEdit, pageItems, selectNewest, widgetMenu, cellAt } from './canvas.js';
import { initLeft, renderLeft, initRight, renderRight, saveToGallery, renamePage, KINDS, newWidget, currentPageId, formatRows, formatSample, publishDialog } from './panels.js';
import { pageSvg, slide, widgetSvg } from './render.js';
import { resolve as vizOptions } from './charts.js';
import { ensureFonts } from './fontui.js';
import { icon, hydrateIcons } from './icons.js';
import { esc, h, mod, clamp, setLocale, fontStack } from './util.js';
import { menu, popover, closeLayer, toast, modal } from './ui.js';
import { showGuide } from './help.js';
import { setAppLanguage } from './i18n.js';
import { closeAllViews, sqlShortcuts, openViews } from './dbx.js';
import { aiButtonHtml, toggleAI, setWhere, registerAI, paintAIButton } from './ai.js';
import { designAssistant } from './aidesign.js';
import { reportAssistant } from './aireport.js';
import { proposalMenu, proposals } from './proposal.js';
import { renderLogin, session, AREAS, currentArea, hasRole } from './auth.js';
import { appearance, setAppearance, applyAppearance, APPEARANCES } from './appearance.js';
import { renderAreas, areaHead } from './areas.js';
import { renderRail, renderDataTabs, goArea, docArea, stageMode, toggleAreaFold } from './rail.js';

// the interface's appearance (also set by the page's head, before the first paint)
applyAppearance();

const $ = (s) => document.querySelector(s);

// ---------------------------------------------------------------- title bar

function renderTitle() {
  const tb = $('#titlebar');
  const st = S.st;
  const mode = stageMode();
  if (!st?.doc || mode !== 'doc') {
    // an area's page or a SQL workspace: the bar says where you are; the workspace has its own menus
    const a = AREAS.find((x) => x.key === currentArea());
    const v = mode === 'view' ? openViews().find((x) => x.active) : null;
    const crumb = v ? `Data <span class="tb-crumb-sep">›</span> ${esc(v.title)}` : a ? esc(a.title) : '';
    tb.innerHTML = `<div class="tb-left" data-tauri-drag-region><span class="app-mark">${icon('logo', 22)}</span><span class="app-name" data-tauri-drag-region>Quire</span>${session() && crumb ? `<span class="tb-crumb" data-tauri-drag-region>${crumb}</span>` : ''}</div><div class="tb-center" data-tauri-drag-region></div>${session() ? `<div class="tb-right">${aiButtonHtml()}</div>` : ''}`;
    tb.querySelector('#ai-btn')?.addEventListener('click', toggleAI);
    return;
  }
  const doc = st.doc;
  const report = isReport();
  const studio = isTheme();
  tb.innerHTML = `
    <div class="tb-left" data-tauri-drag-region>
      <span class="app-mark" title="Home">${icon('logo', 22)}</span>
      <nav class="menubar" role="menubar"></nav>
      <span class="tb-sep"></span>
      <button class="doc-title" id="doc-title" title="${esc(st.path || 'Not saved yet')}">
        <span class="mode-chip ${studio ? 'theme' : report ? 'report' : 'template'}">${studio ? 'Theme' : report ? 'Report' : doc.widget_studio ? 'Widget' : 'Template'}</span>
        <span class="dn">${esc(studio ? doc.theme.name : st.file || doc.meta.title)}</span>
        ${st.dirty ? '<span class="dirty" title="Unsaved changes"></span>' : ''}
      </button>
    </div>
    <div class="tb-center" data-tauri-drag-region></div>
    <div class="tb-right">
      <button class="tb-btn" id="undo" title="Undo (⌘Z)" ${st.undo ? '' : 'disabled'}>${icon('undo', 17)}</button>
      <button class="tb-btn" id="redo" title="Redo (⇧⌘Z)" ${st.redo ? '' : 'disabled'}>${icon('redo', 17)}</button>
      <span class="tb-sep"></span>
      ${report || studio || doc.widget_studio ? '' : `
        <button class="tb-toggle${S.preview ? ' on' : ''}" id="preview" title="See the template the way end users will">${icon('eye', 15)}<span>Preview as user</span></button>
        <button class="tb-pill" id="make-report" title="Start a report from this template">${icon('page', 15)}<span>Create report</span></button>
        ${hasRole('design') ? `<button class="tb-pill" id="publish" title="Put this template in the Reports area for the people you choose">${icon('upload', 15)}<span>${doc.meta.report_id ? 'Publish changes' : 'Publish'}</span></button>` : ''}
      `}
      ${studio && st.stash ? `<button class="tb-pill" id="swap" title="Show «${esc(st.stash)}» with this theme — the studio stays open">${icon('page', 15)}<span>Show ${st.stash_kind === 'report' ? 'report' : 'template'}</span></button>` : ''}
      ${!studio && st.studio_held ? `<button class="tb-pill" id="swap" title="Back to «${esc(st.studio_held)}» in the theme studio — this document stays open">${icon('palette', 15)}<span>Theme studio</span></button>` : ''}
      ${studio && st.stash ? `<button class="tb-pill" id="back" title="Return to «${esc(st.stash)}»">${icon('chevl', 15)}<span>Close studio</span>${icon('chevd', 13)}</button>` : ''}
      ${report ? `<button class="tb-pill" id="proposal" title="Your recommendations to the client as pages of the report">${icon('briefcase', 15)}<span>${proposals().length > 1 ? `Proposals · ${proposals().length}` : proposals().length ? 'Proposal' : 'Add proposal'}</span>${proposals().length ? icon('chevd', 13) : ''}</button>` : ''}
      ${studio || doc.widget_studio ? '' : `<button class="tb-pill" id="export">${icon('download', 15)}<span>Export</span>${icon('chevd', 13)}</button>`}
      ${doc.widget_studio ? `<button class="tb-pill primary" id="save" title="Save the widget to the library (⌘S)">${icon('save', 15)}<span>${doc.widget_studio === 'new' ? 'Save as my widget…' : 'Save widget'}</span></button>`
        : `<button class="tb-pill primary" id="save" title="Save (⌘S)">${icon('save', 15)}<span>${studio ? 'Save theme' : 'Save'}</span></button>`}
      <span class="tb-sep"></span>
      ${aiButtonHtml()}
    </div>`;

  tb.querySelector('.app-mark').addEventListener('click', goHome);
  tb.querySelector('#ai-btn')?.addEventListener('click', toggleAI);
  buildMenuBar(tb.querySelector('.menubar'));
  tb.querySelector('#undo').addEventListener('click', api.undo);
  tb.querySelector('#redo').addEventListener('click', api.redo);
  tb.querySelector('#save').addEventListener('click', () => (S.st?.doc?.widget_studio ? saveStudioWidget() : api.save(false)));
  tb.querySelector('#theme')?.addEventListener('click', (e) => themeMenu(e.currentTarget));
  tb.querySelector('#swap')?.addEventListener('click', () => api.swapStudio());
  tb.querySelector('#proposal')?.addEventListener('click', (e) => proposalMenu(e.currentTarget));
  tb.querySelector('#back')?.addEventListener('click', (e) => menu(e.currentTarget, [
    { label: `Apply to «${st.stash}» and return`, icon: 'check', desc: 'The template takes this theme’s colours, fonts and design views', run: () => api.closeStudio(true).then(() => toast('Theme applied to the template')) },
    { label: 'Return without applying', icon: 'chevl', run: () => api.closeStudio(false) },
  ], { align: 'end' }));
  tb.querySelector('#export')?.addEventListener('click', (e) => menu(e.currentTarget, [
    { label: 'PDF document', icon: 'file-pdf', desc: 'Vector pages with selectable text', hint: '⌘E', run: () => doExport('pdf') },
    { label: 'PowerPoint (.pptx)', icon: 'file-ppt', desc: 'One slide per page, editable text and tables', run: () => doExport('pptx') },
  ], { align: 'end' }));
  tb.querySelector('#doc-title').addEventListener('click', (e) => menu(e.currentTarget, [
    { label: 'Save', icon: 'save', hint: '⌘S', run: () => api.save(false) },
    { label: 'Save as…', icon: 'save', hint: '⇧⌘S', run: () => api.save(true) },
    { label: 'Open…', icon: 'folder', hint: '⌘O', run: openFile },
    { sep: true },
    !report && { label: 'Create a report from this template', icon: 'page', run: () => api.fromTemplate(true) },
    { label: 'New report from a template file…', icon: 'page', run: () => api.fromTemplate(false) },
    { sep: true },
    { label: 'Home', icon: 'layout', run: goHome },
  ]));
  tb.querySelector('#preview')?.addEventListener('click', () => { S.preview = !S.preview; S.leftTab = null; render(); toast(S.preview ? 'Previewing as an end user: only what the template allows is available.' : 'Back to designing', 'info'); });
  tb.querySelector('#publish')?.addEventListener('click', () => publishDialog());
  tb.querySelector('#make-report')?.addEventListener('click', async () => {
    await api.fromTemplate(true);
    toast('New report created from the template. Save it to keep it.', 'info', 3600);
  });
}

// ---------------------------------------------------------------- menu bar

/** The menus, each a function returning its items for the current document and mode. */
function menus() {
  const st = S.st;
  const report = isReport(), studio = isTheme(), user = userMode();
  const pol = policy();
  const f = S.sel ? findWidget(S.sel) : null;
  const pi = Math.min(S.page, st.doc.pages.length - 1);
  const leftTab = (k) => { S.leftTab = k; if (!S.panels.left) togglePanel('left'); render(); };
  const docSettings = () => { S.sel = null; S.inspTab = { ...(S.inspTab || {}), doc: 'doc' }; if (!S.panels.right) togglePanel('right'); render(); };
  const out = [];
  out.push(['File', () => [
    !studio && { label: 'New blank template', icon: 'plus', run: () => newDoc('blank') },
    { label: 'New report from a template…', icon: 'page', run: () => api.fromTemplate(false) },
    !report && !studio && { label: 'Create a report from this template', icon: 'page', run: () => api.fromTemplate(true) },
    { sep: true },
    { label: 'Open…', icon: 'folder', hint: '⌘O', run: openFile },
    { label: studio ? 'Save theme' : 'Save', icon: 'save', hint: '⌘S', run: () => api.save(false) },
    { label: 'Save as…', icon: 'save', hint: '⇧⌘S', run: () => api.save(true) },
    !studio && { sep: true },
    !studio && { label: 'Export PDF', icon: 'file-pdf', hint: '⌘E', run: () => doExport('pdf') },
    !studio && { label: 'Export PowerPoint', icon: 'file-ppt', run: () => doExport('pptx') },
    !studio && { sep: true },
    !studio && { label: report ? 'Report settings' : 'Document & page setup', icon: 'sliders', run: docSettings },
    { sep: true },
    { label: 'Settings…', icon: 'sliders', hint: '⌘,', run: settingsDialog },
    { label: 'Home', icon: 'home', run: goHome },
  ]]);
  out.push(['Edit', () => [
    { label: 'Undo', icon: 'undo', hint: '⌘Z', disabled: !st.undo, run: api.undo },
    { label: 'Redo', icon: 'redo', hint: '⇧⌘Z', disabled: !st.redo, run: api.redo },
    { sep: true },
    ...(f && !studio ? [{ title: f.w.title || 'Selected widget' }, ...widgetItems(f)] : [{ label: studio ? 'Select a design view to edit it' : 'Select a widget to edit it', disabled: true }]),
    f && { label: 'Deselect', icon: 'x', hint: 'Esc', run: () => { S.sel = null; render(); } },
    !report && !studio && { sep: true },
    !report && !studio && { label: 'Preview as user', checked: !!S.preview, run: () => { S.preview = !S.preview; S.leftTab = null; render(); } },
  ]]);
  if (!studio && (!user || pol.add)) out.push(['Insert', () => [
    { title: `Add to page ${pi + 1}` },
    ...KINDS.filter(([k]) => !user || k === 'comment' || pol.add).map(([k, name, ic]) => ({ label: name, icon: ic, run: async () => { const page = currentPageId(); const s2 = await edit({ op: 'add_widget', page, widget: newWidget(k), at: null }); selectNewest(s2, page); } })),
    { sep: true },
    st.doc.catalog.length && { label: 'From the gallery…', icon: 'gallery', run: () => leftTab('gallery') },
    !user && { label: 'Data source…', icon: 'database', run: () => leftTab('data') },
  ]]);
  if (!studio) out.push(['Page', () => [
    { title: `Page ${pi + 1} · ${st.doc.pages[pi].title}` },
    ...pageItems(pi),
    { sep: true },
    { label: 'All pages…', icon: 'page', run: () => leftTab('pages') },
    !user && { label: 'Fit every page', icon: 'wand', desc: 'Arrange each page so nothing overflows', run: async () => { for (const p of st.doc.pages) await edit({ op: 'auto_arrange', page: p.id }, { quiet: true }); toast('Every page arranged'); } },
  ]]);
  out.push(['View', () => [
    { label: 'Left panel', checked: !!S.panels.left, hint: '⌘[', run: () => togglePanel('left') },
    { label: 'Inspector', checked: !!S.panels.right, hint: '⌘]', run: () => togglePanel('right') },
    !studio && !user && { label: 'Grid', checked: !!S.showGrid, run: () => { S.showGrid = !S.showGrid; render(); } },
    !studio && { sep: true },
    !studio && { label: 'Zoom in', icon: 'zoomin', hint: '⌘+', run: () => zoomBy(1.15) },
    !studio && { label: 'Zoom out', icon: 'zoomout', hint: '⌘−', run: () => zoomBy(1 / 1.15) },
    !studio && { label: 'Fit width', icon: 'autofit', hint: '⌘0', run: () => { S.zoomFit = true; render(); } },
    !studio && { sep: true },
    !studio && { label: 'Pages', icon: 'page', run: () => leftTab('pages') },
    !studio && !user && { label: 'Data', icon: 'database', run: () => leftTab('data') },
    !studio && { label: 'Widgets', icon: 'kpi', run: () => leftTab('widgets') },
  ]]);
  if (!studio && (!report || pol.restyle) && !S.preview) out.push(['Theme', null]);
  out.push(['Help', () => [
    { label: 'User guide', icon: 'book', hint: 'F1', desc: 'Every feature, with screenshots and test steps', run: () => showGuide() },
    { label: 'Keyboard shortcuts', icon: 'keyboard', run: shortcutsHelp },
    { label: 'SQL editor shortcuts', icon: 'code', run: sqlShortcuts },
    { label: 'About Quire', icon: 'info', run: () => modal({ title: 'Quire', iconName: 'logo', body: '<p>Professional reports from well-designed templates.</p>', buttons: [{ label: 'OK', kind: 'primary', value: true }] }) },
  ]]);
  return out;
}

/** The selected widget's actions, as in its menu on the canvas. */
function widgetItems(f) {
  const pol = policy(), user = userMode();
  const locked = user && f.w.locked;
  return [
    ['text', 'comment', 'heading'].includes(f.w.kind) && !locked && { label: 'Edit text', icon: 'rename', hint: '↵', run: () => startInlineEdit(f) },
    !locked && (!user || pol.arrange) && { label: 'Duplicate', icon: 'duplicate', hint: '⌘D', run: () => edit({ op: 'duplicate_widget', id: f.w.id }) },
    !locked && (!user || pol.arrange) && { label: 'More…', icon: 'more', run: () => widgetMenu(document.querySelector('.menubar'), f) },
    !user && { label: f.w.locked ? 'Unlock for end users' : 'Lock for end users', icon: f.w.locked ? 'unlock' : 'lock', run: () => edit({ op: 'update_widget', id: f.w.id, patch: { locked: !f.w.locked } }) },
    !locked && (!user || pol.remove || (f.w.kind === 'comment' && pol.comment)) && { label: 'Delete', icon: 'trash', danger: true, hint: '⌫', run: () => edit({ op: 'remove_widget', id: f.w.id }) },
  ];
}

let openBar = null;
function buildMenuBar(nav) {
  for (const [name, items] of menus()) {
    const b = h(`<button class="mb-item" role="menuitem">${esc(name)}</button>`);
    const open = () => {
      openBar = b;
      if (!items) { themeMenu(b, { align: 'start', onClose: () => { if (openBar === b) openBar = null; } }); return; }
      menu(b, items(), { onClose: () => { if (openBar === b) openBar = null; } });
    };
    b.addEventListener('click', () => (openBar === b ? closeLayer() : open()));
    // like a native menu bar: with one menu open, pointing at another opens it
    b.addEventListener('mouseenter', () => { if (openBar && openBar !== b && nav.contains(openBar)) { closeLayer(); open(); } });
    nav.append(b);
  }
}

async function newDoc(kind) {
  S.page = 0; S.sel = null; S.preview = false;
  await api.newDoc(kind);
}

/**
 * Settings → AI: each provider's key (kept in the Keychain — saved at once, never shown
 * again), model and endpoint (saved with Settings), a test; for administrators the bank's
 * policy, bank-wide keys and the request log.
 */
function aiSettingsPage(s) {
  s.ai = s.ai || { default: '', providers: {} };
  s.ai.providers = s.ai.providers || {};
  const wrap = h(`<div class="ai-set"><div class="hint">Loading…</div></div>`);
  const draw = async () => {
    let st;
    try { st = await call('ai_status', {}, { quiet: true }); } catch (e) { wrap.innerHTML = `<div class="hint">${esc(String(e))}</div>`; return; }
    const pol = st.policy || { level: 'structure', providers: [], off: false };
    const usable = st.providers.filter((p) => p.allowed && (p.personal || p.bank));
    wrap.replaceChildren();
    wrap.append(h(`<div class="ai-intro">${icon('lock', 15)}<div><b>What leaves the bank:</b> ${pol.off ? 'nothing — AI is switched off by your administrator.' : pol.level === 'structure' ? 'table and column names, types and SQL — never client data.' : 'structure, and for reports the figures they show — client, account and instrument names and numbers always masked.'} Every request is logged.</div></div>`));
    const def = h(`<label class="set-row"><span>Assistants use</span><select>${['', ...st.providers.map((p) => p.id)].map((id) => `<option value="${id}"${(s.ai.default || '') === id ? ' selected' : ''}>${id ? esc(st.providers.find((p) => p.id === id).name) : 'The first provider with a key'}</option>`).join('')}</select></label>`);
    def.querySelector('select').addEventListener('change', (e) => { s.ai.default = e.target.value; });
    wrap.append(def);
    // the bank's settings (what may be sent, providers, bank keys, the request log) live in Admin → AI
    if (st.admin) {
      const adm = h(`<div class="ai-admin"><div class="sec-title">${icon('shield', 13)} Bank settings · administrators</div>
        <div class="hint">What may be sent (now: <b>${pol.off ? 'AI off' : pol.level === 'structure' ? 'Structure only' : 'Figures'}</b>), providers, bank-wide keys, each person’s level and every AI request are in Admin → AI.</div>
        <div class="ai-row-btns"><button class="btn sm">${icon('shield', 13)} Open Admin → AI</button></div></div>`);
      adm.querySelector('button').addEventListener('click', () => {
        window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
        S.adminTab = 'ai';
        document.dispatchEvent(new CustomEvent('go-area', { detail: 'admin' }));
      });
      wrap.append(adm);
      wrap.append(h(`<div class="sec-title">${icon('key', 13)} Your own keys</div>`));
    }
    if (!usable.length && !pol.off) wrap.append(h(`<div class="hint">Add an API key below to start. Keys are stored in the macOS Keychain and never shown again.</div>`));
    for (const p of st.providers) wrap.append(providerCard(p, st, false));
  };
  const providerCard = (p, st, bank) => {
    const set = (s.ai.providers[p.id] = s.ai.providers[p.id] || { model: '', endpoint: '', scope: '' });
    const has = bank ? p.bank : p.personal;
    const status = bank ? (p.bank ? 'Bank key saved' : 'No bank key') : p.personal ? 'Your key saved' : p.bank ? 'Using the bank’s key' : 'No key';
    const card = h(`<div class="ai-prov${p.allowed ? '' : ' off'}">
      <div class="ai-ph"><b>${esc(p.name)}</b>${p.tools ? '<i>tools</i>' : ''}${p.vision ? '<i>images</i>' : ''}<span class="ai-st ${has || (!bank && p.bank) ? 'ok' : ''}">${esc(status)}</span>${p.allowed ? '' : '<span class="ai-st bad">not allowed by the bank</span>'}</div>
      <div class="ai-key"><input type="password" autocomplete="off" spellcheck="false" placeholder="${has ? '•••••••• saved — paste to replace' : p.id === 'gigachat' ? 'Authorization key (Base64)' : 'Paste the API key'}"><button class="btn sm" data-k="save">Save key</button>${has ? '<button class="btn sm" data-k="del">Remove</button>' : ''}</div>
      ${bank ? '' : `<div class="ai-opts"><label>Model <input list="ai-m-${p.id}" value="${esc(set.model || '')}" placeholder="${esc(p.model)}"><datalist id="ai-m-${p.id}"></datalist></label>
        ${p.id === 'gigachat' ? `<label>Scope <select><option value="GIGACHAT_API_PERS">Personal</option><option value="GIGACHAT_API_B2B">Business (prepaid)</option><option value="GIGACHAT_API_CORP">Corporate (postpaid)</option></select></label>` : ''}
        <details><summary>Endpoint</summary><input class="ai-ep" value="${esc(set.endpoint || '')}" placeholder="${esc(p.endpoint)}"></details>
        <button class="btn sm" data-k="test"${has || p.bank ? '' : ' disabled'}>${icon('play', 12)} Test</button><span class="ai-test"></span></div>`}
      <div class="hint">Keys: ${esc(p.keys_at)}</div></div>`);
    const inp = card.querySelector('.ai-key input');
    inp.addEventListener('keydown', (e) => e.stopPropagation());
    card.querySelector('[data-k="save"]').addEventListener('click', async () => {
      if (!inp.value.trim()) { inp.focus(); return; }
      try { await call('ai_set_key', { provider: p.id, key: inp.value.trim(), bank }); inp.value = ''; toast(`${p.name} key saved to the Keychain.`); draw(); } catch {}
    });
    card.querySelector('[data-k="del"]')?.addEventListener('click', async () => {
      try { await call('ai_set_key', { provider: p.id, key: null, bank }); toast(`${p.name} key removed.`); draw(); } catch {}
    });
    if (!bank) {
      const model = card.querySelector('.ai-opts input[list]');
      model.addEventListener('input', () => (set.model = model.value.trim()));
      model.addEventListener('keydown', (e) => e.stopPropagation());
      const ep = card.querySelector('.ai-ep');
      ep.addEventListener('input', () => (set.endpoint = ep.value.trim()));
      ep.addEventListener('keydown', (e) => e.stopPropagation());
      const scope = card.querySelector('.ai-opts select');
      if (scope) { scope.value = set.scope || 'GIGACHAT_API_PERS'; scope.addEventListener('change', () => (set.scope = scope.value)); }
      const out = card.querySelector('.ai-test');
      card.querySelector('[data-k="test"]').addEventListener('click', async () => {
        out.className = 'ai-test'; out.textContent = 'Testing…';
        try {
          const r = await call('ai_test', { provider: p.id, settings: set }, { quiet: true });
          out.className = 'ai-test ok';
          out.textContent = `Connected (${r.key === 'bank' ? 'bank key' : 'your key'}) · ${r.models.length} models`;
          card.querySelector('datalist').innerHTML = r.models.map((m) => `<option value="${esc(m)}">`).join('');
          if (!set.model && r.models.length && !r.models.includes(p.model)) model.placeholder = r.models[0];
        } catch (e) { out.className = 'ai-test bad'; out.textContent = String(e); }
      });
    }
    return card;
  };
  draw();
  return [wrap];
}

/** Preferences for this computer: formats and what new documents start with (⌘,). */
async function settingsDialog(start = 'region') {
  const st0 = await call('get_settings', {}, { quiet: true }).catch(() => ({}));
  const s = { formats: {}, ...st0 };
  const body = h(`<div class="settings">
    <div class="set-nav"></div>
    <div class="set-pane"></div>
  </div>`);
  const nav = body.querySelector('.set-nav'), pane = body.querySelector('.set-pane');
  const field = (label, ctl, hint) => { const r = h(`<label class="set-row"><span>${esc(label)}</span></label>`); r.append(ctl); if (hint) r.append(h(`<em>${esc(hint)}</em>`)); return r; };
  const sel = (v, opts, on) => { const x = h(`<select>${opts.map(([k, l]) => `<option value="${esc(k)}"${String(k) === String(v ?? '') ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`); x.addEventListener('change', () => on(x.value)); return x; };
  const txt = (v, ph, on) => { const x = h(`<input value="${esc(v || '')}" placeholder="${esc(ph)}">`); x.addEventListener('input', () => on(x.value)); x.addEventListener('keydown', (e) => e.stopPropagation()); return x; };
  const pages = {
    region: ['Language & region', 'globe', () => [
      field('App language', sel(s.ui_lang || 'en', [['en', 'English'], ['ru', 'Русский']], (v) => (s.ui_lang = v)), 'Menus, panels and messages. Reports keep their own language.'),
      field('Appearance', sel(appearance(), APPEARANCES.map(([v, l]) => [v, l]), (v) => setAppearance(v)), 'Light, dark, or as the system — for the app on this computer, at once. Reports keep their own theme.'),
      field('Report language', sel(s.locale, [['', 'As each theme sets it'], ['en-US', 'English (US)'], ['en-GB', 'English (UK)'], ['ru-RU', 'Русский'], ['de-DE', 'Deutsch']], (v) => (s.locale = v))),
      field('Currency symbol', txt(s.currency, 'e.g. $, €, ₽, AED, CHF', (v) => (s.currency = v)), 'New documents use it; any code or symbol works, e.g. AED for UAE dirham.'),
    ]],
    numbers: ['Numbers & dates', 'numbers', () => {
      const wrap = h('<div class="set-fmt"></div>');
      const draw = () => {
        wrap.replaceChildren(...formatRows(s.formats, (patch) => { Object.assign(s.formats, patch); draw(); }, 'set'), formatSample(s.formats, s.locale || 'en-US', s.currency || '$'));
      };
      draw();
      return [wrap, h(`<div class="hint">Every document follows these, except for choices it sets itself (Document → Formats). Empty choices follow the document’s language.</div>`)];
    }],
    defaults: ['New documents', 'page', () => [
      field('Author', txt(s.author, S.st?.user || 'Your name', (v) => (s.author = v))),
      field('Organization', txt(s.organization, 'e.g. Northwind Private Bank', (v) => (s.organization = v))),
      field('Page size', sel(s.page_size, [['', 'A4'], ['A4', 'A4'], ['A3', 'A3'], ['Letter', 'Letter'], ['Slide', 'Slide 16:9']], (v) => (s.page_size = v))),
      field('Orientation', sel(s.landscape == null ? '' : s.landscape ? 'l' : 'p', [['', 'Landscape'], ['l', 'Landscape'], ['p', 'Portrait']], (v) => (s.landscape = v === '' ? null : v === 'l'))),
      field('Period', sel(s.grain, [['', 'Month'], ['month', 'Month'], ['quarter', 'Quarter'], ['year', 'Year'], ['ytd', 'Year to date']], (v) => (s.grain = v))),
      field('Compare with', sel(s.compare, [['', 'Previous period'], ['previous', 'Previous period'], ['year', 'Same period last year'], ['none', 'No comparison']], (v) => (s.compare = v))),
      h(`<div class="hint">New blank templates start with these. Examples keep their own.</div>`),
    ]],
    python: ['Python', 'python', () => {
      if (S.st?.python_off) return [h(`<div class="ar-note">${icon('lock', 14)} Python is switched off on this server: Python data sources and the SQL workspace's Python don't run here. An administrator can switch it on in <b>Admin → Access</b>.</div>`)];
      // the interpreter Python sources and the SQL workspace's Python use
      const box = h(`<div class="py-envs"><div class="hint">Looking for Python on this computer…</div></div>`);
      const custom = h(`<div class="py-custom"><input placeholder="/path/to/python3 — e.g. a project's .venv/bin/python" value="${esc(s.python || '')}"><button class="btn sm">${icon('folder', 13)} Browse…</button><button class="btn sm">Test</button></div>`);
      const [inp, browse, test] = [custom.querySelector('input'), ...custom.querySelectorAll('button')];
      // a path on the server is typed, not picked from this computer
      if (WEB) browse.hidden = true;
      const out = h(`<div class="py-test"></div>`);
      let envs = [];
      const draw = () => {
        const cur = s.python || '';
        box.innerHTML = `<label class="py-env${cur ? '' : ' on'}"><input type="radio" name="pyenv" value=""${cur ? '' : ' checked'}><span class="py-ic">${icon('wand', 15)}</span><span class="py-t"><b>Automatic</b><span>The first Python 3 found: on your shell's PATH, then Homebrew, then the system's</span></span></label>`
          + envs.map((e) => `<label class="py-env${e.path === cur ? ' on' : ''}"><input type="radio" name="pyenv" value="${esc(e.path)}"${e.path === cur ? ' checked' : ''}><span class="py-ic">${icon('python', 15)}</span><span class="py-t"><b>Python ${esc(e.version)} <i>${esc(e.kind)}</i></b><span>${esc(e.path)}</span></span><span class="py-pd ${e.pandas ? 'ok' : 'no'}">${e.pandas ? 'pandas ✓' : 'no pandas'}</span></label>`).join('')
          + (envs.length ? '' : '<div class="hint">No other Python found. Type or browse to one below.</div>');
        box.querySelectorAll('input[name=pyenv]').forEach((r) => r.addEventListener('change', () => { s.python = r.value; inp.value = r.value; draw(); }));
      };
      call('python_envs', {}, { quiet: true }).then((l) => { envs = l || []; draw(); }).catch(() => draw());
      inp.addEventListener('input', () => { s.python = inp.value.trim(); draw(); });
      inp.addEventListener('keydown', (e) => e.stopPropagation());
      browse.addEventListener('click', async () => { const p = await call('pick_data_file', { kind: 'python' }); if (p) { inp.value = p; s.python = p; draw(); } });
      test.addEventListener('click', async () => {
        out.className = 'py-test';
        out.textContent = 'Testing…';
        try { const e = await call('python_test', { path: inp.value.trim() || 'python3' }, { quiet: true }); out.classList.add('ok'); out.innerHTML = `${icon('ok', 14)} Python ${esc(e.version)}${e.pandas ? ' with pandas' : ' — pandas is not installed (Python sources then return plain rows; <code>pip install pandas</code> adds it)'}`; }
        catch (e) { out.classList.add('bad'); out.innerHTML = `${icon('warn', 14)} ${esc(String(e))}`; }
      });
      return [box, h('<div class="sec-title">Or a Python of your own</div>'), custom, out,
        h(`<div class="hint">Python data sources and the SQL workspace's Python run in it. A virtual environment or conda environment brings its own packages (pandas, numpy…). Changing it restarts Python.</div>`)];
    }],
  };
  pages.ai = ['AI assistant', 'sparkles', () => aiSettingsPage(s)];
  let cur = pages[start] ? start : 'region';
  const show = (k) => {
    cur = k;
    nav.replaceChildren(...Object.entries(pages).map(([key, [label, ic]]) => { const b = h(`<button class="${key === cur ? 'on' : ''}">${icon(ic, 15)}<span>${esc(label)}</span></button>`); b.addEventListener('click', () => show(key)); return b; }));
    pane.replaceChildren(h(`<h3>${esc(pages[k][0])}</h3>`), ...pages[k][2]());
  };
  show(cur);
  const res = await modal({ title: 'Settings', iconName: 'sliders', body, wide: true, buttons: [{ label: 'Cancel', value: null }, { label: 'Save', kind: 'primary', value: 'save' }] });
  // keys are saved as they're entered: panes that show the AI in use refresh either way
  document.dispatchEvent(new Event('settings-saved'));
  if (!res) return;
  // open documents follow at once (unless they set their own formats)
  const st = await call('save_settings', { settings: s });
  document.dispatchEvent(new Event('settings-saved'));
  setAppLanguage(s.ui_lang || 'en', render);
  if (st?.doc !== undefined) setState(st);
  const own = S.st?.doc && Object.values(S.st.doc.formats || {}).some(Boolean);
  toast(own ? 'Settings saved. This document sets some formats itself (Document → Formats).' : 'Settings saved and applied.');
}

function shortcutsHelp() {
  const rows = [
    ['⌘S', 'Save'], ['⇧⌘S', 'Save as'], ['⌘O', 'Open'], ['⌘E', 'Export PDF'], ['⌘Z / ⇧⌘Z', 'Undo / redo'],
    ['⌘D', 'Duplicate the selected widget'], ['⌫', 'Delete the selected widget'], ['↵', 'Edit text'], ['Esc', 'Deselect'],
    ['← → ↑ ↓', 'Move the selected widget by a grid cell'], ['⇧ + arrows', 'Resize it'],
    ['F1', 'User guide'], ['⌘F', 'Find a setting in the inspector'], ['⌘+ / ⌘− / ⌘0', 'Zoom in / out / fit width'], ['⌘[ / ⌘]', 'Show or hide the left panel (an area page: its menu or list) / the inspector'], ['⌃Tab', 'Switch to an open SQL workspace and back'],
  ];
  modal({ title: 'Keyboard shortcuts', iconName: 'keyboard', body: `<table class="kbd-table">${rows.map(([k, d]) => `<tr><td><kbd>${esc(k)}</kbd></td><td>${esc(d)}</td></tr>`).join('')}</table>`, buttons: [{ label: 'Done', kind: 'primary', value: true }] });
}

/** The template's theme: edit it in the studio, or put another theme on the template. */
export async function themeMenu(anchor, opts = {}) {
  const list = await api.themes();
  // a report takes only themes with every look it uses
  if (isReport()) {
    const ok = list.filter((t) => t.fits || t.current);
    return menu(anchor, [{ title: 'Theme' }, ...ok.map((t) => ({ label: t.name, icon: 'palette', checked: !!t.current, run: () => !t.current && api.applyTheme(t.id) })),
      ok.length < 2 && { label: 'No other theme has this report’s looks', disabled: true }], { align: 'end', ...opts });
  }
  menu(anchor, [
    { title: `Theme · ${S.st.doc.theme.name || 'unnamed'}` },
    { label: 'Edit in the theme studio', icon: 'palette', desc: 'See every design view with sample data and change it', run: () => api.themeStudio('current') },
    { sep: true },
    { title: 'Apply a theme' },
    ...list.map((t) => ({ label: t.name, icon: t.builtin ? 'sparkles' : 'palette', desc: t.builtin ? 'Built-in' : `${t.views || 0} design views`, run: () => api.applyTheme(t.id) })),
    { sep: true },
    { label: 'Manage themes…', icon: 'library', desc: 'Rename, duplicate or delete your saved themes', run: manageThemes },
  ], { align: 'end', ...opts });
}

/** The theme library: rename, duplicate and delete saved themes; copy built-in ones. */
export async function manageThemes() {
  const body = h('<div class="theme-lib"></div>');
  const ask = (title, value) => {
    const f = h(`<div class="form"><label>Name<input name="v" value="${esc(value)}"></label></div>`);
    setTimeout(() => { const i = f.querySelector('input'); i?.focus(); i?.select(); }, 30);
    return modal({ title, body: f, buttons: [{ label: 'Cancel', value: null }, { label: 'OK', kind: 'primary', value: (b) => b.querySelector('[name=v]').value.trim() || null }] });
  };
  // the manager reopens after a nested prompt, since modals don't stack
  const run = async (fn, reopen = true) => { closeLib(); try { await fn(); } catch (e) { toast(String(e?.message || e), 'error'); } if (reopen) manageThemes(); };
  let closeLib = () => {};
  const list = await api.themes();
  const row = (t) => {
    const r = h(`<div class="tl-row">
      <span class="tl-ic">${icon(t.builtin ? 'sparkles' : 'palette', 16)}</span>
      <span class="tl-name"><b></b><span>${t.builtin ? 'Built-in' : `${t.views || 0} design views`}</span></span>
      <span class="tl-acts"></span>
    </div>`);
    r.querySelector('b').textContent = t.name;
    const act = (ic, title, fn, cls = '', reopen = true) => {
      const b = h(`<button class="btn sm icon ${cls}" title="${esc(title)}">${icon(ic, 14)}</button>`);
      b.addEventListener('click', () => run(fn, reopen));
      r.querySelector('.tl-acts').append(b);
    };
    act('palette', 'Open in the theme studio', () => { S.page = 0; S.sel = null; S.preview = false; return api.themeStudio(t.id); }, '', false);
    act('duplicate', t.builtin ? 'Duplicate into your library' : 'Duplicate', async () => {
      const name = await ask('Duplicate theme', `${t.name} copy`);
      if (name) { await api.themeOp('duplicate', t.id, name); toast(`“${name}” added to your library`); }
    });
    if (!t.builtin) {
      act('rename', 'Rename', async () => {
        const name = await ask('Rename theme', t.name);
        if (name && name !== t.name) { await api.themeOp('rename', t.id, name); toast('Theme renamed'); }
      });
      if (!WEB) act('folder', 'Show in Finder', () => call('reveal', { path: t.path, open: false }), '', false);
      act('trash', 'Delete', async () => {
        const ok = await modal({ title: 'Delete theme', iconName: 'warn', iconKind: 'warn', body: `<p>Delete “${esc(t.name)}” from your library? Templates that use it keep their copy of the theme.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
        if (ok) { await api.themeOp('delete', t.id); toast('Theme deleted'); }
      }, 'danger-ghost');
    }
    return r;
  };
  const mine = list.filter((t) => !t.builtin), built = list.filter((t) => t.builtin);
  body.append(h('<div class="tl-head">Your library</div>'));
  if (mine.length) mine.forEach((t) => body.append(row(t)));
  else body.append(h('<p class="tl-empty">No saved themes yet. Duplicate a built-in theme, or save one from the theme studio.</p>'));
  body.append(h('<div class="tl-head">Built-in</div>'));
  built.forEach((t) => body.append(row(t)));
  modal({ title: 'Themes', iconName: 'library', body, wide: true, buttons: [{ label: 'Done', kind: 'primary', value: true }], onOpen: (_, done) => { closeLib = () => done(null); } });
}

/** The area the open document belongs to; the document stays open in the rail. */
function goHome() {
  goArea(S.st?.doc ? docArea() : null);
}

function confirmDiscard(text, label) {
  return modal({ title: 'Unsaved changes', iconName: 'warn', iconKind: 'warn', body: `<p>${esc(text)}</p>`, buttons: [{ label: 'Cancel', value: false }, { label, kind: 'danger', value: true }] });
}

async function openFile() {
  await api.open();
  S.page = 0;
  S.preview = false;
  scrollToPage(0);
}

// ---------------------------------------------------------------- toolbar: period & parameters

const GRAIN_LABEL = { month: 'Month', quarter: 'Quarter', year: 'Year', ytd: 'Year to date', custom: 'Custom range' };
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const parseD = (s) => { const [y, m, d] = s.split('-').map(Number); return new Date(Date.UTC(y, m - 1, d)); };
const fmtD = (d) => d.toISOString().slice(0, 10);
function addMonths(s, n) {
  const d = parseD(s);
  const t = new Date(Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + n, 1));
  const last = new Date(Date.UTC(t.getUTCFullYear(), t.getUTCMonth() + 1, 0)).getUTCDate();
  t.setUTCDate(Math.min(d.getUTCDate(), last));
  return fmtD(t);
}

function setParams(patch) {
  return edit({ op: 'set_params', params: { ...S.st.doc.params, ...patch } });
}

function renderToolbar() {
  const bar = $('#toolbar');
  const st = S.st;
  if (!st?.doc) { bar.innerHTML = ''; return; }
  if (isTheme()) {
    // the studio has no period, pages or zoom: just the panels
    bar.innerHTML = `
      <button class="tb-btn sm panel-tg${S.panels.left ? '' : ' off'}" id="tg-left" title="${S.panels.left ? 'Hide' : 'Show'} left panel (⌘[)">${icon('sidebar-left', 16)}</button>
      <span class="studio-crumb">${icon('palette', 14)}<b>Theme studio</b><span>Previews use built-in sample data</span></span>
      <span class="grow"></span>
      <button class="tb-btn sm panel-tg${S.panels.right ? '' : ' off'}" id="tg-right" title="${S.panels.right ? 'Hide' : 'Show'} inspector (⌘])">${icon('sidebar-right', 16)}</button>`;
    bar.querySelector('#tg-left').addEventListener('click', () => togglePanel('left'));
    bar.querySelector('#tg-right').addEventListener('click', () => togglePanel('right'));
    return;
  }
  const p = st.doc.params;
  const step0 = { month: 1, quarter: 3, year: 12, ytd: 1 }[p.grain] || 0;
  // report users see only the filters the template offers them
  const pol = st.doc.policy || {};
  const user = userMode();
  const access = (a) => (user ? a || 'change' : 'change');
  // the comparison shows only where something uses it: a change on a KPI card from dated data, a
  // measure against the previous period, a text with {{compare}}, or SQL reading :prev_start / :prev_end
  const compareUsed = usesCompare(st);
  const periodA = access(pol.period), compareA = compareUsed ? access(pol.compare) : 'hide';
  const step = periodA === 'change' ? step0 : 0;
  bar.innerHTML = `
    <button class="tb-btn sm panel-tg${S.panels.left ? '' : ' off'}" id="tg-left" title="${S.panels.left ? 'Hide' : 'Show'} left panel (⌘[)">${icon('sidebar-left', 16)}</button>
    <div class="period${periodA === 'show' ? ' fixed' : ''}"${periodA === 'hide' && compareA === 'hide' ? ' hidden' : ''}>
      <span class="tl">${icon('calendar', 15)}</span>
      <select id="grain" class="ghost-select"${periodA === 'show' ? ' disabled' : ''}>${Object.entries(GRAIN_LABEL).map(([k, l]) => `<option value="${k}"${k === p.grain ? ' selected' : ''}>${l}</option>`).join('')}</select>
      ${step ? `<button class="tb-btn sm" id="prev" title="Previous">${icon('chevl', 15)}</button>` : ''}
      <button class="period-label" id="plabel">${esc(st.period.label)}</button>
      ${step ? `<button class="tb-btn sm" id="next" title="Next">${icon('chevr', 15)}</button>` : ''}
      <select id="compare" class="ghost-select muted"${compareA === 'show' ? ' disabled' : ''}${compareA === 'hide' ? ' hidden' : ''}>
        <option value="previous"${p.compare === 'previous' ? ' selected' : ''}>vs previous period</option>
        <option value="year"${p.compare === 'year' ? ' selected' : ''}>vs same period last year</option>
        <option value="none"${p.compare === 'none' ? ' selected' : ''}>no comparison</option>
      </select>
    </div>
    <div class="params"></div>
    <button class="tb-btn sm" id="refresh" title="Reload data from the sources">${icon('refresh', 15)}</button>
    <span class="grow"></span>
    <div class="fit-summary"></div>
    <div class="zoom">
      ${userMode() ? '' : `<button class="tb-btn sm${S.showGrid ? ' on' : ''}" id="grid" title="Show grid">${icon('grid', 15)}</button>`}
      <button class="tb-btn sm" id="zout" title="Zoom out">${icon('zoomout', 15)}</button>
      <button class="zoom-label" id="zfit" title="Fit width">${Math.round(S.zoom * 100)}%</button>
      <button class="tb-btn sm" id="zin" title="Zoom in">${icon('zoomin', 15)}</button>
    </div>
    <button class="tb-btn sm panel-tg${S.panels.right ? '' : ' off'}" id="tg-right" title="${S.panels.right ? 'Hide' : 'Show'} inspector (⌘])">${icon('sidebar-right', 16)}</button>`;
  bar.querySelector('#tg-left').addEventListener('click', () => togglePanel('left'));
  bar.querySelector('#tg-right').addEventListener('click', () => togglePanel('right'));
  const params = bar.querySelector('.params');
  for (const d of st.doc.param_defs) {
    const a = access(d.user);
    if (a === 'hide') continue;
    const c = paramControl(d, p);
    if (a === 'show') { c.classList.add('fixed'); c.querySelectorAll('select, input, button').forEach((x) => (x.disabled = true)); if (c.matches('button')) c.disabled = true; c.title = 'Set by the template'; }
    params.append(c);
  }
  // back to the template's choices
  const changed = st.doc.param_defs.some((d) => access(d.user) === 'change' && (p.values[d.name] ?? d.default) !== d.default);
  if (changed) {
    const rs = h(`<button class="tb-btn sm" title="Reset filters to their defaults">${icon('undo', 14)}</button>`);
    rs.addEventListener('click', () => setParams({ values: Object.fromEntries(st.doc.param_defs.map((d) => [d.name, access(d.user) === 'change' ? d.default : p.values[d.name] ?? d.default])) }));
    params.append(rs);
  }
  bar.querySelector('#grain').addEventListener('change', (e) => {
    const g = e.target.value;
    const patch = { grain: g };
    // a custom range starts as this year so far: 1 January to today
    if (g === 'custom') { const t = new Date(); patch.start = `${t.getFullYear()}-01-01`; patch.date = fmtD(new Date(Date.UTC(t.getFullYear(), t.getMonth(), t.getDate()))); }
    setParams(patch);
  });
  bar.querySelector('#compare').addEventListener('change', (e) => setParams({ compare: e.target.value }));
  bar.querySelector('#prev')?.addEventListener('click', () => setParams({ date: addMonths(p.date, -step) }));
  bar.querySelector('#next')?.addEventListener('click', () => setParams({ date: addMonths(p.date, step) }));
  bar.querySelector('#plabel').addEventListener('click', (e) => { if (periodA === 'change') periodPicker(e.currentTarget); });
  // a hidden period keeps the comparison when that one is still offered
  if (periodA === 'hide') bar.querySelectorAll('.period > .tl, #grain, #prev, #plabel, #next').forEach((x) => (x.hidden = true));
  bar.querySelector('#refresh').addEventListener('click', async () => { await api.refresh(); toast('Data reloaded'); });
  bar.querySelector('#zin').addEventListener('click', () => zoomBy(1.15));
  bar.querySelector('#zout').addEventListener('click', () => zoomBy(1 / 1.15));
  bar.querySelector('#zfit').addEventListener('click', () => { S.zoomFit = true; render(); });
  bar.querySelector('#grid')?.addEventListener('click', () => { S.showGrid = !S.showGrid; render(); });
  // overall fit status
  const total = st.pages.reduce((a, pg) => a + pg.issues.filter((i) => i.severity !== 'info').length, 0);
  const fs = bar.querySelector('.fit-summary');
  fs.innerHTML = total
    ? `<span class="fs warn">${icon('warn', 14)} ${total} layout issue${total === 1 ? '' : 's'}</span>`
    : `<span class="fs ok">${icon('ok', 14)} All pages fit</span>`;
  fs.addEventListener('click', () => {
    const pi = st.pages.findIndex((pg) => pg.issues.some((i) => i.severity !== 'info'));
    if (pi >= 0) { scrollToPage(pi); setTimeout(() => document.querySelector(`.fit-pill[data-pi="${pi}"]`)?.click(), 350); }
  });
}

/** One report parameter in the toolbar: a list, several choices, text, a number or a date. */
function paramControl(d, p) {
  const el = paramControlEl(d, p);
  el.dataset.param = d.name;
  return el;
}

function paramControlEl(d, p) {
  const st = S.st;
  const v = p.values[d.name] ?? d.default ?? 'All';
  const set = (x) => setParams({ values: { ...p.values, [d.name]: x } });
  const label = esc(d.label || d.name);
  const kind = d.kind || (d.options.length || d.source ? 'list' : 'text');
  const choices = (d.source ? st.param_options?.[d.name] : d.options) || [];
  // a choice's text: its label column (Брокерские счета for BROK), or the value itself
  const lbl = (c) => st.param_labels?.[d.name]?.[c] || c;
  if (kind === 'list' && d.multi) {
    const sel = v.split('\u001f').map((x) => x.trim()).filter((x) => x && x.toLowerCase() !== 'all');
    const b = h(`<button class="param param-multi">${label}<span class="pm-val">${sel.length === 0 ? 'All' : sel.length === 1 ? esc(lbl(sel[0])) : `${sel.length} selected`}</span>${icon('chevd', 12)}</button>`);
    b.addEventListener('click', () => {
      const pick = new Set(sel);
      const el = h(`<div class="menu pm-pop"><input class="pm-search" placeholder="Search"><div class="pm-list"></div><div class="pm-foot"><button class="btn sm" data-a="all">All</button><span class="grow"></span><button class="btn sm primary" data-a="ok">Apply</button></div></div>`);
      const list = el.querySelector('.pm-list');
      const draw = (q = '') => {
        list.replaceChildren(...choices.filter((c) => `${c} ${lbl(c)}`.toLowerCase().includes(q.toLowerCase())).slice(0, 300).map((c) => {
          const r = h(`<label class="pm-item"><input type="checkbox"${pick.has(c) ? ' checked' : ''}><span>${esc(lbl(c))}</span></label>`);
          r.querySelector('input').addEventListener('change', (e) => { e.target.checked ? pick.add(c) : pick.delete(c); });
          return r;
        }));
      };
      draw();
      const q = el.querySelector('.pm-search');
      q.addEventListener('input', () => draw(q.value));
      q.addEventListener('keydown', (e) => e.stopPropagation());
      el.querySelector('[data-a=all]').addEventListener('click', () => { closeLayer(); set('All'); });
      el.querySelector('[data-a=ok]').addEventListener('click', () => { closeLayer(); set(pick.size ? [...pick].join('\u001f') : 'All'); });
      popover(b, el);
      setTimeout(() => q.focus(), 0);
    });
    return b;
  }
  let el;
  if (kind === 'list') el = h(`<label class="param">${label}<select>${['All', ...choices.filter((o) => o !== 'All')].map((o) => `<option value="${esc(o)}"${o === v ? ' selected' : ''}>${esc(lbl(o))}</option>`).join('')}</select></label>`);
  else if (kind === 'date') el = h(`<label class="param">${label}<input type="date" value="${esc(/^\d{4}-\d\d-\d\d$/.test(v) ? v : '')}"></label>`);
  else if (kind === 'number') el = h(`<label class="param">${label}<input type="number" value="${esc(v === 'All' ? '' : v)}" placeholder="All"></label>`);
  else el = h(`<label class="param">${label}<input value="${esc(v === 'All' ? '' : v)}" placeholder="All"></label>`);
  const inp = el.querySelector('select, input');
  inp.addEventListener('change', () => set(inp.value || 'All'));
  inp.addEventListener('keydown', (e) => e.stopPropagation());
  return el;
}

function zoomBy(f) {
  S.zoomFit = false;
  S.zoom = clamp(S.zoom * f, 0.4, 3);
  render();
}

function periodPicker(anchor) {
  const p = S.st.doc.params;
  const d = parseD(p.date);
  let year = d.getUTCFullYear();
  const el = h(`<div class="picker"></div>`);
  const draw = () => {
    if (p.grain === 'custom') {
      el.innerHTML = `<div class="pk-custom"><label>From<input type="date" id="pk-s" value="${p.start || S.st.period.range.start}"></label><label>To<input type="date" id="pk-e" value="${p.date}"></label><button class="btn primary" id="pk-ok">Apply</button></div>`;
      el.querySelector('#pk-ok').addEventListener('click', () => { closeLayer(); setParams({ start: el.querySelector('#pk-s').value, date: el.querySelector('#pk-e').value }); });
      return;
    }
    if (p.grain === 'year') {
      const ys = [];
      for (let y = year - 7; y <= year + 1; y++) ys.push(y);
      el.innerHTML = `<div class="pk-grid y">${ys.map((y) => `<button data-y="${y}" class="${y === d.getUTCFullYear() ? 'on' : ''}">${y}</button>`).join('')}</div>`;
      el.querySelectorAll('[data-y]').forEach((b) => b.addEventListener('click', () => { closeLayer(); setParams({ date: `${b.dataset.y}-12-31` }); }));
      return;
    }
    const quarters = p.grain === 'quarter';
    el.innerHTML = `<div class="pk-nav"><button class="tb-btn sm" id="py">${icon('chevl', 14)}</button><b>${year}</b><button class="tb-btn sm" id="ny">${icon('chevr', 14)}</button></div>
      <div class="pk-grid ${quarters ? 'q' : 'm'}">${(quarters ? [1, 2, 3, 4] : MONTHS.map((_, i) => i)).map((v) => {
        const m = quarters ? (v - 1) * 3 : v;
        const on = year === d.getUTCFullYear() && (quarters ? Math.floor(d.getUTCMonth() / 3) === v - 1 : d.getUTCMonth() === m);
        return `<button data-m="${m}" class="${on ? 'on' : ''}">${quarters ? 'Q' + v : MONTHS[m]}</button>`;
      }).join('')}</div>`;
    el.querySelector('#py').addEventListener('click', () => { year--; draw(); });
    el.querySelector('#ny').addEventListener('click', () => { year++; draw(); });
    el.querySelectorAll('[data-m]').forEach((b) => b.addEventListener('click', () => {
      closeLayer();
      const m = +b.dataset.m + (quarters ? 2 : 0);
      const last = new Date(Date.UTC(year, m + 1, 0)).getUTCDate();
      setParams({ date: `${year}-${String(m + 1).padStart(2, '0')}-${String(last).padStart(2, '0')}` });
    }));
  };
  draw();
  popover(anchor, el, { align: 'center' });
}

// ---------------------------------------------------------------- home

function renderHome() {
  const home = $('#home');
  const signed = !!session();
  const show = !S.st?.doc || !!S.showHome;
  home.hidden = !show || !signed;
  $('#main').hidden = show || !S.st?.doc;
  $('#toolbar').hidden = show || !S.st?.doc;
  renderLogin();
  if (!show || !signed) return;
  // signed in: the areas the person's roles open; Design is the start page below
  renderAreas(home, { designHome });
}

/** Saves the widget studio's widget: the type it edits, or (a built-in one) as a new widget of yours. */
async function saveStudioWidget() {
  const doc = S.st?.doc;
  if (!doc?.widget_studio) return;
  const { saveWidgetType } = await import('./panels.js');
  const w = doc.pages[0]?.widgets[0];
  if (!w) return;
  // the pane's name and description go with it
  const val = (k) => document.querySelector(`.ws-in[data-k="${k}"]`)?.value.trim();
  const t = doc.theme.widget_types?.[doc.widget_studio];
  await saveWidgetType({ w, vm: null }, { update: doc.widget_studio !== 'new', about: t ? { name: val('name') || t.name, description: val('description') ?? t.description } : null });
}

/**
 * The Design area. Two views: **Templates** (the main one — what needs the designer, the published
 * reports and how they're used, recent templates, examples, help) and **Themes** (the gallery, with
 * which reports use each theme). The view picked is kept for the session.
 */
function designHome(host) {
  const view = ['themes', 'widgets'].includes(S.designView) ? S.designView : 'templates';
  const home = h(`<div class="ar-page dh dh-view-${view}"></div>`);
  host.append(home);
  const ago = (at) => {
    if (!at) return '';
    const days = Math.floor((new Date().setHours(0, 0, 0, 0) - new Date(at).setHours(0, 0, 0, 0)) / 864e5);
    if (days <= 0) return 'today';
    if (days === 1) return 'yesterday';
    if (days < 30) return `${days} days ago`;
    return new Date(at).toLocaleDateString(document.documentElement.lang === 'ru' ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'short', year: 'numeric' });
  };
  const fileName = (p) => String(p).split('/').pop().replace(/\.rtpl$/, '');
  const folder = (p) => String(p).split('/').slice(-2, -1)[0] || '';
  const actions = view === 'templates'
    ? `<button class="btn" data-a="open">${icon('folder', 14)} Open…</button><button class="btn primary" data-a="blank">${icon('plus', 14)} New template</button>`
    : view === 'widgets' ? ''
    : `<button class="btn primary" data-t="new">${icon('plus', 14)} New theme</button>`;
  home.innerHTML = `
    ${areaHead('design', view === 'templates' ? 'Design report templates — their pages, data and themes. Reports are made from them.' : view === 'widgets' ? 'The widgets library: every widget once, built-in and yours, with the themes that include it and the templates that use it.' : 'Themes give templates their look: colours, type, shapes and a design view for every kind of widget.', actions)}
    <div class="dh-tabs" role="tablist">
      <button class="dh-tab${view === 'templates' ? ' on' : ''}" data-v="templates">${icon('layout', 15)} Templates</button>
      <button class="dh-tab${view === 'widgets' ? ' on' : ''}" data-v="widgets">${icon('tiles', 15)} Widgets <span class="dh-n dh-nw"></span></button>
      <button class="dh-tab${view === 'themes' ? ' on' : ''}" data-v="themes">${icon('palette', 15)} Themes <span class="dh-n dh-nt"></span></button>
    </div>
    ${view === 'widgets' ? `
    <div class="dh-wbar"><span>Previews in</span><select class="dh-wtheme"></select><span class="dh-sub">Each widget once, grouped by kind: the built-in ones and yours. Click one to work on it in the widget studio; add widgets to a page from a template's <b>Widgets</b> panel.</span></div>
    <section class="ar-sec dh-wmine"><div class="ar-sec-t">Your widget types</div><div class="dh-wgrid"><div class="hint pad">Loading…</div></div></section>
    <section class="ar-sec dh-wbuilt"><div class="dh-wkinds"><div class="hint pad">Loading…</div></div></section>` : view === 'templates' ? `
    <div class="dh-att" hidden></div>
    <section class="ar-sec dh-pub" hidden><div class="ar-sec-t">Your templates <span class="dh-sub">a template is your working copy; publishing puts a copy in Reports · use in the last 30 days</span></div><div class="dh-table"></div></section>
    <section class="ar-sec"><div class="ar-sec-t">Start from</div>
      <div class="dh-start">
        <div class="dh-ex">
          <span class="dh-ex-ic">${icon('layout', 22)}</span>
          <span class="dh-ex-t"><b>Modern UHNWI report</b><span>Private-banking report in Russian: cover, contents, portfolio, results, benchmarks, accounts, holdings, forecast and trades.</span></span>
          <span class="dh-ex-a"><button class="btn sm primary" data-a="uhnwi_report">Open template</button><button class="btn sm" data-a="uhnwi_as_report" title="See it the way report users get it">Preview as a report</button><button class="btn sm" data-dupkind="uhnwi_report" title="A copy under a new name, in a theme you pick">Duplicate…</button></span>
        </div>
        <button class="dh-ex dh-blank" data-a="blank"><span class="dh-ex-ic">${icon('plus', 22)}</span><span class="dh-ex-t"><b>Blank template</b><span>An empty A4 landscape grid, set up from your settings</span></span></button>
      </div>
    </section>
    <section class="ar-sec"><div class="ar-sec-t">Help</div>
      <div class="dh-help">
        ${[['1-getting-started', 'book', 'Getting started', 'The designer in ten minutes'], ['5-charts', 'chart-bar', 'Charts', 'The Design tab, part by part'],
          ['9-data', 'database', 'Data', 'Sources, SQL, Python, fields'], ['3-pages-and-layout', 'layout', 'Pages & layout', 'Grid, fit, repeating pages'],
          ['13-templates-reports-and-permissions', 'shield', 'Publishing', 'Who gets a report, what they may change'], ['16-ai-assistant', 'sparkles', 'AI assistant', 'Build a page, restyle from a picture']]
          .map(([a, ic, t, d]) => `<button class="dh-hl" data-g="${a}">${icon(ic, 16)}<span><b>${t}</b><span>${d}</span></span></button>`).join('')}
      </div>
    </section>` : `
    <section class="ar-sec dh-thm">
      <div class="dh-filter seg"><button data-f="all" class="on">All</button><button data-f="mine">Your library</button><button data-f="built">Built-in</button><button data-f="used">In use</button></div>
      <div class="dh-table"><div class="hint pad">Loading…</div></div>
    </section>
    <section class="ar-sec"><div class="dh-help">
      <button class="dh-hl" data-g="12-themes">${icon('palette', 16)}<span><b>Themes</b><span>Design views, the studio, Typography</span></span></button>
      <button class="dh-hl" data-g="16-ai-assistant">${icon('sparkles', 16)}<span><b>Restyle from a reference</b><span>A picture, a GIF or a PDF</span></span></button>
    </div></section>`}`;
  const go = () => { S.page = 0; S.sel = null; S.preview = false; S.zoomFit = true; };
  const wire = (root) => {
    if (!root) return;
    root.querySelectorAll('[data-v]').forEach((b) => b.addEventListener('click', () => { S.designView = b.dataset.v; S.designTheme = ''; render(); }));
    root.querySelectorAll('[data-a]').forEach((b) => b.addEventListener('click', async () => {
      const a = b.dataset.a;
      go();
      if (a === 'open') await api.open();
      else await api.newDoc(a);
    }));
    root.querySelectorAll('[data-t]').forEach((b) => b.addEventListener('click', () => {
      go();
      if (b.dataset.t === 'new') return api.themeStudio('new');
      if (b.dataset.t === 'manage') return manageThemes();
      return api.themeStudio(b.dataset.t);
    }));
    root.querySelectorAll('[data-g]').forEach((b) => b.addEventListener('click', () => showGuide(b.dataset.g)));
    // a template kept only as its published report: opened from it, and shown (not left behind the home page)
    root.querySelectorAll('[data-edit]').forEach((b) => b.addEventListener('click', async (e) => { e.stopPropagation(); go(); const st = await call('auth', { cmd: 'edit_published', args: { id: b.dataset.edit } }); if (st) { S.showHome = false; setState(st); } }));
    root.querySelectorAll('[data-prev]').forEach((b) => b.addEventListener('click', async (e) => { e.stopPropagation(); go(); await api.newDoc(`pub:${b.dataset.prev}`); }));
    root.querySelectorAll('[data-path]').forEach((b) => b.addEventListener('click', async () => { go(); await api.openPath(b.dataset.path); }));
    root.querySelectorAll('[data-dup]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); const r = (lastData?.reports || []).find((x) => x.id === b.dataset.dup); duplicateTemplate({ report: b.dataset.dup }, r?.title, r?.theme); }));
    root.querySelectorAll('[data-dupfile]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); const f = (lastData?.templates || []).find((x) => x.path === b.dataset.dupfile); duplicateTemplate({ path: b.dataset.dupfile }, f?.title || fileName(b.dataset.dupfile), f?.theme); }));
    // publish from the list: the template opens, with the publish dialog over it
    root.querySelectorAll('[data-path-pub]').forEach((b) => b.addEventListener('click', async (e) => { e.stopPropagation(); go(); await api.openPath(b.dataset.pathPub); publishDialog(); }));
    root.querySelectorAll('[data-edit-pub]').forEach((b) => b.addEventListener('click', async (e) => { e.stopPropagation(); go(); const st = await call('auth', { cmd: 'edit_published', args: { id: b.dataset.editPub } }); if (st) { S.showHome = false; setState(st); publishDialog(); } }));
    // delete a template: always asked first, saying what goes (a published report too, and who loses it)
    root.querySelectorAll('[data-del-title]').forEach((b) => b.addEventListener('click', async (e) => {
      e.stopPropagation();
      const path = b.dataset.del, rid = b.dataset.delReport, title = b.dataset.delTitle;
      const rep = (lastData?.reports || []).find((x) => x.id === rid);
      const reach = rep?.reach || 0;
      const parts = [path && 'the template file', rid && 'its published report'].filter(Boolean).join(' and ');
      const body = `<p>Delete “${esc(title)}”? This removes ${parts}.</p>`
        + (rid ? `<p><b>Report users lose it:</b> ${reach === 1 ? '1 person has' : `${reach} people have`} it now${rep?.published ? '' : ' (it is unpublished)'}. Reports already exported stay as they are.</p>` : '')
        + `<p class="muted">Nothing is erased: ${path ? 'the file goes' : 'it goes'} to the <b>.deleted</b> folder in the templates library, where it can be recovered.</p>`;
      const ok = await modal({ title: 'Delete template', iconName: 'warn', iconKind: 'warn', body, buttons: [{ label: 'Cancel', value: false }, { label: rid ? 'Delete template and report' : 'Delete template', kind: 'danger', value: true }] });
      if (!ok) return;
      try {
        await call('auth', { cmd: 'delete_template', args: { path, report: rid } });
        toast(`Deleted “${title}”. It can be recovered from the .deleted folder in the templates library.`);
        render();
      } catch { /* the call says why */ }
    }));
    root.querySelectorAll('[data-dupkind]').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); duplicateTemplate({ kind: b.dataset.dupkind }, 'Modern UHNWI report', 'Modern UHNWI'); }));
    root.querySelectorAll('[data-theme-name]').forEach((b) => b.addEventListener('click', async (e) => { e.stopPropagation(); const t = (await themes).find((x) => x.name === b.dataset.themeName); if (t) { go(); api.themeStudio(t.id); } }));
  };
  let lastData = null;
  // a theme's name with its accent, a click opens it in the studio
  let themeList = [];
  const themeChip = (name) => {
    if (!name) return '<span class="muted">—</span>';
    const c = themeList.find((t) => t.name === name)?.colors || [];
    return `<button class="dh-chip" data-theme-name="${esc(name)}" title="Open “${esc(name)}” in the theme studio"><i style="background:${esc(c[0] || '#fff')};border-color:${esc(c[2] || '#999')}"></i>${esc(name)}</button>`;
  };
  /** A copy of a template under a new name, in a theme picked here; it opens as a new, unsaved template. */
  async function duplicateTemplate(src, title, theme) {
    const list = themeList.length ? themeList : await themes;
    const cur = list.find((t) => t.name === theme)?.id || '';
    const body = `<label class="dup-l">Name</label><input class="dup-name" value="${esc(`${title || 'Template'} — copy`)}" spellcheck="false">
      <label class="dup-l">Theme</label><div class="dup-themes">${list.map((t) => { const c = t.colors || []; return `<label class="dup-t${t.id === cur ? ' on' : ''}"><input type="radio" name="dup-theme" value="${esc(t.id)}"${t.id === cur ? ' checked' : ''}><span class="dh-sw" style="background:${esc(c[0] || '#fff')}"><i style="background:${esc(c[2] || '#999')}"></i><span class="dh-sw-dots">${c.slice(3, 7).map((x) => `<em style="background:${esc(x)}"></em>`).join('')}</span></span><b>${esc(t.name)}</b>${t.id === cur ? '<span>now</span>' : ''}</label>`; }).join('')}</div>
      <div class="hint">The copy is saved in your templates and opens. It isn't published: report users get it once you publish it. The original stays as it is.</div>`;
    const res = await modal({ title: 'Duplicate template', body, wide: true, iconName: 'duplicate',
      buttons: [{ label: 'Cancel', value: null }, { label: 'Duplicate', kind: 'primary', value: (box) => ({ title: box.querySelector('.dup-name').value, theme: box.querySelector('input[name=dup-theme]:checked')?.value || '' }) }],
      onOpen: (box) => {
        box.querySelectorAll('.dup-t input').forEach((i) => i.addEventListener('change', () => box.querySelectorAll('.dup-t').forEach((l) => l.classList.toggle('on', l.querySelector('input').checked))));
        const n = box.querySelector('.dup-name'); n.focus(); n.select();
      } });
    if (!res) return;
    go();
    try { setState(await call('auth', { cmd: 'duplicate_template', args: { ...src, title: res.title, theme: res.theme === cur ? '' : res.theme } })); toast(`“${res.title}” is saved in your templates. Publish it when it's ready.`); } catch { /* the call shows its error */ }
  }
  wire(home);

  const homeData = call('auth', { cmd: 'design_home', args: {} }, { quiet: true }).catch(() => null);
  const themes = api.themes().catch(() => []);
  themes.then((list) => { themeList = list || []; const n = home.querySelector('.dh-nt'); if (n) n.textContent = themeList.length || ''; });

  if (view === 'widgets') {
    const KIND = { kpi: 'KPI cards', table: 'Tables', bar: 'Bar & column charts', line: 'Line charts', area: 'Area charts', pie: 'Donuts', waterfall: 'Waterfalls', treemap: 'Treemaps', tiles: 'Tiles', heading: 'Headings', text: 'Text', comment: 'Notes', divider: 'Dividers', toc: 'Contents' };
    const sel = home.querySelector('.dh-wtheme');
    const draw = async () => {
      const mine = home.querySelector('.dh-wmine .dh-wgrid'), kinds = home.querySelector('.dh-wkinds');
      kinds.innerHTML = '<div class="hint pad">Loading…</div>';
      let g;
      try { g = await call('auth', { cmd: 'widget_gallery', args: { theme: S.designWidgetTheme || '' } }); } catch (e) { kinds.innerHTML = `<div class="hint pad">${esc(String(e?.message || e))}</div>`; return; }
      const nw = home.querySelector('.dh-nw'); if (nw) nw.textContent = g.items.length || '';
      S.wgItems = g.items;
      const open = async (it) => {
        go();
        try {
          const st = await call('auth', { cmd: 'widget_open', args: it.group === 'mine' ? { type: it.type, theme: S.designWidgetTheme || '' } : { kind: it.kind, view: it.view, theme: S.designWidgetTheme || '' } });
          // the studio comes to the front, as any document opened
          S.showHome = false; S.sel = 'w_studio'; S.zoomFit = true;
          document.dispatchEvent(new Event('doc-replaced'));
          setState(st);
        } catch (err) { toast(String(err?.message || err), 'error'); }
      };
      const thumb = (it) => {
        let svg = '';
        try { const r = widgetSvg(g.state, it.page, 0, { pad: 4 }); svg = r?.svg || r || ''; } catch { svg = ''; }
        const el = h(`<div class="dh-wthumb">${svg}</div>`);
        const sv = el.querySelector('svg'); if (sv) { sv.removeAttribute('width'); sv.removeAttribute('height'); sv.style.width = '100%'; sv.style.height = 'auto'; }
        return el;
      };
      const list = (names, empty) => (names?.length ? `<span class="dh-wlist" title="${esc(names.join('\n'))}">${names.slice(0, 3).map(esc).join(', ')}${names.length > 3 ? ` +${names.length - 3}` : ''}</span>` : `<span class="muted">${empty}</span>`);
      // one table per kind of widget: the built-in ones and yours together
      const byKind = {};
      for (const it of g.items) (byKind[it.kind] ||= []).push(it);
      const order = ['kpi', 'tiles', 'table', 'bar', 'line', 'area', 'pie', 'waterfall', 'treemap', 'heading', 'text', 'comment', 'divider', 'toc'];
      home.querySelector('.dh-wmine').hidden = true;
      kinds.innerHTML = '';
      const kindsSorted = Object.keys(byKind).sort((a, b) => (order.indexOf(a) + 1 || 99) - (order.indexOf(b) + 1 || 99));
      for (const k of kindsSorted) {
        const items = byKind[k].sort((a, b) => (a.group === 'mine') - (b.group === 'mine'));
        const mine = items.filter((x) => x.group === 'mine').length;
        const sec = h(`<div class="dh-wkind"><div class="dh-wkind-t">${esc(KIND[k] || k)} <span class="dh-sub">${items.length - mine} built-in${mine ? ` · ${mine} yours` : ''}</span></div>
          <div class="dh-table"><table class="dh-t dh-wt"><thead><tr><th class="dh-wth">Preview</th><th>Widget</th><th>Source</th><th>Themes</th><th>Used in</th><th></th></tr></thead><tbody></tbody></table></div></div>`);
        const tb = sec.querySelector('tbody');
        for (const it of items) {
          const tr = h(`<tr class="dh-wrow">
            <td class="dh-wtd"></td>
            <td><b>${it.group === 'mine' ? icon(it.icon || 'layers', 13) + ' ' : ''}${esc(it.name || it.view)}</b><span class="dh-desc dh-wdesc">${esc(it.description || '')}${it.group === 'mine' && it.from ? `${it.description ? ' · ' : ''}from “${esc(it.from)}”` : ''}</span></td>
            <td>${it.group === 'mine' ? `<span class="pill ok">Yours</span><span class="dh-desc">v${it.version}${it.by ? ` · ${esc(it.by)}` : ''}</span>` : '<span class="pill">Built-in</span>'}</td>
            <td>${it.group === 'mine' ? '<span class="dh-desc">any theme (it takes the theme’s colours and type)</span>' : `${list(it.themes, '—')}<span class="dh-desc">${it.themes?.length || 0} of ${g.themes} themes</span>`}</td>
            <td>${list(it.used, 'not used yet')}${it.used?.length ? `<span class="dh-desc">${it.used.length} template${it.used.length === 1 ? '' : 's'}</span>` : ''}</td>
            <td class="dh-acts"><button class="btn sm" data-w="open">Open</button>${it.group === 'mine' ? `<button class="btn sm icon danger-ghost" data-w="del" title="Delete from the library">${icon('trash', 13)}</button>` : ''}</td>
          </tr>`);
          tr.querySelector('.dh-wtd').append(thumb(it));
          tr.addEventListener('click', (e) => { if (!e.target.closest('[data-w="del"]')) open(it); });
          tr.querySelector('[data-w="del"]')?.addEventListener('click', async (e) => {
            e.stopPropagation();
            const ok = await modal({ title: 'Delete widget type', iconName: 'warn', iconKind: 'warn', body: `<p>Delete “${esc(it.name)}” from the library? Templates that use it keep their copy, so their widgets look the same.</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
            if (!ok) return;
            try { await call('auth', { cmd: 'widget_type_delete', args: { id: it.type } }); toast('Widget type deleted'); draw(); } catch (err) { toast(String(err?.message || err), 'error'); }
          });
          tb.append(tr);
        }
        kinds.append(sec);
      }
    };
    themes.then((list) => {
      const cur = S.designWidgetTheme || 'builtin:uhnwi';
      sel.innerHTML = (list || []).map((t) => `<option value="${esc(t.id)}"${t.id === cur ? ' selected' : ''}>${esc(t.name)}</option>`).join('');
      sel.addEventListener('change', () => { S.designWidgetTheme = sel.value; draw(); });
    });
    draw();
    return;
  }
  if (view === 'templates') {
    Promise.all([homeData, themes]).then(([d, tl]) => {
      lastData = d;
      themeList = tl || [];
      const reports = d?.reports || [];
      // filtered to one theme (from the Themes view's "Used by")
      const byTheme = S.designTheme || '';
      // by status: published first, then unpublished, drafts last (as listed within each)
      const rank = (t) => (!t.report ? 2 : t.report.published ? 0 : 1);
      const tpls = (d?.templates || []).filter((t) => !byTheme || t.theme === byTheme).map((t, i) => [t, i])
        .sort(([a, i], [b, j]) => rank(a) - rank(b) || i - j).map(([t]) => t);
      const att = [];
      for (const t of tpls) {
        const r = t.report, name = esc(t.title);
        const k = t.path ? `data-path-pub="${esc(t.path)}"` : `data-edit-pub="${esc(r?.id || '')}"`;
        if (r?.pending?.length) att.push(['database', `<span title="${esc(r.pending.join(', '))}">The Data area changed ${r.pending.length} ${r.pending.length === 1 ? 'source' : 'sources'} of “${name}”. Publish again to take the change in.</span>`, `<button class="btn sm" ${k}>Publish…</button>`]);
        if (r && t.changed) att.push(['upload', `“${name}” changed since version ${r.version} was published. Report users still get version ${r.version}.`, `<button class="btn sm" ${k}>Publish changes…</button>`]);
        if (r && !r.published) att.push(['eye-off', `“${name}” is unpublished: nobody sees it in Reports.`, `<button class="btn sm" ${k}>Publish…</button>`]);
        else if (r && !r.reach) att.push(['users', `“${name}” is published to nobody: its audience is empty.`, `<button class="btn sm" ${k}>Choose who…</button>`]);
        else if (r && !r.usage?.opened && !r.usage?.exported) att.push(['clock', `Nobody has used “${name}” in 30 days (${r.reach} ${r.reach === 1 ? 'person has' : 'people have'} it).`, '']);
      }
      const ab = home.querySelector('.dh-att');
      if (att.length) {
        ab.hidden = false;
        ab.innerHTML = `<div class="dh-att-t">${icon('bell', 14)} Needs attention <span class="ar-count">${att.length}</span></div>`
          + att.map(([ic, text, btn]) => `<div class="dh-att-i">${icon(ic, 14)}<span>${text}</span>${btn}</div>`).join('');
        wire(ab);
      }
      if (tpls.length) {
        const sec = home.querySelector('.dh-pub');
        sec.hidden = false;
        if (byTheme) {
          const chip = h(`<div class="dh-fchip">${icon('palette', 13)} Theme: <b>${esc(byTheme)}</b> · ${tpls.length} template${tpls.length === 1 ? '' : 's'}<button class="tb-btn sm" title="Show all templates">${icon('x', 12)}</button></div>`);
          chip.querySelector('button').addEventListener('click', () => { S.designTheme = ''; render(); });
          sec.querySelector('.ar-sec-t').after(chip);
        }
        const status = (t) => {
          const r = t.report;
          if (!r) return '<span class="pill">Draft</span><span class="dh-st">not published</span>';
          const p = r.published ? '<span class="pill ok">Published</span>' : '<span class="pill">Unpublished</span>';
          const ch = t.changed ? '<span class="pill warn" title="The template changed since it was published: report users get the published version">changed since</span>' : '';
          const dc = r.pending?.length ? `<span class="pill warn" title="${esc(r.pending.join(', '))}">${r.pending.length} data change${r.pending.length > 1 ? 's' : ''}</span>` : '';
          return p + ch + dc;
        };
        const where = (t) => (!t.path ? 'published copy · no template file yet' : t.in_library ? `${esc(fileName(t.path))}${t.modified ? ` · saved ${esc(ago(t.modified))}` : ''}` : `${esc(folder(t.path))}/${esc(fileName(t.path))}`);
        sec.querySelector('.dh-table').innerHTML = `<table class="dh-t"><thead><tr><th>Template</th><th>Theme</th><th>Status</th><th class="num">Version</th><th class="num">People</th><th class="num">Opened</th><th class="num">Exported</th><th class="num">Clients</th><th>Last used</th><th></th></tr></thead><tbody>
          ${tpls.map((t) => { const r = t.report; const open = t.path ? `data-path="${esc(t.path)}"` : `data-edit="${esc(r?.id || '')}"`; const pub = t.path ? `data-path-pub="${esc(t.path)}"` : `data-edit-pub="${esc(r?.id || '')}"`; return `<tr>
            <td><button class="dh-name" ${open} title="Open the template">${esc(t.title)}</button><span class="dh-desc" title="${esc(t.path || '')}">${where(t)}</span></td>
            <td>${themeChip(t.theme)}</td>
            <td><span class="dh-status">${status(t)}</span></td>
            <td class="num">${r ? r.version || 1 : '<span class="muted">—</span>'}</td>
            <td class="num">${r ? r.reach ?? 0 : '<span class="muted">—</span>'}</td><td class="num">${r ? r.usage?.opened || 0 : ''}</td><td class="num">${r ? r.usage?.exported || 0 : ''}</td><td class="num">${r ? r.usage?.clients || 0 : ''}</td>
            <td>${r?.usage?.last ? esc(ago(r.usage.last)) : '<span class="muted">—</span>'}</td>
            <td class="dh-acts">${!r || t.changed ? `<button class="btn sm primary" ${pub} title="Choose who gets it and publish">${!r ? 'Publish…' : 'Publish changes…'}</button>` : ''}<span class="dh-icons">`
              + `<button class="tb-btn" ${open} title="Open the template to change it">${icon('rename', 15)}</button>`
              + (r ? `<button class="tb-btn" ${pub} title="Audience: see or change who gets it">${icon('user', 15)}</button>` : '')
              + (r?.published ? `<button class="tb-btn" data-prev="${esc(r.id)}" title="Preview: open it the way report users get it">${icon('eye', 15)}</button>` : '')
              + `<button class="tb-btn" ${t.path ? `data-dupfile="${esc(t.path)}"` : `data-dup="${esc(r?.id || '')}"`} title="Duplicate: a copy under a new name, in a theme you pick">${icon('duplicate', 15)}</button>`
              + `<button class="tb-btn danger-ghost" data-del="${esc(t.path || '')}" data-del-report="${esc(r?.id || '')}" data-del-title="${esc(t.title)}" title="Delete…">${icon('trash', 15)}</button>`
              + `</span></td>
          </tr>`; }).join('')}</tbody></table>`;
        wire(sec);
      }
    });
    return;
  }

  // themes as a table: what each looks like (colours, type, cards), where it's used, and what can be done with it
  Promise.all([themes, homeData]).then(([list, d]) => {
    const usedBy = {};
    for (const t of d?.templates || []) if (t.theme) (usedBy[t.theme] ||= []).push(t);
    const ask = (title, value) => {
      const f = h(`<div class="form"><label>Name<input name="v" value="${esc(value)}"></label></div>`);
      setTimeout(() => { const i = f.querySelector('input'); i?.focus(); i?.select(); }, 30);
      return modal({ title, body: f, buttons: [{ label: 'Cancel', value: null }, { label: 'OK', kind: 'primary', value: (b) => b.querySelector('[name=v]').value.trim() || null }] });
    };
    const again = () => render();
    const CARDS = { outlined: 'Outlined', filled: 'Filled', shadow: 'Raised', plain: 'No cards' };
    const dot = (c, title) => (c ? `<i class="dh-dot" style="background:${esc(c)}" title="${esc(title)} ${esc(c)}"></i>` : '');
    const rows = (list || []).map((t) => {
      const c = t.colors || [];
      const [hf, bf] = t.fonts || [];
      const used = usedBy[t.name] || [];
      const pub = used.filter((x) => x.report?.published).length;
      return { t, used, html: `<tr data-id="${esc(t.id)}">
        <td><span class="dh-tname"><span class="dh-mini" style="background:${esc(c[0] || '#fff')}"><i style="background:${esc(c[2] || '#999')}"></i><u style="color:${esc(c[1] || '#000')};font-family:${esc(fontStack(hf || 'Inter'))}">Aa</u></span>
          <span><b>${esc(t.name)}</b><span class="dh-desc">${t.builtin ? 'Built-in' : `Your library${t.modified ? ` · saved ${esc(ago(t.modified))}` : ''}`}${t.restyled ? ' · restyled' : ''}</span></span></span></td>
        <td><span class="dh-cols">${dot(c[0], 'Paper')}${dot(c[1], 'Text')}${dot(c[2], 'Accent')}<span class="dh-gap"></span>${c.slice(3, 7).map((x, i) => dot(x, `Series ${i + 1}`)).join('')}<span class="dh-gap"></span>${dot(t.good, 'Gains')}${dot(t.bad, 'Losses')}</span></td>
        <td><span class="dh-font" style="font-family:${esc(fontStack(hf || 'Inter'))}">${esc(hf || '—')}</span>${bf && bf !== hf ? `<span class="dh-desc" style="font-family:${esc(fontStack(bf))}">${esc(bf)}</span>` : '<span class="dh-desc">headings and text</span>'}</td>
        <td>${esc(CARDS[t.card] || t.card || '—')}<span class="dh-desc">corners ${+(t.radius ?? 0).toFixed(1)} pt · ${t.chart_look === 'soft' ? 'soft charts' : 'clean charts'}</span></td>
        <td class="num">${t.views || 0}<span class="dh-desc">${t.kinds || 0} kinds</span></td>
        <td>${used.length ? `<button class="dh-usedn" data-usedby="${esc(t.name)}" title="All the templates that use it">${used.length} template${used.length === 1 ? '' : 's'}${pub ? ` · ${pub} published` : ''}${icon('chevd', 11)}</button><span class="dh-desc dh-names">${used.slice(0, 2).map((x) => esc(x.title)).join(', ')}${used.length > 2 ? ` and ${used.length - 2} more` : ''}</span>` : '<span class="muted">—</span>'}</td>
        <td class="dh-acts"><button class="btn sm" data-th="open" title="Open in the theme studio">Open</button><button class="btn sm icon" data-th="dup" title="Duplicate… — ${t.builtin ? 'a copy in your library, to make your own' : 'a copy under a new name'}">${icon('duplicate', 13)}</button>${t.builtin ? '' : `<button class="btn sm icon" data-th="rename" title="Rename">${icon('rename', 13)}</button><button class="btn sm icon danger-ghost" data-th="delete" title="Delete from your library">${icon('trash', 13)}</button>`}</td>
      </tr>` };
    });
    // your library first, then the built-in themes; those in use first within each
    rows.sort((a, b) => (a.t.builtin - b.t.builtin) || (!!b.used.length - !!a.used.length));
    const box = home.querySelector('.dh-thm .dh-table');
    const draw = (f) => {
      const shown = rows.filter((r) => f === 'all' || (f === 'mine' && !r.t.builtin) || (f === 'built' && r.t.builtin) || (f === 'used' && r.used.length));
      box.innerHTML = `<table class="dh-t"><thead><tr><th>Theme</th><th>Colours</th><th>Type</th><th>Cards & charts</th><th class="num">Views</th><th>Used by</th><th></th></tr></thead><tbody>${shown.map((r) => r.html).join('')
        || `<tr><td colspan="7"><div class="hint">${f === 'mine' ? 'Themes you save from the studio appear here. Start a new one, or duplicate a built-in theme to make your own.' : 'No themes here.'}</div></td></tr>`}</tbody></table>`;
      wire(box);
      box.querySelectorAll('[data-th]').forEach((b) => b.addEventListener('click', async (e) => {
        e.stopPropagation();
        const t = (list || []).find((x) => x.id === b.closest('tr').dataset.id);
        try {
          if (b.dataset.th === 'open') { go(); return api.themeStudio(t.id); }
          if (b.dataset.th === 'dup') { const n = await ask('Duplicate theme', `${t.name} copy`); if (n) { await api.themeOp('duplicate', t.id, n); toast(`“${n}” added to your library`); again(); } }
          if (b.dataset.th === 'rename') { const n = await ask('Rename theme', t.name); if (n && n !== t.name) { await api.themeOp('rename', t.id, n); toast('Theme renamed'); again(); } }
          if (b.dataset.th === 'delete') {
            const inUse = (usedBy[t.name] || []).length;
            const ok = await modal({ title: 'Delete theme', iconName: 'warn', iconKind: 'warn', body: `<p>Delete “${esc(t.name)}” from your library?${inUse ? ` ${inUse} template${inUse === 1 ? ' uses' : 's use'} it: they keep their own copy of the theme.` : ''}</p>`, buttons: [{ label: 'Cancel', value: false }, { label: 'Delete', kind: 'danger', value: true }] });
            if (ok) { await api.themeOp('delete', t.id); toast('Theme deleted'); again(); }
          }
        } catch (err) { toast(String(err?.message || err), 'error'); }
      }));
      box.querySelectorAll('tbody tr[data-id]').forEach((tr) => tr.addEventListener('dblclick', () => { go(); api.themeStudio(tr.dataset.id); }));
      // every template using a theme: a list to open one from, or the Templates view filtered to it
      box.querySelectorAll('[data-usedby]').forEach((b) => b.addEventListener('click', (e) => {
        e.stopPropagation();
        const name = b.dataset.usedby;
        const used = [...(usedBy[name] || [])].sort((x, y) => (!!y.report?.published - !!x.report?.published) || String(x.title).localeCompare(String(y.title), undefined, { numeric: true }));
        const el = h(`<div class="menu dh-usedpop">
          <div class="dh-usedpop-h"><b>${esc(name)}</b><span>${used.length} template${used.length === 1 ? '' : 's'}</span></div>
          ${used.length > 8 ? `<input class="pm-search dh-usedpop-q" placeholder="Find a template" spellcheck="false">` : ''}
          <div class="dh-usedpop-l">${used.map((x) => `<button class="dh-usedpop-i" ${x.path ? `data-path="${esc(x.path)}"` : `data-edit="${esc(x.report?.id || '')}"`}><span>${esc(x.title)}</span>${x.report ? (x.report.published ? `<span class="pill ok">v${x.report.version || 1}</span>` : '<span class="pill">Unpublished</span>') : '<span class="pill">Draft</span>'}</button>`).join('')}</div>
          <button class="btn sm dh-usedpop-all">${icon('layout', 13)} Show in Templates</button>
        </div>`);
        wire(el);
        el.querySelectorAll('[data-path],[data-edit]').forEach((x) => x.addEventListener('click', () => closeLayer()));
        const q = el.querySelector('.dh-usedpop-q');
        q?.addEventListener('input', () => { const t = q.value.trim().toLowerCase(); el.querySelectorAll('.dh-usedpop-i').forEach((i) => { i.hidden = !!t && !i.textContent.toLowerCase().includes(t); }); });
        q?.addEventListener('keydown', (e) => e.stopPropagation());
        setTimeout(() => q?.focus(), 30);
        el.querySelector('.dh-usedpop-all').addEventListener('click', () => { closeLayer(); S.designView = 'templates'; S.designTheme = name; render(); });
        popover(b, el);
      }));
    };
    draw('all');
    home.querySelectorAll('.dh-filter [data-f]').forEach((b) => b.addEventListener('click', () => {
      home.querySelectorAll('.dh-filter [data-f]').forEach((x) => x.classList.toggle('on', x === b));
      draw(b.dataset.f);
    }));
  });
}

// ---------------------------------------------------------------- export

/** Whether anything in the document shows the comparison period (else the control does nothing). */
function usesCompare(st) {
  const doc = st?.doc;
  if (!doc) return false;
  const dated = new Set((doc.datasets || []).filter((d) => d.date_field).map((d) => d.id));
  const sql = (doc.datasets || []).some((d) => /:prev_(start|end)\b/.test(d.source?.query || d.source?.sql || d.source?.code || ''));
  if (sql) return true;
  const has = (t) => typeof t === 'string' && t.includes('{{compare}}');
  return (doc.pages || []).some((p) => (p.widgets || []).some((w) => {
    const b = w.binding;
    if (b?.measures?.some((m) => ['prev', 'delta', 'pct'].includes(m.calc))) return true;
    if (has(w.title) || has(w.subtitle) || has(w.opts?.text) || has(w.opts?.viz?.note) || has(w.opts?.viz?.deltaText)) return true;
    if (w.kind === 'kpi' && b && dated.has(b.dataset)) { try { return !!vizOptions(st, w).delta; } catch { return false; } }
    return false;
  }));
}

async function doExport(format) {
  const st = S.st;
  if (!st?.doc) return;
  const issues = st.pages.reduce((a, pg) => a + pg.issues.filter((i) => i.severity !== 'info').length, 0);
  if (issues) {
    const go = await modal({
      title: 'Some pages need attention', iconName: 'warn', iconKind: 'warn',
      body: `<p>${issues} layout issue${issues === 1 ? '' : 's'} — content may be cut off or run past the page. Export anyway?</p>`,
      buttons: [{ label: 'Review issues', value: false }, { label: 'Export anyway', kind: 'primary', value: true }],
    });
    if (!go) { document.querySelector('.fit-summary .fs')?.click(); return; }
  }
  // pages hidden for this data (their show-if is empty) are left out
  const shown = st.doc.pages.map((_, i) => i).filter((i) => !st.pages[i]?.hidden);
  const pages = format === 'pdf'
    ? shown.map((i) => pageSvg(st, i, { exportMode: true }))
    : shown.map((i) => slide(st, i));
  try {
    const path = await call('export', { format, pages });
    // in the browser the file comes as a download
    if (path) actionToast(`${WEB ? 'Downloaded' : 'Exported'} ${path.split('/').pop()}`, WEB ? [] : [
      ['Open', () => call('reveal', { path, open: true })],
      ['Show in Finder', () => call('reveal', { path, open: false })],
    ]);
  } catch { /* toast shown */ }
}

function actionToast(msg, actions) {
  let host = document.querySelector('.toasts');
  if (!host) { host = h('<div class="toasts"></div>'); document.body.append(host); }
  const t = h(`<div class="toast ok"><span class="i">${icon('check', 16)}</span><span class="msg"></span><span class="acts"></span></div>`);
  t.querySelector('.msg').textContent = msg;
  for (const [label, fn] of actions) {
    const b = h(`<button class="toast-btn">${esc(label)}</button>`);
    b.addEventListener('click', () => { fn(); t.remove(); });
    t.querySelector('.acts').append(b);
  }
  host.append(t);
  setTimeout(() => { t.classList.add('out'); setTimeout(() => t.remove(), 220); }, 7000);
}

// ---------------------------------------------------------------- panels

const PANEL_KEY = 'quire.panels';
const PANEL_DEFAULT = { left: true, right: true, lw: 268, rw: 304, aw: 360, ai: false };
S.panels = (() => { try { return { ...PANEL_DEFAULT, ...JSON.parse(localStorage.getItem(PANEL_KEY) || '{}') }; } catch { return { ...PANEL_DEFAULT }; } })();
// the AI pane starts closed
S.panels.ai = false;

function applyPanels() {
  const m = $('#main');
  m.style.setProperty('--left-w', S.panels.lw + 'px');
  m.style.setProperty('--right-w', S.panels.rw + 'px');
  m.style.setProperty('--ai-w', S.panels.aw + 'px');
  m.classList.toggle('ai-open', !!S.panels.ai);
  m.classList.toggle('hide-left', !S.panels.left);
  m.classList.toggle('hide-right', !S.panels.right);
  try { localStorage.setItem(PANEL_KEY, JSON.stringify(S.panels)); } catch { /* storage unavailable */ }
}

function togglePanel(side) {
  S.panels[side] = !S.panels[side];
  applyPanels();
  renderToolbar();
  hydrateIcons();
}

function initSplitters() {
  for (const el of document.querySelectorAll('.splitter')) {
    const left = el.classList.contains('l');
    // l: the left panel, r: the inspector, a: the AI pane right of it
    const key = left ? 'lw' : el.classList.contains('a') ? 'aw' : 'rw';
    el.addEventListener('dblclick', () => { S.panels[key] = PANEL_DEFAULT[key]; applyPanels(); });
    el.addEventListener('pointerdown', (e) => {
      e.preventDefault();
      const start = e.clientX;
      const w0 = S.panels[key];
      el.classList.add('drag');
      document.body.classList.add('resizing');
      const move = (ev) => {
        const dx = ev.clientX - start;
        const max = Math.max(260, innerWidth * 0.45);
        S.panels[key] = Math.round(clamp(left ? w0 + dx : w0 - dx, key === 'aw' ? 280 : 200, max));
        applyPanels();
      };
      const up = () => {
        el.classList.remove('drag');
        document.body.classList.remove('resizing');
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    });
  }
  applyPanels();
}

// ---------------------------------------------------------------- right-click menus

/**
 * A menu for whatever was right-clicked. The canvas handles widgets itself;
 * everything else lands here. Developer tools only with ⌥ held.
 */
function onContextMenu(e) {
  if (e.defaultPrevented) return;
  e.preventDefault();
  const t = e.target;
  const at = { x: e.clientX, y: e.clientY };
  let items = [];
  const editable = t.closest('input:not([type=checkbox]):not([type=range]):not([type=color]), textarea, [contenteditable=true]');
  if (editable) items = textItems(editable);
  else if (S.st?.doc) items = docItems(t, e);
  else items = [{ label: 'Open…', icon: 'folder', hint: '⌘O', run: openFile }];
  if (e.altKey && !WEB) items.push({ sep: true }, { title: 'Developer' }, { label: 'Inspect element', icon: 'code', run: () => call('open_devtools', {}, { quiet: true }).catch(() => toast('Developer tools are not available in this build.', 'info')) });
  if (items.some((i) => i && !i.sep && !i.title)) menu(at, items);
}

/** Cut, copy, paste in text fields. */
function textItems(el) {
  const sel = el.isContentEditable ? String(getSelection()) : el.value?.slice(el.selectionStart, el.selectionEnd);
  const ro = el.readOnly || el.disabled;
  return [
    { label: 'Cut', icon: 'scissors', hint: '⌘X', disabled: ro || !sel, run: () => { el.focus(); document.execCommand('cut'); } },
    { label: 'Copy', icon: 'copy', hint: '⌘C', disabled: !sel, run: () => { el.focus(); document.execCommand('copy'); } },
    { label: 'Paste', icon: 'clipboard', hint: '⌘V', disabled: ro, run: async () => { el.focus(); try { const txt = await navigator.clipboard.readText(); document.execCommand('insertText', false, txt); } catch { toast('Use ⌘V to paste here.', 'info'); } } },
    { sep: true },
    { label: 'Select all', icon: 'check', hint: '⌘A', run: () => { el.focus(); el.select ? el.select() : document.execCommand('selectAll'); } },
  ];
}

const QUICK_KINDS = ['kpi', 'bar', 'table', 'text'];

function docItems(t, e) {
  const st = S.st;
  const studio = isTheme(), user = userMode(), pol = policy();
  const undo = [
    { label: 'Undo', icon: 'undo', hint: '⌘Z', disabled: !st.undo, run: api.undo },
    { label: 'Redo', icon: 'redo', hint: '⇧⌘Z', disabled: !st.redo, run: api.redo },
  ];
  // a page on the canvas, off any widget: add right here, or act on the page
  const sheet = t.closest('.sheet');
  if (sheet && !studio) {
    const pi = +sheet.dataset.pi;
    const page = st.doc.pages[pi];
    const c = cellAt(sheet, e.clientX, e.clientY);
    const add = (k) => {
      const w = newWidget(k);
      const spec = st.specs[k] || {};
      const ww = Math.min(w.w || spec.def_w || 4, st.page.cols);
      const rect = { x: Math.max(0, Math.min(c.col, st.page.cols - ww)), y: Math.max(0, c.row), w: ww, h: w.h || spec.def_h || 2 };
      edit({ op: 'add_widget', page: page.id, widget: w, at: rect }).then((s2) => s2 && selectNewest(s2, page.id));
    };
    const canAdd = !user || pol.add;
    return [
      ...(canAdd ? [{ title: 'Add here' }, ...KINDS.filter(([k]) => QUICK_KINDS.includes(k)).map(([k, name, ic]) => ({ label: name, icon: ic, run: () => add(k) })),
        { label: 'More widgets…', icon: 'plus', run: () => { S.leftTab = 'widgets'; render(); } }, { sep: true }] : []),
      { title: `Page ${pi + 1} · ${page.title}` },
      ...pageItems(pi),
      { label: 'Page settings…', icon: 'sliders', run: () => renamePage(pi) },
    ];
  }
  const pageHead = t.closest('.page-head');
  if (pageHead && !studio) {
    const pi = +pageHead.closest('.page-wrap').dataset.pi;
    return [{ title: `Page ${pi + 1}` }, ...pageItems(pi), { sep: true }, { label: 'Page settings…', icon: 'sliders', run: () => renamePage(pi) }];
  }
  // the left panel: the page, widget type or source under the pointer
  const pg = t.closest('.pg-item');
  if (pg) { const pi = +pg.dataset.pi; return [{ title: `Page ${pi + 1}` }, { label: 'Go to page', icon: 'page', run: () => { S.page = pi; scrollToPage(pi); render(); } }, ...pageItems(pi)]; }
  const kind = t.closest('.kind[data-k]');
  if (kind) {
    const k = kind.dataset.k;
    return [{ label: `Add to page ${S.page + 1}`, icon: 'plus', run: async () => { const page = currentPageId(); const s2 = await edit({ op: 'add_widget', page, widget: newWidget(k), at: null }); selectNewest(s2, page); } }];
  }
  const more = t.closest('.gal-item, .ds-item')?.querySelector('.gi-more');
  if (more) { more.click(); return []; }
  // the inspector
  if (t.closest('#right')) {
    const secs = [...document.querySelectorAll('#right .insp-sec')];
    return [
      { label: 'Find a setting…', icon: 'search', hint: '⌘F', run: () => { const i = document.querySelector('.insp-search input'); i?.focus(); i?.select(); } },
      { sep: true },
      { label: 'Expand all sections', icon: 'chevd', disabled: !secs.length, run: () => secs.forEach((s2) => s2.classList.contains('closed') && s2.querySelector('.insp-title').click()) },
      { label: 'Collapse all sections', icon: 'chevr', disabled: !secs.length, run: () => secs.forEach((s2) => !s2.classList.contains('closed') && s2.querySelector('.insp-title').click()) },
      ...(S.sel ? [{ sep: true }, { label: 'Deselect', icon: 'x', hint: 'Esc', run: () => { S.sel = null; render(); } }] : []),
    ];
  }
  // the canvas around the pages, the toolbars and anything else
  return [
    ...undo,
    { sep: true },
    !studio && { label: 'Zoom in', icon: 'zoomin', hint: '⌘+', run: () => zoomBy(1.15) },
    !studio && { label: 'Zoom out', icon: 'zoomout', hint: '⌘−', run: () => zoomBy(1 / 1.15) },
    !studio && { label: 'Fit width', icon: 'autofit', hint: '⌘0', run: () => { S.zoomFit = true; render(); } },
    !studio && !user && { label: 'Grid', checked: !!S.showGrid, run: () => { S.showGrid = !S.showGrid; render(); } },
    !studio && pol.pages && { sep: true },
    !studio && pol.pages && { label: 'Add page at the end', icon: 'plus', run: () => edit({ op: 'add_page', after: st.doc.pages[st.doc.pages.length - 1].id }) },
    { sep: true },
    { label: 'Save', icon: 'save', hint: '⌘S', run: () => api.save(false) },
    !studio && { label: 'Export PDF', icon: 'file-pdf', hint: '⌘E', run: () => doExport('pdf') },
  ];
}

// ---------------------------------------------------------------- keyboard

function onKey(e) {
  // an SQL workspace in front has its own keys
  if (document.body.classList.contains('sql-active')) return;
  const tag = e.target.tagName;
  const typing = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || e.target.isContentEditable;
  if (mod(e) && e.key.toLowerCase() === 's') { e.preventDefault(); if (S.st?.doc?.widget_studio) saveStudioWidget(); else if (S.st?.doc) api.save(e.shiftKey); return; }
  if (mod(e) && e.key.toLowerCase() === 'o') { e.preventDefault(); openFile(); return; }
  if (mod(e) && e.key.toLowerCase() === 'i' && !typing) { e.preventDefault(); toggleAI(); return; }
  if (mod(e) && e.key === ',') { e.preventDefault(); settingsDialog(); return; }
  if (e.key === 'F1' || (mod(e) && e.key === '?')) { e.preventDefault(); showGuide(); return; }
  // ⌘[ folds the left part of where you are: a document's left panel, an area page's own menu or list (the sidebar stays)
  if (mod(e) && e.code === 'BracketLeft') {
    e.preventDefault();
    if (stageMode() === 'doc' && S.st?.doc) togglePanel('left');
    else toggleAreaFold();
    return;
  }
  // ⌘] folds the right/below part of an area page (the client list: the details opened under the rows)
  if (mod(e) && e.code === 'BracketRight' && stageMode() !== 'doc') { e.preventDefault(); document.dispatchEvent(new CustomEvent('quire:fold-right')); return; }
  if (!S.st?.doc) return;
  if (mod(e) && e.code === 'BracketRight') { e.preventDefault(); togglePanel('right'); return; }
  if (mod(e) && e.key.toLowerCase() === 'e' && !isTheme()) { e.preventDefault(); doExport('pdf'); return; }
  // ⌘F: find a setting in the inspector
  if (mod(e) && e.key.toLowerCase() === 'f') {
    e.preventDefault();
    if (!S.panels.right) togglePanel('right');
    const inp = document.querySelector('.insp-search input');
    if (inp) { inp.focus(); inp.select(); }
    return;
  }
  if (typing) return;
  if (mod(e) && e.key.toLowerCase() === 'z') { e.preventDefault(); e.shiftKey ? api.redo() : api.undo(); return; }
  if (mod(e) && e.key.toLowerCase() === 'y') { e.preventDefault(); api.redo(); return; }
  if (mod(e) && (e.key === '=' || e.key === '+')) { e.preventDefault(); zoomBy(1.15); return; }
  if (mod(e) && e.key === '-') { e.preventDefault(); zoomBy(1 / 1.15); return; }
  if (mod(e) && e.key === '0') { e.preventDefault(); S.zoomFit = true; render(); return; }
  // theme studio: Backspace goes back from an object type to the overview
  if (isTheme() && e.key === 'Backspace' && !e.target.closest?.('input, textarea, select, [contenteditable]') && S.studioKind && S.studioKind !== 'overview') {
    e.preventDefault();
    import('./studio.js').then((m) => m.backToOverview());
    return;
  }
  const f = S.sel ? findWidget(S.sel) : null;
  if (!f) return;
  const pol = policy();
  if (e.key === 'Escape') { S.sel = null; render(); return; }
  // design views are deleted from their card or the inspector, with a confirmation
  if (isTheme()) return;
  if ((e.key === 'Backspace' || e.key === 'Delete') && canEdit(f.w) && (pol.remove || (f.w.kind === 'comment' && pol.comment))) {
    e.preventDefault();
    edit({ op: 'remove_widget', id: f.w.id });
    return;
  }
  if (mod(e) && e.key.toLowerCase() === 'd' && canEdit(f.w) && pol.arrange) { e.preventDefault(); edit({ op: 'duplicate_widget', id: f.w.id }); return; }
  if (e.key === 'Enter' && ['text', 'comment', 'heading'].includes(f.w.kind) && canEdit(f.w)) { e.preventDefault(); startInlineEdit(f); return; }
  const arrows = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  if (arrows[e.key] && canEdit(f.w) && pol.arrange) {
    e.preventDefault();
    const [dx, dy] = arrows[e.key];
    const r = { x: f.w.x, y: f.w.y, w: f.w.w, h: f.w.h };
    const spec = S.st.specs[f.w.kind];
    if (e.shiftKey) { r.w = clamp(r.w + dx, spec.min_w, S.st.page.cols - r.x); r.h = clamp(r.h + dy, spec.min_h, spec.max_h); }
    else { r.x = clamp(r.x + dx, 0, S.st.page.cols - r.w); r.y = Math.max(0, r.y + dy); }
    edit({ op: 'set_rect', id: f.w.id, rect: r });
  }
}

// ---------------------------------------------------------------- boot

const railHooks = { onGuide: () => showGuide(), onSettings: () => settingsDialog() };

async function boot() {
  await Promise.all([400, 500, 600, 700].map((w) => document.fonts.load(`${w} 12px Inter`, 'Aa€Жж▲')));
  initCanvas($('#canvas'));
  // an empty number box shows the value in force as its placeholder (e.g. 18): its arrows, ↑ / ↓ and the
  // spinner start from that value, not from the box's minimum
  const fromShown = (e) => {
    const el = e.target;
    if (!(el instanceof HTMLInputElement) || el.type !== 'number' || el.value !== '' || el.disabled) return;
    if (e.type === 'keydown' && e.key !== 'ArrowUp' && e.key !== 'ArrowDown') return;
    // a click on the box itself (to type) leaves it empty; only the spinner at its right end steps
    if (e.type === 'mousedown' && e.offsetX < el.clientWidth - 22) return;
    const m = String(el.placeholder || '').match(/-?\d+(?:[.,]\d+)?/);
    if (m) el.value = m[0].replace(',', '.');
  };
  window.addEventListener('keydown', fromShown, true);
  window.addEventListener('mousedown', fromShown, true);
  initLeft($('#left'));
  initRight($('#right'));
  initSplitters();
  onChange(() => {
    setLocale(S.st?.doc?.theme?.locale, S.st?.formats || S.st?.doc?.formats);
    renderTitle();
    renderHome();
    renderRail(railHooks);
    renderDataTabs();
    if (!S.st?.doc || S.showHome) return;
    renderToolbar();
    renderLeft();
    renderCanvas();
    renderRight();
    hydrateIcons();
    // zoom label reflects the fitted zoom
    const zl = $('#zfit');
    if (zl) zl.textContent = Math.round(S.zoom * 100) + '%';
    ensureFonts(S.st);
  });
  document.addEventListener('pagechange', () => { renderLeft(); document.querySelectorAll('.page-wrap').forEach((p) => p.classList.toggle('current', +p.dataset.pi === S.page)); });
  document.addEventListener('save-to-gallery', (e) => saveToGallery(e.detail));
  document.addEventListener('rename-page', (e) => renamePage(e.detail));
  document.addEventListener('open-settings', (e) => {
    // "an administrator must allow it": administrators go straight to Admin → AI
    if (e.detail === 'ai-bank' && session()?.roles?.admin) { S.adminTab = 'ai'; goArea('admin'); return; }
    settingsDialog(e.detail === 'ai-bank' ? 'ai' : e.detail || 'region');
  });
  document.addEventListener('go-area', (e) => goArea(e.detail));

  // time in a document: every half minute the window is in front and someone typed, clicked or
  // scrolled in the last two minutes, the document in front gets the half minute (Admin → Activity)
  let lastInput = Date.now();
  for (const ev of ['pointerdown', 'keydown', 'wheel', 'pointermove']) window.addEventListener(ev, () => { lastInput = Date.now(); }, { passive: true, capture: true });
  setInterval(() => {
    if (document.visibilityState !== 'visible' || !document.hasFocus() || Date.now() - lastInput > 120000) return;
    // the document in front, or an administrator's time in Admin
    const area = stageMode() !== 'doc' && currentArea() === 'admin' ? 'admin' : '';
    if (!area && (stageMode() !== 'doc' || !S.st?.doc)) return;
    call('activity_ping', { seconds: 30, area }, { quiet: true }).catch(() => {});
  }, 30000);
  setWhere(() => ({ mode: stageMode(), area: currentArea() }));
  // the design assistant, right of the inspector (⌘I in an open template)
  const design = designAssistant({ pane: $('#aipane'), onToggle: (open) => { S.panels.ai = open; applyPanels(); paintAIButton(); } });
  registerAI('doc:design', design);
  // the report user's assistant, in the same place for a report
  const reportAI = reportAssistant({ pane: $('#aipane-r'), onToggle: (open) => { S.panels.ai = open; applyPanels(); paintAIButton(); } });
  registerAI('doc:report', reportAI);
  // a pane of the other kind closes when the document in front changes kind
  onChange(() => {
    if (!design.available() && design.isOpen()) design.close();
    if (!reportAI.available() && reportAI.isOpen()) reportAI.close();
  });
  document.addEventListener('signed-out', () => closeAllViews());
  // a workspace on the replaced document's data goes with it; the Data area's stay
  document.addEventListener('doc-replaced', () => closeAllViews({ keepGlobal: true }));
  document.addEventListener('views-changed', () => { renderRail(railHooks); renderDataTabs(); renderTitle(); renderHome(); });
  // opening another document keeps this one open (in the rail), so there is nothing to confirm
  S.beforeReplace = async () => true;
  window.addEventListener('keydown', onKey);
  document.addEventListener('contextmenu', onContextMenu);
  // the interface language from this computer's settings
  call('get_settings', {}, { quiet: true }).then((st) => setAppLanguage(st?.ui_lang || 'en', render)).catch(() => {});
  setState(await call('get_state'));
}

boot();
