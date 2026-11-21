// Records the product videos used on the website (site/media/*.mp4) by driving
// the real web app in Chromium. Usage: node tools/record-videos.mjs
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { ROOT } from './shared.mjs';

const require = createRequire(import.meta.url);
const { chromium } = require('playwright');
const WEB = path.join(ROOT, 'web'), OUT = path.join(ROOT, 'site/media'), TMP = path.join(OUT, '.raw');
fs.mkdirSync(TMP, { recursive: true });
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
const wait = (ms) => new Promise((r) => setTimeout(r, ms));

const ONLY = process.argv.slice(2);
async function record(name, size, fn) {
  if (ONLY.length && !ONLY.includes(name)) return;
  const ctx = await browser.newContext({ viewport: size, recordVideo: { dir: TMP, size }, colorScheme: 'light' });
  const page = await ctx.newPage();
  page.t0 = Date.now(); page.cut = 0;
  await page.addInitScript(() => localStorage.setItem('piccasso.settings.v1', JSON.stringify({ onboarded: true, theme: 'light' })));
  await page.goto(base + '#/home');
  await fn(page);
  const v = page.video();
  await ctx.close();
  const raw = await v.path();
  const ss = page.cut ? Math.max(0, (page.cut - page.t0) / 1000 - 0.3).toFixed(2) : '0';
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-ss', ss, '-i', raw, '-vf', 'fps=30,format=yuv420p', '-c:v', 'libx264', '-preset', 'slow', '-crf', '30', '-movflags', '+faststart', '-an', path.join(OUT, name + '.mp4')]);
  const dur = parseFloat(execFileSync('ffprobe', ['-v', 'error', '-show_entries', 'format=duration', '-of', 'csv=p=0', path.join(OUT, name + '.mp4')]).toString());
  const at = page.cut ? (dur * 0.55).toFixed(2) : '1.5';
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-ss', at, '-i', path.join(OUT, name + '.mp4'), '-frames:v', '1', path.join(OUT, name + '.jpg')]);
  console.log('recorded', name);
}
const stepLabel = (page) => page.locator('.step-progress strong');
const ready = async (page) => { await stepLabel(page).waitFor(); await page.waitForFunction(() => !document.querySelector('.split-label:not([hidden])'), null, { timeout: 30000 }); };

await record('guided', { width: 1280, height: 800 }, async (page) => {
  await page.getByRole('button', { name: /Try the demo/ }).first().click();
  await ready(page); await wait(1200);
  await page.getByRole('button', { name: /Done, next/ }).click(); await wait(900);
  await page.getByRole('button', { name: 'Show me the motion' }).click(); await wait(6500);
  await page.getByRole('button', { name: /Done, next/ }).click(); await wait(700);
  await page.getByRole('button', { name: /Done, next/ }).click(); await wait(700);
  await page.getByRole('button', { name: 'Show me the motion' }).click(); await wait(7000);
  for (let i = 0; i < 3; i++) { await page.getByRole('button', { name: /Done, next/ }).click(); await wait(1100); }
  await page.getByRole('button', { name: 'Show me the motion' }).click(); await wait(4500);
});

await record('prepare', { width: 1280, height: 800 }, async (page) => {
  await page.getByRole('button', { name: /Try the demo/ }).first().click();
  await ready(page);
  await page.getByRole('button', { name: /Edit picture/ }).click();
  await page.getByRole('heading', { name: 'Prepare your picture' }).waitFor(); await wait(1500);
  for (const s of ['Clean outline', 'Shading guide', 'Basic shapes', 'Pencil sketch']) { await page.getByRole('button', { name: s }).click(); await wait(1500); }
  await page.getByRole('button', { name: 'Colours' }).click(); await wait(500);
  await page.getByLabel('Paper size').selectOption('custom'); await wait(400);
  await page.getByLabel('Paper width (cm)').fill('30'); await page.getByLabel('Paper height (cm)').fill('20'); await wait(1200);
  await page.getByRole('button', { name: 'Create my drawing steps' }).click();
  await ready(page); await wait(800);
  const n = await page.locator('.step-dots button').count();
  await page.locator('.step-dots button').nth(n - 3).click(); await wait(2800);
  await page.locator('.step-dots button').nth(n - 1).click(); await wait(3200);
});

