// Quire — the user guide, shown inside the app (Help → User guide, F1).
// Renders ui/help/guide.md: chapters on the left, search, screenshots, and
// test checkboxes whose state is remembered on this computer.

import { icon } from './icons.js';
import { esc, h } from './util.js';

// relative to this file, so it loads wherever the UI is served from
const BASE = new URL('./help/', import.meta.url).href;
const SRC = BASE + 'guide.md';
const DONE_KEY = 'quire.guide.checks';

/** GitHub-style anchor for a heading. */
const slug = (t) => t.toLowerCase().replace(/[^\p{L}\p{N}\s-]/gu, '').trim().replace(/\s+/g, '-');

/** Inline Markdown: code, images, links, bold, italic. */
function inline(t) {
  const code = [];
  let s = esc(t).replace(/`([^`]+)`/g, (_, c) => `\u0000${code.push(c) - 1}\u0000`);
  s = s.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_, alt, src) => `<img src="${BASE}${src}" alt="${alt}" width="1440" height="900" loading="lazy">`);
  s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, txt, href) => (href.startsWith('#') ? `<a href="${href}" data-anchor="${href.slice(1)}">${txt}</a>` : `<a href="${href}" target="_blank" rel="noopener">${txt}</a>`));
  s = s.replace(/\*\*([^*]+)\*\*/g, '<b>$1</b>').replace(/(^|[^*])\*([^*\s][^*]*)\*/g, '$1<i>$2</i>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${code[+i]}</code>`);
}

/** The guide's Markdown as HTML, with its chapters. */
function render(md) {
  const lines = md.replace(/\r/g, '').split('\n');
  const out = [];
  const toc = [];
  let i = 0, box = 0;
  const done = (() => { try { return JSON.parse(localStorage.getItem(DONE_KEY) || '{}'); } catch { return {}; } })();
  while (i < lines.length) {
    const l = lines[i];
    if (!l.trim()) { i++; continue; }
    let m;
    if ((m = l.match(/^(#{1,4})\s+(.*)$/))) {
      const lvl = m[1].length, text = m[2], id = slug(text);
      if (lvl === 2) toc.push([id, text]);
      out.push(`<h${lvl} id="${id}">${inline(text)}</h${lvl}>`);
      i++; continue;
    }
    if (/^---+$/.test(l.trim())) { out.push('<hr>'); i++; continue; }
    // a fenced code block, kept as written
    if (/^```/.test(l)) {
      const code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) { code.push(lines[i]); i++; }
      i++;
      out.push(`<pre class="hg-code"><code>${esc(code.join('\n'))}</code></pre>`);
      continue;
    }
    if (l.startsWith('|')) {
      const rows = [];
      while (i < lines.length && lines[i].startsWith('|')) { rows.push(lines[i]); i++; }
      const cells = (r) => r.replace(/^\||\|$/g, '').split('|').map((c) => c.trim());
      const head = cells(rows[0]);
      const body = rows.slice(2).map(cells);
      out.push(`<table><thead><tr>${head.map((c) => `<th>${inline(c)}</th>`).join('')}</tr></thead><tbody>${body.map((r) => `<tr>${r.map((c) => {
        if (c === '☐') { const k = `t${box++}`; return `<td><input type="checkbox" data-check="${k}"${done[k] ? ' checked' : ''}></td>`; }
        return `<td>${inline(c)}</td>`;
      }).join('')}</tr>`).join('')}</tbody></table>`);
      continue;
    }
    if (/^\s*(-|\d+\.)\s+/.test(l)) {
      const ordered = /^\s*\d+\./.test(l);
      const items = [];
      while (i < lines.length && /^\s*(-|\d+\.)\s+/.test(lines[i])) {
        let t = lines[i].replace(/^\s*(-|\d+\.)\s+/, '');
        i++;
        // a wrapped item continues on indented lines
        while (i < lines.length && /^\s{2,}\S/.test(lines[i]) && !/^\s*(-|\d+\.)\s+/.test(lines[i])) { t += ' ' + lines[i].trim(); i++; }
        const task = t.match(/^\[( |x)\]\s+(.*)$/);
        if (task) { const k = `c${box++}`; items.push(`<li class="task"><label><input type="checkbox" data-check="${k}"${done[k] ? ' checked' : ''}><span>${inline(task[2])}</span></label></li>`); }
        else items.push(`<li>${inline(t)}</li>`);
      }
      out.push(`<${ordered ? 'ol' : 'ul'}>${items.join('')}</${ordered ? 'ol' : 'ul'}>`);
      continue;
    }
    // a paragraph: lines until a blank line or another block; a run of images becomes a strip
    const para = [];
    while (i < lines.length && lines[i].trim() && !/^(#|\||---|```|\s*(-|\d+\.)\s)/.test(lines[i])) { para.push(lines[i]); i++; }
    const text = para.join(' ');
    out.push(/^(!\[[^\]]*\]\([^)]+\)\s*)+$/.test(text) ? `<div class="hg-shots">${inline(text)}</div>` : `<p>${inline(text)}</p>`);
  }
  return { html: out.join('\n'), toc };
}

