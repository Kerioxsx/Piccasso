// Geometry for drawing guides: perspective (1/2/3-point), face and figure
// proportions, composition grids. Pure functions returning shapes in pixel
// coordinates: {type:'line', a:[x,y], b:[x,y], w, kind} | {type:'circle', c, r} |
// {type:'ellipse', c, rx, ry} | {type:'label', at, text}.

/** Clip segment a→b to the rectangle [0,0,W,H] (Liang–Barsky); null if outside. */
export function clipSegment(a, b, W, H) {
  let t0 = 0, t1 = 1;
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const tests = [[-dx, a[0]], [dx, W - a[0]], [-dy, a[1]], [dy, H - a[1]]];
  for (const [p, q] of tests) {
    if (p === 0) { if (q < 0) return null; continue; }
    const r = q / p;
    if (p < 0) { if (r > t1) return null; if (r > t0) t0 = r; } else { if (r < t0) return null; if (r < t1) t1 = r; }
  }
  return [[a[0] + t0 * dx, a[1] + t0 * dy], [a[0] + t1 * dx, a[1] + t1 * dy]];
}

/** Rays from a vanishing point across the picture, evenly spaced in angle. */
export function raysFrom(vp, W, H, count) {
  const out = [];
  const far = (W + H) * 4;
  // aim at evenly spaced points around the picture's border so rays cover it whatever the VP position
  const pts = [];
  const per = Math.max(2, Math.round(count / 4));
  for (let i = 0; i <= per; i++) { const t = i / per; pts.push([t * W, 0], [t * W, H], [0, t * H], [W, t * H]); }
  const angles = [...new Set(pts.map(([x, y]) => Math.round(Math.atan2(y - vp[1], x - vp[0]) * 1000) / 1000))].sort((p, q) => p - q);
  for (const ang of angles) {
    const b = [vp[0] + Math.cos(ang) * far, vp[1] + Math.sin(ang) * far];
    const seg = clipSegment(vp, b, W, H);
    if (seg) out.push({ type: 'line', a: seg[0], b: seg[1], w: 1, kind: 'ray' });
  }
  return out;
}

/**
 * Perspective guide. points: 1, 2 or 3. vps: array of vanishing points (pixels,
 * may lie outside the picture). horizonY: the eye level. count: rays per point.
 */
export function perspectiveGuide({ points = 2, vps, horizonY, count = 12 }, W, H) {
  const shapes = [];
  const hz = clipSegment([-W * 10, horizonY], [W * 11, horizonY], W, H);
  if (hz) shapes.push({ type: 'line', a: hz[0], b: hz[1], w: 2, kind: 'horizon' }, { type: 'label', at: [8, horizonY - 6], text: 'Horizon (eye level)' });
  vps.slice(0, points).forEach((vp, i) => {
    shapes.push(...raysFrom(vp, W, H, count));
    shapes.push({ type: 'vp', c: vp, label: `VP${i + 1}` });
  });
  // one-point perspective also shows the verticals and horizontals objects keep
  if (points === 1) {
    const n = Math.max(2, Math.round(count / 3));
    for (let i = 1; i < n; i++) {
      shapes.push({ type: 'line', a: [(W * i) / n, 0], b: [(W * i) / n, H], w: 1, kind: 'vertical' });
    }
  }
  return shapes;
}

/** Sensible starting vanishing points for 1/2/3-point perspective. */
export function defaultVps(points, W, H) {
  const hy = H * 0.42;
  if (points === 1) return { horizonY: hy, vps: [[W / 2, hy]] };
  if (points === 2) return { horizonY: hy, vps: [[-W * 0.15, hy], [W * 1.15, hy]] };
  return { horizonY: hy, vps: [[-W * 0.15, hy], [W * 1.15, hy], [W / 2, H * 1.9]] };
}

/**
 * Face proportions inside box {x, y, w, h} (front view, classical canon):
 * head oval, centre line, eye line at half height, brow / nose base / chin in
 * thirds from the hairline, eyes one eye-width apart, mouth a third below the nose.
 */
