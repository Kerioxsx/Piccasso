import test from 'node:test';
import assert from 'node:assert/strict';
import { mesh, worldVerts, cameraBasis, project, vanishingPoint, objectAxes, lightDir, shadowPoint, buildScene, defaultScene, v3 } from '../js/core/sandbox3d.js';

test('meshes are closed and rest on the ground', () => {
  for (const k of ['cube', 'pyramid', 'cylinder', 'cone', 'sphere']) {
    const m = mesh(k);
    const ys = m.V.map((p) => p[1]);
    assert.ok(Math.abs(Math.min(...ys)) < 1e-9 && Math.abs(Math.max(...ys) - 1) < 1e-9, k);
    for (const f of m.F) for (const i of f) assert.ok(i >= 0 && i < m.V.length, k);
  }
  assert.equal(mesh('cube').F.length, 6);
});

test('parallel edges of a box meet at their vanishing point on screen', () => {
  const sc = defaultScene(), W = 800, H = 600;
  const cube = sc.objects[0], m = mesh('cube'), wv = worldVerts(cube, m), basis = cameraBasis(sc.cam);
  const sv = wv.map((p) => project(p, sc.cam, basis, W, H));
  const ax = objectAxes(cube);
  // edges along local x: vertex pairs (xi=0 → xi=1) with same y,z → indices i and i+4
  const vp = vanishingPoint(ax[0], sc.cam, basis, W, H);
  for (const i of [0, 1, 2, 3]) {
    const a = sv[i], b = sv[i + 4];
    const cross = (b[0] - a[0]) * (vp[1] - a[1]) - (b[1] - a[1]) * (vp[0] - a[0]);
    assert.ok(Math.abs(cross) / (Math.hypot(b[0] - a[0], b[1] - a[1]) * Math.hypot(vp[0] - a[0], vp[1] - a[1])) < 1e-6, 'edge points at the VP');
  }
  assert.equal(vanishingPoint(ax[0], { ...sc.cam, ortho: true }, basis, W, H), null, 'orthographic: no vanishing points');
});

test('shadows fall away from the light, and only lit faces face the camera', () => {
  const L = lightDir(0, 45); // light from +z, 45° up
  const s = shadowPoint([0, 1, 0], L);
  assert.ok(Math.abs(s[1]) < 1e-9 && s[2] < 0, 'shadow of a raised point lands on the ground, away from the light');
  assert.equal(shadowPoint([0, 1, 0], lightDir(0, -5)), null);
  const sc = defaultScene();
  const { faces, shadows } = buildScene(sc.objects, sc.cam, lightDir(sc.light.az, sc.light.el), 800, 600);
  assert.ok(faces.length > 20 && shadows.length > 20);
  for (let i = 1; i < faces.length; i++) assert.ok(faces[i - 1].depth >= faces[i].depth, 'back to front');
  for (const f of faces) assert.ok(f.shade >= 0 && f.shade <= 1);
  assert.ok(v3.len(cameraBasis(sc.cam).fwd) > 0.999);
});
