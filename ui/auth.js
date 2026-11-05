// Sign-in and the signed-in person: the login screen, the account menu
// (areas, password, sign out) and what the person's roles allow.

import { appearance, setAppearance, APPEARANCES } from './appearance.js';
import { S, call, setState, render } from './state.js';
import { icon } from './icons.js';
import { esc, h } from './util.js';
import { menu, modal, toast } from './ui.js';

/** The four areas of the app, in the order they appear. */
export const AREAS = [
  { key: 'user', label: 'Reports', title: 'Reports', role: 'User', icon: 'page', desc: 'Reports — for the clients in your access' },
  { key: 'design', label: 'Design', title: 'Design', role: 'Design', icon: 'brush', desc: 'Design — report templates and themes' },
  { key: 'data', label: 'Data', title: 'Data', role: 'Data', roles: ['data', 'design'], icon: 'database', desc: 'Data — connections, data sources and SQL' },
  { key: 'admin', label: 'Admin', title: 'Admin', role: 'Admin', icon: 'shield', desc: 'Admin — users, roles and access' },
];

export const session = () => S.st?.session || null;
export const hasRole = (r) => !!session()?.roles?.[r];
export const myAreas = () => AREAS.filter((a) => (a.roles || [a.key]).some(hasRole));

const AREA_KEY = (login) => `quire.area.${login}`;
/** The area the person is in (the last one they used, if they still have it). */
export function currentArea() {
  const s = session();
  if (!s) return null;
  const mine = myAreas().map((a) => a.key);
  if (S.area && mine.includes(S.area)) return S.area;
  let last = null;
  try { last = localStorage.getItem(AREA_KEY(s.login)); } catch { /* private window */ }
  S.area = mine.includes(last) ? last : s.roles.admin ? 'admin' : mine[0];
  return S.area;
}
/** Where a sign-in starts: Admin, else Design, else Data, else Reports. */
export function startArea() {
  const r = session()?.roles || {};
  return r.admin ? 'admin' : r.design ? 'design' : r.data ? 'data' : 'user';
}

export function setArea(key) {
  S.area = key;
  try { localStorage.setItem(AREA_KEY(session().login), key); } catch { /* private window */ }
}

export const initials = (name) => String(name || '?').split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('');

// ---------------------------------------------------------------- login screen

/** Shows the sign-in screen (when nobody is signed in) or hides it. */
export async function renderLogin() {
  let el = document.getElementById('login');
  if (session()) { el?.remove(); return; }
  if (el) return;
  el = h(`<div id="login">
    <div class="lg-card">
      <div class="lg-brand">${icon('logo', 40)}<div><h1>Quire</h1><p>Professional reports from well-designed templates</p></div></div>
      <form class="lg-form" autocomplete="on">
        <label>Login<input name="login" autocomplete="username" autocapitalize="off" spellcheck="false" required></label>
        <label>Password<span class="lg-pw"><input name="password" type="password" autocomplete="current-password" required><button type="button" class="tb-btn sm" data-a="peek" title="Show the password">${icon('eye', 15)}</button></span></label>
        <div class="lg-err" hidden></div>
        <button class="btn primary lg-go" type="submit">Sign in</button>
      </form>
      <div class="lg-demo" hidden></div>
    </div>
    <div class="lg-foot">Accounts, roles and client access are set by an administrator in the Admin area.<span class="lg-ver" hidden></span></div>
  </div>`);
  document.body.append(el);
  const form = el.querySelector('form');
  const err = el.querySelector('.lg-err');
  const $ = (n) => form.querySelector(`[name="${n}"]`);
  el.querySelector('[data-a="peek"]').addEventListener('click', () => { const p = $('password'); p.type = p.type === 'password' ? 'text' : 'password'; });
  const go = async () => {
    err.hidden = true;
    const btn = form.querySelector('.lg-go');
    btn.disabled = true;
    btn.textContent = 'Signing in…';
    try {
      const st = await call('auth', { cmd: 'login', args: { login: $('login').value, password: $('password').value } }, { quiet: true });
      el.remove();
      S.st = { ...(S.st || {}), session: st.session };
      setArea(startArea());
      setState(st);
      if (st.must_change) await passwordDialog({ forced: true });
      toast(`Welcome, ${st.session.name}`, 'ok', 1800);
    } catch (e) {
      err.textContent = String(e);
      err.hidden = false;
      $('password').select();
      el.querySelector('.lg-card').classList.remove('shake');
      void el.offsetWidth;
      el.querySelector('.lg-card').classList.add('shake');
    } finally {
      btn.disabled = false;
      btn.textContent = 'Sign in';
    }
  };
  form.addEventListener('submit', (e) => { e.preventDefault(); go(); });
  setTimeout(() => $('login').focus(), 50);
  // the demo accounts, while they still have the demo password
  const st = await call('auth', { cmd: 'status' }, { quiet: true }).catch(() => null);
  if (st?.version) { const v = el.querySelector('.lg-ver'); v.textContent = `Quire ${st.version}`; v.hidden = false; }
  if (st?.demo?.length) {
    const box = el.querySelector('.lg-demo');
    const roleText = (r) => AREAS.filter((a) => r[a.key]).map((a) => a.role).join(' · ');
    box.innerHTML = `<div class="lg-demo-t">${icon('sparkles', 13)} Demo accounts <span>password <code>${esc(st.demo_password)}</code></span></div>
      <div class="lg-demo-l">${st.demo.map((d) => `<button type="button" data-l="${esc(d.login)}"><b>${esc(d.login)}</b><span>${esc(d.name)}</span><i>${esc(roleText(d.roles))}</i></button>`).join('')}</div>`;
    box.hidden = false;
    box.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => { $('login').value = b.dataset.l; $('password').value = st.demo_password; go(); }));
  }
}

