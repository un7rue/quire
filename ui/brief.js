// A report assistant's brief & pitch, three ways: highlighted in the pane, as rich text for an
// e-mail (inline styles, so mail programs keep them) and as a PDF in the report's own colours and
// typefaces, headed with the client and the period. The model marks the main ideas with **…**;
// key figures (with a unit, a sign or thousands) are found here and coloured by their sign.

import { esc, tokenColor } from './util.js';
import { appLanguage } from './i18n.js';

// a figure: an optional sign, digits (thousands may be spaced), decimals, a unit
const NUM = /(?<![\p{L}\d.,\-\[*\/])([+\-−]?)((?:[₽$€]\s?)?)((?:\d{1,3}(?:[   ]\d{3})+|\d+)(?:[.,]\d+)?)((?:\s?(?:%|п\.\s?п\.|б\.\s?п\.|pp|bp)(?![\p{L}]))|(?:[  ]?(?:млрд|млн|тыс\.|bn|billion|million|mln|M|K)(?![\p{L}])(?:[  ]?(?:₽|руб\.|RUB|USD|EUR|\$|€))?)|(?:[  ]?(?:₽|руб\.|RUB|USD|EUR)))?(?!\d|[.,]\d|-\d)/gu;
// "page 3" / "стр. 3" stay plain
const PAGE_BEFORE = /(?:стр\.|p\.|page|pages|стр)\s*$/i;

/** Text as runs: {t, b (bold), c ('good' | 'bad' | 'key' | 'num' | null)}. `tone` colours the marked ideas. */
export function runsOf(text, tone = '') {
  const out = [];
  String(text || '').split(/\*\*(.+?)\*\*/s).forEach((seg, k) => {
    if (!seg) return;
    if (k % 2) { out.push({ t: seg, b: true, c: tone === 'good' || tone === 'bad' ? tone : 'key' }); return; }
    let last = 0;
    for (const m of seg.matchAll(NUM)) {
      const [all, sign, cur, digits, unit] = m;
      const grouped = /[ \u00a0\u202f]/.test(digits) || digits.replace(/\D/g, '').length >= 5;
      if (!(sign || cur || unit || grouped) || PAGE_BEFORE.test(seg.slice(0, m.index))) continue;
      if (m.index > last) out.push({ t: seg.slice(last, m.index) });
      out.push({ t: all, b: true, c: sign === '+' ? 'good' : sign ? 'bad' : 'num' });
      last = m.index + all.length;
    }
    if (last < seg.length) out.push({ t: seg.slice(last) });
  });
  return out;
}

/** The text as HTML for the pane (classes) or an e-mail (`colours`: inline styles). */
export function rich(text, tone = '', colours = null) {
  return runsOf(text, tone).map((r) => {
    if (!r.b) return esc(r.t);
    if (!colours) return `<b class="hl ${r.c}">${esc(r.t)}</b>`;
    return `<b style="color:${colours[r.c] || colours.ink};font-weight:700">${esc(r.t)}</b>`;
  }).join('');
}

// ---------------------------------------------------------------- the report's look

const L = {
  en: { kicker: 'Brief for the client meeting', summary: 'Summary', attention: 'Pay attention', present: 'How to present it', opening: 'Opening', messages: 'Key messages', asks: 'If the client asks', closing: 'Closing', page: 'p.', foot: 'For internal use — not for the client', prepared: 'Prepared', of: 'of' },
  ru: { kicker: 'Бриф к встрече с клиентом', summary: 'Коротко', attention: 'На что обратить внимание', present: 'Как представить отчёт', opening: 'Начало', messages: 'Главное', asks: 'Если клиент спросит', closing: 'Завершение', page: 'стр.', foot: 'Для внутреннего использования — не для клиента', prepared: 'Подготовлено', of: 'из' },
};
const words = () => L[appLanguage() === 'ru' ? 'ru' : 'en'];

// a theme colour may be a $token; anything unreadable falls back
function colour(th, v, dflt) {
  if (!v) return dflt;
  if (v[0] === '$') v = th?.tokens?.[v.slice(1)] ? tokenColor(th, v) : th?.[v.slice(1)] || '';
  return /^#[0-9a-f]{3,8}$/i.test(v) || /^rgb/i.test(v) ? v : dflt;
}

// hue (degrees), saturation and lightness of a #hex colour
function hsl(hex) {
  let v = hex.replace('#', '');
  if (v.length < 6) v = [...v.slice(0, 3)].map((x) => x + x).join('');
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(v.slice(i, i + 2), 16) / 255);
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b), l = (mx + mn) / 2, d = mx - mn;
  if (!d) return [0, 0, l];
  const h = mx === r ? ((g - b) / d) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return [(h * 60 + 360) % 360, d / (1 - Math.abs(2 * l - 1)), l];
}
const hueGap = (a, b) => { const d = Math.abs(hsl(a)[0] - hsl(b)[0]) % 360; return Math.min(d, 360 - d); };