await record('locked', { width: 1280, height: 800 }, async (page) => {
  await page.getByRole('button', { name: /Try the demo/ }).first().click();
  await ready(page);
  await page.locator('.step-dots button').nth(3).click(); await wait(800);
  await page.getByRole('button', { name: /Locked view/ }).click(); await wait(1200);
  await page.getByRole('button', { name: 'Lock screen' }).click(); await wait(800);
  const box = await page.locator('.locked-canvas').boundingBox();
  await page.mouse.move(box.x + 300, box.y + 300); await page.mouse.down(); await page.mouse.move(box.x + 600, box.y + 400, { steps: 25 }); await page.mouse.up();
  await page.mouse.wheel(0, -600); await wait(1200);
  const ub = await page.getByRole('button', { name: /Unlock/ }).boundingBox();
  await page.mouse.move(ub.x + 20, ub.y + 15); await page.mouse.down(); await wait(1600); await page.mouse.up(); await wait(1500);
});

await record('mobile', { width: 390, height: 844 }, async (page) => {
  await page.getByRole('button', { name: /Try the demo/ }).first().click();
  await ready(page); await wait(800);
  await page.getByRole('button', { name: /Done, next/ }).click(); await wait(800);
  await page.getByRole('button', { name: 'Show me the motion' }).click(); await wait(6000);
});

// Feature tours (Studio, Guides, Practice, 3D, Lessons, Analyze, Settings).
const demo = async (page) => { await page.getByRole('button', { name: /Try the demo/ }).first().click(); await ready(page); };
const goto = async (page, route) => { await page.evaluate((r) => { location.hash = '#/' + r; }, route); await wait(900); page.cut ||= Date.now(); };
const chip = (page, name) => page.locator('button.chip', { hasText: name }).first();
const SKETCH = path.join(TMP, 'my-drawing.png');

await record('studio', { width: 1280, height: 800 }, async (page) => {
  await demo(page); await goto(page, 'studio');
  await page.getByRole('button', { name: 'Use current project' }).click(); await wait(1400);
  for (const c of ['Pencil sketch', 'Edge detection', 'Contours', 'Three values', 'Colour → value', 'Crosshatching', 'Halftone', 'Anime simplify', 'Silhouette']) {
    await chip(page, c).click(); await wait(1300);
  }
});

await record('guides', { width: 1280, height: 800 }, async (page) => {
  await demo(page); await goto(page, 'guides');
  await page.getByRole('button', { name: 'Blank paper' }).click(); await wait(1000);
  const cv = page.locator('canvas').first();
  const b = await cv.boundingBox();
  await page.mouse.move(b.x + b.width * 0.2, b.y + b.height * 0.42); await page.mouse.down();
  await page.mouse.move(b.x + b.width * 0.35, b.y + b.height * 0.3, { steps: 30 }); await page.mouse.up(); await wait(600);
  await page.getByRole('button', { name: '3-point' }).click(); await wait(1400);
  await page.getByRole('button', { name: '1-point' }).click(); await wait(1200);
  await page.getByRole('button', { name: '2-point' }).click(); await wait(800);
  for (const g of ['Face proportions', 'Figure proportions', 'Composition', 'Line weights']) { await chip(page, g).click(); await wait(1800); }
  await chip(page, 'Composition').click(); await wait(500);
  for (const g of ['Golden ratio', 'Diagonals']) { await page.getByRole('button', { name: g }).click(); await wait(1200); }
});

await record('practice', { width: 1280, height: 800 }, async (page) => {
  await goto(page, 'practice'); await wait(600);
  const s = page.getByLabel('Seconds per pose', { exact: true });
  if (await s.count()) await s.fill('10');
  await page.getByRole('button', { name: 'Start practice' }).click(); await wait(3500);
  for (let i = 0; i < 3; i++) { await page.getByRole('button', { name: 'Next pose' }).click(); await wait(2600); }
});