let open = null;

/** Opens the guide; `anchor` jumps to a chapter. */
export async function showGuide(anchor) {
  if (open) { open.remove(); open = null; }
  let md = '';
  try { md = await (await fetch(SRC, { cache: 'no-store' })).text(); } catch { md = '# User guide\n\nThe guide could not be loaded.'; }
  const { html, toc } = render(md);
  const el = h(`<div class="hg-back"><div class="hg">
    <div class="hg-side">
      <div class="hg-title">${icon('book', 18)}<b>User guide</b></div>
      <input class="hg-search" placeholder="Search the guide" spellcheck="false">
      <nav class="hg-toc">${toc.map(([id, t]) => `<button data-anchor="${id}">${esc(t)}</button>`).join('')}</nav>
      <div class="hg-progress"></div>
    </div>
    <div class="hg-main"><button class="tb-btn hg-close" title="Close (Esc)">${icon('x', 18)}</button><article class="hg-doc">${html}</article></div>
  </div></div>`);
  document.body.append(el);
  open = el;
  const doc = el.querySelector('.hg-doc'), main = el.querySelector('.hg-main');
  const go = (id) => doc.querySelector(`#${CSS.escape(id)}`)?.scrollIntoView({ block: 'start' });
  el.addEventListener('click', (e) => {
    const a = e.target.closest('[data-anchor]');
    if (a) { e.preventDefault(); go(a.dataset.anchor); }
    if (e.target === el) close();
    // a screenshot opens larger
    const img = e.target.closest('.hg-doc img');
    if (img) img.classList.toggle('zoom');
  });
  // checklist ticks are remembered on this computer
  const progress = () => {
    const all = [...doc.querySelectorAll('[data-check]')];
    const n = all.filter((x) => x.checked).length;
    el.querySelector('.hg-progress').innerHTML = `${icon('check', 13)} ${n} of ${all.length} checks done${n ? ' · <button class="hg-reset">Clear</button>' : ''}`;
    el.querySelector('.hg-reset')?.addEventListener('click', () => { try { localStorage.removeItem(DONE_KEY); } catch { /* storage off */ } all.forEach((x) => (x.checked = false)); progress(); });
  };
  doc.addEventListener('change', (e) => {
    const c = e.target.closest('[data-check]');
    if (!c) return;
    try { const d = JSON.parse(localStorage.getItem(DONE_KEY) || '{}'); d[c.dataset.check] = c.checked; localStorage.setItem(DONE_KEY, JSON.stringify(d)); } catch { /* storage off */ }
    progress();
  });
  progress();
  // search: highlight chapters that mention the words, jump to the first match
  const q = el.querySelector('.hg-search');
  q.addEventListener('keydown', (e) => e.stopPropagation());
  q.addEventListener('input', () => {
    const words = q.value.toLowerCase().split(/\s+/).filter(Boolean);
    const heads = [...doc.querySelectorAll('h2')];
    el.querySelectorAll('.hg-toc button').forEach((b, k) => {
      const h2 = heads[k];
      let text = '';
      for (let n = h2?.nextElementSibling; n && n.tagName !== 'H2'; n = n.nextElementSibling) text += ' ' + n.textContent;
      const hit = !words.length || words.every((w) => (h2.textContent + text).toLowerCase().includes(w));
      b.classList.toggle('dim', !hit);
    });
    const first = [...el.querySelectorAll('.hg-toc button')].find((b) => !b.classList.contains('dim'));
    if (words.length && first) go(first.dataset.anchor);
  });
  const close = () => { el.remove(); open = null; window.removeEventListener('keydown', onKey, true); };
  const onKey = (e) => { if (e.key === 'Escape') { e.stopPropagation(); close(); } };
  window.addEventListener('keydown', onKey, true);
  el.querySelector('.hg-close').addEventListener('click', close);
  if (anchor) setTimeout(() => go(anchor), 50);
}
