import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { loadShared, ROOT } from '../../tools/shared.mjs';
import { paintDemo } from '../../tools/demo-image.mjs';
import { computeVectors } from '../../tools/vectors.mjs';
import * as I from '../js/core/image.js';
import { analyze, posCode, r4 } from '../js/core/analysis.js';
import { generateSteps, fill, fmt1, paperLayout } from '../js/core/steps.js';
import { renderStyle } from '../js/core/render.js';
import { planProject } from '../js/core/pipeline.js';
import { defaultPrep } from '../js/core/project.js';

const shared = loadShared();

function solid(w, h, rgb) {
  const img = I.makeRGBA(w, h);
  for (let i = 0; i < w * h; i++) img.data.set([...rgb, 255], i * 4);
  return img;
}
const px = (img, x, y) => Array.from(img.data.slice((y * img.width + x) * 4, (y * img.width + x) * 4 + 4));

test('crop uses normalized rect and clamps', () => {
  const img = I.makeRGBA(10, 10);
  img.data.set([9, 9, 9, 255], (3 * 10 + 2) * 4);
  const c = I.crop(img, { x: 0.2, y: 0.3, w: 0.5, h: 0.5 });
  assert.equal(c.width, 5); assert.equal(c.height, 5);
  assert.deepEqual(px(c, 0, 0), [9, 9, 9, 255]);
  const c2 = I.crop(img, { x: 0.9, y: 0.9, w: 0.9, h: 0.9 });
  assert.equal(c2.width, 1); assert.equal(c2.height, 1);
});

test('rotate 90/180/270 and flip move pixels correctly', () => {
  const img = I.makeRGBA(3, 2);
  img.data.set([1, 0, 0, 255], 0); // top-left marker
  const r90 = I.rotate(img, 90);
  assert.equal(r90.width, 2); assert.equal(r90.height, 3);
  assert.deepEqual(px(r90, 1, 0), [1, 0, 0, 255]); // TL → TR
  assert.deepEqual(px(I.rotate(img, 180), 2, 1), [1, 0, 0, 255]);
  assert.deepEqual(px(I.rotate(img, 270), 0, 2), [1, 0, 0, 255]);
  assert.deepEqual(px(I.flip(img, true, false), 2, 0), [1, 0, 0, 255]);
  assert.deepEqual(px(I.flip(img, false, true), 0, 1), [1, 0, 0, 255]);
  assert.equal(I.rotate(img, 360), img);
});

test('fitDims never upscales and keeps aspect', () => {
  assert.deepEqual(I.fitDims(100, 50, 200), [100, 50]);
  assert.deepEqual(I.fitDims(1000, 500, 200), [200, 100]);
});

test('resizeBox averages', () => {
  const img = I.makeRGBA(2, 1);
  img.data.set([0, 0, 0, 255, 200, 100, 50, 255]);
  assert.deepEqual(px(I.resizeBox(img, 1, 1), 0, 0), [100, 50, 25, 255]);
});

test('adjust composites transparency on white and applies brightness/contrast', () => {
  const img = I.makeRGBA(1, 1); // fully transparent black
  assert.deepEqual(px(I.adjust(img), 0, 0), [255, 255, 255, 255]);
  const gray = solid(1, 1, [128, 128, 128]);
  assert.equal(px(I.adjust(gray, 20, 0), 0, 0)[0], 179);
  assert.equal(px(I.adjust(solid(1, 1, [200, 200, 200]), 0, 100), 0, 0)[0], 255);
});

test('otsu splits a bimodal image', () => {
  const g = I.makeGray(10, 1);
  g.data.set([10, 12, 11, 10, 13, 200, 201, 199, 202, 200]);
  const t = I.otsu(g);
  assert.ok(t >= 13 && t < 199, `t=${t}`);
});

test('components, trace and Douglas–Peucker on a square', () => {
  const W = 10, H = 10, m = new Uint8Array(W * H);
  for (let y = 2; y <= 6; y++) for (let x = 3; x <= 7; x++) m[y * W + x] = 1;
  m[9 * W + 9] = 1;
  const { labels, comps } = I.labelComponents(m, W, H);
  assert.equal(comps.length, 2);
  assert.equal(comps[0].area, 25);
  assert.equal(comps[1].borders, 2);
  const pts = I.traceBoundary(labels, W, H, 1, comps[0].startX, comps[0].startY);
  assert.equal(pts.length, 16);
  const simple = I.douglasPeucker(pts, 0.5, true);
  assert.deepEqual(simple, [[3, 2], [7, 2], [7, 6], [3, 6]]);
});

