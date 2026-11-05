// Desktop or browser: the one place the UI tells them apart. Both send the same commands to the
// same core — the desktop app through Tauri, the browser to the server's /api/call.

/**
 * Running in a browser (the web server), not the desktop app. A test page may say which to look
 * like (`window.__QUIRE_AS__`: the guide's screenshots are of the desktop app).
 */
export const WEB = window.__QUIRE_AS__ ? window.__QUIRE_AS__ === 'web' : !window.__TAURI__;

document.documentElement.dataset.platform = WEB ? 'web' : 'desktop';

async function overHttp(cmd, args) {
  const r = await fetch(window.__QUIRE_API__ || '/api/call', {
    method: 'POST',
    // text/plain: a simple request, no preflight when the page comes from another local port
    headers: { 'Content-Type': 'text/plain' },
    body: JSON.stringify({ cmd, args }),
  });
  const j = await r.json().catch(() => `The server answered ${r.status}.`);
  if (!r.ok) throw j;
  return j;
}

// `window.__QUIRE__.invoke` is looked up on every call, so a test page can wrap it
window.__QUIRE__ ||= {
  invoke: WEB ? overHttp : (cmd, args) => window.__TAURI__.core.invoke('call', { cmd, args }),
};

// ------------------------------------------------------------------ files in the browser

/** What each file picker accepts (as the desktop app's dialogs filter). */
const ACCEPT = {
  csv: '.csv,.tsv,.txt',
  excel: '.xlsx,.xlsm,.xls,.ods',
  sqlite: '.db,.sqlite,.sqlite3',
  duckdb: '.duckdb,.ddb,.db,.parquet,.csv',
  image: '.png,.jpg,.jpeg,.svg',
  font: '.ttf,.otf,.ttc',
};

/** The browser's file picker: a File, or null when cancelled. */
function chooseFile(accept) {
  return new Promise((done) => {
    const input = Object.assign(document.createElement('input'), { type: 'file', accept: accept || '' });
    input.addEventListener('change', () => done(input.files[0] || null));
    input.addEventListener('cancel', () => done(null));
    input.click();
  });
}

const base64 = (file) => new Promise((done, fail) => {
  const r = new FileReader();
  r.onload = () => done(String(r.result).split(',')[1] || '');
  r.onerror = () => fail('The file could not be read.');
  r.readAsDataURL(file);
});

/** Picks a file and puts it on the server: its path there, or null. */
async function uploadPicked(accept) {
  const f = await chooseFile(accept);
  if (!f) return null;
  return window.__QUIRE__.invoke('upload_file', { name: f.name, data: await base64(f) });
}

/** Hands the browser a file to save (its downloads folder, or where it asks). */
function download(name, b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const url = URL.createObjectURL(new Blob([bytes]));
  const a = Object.assign(document.createElement('a'), { href: url, download: name });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

/** A command that writes a file on the server (its path back), then the file downloaded here. */
const thenDownload = (cmd) => async (args) => {
  const path = await window.__QUIRE__.invoke(cmd, args);
  if (typeof path === 'string' && path) {
    try {
      const f = await window.__QUIRE__.invoke('saved_file', { path });
      download(f.name, f.data);
    } catch (e) {
      console.warn(`${cmd}: saved on the server, not downloaded: ${e}`);
    }
  }
  return path;
};

/**
 * Commands that need the person's own computer, done in the page in the browser version: a file
 * picked here is uploaded, and the core goes on with its path on the server, as on the desktop.
 */
const IN_PAGE = {
  pick_data_file: ({ kind }) => (kind in ACCEPT ? uploadPicked(ACCEPT[kind]) : null),
  open_dialog: async () => {
    const path = await uploadPicked('.rtpl,.rpt,.qtheme');
    return path ? window.__QUIRE__.invoke('open_path', { path }) : null;
  },
  // what the desktop app saves where the person picks, the browser downloads
  export: thenDownload('export'),
  brief_pdf: thenDownload('brief_pdf'),
  save_text_file: thenDownload('save_text_file'),
  batch_save: thenDownload('batch_save'),
  sql_file_save: thenDownload('sql_file_save'),
  sql_file_open: async ({ ext }) => {
    const f = await chooseFile(ext === 'py' ? '.py,.txt' : '.sql,.txt');
    // the text only: saving it again goes to the server's files
    return f ? { path: null, text: await f.text() } : null;
  },
};

/** Sends a command to the core (or, in the browser, does one that needs this computer here). */
export function invoke(cmd, args = {}) {
  if (WEB && IN_PAGE[cmd] && (cmd !== 'pick_data_file' || args.kind in ACCEPT)) return IN_PAGE[cmd](args);
  return window.__QUIRE__.invoke(cmd, args);
}

/** Opens a link (mailto:, a web page) the way the platform does. */
export function openUrl(url) {
  if (WEB) window.open(url, '_blank');
  else window.__QUIRE__.invoke('reveal', { path: url, open: true });
}
