// Perspective geometry shared by AR alignment and drawing comparison.
// Mirrored in rust/crates/drawstep-core/src/geometry.rs.

import { blur2, otsu, labelComponents } from './image.js';

/**
 * Homography H (row-major 3×3, h[8] = 1) mapping each src[i] → dst[i].
 * src/dst: four [x, y] points. Returns null if degenerate.
 */
export function homography(src, dst) {
  const A = [];
  for (let i = 0; i < 4; i++) {
    const [x, y] = src[i], [u, v] = dst[i];
    A.push([x, y, 1, 0, 0, 0, -u * x, -u * y, u]);
    A.push([0, 0, 0, x, y, 1, -v * x, -v * y, v]);
  }
  // Gaussian elimination with partial pivoting on the 8×9 augmented matrix.
  for (let c = 0; c < 8; c++) {
    let p = c;
    for (let r = c + 1; r < 8; r++) if (Math.abs(A[r][c]) > Math.abs(A[p][c])) p = r;
    if (Math.abs(A[p][c]) < 1e-12) return null;
    [A[c], A[p]] = [A[p], A[c]];
    for (let r = 0; r < 8; r++) {
      if (r === c) continue;
      const f = A[r][c] / A[c][c];
      for (let k = c; k < 9; k++) A[r][k] -= f * A[c][k];
    }
  }
  const h = [];
  for (let i = 0; i < 8; i++) h.push(A[i][8] / A[i][i]);
  h.push(1);
  return h;
}

