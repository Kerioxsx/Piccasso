import test from 'node:test';
import assert from 'node:assert/strict';
import { planSession, PRESETS, randomPose, mannequin, mannequinBounds, rng, stats } from '../js/core/practice.js';

test('sessions use every reference before repeating, with the preset times', () => {
  const p = planSession({ refs: ['a', 'b', 'c'], poses: 7, seconds: PRESETS.ladder.seconds, seed: 3 });
  assert.equal(p.length, 7);
  assert.deepEqual(new Set(p.slice(0, 3).map((x) => x.ref)), new Set(['a', 'b', 'c']));
  assert.deepEqual(p.map((x) => x.seconds), [30, 30, 30, 60, 60, 120, 120]);
  assert.deepEqual(planSession({ refs: ['a', 'b', 'c'], poses: 7, seconds: [30], seed: 3 }).map((x) => x.ref), planSession({ refs: ['a', 'b', 'c'], poses: 7, seconds: [30], seed: 3 }).map((x) => x.ref), 'same seed, same order');
});

test('mannequin limbs keep their lengths and the figure is about 8 heads tall when standing', () => {
  const stand = { torso: 0, neck: 0, upperArmL: 10, foreArmL: 0, upperArmR: -10, foreArmR: 0, thighL: 0, shinL: 0, thighR: 0, shinR: 0 };
  const m = mannequin(stand, [0, 0], 400);
  const b = mannequinBounds(m);
  assert.ok(b.h > 360 && b.h < 440, `height ${b.h}`);
  const r = rng(7);
  for (let i = 0; i < 50; i++) {
    const mm = mannequin(randomPose(r), [0, 0], 400);
    const len = (p, q) => Math.hypot(p[0] - q[0], p[1] - q[1]);
    assert.ok(Math.abs(len(mm.joints.hipL, mm.joints.kneeL) - 1.9 * 50) < 1e-6);
    assert.ok(Math.abs(len(mm.joints.elbowR, mm.joints.handR) - 1.3 * 50) < 1e-6);
  }
});

test('practice statistics and streaks', () => {
  const now = Date.parse('2026-11-21T12:00:00Z'), D = 86400000;
  const s = stats([{ at: now - 2 * D, poses: 8, seconds: 600 }, { at: now - D, poses: 4, seconds: 1200 }, { at: now, poses: 10, seconds: 300 }, { at: now - 30 * D, poses: 5, seconds: 60 }], now);
  assert.equal(s.streak, 3); assert.equal(s.poses, 27); assert.equal(s.minutes, 36); assert.equal(s.weekMinutes, 35); assert.equal(s.sessions, 4);
  assert.equal(stats([{ at: now - D, poses: 1, seconds: 60 }], now).streak, 1, 'yesterday keeps the streak alive today');
  assert.equal(stats([], now).streak, 0);
});
