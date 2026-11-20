// Piccasso local analysis engine ("local-v1").
//
// Turns a prepared image into a drawing plan: subject/objects, construction
// shapes, silhouettes, internal forms, value bands, light direction, detail
// location and landmarks. All coordinates are normalized: points are
// (x / width, y / height); ellipse radii are fractions of the image WIDTH.
// See docs/ALGORITHMS.md. Mirrored in rust/crates/piccasso-core/src/analysis.rs.

import {
  toGray, blur2, foregroundFromBorder, cleanMask, labelComponents, traceBoundary,
  douglasPeucker, posterizeLevels, colorSobel, percentileThreshold,
} from './image.js';

/** Symmetric rounding to 4 decimals (identical in JS and Rust). */
export function r4(v) {
  const s = v < 0 ? -1 : 1;
  return (s * Math.floor(Math.abs(v) * 10000 + 0.5)) / 10000;
}

/** 3×3 position code for a point inside a box: row (t/m/b) + column (l/c/r). */
export function posCode(x, y, box = [0, 0, 1, 1]) {
  const w = box[2] - box[0] || 1, h = box[3] - box[1] || 1;
  const fx = (x - box[0]) / w, fy = (y - box[1]) / h;
  const col = fx < 1 / 3 ? 'l' : fx < 2 / 3 ? 'c' : 'r';
  const row = fy < 1 / 3 ? 't' : fy < 2 / 3 ? 'm' : 'b';
  return row + col;
}

function fitShape(c, W, H, id, source) {
  const A = c.area;
  const mx = c.sumX / A, my = c.sumY / A;
  const cxx = c.sumXX / A - mx * mx, cyy = c.sumYY / A - my * my, cxy = c.sumXY / A - mx * my;
  const tr = cxx + cyy, det = cxx * cyy - cxy * cxy;
  const disc = Math.sqrt(Math.max(0, (tr * tr) / 4 - det));
  const l1 = tr / 2 + disc, l2 = Math.max(0, tr / 2 - disc);
  let rx = 2 * Math.sqrt(l1), ry = 2 * Math.sqrt(l2);
  let angle = (0.5 * Math.atan2(2 * cxy, cxx - cyy) * 180) / Math.PI;
  const bw = c.maxX - c.minX + 1, bh = c.maxY - c.minY + 1;
  const fill = A / (bw * bh);
  let kind = 'ellipse', cx = mx + 0.5, cy = my + 0.5;
  if (fill > 0.85) {
    kind = 'box'; rx = bw / 2; ry = bh / 2; angle = 0;
    cx = c.minX + bw / 2; cy = c.minY + bh / 2;
  } else if (ry / Math.max(rx, 1e-9) > 0.85) {
    kind = 'circle'; rx = ry = (rx + ry) / 2; angle = 0;
  }
  return {
    id, kind, source,
    cx: r4(cx / W), cy: r4(cy / H), rx: r4(Math.max(rx, 0.5) / W), ry: r4(Math.max(ry, 0.5) / W),
    angle: r4(angle), areaFrac: r4(A / (W * H)),
  };
}

const normPts = (pts, W, H) => pts.map(([x, y]) => [r4((x + 0.5) / W), r4((y + 0.5) / H)]);
const normBox = (c, W, H) => [r4(c.minX / W), r4(c.minY / H), r4((c.maxX + 1) / W), r4((c.maxY + 1) / H)];
const byArea = (a, b) => b.area - a.area || a.label - b.label;

/**
 * @param img     prepared RGBA image (ideally ≤ difficulty.analysisMaxDim)
 * @param level   a difficulty level object from shared/difficulty.json
 * @param opts    { excluded: string[], isolate: string|null }
 */