export function applyH(h, x, y) {
  const w = h[6] * x + h[7] * y + h[8];
  return [(h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w];
}

/** CSS matrix3d() string mapping a w×h element (transform-origin 0 0) onto quad. */
export function cssMatrix3d(w, h, quad) {
  const H = homography([[0, 0], [w, 0], [w, h], [0, h]], quad);
  if (!H) return 'none';
  const m = [H[0], H[3], 0, H[6], H[1], H[4], 0, H[7], 0, 0, 1, 0, H[2], H[5], 0, H[8]];
  return `matrix3d(${m.map((v) => +v.toFixed(10)).join(',')})`;
}

/** Corners of a rect of size (w,h) centred at (cx,cy), scaled and rotated (degrees). */
export function transformedRect(cx, cy, w, h, scale, rotDeg) {
  const a = (rotDeg * Math.PI) / 180, c = Math.cos(a), s = Math.sin(a);
  return [[-w / 2, -h / 2], [w / 2, -h / 2], [w / 2, h / 2], [-w / 2, h / 2]].map(([x, y]) => [
    cx + (x * c - y * s) * scale, cy + (x * s + y * c) * scale,
  ]);
}

/** Order four points as TL, TR, BR, BL. */
export function orderCorners(pts) {
  const sum = (p) => p[0] + p[1], diff = (p) => p[0] - p[1];
  const tl = pts.reduce((a, b) => (sum(b) < sum(a) ? b : a));
  const br = pts.reduce((a, b) => (sum(b) > sum(a) ? b : a));
  const tr = pts.reduce((a, b) => (diff(b) > diff(a) ? b : a));
  const bl = pts.reduce((a, b) => (diff(b) < diff(a) ? b : a));
  return [tl, tr, br, bl];
}

export function polygonArea(q) {
  let a = 0;
  for (let i = 0; i < q.length; i++) {
    const [x1, y1] = q[i], [x2, y2] = q[(i + 1) % q.length];
    a += x1 * y2 - x2 * y1;
  }
  return Math.abs(a) / 2;
}

/**
 * Find a sheet of paper: the largest bright region, approximated by its four
 * extreme corners. Returns { corners (normalized TL,TR,BR,BL), confidence } or null.
 */
export function detectPaper(gray) {
  const W = gray.width, H = gray.height, N = W * H;
  const b = blur2(gray, 2);
  const t = otsu(b);
  const m = new Uint8Array(N);
  for (let i = 0; i < N; i++) m[i] = b.data[i] > t ? 1 : 0;
  const { labels, comps } = labelComponents(m, W, H);
  if (!comps.length) return null;
  let best = comps[0];
  for (const c of comps) if (c.area > best.area) best = c;
  if (best.area < 0.12 * N) return null;
  let tl = null, tr = null, br = null, bl = null;
  for (let y = best.minY; y <= best.maxY; y++) for (let x = best.minX; x <= best.maxX; x++) {
    if (labels[y * W + x] !== best.label) continue;
    if (!tl || x + y < tl[0] + tl[1]) tl = [x, y];
    if (!br || x + y > br[0] + br[1]) br = [x, y];
    if (!tr || x - y > tr[0] - tr[1]) tr = [x, y];
    if (!bl || x - y < bl[0] - bl[1]) bl = [x, y];
  }
  const quad = [tl, tr, br, bl];
  const qa = polygonArea(quad);
  if (qa < 1) return null;
  const fill = Math.min(1, best.area / qa);
  const confidence = Math.round(Math.min(1, (best.area / N) * 2) * fill * fill * 100) / 100;
  return { corners: quad.map(([x, y]) => [(x + 0.5) / W, (y + 0.5) / H]), confidence };
}

/** Warp the quad (normalized corners TL,TR,BR,BL) of a gray image into an outW×outH rectangle (bilinear). */
export function warpGray(gray, corners, outW, outH) {
  const W = gray.width, H = gray.height;
  const quad = corners.map(([x, y]) => [x * W - 0.5, y * H - 0.5]);
  const h = homography([[0, 0], [outW - 1, 0], [outW - 1, outH - 1], [0, outH - 1]], quad);
  const out = { width: outW, height: outH, data: new Uint8Array(outW * outH) };
  if (!h) return out;
  for (let v = 0; v < outH; v++) for (let u = 0; u < outW; u++) {
    let [x, y] = applyH(h, u, v);
    x = Math.min(Math.max(x, 0), W - 1); y = Math.min(Math.max(y, 0), H - 1);
    const x0 = Math.floor(x), y0 = Math.floor(y), x1 = Math.min(x0 + 1, W - 1), y1 = Math.min(y0 + 1, H - 1);
    const fx = x - x0, fy = y - y0;
    const d = gray.data;
    const top = d[y0 * W + x0] * (1 - fx) + d[y0 * W + x1] * fx;
    const bot = d[y1 * W + x0] * (1 - fx) + d[y1 * W + x1] * fx;
    out.data[v * outW + u] = Math.round(top * (1 - fy) + bot * fy);
  }
  return out;
}

/** Rasterize a polyline (pixel coords) into a 0/1 mask with a square brush of radius r. */
export function rasterPolyline(mask, W, H, pts, closed, r) {
  const stamp = (x, y) => {
    for (let yy = y - r; yy <= y + r; yy++) for (let xx = x - r; xx <= x + r; xx++)
      if (xx >= 0 && yy >= 0 && xx < W && yy < H) mask[yy * W + xx] = 1;
  };
  const n = pts.length;
  const segs = closed ? n : n - 1;
  for (let i = 0; i < segs; i++) {
    let [x0, y0] = pts[i].map(Math.round);
    const [x1, y1] = pts[(i + 1) % n].map(Math.round);
    const dx = Math.abs(x1 - x0), dy = -Math.abs(y1 - y0);
    const sx = x0 < x1 ? 1 : -1, sy = y0 < y1 ? 1 : -1;
    let err = dx + dy;
    for (;;) {
      stamp(x0, y0);
      if (x0 === x1 && y0 === y1) break;
      const e2 = 2 * err;
      if (e2 >= dy) { err += dy; x0 += sx; }
      if (e2 <= dx) { err += dx; y0 += sy; }
    }
  }
  if (n === 1) stamp(Math.round(pts[0][0]), Math.round(pts[0][1]));
}