// ---------------------------------------------------------------- the account menu

/** The person's chip: areas, password, sign out. `onArea(key)` opens an area. */
export function accountChip(onArea) {
  const s = session();
  if (!s) return null;
  const b = h(`<button class="acct-chip" title="${esc(s.name)} — account">${esc(initials(s.name))}</button>`);
  b.addEventListener('click', () => {
    const roles = AREAS.filter((a) => s.roles[a.key]).map((a) => a.role).join(', ');
    menu(b, [
      { title: `${s.name} · ${s.login}` },
      { title: `${roles} · ${s.restricted ? `${s.clients} client${s.clients === 1 ? '' : 's'}` : 'every client'}` },
      { sep: true },
      // the interface: light, dark or as the system
      { title: 'Appearance' },
      ...APPEARANCES.map(([v, label, ic]) => ({ label, icon: appearance() === v ? 'ok' : ic, run: () => setAppearance(v) })),
      { sep: true },
      { label: 'Change password…', icon: 'key', run: () => passwordDialog() },
      { label: 'Sign out', icon: 'logout', run: signOut },
    ], { align: 'end' });
  });
  return b;
}

export async function signOut() {
  if (S.st?.dirty) {
    const ok = await modal({ title: 'Unsaved changes', iconName: 'warn', iconKind: 'warn', body: '<p>You have unsaved changes. Sign out anyway?</p>', buttons: [{ label: 'Cancel', value: false }, { label: 'Sign out', kind: 'danger', value: true }] });
    if (!ok) return;
  }
  const st = await call('auth', { cmd: 'logout' });
  S.area = null; S.sel = null; S.preview = false; S.page = 0;
  document.dispatchEvent(new Event('signed-out'));
  setState(st);
  renderLogin();
}

/** Changes the signed-in person's password; `forced` after an admin's reset. */
export async function passwordDialog({ forced = false } = {}) {
  for (;;) {
    const body = h(`<div class="modal-body pw-dlg">
      ${forced ? '<p class="lead">An administrator asked you to choose a new password.</p>' : ''}
      <label>Current password<input name="old" type="password" autocomplete="current-password"></label>
      <label>New password<input name="new" type="password" autocomplete="new-password"></label>
      <label>Repeat the new password<input name="again" type="password" autocomplete="new-password"></label>
      <div class="hint">4 characters or more.</div>
    </div>`);
    const v = await modal({ title: 'Change password', iconName: 'key', body,
      buttons: [...(forced ? [] : [{ label: 'Cancel', value: null }]), { label: 'Change password', kind: 'primary', value: (box) => Object.fromEntries(['old', 'new', 'again'].map((n) => [n, box.querySelector(`[name=${n}]`).value])) }] });
    if (!v) return false;
    if (v.new !== v.again) { toast('The two new passwords differ.', 'error'); continue; }
    try {
      await call('auth', { cmd: 'password', args: { old: v.old, new: v.new } }, { quiet: true });
      toast('Password changed');
      return true;
    } catch (e) {
      toast(String(e), 'error', 3600);
    }
  }
}

/** Asks Rust who is signed in (after a reload). */
export async function refreshSession() {
  const st = await call('auth', { cmd: 'status' }, { quiet: true }).catch(() => null);
  if (S.st && st) S.st = { ...S.st, session: st.session };
  render();
}
