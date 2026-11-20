import { test } from 'node:test';
import assert from 'node:assert/strict';
import { loadShared } from '../../tools/shared.mjs';
import { paintDemo } from '../../tools/demo-image.mjs';
import { planProject } from '../js/core/pipeline.js';
import { defaultPrep } from '../js/core/project.js';
import { paperLayout, pressureFor, colorName, gridFractions } from '../js/core/steps.js';
import { motionPaths, shapePath, startAtTop } from '../js/core/motion.js';

const shared = loadShared();

test('pressure guidance follows tone rank', () => {
  const bands = [{ band: 0 }, { band: 1 }, { band: 2 }];
  assert.equal(pressureFor(bands, bands[0], shared.pressure).label, 'firm');
  assert.equal(pressureFor(bands, bands[2], shared.pressure).label, 'light');
  const two = [{ band: 0 }, { band: 3 }];
  assert.equal(pressureFor(two, two[0], shared.pressure).grade, '4B');
  assert.equal(pressureFor(two, two[1], shared.pressure).grade, 'HB');
});

test('colour names are perceptually sensible', () => {
  const t = shared.colors.colors;
  assert.equal(colorName([83, 121, 176], t), 'blue');
  assert.equal(colorName([52, 73, 104], t), 'navy');
  assert.equal(colorName([128, 128, 128], t), 'grey');
  assert.equal(colorName([200, 40, 40], t), 'red');
});

test('custom paper size in cm and grid method', () => {
  const l = paperLayout({ preset: 'custom', widthMm: 300, heightMm: 200, orientation: 'landscape', marginMm: 10 }, shared.paper.presets, 1.5, 15);
  assert.equal(l.preset, 'custom');
  assert.equal(l.paperW, 300);
  assert.ok(Math.abs(l.drawW - 270) < 1e-9 && Math.abs(l.drawH - 180) < 1e-9);
  const g = gridFractions(l, 2);
  assert.ok(Math.abs(g.fx - 20 / 270) < 1e-12);
  const prep = { ...defaultPrep(), paper: { ...defaultPrep().paper, gridCm: 2 } };
  const page = planProject(paintDemo(), prep, shared).steps.find((s) => s.kind === 'page');
  assert.match(page.instruction, /grid of 2\.0 cm squares, 12 across and 9 down/);
  assert.ok(page.layers.includes('cmgrid'));
});

test('pressure, shading and colour steps', () => {
  const plain = planProject(paintDemo(), defaultPrep(), shared).steps;
  const pressure = plain.find((s) => s.kind === 'pressure');
  assert.ok(pressure, 'pressure step present');
  assert.match(pressure.instruction, /% pressure, (H|HB|2B|4B|6B) pencil/);
  assert.ok(!plain.some((s) => s.kind === 'color'));
  const color = planProject(paintDemo(), { ...defaultPrep(), medium: 'color' }, shared).steps.find((s) => s.kind === 'color');
  assert.match(color.instruction, /blue/);
  assert.ok(plain.every((s) => typeof s.motion === 'string' && s.motion.length > 10));
  const values = planProject(paintDemo(), { ...defaultPrep(), difficulty: 'detailed' }, shared).steps.find((s) => s.kind === 'values');
  assert.match(values.instruction, /crosshatching/);
});

test('motion paths: order, start at top, hatching for shading steps', () => {
  const { analysis, steps } = planProject(paintDemo(), defaultPrep(), shared);
  const shapes = motionPaths(analysis, steps.find((s) => s.kind === 'shapes'));
  assert.ok(shapes.length >= 2 && shapes.every((s) => s.kind === 'shape'));
  const outline = motionPaths(analysis, steps.find((s) => s.kind === 'contours')).filter((s) => s.kind === 'outline');
  const pts = outline[0].points;
  assert.ok(pts.every((p) => p[1] >= pts[0][1]), 'outline starts at its top-most point');
  assert.deepEqual(pts[0], pts[pts.length - 1]);
  assert.ok(motionPaths(analysis, steps.find((s) => s.kind === 'values')).every((s) => s.kind === 'hatch'));
  const c = shapePath({ kind: 'circle', cx: 0.5, cy: 0.5, rx: 0.1, ry: 0.1, angle: 0 }, 1);
  assert.ok(Math.abs(c[0][1] - 0.4) < 1e-9, 'ellipse starts at the top');
  assert.deepEqual(startAtTop([[0, 1], [0, 0], [1, 1]], true)[0], [0, 0]);
});
