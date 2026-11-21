// Gesture and timed practice: session planning, a procedural mannequin for
// pose references (random joint angles within natural limits; no photos
// needed), and practice statistics. Pure functions, no DOM.

/** Small seeded random generator (mulberry32) so sessions can be repeated. */
export function rng(seed = Date.now()) {
  let a = seed >>> 0;
  return () => { a = (a + 0x6d2b79f5) >>> 0; let t = a; t = Math.imul(t ^ (t >>> 15), t | 1); t ^= t + Math.imul(t ^ (t >>> 7), t | 61); return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
}

export function shuffle(list, rand = Math.random) {
  const a = list.slice();
  for (let i = a.length - 1; i > 0; i--) { const j = Math.floor(rand() * (i + 1)); [a[i], a[j]] = [a[j], a[i]]; }
  return a;
}

/** Classic warm-up ladders (seconds per pose). */
export const PRESETS = {
  gesture30: { label: '30-second gestures', poses: 10, seconds: [30] },
  gesture60: { label: '1-minute gestures', poses: 8, seconds: [60] },
  gesture120: { label: '2-minute poses', poses: 6, seconds: [120] },
  study300: { label: '5-minute studies', poses: 4, seconds: [300] },
  ladder: { label: 'Warm-up ladder (30 s → 5 min)', poses: 8, seconds: [30, 30, 30, 60, 60, 120, 120, 300] },
};

/**
 * Session plan: which reference to show and for how long.
 * refs: array of reference ids (may be fewer than poses: they repeat, reshuffled).
 */
export function planSession({ refs, poses, seconds, seed = 1 }) {
  const rand = rng(seed);
  const out = [];
  let pool = [];
  for (let i = 0; i < poses; i++) {
    if (!pool.length) pool = shuffle(refs, rand);
    out.push({ ref: refs.length ? pool.pop() : null, seconds: seconds[Math.min(i, seconds.length - 1)] ?? seconds[seconds.length - 1] });
  }
  return out;
}

// ----- mannequin -----
// Joint angles in degrees, 0 = pointing down, positive = towards the figure's left (screen right).
const LIMITS = {
  torso: [-20, 20], neck: [-20, 20],
  upperArmL: [-150, 30], foreArmL: [0, 130], upperArmR: [-30, 150], foreArmR: [-130, 0],
  thighL: [-70, 30], shinL: [-110, 0], thighR: [-30, 70], shinR: [0, 110],
};
const LENGTHS = { torso: 3, neck: 0.4, head: 1, upperArm: 1.4, foreArm: 1.3, thigh: 1.9, shin: 1.8 };

/** Random natural pose (angles in degrees). */
export function randomPose(rand = Math.random) {
  const p = {};
  for (const [k, [lo, hi]] of Object.entries(LIMITS)) p[k] = lo + (hi - lo) * rand();
  // keep at least one leg roughly under the body so the figure stands, kneels or sits believably
  if (Math.abs(p.thighL) > 60 && Math.abs(p.thighR) > 60) p.thighR = -10 + 20 * rand();
  return p;
}

const rad = (d) => (d * Math.PI) / 180;
const step = (from, angle, len) => [from[0] + Math.sin(rad(angle)) * len, from[1] + Math.cos(rad(angle)) * len];

/**
 * Mannequin skeleton for a pose, scaled to height `H` (head units: the figure
 * is about 8 heads tall) with the hips at `hip`. Returns joints and bones.
 */
export function mannequin(pose, hip = [0, 0], H = 400) {
  const u = H / 8;
  const j = { hip };
  j.chest = step(hip, 180 + pose.torso, LENGTHS.torso * u);
  j.neck = step(j.chest, 180 + pose.torso + pose.neck, LENGTHS.neck * u);
  j.head = step(j.neck, 180 + pose.torso + pose.neck, (LENGTHS.head * u) / 2);
  const sh = (side) => step(j.chest, pose.torso + (side < 0 ? -90 : 90), 0.9 * u);
  j.shoulderL = sh(1); j.shoulderR = sh(-1);
  j.elbowL = step(j.shoulderL, pose.upperArmL, LENGTHS.upperArm * u); j.handL = step(j.elbowL, pose.upperArmL + pose.foreArmL, LENGTHS.foreArm * u);
  j.elbowR = step(j.shoulderR, pose.upperArmR, LENGTHS.upperArm * u); j.handR = step(j.elbowR, pose.upperArmR + pose.foreArmR, LENGTHS.foreArm * u);
  const hp = (side) => step(hip, pose.torso + (side < 0 ? -90 : 90), 0.55 * u);
  j.hipL = hp(1); j.hipR = hp(-1);
  j.kneeL = step(j.hipL, pose.thighL, LENGTHS.thigh * u); j.footL = step(j.kneeL, pose.thighL + pose.shinL, LENGTHS.shin * u);
  j.kneeR = step(j.hipR, pose.thighR, LENGTHS.thigh * u); j.footR = step(j.kneeR, pose.thighR + pose.shinR, LENGTHS.shin * u);
  // [from, to, width in head units, side: 0 centre, 1 figure's left, -1 right]
  const bones = [['chest', 'neck', 0.22, 0],
    ['shoulderL', 'elbowL', 0.24, 1], ['elbowL', 'handL', 0.19, 1], ['shoulderR', 'elbowR', 0.24, -1], ['elbowR', 'handR', 0.19, -1],
    ['hipL', 'kneeL', 0.32, 1], ['kneeL', 'footL', 0.24, 1], ['hipR', 'kneeR', 0.32, -1], ['kneeR', 'footR', 0.24, -1]];
  const torso = [j.shoulderL, j.shoulderR, j.hipR, j.hipL];
  return { joints: j, bones: bones.map(([a, b, w, side]) => ({ a: j[a], b: j[b], w: w * u, side })), torso, headR: (LENGTHS.head * u) / 2, unit: u };
}

/** Bounding box of a mannequin (for centring it in a frame). */
export function mannequinBounds(m) {
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (const b of m.bones) for (const p of [b.a, b.b]) { x0 = Math.min(x0, p[0] - b.w); y0 = Math.min(y0, p[1] - b.w); x1 = Math.max(x1, p[0] + b.w); y1 = Math.max(y1, p[1] + b.w); }
  const hd = m.joints.head;
  x0 = Math.min(x0, hd[0] - m.headR); y0 = Math.min(y0, hd[1] - m.headR); x1 = Math.max(x1, hd[0] + m.headR); y1 = Math.max(y1, hd[1] + m.headR);
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

// ----- history and statistics -----
const day = (t) => new Date(t).toISOString().slice(0, 10);

/** sessions: [{at: ms, poses, seconds (total drawing time)}] → totals and streak. */
export function stats(sessions, now = Date.now()) {
  const total = sessions.reduce((s, x) => s + (x.seconds || 0), 0);
  const poses = sessions.reduce((s, x) => s + (x.poses || 0), 0);
  const days = new Set(sessions.map((x) => day(x.at)));
  let streak = 0;
  for (let t = now; days.has(day(t)); t -= 86400000) streak++;
  // yesterday still counts until the end of today
  if (!streak && days.has(day(now - 86400000))) for (let t = now - 86400000; days.has(day(t)); t -= 86400000) streak++;
  const week = sessions.filter((x) => now - x.at < 7 * 86400000).reduce((s, x) => s + (x.seconds || 0), 0);
  return { sessions: sessions.length, poses, minutes: Math.round(total / 60), weekMinutes: Math.round(week / 60), streak, days: days.size };
}
