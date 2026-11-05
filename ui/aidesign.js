// The designer's AI pane, on the right of the template: describe a page and the assistant
// lays it out from the template's sources, widget types and design views — or changes the
// page in front (add, remove, rearrange, another chart type or view). Each answer is applied
// at once as one undo step; the app checks it first (fields exist, everything fits, every
// widget shows its data). The model sees the template's structure only, never data.
// In "Look" it restyles the template after a picture (a report, a deck, a brand page): the
// model reads the picture and the app builds the whole theme from what it saw.

import { isPdf, pdfPictures } from './pdfpages.js';
import { S, api, call, setState, onChange } from './state.js';
import { icon } from './icons.js';
import { esc } from './util.js';
import { toast } from './ui.js';
import { appLanguage } from './i18n.js';
import { chatStore, providerChip, historyMenu, feedbackHtml } from './aichat.js';
import { currentPageId } from './panels.js';
import { scrollToPage } from './canvas.js';

const EXAMPLES = {
  page: ['An overview page: total assets, result and return as KPI cards, allocation by asset class as a donut, and the 10 largest holdings in a table',
    'A page on income: monthly income as bars for the last 12 months and a table of income by account'],
  change: ['Put the KPI cards in one row of equal width', 'Show the allocation as a horizontal bar chart instead', 'Add a section heading above the charts'],
  look: ['Take the colours and the chart shapes, keep our typefaces', 'Only the colours', 'Everything, but keep gains green and losses red'],
};

const MAX_PICS = 8;

/** A file as base64 (for the app to read a GIF's frames). */
const base64Of = (file) => new Promise((ok, bad) => { const r = new FileReader(); r.onload = () => ok(String(r.result).split(',')[1]); r.onerror = () => bad(r.error); r.readAsDataURL(file); });

/** A picture made ready to send: at most 1568 px on its long side (what vision models read), and a small thumbnail. */
async function readPicture(file) {
  const url = URL.createObjectURL(file);
  try {
    const img = await new Promise((ok, bad) => { const i = new Image(); i.onload = () => ok(i); i.onerror = () => bad(new Error('That file isn’t a picture.')); i.src = url; });
    const scaled = (max, type, q) => {
      const k = Math.min(1, max / Math.max(img.naturalWidth, img.naturalHeight));
      const c = document.createElement('canvas');
      c.width = Math.max(1, Math.round(img.naturalWidth * k));
      c.height = Math.max(1, Math.round(img.naturalHeight * k));
      const g = c.getContext('2d');
      g.fillStyle = '#fff';
      g.fillRect(0, 0, c.width, c.height);
      g.drawImage(img, 0, 0, c.width, c.height);
      return c.toDataURL(type, q);
    };
    const full = scaled(1568, 'image/jpeg', 0.9);
    return { media: 'image/jpeg', data: full.split(',')[1], thumb: scaled(240, 'image/jpeg', 0.7) };
  } finally { URL.revokeObjectURL(url); }
}

