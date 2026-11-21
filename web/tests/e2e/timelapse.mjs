// End-to-end test of the camera timelapse (Chromium with a simulated camera).
// Usage: node web/tests/e2e/timelapse.mjs [outDir]
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const WEB = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const out = process.argv[2];
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const server = http.createServer((req, res) => {
  const u = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  const f = path.join(WEB, u === '/' ? 'index.html' : u);
  if (!f.startsWith(WEB) || !fs.existsSync(f) || fs.statSync(f).isDirectory()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream' });
  fs.createReadStream(f).pipe(res);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const base = `http://127.0.0.1:${server.address().port}/`;
const browser = await chromium.launch({ args: ['--use-fake-ui-for-media-stream', '--use-fake-device-for-media-stream'] });
const ctx = await browser.newContext({ viewport: { width: 1280, height: 860 }, permissions: ['camera'], acceptDownloads: true });
const page = await ctx.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(String(e)));
let failed = false;
try {
  await page.goto(base);
  await page.getByRole('button', { name: 'Try the demo first' }).click();
  await page.locator('.step-progress strong', { hasText: /Step 1 of/ }).waitFor({ timeout: 15000 });
  await page.goto(base + '#/ar');
  await page.waitForFunction(() => { const v = document.querySelector('.ar-screen video'); return v && v.videoWidth > 0; }, null, { timeout: 15000 });
  const box = page.getByLabel('Record timelapse');
  await box.check();
  await page.waitForTimeout(7000);
  await box.uncheck();
  const status = await page.locator('.ar-panel').innerText();
  const m = /(\d+) photos recorded/.exec(status);
  if (!m || +m[1] < 3) throw new Error('too few photos: ' + status.slice(0, 200));
  const dl = page.waitForEvent('download', { timeout: 60000 });
  await page.getByRole('button', { name: 'Save timelapse video' }).click();
  const d = await dl;
  const file = path.join(out || '/tmp', d.suggestedFilename());
  await d.saveAs(file);
  const size = fs.statSync(file).size;
  if (size < 1000) throw new Error('video too small: ' + size);
  console.log(`ok: ${m[1]} photos -> ${d.suggestedFilename()} (${size} bytes)`);
  if (errors.length) throw new Error(errors.join('\n'));
} catch (e) { console.error('FAIL', e.message); failed = true; }
await browser.close(); server.close();
process.exit(failed ? 1 : 0);
