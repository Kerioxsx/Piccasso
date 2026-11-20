import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { ROOT, loadShared } from '../../tools/shared.mjs';
import * as P from '../js/core/project.js';
import { ProjectStore, MemoryBackend } from '../js/core/store.js';
import { ReferenceLock, applyGesture, LOCKED, UNLOCKED, UNLOCKING } from '../js/core/lock.js';
import { evaluateAr } from '../js/core/capabilities.js';
import { homography, applyH, detectPaper, warpGray, cssMatrix3d, orderCorners } from '../js/core/geometry.js';
import { compareDrawing, guideMask } from '../js/core/compare.js';
import { paperLayout } from '../js/core/steps.js';

const src = { mime: 'image/png', width: 4, height: 3, data: 'iVBORw0KGgo=' };

test('project create → export → import round trip', () => {
  const p = P.createProject({ name: 'Cat', source: src });
  p.steps = [{ id: 's1', kind: 'page', instruction: 'x' }, { id: 's2', kind: 'shapes', instruction: 'y' }];
  p._runtimeCache = { big: true };
  const text = P.exportProject(p);
  assert.ok(!text.includes('_runtimeCache'));
  const back = P.importProject(text);
  assert.equal(back.name, 'Cat');
  assert.equal(back.id, p.id);
  assert.equal(P.importProject(text, { newId: true }).id === p.id, false);
});

test('demo sample validates', () => {
  const text = fs.readFileSync(path.join(ROOT, 'samples/demo-project.drawstep.json'), 'utf8');
  const p = P.importProject(text);
  assert.ok(p.steps.length >= 5);
  assert.ok(p.analysis && p.analysis.engine === 'local-v1');
});

test('invalid projects are rejected with readable errors', () => {
  assert.throws(() => P.importProject('not json'), /not valid JSON/);
  assert.throws(() => P.importProject('{"format":"other"}'), /format must be/);
  const newer = { ...P.createProject({ source: src }), version: 99 };
  assert.throws(() => P.importProject(JSON.stringify(newer)), /newer DrawStep/);
  const badStyle = P.createProject({ source: src }); badStyle.prep.style = 'neon';
  assert.throws(() => P.importProject(JSON.stringify(badStyle)), /prep.style/);
});

test('normalize fills defaults and clamps current step', () => {
  const p = P.normalizeProject({ format: P.FORMAT, version: 1, id: 'a', name: 'n', source: src, steps: [{ id: 'a', kind: 'page', instruction: '' }], progress: { currentStep: 9 } });
  assert.equal(p.progress.currentStep, 0);
  assert.equal(p.prep.style, 'pencil_sketch');
  assert.equal(p.ar.opacity, 0.5);
});

test('step navigation: complete, skip, restart', () => {
  let p = P.createProject({ source: src });
  p.steps = ['a', 'b', 'c'].map((id) => ({ id, kind: 'page', instruction: '' }));
  p = P.completeStep(p);
  p = P.completeStep(p, { skipped: true });
  assert.deepEqual(p.progress.completed, ['a']);
  assert.deepEqual(p.progress.skipped, ['b']);
  assert.equal(p.progress.currentStep, 2);
  p = P.gotoStep(p, -5); assert.equal(p.progress.currentStep, 0);
  p.progress.timeSpentSec = 42;
  p = P.restartTutorial(p);
  assert.equal(p.progress.currentStep, 0); assert.deepEqual(p.progress.completed, []); assert.equal(p.progress.timeSpentSec, 42);
});

test('project store: save, list order, rename, duplicate, delete, resume', async () => {
  const store = new ProjectStore(new MemoryBackend());
  const a = await store.save(P.createProject({ name: 'A', source: src }));
  await new Promise((r) => setTimeout(r, 5));
  const b = await store.save({ ...P.createProject({ name: 'B', source: src }), _tmp: 1 });
  assert.deepEqual((await store.list()).map((p) => p.name), ['B', 'A']);
  assert.equal((await store.get(b.id))._tmp, undefined);
  await store.rename(a.id, '  Apple ');
  assert.equal((await store.get(a.id)).name, 'Apple');
  const d = await store.duplicate(a.id);
  assert.notEqual(d.id, a.id); assert.equal(d.name, 'Apple (copy)');
  const saved = await store.get(a.id);
  saved.progress.currentStep = 3;
  await store.save(saved);
  assert.equal((await store.get(a.id)).progress.currentStep, 3);
  await store.delete(a.id);
  assert.equal(await store.get(a.id), null);
  assert.equal((await store.list()).length, 2);
  await assert.rejects(store.rename('missing', 'x'), /not found/);
});

