// The page canvas: renders every page, and handles selection, moving and
// resizing on the grid, drops from the gallery, inline text editing and the
// fit advisor.

import { S, findWidget, edit, render, userMode, policy, canEdit, isReport, isTheme, selection, toggleSelected } from './state.js';
import { renderStudio } from './studio.js';
import { pageSvg, widgetParts } from './render.js';
import { icon } from './icons.js';
import { esc, h, clamp } from './util.js';
import { menu, popover, closeLayer, modal, toast } from './ui.js';
import { replaceMenu, moveToPageItems, pickTocIcon, widgetName, focusPart, partName, addTypedWidget, saveWidgetType } from './panels.js';
import { repeatBadge, wireBadges, repeatDialog } from './repeat.js';
import { textWindow } from './richtext.js';

let host = null;
let drag = null;

export function initCanvas(el) {
  host = el;
  // the theme studio draws its own workspace and handles its own clicks
  const pages = (fn) => (e) => { if (!isTheme()) fn(e); };
  el.addEventListener('pointerdown', pages(onDown));
  el.addEventListener('dblclick', pages(onDblClick));
  el.addEventListener('pointermove', pages(onHover));
  el.addEventListener('pointerleave', clearHover);
  el.addEventListener('scroll', clearHover, { passive: true });
  el.addEventListener('contextmenu', pages(onContext));
  el.addEventListener('dragover', pages(onDragOver));
  el.addEventListener('dragleave', (e) => { if (!host.contains(e.relatedTarget)) clearGhost(); });
  el.addEventListener('drop', pages(onDrop));
  el.addEventListener('scroll', () => {
    // the page nearest the viewport centre is "current"
    const mid = el.scrollTop + el.clientHeight / 3;
    let best = 0;
    el.querySelectorAll('.page-wrap').forEach((p) => { if (p.offsetTop <= mid) best = +p.dataset.pi; });
    if (best !== S.page && !drag) { S.page = best; document.dispatchEvent(new CustomEvent('pagechange')); }
  });
  let raf = 0;
  new ResizeObserver(() => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => { if (S.zoomFit && S.st?.doc && !isTheme() && Math.abs(fitZoom() - S.zoom) > 0.005) renderCanvas(); });
  }).observe(el);
}

// the zoom the pages were last drawn at
let drawnZoom = 0;

/**
 * What the view shows, in page units: the selected widget when it is on screen, else the point a
 * third down the view — with where it sits on screen, so a redraw at another zoom keeps it there.
 */
function viewAnchor(zoom) {
  const box = host.getBoundingClientRect();
  const seen = (r) => r.bottom > box.top && r.top < box.bottom;
  const sel = S.sel && host.querySelector(`.hit[data-id="${S.sel}"]`);
  if (sel && seen(sel.getBoundingClientRect())) {
    const sheet = sel.closest('.sheet');
    const r = sel.getBoundingClientRect(), sr = sheet.getBoundingClientRect();
    return { pi: sheet.dataset.pi, y: (r.top - sr.top) / zoom, screen: r.top - box.top };
  }
  const at = box.top + box.height / 3;
  for (const sheet of host.querySelectorAll('.sheet')) {
    const sr = sheet.getBoundingClientRect();
    // the sheet under that point, or the next one below it (between pages)
    if (sr.bottom >= at) return { pi: sheet.dataset.pi, y: Math.max(0, (at - sr.top) / zoom), screen: Math.max(at - box.top, sr.top - box.top) };
  }
  return null;
}

function restoreAnchor(a) {
  const sheet = host.querySelector(`.sheet[data-pi="${a.pi}"]`);
  if (!sheet) return;
  const box = host.getBoundingClientRect();
  const top = sheet.getBoundingClientRect().top - box.top + host.scrollTop;
  host.scrollTop = Math.max(0, top + a.y * S.zoom - a.screen);
}

export function fitZoom() {
  if (!S.st?.doc || !host) return S.zoom;
  const avail = host.clientWidth - 96;
  // the widget studio: zoomed onto its one widget (its text reads as it will on a page, only larger)
  const sw = S.st.doc.widget_studio && S.st.doc.pages[0]?.widgets[0];
  // (at its full size, so a smaller preview size shows smaller)
  if (sw) return clamp(avail / (S.st.page.w * (((S.wsBase?.w || sw.w) + 2) / (S.st.doc.page.cols || 24))), 0.45, 3.2);
  return clamp(avail / S.st.page.w, 0.45, 2.2);
}

const px = (v) => v * S.zoom;

/**
 * Grips on a selected table's column borders: drag one to move the border.
 * On release every shown column keeps its width as a % of the table, so the
 * others stay where they are; numbers never get narrower than they need.
 */
function colGrips(el, w, vm, f) {
  let tb = null;
  try { tb = widgetParts(S.st, w, vm).table; } catch { tb = null; }
  if (!tb?.keys || tb.cols.length < 2) return;
  const edges = tb.cols.slice(0, -1).map((_, k) => tb.cols.slice(0, k + 1).reduce((a, x) => a + x, 0));
  edges.forEach((e0, k) => {
    const g = h(`<div class="col-grip" title="Drag to resize the columns · double-click: back to automatic widths" style="left:${px(tb.x - f.x + e0)}px"><span></span></div>`);
    g.addEventListener('pointerdown', (ev) => {
      ev.stopPropagation();
      ev.preventDefault();
      const x0 = ev.clientX;
      const a = tb.cols[k], b = tb.cols[k + 1];
      const tip = h(`<div class="col-grip-tip"></div>`);
      g.append(tip);
      g.classList.add('on');
      let wa = a;
      const pct = (v) => Math.round((v / tb.w) * 1000) / 10;
      const show = () => { tip.textContent = `${pct(wa)}% | ${pct(a + b - wa)}%`; };
      show();
      const move = (e2) => {
        wa = clamp(a + (e2.clientX - x0) / S.zoom, Math.min(a, tb.mins?.[k] ?? 14), Math.max(a, a + b - (tb.mins?.[k + 1] ?? 14)));
        g.style.left = `${px(tb.x - f.x + e0 - a + wa)}px`;
        show();
      };
      const up = () => {
        window.removeEventListener('pointermove', move);
        window.removeEventListener('pointerup', up);
        g.classList.remove('on');
        tip.remove();
        if (Math.abs(wa - a) < 0.5) return;
        const ws = [...tb.cols];
        ws[k] = wa;
        ws[k + 1] = a + b - wa;
        const cols = Object.fromEntries(tb.keys.map((key, j) => [key, { width: Math.max(3, pct(ws[j])) }]));
        edit({ op: 'update_widget', id: w.id, patch: { opts: { table: { cols } } } });
      };
      window.addEventListener('pointermove', move);
      window.addEventListener('pointerup', up);
    });
    g.addEventListener('dblclick', (ev) => {
      ev.stopPropagation();
      const cols = Object.fromEntries(tb.keys.map((key) => [key, { width: null }]));
      edit({ op: 'update_widget', id: w.id, patch: { opts: { table: { cols } } } });
    });
    el.append(g);
  });
}