export function analyze(img, level, opts = {}) {
  const W = img.width, H = img.height, N = W * H;
  const excluded = new Set(opts.excluded || []);
  const isolate = opts.isolate || null;
  const g = toGray(img);
  const b = blur2(g, level.blurRadius);
  // Subject = colours far from the border colours (see foregroundFromBorder).
  const fg = foregroundFromBorder(img, level.blurRadius);
  const t = fg.threshold;
  const separation = fg.separation;
  let mask = fg.mask;
  mask = cleanMask(mask, W, H);
  const { labels, comps } = labelComponents(mask, W, H);
  const minArea = Math.max(4, Math.floor(level.minAreaFrac * N));
  const candidates = comps
    .filter((c) => c.area >= minArea && !(c.borders >= 3 && c.area > 0.5 * N))
    .sort(byArea);
  const lowContrast = separation < 18 || candidates.length === 0;

  const objects = lowContrast ? [] : candidates.slice(0, level.maxObjects).map((c, i) => ({
    id: `obj-${i + 1}`, label: c.label, bbox: normBox(c, W, H),
    centroid: [r4((c.sumX / c.area + 0.5) / W), r4((c.sumY / c.area + 0.5) / H)],
    areaFrac: r4(c.area / N), _c: c,
  }));
  let active = objects.filter((o) => (isolate ? o.id === isolate : !excluded.has(o.id)));
  const useWhole = active.length === 0;

  // Subject mask + bbox.
  const subj = new Uint8Array(N);
  let sx0 = W, sy0 = H, sx1 = -1, sy1 = -1;
  if (useWhole) {
    subj.fill(1); sx0 = 0; sy0 = 0; sx1 = W - 1; sy1 = H - 1;
  } else {
    const act = new Set(active.map((o) => o.label));
    for (let i = 0; i < N; i++) if (act.has(labels[i])) subj[i] = 1;
    for (const o of active) {
      sx0 = Math.min(sx0, o._c.minX); sy0 = Math.min(sy0, o._c.minY);
      sx1 = Math.max(sx1, o._c.maxX); sy1 = Math.max(sy1, o._c.maxY);
    }
  }
  const subjBoxPx = [sx0, sy0, sx1 + 1, sy1 + 1];
  const subjBox = [r4(sx0 / W), r4(sy0 / H), r4((sx1 + 1) / W), r4((sy1 + 1) / H)];
  const subjW = sx1 - sx0 + 1, subjH = sy1 - sy0 + 1;
  const eps = level.dpEpsilon * Math.max(W, H);

  const silhouettes = useWhole ? [] : active.slice(0, 3).map((o) => {
    const raw = traceBoundary(labels, W, H, o.label, o._c.startX, o._c.startY);
    return { objectId: o.id, closed: true, points: normPts(douglasPeucker(raw, eps, true), W, H) };
  });

  // Value bands inside the subject.
  const L = level.valueLevels;
  const vb = blur2(g, level.blurRadius * 2 + 1);
  const lv = posterizeLevels(vb, L);
  const counts = new Array(L).fill(0);
  let subjCount = 0, bxs = 0, bys = 0, bws = 0, dxs = 0, dys = 0, dws = 0;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x;
    if (!subj[i]) continue;
    subjCount++; counts[lv[i]]++;
    const v = vb.data[i];
    bxs += x * v; bys += y * v; bws += v;
    dxs += x * (255 - v); dys += y * (255 - v); dws += 255 - v;
  }
  let dl = 0;
  while (dl < L - 1 && counts[dl] < 0.01 * subjCount) dl++;
  let hl = L - 1;
  while (hl > 0 && counts[hl] < 0.01 * subjCount) hl--;

  // Light direction: vector from the darkness-weighted to the brightness-weighted centroid.
  let lightDir = 'even';
  if (bws > 0 && dws > 0) {
    const vx = (bxs / bws - dxs / dws) / Math.max(subjW, subjH);
    const vy = (bys / bws - dys / dws) / Math.max(subjW, subjH);
    const m = Math.hypot(vx, vy);
    if (m >= 0.04) {
      const col = vx > 0.38 * m ? 'r' : vx < -0.38 * m ? 'l' : 'c';
      const row = vy > 0.38 * m ? 'b' : vy < -0.38 * m ? 't' : 'm';
      lightDir = row + col === 'mc' ? 'even' : row + col;
    }
  }

  // Internal forms: connected dark-band and light-band regions inside the subject.
  const internalsRaw = [];
  let darkSX = 0, darkSY = 0, darkN = 0;
  for (const [tone, band] of [['dark', dl], ['light', hl]]) {
    if (tone === 'light' && hl === dl) continue;
    const m2 = new Uint8Array(N);
    for (let i = 0; i < N; i++) if (subj[i] && lv[i] === band) {
      m2[i] = 1;
      if (tone === 'dark') { darkSX += i % W; darkSY += Math.floor(i / W); darkN++; }
    }
    const lc = labelComponents(m2, W, H);
    for (const c of lc.comps) {
      if (c.area < Math.max(4, minArea >> 1) || c.area > 0.9 * subjCount) continue;
      internalsRaw.push({ tone, c, labels: lc.labels });
    }
  }
  internalsRaw.sort((a, b2) => b2.c.area - a.c.area || (a.tone === b2.tone ? a.c.label - b2.c.label : a.tone === 'dark' ? -1 : 1));
  const internalsSel = internalsRaw.slice(0, level.maxInternal);
  const internals = internalsSel.map((r, i) => ({
    id: `f-${i + 1}`, tone: r.tone, bbox: normBox(r.c, W, H), areaFrac: r4(r.c.area / N),
    centroid: [r4((r.c.sumX / r.c.area + 0.5) / W), r4((r.c.sumY / r.c.area + 0.5) / H)],
    closed: true,
    points: normPts(douglasPeucker(traceBoundary(r.labels, W, H, r.c.label, r.c.startX, r.c.startY), eps, true), W, H),
  }));

  // Construction shapes: objects first, then the biggest internal forms.
  const shapes = [];
  for (const o of active) {
    if (shapes.length >= level.maxShapes) break;
    shapes.push(fitShape(o._c, W, H, `s-${shapes.length + 1}`, 'object'));
  }
  for (const r of internalsSel) {
    if (shapes.length >= level.maxShapes) break;
    const s = fitShape(r.c, W, H, `s-${shapes.length + 1}`, 'form');
    if (s.rx >= 0.02) shapes.push(s);
  }
  if (shapes.length === 0) {
    shapes.push({ id: 's-1', kind: 'box', source: 'frame', cx: r4((subjBox[0] + subjBox[2]) / 2), cy: r4((subjBox[1] + subjBox[3]) / 2),
      rx: r4((subjBox[2] - subjBox[0]) / 2), ry: r4(((subjBox[3] - subjBox[1]) * H) / W / 2), angle: 0, areaFrac: r4((subjW * subjH) / N) });
  }

  // Detail: where the edges concentrate within the subject box (3×3 cells).
  const edges = colorSobel(img, level.blurRadius);
  const et = percentileThreshold(edges, level.edgeFraction, level.minEdge);
  const cells = new Array(9).fill(0);
  let edgeCount = 0;
  for (let y = sy0; y <= sy1; y++) for (let x = sx0; x <= sx1; x++) {
    if (edges.data[y * W + x] < et) continue;
    const col = Math.min(2, Math.floor(((x - sx0) * 3) / subjW));
    const row = Math.min(2, Math.floor(((y - sy0) * 3) / subjH));
    cells[row * 3 + col]++; edgeCount++;
  }
  let best = 4;
  for (let i = 0; i < 9; i++) if (cells[i] > cells[best]) best = i;
  const detailPos = 'tmb'[Math.floor(best / 3)] + 'lcr'[best % 3];

  // Landmarks: extreme points of the subject.
  let top = null, bottom = null, left = null, right = null;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (!subj[y * W + x]) continue;
    if (!top) top = [x, y];
    bottom = [x, y];
    if (!left || x < left[0]) left = [x, y];
    if (!right || x > right[0]) right = [x, y];
  }
  const lm = (name, p) => ({ name, x: r4((p[0] + 0.5) / W), y: r4((p[1] + 0.5) / H) });
  const landmarks = top ? [lm('top', top), lm('bottom', bottom), lm('left', left), lm('right', right)] : [];
  if (darkN > 0) landmarks.push(lm('darkest', [darkSX / darkN, darkSY / darkN]));

  const darkPos = darkN > 0
    ? posCode((darkSX / darkN + 0.5) / W, (darkSY / darkN + 0.5) / H, subjBox)
    : 'mc';

  const technique = computeTechnique(img, subj, lv, L, subjCount, subjBox, W, H);

  return {
    engine: 'local-v1',
    width: W, height: H, aspect: r4(W / H),
    threshold: t, lowContrast,
    subject: { bbox: subjBox, aspect: r4(subjW / subjH), whole: useWhole, pixelBox: subjBoxPx },
    objects: objects.map(({ _c, label, ...o }) => o),
    activeObjects: active.map((o) => o.id),
    silhouettes, internals, shapes,
    values: { levels: L, fractions: counts.map((c) => r4(subjCount ? c / subjCount : 0)), darkPos, lightDir },
    detail: { cells, pos: detailPos, density: r4(subjCount ? edgeCount / (subjW * subjH) : 0) },
    landmarks,
    technique,
  };
}

