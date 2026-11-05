// Colour picking without the system colour window (it would close the popover it opens
// from): a shade square, a hue slider, a hex field and the screen eyedropper.

import { esc, h } from './util.js';
import { icon } from './icons.js';
import { toast } from './ui.js';


const HEX = /^#?([0-9a-f]{6}|[0-9a-f]{3})$/i;
/** "#abc", "abc", "#aabbcc" → "#aabbcc" (or null). */
export function normHex(v) {
  const m = String(v || '').trim().match(HEX);
  if (!m) return null;
  const x = m[1].length === 3 ? m[1].split('').map((c) => c + c).join('') : m[1];
  return `#${x.toLowerCase()}`;
}
const toHsv = (hex) => {
  const n = parseInt(hex.slice(1), 16), r = (n >> 16) / 255, g = ((n >> 8) & 255) / 255, b = (n & 255) / 255;
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn;
  let hh = 0;
  if (d) hh = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [(hh * 60 + 360) % 360, mx ? d / mx : 0, mx];
};
const fromHsv = (hh, sat, val) => {
  const f = (n) => { const k = (n + hh / 60) % 6; return val - val * sat * Math.max(0, Math.min(k, 4 - k, 1)); };
  return '#' + [f(5), f(3), f(1)].map((x) => Math.round(x * 255).toString(16).padStart(2, '0')).join('');
};

/**
 * The screen eyedropper: the system sampler in the app (any pixel on screen, other apps'
 * windows included), the browser's where it has one. Resolves to "#rrggbb" or null.
 */
export async function pickFromScreen() {
  try {
    const { call } = await import('./state.js');
    return await call('pick_screen_color', {}, { quiet: true });
  } catch (e) {
    if (window.EyeDropper) {
      try { return (await new window.EyeDropper().open()).sRGBHex; } catch { return null; }
    }
    toast('The screen eyedropper needs the desktop app.', 'info');
    return null;
  }
}

/**
 * A colour picker drawn in the page: shade square, hue slider, hex field, eyedropper.
 * `onPick(hex)` runs on Apply (or Enter in the hex field, or a pick from the screen).
 */
export function colorPicker(start, onPick) {
  let [hh, sat, val] = toHsv(normHex(start) || '#2a78d6');
  const el = h(`<div class="cpk">
    <div class="cpk-sv"><div class="cpk-dot"></div></div>
    <input class="cpk-hue" type="range" min="0" max="360" step="1">
    <div class="cpk-row"><span class="cpk-now"></span><input class="cpk-hex" spellcheck="false" maxlength="7">
      <button class="btn sm cpk-drop" title="Pick a colour anywhere on the screen">${icon('pipette', 13)}</button>
      <button class="btn sm primary cpk-ok">Apply</button></div></div>`);
  const sv = el.querySelector('.cpk-sv'), dot = el.querySelector('.cpk-dot'), hue = el.querySelector('.cpk-hue');
  const hex = el.querySelector('.cpk-hex'), now = el.querySelector('.cpk-now');
  const cur = () => fromHsv(hh, sat, val);
  const draw = (typed = false) => {
    sv.style.background = `linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent), ${fromHsv(hh, 1, 1)}`;
    dot.style.left = `${sat * 100}%`; dot.style.top = `${(1 - val) * 100}%`;
    hue.value = String(Math.round(hh));
    now.style.background = cur();
    if (!typed) hex.value = cur();
  };
  const at = (e) => {
    const r = sv.getBoundingClientRect();
    sat = Math.max(0, Math.min(1, (e.clientX - r.left) / r.width));
    val = Math.max(0, Math.min(1, 1 - (e.clientY - r.top) / r.height));
    draw();
  };
  sv.addEventListener('pointerdown', (e) => { e.preventDefault(); sv.setPointerCapture(e.pointerId); at(e); });
  sv.addEventListener('pointermove', (e) => { if (sv.hasPointerCapture(e.pointerId)) at(e); });
  hue.addEventListener('input', () => { hh = +hue.value; draw(); });
  hex.addEventListener('input', () => {
    const c = normHex(hex.value);
    hex.classList.toggle('bad', !c);
    if (c) { [hh, sat, val] = toHsv(c); draw(true); }
  });
  hex.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); const c = normHex(hex.value); if (c) onPick(c); } });
  el.querySelector('.cpk-ok').addEventListener('click', () => onPick(normHex(hex.value) || cur()));
  el.querySelector('.cpk-drop').addEventListener('click', async () => {
    const c = normHex(await pickFromScreen());
    if (c) { [hh, sat, val] = toHsv(c); draw(); onPick(c); }
  });
  draw();
  return el;
}

/** A hex field next to a colour: type or paste "#1a9e32" and press Enter. */
export function hexInput(value, onSet) {
  const inp = h(`<input class="hex-in" spellcheck="false" maxlength="7" value="${esc(value || '')}" placeholder="#rrggbb">`);
  const commit = () => {
    const c = normHex(inp.value);
    inp.classList.toggle('bad', !!inp.value && !c);
    if (c && c !== normHex(value)) onSet(c);
  };
  inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') { e.preventDefault(); commit(); } });
  inp.addEventListener('change', commit);
  return inp;
}

