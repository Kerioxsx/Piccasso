import test from 'node:test';
import assert from 'node:assert/strict';
import { CONVERSIONS, preAdjust, inRegion, upscale, threshold, halftone, silhouette, edges } from '../js/core/filters.js';

// a test picture: light paper, a dark disc in the middle, a mid-gray square
function picture(W = 64, H = 48) {
  const data = new Uint8ClampedArray(W * H * 4);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = (y * W + x) * 4;
    let v = 240;
    if ((x - 32) ** 2 + (y - 24) ** 2 < 100) v = 20;
    else if (x > 44 && x < 58 && y > 8 && y < 20) v = 128;
    data[i] = data[i + 1] = data[i + 2] = v; data[i + 3] = 255;
  }
  return { width: W, height: H, data };
}
const at = (img, x, y) => img.data[(y * img.width + x) * 4];

test('every conversion runs and keeps the size', () => {
  const img = picture();
  for (const [key, c] of Object.entries(CONVERSIONS)) {
    const p = Object.fromEntries(Object.entries(c.params || {}).map(([k, v]) => [k, v[2]]));
    const out = c.run(img, p);
    assert.equal(out.width, img.width, key); assert.equal(out.height, img.height, key);
    assert.equal(out.data.length, img.data.length, key);
  }
});

test('threshold, silhouette and edges behave', () => {
  const img = picture();
  const t = threshold(img, 128);
  assert.equal(at(t, 32, 24), 0); assert.equal(at(t, 2, 2), 255);
  const s = silhouette(img);
  assert.equal(at(s, 32, 24), 0, 'subject is black'); assert.equal(at(s, 2, 2), 255, 'background is white');
  const e = edges(img, 0.5, 1);
  assert.equal(at(e, 32, 24), 255, 'flat inside has no edge');
  let edgeOnRim = false; for (let x = 20; x < 32; x++) if (at(e, x, 24) === 0) edgeOnRim = true;
  assert.ok(edgeOnRim, 'the disc rim is an edge');
});

test('halftone dots are bigger where it is darker', () => {
  const h = halftone(picture(), 8);
  let darkCell = 0, lightCell = 0;
  for (let y = 16; y < 24; y++) for (let x = 24; x < 32; x++) if (at(h, x, y) === 0) darkCell++;
  for (let y = 32; y < 40; y++) for (let x = 0; x < 8; x++) if (at(h, x, y) === 0) lightCell++;
  assert.ok(darkCell > lightCell * 3, `${darkCell} vs ${lightCell}`);
});

test('mirror, region processing and upscale', () => {
  const img = picture();
  const m = preAdjust(img, { mirror: true });
  assert.equal(at(m, 63 - 50, 12), at(img, 50, 12));
  const r = inRegion(img, { x: 0, y: 0, w: 32, h: 48 }, (i) => threshold(i, 128));
  assert.equal(at(r, 2, 2), 255); assert.equal(at(r, 50, 12), 128, 'outside the region is untouched');
  const u = upscale(img, 2);
  assert.equal(u.width, 128); assert.ok(Math.abs(at(u, 64, 48) - 20) < 10);
});