await record('sandbox', { width: 1280, height: 800 }, async (page) => {
  await goto(page, 'sandbox'); await wait(800);
  const b = await page.locator('canvas').first().boundingBox();
  const cx = b.x + b.width / 2, cy = b.y + b.height / 2;
  await page.mouse.move(cx, cy); await page.mouse.down(); await page.mouse.move(cx + 260, cy + 40, { steps: 60 }); await page.mouse.up(); await wait(600);
  await page.getByRole('button', { name: 'Wireframe' }).click(); await wait(400);
  await page.mouse.move(cx, cy); await page.mouse.down(); await page.mouse.move(cx - 200, cy - 30, { steps: 50 }); await page.mouse.up(); await wait(900);
  await page.getByRole('button', { name: 'Silhouette' }).click(); await wait(1200);
  await page.getByRole('button', { name: 'Solid' }).click(); await wait(400);
  const light = page.getByLabel('Light direction', { exact: true });
  for (const v of [-20, 20, 60, 100]) { await light.evaluate((el, x) => { el.value = x; el.dispatchEvent(new Event('input', { bubbles: true })); }, String(v)); await wait(350); }
  await page.getByLabel(/Vanishing points of the selected/).check(); await wait(400);
  await page.mouse.move(cx, cy); await page.mouse.down(); await page.mouse.move(cx + 120, cy - 20, { steps: 50 }); await page.mouse.up(); await wait(1500);
});

await record('lessons', { width: 1280, height: 800 }, async (page) => {
  await goto(page, 'skills'); await wait(1500);
  await page.mouse.wheel(0, 500); await wait(900); await page.mouse.wheel(0, -500); await wait(600);
  await page.getByRole('button', { name: 'Start lesson' }).click(); await wait(2500);
  await page.getByLabel('My notes').first().fill('Ghosted the lines three times before drawing. Much straighter!'); await wait(900);
  await page.getByRole('button', { name: 'Mark as done' }).click(); await wait(2600);
});

// A stand-in "photo of my drawing": the demo picture as a pencil sketch.
async function makeSketch() {
  const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 } });
  const page = await ctx.newPage();
  await page.addInitScript(() => localStorage.setItem('piccasso.settings.v1', JSON.stringify({ onboarded: true, theme: 'light' })));
  await page.goto(base + '#/home'); await demo(page); await goto(page, 'studio');
  await page.getByRole('button', { name: 'Use current project' }).click(); await wait(1200);
  await chip(page, 'Pencil sketch').click(); await wait(1500);
  await page.locator('canvas').first().screenshot({ path: SKETCH });
  await ctx.close();
}

await record('analyze', { width: 1280, height: 800 }, async (page) => {
  await makeSketch();
  await demo(page); await goto(page, 'analyze'); await wait(800);
  {
    const fc = page.waitForEvent('filechooser');
    await page.getByRole('button', { name: /Photo of my drawing/ }).first().click();
    await (await fc).setFiles(SKETCH); await wait(2200);
  }
  for (const t of ['Symmetry', 'Grid', 'Values']) { await page.getByRole('button', { name: t, exact: true }).click(); await wait(2000); }
  await page.mouse.move(640, 500); await page.mouse.wheel(0, 420); await wait(2500);
});

await record('themes', { width: 1280, height: 800 }, async (page) => {
  await goto(page, 'settings'); await wait(900);
  for (const t of ['Paper', 'Graphite', 'Dark', 'Light']) { await page.getByRole('button', { name: t, exact: true }).first().click(); await wait(1000); }
  const sw = page.locator('[aria-label="Accent colour"] button');
  const n = await sw.count();
  for (let i = 0; i < Math.min(n, 5); i++) { await sw.nth(i).click(); await wait(600); }
  await goto(page, 'home'); await wait(1500);
});

await browser.close();
server.close();
fs.rmSync(TMP, { recursive: true, force: true });