function frameOf(pg, x, y, w, h) {
  const [cw, ch] = pg.cell;
  return {
    x: pg.content.x + x * (cw + pg.gap),
    y: pg.content.y + y * (ch + pg.gap),
    w: w * cw + (w - 1) * pg.gap,
    h: h * ch + (h - 1) * pg.gap,
  };
}

function issueSummary(issues) {
  const bad = issues.filter((i) => i.severity !== 'info');
  if (bad.some((i) => i.severity === 'error')) return { cls: 'err', text: `${bad.length} to fix`, ic: 'warn' };
  if (bad.length) return { cls: 'warn', text: `${bad.length} suggestion${bad.length === 1 ? '' : 's'}`, ic: 'warn' };
  if (issues.length) return { cls: 'ok tip', text: 'Fits · tip', ic: 'ok' };
  return { cls: 'ok', text: 'Fits', ic: 'ok' };
}

export function renderCanvas() {
  if (!host) return;
  const st = S.st;
  if (!st?.doc) { host.innerHTML = ''; return; }
  if (isTheme()) return renderStudio(host);
  if (host.classList.contains('studio')) { host.classList.remove('studio'); delete host.dataset.studio; host.replaceChildren(); }
  if (S.zoomFit) S.zoom = fitZoom();
  // the widget studio's widget sits in the middle of its page: keep it in view
  if (st.doc.widget_studio && S.zoomFit) requestAnimationFrame(() => {
    const el = host.querySelector('.hit[data-id="w_studio"]');
    if (el) { const r = el.getBoundingClientRect(), hr = host.getBoundingClientRect(); host.scrollLeft += r.left + r.width / 2 - (hr.left + hr.width / 2); host.scrollTop += r.top - hr.top - 60; }
  });
  const pg = st.page;
  const keepScroll = host.scrollTop;
  // a new zoom (a pane opened or closed, the window resized): the same spot stays in view
  const anchor = drawnZoom && Math.abs(drawnZoom - S.zoom) > 0.001 ? viewAnchor(drawnZoom) : null;
  const frag = document.createDocumentFragment();
  const user = userMode();
  st.doc.pages.forEach((page, pi) => {
    const vmPage = st.pages[pi];
    // a page with nothing to show for this data: end users don't see it; designers see it dimmed
    if (vmPage.hidden && (user || S.preview)) return;
    const sum = issueSummary(vmPage.issues);
    // (a widget linked to a table's end and shown after its last rows on a later page doesn't count here)
    const overflowRows = Math.max(0, ...page.widgets.filter((w, wi) => !vmPage.widgets[wi]?.moved).map((w) => w.y + w.h - pg.rows));
    const extra = overflowRows ? px(overflowRows * (pg.cell[1] + pg.gap)) + 24 : 0;
    const wrap = h(`<section class="page-wrap${pi === S.page ? ' current' : ''}${vmPage.hidden ? ' page-off' : ''}${page.repeat_of ? ' page-copy' : ''}" data-pi="${pi}" style="padding-bottom:${extra}px">
      <header class="page-head" style="width:${px(pg.w)}px">
        <span class="pnum">${vmPage.hidden ? '–' : vmPage.number || pi + 1}</span>
        <span class="ptitle" title="${esc(page.title)}">${esc(vmPage.title || page.title)}</span>
        ${user ? '' : repeatBadge(page, vmPage, pi)}
        ${vmPage.hidden ? `<span class="page-off-note" title="Its condition (${esc(page.show_if || '')}) is empty or 0 for this data. Page settings → Show only when.">${icon('eye-off', 13)} Hidden for this data — not exported</span>` : ''}
        <button class="fit-pill ${sum.cls}" data-pi="${pi}">${icon(sum.ic, 13)}<span>${sum.text}</span></button>
        <span class="grow"></span>
        <button class="tb-btn sm page-more" data-pi="${pi}" title="Page actions">${icon('more', 16)}</button>
      </header>
      <div class="sheet" data-pi="${pi}" style="width:${px(pg.w)}px;height:${px(pg.h)}px">
        <div class="svg-host">${pageSvg(st, pi)}</div>
        <svg class="grid-ov" viewBox="0 0 ${pg.w} ${pg.h}" width="100%" height="100%">${gridCells(pg)}</svg>
        ${overflowRows ? `<div class="page-end" style="top:${px(pg.content.y + pg.content.h)}px"><span>Page ends here</span></div>` : ''}
        <div class="hits"></div>
      </div>
    </section>`);
    const hits = wrap.querySelector('.hits');
    const flagged = new Map();
    for (const is of vmPage.issues) if (is.widget && is.severity !== 'info') flagged.set(is.widget, is.severity);
    // a copy of a repeated page isn't edited itself: the page it copies is
    if (page.repeat_of) {
      const src = st.doc.pages.findIndex((x) => x.id === page.repeat_of);
      const veil = h(`<div class="copy-veil" title="A copy for this row — edit page ${st.pages[src]?.number || src + 1}, every copy follows"></div>`);
      veil.addEventListener('dblclick', () => { S.page = src; scrollToPage(src); });
      hits.append(veil);
    }
    if (!page.repeat_of) page.widgets.forEach((w, wi) => {
      const f = st.pages[pi].widgets[wi].frame;
      const wvm = st.pages[pi].widgets[wi];
      // a linked widget moved after its table's last rows: the table uses this space; it shows only when selected
      if (wvm.moved && S.sel !== w.id) return;
      // a mirror of a linked widget: placed after its table's last rows; editing happens on the original
      if (w.opts?.mirror) {
        const m = h(`<div class="hit mirror" style="left:${px(f.x)}px;top:${px(f.y)}px;width:${px(f.w)}px;height:${px(f.h)}px" title="Kept with the end of the table — click to edit it where it was designed"><span class="link-chip">${icon('link', 11)} after the table</span></div>`);
        m.addEventListener('pointerdown', (e) => { e.stopPropagation(); const o = findWidget(w.opts.mirror); if (o) { S.sel = o.w.id; S.page = o.pi; render(); scrollToWidget(o.w.id); } });
        hits.append(m);
        return;
      }
      const sel = S.sel === w.id;
      // one of several selected (⇧-click): outlined, its handles and label on the first
      const also = !sel && S.more.includes(w.id);
      const locked = user && w.locked;
      // a divider in the gap: a thin strip on its line, above the widgets either side
      const eo = wvm.eff?.opts || w.opts || {};
      const inGap = w.kind === 'divider' && eo.vertical && eo.gap;
      const el = h(`<div class="hit${inGap ? ' gap-div' : ''}${sel || also ? ' sel' : ''}${also ? ' also' : ''}${locked ? ' locked' : ''}${flagged.has(w.id) ? ' flag-' + flagged.get(w.id) : ''}" data-id="${w.id}"
        style="left:${inGap ? px(f.x - st.page.gap / 2) - 4 : px(f.x)}px;top:${px(f.y)}px;width:${inGap ? 8 : px(f.w)}px;height:${px(f.h)}px"></div>`);
      if (sel) {
        el.append(h(`<div class="sel-label">${S.more.length ? `${S.more.length + 1} selected` : esc(widgetName(w))}${locked ? ` ${icon('lock', 11)}` : ''}<span class="dim">${S.more.length ? '⇧-click to add or remove' : `${w.w}×${w.h}`}</span></div>`));
        if (!locked && policy().arrange) for (const d of ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw']) el.append(h(`<div class="handle h-${d}" data-dir="${d}"></div>`));
        if (!locked && w.kind === 'table') colGrips(el, w, st.pages[pi].widgets[wi], f);
      }
      if (locked && sel) el.append(h(`<div class="lock-badge">${icon('lock', 12)} Part of the template</div>`));
      if (w.opts?.follows) {
        const tbl = findWidget(w.opts.follows);
        el.classList.toggle('moved', !!wvm.moved);
        el.append(h(`<div class="link-chip on-hit" title="Kept with the end of “${esc(tbl?.w.title || 'the table')}”: while the table continues onto more pages, this goes right after its last rows${wvm.moved ? ' — it does now' : ''}.">${icon('link', 11)} ${wvm.moved ? 'designed here · shown after the table’s last rows' : 'kept with the table’s end'}</div>`));
      }
      const tv = st.pages[pi].widgets[wi].table;
      if (tv?.continued) {
        // free grid rows right below the table: nothing else in its columns until the next widget or the page end
        const below = page.widgets.filter((o) => o.id !== w.id && o.x < w.x + w.w && o.x + o.w > w.x && o.y >= w.y + w.h).map((o) => o.y);
        const free = Math.max(0, Math.min(pg.rows, ...below) - (w.y + w.h));
        const src0 = w.opts?.table?.continues || w.id;
        const following = page.widgets.filter((o) => o.opts?.follows === src0 && st.pages[pi].widgets[page.widgets.indexOf(o)]?.moved).length;
        const badge = h(`<div class="cont-badge${free && !locked && policy().arrange ? ' can-fit' : ''}">Continues on the next page ${icon('arrow-down', 11)}${following ? `<span class="cont-follow" title="Widgets linked to this table are shown after its last rows">· ${following} linked widget${following === 1 ? '' : 's'} follow${following === 1 ? 's' : ''}</span>` : ''}${free && !locked && policy().arrange ? `<button title="Make the table ${free} row${free === 1 ? '' : 's'} taller, into the free space below — the next page goes away once every row fits">Use free space</button>` : ''}</div>`);
        badge.querySelector('button')?.addEventListener('click', (e) => { e.stopPropagation(); edit({ op: 'update_widget', id: w.id, patch: { h: w.h + free } }); });
        badge.querySelector('button')?.addEventListener('pointerdown', (e) => e.stopPropagation());
        // widgets under it that belong after its last rows (a total, a chart): one click links them
        const src = w.opts?.table?.continues || w.id;
        const under = page.widgets.filter((o) => o.id !== w.id && !o.opts?.mirror && o.kind !== 'table' && o.y >= w.y + w.h && o.x < w.x + w.w && o.x + o.w > w.x && o.opts?.follows !== src);
        if (under.length && !locked && policy().arrange) {
          const lk = h(`<button class="link-btn" title="While the table continues, put these ${under.length} widgets right after its last rows (and let the table use their place here)">${icon('link', 11)} Keep the ${under.length} below with its end</button>`);
          lk.addEventListener('pointerdown', (e) => e.stopPropagation());
          lk.addEventListener('click', (e) => { e.stopPropagation(); edit({ op: 'batch', edits: under.map((o) => ({ op: 'update_widget', id: o.id, patch: { opts: { follows: src } } })) }); });
          badge.append(lk);
        }
        el.append(badge);
      }
      if (tv?.part > 0) el.append(h(`<div class="cont-badge top">${icon('arrow-up', 11)} Continued from the previous page</div>`));
      hits.append(el);
      // a composite (or one of its parts) selected: its parts can be clicked, moved and resized on its grid
      const partSel = typeof S.sel === 'string' && S.sel.startsWith(`${w.id}::`);
      if (w.kind === 'composite' && (sel || partSel) && !S.more.length) {
        if (partSel) el.classList.add('has-part-sel');
        (w.parts || []).forEach((p, k) => {
          const pf = wvm.parts?.[k]?.frame;
          if (!pf) return;
          const pid = `${w.id}::${p.id}`, on = S.sel === pid;
          const ph = h(`<div class="hit part${on ? ' sel' : ''}" data-id="${pid}" style="left:${px(pf.x)}px;top:${px(pf.y)}px;width:${px(pf.w)}px;height:${px(pf.h)}px"></div>`);
          if (on) {
            ph.append(h(`<div class="sel-label">${esc(widgetName({ ...p, id: pid }))}<span class="dim">part · ${p.w}×${p.h}</span></div>`));
            if (!locked && policy().arrange) for (const d of ['n', 's', 'e', 'w', 'ne', 'nw', 'se', 'sw']) ph.append(h(`<div class="handle h-${d}" data-dir="${d}"></div>`));
          }
          hits.append(ph);
        });
      }
    });
    frag.append(wrap);
  });
  host.replaceChildren(frag);
  host.scrollTop = keepScroll;
  if (anchor) restoreAnchor(anchor);
  drawnZoom = S.zoom;
  requestAnimationFrame(markClipped);
  const zl = document.querySelector('#zfit');
  if (zl) zl.textContent = Math.round(S.zoom * 100) + '%';
  host.classList.toggle('show-grid', !!(!isReport() && !S.preview && S.st.doc && S.showGrid));
  host.querySelectorAll('.fit-pill').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); advisor(b, +b.dataset.pi); }));
  host.querySelectorAll('.page-more').forEach((b) => b.addEventListener('click', (e) => { e.stopPropagation(); pageMenu(b, +b.dataset.pi); }));
  wireBadges(host, scrollToPage);
}

