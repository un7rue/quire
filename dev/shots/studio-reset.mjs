// In the widget studio: double-click resets (a chart setting, a card setting, a text size) and the arrow on an
// empty number box starting from the value shown. node dev/shots/studio-reset.mjs <out>
import { send, ev, sleep, chrome } from './cdp.mjs';
await send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1000, deviceScaleFactor: 1, mobile: false });
await send('Page.navigate', { url: 'http://localhost:8767/dev/shots/live.html' }); await sleep(3000);
await ev(`(async () => { const m = await import('/ui/state.js'); try { await m.call('auth', { cmd: 'logout' }, { quiet: true }); } catch {} })()`);
await send('Page.reload', {}); await sleep(3000);
await ev(`window.__errs = []; window.addEventListener('error', (e) => window.__errs.push(e.message))`);
await ev(`document.querySelector('.lg-demo button[data-l=design]')?.click()`); await sleep(3000);
await ev(`(async () => { const s = await import('/ui/state.js'); s.setState(await s.call('auth', { cmd: 'widget_open', args: { kind: 'kpi', view: 'card' } })); s.S.sel = 'w_studio'; s.render(); })()`); await sleep(3000);
const tryReset = (label, patch, sel, check, tab) => ev(`(async () => {
  const s = await import('/ui/state.js'); const p = await import('/ui/panels.js');
  await s.edit({ op: 'update_widget', id: 'w_studio', patch: ${JSON.stringify(patch)} });
  s.S.sel = 'w_studio'; s.S.inspTab = { ...(s.S.inspTab || {}), widget: ${JSON.stringify(tab)} }; p.renderRight();
  await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('.insp-sec.closed .insp-title').forEach((b) => b.click());
  document.querySelectorAll('details.role').forEach((d) => { d.open = true; });
  const el = [...document.querySelectorAll('.insp ' + ${JSON.stringify(sel)})].find((l) => l.textContent.trim() === ${JSON.stringify(label)});
  if (!el) return 'name not shown';
  if (!el.classList.contains('dbl-reset')) return 'not marked as set';
  el.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  await new Promise((r) => setTimeout(r, 600));
  return (${check})(s.findWidget('w_studio').w) ? 'reset ✓' : 'still set ✗';
})()`);
console.log('chart setting (Align):', await tryReset('Align', { opts: { viz: { kpiAlign: 'right' } } }, '.f-row > label', '(w) => w.opts?.viz?.kpiAlign == null', 'design'));
console.log('card setting (Fill):', await tryReset('Fill', { style: { background: '#ff0000' } }, '.f-row > label', '(w) => w.style?.background == null', 'design'));
console.log('text size:', await tryReset('Size', { style: { fonts: { kpi: { size: 30 } } } }, 'details[data-role=kpi] .fc-l', '(w) => w.style?.fonts?.kpi?.size == null', 'text'));
// ↑ on the empty text size box: from the value shown
console.log('arrow up:', await ev(`(async () => {
  const s = await import('/ui/state.js'); const p = await import('/ui/panels.js');
  s.S.inspTab = { ...(s.S.inspTab || {}), widget: 'text' }; p.renderRight(); await new Promise((r) => setTimeout(r, 400));
  document.querySelectorAll('details.role').forEach((d) => { d.open = true; });
  const inp = document.querySelector('details[data-role=kpi] .fc-size');
  const shown = inp.placeholder; inp.focus();
  return 'shown ' + shown + ' · value ' + inp.value + ' (before)';
})()`));
await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await sleep(500);
await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
await sleep(500);
console.log('after ↑ twice:', await ev(`(async () => { const s = await import('/ui/state.js'); return document.querySelector('details[data-role=kpi] .fc-size').value + ' · saved ' + s.findWidget('w_studio').w.style?.fonts?.kpi?.size + ' · focused: ' + (document.activeElement?.className || document.activeElement?.tagName); })()`));
console.log('errors:', await ev(`JSON.stringify(window.__errs)`));
chrome.kill(); process.exit(0);