test('reference lock blocks canvas gestures but never the exit', () => {
  const lock = new ReferenceLock({ holdMs: 1000 });
  const view = { zoom: 1, panX: 0, panY: 0 };
  assert.deepEqual(applyGesture(view, { type: 'pan', dx: 5, dy: 0 }, lock).panX, 5);
  const events = [];
  lock.on((s) => events.push(s));
  lock.lock();
  for (const g of ['pan', 'zoom', 'scroll', 'rotate', 'tap', 'doubletap']) assert.equal(lock.allows(g), false, g);
  for (const g of ['exit', 'unlock', 'returnToStep', 'brightness']) assert.equal(lock.allows(g), true, g);
  assert.equal(applyGesture(view, { type: 'pan', dx: 5, dy: 0 }, lock), view);
  assert.equal(applyGesture(view, { type: 'zoom', factor: 2 }, lock), view);
  lock.beginUnlock(0);
  assert.equal(lock.state, UNLOCKING);
  assert.ok(Math.abs(lock.tick(500) - 0.5) < 1e-9);
  assert.equal(lock.allows('pan'), false, 'still locked mid-hold');
  lock.cancelUnlock();
  assert.equal(lock.state, LOCKED);
  lock.beginUnlock(1000); lock.tick(2000);
  assert.equal(lock.state, UNLOCKED);
  assert.deepEqual(events, [LOCKED, UNLOCKING, LOCKED, UNLOCKING, UNLOCKED]);
  const quick = new ReferenceLock({ requireHold: false });
  quick.lock(); quick.beginUnlock(0);
  assert.equal(quick.state, UNLOCKED);
  quick.lock(); quick.forceUnlock(); assert.equal(quick.locked, false);
});

test('zoom gesture is clamped and anchored', () => {
  const v = applyGesture({ zoom: 1, panX: 0, panY: 0 }, { type: 'zoom', factor: 100, cx: 10, cy: 10 }, null);
  assert.equal(v.zoom, 8);
  assert.equal(v.panX, 10 - 10 * 8);
});

test('AR capability evaluation', () => {
  const base = { secure: true, hasXR: true, immersiveAr: true, hasCamera: true, cameraPermission: 'granted' };
  assert.equal(evaluateAr(base).mode, 'webxr');
  assert.equal(evaluateAr({ ...base, immersiveAr: false }).mode, 'camera');
  assert.equal(evaluateAr({ ...base, hasXR: false }).mode, 'camera');
  const denied = evaluateAr({ ...base, cameraPermission: 'denied' });
  assert.equal(denied.mode, 'manual');
  assert.ok(denied.reasons.some((r) => /denied/.test(r)));
  assert.equal(evaluateAr({ ...base, secure: false }).mode, 'manual');
  assert.equal(evaluateAr({ ...base, hasCamera: false, hasXR: false }).status, 'AR unavailable');
});

test('homography maps the four corners', () => {
  const s = [[0, 0], [100, 0], [100, 50], [0, 50]], d = [[10, 5], [90, 12], [95, 60], [3, 55]];
  const h = homography(s, d);
  for (let i = 0; i < 4; i++) {
    const [x, y] = applyH(h, ...s[i]);
    assert.ok(Math.abs(x - d[i][0]) < 1e-6 && Math.abs(y - d[i][1]) < 1e-6);
  }
  assert.equal(homography(s, [[0, 0], [0, 0], [0, 0], [0, 0]]), null);
  assert.match(cssMatrix3d(100, 50, d), /^matrix3d\(/);
  assert.deepEqual(orderCorners([[9, 9], [0, 0], [9, 0], [0, 9]]), [[0, 0], [9, 0], [9, 9], [0, 9]]);
});

test('paper detection and warp', () => {
  const W = 80, H = 60, g = { width: W, height: H, data: new Uint8Array(W * H).fill(30) };
  for (let y = 10; y < 50; y++) for (let x = 15; x < 70; x++) g.data[y * W + x] = 240;
  const p = detectPaper(g);
  assert.ok(p && p.confidence > 0.5);
  assert.ok(Math.abs(p.corners[0][0] - 15.5 / W) < 0.04 && Math.abs(p.corners[2][1] - 49.5 / H) < 0.04);
  const flat = warpGray(g, p.corners, 20, 10);
  assert.ok(flat.data.every((v) => v > 200));
  assert.equal(detectPaper({ width: 10, height: 10, data: new Uint8Array(100) }), null);
});

test('drawing comparison gives gentle structural feedback', () => {
  const shared = loadShared();
  const analysis = { silhouettes: [{ closed: true, points: [[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]] }], internals: [] };
  const layout = paperLayout({ preset: 'square', marginMm: 20 }, shared.paper.presets, 1, 15);
  const W = 200, H = 200;
  const draw = (scaleX) => {
    const g = { width: W, height: H, data: new Uint8Array(W * H).fill(235) };
    const gm = guideMask({ ...analysis, silhouettes: [{ closed: true, points: analysis.silhouettes[0].points.map(([x, y]) => [0.5 + (x - 0.5) * scaleX, y]) }] }, layout, W, H);
    for (let i = 0; i < gm.length; i++) if (gm[i]) g.data[i] = 60;
    return g;
  };
  const guide = guideMask(analysis, layout, W, H);
  assert.deepEqual(compareDrawing(draw(1), guide, shared.strings).observations, [shared.strings.compare.good]);
  const wide = compareDrawing(draw(1.3), guide, shared.strings);
  assert.ok(wide.observations.some((o) => /wider/.test(o)), wide.observations.join(' | '));
  const blank = { width: W, height: H, data: new Uint8Array(W * H).fill(235) };
  assert.deepEqual(compareDrawing(blank, guide, shared.strings).observations, [shared.strings.compare.empty]);
});