function kindLabel(k) {
  return { kpi: 'KPI card', bar: 'Bar chart', line: 'Line chart', area: 'Area chart', pie: 'Donut chart', table: 'Table', heading: 'Heading', text: 'Text', comment: 'Note', divider: 'Divider', image: 'Image', treemap: 'Treemap', waterfall: 'Waterfall', tiles: 'Tiles', scale: 'Dot scale', toc: 'Contents' }[k] || k;
}

function gridCells(pg) {
  let s = '';
  for (let y = 0; y < pg.rows; y++) for (let x = 0; x < pg.cols; x++) {
    const f = frameOf(pg, x, y, 1, 1);
    s += `<rect x="${f.x.toFixed(2)}" y="${f.y.toFixed(2)}" width="${f.w.toFixed(2)}" height="${f.h.toFixed(2)}" rx="2"/>`;
  }
  return s;
}

/**
 * Flags widgets whose drawing runs past their frame (it is clipped there),
 * with a fix that grows the widget by the rows it needs. Text and tables are
 * checked by the layout engine; this catches charts, KPIs and the rest.
 */
function markClipped() {
  const st = S.st;
  if (!st?.doc || isTheme()) return;
  const [, ch] = st.page.cell;
  const fixable = !userMode() || policy().arrange;
  host.querySelectorAll('.page-wrap').forEach((wrap) => {
    const pi = +wrap.dataset.pi;
    const page = st.doc.pages[pi], vms = st.pages[pi]?.widgets;
    if (!page || !vms) return;
    wrap.querySelectorAll('svg g[data-w]').forEach((g) => {
      const wi = page.widgets.findIndex((w) => w.id === g.dataset.w);
      const w = page.widgets[wi], vm = vms[wi];
      if (!w || ['text', 'comment', 'heading', 'table', 'image', 'divider'].includes(w.kind)) return;
      const inner = g.querySelector(':scope > g[clip-path]');
      let bb;
      try { bb = inner?.getBBox(); } catch { return; }
      if (!bb || !bb.height) return;
      const f = vm.frame;
      const over = bb.y + bb.height - (f.y + f.h);
      // a few points of descenders are fine
      if (over <= 3) return;
      const hit = wrap.querySelector(`.hit[data-id="${w.id}"]`);
      if (!hit || hit.querySelector('.clip-badge')) return;
      hit.classList.add('flag-clip');
      const rows = Math.ceil(over / (ch + st.page.gap));
      const b = h(`<button class="clip-badge" title="Part of this widget is cut off at the bottom">${icon('warn', 11)} Cut off${fixable && canEdit(w) ? ` · <b>Fit</b>` : ''}</button>`);
      if (fixable && canEdit(w)) b.addEventListener('pointerdown', (e) => {
        e.stopPropagation();
        edit({ op: 'set_rect', id: w.id, rect: { x: w.x, y: w.y, w: w.w, h: w.h + rows } });
      });
      hit.append(b);
    });
  });
}

