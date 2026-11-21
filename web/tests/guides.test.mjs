import test from 'node:test';
import assert from 'node:assert/strict';
import { clipSegment, perspectiveGuide, defaultVps, faceGuide, figureGuide, compositionGuide, mmToPx } from '../js/core/guides.js';

test('segments are clipped to the picture', () => {
  assert.deepEqual(clipSegment([-10, 50], [110, 50], 100, 100), [[0, 50], [100, 50]]);
  assert.equal(clipSegment([-10, -10], [-5, -20], 100, 100), null);
  const [a, b] = clipSegment([50, 50], [250, 50], 100, 100);
  assert.deepEqual(a, [50, 50]); assert.deepEqual(b, [100, 50]);
});

test('every perspective ray points at its vanishing point and stays inside', () => {
  const W = 400, H = 300;
  for (const points of [1, 2, 3]) {
    const { vps, horizonY } = defaultVps(points, W, H);
    const shapes = perspectiveGuide({ points, vps, horizonY, count: 12 }, W, H);
    const rays = shapes.filter((s) => s.kind === 'ray');
    assert.ok(rays.length >= points * 6, `${points}-point: ${rays.length} rays`);
    for (const r of rays) {
      for (const p of [r.a, r.b]) assert.ok(p[0] >= -1e-6 && p[0] <= W + 1e-6 && p[1] >= -1e-6 && p[1] <= H + 1e-6);
      // collinear with one of the vanishing points
      const ok = vps.slice(0, points).some((vp) => Math.abs((r.b[0] - r.a[0]) * (vp[1] - r.a[1]) - (r.b[1] - r.a[1]) * (vp[0] - r.a[0])) < 1e-3 * (W + H) ** 2);
      assert.ok(ok);
    }
    assert.equal(shapes.filter((s) => s.type === 'vp').length, points);
    assert.ok(shapes.some((s) => s.kind === 'horizon'));
  }
});

test('face and figure proportions follow the canon', () => {
  const f = faceGuide({ x: 0, y: 0, w: 100, h: 140 });
  const eyes = f.find((s) => s.type === 'label' && s.text.startsWith('Eyes'));
  assert.equal(eyes.at[1] - 4, 70, 'eyes at half the head height');
  const g = figureGuide({ x: 0, y: 0, w: 100, h: 800 }, 8);
  const half = g.find((s) => s.type === 'label' && s.text.includes('Crotch'));
  assert.equal(half.at[1] - 4, 400, 'crotch at half the height of an 8-head figure');
});

test('composition grids', () => {
  const t = compositionGuide('thirds', 300, 90).map((s) => s.a);
  assert.deepEqual(t.map((p) => p.map(Math.round)), [[100, 0], [200, 0], [0, 30], [0, 60]]);
  assert.equal(compositionGuide('golden', 100, 100).length, 4);
  assert.ok(Math.abs(mmToPx(25.4) - 96) < 1e-9);
});