/** Mounts the pane into `pane` (an empty aside). `onToggle(open)` shows its splitter. */
export function designAssistant({ pane, onToggle = () => {} }) {
  const chats = chatStore('quire.ai.chats.design');
  let all = chats.load();
  let chat = null;
  let busy = false;
  let mode = null; // 'page' (a new page), 'change' (the page in front) or 'look' (restyle); null = by the page
  let applied = null; // the last change: { i, snap } — Undo works while the template is as it left it
  let pics = []; // the pictures to restyle after: { media, data, thumb } (kept for follow-ups), up to MAX_PICS
  let facts = []; // what attached PDFs say about their look (typefaces, exact colours), sent with the pictures

  pane.innerHTML = `
    <div class="aip-head">
      <span class="aip-ic">${icon('sparkles', 15)}</span><b>AI assistant</b>
      <button class="aip-model" title="Which AI answers — click to change">…</button>
      <span class="grow"></span>
      <button class="tb-btn" data-a="history" title="Earlier conversations">${icon('clock', 15)}</button>
      <button class="tb-btn" data-a="new" title="New conversation">${icon('plus', 15)}</button>
      <button class="tb-btn" data-a="close" title="Close">${icon('x', 15)}</button>
    </div>
    <div class="aip-thread"></div>
    <div class="aip-mode seg" role="tablist">
      <button data-m="page" title="The assistant adds a new page after this one">New page</button>
      <button data-m="change" title="The assistant changes the page in front">This page</button>
      <button data-m="look" title="The assistant restyles the template after a picture">Look</button>
    </div>
    <div class="aip-pic" hidden></div>
    <div class="aip-compose">
      <button class="tb-btn aip-attach" title="Attach a picture or a PDF (or paste one with ⌘V, or drop it here)">${icon('image', 16)}</button>
      <input type="file" accept="image/*,application/pdf,.pdf" hidden>
      <textarea rows="3"></textarea>
      <button class="btn primary aip-send" title="Send (↵ · ⇧↵ for a new line)">${icon('send', 15)}</button>
    </div>
    <div class="aip-note">${icon('lock', 11)} <span></span></div>`;
  const $ = (s) => pane.querySelector(s);
  const thread = $('.aip-thread'), ta = $('textarea');
  const chip = providerChip($('.aip-model'));

  // ---- which page, and what to do with it
  const pageIndex = () => Math.min(S.page || 0, (S.st?.doc?.pages.length || 1) - 1);
  const pageNow = () => S.st?.doc?.pages[pageIndex()];
  const modeNow = () => mode || (pageNow()?.widgets.length ? 'page' : 'change');
  const paintMode = () => {
    const m = modeNow();
    pane.querySelectorAll('[data-m]').forEach((b) => b.classList.toggle('on', b.dataset.m === m));
    const p = pageNow();
    ta.placeholder = m === 'page'
      ? 'Describe the page — e.g. “total assets and return as KPI cards, allocation by asset class as a donut, the top 10 holdings in a table”.'
      : m === 'look'
        ? (pics.length ? 'Anything to keep or change — e.g. “only the colours”, “keep our typefaces”. Or just send.' : 'Paste a picture to restyle after (⌘V), or drop or attach a picture or a PDF.')
        : `What to change on “${p?.title || 'this page'}” — e.g. “put the KPI cards in one row”, “show the allocation as bars”.`;
    $('.aip-attach').hidden = m !== 'look';
    $('.aip-note span').textContent = m === 'look'
      ? 'Sends the picture and the theme’s look (colours, typefaces, shapes) — never data. The new theme is one undo step.'
      : 'Sends the template’s structure — the page grid, source and field names, widget types and design views — never data. Each change is one undo step.';
    paintPic();
  };
  const paintPic = () => {
    const box = $('.aip-pic');
    box.hidden = modeNow() !== 'look' || !pics.length;
    if (box.hidden) return;
    box.innerHTML = `<div class="aip-pics">${pics.map((p, k) => `<span class="aip-pk"><img src="${p.thumb}" alt=""><button class="tb-btn sm" data-k="${k}" title="Remove the picture">${icon('x', 11)}</button></span>`).join('')}</div>
      <span>${pics.length === 1 ? 'Reference picture' : `${pics.length} reference pictures`}</span>`;
    box.querySelectorAll('[data-k]').forEach((b) => b.addEventListener('click', () => { pics.splice(+b.dataset.k, 1); if (!pics.length) facts = []; paintMode(); }));
  };
  // pictures: a still one as it is; a GIF (a style guide, a recorded deck) as its distinct pages, read by the app;
  // a PDF (a brand book, a report) as its pages, drawn here — up to the room left, spread over the file
  const takePictures = async (files) => {
    const list = [...files].filter((f) => f && (/^image\//.test(f.type) || isPdf(f)));
    if (!list.length) return false;
    for (const f of list) {
      if (pics.length >= MAX_PICS) { toast(`Up to ${MAX_PICS} pictures.`, 'info'); break; }
      try {
        if (isPdf(f)) {
          const room = MAX_PICS - pics.length;
          toast('Reading the PDF’s pages…', 'info', 1500);
          const { pages, total, facts: ff } = await pdfPictures(f, { max: room });
          if (ff) facts.push(`${f.name}:\n${ff}`);
          pics.push(...pages.map((x) => ({ media: x.media, data: x.data, thumb: x.thumb })));
          if (total > pages.length) toast(`${pages.length} of the PDF’s ${total} pages, spread over the file (${pages.map((x) => x.page).join(', ')}).`, 'info', 4000);
        } else if (f.type === 'image/gif') {
          toast('Reading the GIF’s pages…', 'info', 1500);
          const frames = await call('ai_pictures', { data: await base64Of(f) }, { quiet: true });
          pics.push(...frames.slice(0, MAX_PICS - pics.length).map((x) => ({ media: x.media, data: x.data, thumb: x.thumb })));
        } else pics.push(await readPicture(f));
      } catch (e) { toast(String(e.message || e), 'error'); }
    }
    mode = 'look';
    paintMode();
    if (!chat || !chat.msgs.length) draw();
    ta.focus();
    return true;
  };
  pane.querySelectorAll('[data-m]').forEach((b) => b.addEventListener('click', () => { mode = b.dataset.m; paintMode(); ta.focus(); }));

  // ---- the conversation
  const save = () => {
    if (!chat) return;
    all = [chat, ...all.filter((c) => c.id !== chat.id)];
    chats.save(all);
  };
  const newChat = () => { chat = null; applied = null; pics = []; draw(); ta.focus(); };
  const docKey = () => `${S.st?.doc?.meta?.title || ''}`;
  const snap = () => JSON.stringify([S.st?.doc?.pages, S.st?.doc?.theme]);
  const undoable = (i) => applied && applied.i === i && chat && snap() === applied.snap;

  const summary = (m) => {
    // each part its own text, so each translates on its own
    const dot = '<span class="muted"> · </span>';
    if (m.kind === 'page') return `${icon('plus', 12)} <span>New page</span> <b>${esc(m.title || '')}</b>${dot}<span>${m.added} widget${m.added === 1 ? '' : 's'}</span>`;
    const bits = [m.added && `${m.added} added`, m.changed && `${m.changed} changed`, m.removed && `${m.removed} removed`].filter(Boolean);
    return `${icon('layout', 12)} <b>${esc(m.title || '')}</b>${dot}${(bits.length ? bits : ['nothing changed']).map((b) => `<span>${b}</span>`).join(dot)}`;
  };
  // a restyle's answer: the theme's name, its colours, typefaces and shapes
  const lookHtml = (m) => {
    const c = m.look?.colors || {}, t = m.look?.type || {}, sh = m.look?.shape || {};
    const changes = (m.changes || []).length ? `<div class="aip-list"><span>What changed</span>${m.changes.map((x) => `<div>${esc(x)}</div>`).join('')}</div>` : '';
    const sw = (x, title) => (x ? `<i style="background:${esc(x)}" title="${esc(title)} ${esc(x)}"></i>` : '');
    const bars = sh.bar_radius >= 20 ? 'capsule bars' : sh.bar_radius > 0 ? 'rounded bars' : 'square bars';
    return `<div class="aip-src">${icon('brush', 12)} <span>Theme</span> <b>${esc(m.name || '')}</b></div>
      <div class="aip-look"><div class="aip-sw">${sw(c.page, 'page')}${sw(c.card, 'card')}${sw(c.ink, 'ink')}${sw(c.accent, 'accent')}${sw(c.good, 'good')}${sw(c.bad, 'bad')}</div>
        <div class="aip-sw pal">${(c.palette || []).map((x) => sw(x, 'series')).join('')}</div>
        <div class="aip-lk"><span>${esc(t.display || '')}</span>${t.ui && t.ui !== t.display ? `<span class="muted"> · </span><span>${esc(t.ui)}</span>` : ''}</div>
        <div class="aip-lk muted"><span>${bars}</span> · <span>${sh.grid === 'none' ? 'no grid' : `${esc(sh.grid || '')} grid`}</span> · <span>${esc(sh.cards || '')} cards</span>${m.views ? ` · <span>${m.views} design views</span>` : ''}</div></div>${changes}`;
  };
  const msgHtml = (m, i) => {
    if (m.role === 'user') {
      const tag = m.kind === 'page' ? `${icon('plus', 12)} New page` : m.kind === 'look' ? `${icon('brush', 12)} Look` : `${icon('layout', 12)} This page`;
      const thumbs = m.thumbs || (m.thumb ? [m.thumb] : []);
      return `<div class="aip-msg me"><div class="aip-tag">${tag}</div>${thumbs.length ? `<div class="aip-thumbs n${Math.min(thumbs.length, 4)}">${thumbs.slice(0, 4).map((t) => `<img class="aip-thumb" src="${t}" alt="">`).join('')}</div>` : ''}${m.text ? `<div>${esc(m.text)}</div>` : ''}</div>`;
    }
    if (m.failed) return `<div class="aip-msg ai bad">${icon('warn', 14)}<div>${esc(m.failed)}${/Settings → AI/.test(m.failed) ? ' <button class="btn sm" data-set>Open Settings</button>' : ''}</div></div>`;
    const meta = `<div class="aip-meta">${esc(m.provider || '')} · ${esc(m.model || '')}${m.attempts > 1 ? ` · ${m.attempts} tries` : ''}${m.ms ? ` · ${(m.ms / 1000).toFixed(1)} s` : ''}${feedbackHtml(m.logAt)}</div>`;
    const issues = (m.issues || []).length ? `<div class="aip-list kept"><span>Still to fix</span>${m.issues.map((x) => `<div>${esc(x)}</div>`).join('')}</div>` : '';
    return `<div class="aip-msg ai${m.undone ? ' undone' : ''}" data-i="${i}">${m.explanation ? `<p>${esc(m.explanation)}</p>` : ''}
      ${m.kind === 'look' ? lookHtml(m) : `<div class="aip-src">${summary(m)}</div>`}${issues}
      <div class="aip-acts">${m.undone ? `<span class="muted">${icon('undo', 12)} Undone</span>` : `<span class="ok">${icon('ok', 12)} Applied</span>`}<span class="grow"></span>
        ${m.kind === 'look' ? '<button class="btn sm" data-act="studio" title="Open the theme in the theme studio to fine-tune or save it to the library">Theme studio</button>' : '<button class="btn sm" data-act="show">Show page</button>'}
        ${undoable(i) ? `<button class="btn sm" data-act="undo" title="Take this change back (⌘Z)">${icon('undo', 11)} Undo</button>` : ''}</div>${meta}</div>`;
  };
  const draw = () => {
    paintMode();
    if (!chat || !chat.msgs.length) {
      const m = modeNow();
      thread.innerHTML = m === 'look'
        ? `<div class="aip-empty">${icon('brush', 22)}<b>Restyle after a picture</b><span>Paste a screenshot (⌘V), drop a picture or a PDF here or attach one — a report page, a deck, a brand book, a brand site. The assistant takes its colours, typefaces and chart shapes and builds this template’s theme from them.</span>
          <div class="aip-ex">${EXAMPLES.look.map((x) => `<button class="aip-chip" data-ex>${esc(x)}</button>`).join('')}</div></div>`
        : `<div class="aip-empty">${icon('sparkles', 22)}<b>Describe the page you need</b><span>The assistant lays it out from this template’s sources and design views. The app checks it and applies it as one step you can undo.</span>
          <div class="aip-ex">${EXAMPLES[m].map((x) => `<button class="aip-chip" data-ex>${esc(x)}</button>`).join('')}</div></div>`;
      // a look needs its picture first: the example only fills in the words
      thread.querySelectorAll('[data-ex]').forEach((b) => b.addEventListener('click', () => { ta.value = b.textContent; if (m !== 'look' || pics.length) ask(); else ta.focus(); }));
      return;
    }
    thread.innerHTML = chat.msgs.map(msgHtml).join('') + (busy ? `<div class="aip-msg ai wait"><span class="spin"></span>Designing — the app checks each plan against the template…</div>` : '');
    thread.querySelectorAll('[data-set]').forEach((b) => b.addEventListener('click', () => document.dispatchEvent(new CustomEvent('open-settings', { detail: 'ai' }))));
    thread.querySelectorAll('.aip-msg.ai[data-i]').forEach((el) => {
      const i = +el.dataset.i, m = chat.msgs[i];
      el.querySelector('[data-act="show"]')?.addEventListener('click', () => goPage(m.page));
      el.querySelector('[data-act="studio"]')?.addEventListener('click', () => api.themeStudio('current'));
      el.querySelector('[data-act="undo"]')?.addEventListener('click', async () => {
        await call('undo').then(setState);
        m.undone = true;
        applied = null;
        save();
        draw();
      });
    });
    thread.scrollTop = thread.scrollHeight;
  };
  const goPage = (id) => {
    const pi = S.st?.doc?.pages.findIndex((p) => p.id === id) ?? -1;
    if (pi < 0) { toast('That page is no longer in the template.', 'info'); return; }
    S.page = pi;
    document.dispatchEvent(new Event('pagechange'));
    scrollToPage(pi);
  };

  // what the model has already seen: the requests and its plans (structure only)
  // (only of the same kind: a page's plans mean nothing to a restyle, and the other way round)
  const history = (kind) => (chat?.msgs || []).filter((m) => !m.failed && !m.undone && (m.kind === 'look') === (kind === 'look')).map((m) => (m.role === 'user'
    ? { role: 'user', content: m.kind === 'look' ? `RESTYLE AFTER THE PICTURE: ${m.text}` : `${m.kind === 'page' ? 'BUILD A NEW PAGE' : 'CHANGE THE CURRENT PAGE'}: ${m.text}` }
    : { role: 'assistant', content: JSON.stringify(m.kind === 'look' ? { explanation: m.explanation, name: m.name, look: m.look } : { explanation: m.explanation, title: m.title, widgets: m.plan || [] }) }));

  async function ask() {
    if (busy) return;
    const text = ta.value.trim();
    const kind = modeNow();
    if (kind === 'look' && !pics.length) { toast('Paste, drop or attach a picture to restyle after.', 'info'); ta.focus(); return; }
    if (!text && kind !== 'look') { ta.focus(); return; }
    if (S.st?.doc?.kind !== 'template') { toast('The design assistant works on templates.', 'info'); return; }
    const hist = history(kind);
    if (!chat) chat = { id: `c${Date.now()}`, title: (text || 'Restyle after a picture').slice(0, 80), at: Date.now(), doc: docKey(), msgs: [] };
    // thumbnails show the pictures in the conversation (only the first message with them carries them)
    const key = pics.map((p) => p.thumb.slice(-24)).join('|');
    const shown = kind === 'look' && !chat.msgs.some((x) => x.picKey === key);
    chat.msgs.push({ role: 'user', kind, text, ...(shown ? { thumbs: pics.slice(0, 4).map((p) => p.thumb), picKey: key } : {}) });
    ta.value = '';
    busy = true;
    save();
    draw();
    try {
      const provider = chip.current()?.id || null;
      const r = kind === 'look'
        ? await call('ai_restyle', { images: pics.map((p) => ({ media: p.media, data: p.data })), prompt: text, facts: facts.join('\n\n') || null, provider, history: hist, lang: appLanguage() }, { quiet: true })
        : await call('ai_design', { kind, prompt: text, page: currentPageId(), provider, history: hist, lang: appLanguage() }, { quiet: true });
      if (r.st) setState(r.st);
      chat.msgs.push(kind === 'look'
        ? { role: 'ai', kind, explanation: r.explanation || '', name: r.name || '', look: r.look || {}, views: r.views || 0, changes: r.changes || [], issues: r.issues || [],
          provider: r.provider, model: r.model, attempts: r.attempts, ms: r.ms, logAt: r.log_at }
        : { role: 'ai', kind, explanation: r.explanation || '', title: r.title || '', page: r.page, added: r.added || 0, changed: r.changed || 0, removed: r.removed || 0,
          issues: r.issues || [], plan: r.plan || [], provider: r.provider, model: r.model, attempts: r.attempts, ms: r.ms, logAt: r.log_at });
      applied = { i: chat.msgs.length - 1, snap: snap() };
      if (kind === 'page') { mode = 'change'; goPage(r.page); }
    } catch (e) {
      chat.msgs.push({ role: 'ai', failed: String(e) });
    } finally {
      busy = false;
      save();
      draw();
    }
  }

  ta.addEventListener('keydown', (e) => {
    // typing stays here; ⌘ shortcuts (⌘I, ⌘Z, ⌘[ ⌘]…) work as anywhere
    if (!(e.metaKey || e.ctrlKey)) e.stopPropagation();
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); ask(); }
  });
  $('.aip-send').addEventListener('click', ask);
  // a picture: attached, pasted or dropped
  const fileIn = $('input[type=file]');
  $('.aip-attach').addEventListener('click', () => fileIn.click());
  fileIn.multiple = true;
  fileIn.addEventListener('change', () => { takePictures(fileIn.files).finally(() => { fileIn.value = ''; }); });
  ta.addEventListener('paste', (e) => {
    const fs = [...(e.clipboardData?.files || [])].filter((x) => x.type.startsWith('image/') || isPdf(x));
    if (fs.length) { e.preventDefault(); takePictures(fs); }
  });
  pane.addEventListener('dragover', (e) => { if ([...(e.dataTransfer?.items || [])].some((x) => x.type.startsWith('image/') || x.type === 'application/pdf')) { e.preventDefault(); pane.classList.add('drop'); } });
  pane.addEventListener('dragleave', (e) => { if (!pane.contains(e.relatedTarget)) pane.classList.remove('drop'); });
  pane.addEventListener('drop', (e) => {
    pane.classList.remove('drop');
    const fs = [...(e.dataTransfer?.files || [])].filter((x) => x.type.startsWith('image/') || isPdf(x));
    if (fs.length) { e.preventDefault(); e.stopPropagation(); takePictures(fs); }
  });
  $('[data-a="new"]').addEventListener('click', newChat);
  $('[data-a="close"]').addEventListener('click', () => close());
  $('[data-a="history"]').addEventListener('click', (e) => historyMenu(e.currentTarget, chats, chat?.id, (c) => { chat = c; applied = null; draw(); }, () => { all = []; chat = null; draw(); }));

  // another document, another conversation; the page in front sets the mode and the Undo button
  let lastDoc = docKey();
  onChange(() => {
    if (docKey() !== lastDoc) { lastDoc = docKey(); chat = null; applied = null; mode = null; pics = []; if (!pane.hidden) draw(); return; }
    if (!pane.hidden && !busy) draw();
  });
  document.addEventListener('pagechange', () => { if (!pane.hidden && !busy) paintMode(); });

  function open() {
    if (!pane.hidden) return;
    pane.hidden = false;
    onToggle(true);
    if (!chip.loaded()) chip.load();
    draw();
    setTimeout(() => ta.focus());
  }
  function close() {
    pane.hidden = true;
    onToggle(false);
  }
  return {
    open,
    close,
    toggle: () => (pane.hidden ? open() : close()),
    isOpen: () => !pane.hidden,
    /** Only templates are designed with it. */
    available: () => S.st?.doc?.kind === 'template',
  };
}
