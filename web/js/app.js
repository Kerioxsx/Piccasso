// Piccasso web app entry: loads shared data, registers the service worker and routes screens.
import { app, applySettings } from './state.js';
import { toast, h } from './ui/dom.js';

const SHARED_FILES = { difficulty: 'difficulty.json', styles: 'styles.json', paper: 'paper.json', stepKinds: 'step-kinds.json', pressure: 'pressure.json', colors: 'colors.json' };

async function loadShared(lang) {
  const out = {};
  await Promise.all(Object.entries(SHARED_FILES).map(async ([k, f]) => { out[k] = await (await fetch(`shared/${f}`)).json(); }));
  out.strings = await (await fetch(`shared/strings.${lang}.json`)).json();
  out.glossary = await (await fetch(`shared/glossary.${lang}.json`)).json();
  return out;
}

const routes = {
  onboarding: () => import('./ui/screens/home.js').then((m) => m.renderOnboarding),
  home: () => import('./ui/screens/home.js').then((m) => m.renderHome),
  learn: () => import('./ui/screens/home.js').then((m) => m.renderLearn),
  prepare: () => import('./ui/screens/prepare.js').then((m) => m.renderPrepare),
  process: () => import('./ui/screens/process.js').then((m) => m.renderProcess),
  workspace: () => import('./ui/screens/workspace.js').then((m) => m.renderWorkspace),
  locked: () => import('./ui/screens/locked.js').then((m) => m.renderLocked),
  ar: () => import('./ui/screens/ar.js').then((m) => m.renderAr),
  compare: () => import('./ui/screens/compare.js').then((m) => m.renderCompare),
  library: () => import('./ui/screens/library.js').then((m) => m.renderLibrary),
  settings: () => import('./ui/screens/settings.js').then((m) => m.renderSettings),
};
const NEEDS_PROJECT = new Set(['prepare', 'process', 'workspace', 'locked', 'ar', 'compare']);

let cleanup = null;
let navToken = 0;

export function go(route) { if (location.hash === `#/${route}`) router(); else location.hash = `#/${route}`; }

async function router() {
  const token = ++navToken;
  let [name, query] = (location.hash.replace(/^#\/?/, '') || '').split('?');
  if (!name) name = app.settings.onboarded ? 'home' : 'onboarding';
  if (!routes[name]) name = 'home';
  if (NEEDS_PROJECT.has(name) && !app.project) {
    const last = sessionStorage.getItem('piccasso.current');
    if (last) { try { await app.openProject(last); } catch { /* fall through */ } }
    if (!app.project) name = 'home';
  }
  if (cleanup) { try { cleanup(); } catch (e) { console.error(e); } cleanup = null; }
  const main = document.getElementById('main');
  main.className = ''; main.innerHTML = '';
  document.body.classList.remove('immersive');
  document.getElementById('nav-back').hidden = name === 'home' || name === 'onboarding';
  document.querySelectorAll('.topnav a').forEach((a) => a.toggleAttribute('aria-current', a.getAttribute('href') === `#/${name}`));
  if (app.project) { try { sessionStorage.setItem('piccasso.current', app.project.id); } catch { /* ignore */ } }
  try {
    const render = await routes[name]();
    if (token !== navToken) return;
    cleanup = (await render(main, new URLSearchParams(query || ''))) || null;
    main.focus({ preventScroll: true });
  } catch (e) {
    console.error(e);
    main.innerHTML = '';
    const p = document.createElement('div'); p.className = 'card notice';
    p.textContent = 'Something went wrong: ' + (e.message || e) + '. ';
    const a = document.createElement('a'); a.href = '#/home'; a.textContent = 'Go home';
    p.appendChild(a); main.appendChild(p);
  }
}

async function start() {
  applySettings(app.settings);
  try { app.shared = await loadShared(app.settings.lang); }
  catch {
    document.getElementById('main').innerHTML = '<div class="card notice">Piccasso could not load its data files. If you opened index.html directly from disk, serve the folder over http (see README).</div>';
    return;
  }
  window.addEventListener('hashchange', router);
  document.getElementById('nav-back').addEventListener('click', () => { if (history.length > 1) history.back(); else go('home'); });
  window.addEventListener('piccasso:navigate', (e) => go(e.detail));
  router();
  if ('serviceWorker' in navigator && location.protocol !== 'file:') registerUpdates();
  window.addEventListener('unhandledrejection', (e) => {
    const msg = e.reason && e.reason.message;
    if (msg && msg !== 'cancelled') toast(msg, { error: true });
  });
}
/** Offline support plus updates: the service worker is checked on start, when
 *  the app comes back to the front and every hour; when a new version has been
 *  installed in the background, offer to switch to it. */
async function registerUpdates() {
  let reg;
  try { reg = await navigator.serviceWorker.register('sw.js'); } catch { return; }
  const hadController = !!navigator.serviceWorker.controller;
  let reloading = false;
  navigator.serviceWorker.addEventListener('controllerchange', () => {
    if (!hadController || reloading) return;
    const btn = h('button.btn.small', { type: 'button', on: { click: () => { reloading = true; location.reload(); } } }, 'Update now');
    toast(h('span', 'A new version of Piccasso is ready. ', btn), { ms: 120000 });
  });
  const check = () => reg.update().catch(() => {});
  setInterval(check, 60 * 60 * 1000);
  document.addEventListener('visibilitychange', () => { if (!document.hidden) check(); });
}

start();
