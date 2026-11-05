// Quire — the interface language. English is the source; other languages
// translate the interface as it renders (text, tooltips, placeholders), never
// the report itself: pages (SVG), the user's inputs, code and data stay as they are.

import RU from './i18n-ru.js';

const DICTS = { ru: RU };
let lang = 'en';
export const appLanguage = () => lang;

// report content, the user's own text and code are never translated
const SKIP = 'svg, input, textarea, select.no-i18n, code, kbd, pre, [contenteditable="true"], .hg-doc, .pv-table, .var-val, .calc-vals, .no-i18n';
const ATTRS = ['title', 'placeholder', 'aria-label'];

/** Which plural form a number takes in Russian: 0 one (1, 21), 1 few (2–4, 22, 1.5), 2 many (5–20, 0). */
function pluralForm(n) {
  const x = Math.abs(parseFloat(String(n).replace(',', '.').replace(/\s/g, '')));
  if (!Number.isFinite(x)) return 2;
  if (!Number.isInteger(x)) return 1;
  const m10 = x % 10, m100 = x % 100;
  return m10 === 1 && m100 !== 11 ? 0 : m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14) ? 1 : 2;
}
/** An entry may give plural forms [one, few, many]: the first number in the text picks one. */
const pick = (r, nums) => (Array.isArray(r) ? r[pluralForm(nums[0] ?? 0)] : r);

/**
 * A string in the interface language, or null when there is nothing to translate.
 * `ctx`: where it shows (an element's data-i18n), for words that read differently
 * in different places — "ctx|text" entries come first.
 */
export function translate(s, ctx = '') {
  const d = DICTS[lang];
  if (!d || !s) return null;
  const t = s.trim();
  if (!t || !/[A-Za-z]/.test(t)) return null;
  let r = (ctx ? d[`${ctx}|${t}`] : undefined) ?? d[t];
  if (Array.isArray(r)) r = r[0];
  if (r == null) {
    // numbers and quoted names vary: "3 matches", "Icon for “Deposits”"
    const nums = [], qs = [];
    const toks = [], cols = [];
    const k = t.replace(/\{\{[^}]*\}\}/g, (x) => { toks.push(x); return '{t}'; }).replace(/#[0-9a-f]{3,8}\b/gi, (x) => { cols.push(x); return '{c}'; })
      .replace(/[“«]([^”»]*)[”»]/g, (_, q) => { qs.push(q); return '“{q}”'; }).replace(/\d+(?:[.,]\d+)?/g, (n) => { nums.push(n); return '{n}'; });
    if (k !== t && (r = (ctx ? d[`${ctx}|${k}`] : undefined) ?? d[k]) != null) r = pick(r, nums).replace(/\{n\}/g, () => nums.shift() ?? '').replace(/\{q\}/g, () => qs.shift() ?? '').replace(/\{t\}/g, () => toks.shift() ?? '').replace(/\{c\}/g, () => cols.shift() ?? '');
    else r = prefixed(d, t) ?? pieces(t);
    if (r == null) return null;
  }
  return s.replace(t, r);
}

/**
 * A fixed phrase around a name: "^phrase" matches the start ("SQL on Demo bank"),
 * "$phrase" the end ("Брокерский счёт · состав (continued)").
 */
function prefixed(d, t) {
  for (const k in d) {
    if (k[0] === '^' && t.startsWith(k.slice(1))) return d[k] + t.slice(k.length - 1);
    if (k[0] === '$' && t.endsWith(k.slice(1)) && t.length > k.length - 1) return t.slice(0, t.length - (k.length - 1)) + d[k];
  }
  return null;
}

/** Text built from pieces ("Paste data · 12 rows", "Shows: …"): each piece on its own. */
function pieces(t) {
  for (const sep of [' · ', ': ', ' — ', '. ']) {
    if (!t.includes(sep)) continue;
    const parts = t.split(sep);
    let any = false;
    const out = parts.map((p) => { const r = p && translate(p); if (r != null) { any = true; return r; } return p; });
    if (any) return out.join(sep);
  }
  return null;
}

/** Translates `t` for code that builds text itself (falls back to the English). */
export const tr = (t) => translate(t) ?? t;

const ctxOf = (el) => el?.closest?.('[data-i18n]')?.dataset.i18n || '';

function fixText(n) {
  const p = n.parentElement;
  if (!p || p.closest(SKIP)) return;
  const r = translate(n.nodeValue, ctxOf(p));
  if (r != null && r !== n.nodeValue) n.nodeValue = r;
}
function fixAttrs(el) {
  for (const a of ATTRS) {
    const v = el.getAttribute(a);
    if (!v) continue;
    const r = translate(v, ctxOf(el));
    if (r != null && r !== v) el.setAttribute(a, r);
  }
}
function walk(root) {
  if (lang === 'en' || !root) return;
  if (root.nodeType === 3) return fixText(root);
  if (root.nodeType !== 1) return;
  if (root.closest(SKIP)) { if (root.matches('input, textarea')) fixAttrs(root); return; }
  fixAttrs(root);
  const w = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
    acceptNode: (n) => {
      if (n.nodeType !== 1 || !n.matches(SKIP)) return NodeFilter.FILTER_ACCEPT;
      // an input's own text is the user's, but its placeholder and tooltip are ours
      if (n.matches('input, textarea')) fixAttrs(n);
      return NodeFilter.FILTER_REJECT;
    },
  });
  for (let n = w.nextNode(); n; n = w.nextNode()) {
    if (n.nodeType === 3) fixText(n);
    else fixAttrs(n);
  }
}

let observer = null;
/** Switches the interface language ('en' or 'ru'); `rerender` redraws what was already shown. */
export function setAppLanguage(l, rerender) {
  const next = DICTS[l] ? l : 'en';
  const was = lang;
  lang = next;
  document.documentElement.lang = next;
  if (next !== 'en' && !observer) {
    observer = new MutationObserver((list) => {
      if (lang === 'en') return;
      for (const m of list) {
        if (m.type === 'characterData') fixText(m.target);
        else if (m.type === 'attributes') fixAttrs(m.target);
        else m.addedNodes.forEach((n) => walk(n));
      }
    });
    observer.observe(document.body, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ATTRS });
  }
  if (next !== 'en') walk(document.body);
  // back to English: draw everything again from the source text
  if (was !== next && next === 'en') rerender?.();
}
