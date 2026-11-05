// The user guide's screenshots (01–48) in the current app, in the Quire theme: the Design home, the workspace,
// menus, the inspector's parts, tables, contents, data, settings, parameters and the theme studio.
// node dev/shots/guide-all.mjs ui/help/img [only, e.g. 14,15]   (49–58: guide-ai, design-ai, restyle-ai, admin)
import { send, ev, shot, sleep, chrome } from './cdp.mjs';
const only = (process.argv[3] || '').split(',').filter(Boolean);
const want = (n) => !only.length || only.some((o) => n.startsWith(o));
await send('Emulation.setDeviceMetricsOverride', { width: 1440, height: 900, deviceScaleFactor: 1, mobile: false });
const failed = [];

/** A fresh session as a designer, the interface in English, at the Design home. */
async function start() {
  await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
  await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
  await send('Page.reload', {}); await sleep(3000);
  await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3500);
  await ev(`(async () => { const i = await import('/ui/i18n.js'); const m = await import('/ui/state.js'); i.setAppLanguage('en', m.render); })()`); await sleep(800);
  await ev(`(async () => { window.G = { s: await import('/ui/state.js'), p: await import('/ui/panels.js'), c: await import('/ui/canvas.js') }; return 1; })()`);
}
/** A document open, in Quire. */
async function openDoc(kind = 'uhnwi_report') {
  await ev(`(async () => { const s = G.s; s.S.showHome = false; s.setState(await s.call('new_doc', { kind: ${JSON.stringify(kind)} })); s.setState(await s.call('apply_theme', { id: 'builtin:quire' })); })()`); await sleep(4500);
}
/** Back to a plain workspace: nothing selected, no menus or dialogs, the Widgets tab. */
async function reset(leftTab = 'widgets') {
  // back to the Design area's open document (a data source opens in the Data area)
  await ev(`(() => { document.dispatchEvent(new CustomEvent('go-area', { detail: 'design' })); G.s.S.showHome = false; G.s.render(); })()`); await sleep(600);
  await ev(`(() => { for (let i = 0; i < 3; i++) document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); document.querySelectorAll('.modal-back, .layer, .menu').forEach((n) => n.remove()); })()`);
  await ev(`(() => { const S = G.s.S; S.colEdit = null; S.inspQuery = ''; S.sel = null; S.preview = false; S.inspTab = {}; S.leftTab = ${JSON.stringify(leftTab)}; S.zoomFit = true; G.s.render(); })()`); await sleep(700);
}
/** The first widget a test picks: its id and page. */
const find = (test) => ev(`(() => { const d = G.s.S.st.doc; for (const [pi, p] of d.pages.entries()) { const w = p.widgets.find(${test}); if (w) return JSON.stringify({ id: w.id, pi }); } return 'null'; })()`).then((r) => JSON.parse(r || 'null'));
/** Select a widget, open an inspector tab and show it on the page. */
async function select(w, tab = 'design') {
  await ev(`(() => { const S = G.s.S; S.sel = ${JSON.stringify(w.id)}; S.inspTab = { ...(S.inspTab || {}), widget: ${JSON.stringify(tab)} }; G.s.render(); })()`); await sleep(500);
  await ev(`document.querySelector('.hit[data-id="${w.id}"]')?.scrollIntoView({ block: 'center' })`); await sleep(700);
}
const page = async (pi) => { await ev(`document.querySelector('.sheet[data-pi="${pi}"]')?.scrollIntoView({ block: 'start' })`); await sleep(800); };
const pageIndex = (title) => ev(`G.s.S.st.doc.pages.findIndex((p) => p.title === ${JSON.stringify(title)})`);
/** Open one inspector section (and fold the others), scrolled to its top. */
async function section(name) {
  await ev(`(() => { const secs = [...document.querySelectorAll('.insp .insp-sec')]; for (const s of secs) { const me = s.dataset.sec === ${JSON.stringify(name)}; if (me === s.classList.contains('closed')) s.querySelector('.insp-title').click(); } const t = secs.find((s) => s.dataset.sec === ${JSON.stringify(name)}); t?.scrollIntoView({ block: 'start' }); return !!t; })()`); await sleep(500);
}
const menuBar = async (name) => { await ev(`[...document.querySelectorAll('.menubar .mb-item')].find((b) => b.textContent.trim() === ${JSON.stringify(name)})?.click()`); await sleep(600); };
const clickText = async (sel, text) => ev(`(() => { const b = [...document.querySelectorAll(${JSON.stringify(sel)})].find((x) => x.textContent.trim().startsWith(${JSON.stringify(text)})); b?.click(); return !!b; })()`);
async function rightClick(x, y) {
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'right', buttons: 2, clickCount: 1 });
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'right', buttons: 0, clickCount: 1 }); await sleep(700);
}
const rect = async (sel) => JSON.parse(await ev(`JSON.stringify(document.querySelector(${JSON.stringify(sel)})?.getBoundingClientRect() || null)`));
async function take(name, fn) {
  if (!want(name)) return;
  try { await fn(); await shot(name); console.log('✓', name); } catch (e) { failed.push(name); console.log('✗', name, String(e).slice(0, 160)); }
}

