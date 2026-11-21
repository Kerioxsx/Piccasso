// Piccasso service worker: offline app shell + runtime cache.
// Projects live in IndexedDB, so they are available offline too.
const VERSION = 'piccasso-v3';
const SHELL = [
  './', 'index.html', 'manifest.webmanifest', 'css/app.css',
  'js/app.js', 'js/state.js', 'js/worker.js',
  'js/core/image.js', 'js/core/analysis.js', 'js/core/steps.js', 'js/core/render.js', 'js/core/pipeline.js',
  'js/core/geometry.js', 'js/core/compare.js', 'js/core/lock.js', 'js/core/capabilities.js', 'js/core/project.js', 'js/core/store.js',
  'js/ui/dom.js', 'js/ui/imageio.js', 'js/ui/canvas.js', 'js/ui/gestures.js', 'js/ui/speech.js', 'js/ui/wakelock.js',
  'js/ui/processing.js', 'js/ui/exporter.js', 'js/ui/timelapse.js', 'js/ui/screens/studio.js', 'js/core/filters.js', 'js/ui/screens/guides.js', 'js/core/guides.js', 'js/ui/screens/practice.js', 'js/core/practice.js', 'js/ui/screens/sandbox.js', 'js/core/sandbox3d.js', 'js/ui/screens/skills.js', 'js/core/skills.js', 'js/ui/screens/analyze.js', 'js/core/inspect.js', 'shared/lessons.en.json', 'js/ui/webxr.js',
  'js/ui/screens/home.js', 'js/ui/screens/prepare.js', 'js/ui/screens/process.js', 'js/ui/screens/workspace.js',
  'js/ui/screens/locked.js', 'js/ui/screens/ar.js', 'js/ui/screens/compare.js', 'js/ui/screens/library.js', 'js/ui/screens/settings.js',
  'shared/difficulty.json', 'shared/styles.json', 'shared/paper.json', 'shared/step-kinds.json', 'shared/pressure.json', 'shared/colors.json', 'shared/strings.en.json', 'shared/glossary.en.json',
  'demo/demo-project.piccasso.json', 'demo/hero.svg',
  'icons/icon.svg', 'icons/icon-192.png', 'icons/icon-512.png', 'icons/icon-maskable-512.png',
];

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(VERSION).then((c) => c.addAll(SHELL)).then(() => self.skipWaiting()));
});
self.addEventListener('activate', (e) => {
  e.waitUntil(caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== VERSION).map((k) => caches.delete(k)))).then(() => self.clients.claim()));
});
self.addEventListener('fetch', (e) => {
  const req = e.request;
  if (req.method !== 'GET') return;
  const url = new URL(req.url);
  if (url.origin !== location.origin) return; // never cache third-party (e.g. AI SDK, API calls)
  // Stale-while-revalidate for same-origin files.
  e.respondWith(caches.open(VERSION).then(async (cache) => {
    const cached = await cache.match(req, { ignoreSearch: true });
    const network = fetch(req).then((res) => { if (res.ok) cache.put(req, res.clone()); return res; }).catch(() => cached);
    return cached || network;
  }));
});