export function scrollToPage(pi) {
  const el = host?.querySelector(`.page-wrap[data-pi="${pi}"]`);
  if (el) host.scrollTo({ top: el.offsetTop - 16, behavior: 'smooth' });
}

export function scrollToWidget(id) {
  const el = host?.querySelector(`.hit[data-id="${id}"], .sv-card[data-id="${id}"]`);
  el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
}

// ---------------------------------------------------------------- geometry

/** Grid cell under a client point on a given sheet. */
export function cellAt(sheet, cx, cy) {
  const pg = S.st.page;
  const r = sheet.getBoundingClientRect();
  const x = (cx - r.left) / S.zoom, y = (cy - r.top) / S.zoom;
  const [cw, ch] = pg.cell;
  return {
    col: Math.floor((x - pg.content.x + pg.gap / 2) / (cw + pg.gap)),
    row: Math.floor((y - pg.content.y + pg.gap / 2) / (ch + pg.gap)),
  };
}

function sheetAt(cx, cy) {
  const els = document.elementsFromPoint(cx, cy);
  for (const e of els) {
    const s = e.closest?.('.sheet');
    if (s && host.contains(s)) return s;
  }
  // nearest sheet vertically
  let best = null, bd = Infinity;
  host.querySelectorAll('.sheet').forEach((s) => {
    const r = s.getBoundingClientRect();
    const d = cy < r.top ? r.top - cy : cy > r.bottom ? cy - r.bottom : 0;
    if (d < bd) { bd = d; best = s; }
  });
  return best;
}

function showGhost(sheet, rect, kind = 'move', more = []) {
  clearGhost();
  const pg = S.st.page;
  // the dragged widget, and the others selected with it (moving along)
  for (const [i, r] of [rect, ...more].entries()) {
    const f = frameOf(pg, r.x, r.y, r.w, r.h);
    const over = r.y + r.h > pg.rows;
    sheet.querySelector('.hits').append(h(`<div class="ghost ${kind}${over ? ' over' : ''}" style="left:${px(f.x)}px;top:${px(f.y)}px;width:${px(f.w)}px;height:${px(f.h)}px">${i ? '' : `<span>${r.w}×${r.h}</span>`}</div>`));
  }
  sheet.closest('.page-wrap').classList.add('dragging');
  host.classList.add('dragging');
}

function clearGhost() {
  host?.querySelectorAll('.ghost').forEach((g) => g.remove());
  host?.querySelectorAll('.page-wrap.dragging').forEach((p) => p.classList.remove('dragging'));
  host?.classList.remove('dragging');
}

// ---------------------------------------------------------------- pointer