/** The report's colours and typefaces (its theme), for the e-mail and the PDF. */
export function lookOf(st) {
  const th = st?.doc?.theme || {};
  const f = st?.fonts || {};
  const accent = colour(th, th.accent, '') || colour(th, th.palette?.[0], '#1f4e8c');
  const good = colour(th, th.good, '#1d7f3a'), bad = colour(th, th.bad, '#c4311f'), ink = colour(th, th.ink, '#1d1d1b');
  // the highlight must not read as a gain or a loss: the first theme colour far from both, dark enough to read
  const key = [accent, ...(th.palette || []).map((x) => colour(th, x, ''))].filter((x) => /^#/.test(x))
    .find((x) => { const [, sat, l] = hsl(x); return sat > 0.2 && l < 0.5 && hueGap(x, good) > 40 && hueGap(x, bad) > 40; }) || ink;
  return {
    ink, muted: colour(th, th.muted, '#6b6a65'), grid: colour(th, th.grid, '#e2e0da'),
    good, bad, key, num: key, accent,
    page: colour(th, th.page_bg, '#ffffff'),
    head: f.heading?.family || f.title?.family || th.font_family || 'Inter',
    body: f.body?.family || th.font_family || 'Inter',
  };
}

// ---------------------------------------------------------------- e-mail

/** The whole brief as HTML with inline styles, to paste for yourself or a colleague (it is internal). */
export function briefEmail(r, st) {
  const c = lookOf(st), w = words(), hd = r.heading || {};
  const font = `font-family:'${c.body}',Arial,Helvetica,sans-serif;`;
  const h2 = (t) => `<p style="${font}margin:18px 0 6px;font-size:12px;letter-spacing:.06em;text-transform:uppercase;color:${c.accent};font-weight:700">${esc(t)}</p>`;
  const p = (html, extra = '') => `<p style="${font}margin:0 0 8px;font-size:14px;line-height:1.5;color:${c.ink};${extra}">${html}</p>`;
  const pg = (n) => (n ? ` <span style="color:${c.muted};font-size:12px">(${w.page} ${esc(String(n))})</span>` : '');
  const quote = (t) => p(`“${rich(t, '', c)}”`, `border-left:3px solid ${c.accent};padding:2px 0 2px 10px;font-style:italic;`);
  const pi = r.pitch || {};
  const toneDot = (t) => `<span style="color:${t === 'good' ? c.good : t === 'bad' ? c.bad : c.muted}">●</span> `;
  const parts = [];
  {
    parts.push(`<p style="${font}margin:0 0 2px;font-size:11px;letter-spacing:.08em;text-transform:uppercase;color:${c.muted}">${esc(w.kicker)}</p>`);
    if (hd.client || hd.title) parts.push(`<p style="font-family:'${c.head}',Georgia,serif;margin:0 0 2px;font-size:20px;font-weight:700;color:${c.ink}">${esc(hd.client || hd.title)}</p>`);
    if (hd.title || hd.period) parts.push(`<p style="${font}margin:0 0 10px;font-size:13px;color:${c.muted}">${esc([hd.client ? hd.title : '', hd.period].filter(Boolean).join(' · '))}</p>`);
    if (r.summary) parts.push(h2(w.summary), p(rich(r.summary, '', c)));
    if ((r.attention || []).length) parts.push(h2(w.attention), ...r.attention.map((a) => p(`${toneDot(a.tone)}${rich(a.text, a.tone, c)}${pg(a.page)}`)));
    parts.push(h2(w.present));
  }
  if (pi.opening) parts.push(quote(pi.opening));
  if ((pi.messages || []).length) parts.push(`<ol style="${font}margin:6px 0 8px 0;padding-left:22px;font-size:14px;line-height:1.5;color:${c.ink}">${pi.messages.map((x) => `<li style="margin:0 0 6px">${rich(x.text, '', c)}${pg(x.page)}</li>`).join('')}</ol>`);
  if ((pi.questions || []).length) parts.push(h2(w.asks), ...pi.questions.map((q) => p(`<b style="color:${c.ink}">${esc(q.q)}</b><br>${rich(q.a, '', c)}`)));
  if (pi.closing) parts.push(quote(pi.closing));
  return `<div style="${font}color:${c.ink};max-width:680px">${parts.join('')}</div>`;
}

/** Puts HTML (with a plain-text twin) on the clipboard. */
export async function copyHtml(html, text) {
  try {
    await navigator.clipboard.write([new ClipboardItem({ 'text/html': new Blob([html], { type: 'text/html' }), 'text/plain': new Blob([text], { type: 'text/plain' }) })]);
    return true;
  } catch {
    // older web views: copy a selection of the rendered HTML
    const box = document.createElement('div');
    box.contentEditable = 'true';
    box.style.cssText = 'position:fixed;left:-9999px;top:0;opacity:0';
    box.innerHTML = html;
    document.body.append(box);
    const range = document.createRange();
    range.selectNodeContents(box);
    const sel = getSelection();
    sel.removeAllRanges(); sel.addRange(range);
    const ok = document.execCommand('copy');
    sel.removeAllRanges(); box.remove();
    return ok;
  }
}

// ---------------------------------------------------------------- PDF

const W = 595.28, H = 841.89, M = 54; // A4 portrait, points
const xml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** The brief as A4 pages of SVG (for `brief_pdf`), in the report's colours and typefaces. */
export function briefPages(r, st) {
  const c = lookOf(st), w = words(), hd = r.heading || {};
  const ctx = document.createElement('canvas').getContext('2d');
  const measure = (t, size, bold, fam) => { ctx.font = `${bold ? 700 : 400} ${size}px "${fam}", Arial`; return ctx.measureText(t).width; };
  const pages = [];
  let body = [], y = 0;
  const startPage = (first) => {
    body = [];
    if (first) {
      body.push(`<rect x="0" y="0" width="${W}" height="6" fill="${c.accent}"/>`);
      y = M + 4;
      const line = (t, size, bold, fill, fam, gap, upper, track = 0) => {
        body.push(`<text x="${M}" y="${y + size}" font-family="${xml(fam)}" font-size="${size}" font-weight="${bold ? 700 : 400}" fill="${fill}"${track ? ` letter-spacing="${track}"` : ''}>${xml(upper ? t.toUpperCase() : t)}</text>`);
        y += size * 1.25 + gap;
      };
      line(w.kicker, 8.5, true, c.muted, c.body, 6, true, 1);
      line(hd.client || hd.title || '', 24, true, c.ink, c.head, 2);
      const sub = [hd.client ? hd.title : '', hd.period].filter(Boolean).join(' · ');
      if (sub) line(sub, 11, false, c.muted, c.body, 4);
      line(`${w.prepared} ${new Date().toLocaleDateString(appLanguage() === 'ru' ? 'ru-RU' : 'en-GB', { day: 'numeric', month: 'long', year: 'numeric' })}`, 8.5, false, c.muted, c.body, 10);
      body.push(`<line x1="${M}" y1="${y}" x2="${W - M}" y2="${y}" stroke="${c.grid}" stroke-width="0.8"/>`);
      y += 14;
    } else {
      body.push(`<rect x="0" y="0" width="${W}" height="3" fill="${c.accent}"/>`);
      y = M;
    }
  };
  const endPage = () => pages.push(body);
  const room = (need) => { if (y + need > H - M - 18) { endPage(); startPage(false); } };

  // lays out runs in a box: wraps by words, keeps each run's look; returns lines of [{t, b, c}]
  const wrap = (runs, size, width, fam) => {
    const lines = [[]];
    let x = 0;
    for (const run of runs) {
      for (const tok of run.t.split(/(\s+)/)) {
        if (!tok) continue;
        const space = /^\s+$/.test(tok);
        const wd = measure(space ? ' ' : tok, size, run.b, fam);
        if (!space && x + wd > width && lines[lines.length - 1].length) { lines.push([]); x = 0; }
        if (space && x === 0) continue;
        const line = lines[lines.length - 1];
        const prev = line[line.length - 1];
        const t = space ? ' ' : tok;
        if (prev && prev.b === run.b && prev.c === run.c) prev.t += t; else line.push({ t, b: run.b, c: run.c });
        x += wd;
      }
    }
    return lines.map((l) => { const z = l[l.length - 1]; if (z) z.t = z.t.replace(/\s+$/, ''); return l; });
  };
  const fill = (cc) => (cc ? c[cc] || c.ink : c.ink);
  const para = (runs, { size = 10.5, indent = 0, lead = 1.45, gap = 6, italic = false, fam = c.body, before = null } = {}) => {
    const lines = wrap(runs, size, W - 2 * M - indent, fam);
    const lh = size * lead;
    room(Math.min(lines.length, 2) * lh);
    let first = true;
    for (const l of lines) {
      room(lh);
      if (first && before) before(y + size);
      first = false;
      body.push(`<text x="${M + indent}" y="${y + size}" font-family="${xml(fam)}" font-size="${size}"${italic ? ' font-style="italic"' : ''} fill="${c.ink}">${l.map((s) => `<tspan font-weight="${s.b ? 700 : 400}" fill="${fill(s.c)}">${xml(s.t)}</tspan>`).join('')}</text>`);
      y += lh;
    }
    y += gap;
  };
  const heading = (t) => {
    room(40);
    y += 8;
    body.push(`<text x="${M}" y="${y + 9}" font-family="${xml(c.body)}" font-size="9" font-weight="700" letter-spacing="0.9" fill="${c.accent}">${xml(t.toUpperCase())}</text>`);
    y += 18;
  };
  const pgRun = (n) => (n ? [{ t: `  ${w.page} ${n}`, c: 'muted' }] : []);
  const quote = (t) => {
    const y0 = y;
    para(runsOf(`“${t}”`), { size: 11.5, indent: 12, italic: true, fam: c.head, gap: 0 });
    body.push(`<rect x="${M}" y="${y0 + 1}" width="2.5" height="${y - y0 - 2}" fill="${c.accent}"/>`);
    y += 8;
  };

  startPage(true);
  if (r.summary) { heading(w.summary); para(runsOf(r.summary), { size: 11 }); }
  if ((r.attention || []).length) {
    heading(w.attention);
    for (const a of r.attention) {
      const dot = a.tone === 'good' ? c.good : a.tone === 'bad' ? c.bad : c.muted;
      para([...runsOf(a.text, a.tone), ...pgRun(a.page)], { indent: 14, before: (base) => body.push(`<circle cx="${M + 4}" cy="${base - 3.6}" r="3" fill="${dot}"/>`) });
    }
  }
  const pi = r.pitch || {};
  heading(w.present);
  if (pi.opening) quote(pi.opening);
  (pi.messages || []).forEach((x, k) => para([...runsOf(x.text), ...pgRun(x.page)], { indent: 18, before: (base) => body.push(`<text x="${M}" y="${base}" font-family="${xml(c.head)}" font-size="11" font-weight="700" fill="${c.accent}">${k + 1}</text>`) }));
  if ((pi.questions || []).length) {
    heading(w.asks);
    for (const q of pi.questions) {
      room(40);
      para([{ t: q.q, b: true }], { gap: 2 });
      para(runsOf(q.a), { indent: 10, gap: 8 });
    }
  }
  if (pi.closing) { y += 4; quote(pi.closing); }
  endPage();

  // every page: the background, and a footer with the client and page numbers
  const foot = [hd.client, hd.period].filter(Boolean).join(' · ');
  return pages.map((b, i) => `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
<rect width="${W}" height="${H}" fill="${c.page}"/>${b.join('\n')}
<line x1="${M}" y1="${H - M + 6}" x2="${W - M}" y2="${H - M + 6}" stroke="${c.grid}" stroke-width="0.6"/>
<text x="${M}" y="${H - M + 20}" font-family="${xml(c.body)}" font-size="7.5" fill="${c.muted}">${xml(`${w.foot}${foot ? ` · ${foot}` : ''}`)}</text>
<text x="${W - M}" y="${H - M + 20}" text-anchor="end" font-family="${xml(c.body)}" font-size="7.5" fill="${c.muted}">${i + 1} ${w.of} ${pages.length}</text>
</svg>`);
}

/** The brief's PDF size, for `brief_pdf`. */
export const PDF_SIZE = { width: W, height: H };

// ---------------------------------------------------------------- the e-mail to the client

// page references are for the banker, not the client
const noPages = (t) => String(t || '').replace(/\s*\((?:стр\.|p\.|page|pages)\s*[\d,\s–-]+\)/gi, '').replace(/\s*(?:стр\.|p\.)\s*\d+(?:[,\s–-]+\d+)*\.?$/i, '');

/**
 * The e-mail to send the client with the report: the brief's own (`email`), or — for briefs made
 * before it had one — the pitch's opening, messages and closing. Signed with `sender`.
 * Answers {subject, html, text}.
 */
export function clientEmail(r, st, sender = '') {
  const hd = r.heading || {}, ru = /^ru/.test(hd.locale || '') || /[а-яё]/i.test(r.pitch?.opening || r.email?.body || '');
  let subject = r.email?.subject || [ru ? 'Отчёт по Вашим активам' : 'Your portfolio report', hd.period].filter(Boolean).join(' — ');
  let paras;
  if (r.email?.body) paras = String(r.email.body).split(/\n\s*\n/);
  else {
    const p = r.pitch || {};
    paras = [p.opening, ...(p.messages || []).map((x) => x.text), p.closing, ru ? 'С уважением,' : 'Kind regards,'];
  }
  paras = paras.map((x) => noPages(x).trim()).filter(Boolean);
  if (sender) {
    const last = paras.length - 1;
    if (/^(с уважением|kind regards|best regards|regards|sincerely)[,.]?$/i.test(paras[last] || '')) paras[last] += `\n${sender}`;
    else paras.push(`${ru ? 'С уважением' : 'Kind regards'},\n${sender}`);
  }
  // to the client: only the marked ideas in bold, no colours
  const ink = '#222222';
  const bold = (x) => String(x).split(/\*\*(.+?)\*\*/s).map((seg, k) => (k % 2 ? `<b>${esc(seg)}</b>` : esc(seg))).join('');
  const html = `<div style="font-family:Arial,Helvetica,sans-serif;font-size:14px;line-height:1.55;color:${ink}">${paras.map((x) => `<p style="margin:0 0 12px">${bold(x).replace(/\n/g, '<br>')}</p>`).join('')}</div>`;
  const text = paras.map((x) => x.replace(/\*\*/g, '')).join('\n\n');
  subject = subject.replace(/\*\*/g, '');
  return { subject, html, text };
}
