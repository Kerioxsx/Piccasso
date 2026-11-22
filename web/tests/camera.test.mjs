import test from 'node:test';
import assert from 'node:assert/strict';
import { frameStats, lightingHints, sharpness, skinMask, dilate } from '../js/core/camera.js';
import { addPhoto, removePhoto, coverOf, beforeAfter, gapText, setNote, MAX_HISTORY } from '../js/core/history.js';

const gray = (W, H, f) => { const d = new Uint8Array(W * H); for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) d[y * W + x] = f(x, y); return { width: W, height: H, data: d }; };
const rgb = (W, H, f) => { const d = new Uint8ClampedArray(W * H * 4); for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) { const [r, g, b] = f(x, y), i = (y * W + x) * 4; d[i] = r; d[i + 1] = g; d[i + 2] = b; d[i + 3] = 255; } return { width: W, height: H, data: d }; };
const ids = (st) => lightingHints(st).map((h) => h.id);

test('lighting hints: dark, glare, uneven, blur, fine', () => {
  const sharpPaper = (base) => gray(80, 60, (x, y) => base + (((x >> 2) + (y >> 2)) % 2 ? 20 : -20));
  assert.deepEqual(ids(frameStats(sharpPaper(170))), []);
  assert.ok(ids(frameStats(sharpPaper(30))).includes('dark'));
  assert.ok(ids(frameStats(gray(80, 60, (x, y) => (x > 60 && y < 20 ? 255 : 170 + ((x + y) % 2) * 30)))).includes('glare'));
  assert.ok(ids(frameStats(gray(80, 60, (x, y) => (x < 40 ? 80 : 200) + ((x + y) % 2) * 20))).includes('uneven'));
  assert.ok(ids(frameStats(gray(80, 60, (x) => 100 + Math.round(x * 1.2)))).includes('blur'));
});

test('sharpness is higher for crisp detail', () => {
  const crisp = gray(40, 40, (x) => (x % 2 ? 0 : 255));
  const soft = gray(40, 40, (x) => 128 + Math.round(Math.sin(x / 6) * 40));
  assert.ok(sharpness(crisp) > sharpness(soft) * 10);
});

test('skin mask finds a hand-coloured blob, not paper or a blue pen', () => {
  const img = rgb(60, 40, (x, y) => (x > 10 && x < 30 && y > 8 && y < 32 ? [210, 150, 120] : x > 40 && x < 50 ? [40, 60, 180] : [235, 233, 228]));
  const m = skinMask(img);
  assert.equal(m.data[20 * 60 + 20], 255);
  assert.equal(m.data[20 * 60 + 45], 0);
  assert.equal(m.data[5 * 60 + 5], 0);
  assert.ok(m.share > 0.15 && m.share < 0.25);
  const lone = rgb(20, 20, (x, y) => (x === 10 && y === 10 ? [210, 150, 120] : [235, 233, 228]));
  assert.equal(skinMask(lone).share, 0);
  const g = dilate({ width: 5, height: 5, data: Uint8Array.from({ length: 25 }, (_, i) => (i === 12 ? 255 : 0)) }, 1);
  assert.equal(g.data.filter(Boolean).length, 9);
});

test('drawing history: photos, cover, before/after, limits', () => {
  let p = { id: 'p', thumb: 'data:thumb', history: [] };
  assert.equal(coverOf(p), 'data:thumb');
  assert.equal(beforeAfter(p), null);
  p = addPhoto(p, { image: 'data:a', at: '2026-01-01T10:00:00Z' });
  p = addPhoto(p, { image: 'data:b', at: '2026-01-04T10:00:00Z' });
  const [a, b] = p.history;
  assert.deepEqual(beforeAfter(p), [a.id, b.id]);
  assert.equal(gapText(a.at, b.at), '3 days later');
  p = { ...p, cover: 'photo:' + b.id };
  assert.equal(coverOf(p), 'data:b');
  p = setNote(p, b.id, 'darker shadows');
  assert.equal(p.history[1].note, 'darker shadows');
  p = removePhoto(p, b.id);
  assert.equal(p.cover, null);
  assert.equal(coverOf(p), 'data:thumb');
  for (let i = 0; i < MAX_HISTORY + 5; i++) p = addPhoto(p, { image: 'data:' + i });
  assert.equal(p.history.length, MAX_HISTORY);
  assert.equal(p.history[0].image, 'data:a', 'the first photo is kept as "before"');
});
