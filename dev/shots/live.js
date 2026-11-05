// The real UI in a page of its own, its commands sent to the web server on :8766
// (`cargo run --no-default-features --bin quire-server -- --dev`). Logs are kept for the checks.
// looks like the desktop app (the guide's pictures), or the browser version with ?web
window.__QUIRE_AS__ = new URLSearchParams(location.search).has('web') ? 'web' : 'desktop';
window.__QUIRE__ = { invoke: async (cmd, args) => {
  if (cmd === 'js_log') { (window.__logs ||= []).push(args); return null; }
  const r = await fetch('http://127.0.0.1:8766/api/call', { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: JSON.stringify({ cmd, args }) });
  const j = await r.json();
  if (!r.ok) throw j;
  return j;
} };
window.addEventListener('error', (e) => (window.__logs ||= []).push({ level: 'error', msg: e.message }));
await import('/ui/app.js');
