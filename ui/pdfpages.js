// A PDF as pictures: its pages drawn by pdf.js (bundled in ui/vendor/pdfjs, Apache 2.0 — works
// offline), as JPEGs of at most `side` px on the long side with small thumbnails. A long file gives
// up to `max` pages spread evenly over it (as a GIF's frames are picked).

let lib = null;
async function pdfjs() {
  if (!lib) {
    lib = await import('./vendor/pdfjs/pdf.min.mjs');
    lib.GlobalWorkerOptions.workerSrc = new URL('./vendor/pdfjs/pdf.worker.min.mjs', import.meta.url).href;
  }
  return lib;
}

/** Whether a file is a PDF (by its type, or its name when the type is missing). */
export const isPdf = (f) => !!f && (f.type === 'application/pdf' || /\.pdf$/i.test(f.name || ''));

/** The pages to take from `n`: all of them, or `max` spread evenly (first and last included). */
export function pickPages(n, max) {
  if (n <= max) return Array.from({ length: n }, (_, i) => i + 1);
  return Array.from({ length: max }, (_, k) => 1 + Math.round((k * (n - 1)) / (max - 1)));
}

/** The pages of a PDF file as { media, data (base64), thumb (data URL) }, and how many pages it has. */
export async function pdfPictures(file, { max = 8, side = 1568 } = {}) {
  const pdf = await pdfjs();
  let doc;
  try {
    doc = await pdf.getDocument({
      data: new Uint8Array(await file.arrayBuffer()),
      standardFontDataUrl: new URL('./vendor/pdfjs/standard_fonts/', import.meta.url).href,
      isEvalSupported: false,
    }).promise;
  } catch (e) {
    throw new Error(/password/i.test(String(e?.name || e)) ? 'That PDF is protected with a password.' : 'That PDF can’t be read.');
  }
  try {
    const pages = [];
    const facts = { fonts: new Map(), colors: new Map(), exact: new Set(), pixels: 0 };
    for (const n of pickPages(doc.numPages, max)) {
      const page = await doc.getPage(n);
      const v0 = page.getViewport({ scale: 1 });
      const scale = side / Math.max(v0.width, v0.height);
      const vp = page.getViewport({ scale });
      const c = document.createElement('canvas');
      c.width = Math.max(1, Math.round(vp.width));
      c.height = Math.max(1, Math.round(vp.height));
      const g = c.getContext('2d');
      g.fillStyle = '#fff';
      g.fillRect(0, 0, c.width, c.height);
      await page.render({ canvasContext: g, viewport: vp }).promise;
      const full = c.toDataURL('image/jpeg', 0.9);
      const t = document.createElement('canvas');
      const k = 240 / Math.max(c.width, c.height);
      t.width = Math.max(1, Math.round(c.width * k));
      t.height = Math.max(1, Math.round(c.height * k));
      t.getContext('2d').drawImage(c, 0, 0, t.width, t.height);
      pages.push({ media: 'image/jpeg', data: full.split(',')[1], thumb: t.toDataURL('image/jpeg', 0.7), page: n });
      await readFacts(pdf, page, g, c, facts);
      page.cleanup();
    }
    return { pages, total: doc.numPages, facts: factsText(facts) };
  } finally {
    doc.destroy();
  }
}

// ---- what the PDF itself says about its look: its typefaces (where and how big) and its colours
// (exact, as the file draws them, by how much of the pages they cover)

const hex = (r, g, b) => '#' + [r, g, b].map((x) => Math.round(x).toString(16).padStart(2, '0')).join('');
const dist = (a, b) => { const p = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16)); const [x, y] = [p(a), p(b)]; return Math.hypot(x[0] - y[0], x[1] - y[1], x[2] - y[2]); };

