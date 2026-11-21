// End-to-end smoke test of the web app in Chromium (Playwright).
// Usage: node web/tests/e2e/smoke.mjs [screenshotDir]
// Starts a static server on a free port, walks the main flows and fails on console errors.
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const WEB = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const shots = process.argv[2];
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

const browser = await chromium.launch();
const ctx = await browser.newContext({ viewport: { width: 1280, height: 860 }, deviceScaleFactor: 1 });
const page = await ctx.newPage();
const errors = [];
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
page.on('pageerror', (e) => errors.push(String(e)));
const shot = async (name) => { if (shots) { fs.mkdirSync(shots, { recursive: true }); await page.screenshot({ path: path.join(shots, name + '.png') }); } };
const step = (s) => console.log('•', s);

try {
  step('onboarding');
  await page.goto(base);
  await page.getByRole('heading', { name: 'Welcome to Piccasso' }).waitFor();
  await shot('01-onboarding');
  await page.getByRole('button', { name: 'Try the demo first' }).click();

  step('workspace with demo');
  await page.locator('.step-progress strong', { hasText: /Step 1 of/ }).waitFor({ timeout: 15000 });
  await page.waitForFunction(() => !document.querySelector('.split-label:not([hidden])'), null, { timeout: 20000 });
  await shot('02-workspace-step1');
  await page.getByRole('button', { name: /Done, next/ }).click();
  await page.locator('.step-progress strong', { hasText: /Step 2 of/ }).waitFor();
  await shot('03-workspace-step2');
  await page.getByRole('button', { name: 'Compare with original (split view)' }).click();
  await shot('04-compare-split');
  await page.getByRole('button', { name: 'Compare with original (split view)' }).click();
  const chip = page.locator('.chips .chip').first();
  if (await chip.count()) { await chip.click(); await page.locator('dialog[open]').waitFor(); await shot('05-glossary'); await page.locator('dialog[open] button').click(); }

  step('locked reference');
  await page.getByRole('button', { name: /Locked view/ }).click();
  await page.getByRole('button', { name: 'Lock screen' }).click();
  await page.locator('.locked-screen.is-locked').waitFor();
  // pan attempt while locked must not move anything
  const before = await page.locator('.locked-canvas canvas').screenshot();
  const box = await page.locator('.locked-canvas').boundingBox();
  await page.mouse.move(box.x + 200, box.y + 200); await page.mouse.down(); await page.mouse.move(box.x + 400, box.y + 300, { steps: 5 }); await page.mouse.up();
  await page.mouse.wheel(0, -500);
  const after = await page.locator('.locked-canvas canvas').screenshot();
  if (Buffer.compare(before, after) !== 0) throw new Error('Locked canvas changed after pan/zoom gestures');
  await shot('06-locked');
  const ub = page.getByRole('button', { name: /Unlock/ });
  const ubb = await ub.boundingBox();
  await page.mouse.move(ubb.x + 10, ubb.y + 10); await page.mouse.down(); await page.waitForTimeout(1500); await page.mouse.up();
  await page.locator('.locked-screen:not(.is-locked)').waitFor();
  await page.getByRole('button', { name: 'Exit' }).click();
  await page.locator('.step-progress strong').waitFor();

  step('camera tracing (no camera in CI → fallback)');
  await page.getByRole('button', { name: /Camera trace/ }).click();
  await page.locator('.ar-status').waitFor();
  await page.waitForTimeout(800);
  await shot('07-ar');
  await page.getByRole('button', { name: 'Exit camera tracing' }).click();

  step('prepare screen');
  await page.getByRole('button', { name: /Edit picture/ }).click();
  await page.getByRole('heading', { name: 'Prepare your picture' }).waitFor();
  await page.getByRole('button', { name: 'Clean outline' }).click();
  await page.waitForTimeout(1200);
  await shot('08-prepare');
  await page.getByRole('button', { name: 'Very Easy' }).click();
  await page.getByRole('button', { name: 'Create my drawing steps' }).click();
  await page.locator('.step-progress strong', { hasText: /Step 1 of/ }).waitFor({ timeout: 20000 });
  await shot('09-workspace-after-prepare');

  step('library + settings');
  await page.goto(base + '#/library');
  await page.getByRole('heading', { name: 'Your projects' }).waitFor();
  await shot('10-library');
  await page.goto(base + '#/settings');
  await page.getByRole('heading', { name: 'Settings' }).waitFor();
  await page.getByRole('button', { name: 'Dark' }).click();
  await shot('11-settings-dark');
  await page.goto(base + '#/home');
  await page.getByRole('heading', { name: 'What would you like to draw?' }).waitFor();
  await shot('12-home-dark');

  step('mobile layout');
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(base + '#/workspace');
  await page.locator('.step-progress strong').waitFor({ timeout: 15000 });
  await page.waitForTimeout(800);
  await shot('13-mobile-workspace');
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
  if (overflow) throw new Error('Horizontal overflow on mobile');

  const real = errors.filter((e) => !/favicon|Permissions policy|camera|getUserMedia|NotFoundError|Requested device not found/i.test(e));
  if (real.length) throw new Error('Console errors:\n' + real.join('\n'));
  console.log('E2E smoke: OK');
} catch (e) {
  console.error('E2E smoke FAILED:', e.message);
  if (errors.length) console.error('Console:', errors.join('\n'));
  await shot('zz-failure');
  process.exitCode = 1;
} finally {
  await browser.close();
  server.close();
}