function onDown(e) {
  if (e.button !== 0) return;
  const t = e.target;
  if (t.closest('.page-head')) return;
  const hit = t.closest('.hit');
  const sheet = t.closest('.sheet');
  if (sheet) {
    const pi = +sheet.dataset.pi;
    if (pi !== S.page) { S.page = pi; document.dispatchEvent(new CustomEvent('pagechange')); }
  }
  if (!hit) {
    if (S.sel) { S.sel = null; render(); }
    return;
  }
  const id = hit.dataset.id;
  const f = findWidget(id);
  if (!f) return;
  if (f.parent) return partDown(e, f);
  if (f.w.kind === 'toc' && canEdit(f.w) && !t.closest('.handle')) {
    // a click on a section's icon opens its picker
    const ic = document.elementsFromPoint(e.clientX, e.clientY).find((n) => n.closest?.('[data-toc-icon]'))?.closest('[data-toc-icon]');
    if (ic) { S.sel = id; pickTocIcon(ic.dataset.tocIcon); return; }
  }
  // ⇧-click: one more selected (or one less), on the same page — their common settings show together
  if (e.shiftKey && S.sel && !userMode() && !isTheme() && !t.closest('.handle')) {
    const first = findWidget(S.sel);
    if (first && first.pi === f.pi) { e.preventDefault(); toggleSelected(id); render(); return; }
  }
  // a click on a part of the selected widget (its legend, axis, a series…) opens that part's settings
  const wasSel = S.sel === id;
  const part = wasSel && !S.more.length && !userMode() && !t.closest('.handle') ? partAt(id, e.clientX, e.clientY) : null;
  // a click on one of several selected keeps them all (dragging it moves them together)
  if (S.sel !== id && !selection().includes(id)) { S.sel = id; render(); }
  if (!canEdit(f.w) || !policy().arrange) { if (part) focusPart(id, part); return; }
  const handle = t.closest('.handle');
  const pg = S.st.page;
  const start = { x: e.clientX, y: e.clientY };
  const startCell = cellAt(host.querySelector(`.sheet[data-pi="${f.pi}"]`), e.clientX, e.clientY);
  drag = { id, pi: f.pi, w: f.w, dir: handle?.dataset.dir, start, startCell, moved: false, rect: { x: f.w.x, y: f.w.y, w: f.w.w, h: f.w.h }, sheet: null };
  const spec = S.st.specs[f.w.kind] || { min_w: 1, min_h: 1, max_h: 99 };
  const onMove = (ev) => {
    if (!drag) return;
    if (!drag.moved && Math.hypot(ev.clientX - start.x, ev.clientY - start.y) < 4) return;
    drag.moved = true;
    const w = drag.w;
    if (drag.dir) {
      const sheet = host.querySelector(`.sheet[data-pi="${drag.pi}"]`);
      const c = cellAt(sheet, ev.clientX, ev.clientY);
      let { x, y, w: ww, h: hh } = w;
      const right = w.x + w.w, bottom = w.y + w.h;
      if (drag.dir.includes('e')) ww = clamp(c.col - x + 1, spec.min_w, pg.cols - x);
      // down to the page's last row (or the widget's own bottom, if it already runs past). By how far
      // the edge moved: a table that continues is drawn taller than it is (it takes the rows its
      // linked widgets left), and one row up from that edge is one row less of the table itself
      if (drag.dir.includes('s')) hh = clamp(w.h + c.row - drag.startCell.row, spec.min_h, Math.max(spec.min_h, w.h, Math.min(spec.max_h, pg.rows - y)));
      if (drag.dir.includes('w')) { x = clamp(c.col, 0, right - spec.min_w); ww = right - x; }
      if (drag.dir.includes('n')) { y = clamp(c.row, 0, bottom - spec.min_h); hh = Math.min(bottom - y, spec.max_h); y = bottom - hh; }
      drag.rect = { x, y, w: ww, h: hh };
      drag.sheet = sheet;
      showGhost(sheet, drag.rect, 'resize');
    } else {
      const sheet = sheetAt(ev.clientX, ev.clientY);
      if (!sheet) return;
      const c = cellAt(sheet, ev.clientX, ev.clientY);
      const dx = c.col - drag.startCell.col, dy = c.row - drag.startCell.row;
      const samePage = +sheet.dataset.pi === drag.pi;
      let x = samePage ? w.x + dx : c.col - Math.floor(w.w / 2);
      let y = samePage ? w.y + dy : c.row;
      x = clamp(x, 0, pg.cols - w.w);
      y = clamp(y, 0, pg.rows + 8);
      drag.rect = { x, y, w: w.w, h: w.h };
      drag.sheet = sheet;
      // several selected: the others move by as much (as they will when dropped)
      const others = samePage ? selection().filter((o) => o !== drag.id).map((o) => findWidget(o)?.w).filter(Boolean)
        .map((o) => ({ x: clamp(o.x + x - w.x, 0, pg.cols - o.w), y: Math.max(0, o.y + y - w.y), w: o.w, h: o.h })) : [];
      showGhost(sheet, drag.rect, 'move', others);
    }
    autoScroll(ev);
  };
  const onUp = async () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    const d = drag;
    drag = null;
    clearGhost();
    if (!d?.moved && part) { focusPart(d.id, part); return; }
    if (!d?.moved || !d.sheet) return;
    const target = +d.sheet.dataset.pi;
    const r = d.rect;
    if (target !== d.pi && S.more.length) {
      toast('Several selected widgets move on their page. To move them to another page, move them one at a time.', 'info', 3600);
    } else if (target !== d.pi) {
      const pageId = S.st.doc.pages[target].id;
      const ok = await edit({ op: 'move_to_page', id: d.id, page: pageId });
      if (ok) await edit({ op: 'set_rect', id: d.id, rect: r });
      S.page = target;
    } else if (r.x !== d.w.x || r.y !== d.w.y || r.w !== d.w.w || r.h !== d.w.h) {
      await edit({ op: 'set_rect', id: d.id, rect: r });
    }
  };
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
  e.preventDefault();
}

/** The space between a composite's parts, in points (as the core lays them out). */
const PART_GAP = 6;
const gridOf = (w) => [Math.min(48, Math.max(1, w.opts?.grid?.cols || 12)), Math.min(48, Math.max(1, w.opts?.grid?.rows || 6))];

