// Shared UI primitives: menus, popovers, modals, toasts, color palette.
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { colorPicker, normHex, pickFromScreen } from './colorpick.js';
import { CAN_PICK_SCREEN } from './platform.js';

const root = () => document.getElementById('overlay-root');
let openLayer = null;

export function closeLayer() {
  if (openLayer) {
    const l = openLayer;
    openLayer = null;
    l.close();
  }
}

function placeNear(el, anchor, { align = 'start', gap = 4 } = {}) {
  const r = anchor instanceof Element ? anchor.getBoundingClientRect() : { left: anchor.x, right: anchor.x, top: anchor.y, bottom: anchor.y, width: 0 };
  el.style.visibility = 'hidden';
  root().append(el);
  const w = el.offsetWidth, hgt = el.offsetHeight;
  let x = align === 'end' ? r.right - w : align === 'center' ? r.left + r.width / 2 - w / 2 : r.left;
  let y = r.bottom + gap;
  if (y + hgt > innerHeight - 8) y = Math.max(8, r.top - hgt - gap);
  x = Math.max(8, Math.min(x, innerWidth - w - 8));
  el.style.left = x + 'px';
  el.style.top = y + 'px';
  el.style.visibility = '';
}

/** Shows `el` as a dismissable layer anchored to an element or point. */
export function popover(anchor, el, opts = {}) {
  closeLayer();
  placeNear(el, anchor, opts);
  const onDown = (e) => {
    if (!el.contains(e.target) && !(anchor instanceof Element && anchor.contains(e.target))) closeLayer();
  };
  const onKey = (e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      closeLayer();
      opts.onEscape?.();
    }
  };
  setTimeout(() => window.addEventListener('pointerdown', onDown, true));
  window.addEventListener('keydown', onKey, true);
  anchor instanceof Element && anchor.classList.add('on');
  const layer = {
    el,
    close() {
      window.removeEventListener('pointerdown', onDown, true);
      window.removeEventListener('keydown', onKey, true);
      anchor instanceof Element && anchor.classList.remove('on');
      el.remove();
      opts.onClose?.();
    },
  };
  openLayer = layer;
  return layer;
}

/**
 * items: { label, icon, hint, desc, run, disabled, danger, checked } | { sep: true } | { title }
 */
export function menu(anchor, items, opts = {}) {
  const el = h('<div class="menu" role="menu"></div>');
  for (const it of items) {
    if (!it) continue;
    if (it.sep) { el.append(h('<div class="menu-sep"></div>')); continue; }
    if (it.title) { el.append(h(`<div class="menu-title">${esc(it.title)}</div>`)); continue; }
    const ic = it.swatch ? `<span class="sw-dot" style="background:${it.swatch}"></span>` : it.checked !== undefined ? (it.checked ? icon('check', 16) : '') : it.icon ? icon(it.icon, 16) : '';
    if (it.swatch && it.checked) it.hint = '✓';
    const b = h(`<button class="menu-item${it.danger ? ' danger' : ''}${it.desc ? ' tall' : ''}" role="menuitem">
      <span class="i">${ic}</span>
      <span class="lbl">${esc(it.label)}${it.desc ? `<span class="menu-desc">${esc(it.desc)}</span>` : ''}</span>
      ${it.hint ? `<span class="hint">${esc(it.hint)}</span>` : ''}
    </button>`);
    if (it.disabled) b.disabled = true;
    b.addEventListener('click', () => {
      closeLayer();
      it.run?.();
    });
    el.append(b);
  }
  const layer = popover(anchor, el, opts);
  // keyboard navigation
  const buttons = [...el.querySelectorAll('.menu-item:not(:disabled)')];
  let idx = -1;
  el.tabIndex = -1;
  el.focus({ preventScroll: true });
  el.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      idx = (idx + (e.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
      buttons.forEach((b, i) => b.classList.toggle('active', i === idx));
    } else if (e.key === 'Enter' && idx >= 0) {
      e.preventDefault();
      buttons[idx].click();
    }
  });
  return layer;
}

export function toast(msg, kind = 'ok', ms = 2800) {
  let host = document.querySelector('.toasts');
  if (!host) {
    host = h('<div class="toasts"></div>');
    document.body.append(host);
  }
  const t = h(`<div class="toast ${kind}"><span class="i">${icon(kind === 'error' ? 'alert' : kind === 'info' ? 'info' : 'check', 16)}</span><span></span></div>`);
  t.lastChild.textContent = msg;
  host.append(t);
  setTimeout(() => {
    t.classList.add('out');
    setTimeout(() => t.remove(), 220);
  }, ms);
  return t;
}