await start();
// a template of their own in the library, in Quire, beside the published example
await ev(`(async () => { const s = G.s; await s.call('auth', { cmd: 'duplicate_template', args: { kind: 'uhnwi_report', title: 'Client portfolio review', theme: 'builtin:quire' } }); s.S.showHome = true; s.render(); })()`); await sleep(2500);
await take('01-home', async () => { await sleep(500); });
await openDoc();
const overview = await pageIndex('Обзор портфеля');
const brief = await pageIndex('Портфель — кратко');
const kpiCard = await find(`(w) => w.kind === 'kpi' && w.variant === 'breakdown'`) || await find(`(w) => w.kind === 'kpi' && /card/.test(w.variant || '')`);
const monthly = await find(`(w) => w.kind === 'bar' && (w.binding?.measures || []).length > 1`);
const donut = await find(`(w) => w.kind === 'pie'`);
const products = await find(`(w) => w.kind === 'table' && /продукт/i.test(w.title || '')`);
const toc = await find(`(w) => w.kind === 'toc'`);

await take('02-workspace', async () => { await reset(); await page(brief); });
await take('03-menu-file', async () => { await reset(); await page(brief); await menuBar('File'); });
await take('04-menu-insert', async () => { await reset(); await page(brief); await menuBar('Insert'); });
await take('05-menu-view', async () => { await reset(); await page(brief); await menuBar('View'); });
await take('06-rightclick-page', async () => {
  await reset(); await page(overview);
  const s = await rect(`.sheet[data-pi="${overview}"]`); await rightClick(s.x + s.width * 0.5, s.y + s.height * 0.93);
});
await take('07-rightclick-widget', async () => {
  await reset(); await select(monthly);
  const r = await rect(`.hit[data-id="${monthly.id}"]`); await rightClick(r.x + r.width * 0.5, r.y + r.height * 0.55);
});
await take('08-inspector-search', async () => { await reset(); await select(monthly); await ev(`G.s.S.inspQuery = 'legend'; G.p.renderRight(); 1`); await sleep(600); });
await take('09-shortcuts', async () => { await reset(); await page(brief); await menuBar('Help'); await clickText('.menu .mi, .menu button', 'Keyboard shortcuts'); await sleep(800); });
await take('10-pages', async () => { await reset('pages'); await page(brief); });
await take('11-page-settings', async () => { await reset(); await page(brief); await ev(`document.dispatchEvent(new CustomEvent('rename-page', { detail: ${brief} }))`); await sleep(900); });
await take('12-fit-advisor', async () => {
  await reset();
  // a page whose widgets don't all fit (else any page with something to say)
  const pi = await ev(`(() => { const ps = G.s.S.st.pages; const i = ps.findIndex((p) => (p.issues || []).some((x) => !/gaps/.test(x.message || ''))); return i >= 0 ? i : ps.findIndex((p) => (p.issues || []).length); })()`);
  await page(pi); await ev(`document.querySelector('.fit-pill[data-pi="${pi}"]')?.click()`); await sleep(900);
});
await take('13-cut-off', async () => {
  await reset();
  // a text box made too short for its words: the page reports it, the widget is outlined
  const w = await find(`(w) => w.kind === 'text' && String(w.opts?.text || '').length > 120 && w.h >= 2`);
  if (!w) throw new Error('no text to cut');
  await ev(`(async () => { const x = G.s.findWidget(${JSON.stringify(w.id)}).w; await G.s.edit({ op: 'set_rect', id: x.id, rect: { x: x.x, y: x.y, w: x.w, h: 1 } }); return 1; })()`); await sleep(900);
  await select(w, 'layout');
});
await ev(`(async () => { await G.s.api.undo(); return 1; })()`); await sleep(800);
await take('14-kpi-design', async () => { await reset(); await select(kpiCard); await section('Value'); });
await take('15-card-frame', async () => { await reset(); await select(kpiCard); await section('Card & title'); });
await take('16-chart-design', async () => { await reset(); await select(monthly); await section('Data labels'); });
await take('17-chart-type', async () => { await reset(); await select(monthly); await section('Type & view'); });
await take('18-chart-icons', async () => { await reset(); await select(donut); await section('Icons'); });
await take('19-icon-picker', async () => { await reset(); await select(donut); await section('Icons'); await ev(`document.querySelector('.insp-sec[data-sec="Icons"] .icon-pick')?.click()`); await sleep(600); await clickText('.ip-tabs button', 'Icons'); await sleep(600); });
await take('20-icon-flags', async () => { await reset(); await select(donut); await section('Icons'); await ev(`document.querySelector('.insp-sec[data-sec="Icons"] .icon-pick')?.click()`); await sleep(600); await clickText('.ip-tabs button', 'Flags'); await sleep(600); });
await take('21-table-design', async () => { await reset(); await select(products); await section('Lines & rows'); });
await take('22-best-worst', async () => { await reset(); await select(products); await section('Highlights'); });
await take('23-column-bar', async () => {
  await reset(); await select(products);
  const key = await ev(`(() => { const w = G.s.findWidget(${JSON.stringify(products.id)}).w; const cols = w.opts?.table?.cols || {}; return Object.keys(cols).find((k) => cols[k]?.bar?.on) || (w.binding?.measures || [])[0]?.id; })()`);
  await ev(`G.s.S.colEdit = { id: ${JSON.stringify(products.id)}, key: ${JSON.stringify(key)} }; G.p.renderRight(); 1`); await sleep(700);
});
await take('24-column-icons', async () => {
  await reset();
  const deals = await find(`(w) => G.s.S.st.doc.datasets.find((d) => d.id === w.binding?.dataset)?.name === 'deals' || w.binding?.dataset === 'deals'`);
  await select(deals); await ev(`G.s.S.colEdit = { id: ${JSON.stringify(deals.id)}, key: 'm1' }; G.p.renderRight(); 1`); await sleep(600);
  await ev(`document.querySelector('.insp-sec[data-sec="Icons"]')?.scrollIntoView({ block: 'start' })`); await sleep(400);
});
await take('25-holdings-best-worst', async () => {
  await reset();
  const pi = await ev(`G.s.S.st.doc.pages.findIndex((p) => /состав/.test(p.title) && !p.auto)`);
  await page(pi);
});
await take('26-contents-design', async () => { await reset(); await select(toc); });
await take('27-contents-entries', async () => { await reset(); await select(toc, 'data'); });
await take('28-contents-assistant', async () => { await reset(); await select(toc, 'data'); await clickText('.insp button', 'Fill in'); await sleep(1200); });
await take('29-data-tab', async () => { await reset('data'); await page(overview); });
await take('30-dataset-editor', async () => { await reset('data'); await page(overview); await ev(`document.querySelector('.ds-item')?.click()`); await sleep(1500); });
await take('31-theme-menu', async () => { await reset(); await page(overview); await menuBar('Theme'); await sleep(500); });
await take('32-document-formats', async () => { await reset(); await page(overview); await ev(`G.s.S.inspTab = { doc: 'doc' }; G.p.renderRight(); 1`); await sleep(500); await section('Number & date formats'); });
await take('33-access', async () => { await reset(); await page(overview); await ev(`G.s.S.inspTab = { doc: 'access' }; G.p.renderRight(); 1`); await sleep(600); });
await take('34-preview-as-user', async () => { await reset(); await ev(`G.s.S.preview = true; G.s.render(); 1`); await sleep(1200); await page(overview); });
await take('35-export', async () => { await reset(); await page(overview); await clickText('.topbar button, header button, .tb button', 'Export'); await sleep(700); });
for (const [name, part] of [['36-settings-region', 'region'], ['37-settings-numbers', 'numbers'], ['38-settings-new', 'new']]) {
  await take(name, async () => { await reset(); await page(overview); await ev(`document.dispatchEvent(new CustomEvent('open-settings', { detail: ${JSON.stringify(part)} }))`); await sleep(1200); });
}

