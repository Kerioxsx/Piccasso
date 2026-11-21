// Hand-motion paths: the order and direction a beginner should draw the
// current step's lines, in normalized image coordinates. Used for the
// "Show me the motion" animation and mirrored in Rust (motion.rs) for auto-draw.

/** Ellipse/box as a polyline starting at the top, clockwise. Radii are fractions of width. */
export function shapePath(s, aspect, n = 48) {
  if (s.kind === 'box') {
    const x0 = s.cx - s.rx, x1 = s.cx + s.rx, y0 = s.cy - s.ry * aspect, y1 = s.cy + s.ry * aspect;
    return [[s.cx, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0], [s.cx, y0]];
  }
  const t = (s.angle * Math.PI) / 180, pts = [];
  for (let k = 0; k <= n; k++) {
    const a = -Math.PI / 2 + (k / n) * Math.PI * 2;
    const ex = Math.cos(a) * s.rx, ey = Math.sin(a) * s.ry;
    pts.push([s.cx + ex * Math.cos(t) - ey * Math.sin(t), s.cy + (ex * Math.sin(t) + ey * Math.cos(t)) * aspect]);
  }
  return pts;
}

/** Rotate a closed polyline so it starts at its top-most point (how people naturally begin). */
export function startAtTop(points, closed) {
  if (!closed || points.length < 3) return points.slice();
  let k = 0;
  for (let i = 1; i < points.length; i++) if (points[i][1] < points[k][1] || (points[i][1] === points[k][1] && points[i][0] < points[k][0])) k = i;
  const out = points.slice(k).concat(points.slice(0, k));
  out.push(out[0]);
  return out;
}

/** Parallel hatching strokes inside a box, angled away from the light. */
export function hatchPaths(box, lightDir, aspect, count = 9) {
  const [x0, y0, x1, y1] = box;
  // stroke direction: perpendicular-ish to the light so shading reads as form
  const fromLeft = lightDir && lightDir[1] === 'l';
  const out = [];
  const w = x1 - x0, h = y1 - y0;
  for (let i = 0; i < count; i++) {
    const f = (i + 0.5) / count;
    // a diagonal stroke across a band of the box; alternate direction (back-and-forth)
    const ya = y0 + h * Math.max(0, f - 0.18), yb = y0 + h * Math.min(1, f + 0.18);
    const xa = fromLeft ? x0 + w * 0.55 : x0 + w * 0.45, xb = fromLeft ? x1 - w * 0.02 : x0 + w * 0.02;
    const seg = [[xa, yb], [xb, ya]];
    out.push(i % 2 ? seg.reverse() : seg);
  }
  void aspect;
  return out;
}

/**
 * Ordered strokes for a step: [{points:[[x,y]...], kind:'shape'|'outline'|'inner'|'hatch'|'dot'}].
 */
export function motionPaths(analysis, step) {
  if (!analysis || !step) return [];
  const has = (l) => step.layers.includes(l);
  const aspect = analysis.aspect || 1;
  const out = [];
  if (step.kind === 'page') {
    const [x0, y0, x1, y1] = [0, 0, 1, 1];
    for (const p of [[x0, y0], [x1, y0], [x1, y1], [x0, y1]]) out.push({ kind: 'dot', points: [p, p] });
    return out;
  }
  const shading = ['pressure', 'values', 'color', 'details', 'clean'].includes(step.kind);
  if (!shading && has('shapes')) {
    const shapes = analysis.shapes.slice().sort((a, b) => b.rx * b.ry - a.rx * a.ry);
    for (const s of shapes) out.push({ kind: 'shape', points: shapePath(s, aspect) });
  }
  if (!shading && has('silhouette')) for (const c of analysis.silhouettes) out.push({ kind: 'outline', points: startAtTop(c.points, c.closed) });
  if (!shading && has('internal')) for (const c of analysis.internals) out.push({ kind: 'inner', points: startAtTop(c.points, c.closed) });
  if (!out.length) {
    // Shading-type steps: demonstrate the stroke on the darkest band region.
    const t = analysis.technique;
    const dark = t && t.bands.length ? t.bands[0] : null;
    const b = analysis.subject.bbox;
    const cx = dark ? dark.x : (b[0] + b[2]) / 2, cy = dark ? dark.y : (b[1] + b[3]) / 2;
    const r = Math.min(b[2] - b[0], b[3] - b[1]) * 0.22;
    for (const seg of hatchPaths([cx - r, cy - r, cx + r, cy + r], analysis.values.lightDir, aspect)) out.push({ kind: 'hatch', points: seg });
  }
  return out;
}

/** Total length of strokes in pixel space (for timing). */
export function pathLength(points, W, H) {
  let L = 0;
  for (let i = 1; i < points.length; i++) L += Math.hypot((points[i][0] - points[i - 1][0]) * W, (points[i][1] - points[i - 1][1]) * H);
  return L;
}