export function faceGuide({ x, y, w, h }) {
  const cx = x + w / 2, s = [];
  s.push({ type: 'ellipse', c: [cx, y + h / 2], rx: w / 2, ry: h / 2 });
  s.push({ type: 'line', a: [cx, y], b: [cx, y + h], w: 1, kind: 'axis' });
  const hair = y + h * 0.1, brow = y + h * 0.4, eye = y + h * 0.5, nose = y + h * 0.7, mouth = y + h * 0.8, chin = y + h;
  const row = (yy, label) => { s.push({ type: 'line', a: [x, yy], b: [x + w, yy], w: 1, kind: 'measure' }, { type: 'label', at: [x + w + 6, yy + 4], text: label }); };
  row(hair, 'Hairline'); row(brow, 'Brows'); row(eye, 'Eyes (half the head)'); row(nose, 'Nose base'); row(mouth, 'Mouth'); row(chin, 'Chin');
  // five eye-widths across the head
  const ew = w / 5;
  for (const k of [1, 3]) s.push({ type: 'ellipse', c: [x + ew * (k + 0.5), eye], rx: ew * 0.45, ry: ew * 0.2 });
  // ears between brow and nose base
  s.push({ type: 'line', a: [x, brow], b: [x, nose], w: 2, kind: 'feature' }, { type: 'line', a: [x + w, brow], b: [x + w, nose], w: 2, kind: 'feature' });
  return s;
}

/** Figure proportions: `heads` head-units tall inside box {x, y, w, h}, with landmarks. */
export function figureGuide({ x, y, w, h }, heads = 8) {
  const s = [], u = h / heads, cx = x + w / 2;
  s.push({ type: 'line', a: [cx, y], b: [cx, y + h], w: 1, kind: 'axis' });
  const marks = heads === 8 ? ['Chin', 'Nipples', 'Navel', 'Crotch (half)', 'Mid-thigh', 'Knees', 'Mid-shin', 'Feet']
    : Array.from({ length: heads }, (_, i) => (i === heads - 1 ? 'Feet' : i === 0 ? 'Chin' : `${i + 1} heads`));
  for (let i = 1; i <= heads; i++) {
    const yy = y + u * i;
    s.push({ type: 'line', a: [x, yy], b: [x + w, yy], w: 1, kind: 'measure' }, { type: 'label', at: [x + w + 6, yy + 4], text: `${i}  ${marks[i - 1] || ''}` });
  }
  s.push({ type: 'ellipse', c: [cx, y + u / 2], rx: u * 0.38, ry: u / 2 });
  // shoulders about two head-widths wide, hips about 1.5
  s.push({ type: 'line', a: [cx - u, y + u * 1.4], b: [cx + u, y + u * 1.4], w: 2, kind: 'feature' });
  s.push({ type: 'line', a: [cx - u * 0.8, y + u * 4], b: [cx + u * 0.8, y + u * 4], w: 2, kind: 'feature' });
  return s;
}

/** Composition grids over the whole picture: thirds, golden ratio, symmetry axes, diagonals. */
export function compositionGuide(kind, W, H) {
  const s = [], v = (xx) => s.push({ type: 'line', a: [xx, 0], b: [xx, H], w: 1, kind: 'grid' }), hz = (yy) => s.push({ type: 'line', a: [0, yy], b: [W, yy], w: 1, kind: 'grid' });
  if (kind === 'thirds') { v(W / 3); v((2 * W) / 3); hz(H / 3); hz((2 * H) / 3); }
  else if (kind === 'golden') { const g = 1 / 1.618034; v(W * (1 - g)); v(W * g); hz(H * (1 - g)); hz(H * g); }
  else if (kind === 'symmetry') { s.push({ type: 'line', a: [W / 2, 0], b: [W / 2, H], w: 2, kind: 'axis' }, { type: 'line', a: [0, H / 2], b: [W, H / 2], w: 1, kind: 'axis' }); }
  else if (kind === 'diagonals') { s.push({ type: 'line', a: [0, 0], b: [W, H], w: 1, kind: 'grid' }, { type: 'line', a: [W, 0], b: [0, H], w: 1, kind: 'grid' }); v(W / 2); hz(H / 2); }
  return s;
}

/** Line-weight chart rows: width in px at 96 dpi for a mark, grade and use. */
export const LINE_WEIGHTS = [
  { mm: 0.2, grade: '2H, light hand', use: 'Construction lines and first sketch (erase later)' },
  { mm: 0.35, grade: 'HB', use: 'Outlines in light and far areas' },
  { mm: 0.5, grade: 'B / 2B', use: 'Main outlines' },
  { mm: 0.8, grade: '4B', use: 'Edges in shadow, overlaps, near objects' },
  { mm: 1.2, grade: '6B, side of the lead', use: 'Darkest accents, where objects touch the ground' },
];
export const mmToPx = (mm, dpi = 96) => (mm / 25.4) * dpi;
