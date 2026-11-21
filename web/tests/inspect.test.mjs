import test from 'node:test';
import assert from 'node:assert/strict';
import { valueStats, compareValues, symmetry, measure, sample } from '../js/core/inspect.js';

const img = (W, H, f) => { const d = new Uint8ClampedArray(W * H * 4); for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) { const v = f(x, y), i = (y * W + x) * 4; d[i] = d[i + 1] = d[i + 2] = v; d[i + 3] = 255; } return { width: W, height: H, data: d }; };

test('value statistics and advice', () => {
  const ref = valueStats(img(100, 100, (x) => (x < 50 ? 10 : 245)));
  assert.ok(ref.dark <= 10 && ref.light >= 245 && ref.contrast > 100);
  assert.ok(Math.abs(ref.bands[0] - 0.5) < 0.01 && Math.abs(ref.bands[4] - 0.5) < 0.01);
  const weak = valueStats(img(100, 100, (x) => (x < 50 ? 90 : 220)));
  const tips = compareValues(ref, weak).map((t) => t.text).join(' ');
  assert.match(tips, /darkest darks are lighter/); assert.match(tips, /less contrast/);
  assert.equal(compareValues(ref, ref)[0].level, 'good');
});

test('symmetry, measurements and sampling', () => {
  assert.ok(symmetry(img(80, 40, (x) => Math.abs(x - 39.5) * 3)).score > 0.99, 'mirrored picture');
  assert.ok(symmetry(img(80, 40, (x) => (x < 40 ? 0 : 255))).score < 0.05, 'opposite halves');
  const m = measure([0, 0], [30, 40], 200, 21);
  assert.equal(m.px, 50); assert.equal(m.ofWidth, 0.25); assert.ok(Math.abs(m.cm - 5.25) < 1e-9);
  const s = sample(img(4, 4, () => 128), 1, 1);
  assert.equal(s.hex, '#808080'); assert.equal(s.value, 50);
});
