// The app's appearance: light, dark, or as the system — this computer's choice, applied before the
// first paint. Reports keep their own theme; this is the interface around them.

const KEY = 'quire.appearance';

/** "light", "dark" or "system". */
export function appearance() {
  try { const v = localStorage.getItem(KEY); return v === 'light' || v === 'dark' ? v : 'system'; } catch { return 'system'; }
}

/** Puts the choice on the page: data-theme on <html> (none follows the system). */
export function applyAppearance() {
  const v = appearance();
  if (v === 'system') delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = v;
}

export function setAppearance(v) {
  try { localStorage.setItem(KEY, v); } catch {}
  applyAppearance();
  // charts and anything drawn with colours read at draw time
  window.dispatchEvent(new Event('appearance'));
}

export const APPEARANCES = [['system', 'As the system', 'monitor'], ['light', 'Light', 'sun'], ['dark', 'Dark', 'moon']];