async function readFacts(pdf, page, g, canvas, f) {
  // typefaces: each text run's font (its real name, once drawn), size and length
  try {
    const tc = await page.getTextContent();
    for (const it of tc.items) {
      if (!it.str || !it.str.trim()) continue;
      let name = '';
      try { name = page.commonObjs.has(it.fontName) ? page.commonObjs.get(it.fontName)?.name || '' : ''; } catch {}
      name = String(name || tc.styles?.[it.fontName]?.fontFamily || it.fontName).replace(/^[A-Z]{6}\+/, '');
      const size = Math.hypot(it.transform[2], it.transform[3]);
      const e = f.fonts.get(name) || { chars: 0, min: Infinity, max: 0, weighted: 0 };
      const n = it.str.trim().length;
      e.chars += n; e.min = Math.min(e.min, size); e.max = Math.max(e.max, size); e.weighted += size * n;
      f.fonts.set(name, e);
    }
  } catch {}
  // colours the file names exactly (fills)
  try {
    const ops = await page.getOperatorList();
    for (let i = 0; i < ops.fnArray.length; i++) {
      if (ops.fnArray[i] === pdf.OPS.setFillRGBColor) {
        const a = ops.argsArray[i];
        // a "#rrggbb" string, or three numbers (0–255, or 0–1)
        const n = a && typeof a[0] === 'number' && a.length >= 3 ? [a[0], a[1], a[2]] : null;
        const k = n && Math.max(...n) <= 1 ? 255 : 1;
        const c = typeof a?.[0] === 'string' ? a[0] : n ? hex(n[0] * k, n[1] * k, n[2] * k) : null;
        if (c && /^#[0-9a-f]{6}$/i.test(c)) f.exact.add(c.toLowerCase());
      }
    }
  } catch {}
  // how much of the page each colour covers (every 3rd pixel, colours grouped)
  try {
    const { data } = g.getImageData(0, 0, canvas.width, canvas.height);
    for (let i = 0; i < data.length; i += 4 * 3) {
      const key = ((data[i] >> 4) << 8) | ((data[i + 1] >> 4) << 4) | (data[i + 2] >> 4);
      const e = f.colors.get(key) || { n: 0, r: 0, g: 0, b: 0 };
      e.n++; e.r += data[i]; e.g += data[i + 1]; e.b += data[i + 2];
      f.colors.set(key, e);
      f.pixels++;
    }
  } catch {}
}

function factsText(f) {
  const out = [];
  const fonts = [...f.fonts.entries()].filter(([, e]) => e.chars > 3).sort((a, b) => b[1].chars - a[1].chars);
  if (fonts.length) {
    out.push('Typefaces (as named in the file; by how much text uses them):');
    for (const [name, e] of fonts.slice(0, 8)) {
      const avg = e.weighted / e.chars;
      // by size on the page: big type is headings and figures, the smallest is small print (footnotes, legal)
      const role = e.max >= 18 ? `headings / big figures (up to ${e.max.toFixed(0)} pt)` : avg < 8 ? 'small print' : 'body text';
      out.push(`- ${name}: ${role}, ${e.min.toFixed(0)}–${e.max.toFixed(0)} pt, mostly ${avg.toFixed(0)} pt (${Math.round((100 * e.chars) / fonts.reduce((t, x) => t + x[1].chars, 0))}% of the text)`);
    }
  }
  // the colours covering the most paper; snapped to the file's exact colours where one is close
  const exact = [...f.exact];
  const merged = [];
  for (const e of [...f.colors.values()].sort((a, b) => b.n - a.n)) {
    const c = hex(e.r / e.n, e.g / e.n, e.b / e.n);
    const near = merged.find((m) => dist(m.c, c) < 36);
    if (near) { near.n += e.n; continue; }
    if (merged.length < 40) merged.push({ c, n: e.n });
  }
  const snapped = [];
  for (const m of merged) {
    const x = exact.reduce((best, h) => (dist(h, m.c) < (best ? dist(best, m.c) : 40) ? h : best), null);
    const c = x || m.c;
    const same = snapped.find((s) => s.c === c);
    if (same) same.share += m.n / Math.max(1, f.pixels);
    else snapped.push({ c, exact: !!x, share: m.n / Math.max(1, f.pixels) });
  }
  const shown = snapped.sort((a, b) => b.share - a.share).filter((m) => m.share >= 0.004).slice(0, 12);
  if (shown.length) {
    out.push('Colours by how much of the pages they cover (exact = as the file draws it; others are from the pictures, photos included):');
    out.push(shown.map((m) => `${m.c} ${(m.share * 100).toFixed(1)}%${m.exact ? ' exact' : ''}`).join(' · '));
  }
  if (exact.length) out.push(`Fill colours the file uses: ${exact.slice(0, 24).join(', ')}`);
  return out.join('\n');
}