/** Modal dialog. Resolves with the clicked button's value (or null on Esc). */
export function modal({ title, body, buttons = [{ label: 'OK', value: true, kind: 'primary' }], iconName, iconKind = '', wide = false, onOpen }) {
  return new Promise((resolve) => {
    const back = h('<div class="modal-back"></div>');
    const box = h(`<div class="modal${wide ? ' wide' : ''}" role="dialog" aria-modal="true">
      <div class="modal-head">${iconName ? `<span class="ic ${iconKind}">${icon(iconName, 18)}</span>` : ''}<span>${esc(title)}</span></div>
      <div class="modal-body"></div>
      <div class="modal-foot"></div>
    </div>`);
    const bodyEl = box.querySelector('.modal-body');
    if (typeof body === 'string') bodyEl.innerHTML = body;
    else if (body) bodyEl.replaceWith(body);
    const foot = box.querySelector('.modal-foot');
    if (!buttons.length) foot.remove();
    const done = (v) => {
      window.removeEventListener('keydown', onKey, true);
      back.remove();
      resolve(v);
    };
    for (const b of buttons) {
      const el = h(`<button class="btn ${b.kind || ''}">${esc(b.label)}</button>`);
      el.addEventListener('click', () => done(typeof b.value === 'function' ? b.value(box) : b.value));
      foot.append(el);
    }
    const onKey = (e) => {
      // only the topmost dialog answers keys; editors that use Esc themselves keep it
      if ([...document.querySelectorAll('.modal-back')].pop() !== back) return;
      if (e.key === 'Escape' && (e.target.closest?.('[data-own-esc]') || document.querySelector('.menu'))) return;
      if (e.key === 'Escape') { e.stopPropagation(); e.preventDefault(); done(null); }
      if (e.key === 'Enter' && !e.isComposing && !(e.target instanceof HTMLTextAreaElement) && !e.target.isContentEditable) {
        const primary = buttons.find((b) => b.kind === 'primary' || b.kind === 'danger');
        if (primary && !box.querySelector('.fn-dialog')) {
          e.preventDefault();
          e.stopPropagation();
          done(typeof primary.value === 'function' ? primary.value(box) : primary.value);
        }
      }
    };
    window.addEventListener('keydown', onKey, true);
    back.addEventListener('pointerdown', (e) => { if (e.target === back) done(null); });
    back.append(box);
    root().append(back);
    const focusTarget = box.querySelector('input, select') || foot.querySelector('.primary, .danger') || foot.lastChild;
    focusTarget?.focus();
    if (focusTarget instanceof HTMLInputElement) focusTarget.select();
    onOpen?.(box, done);
  });
}

// ---------------------------------------------------------------- colors

const THEME = ['#FFFFFF', '#000000', '#E7E6E6', '#44546A', '#4472C4', '#ED7D31', '#A5A5A5', '#FFC000', '#5B9BD5', '#70AD47'];
const STANDARD = ['#C00000', '#FF0000', '#FFC000', '#FFFF00', '#92D050', '#00B050', '#00B0F0', '#0070C0', '#002060', '#7030A0'];

function mix(hex, t) {
  const n = parseInt(hex.slice(1), 16);
  let r = n >> 16, g = (n >> 8) & 255, b = n & 255;
  if (t > 0) { r += (255 - r) * t; g += (255 - g) * t; b += (255 - b) * t; }
  else { r *= 1 + t; g *= 1 + t; b *= 1 + t; }
  return '#' + [r, g, b].map((x) => Math.round(x).toString(16).padStart(2, '0')).join('').toUpperCase();
}

export function colorPalette(anchor, { onPick, noneLabel = 'No fill', title = 'Theme colors' }) {
  const el = h(`<div class="menu palette-pop">
    <div class="menu-title" style="padding-left:0">${esc(title)}</div>
    <div class="swatches theme"></div>
    <div class="menu-title" style="padding-left:0">Standard colors</div>
    <div class="swatches std"></div>
    <div class="pal-actions">
      <button class="btn none">${icon('border-none', 14)} ${esc(noneLabel)}</button>
      <button class="btn more">${icon('sparkles', 14)} Custom…</button>
      <button class="btn drop" title="Pick a colour anywhere on the screen">${icon('pipette', 14)} From screen</button>
    </div>
    <div class="pal-custom" hidden></div>
  </div>`);
  const theme = el.querySelector('.theme');
  const tints = [0, 0.8, 0.6, 0.4, -0.25, -0.5];
  for (const t of tints) {
    for (const base of THEME) {
      const c = t === 0 ? base : base === '#FFFFFF' ? mix('#FFFFFF', -Math.abs(t) * 0.35) : base === '#000000' ? mix('#000000', Math.abs(t)) : mix(base, t);
      theme.append(swatch(c));
    }
  }
  const std = el.querySelector('.std');
  STANDARD.forEach((c) => std.append(swatch(c)));
  function swatch(c) {
    const b = h(`<button class="sw" title="${c}" style="background:${c}"></button>`);
    b.addEventListener('click', () => { closeLayer(); onPick(c); });
    return b;
  }
  el.querySelector('.none').addEventListener('click', () => { closeLayer(); onPick(null); });
  // a custom colour is picked right here (the system colour window would close this popover)
  const custom = el.querySelector('.pal-custom');
  el.querySelector('.more').addEventListener('click', () => {
    if (!custom.childElementCount) custom.append(colorPicker('#2a78d6', (c) => { closeLayer(); onPick(c.toUpperCase()); }));
    custom.hidden = !custom.hidden;
  });
  el.querySelector('.drop').hidden = !CAN_PICK_SCREEN;
  el.querySelector('.drop').addEventListener('click', async () => {
    const c = normHex(await pickFromScreen());
    if (c) { closeLayer(); onPick(c.toUpperCase()); }
  });
  return popover(anchor, el);
}
