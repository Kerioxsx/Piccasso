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

async function record(name, size, fn) {
  const ctx = await browser.newContext({ viewport: size, recordVideo: { dir: TMP, size }, colorScheme: 'light' });
  const page = await ctx.newPage();
  await page.addInitScript(() => localStorage.setItem('piccasso.settings.v1', JSON.stringify({ onboarded: true, theme: 'light' })));
  await page.goto(base + '#/home');
  await fn(page);
  const v = page.video();
  await ctx.close();
  const raw = await v.path();
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-i', raw, '-vf', 'fps=30,format=yuv420p', '-c:v', 'libx264', '-preset', 'slow', '-crf', '30', '-movflags', '+faststart', '-an', path.join(OUT, name + '.mp4')]);
  execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-ss', '1.5', '-i', path.join(OUT, name + '.mp4'), '-frames:v', '1', path.join(OUT, name + '.jpg')]);
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

await browser.close();
server.close();
fs.rmSync(TMP, { recursive: true, force: true });