/** A composite's part: selected, then moved or resized on the composite's grid; a click on a part of it (axis, legend…) opens that. */
function partDown(e, f) {
  const id = f.w.id, t = e.target;
  const wasSel = S.sel === id;
  const part = wasSel && !t.closest('.handle') ? partAt(id, e.clientX, e.clientY) : null;
  if (!wasSel) { S.sel = id; render(); }
  if (!canEdit(f.parent) || !policy().arrange || userMode()) { if (part) focusPart(id, part); return; }
  const sheet = host.querySelector(`.sheet[data-pi="${f.pi}"]`);
  const body = S.st.pages[f.pi].widgets[f.wi].body;
  const [cols, rows] = gridOf(f.parent);
  const cw = (body.w - PART_GAP * (cols - 1)) / cols, rh = (body.h - PART_GAP * (rows - 1)) / rows;
  const cell = (ev) => {
    const r = sheet.getBoundingClientRect();
    const x = (ev.clientX - r.left) / S.zoom, y = (ev.clientY - r.top) / S.zoom;
    return { col: clamp(Math.floor((x - body.x + PART_GAP / 2) / (cw + PART_GAP)), 0, cols - 1), row: clamp(Math.floor((y - body.y + PART_GAP / 2) / (rh + PART_GAP)), 0, rows - 1) };
  };
  const frameAt = (r) => ({ x: body.x + r.x * (cw + PART_GAP), y: body.y + r.y * (rh + PART_GAP), w: r.w * cw + (r.w - 1) * PART_GAP, h: r.h * rh + (r.h - 1) * PART_GAP });
  const p = f.w, dir = t.closest('.handle')?.dataset.dir, start = cell(e), sx = e.clientX, sy = e.clientY;
  let rect = { x: p.x, y: p.y, w: p.w, h: p.h }, moved = false;
  // another part in the way: the ghost shows it (red), and the drop leaves the part where it was
  const covers = (r) => (f.parent.parts || []).some((q) => q.id !== id.split('::')[1] && r.x < q.x + q.w && q.x < r.x + r.w && r.y < q.y + q.h && q.y < r.y + r.h);
  const ghost = (r) => {
    clearGhost();
    const g = frameAt(r);
    sheet.querySelector('.hits').append(h(`<div class="ghost ${dir ? 'resize' : 'move'}${covers(r) ? ' over' : ''}" style="left:${px(g.x)}px;top:${px(g.y)}px;width:${px(g.w)}px;height:${px(g.h)}px"><span>${r.w}×${r.h}</span></div>`));
  };
  const onMove = (ev) => {
    if (!moved && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 4) return;
    moved = true;
    const c = cell(ev);
    let { x, y, w: ww, h: hh } = p;
    if (dir) {
      const right = p.x + p.w, bottom = p.y + p.h;
      if (dir.includes('e')) ww = clamp(c.col - x + 1, 1, cols - x);
      if (dir.includes('s')) hh = clamp(c.row - y + 1, 1, rows - y);
      if (dir.includes('w')) { x = clamp(c.col, 0, right - 1); ww = right - x; }
      if (dir.includes('n')) { y = clamp(c.row, 0, bottom - 1); hh = bottom - y; }
    } else {
      x = clamp(p.x + c.col - start.col, 0, cols - p.w);
      y = clamp(p.y + c.row - start.row, 0, rows - p.h);
    }
    rect = { x, y, w: ww, h: hh };
    ghost(rect);
  };
  const onUp = async () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    clearGhost();
    if (!moved) { if (part) focusPart(id, part); return; }
    if (covers(rect)) { toast('That place is taken by another part: move or shrink it first.', 'info', 3000); return; }
    if (rect.x !== p.x || rect.y !== p.y || rect.w !== p.w || rect.h !== p.h) await edit({ op: 'set_rect', id, rect });
  };
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
  e.preventDefault();
}

// ---- parts of a widget under the pointer (charts tag them: data-part)

/** The part of widget `id` at a point of the screen, or null; `where` gets the drawing it is in (one copy of it). */
function partAt(id, x, y, where = null) {
  for (const n of document.elementsFromPoint(x, y)) {
    const p = n.closest?.('[data-part]');
    if (!p) continue;
    const root = p.closest('[data-w]');
    const w = root?.dataset.w || '';
    if (w === id || w.startsWith(`${id}~`)) { if (where) where.root = root; return p.dataset.part; }
  }
  return null;
}

/** On the selected widget, the part under the pointer is outlined and named: a click opens its settings. */
let hoverPart = null;
function onHover(e) {
  if (drag || !S.sel || userMode() || !host) return clearHover();
  const hit = e.target.closest?.('.hit');
  if (!hit || hit.dataset.id !== S.sel || e.target.closest('.handle')) return clearHover();
  const where = {};
  const part = partAt(S.sel, e.clientX, e.clientY, where);
  if (!part) return clearHover();
  // every piece of that part in the drawing under the pointer, outlined as one box (a repeated page or a
  // continued table draws the widget again elsewhere: those copies aren't part of it)
  const nodes = [...where.root.querySelectorAll('[data-part]')].filter((g) => g.dataset.part === part && !g.parentElement.closest('[data-part]')?.dataset.part?.startsWith(part));
  const rs = nodes.map((g) => g.getBoundingClientRect()).filter((r) => r.width || r.height);
  if (!rs.length) return clearHover();
  const box = { l: Math.min(...rs.map((r) => r.left)), t: Math.min(...rs.map((r) => r.top)), r: Math.max(...rs.map((r) => r.right)), b: Math.max(...rs.map((r) => r.bottom)) };
  if (!hoverPart) { hoverPart = h(`<div class="part-hl"><span></span></div>`); document.body.append(hoverPart); }
  Object.assign(hoverPart.style, { left: `${box.l - 3}px`, top: `${box.t - 3}px`, width: `${box.r - box.l + 6}px`, height: `${box.b - box.t + 6}px` });
  hoverPart.querySelector('span').textContent = partName(part);
}
function clearHover() { hoverPart?.remove(); hoverPart = null; }

let scrollTimer = null;
function autoScroll(ev) {
  const r = host.getBoundingClientRect();
  const edge = 48;
  const dy = ev.clientY < r.top + edge ? -14 : ev.clientY > r.bottom - edge ? 14 : 0;
  clearInterval(scrollTimer);
  if (dy) scrollTimer = setInterval(() => { if (!drag) return clearInterval(scrollTimer); host.scrollTop += dy; }, 16);
}

// ---------------------------------------------------------------- inline text editing

function onDblClick(e) {
  const hit = e.target.closest('.hit');
  if (!hit) return;
  const f = findWidget(hit.dataset.id);
  if (!f || !['text', 'comment', 'heading'].includes(f.w.kind)) return;
  const pol = policy();
  const allowed = !userMode() || (!f.w.locked && (pol.edit_text || (f.w.kind === 'comment' && pol.comment)));
  if (!allowed) return;
  startInlineEdit(f);
}