test('foregroundFromBorder finds an object on a two-tone background', () => {
  const img = I.makeRGBA(40, 40);
  for (let y = 0; y < 40; y++) for (let x = 0; x < 40; x++) {
    const bg = y < 25 ? [230, 225, 215] : [180, 150, 120];
    const inObj = (x - 20) ** 2 + (y - 18) ** 2 < 64;
    img.data.set([...(inObj ? [40, 120, 40] : bg), 255], (y * 40 + x) * 4);
  }
  const fg = I.foregroundFromBorder(img, 1);
  assert.equal(fg.mask[18 * 40 + 20], 1);
  assert.equal(fg.mask[35 * 40 + 5], 0, 'table is background');
  assert.equal(fg.mask[5 * 40 + 5], 0, 'wall is background');
});

test('demo analysis finds pear and mug, light from top-left', () => {
  const a = analyze(I.prepareImage(paintDemo(), defaultPrep(), 320), shared.difficulty.levels.easy);
  assert.equal(a.lowContrast, false);
  assert.equal(a.objects.length, 2);
  assert.equal(a.values.lightDir, 'tl');
  assert.ok(a.silhouettes.length === 2 && a.silhouettes[0].points.length >= 6);
  assert.ok(a.shapes.length >= 2);
  for (const s of a.shapes) assert.ok(s.cx > 0 && s.cx < 1 && s.cy > 0 && s.cy < 1);
});

test('isolating an object narrows the subject', () => {
  const src = paintDemo();
  const full = planProject(src, defaultPrep(), shared).analysis;
  const iso = planProject(src, { ...defaultPrep(), isolateObject: full.objects[1].id }, shared).analysis;
  assert.deepEqual(iso.activeObjects, [full.objects[1].id]);
  assert.ok(iso.subject.bbox[2] - iso.subject.bbox[0] < full.subject.bbox[2] - full.subject.bbox[0]);
});

test('low-contrast image falls back to whole-image subject without throwing', () => {
  const plan = planProject(solid(64, 48, [128, 128, 128]), defaultPrep(), shared);
  assert.equal(plan.analysis.lowContrast, true);
  assert.equal(plan.analysis.subject.whole, true);
  assert.ok(plan.steps[0].instruction.includes('low contrast'));
  assert.ok(plan.steps.length >= 3);
});

test('steps are image-specific, filled, and adapt to difficulty', () => {
  const src = paintDemo();
  const counts = {};
  for (const d of ['very_easy', 'easy', 'intermediate', 'detailed']) {
    const { steps } = planProject(src, { ...defaultPrep(), difficulty: d }, shared);
    counts[d] = steps.length;
    for (const s of steps) {
      assert.ok(!/\{\w+\}/.test(s.instruction), `unfilled placeholder in ${s.kind}: ${s.instruction}`);
      assert.ok(s.title && s.tip && Array.isArray(s.layers));
    }
    assert.ok(new Set(steps.map((s) => s.id)).size === steps.length);
  }
  assert.ok(counts.very_easy < counts.easy && counts.easy <= counts.detailed);
  const contour = planProject(src, { ...defaultPrep(), style: 'contour_only' }, shared).steps;
  assert.ok(!contour.some((s) => s.kind === 'values'));
});

test('paper layout fits the image inside the margins', () => {
  const l = paperLayout({ preset: 'a4', orientation: 'auto', marginMm: 15 }, shared.paper.presets, 2, 15);
  assert.equal(l.orient, 'landscape');
  assert.ok(Math.abs(l.drawW - 267) < 1e-9 && Math.abs(l.drawH - 133.5) < 1e-9);
});

test('helpers', () => {
  assert.equal(fill('a {x} {y}', { x: 1 }), 'a 1 {y}');
  assert.equal(fmt1(12.25), '12.3');
  assert.equal(fmt1(-0.04), '-0.0');
  assert.equal(posCode(0.1, 0.9), 'bl');
  assert.equal(r4(-0.00005), -0.0001);
});

test('every style renders at the input size', () => {
  const img = I.prepareImage(paintDemo(), defaultPrep(), 160);
  for (const s of shared.styles.styles) {
    const out = renderStyle(img, s.id, { level: shared.difficulty.levels.easy, lineThickness: 2, sketchContrast: 20, simplifyBackground: true });
    assert.equal(out.width, img.width); assert.equal(out.height, img.height);
  }
  assert.throws(() => renderStyle(img, 'nope', { level: shared.difficulty.levels.easy }));
});

test('shared/test-vectors.json is up to date with the JS engine', () => {
  const file = JSON.parse(fs.readFileSync(path.join(ROOT, 'shared/test-vectors.json'), 'utf8'));
  assert.deepEqual(JSON.parse(JSON.stringify(computeVectors(shared))), file, 'run: node tools/make-vectors.mjs');
});

test('web/shared mirrors shared/', () => {
  for (const f of fs.readdirSync(path.join(ROOT, 'shared'))) {
    if (!f.endsWith('.json') || f === 'test-vectors.json') continue;
    assert.equal(fs.readFileSync(path.join(ROOT, 'web/shared', f), 'utf8'), fs.readFileSync(path.join(ROOT, 'shared', f), 'utf8'), `${f} out of sync: run node tools/sync-shared.mjs`);
  }
});
