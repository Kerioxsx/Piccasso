// Camera-assisted drawing comparison. Works entirely on-device: a photo of the
// user's paper is warped flat, pencil lines are extracted and compared with
// the guide's contours. Feedback is structural and gentle, never a score.
// Mirrored in rust/crates/piccasso-core/src/compare.rs.

import { blur2, dilate } from './image.js';
import { rasterPolyline } from './geometry.js';
import { fill } from './steps.js';

/** Pencil-line mask: pixels clearly darker than their neighbourhood. */
export function extractLines(g) {
  const W = g.width, H = g.height, N = W * H;
  const mean = blur2(g, Math.max(2, Math.round(Math.max(W, H) / 40)));
  const m = new Uint8Array(N);
  const bx = Math.round(W * 0.03), by = Math.round(H * 0.03);
  for (let y = by; y < H - by; y++) for (let x = bx; x < W - bx; x++) {
    const i = y * W + x;
    if (g.data[i] < 200 && g.data[i] + 18 < mean.data[i]) m[i] = 1;
  }
  return m;
}

/** Guide mask on the paper: contours placed inside the drawing area from paperLayout(). */
export function guideMask(analysis, layout, W, H) {
  const m = new Uint8Array(W * H);
  const toPx = ([nx, ny]) => [((layout.drawX + nx * layout.drawW) / layout.paperW) * W,
    ((layout.drawY + ny * layout.drawH) / layout.paperH) * H];
  const r = Math.max(1, Math.round(Math.max(W, H) / 250));
  for (const c of analysis.silhouettes.concat(analysis.internals)) rasterPolyline(m, W, H, c.points.map(toPx), c.closed, r);
  return m;
}

/** Robust bbox (2nd–98th percentile of coordinates) of a mask. */
export function robustBox(mask, W, H) {
  const xs = new Array(W).fill(0), ys = new Array(H).fill(0);
  let n = 0;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) if (mask[y * W + x]) { xs[x]++; ys[y]++; n++; }
  if (!n) return null;
  const pct = (hist, p) => { let acc = 0; for (let i = 0; i < hist.length; i++) { acc += hist[i]; if (acc >= p * n) return i; } return hist.length - 1; };
  return { x0: pct(xs, 0.02), x1: pct(xs, 0.98), y0: pct(ys, 0.02), y1: pct(ys, 0.98), count: n };
}

/**
 * @param drawnGray warped photo of the paper (gray image, paper aspect)
 * @param guide     guideMask() at the same size
 * @param strings   shared strings (for observation templates)
 */
export function compareDrawing(drawnGray, guide, strings) {
  const W = drawnGray.width, H = drawnGray.height, N = W * H;
  const S = strings.compare;
  const drawn = extractLines(drawnGray);
  const db = robustBox(drawn, W, H), gb = robustBox(guide, W, H);
  const obs = [];
  if (!db || db.count < 0.002 * N || !gb) {
    return { observations: [S.empty], metrics: null, drawn };
  }
  const gw = gb.x1 - gb.x0 + 1, gh = gb.y1 - gb.y0 + 1;
  const dw = db.x1 - db.x0 + 1, dh = db.y1 - db.y0 + 1;
  const wr = dw / gw, hr = dh / gh;
  const offX = ((db.x0 + db.x1) / 2 - (gb.x0 + gb.x1) / 2) / gw;
  const offY = ((db.y0 + db.y1) / 2 - (gb.y0 + gb.y1) / 2) / gh;
  const pct = (r) => Math.round(Math.abs(r - 1) * 100);
  if (wr > 1.08) obs.push(fill(S.wider, { pct: pct(wr) }));
  else if (wr < 0.92) obs.push(fill(S.narrower, { pct: pct(wr) }));
  if (hr > 1.08) obs.push(fill(S.taller, { pct: pct(hr) }));
  else if (hr < 0.92) obs.push(fill(S.shorter, { pct: pct(hr) }));
  if (offX < -0.08) obs.push(S.shiftLeft); else if (offX > 0.08) obs.push(S.shiftRight);
  if (offY < -0.08) obs.push(S.shiftUp); else if (offY > 0.08) obs.push(S.shiftDown);

  // Coverage of guide lines by drawn lines, per 3×3 cell of the guide box.
  const near = dilate(drawn, W, H, Math.max(2, Math.round(Math.max(W, H) / 60)));
  const tot = new Array(9).fill(0), hit = new Array(9).fill(0);
  for (let y = gb.y0; y <= gb.y1; y++) for (let x = gb.x0; x <= gb.x1; x++) {
    const i = y * W + x;
    if (!guide[i]) continue;
    const c = Math.min(2, Math.floor(((y - gb.y0) * 3) / gh)) * 3 + Math.min(2, Math.floor(((x - gb.x0) * 3) / gw));
    tot[c]++; if (near[i]) hit[c]++;
  }
  let worst = -1, worstCov = 1, gtot = 0, ghit = 0;
  const minCell = Math.max(8, 0.04 * tot.reduce((a, b) => a + b, 0));
  for (let c = 0; c < 9; c++) {
    gtot += tot[c]; ghit += hit[c];
    if (tot[c] < minCell) continue;
    const cov = hit[c] / tot[c];
    if (cov < worstCov) { worstCov = cov; worst = c; }
  }
  if (worst >= 0 && worstCov < 0.35) {
    const code = 'tmb'[Math.floor(worst / 3)] + 'lcr'[worst % 3];
    obs.push(fill(S.sparse, { pos: strings.positions[code] }));
  }
  if (!obs.length) obs.push(S.good);
  return {
    observations: obs, drawn,
    metrics: { widthRatio: wr, heightRatio: hr, offsetX: offX, offsetY: offY, coverage: gtot ? ghit / gtot : 0 },
  };
}