export function startInlineEdit(f) {
  const hit = host.querySelector(`.hit[data-id="${f.w.id}"]`);
  if (!hit) return;
  const vm = f.vm;
  const isHeading = f.w.kind === 'heading';
  const block = vm.texts[vm.texts.length - (isHeading && vm.texts.length > 1 ? 2 : 1)] || null;
  const size = (block?.block.size || 9.5) * S.zoom;
  const box = isHeading ? vm.frame : vm.body;
  // text and notes: their own window (words, icons, the note's background and mark)
  if (!isHeading) {
    // one window at a time
    if (document.querySelector('.modal .tw')) return;
    const pol = policy();
    textWindow(f.w, S.st.doc.theme, { modal, canStyle: !userMode() || pol.restyle || pol.comment })
      .then((patch) => patch && edit({ op: 'update_widget', id: f.w.id, patch }));
    return;
  }
  const ta = h(`<textarea class="inline-edit" spellcheck="true"></textarea>`);
  ta.value = f.w.opts?.text ?? '';
  Object.assign(ta.style, {
    left: px(box.x - vm.frame.x) - 4 + 'px',
    top: px((block ? block.y : box.y) - vm.frame.y) - 4 + 'px',
    width: px(box.w) + 8 + 'px',
    minHeight: px(Math.max(box.h - ((block ? block.y : box.y) - box.y), 30)) + 8 + 'px',
    fontSize: size + 'px',
    lineHeight: (block?.block.line_h || 13) * S.zoom + 'px',
    fontWeight: block?.block.weight || 400,
  });
  hit.classList.add('editing');
  hit.append(ta);
  ta.focus();
  ta.setSelectionRange(ta.value.length, ta.value.length);
  const grow = () => { ta.style.height = 'auto'; ta.style.height = ta.scrollHeight + 'px'; };
  grow();
  ta.addEventListener('input', grow);
  ta.addEventListener('pointerdown', (e) => e.stopPropagation());
  let done = false;
  const finish = async (save) => {
    if (done) return;
    done = true;
    const text = ta.value;
    hit.classList.remove('editing');
    ta.remove();
    if (save && text !== (f.w.opts?.text ?? '')) await edit({ op: 'update_widget', id: f.w.id, patch: { opts: { text } } });
  };
  ta.addEventListener('keydown', (e) => {
    e.stopPropagation();
    if (e.key === 'Escape') { e.preventDefault(); finish(false); }
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); finish(true); }
  });
  ta.addEventListener('blur', () => finish(true));
}

// ---------------------------------------------------------------- drops from the gallery

export const dragPayload = { current: null };

function dropRect(e, sheet, p = dragPayload.current) {
  if (!p) return null;
  const pg = S.st.page;
  const c = cellAt(sheet, e.clientX, e.clientY);
  const w = Math.min(p.w, pg.cols);
  const x = clamp(c.col - Math.floor(w / 2), 0, pg.cols - w);
  const y = clamp(c.row, 0, pg.rows - 1);
  return { x, y, w, h: p.h };
}

function onDragOver(e) {
  if (!dragPayload.current) return;
  const sheet = sheetAt(e.clientX, e.clientY);
  if (!sheet) return;
  e.preventDefault();
  e.dataTransfer.dropEffect = 'copy';
  const r = dropRect(e, sheet);
  if (r) showGhost(sheet, r, 'add');
}

async function onDrop(e) {
  const p = dragPayload.current;
  dragPayload.current = null;
  clearGhost();
  if (!p) return;
  const sheet = sheetAt(e.clientX, e.clientY);
  if (!sheet) return;
  e.preventDefault();
  const pi = +sheet.dataset.pi;
  const at = dropRect(e, sheet, p);
  const page = S.st.doc.pages[pi].id;
  S.page = pi;
  const st = p.wtype
    ? await addTypedWidget(p.wtype, page, at)
    : p.catalog
    ? await edit({ op: 'add_from_catalog', page, catalog: p.catalog, at })
    : await edit({ op: 'add_widget', page, widget: p.widget, at });
  selectNewest(st, page);
}

/** Selects the widget an add just created. */
export function selectNewest(st, pageId) {
  if (!st) return;
  const page = st.doc.pages.find((p) => p.id === pageId);
  if (!page?.widgets.length) return;
  const newest = page.widgets.reduce((a, w) => (+w.id.slice(1) > +a.id.slice(1) ? w : a));
  S.sel = newest.id;
  render();
  if (newest.kind === 'comment' || newest.kind === 'text') {
    requestAnimationFrame(() => { const f = findWidget(newest.id); if (f) startInlineEdit(f); });
  }
}

// ---------------------------------------------------------------- menus

function onContext(e) {
  const hit = e.target.closest('.hit');
  if (!hit) return;
  e.preventDefault();
  const f = findWidget(hit.dataset.id);
  if (!f) return;
  S.sel = f.w.id;
  render();
  widgetMenu({ x: e.clientX, y: e.clientY }, f);
}

export function widgetMenu(anchor, f) {
  // a composite's part: what can be done to a part
  if (f.parent) {
    const items = [{ title: `A part of “${widgetName(f.parent)}”` }];
    if (!userMode()) {
      if (['text', 'heading'].includes(f.w.kind)) items.push({ label: 'Edit text', icon: 'rename', hint: 'Double-click', run: () => startInlineEdit(f) });
      items.push({ label: 'Advanced settings…', icon: 'code', desc: 'Its settings as JSON, checked before applying', run: () => import('./advanced.js').then((m) => m.advancedSettings(f)) });
      items.push({ label: 'Select the composite', icon: 'arrow-up', run: () => { S.sel = f.parent.id; render(); } });
      items.push({ sep: true });
      items.push({ label: 'Remove this part', icon: 'trash', danger: true, hint: '⌫', run: () => edit({ op: 'remove_widget', id: f.w.id }) });
    }
    return menu(anchor, items);
  }
  const pol = policy();
  const user = userMode();
  const lockedForUser = user && f.w.locked;
  const items = [];
  if (lockedForUser) items.push({ title: 'Locked by the template' });
  if (['text', 'comment', 'heading'].includes(f.w.kind) && !lockedForUser && (!user || pol.edit_text || (f.w.kind === 'comment' && pol.comment))) {
    items.push({ label: 'Edit text', icon: 'rename', hint: 'Double-click', run: () => startInlineEdit(f) });
  }
  // the same data as another chart, or a widget from the gallery
  const types = !lockedForUser && (!user || pol.restyle) && !!f.w.binding && !['text', 'heading', 'comment', 'divider', 'image', 'toc'].includes(f.w.kind);
  const gallery = !lockedForUser && (!user || (pol.add && pol.remove)) && !!S.st.doc.catalog.length;
  if (types || gallery) items.push({ label: 'Replace with…', icon: 'swap', run: () => replaceMenu(anchor, f, { types, gallery }) });
  if (!lockedForUser && (!user || pol.arrange)) {
    items.push({ label: 'Duplicate', icon: 'duplicate', hint: '⌘D', run: () => edit({ op: 'duplicate_widget', id: f.w.id }) });
    items.push({ label: 'Move to page…', icon: 'move', run: () => menu(anchor, moveToPageItems(f)) });
  }
  if (!user) {
    items.push({ sep: true });
    items.push({ label: f.w.locked ? 'Unlock for end users' : 'Lock for end users', icon: f.w.locked ? 'unlock' : 'lock', run: () => edit({ op: 'update_widget', id: f.w.id, patch: { locked: !f.w.locked } }) });
    items.push({ label: 'Save to gallery…', icon: 'gallery', run: () => document.dispatchEvent(new CustomEvent('save-to-gallery', { detail: f.w.id })) });
    items.push({ label: 'Advanced settings…', icon: 'code', desc: 'Its settings as JSON, checked before applying', run: () => import('./advanced.js').then((m) => m.advancedSettings(f)) });
    // the designer's own widget types (a library every template can use)
    if (S.st.doc.kind === 'template' && f.w.kind !== 'toc') {
      const t = f.w.type && S.st.doc.theme.widget_types?.[f.w.type];
      items.push({ sep: true });
      if (t) {
        items.push({ label: `Update “${t.name}” from this widget`, icon: 'upload', desc: 'Every widget of the type follows', run: () => saveWidgetType(f, { update: true }) });
        items.push({ label: 'Detach from the type', icon: 'link', desc: 'Keeps its look, no longer follows the type', run: () => edit({ op: 'detach_widget_type', id: f.w.id }) });
      }
      items.push({ label: t ? 'Save as a new widget type…' : 'Save as widget type…', icon: 'layers', run: () => saveWidgetType(f) });
    }
  }
  if (!lockedForUser && (!user || pol.remove || (f.w.kind === 'comment' && pol.comment))) {
    items.push({ sep: true });
    items.push({ label: 'Delete', icon: 'trash', danger: true, hint: '⌫', run: () => edit({ op: 'remove_widget', id: f.w.id }) });
  }
  if (!items.filter((i) => !i.title).length) items.push({ label: 'Nothing to change here', disabled: true });
  menu(anchor, items);
}