/** Nearest pixel (raster order on ties) with mask[i] to the point (x, y). */
function nearestIn(pred, W, H, x, y) {
  let best = -1, bd = Infinity;
  for (let yy = 0; yy < H; yy++) for (let xx = 0; xx < W; xx++) {
    const i = yy * W + xx;
    if (!pred(i)) continue;
    const d = (xx - x) * (xx - x) + (yy - y) * (yy - y);
    if (d < bd) { bd = d; best = i; }
  }
  return best;
}

/**
 * Shading/colour technique data: value bands with label anchors (for pencil
 * pressure guidance) and a subject colour palette (deterministic k-means, k=5).
 */
export function computeTechnique(img, subj, lv, L, subjCount, subjBox, W, H) {
  const bands = [];
  for (let b = 0; b < L; b++) {
    let n = 0, sx = 0, sy = 0;
    for (let i = 0; i < W * H; i++) if (subj[i] && lv[i] === b) { n++; sx += i % W; sy += Math.floor(i / W); }
    if (n < 0.01 * subjCount || n === 0) continue;
    const at = nearestIn((i) => subj[i] && lv[i] === b, W, H, sx / n, sy / n);
    const ax = at % W, ay = Math.floor(at / W);
    const cx = r4((ax + 0.5) / W), cy = r4((ay + 0.5) / H);
    bands.push({ band: b, frac: r4(n / subjCount), x: cx, y: cy, pos: posCode(cx, cy, subjBox) });
  }
  // Palette
  const px = [];
  for (let i = 0; i < W * H; i++) if (subj[i]) px.push(i);
  const lum = (i) => 77 * img.data[i * 4] + 150 * img.data[i * 4 + 1] + 29 * img.data[i * 4 + 2];
  const sorted = px.slice().sort((a, b) => lum(a) - lum(b) || a - b);
  let centers = [0.1, 0.3, 0.5, 0.7, 0.9].map((q) => {
    const i = sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
    return [img.data[i * 4], img.data[i * 4 + 1], img.data[i * 4 + 2]];
  });
  const assign = new Uint8Array(W * H);
  for (let it = 0; it < 8; it++) {
    const acc = centers.map(() => [0, 0, 0, 0]);
    for (const i of px) {
      let best = 0, bd = Infinity;
      for (let k = 0; k < centers.length; k++) {
        const c = centers[k];
        const d = Math.abs(img.data[i * 4] - c[0]) + Math.abs(img.data[i * 4 + 1] - c[1]) + Math.abs(img.data[i * 4 + 2] - c[2]);
        if (d < bd) { bd = d; best = k; }
      }
      assign[i] = best;
      const a = acc[best]; a[0] += img.data[i * 4]; a[1] += img.data[i * 4 + 1]; a[2] += img.data[i * 4 + 2]; a[3]++;
    }
    centers = centers.map((c, k) => (acc[k][3] ? [acc[k][0] / acc[k][3], acc[k][1] / acc[k][3], acc[k][2] / acc[k][3]] : c));
  }
  const counts = centers.map(() => [0, 0, 0]);
  for (const i of px) { const c = counts[assign[i]]; c[0]++; c[1] += i % W; c[2] += Math.floor(i / W); }
  const palette = [];
  centers.forEach((c, k) => {
    const [n, sx, sy] = counts[k];
    if (!n || n < 0.03 * px.length) return;
    const at = nearestIn((i) => subj[i] && assign[i] === k, W, H, sx / n, sy / n);
    const x = r4(((at % W) + 0.5) / W), y = r4((Math.floor(at / W) + 0.5) / H);
    const rgb = c.map((v) => Math.round(v));
    palette.push({ rgb, frac: r4(n / px.length), x, y, pos: posCode(x, y, subjBox), lum: (77 * rgb[0] + 150 * rgb[1] + 29 * rgb[2]) >> 8 });
  });
  palette.sort((a, b) => b.frac - a.frac || a.lum - b.lum);
  return { bands, palette };
}
