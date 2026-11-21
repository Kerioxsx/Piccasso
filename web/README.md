# Piccasso web app

Plain HTML/CSS/JavaScript ES modules. No build step, no runtime dependencies.

## Run
```bash
python3 -m http.server 8080 -d web   # or any static server
```
Open http://localhost:8080. For camera, AR and installation on a phone, serve it over **https** (any static host, or a local tunnel). Opening `index.html` from disk does not work because ES modules and workers need http(s).

## Install / offline
`manifest.webmanifest` + `sw.js` (stale-while-revalidate app shell). After the first visit the app and saved projects work offline. Use the browser's "Install app" / "Add to Home Screen".

## Structure
- `js/core/` engine (pure functions, also run under Node tests)
- `js/ui/` screens and browser services (canvas renderer, gestures, wake lock, WebXR, speech, export)
- `js/worker.js` background processing
- `shared/` copies of `/shared/*.json` (run `node tools/sync-shared.mjs` after editing)
- `demo/` demo project

## Test
```bash
node --test web/tests/*.test.mjs
node web/tests/e2e/smoke.mjs [screenshotDir]   # Chromium via Playwright
```