function pageMenu(anchor, pi) {
  menu(anchor, pageItems(pi), { align: 'end' });
}

/** What can be done with a page: shared by its menu and the Page menu in the title bar. */
export function pageItems(pi) {
  const page = S.st.doc.pages[pi];
  const pol = policy();
  const n = S.st.doc.pages.length;
  // a copy of a repeated page: its actions are the page it copies
  if (page.repeat_of) {
    const src = S.st.doc.pages.findIndex((x) => x.id === page.repeat_of);
    return [
      { title: `Copy ${(page.copy || 0) + 1} of ${page.copies}` },
      { label: `Go to the page it copies (page ${S.st.pages[src]?.number || src + 1})`, icon: 'page', run: () => { S.page = src; scrollToPage(src); } },
      pol.pages && { label: 'Repeat settings…', icon: 'refresh', run: () => repeatDialog(pi) },
    ];
  }
  return [
    { label: 'Fit everything on this page', icon: 'wand', run: () => edit({ op: 'auto_arrange', page: page.id }) },
    { sep: true },
    pol.pages && !page.auto && { label: page.repeat?.dataset ? 'Repeat settings…' : 'Repeat for each…', icon: 'refresh', desc: page.repeat?.dataset ? '' : 'One copy of this page (or its group) per row of a data source', run: () => repeatDialog(pi) },
    pol.pages && { label: 'Title & section…', icon: 'rename', run: () => document.dispatchEvent(new CustomEvent('rename-page', { detail: pi })) },
    pol.pages && { label: 'Add page after', icon: 'plus', run: () => edit({ op: 'add_page', after: page.id }) },
    pol.pages && { label: 'Duplicate page', icon: 'duplicate', run: () => edit({ op: 'duplicate_page', page: page.id }) },
    pol.pages && pi > 0 && { label: 'Move up', icon: 'chevl', run: () => edit({ op: 'move_page', page: page.id, to: pi - 1 }) },
    pol.pages && pi < n - 1 && { label: 'Move down', icon: 'chevr', run: () => edit({ op: 'move_page', page: page.id, to: pi + 1 }) },
    pol.pages && n > 1 && { sep: true },
    pol.pages && n > 1 && { label: 'Delete page', icon: 'trash', danger: true, run: () => edit({ op: 'remove_page', page: page.id }) },
  ];
}

// ---------------------------------------------------------------- fit advisor

function advisor(anchor, pi) {
  const issues = S.st.pages[pi].issues;
  const page = S.st.doc.pages[pi];
  const el = h(`<div class="advisor">
    <div class="adv-head">${icon('layout', 16)}<span>Page fit · ${esc(page.title)}</span></div>
    <div class="adv-list"></div>
  </div>`);
  const list = el.querySelector('.adv-list');
  if (!issues.length) {
    list.append(h(`<div class="adv-empty">${icon('ok', 22)}<div><b>Everything fits.</b><br>No overlaps, nothing cut off, and the page is well used.</div></div>`));
  }
  const order = { error: 0, warning: 1, info: 2 };
  [...issues].sort((a, b) => order[a.severity] - order[b.severity]).forEach((is) => {
    const item = h(`<div class="adv-item ${is.severity}">
      <div class="adv-msg">${icon(is.severity === 'info' ? 'info' : 'warn', 15)}<span>${esc(is.message)}</span></div>
      <div class="adv-sugg"></div>
    </div>`);
    const sugg = item.querySelector('.adv-sugg');
    is.suggestions.forEach((s, k) => {
      const b = h(`<button class="sugg${k === 0 ? ' primary' : ''}"><b>${esc(s.label)}</b><span>${esc(s.detail)}</span></button>`);
      b.addEventListener('mouseenter', () => previewOps(pi, s.ops, is.widget));
      b.addEventListener('mouseleave', () => clearPreview());
      b.addEventListener('click', async () => {
        closeLayer();
        clearPreview();
        await edit({ op: 'apply_ops', page: page.id, ops: s.ops });
      });
      sugg.append(b);
    });
    if (is.widget) item.addEventListener('mouseenter', () => host.querySelector(`.hit[data-id="${is.widget}"]`)?.classList.add('pulse'));
    if (is.widget) item.addEventListener('mouseleave', () => host.querySelector(`.hit[data-id="${is.widget}"]`)?.classList.remove('pulse'));
    list.append(item);
  });
  popover(anchor, el, { onClose: clearPreview });
}

function previewOps(pi, ops, widget) {
  clearPreview();
  const sheet = host.querySelector(`.sheet[data-pi="${pi}"]`);
  if (!sheet) return;
  const pg = S.st.page;
  for (const op of ops) {
    if (op.op === 'set_rect') {
      const f = frameOf(pg, op.rect.x, op.rect.y, op.rect.w, op.rect.h);
      sheet.querySelector('.hits').append(h(`<div class="preview-ghost" style="left:${px(f.x)}px;top:${px(f.y)}px;width:${px(f.w)}px;height:${px(f.h)}px"></div>`));
    } else if (op.id || widget) {
      host.querySelector(`.hit[data-id="${op.id || widget}"]`)?.classList.add('preview-' + op.op);
    }
  }
}

function clearPreview() {
  host?.querySelectorAll('.preview-ghost').forEach((g) => g.remove());
  host?.querySelectorAll('.hit').forEach((x) => x.classList.remove('preview-remove', 'preview-move_to_page', 'preview-set_limit', 'pulse'));
}