// the sales sample: parameters, calculated fields, an SQL source
await openDoc('sample');
await take('39-parameters-applied', async () => { await reset(); await sleep(500); });
await take('40-parameter-multiselect', async () => {
  await reset();
  // Region chosen several at a time: its list open, two picked
  await ev(`(async () => { const d = G.s.S.st.doc; await G.s.edit({ op: 'set_param_defs', defs: d.param_defs.map((x) => (/region/i.test(x.name) ? { ...x, multi: true } : x)) }); return 1; })()`); await sleep(1500);
  await ev(`document.querySelector('.param-multi')?.click()`); await sleep(700);
  await ev(`[...document.querySelectorAll('.pm-pop .pm-item input')].slice(0, 2).forEach((c) => c.click())`); await sleep(500);
});
await ev(`(async () => { document.querySelectorAll('.menu').forEach((n) => n.remove()); await G.s.api.undo(); return 1; })()`); await sleep(800);
await take('41-parameter-editor', async () => { await reset('data'); await clickText('.rail-body button', 'Region'); await sleep(1200); });
await take('42-lod-builder', async () => {
  await reset('data'); await ev(`document.querySelector('.ds-item')?.click()`); await sleep(1800);
  // "Calculated fields … + Add"
  await ev(`(() => { const h = [...document.querySelectorAll('*')].find((n) => n.children.length < 4 && /^Calculated fields/.test(n.textContent.trim())); const b = h && (h.querySelector('button') || h.parentElement.querySelector('button')); b?.click(); return !!b; })()`); await sleep(1300);
});
await take('43-sql-dataset', async () => {
  await reset('data'); await ev(`document.querySelector('.ds-item')?.click()`); await sleep(1500);
  await clickText('button', 'New data source'); await sleep(700); await clickText('.menu .mi, .menu button', 'SQL'); await sleep(1500);
});

// the theme studio, in Quire
await openDoc();
await take('44-theme-studio-overview', async () => { await reset(); await ev(`G.s.api.themeStudio('builtin:quire')`); await sleep(4000); });
await take('45-theme-studio-kpi', async () => { await ev(`(() => { const S = G.s.S; S.studioKind = 'kpi'; S.sel = null; G.s.render(); })()`); await sleep(1500); });
await take('46-design-view-selected', async () => { await ev(`document.querySelector('.sv-card')?.click()`); await sleep(1200); });
await take('47-theme-studio-contents', async () => { await ev(`(() => { const S = G.s.S; S.studioKind = 'toc'; S.sel = null; G.s.render(); })()`); await sleep(1500); });
await take('48-theme-library', async () => { await ev(`(async () => { const a = await import('/ui/app.js'); (a.manageThemes || (() => {}))(); })()`); await sleep(1500); });

console.log(failed.length ? `failed: ${failed.join(', ')}` : 'all done');
chrome.kill(); process.exit(0);
