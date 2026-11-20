// Piccasso image-processing primitives.
//
// Every function here is pure and DOM-free so it can run in the main thread,
// in a Web Worker, or under Node's test runner. The algorithms are specified in
// docs/ALGORITHMS.md and mirrored 1:1 in rust/crates/piccasso-core/src/imgproc.rs;
// shared test vectors (shared/test-vectors.json) keep the two in step.
//
// Types:
//   RGBA image : { width, height, data: Uint8ClampedArray(width*height*4) }
//   Gray image : { width, height, data: Uint8Array(width*height) }
//   Mask       : Uint8Array of 0/1 values, same layout as Gray.data

export function makeRGBA(width, height) {
  return { width, height, data: new Uint8ClampedArray(width * height * 4) };
}

export function makeGray(width, height) {
  return { width, height, data: new Uint8Array(width * height) };
}

const clamp255 = (v) => (v < 0 ? 0 : v > 255 ? 255 : v);

/** Crop with a normalized rect {x,y,w,h} in 0..1. Always returns at least 1×1. */
export function crop(img, rect) {
  const { width: W, height: H } = img;
  if (!rect) return img;
  let x0 = Math.round(rect.x * W), y0 = Math.round(rect.y * H);
  x0 = Math.min(Math.max(x0, 0), W - 1);
  y0 = Math.min(Math.max(y0, 0), H - 1);
  let w = Math.max(1, Math.round(rect.w * W)), h = Math.max(1, Math.round(rect.h * H));
  w = Math.min(w, W - x0);
  h = Math.min(h, H - y0);
  if (x0 === 0 && y0 === 0 && w === W && h === H) return img;
  const out = makeRGBA(w, h);
  for (let y = 0; y < h; y++) {
    const s = ((y0 + y) * W + x0) * 4;
    out.data.set(img.data.subarray(s, s + w * 4), y * w * 4);
  }
  return out;
}

/** Rotate clockwise by 0/90/180/270 degrees. */
export function rotate(img, deg) {
  const d = ((deg % 360) + 360) % 360;
  if (d === 0) return img;
  const { width: W, height: H, data } = img;
  const out = d === 180 ? makeRGBA(W, H) : makeRGBA(H, W);
  const ow = out.width;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      let nx, ny;
      if (d === 90) { nx = H - 1 - y; ny = x; }
      else if (d === 180) { nx = W - 1 - x; ny = H - 1 - y; }
      else { nx = y; ny = W - 1 - x; }
      const si = (y * W + x) * 4, di = (ny * ow + nx) * 4;
      out.data[di] = data[si]; out.data[di + 1] = data[si + 1];
      out.data[di + 2] = data[si + 2]; out.data[di + 3] = data[si + 3];
    }
  }
  return out;
}

export function flip(img, horizontal, vertical) {
  if (!horizontal && !vertical) return img;
  const { width: W, height: H, data } = img;
  const out = makeRGBA(W, H);
  for (let y = 0; y < H; y++) {
    const sy = vertical ? H - 1 - y : y;
    for (let x = 0; x < W; x++) {
      const sx = horizontal ? W - 1 - x : x;
      const si = (sy * W + sx) * 4, di = (y * W + x) * 4;
      out.data[di] = data[si]; out.data[di + 1] = data[si + 1];
      out.data[di + 2] = data[si + 2]; out.data[di + 3] = data[si + 3];
    }
  }
  return out;
}

/** Target size that fits inside maxDim while preserving aspect. Never upscales. */
export function fitDims(w, h, maxDim) {
  const m = Math.max(w, h);
  if (m <= maxDim) return [w, h];
  const s = maxDim / m;
  return [Math.max(1, Math.round(w * s)), Math.max(1, Math.round(h * s))];
}

/** Area-average (box) resize. Deterministic integer arithmetic. */
export function resizeBox(img, dw, dh) {
  const { width: sw, height: sh, data } = img;
  if (dw === sw && dh === sh) return img;
  const out = makeRGBA(dw, dh);
  for (let y = 0; y < dh; y++) {
    const y0 = Math.floor((y * sh) / dh);
    const y1 = Math.max(y0 + 1, Math.floor(((y + 1) * sh) / dh));
    for (let x = 0; x < dw; x++) {
      const x0 = Math.floor((x * sw) / dw);
      const x1 = Math.max(x0 + 1, Math.floor(((x + 1) * sw) / dw));
      let r = 0, g = 0, b = 0, a = 0;
      for (let yy = y0; yy < y1; yy++) {
        let i = (yy * sw + x0) * 4;
        for (let xx = x0; xx < x1; xx++, i += 4) {
          r += data[i]; g += data[i + 1]; b += data[i + 2]; a += data[i + 3];
        }
      }
      const n = (y1 - y0) * (x1 - x0), half = n >> 1;
      const o = (y * dw + x) * 4;
      out.data[o] = Math.floor((r + half) / n);
      out.data[o + 1] = Math.floor((g + half) / n);
      out.data[o + 2] = Math.floor((b + half) / n);
      out.data[o + 3] = Math.floor((a + half) / n);
    }
  }
  return out;
}

/** brightness, contrast in -100..100. Transparent pixels are composited on white. */
export function adjust(img, brightness = 0, contrast = 0) {
  const out = makeRGBA(img.width, img.height);
  const cf = (100 + contrast) / 100;
  const bo = brightness * 2.55;
  const src = img.data, dst = out.data;
  for (let i = 0; i < src.length; i += 4) {
    const a = src[i + 3];
    for (let c = 0; c < 3; c++) {
      // composite on white first so transparent PNGs behave like paper
      const v = Math.floor((src[i + c] * a + 255 * (255 - a) + 127) / 255);
      dst[i + c] = clamp255(Math.round((v - 128) * cf + 128 + bo));
    }
    dst[i + 3] = 255;
  }
  return out;
}

/** Integer luma: (77R + 150G + 29B + 128) >> 8 */
export function toGray(img) {
  const out = makeGray(img.width, img.height);
  const s = img.data, d = out.data;
  for (let i = 0, j = 0; j < d.length; i += 4, j++) {
    d[j] = (77 * s[i] + 150 * s[i + 1] + 29 * s[i + 2] + 128) >> 8;
  }
  return out;
}

export function grayToRGBA(g) {
  const out = makeRGBA(g.width, g.height);
  for (let j = 0, i = 0; j < g.data.length; j++, i += 4) {
    const v = g.data[j];
    out.data[i] = v; out.data[i + 1] = v; out.data[i + 2] = v; out.data[i + 3] = 255;
  }
  return out;
}

/** One separable box-blur pass with edge clamping. */
export function boxBlur(g, r) {
  if (r <= 0) return g;
  const { width: W, height: H } = g;
  const n = 2 * r + 1, half = n >> 1;
  const tmp = new Uint8Array(W * H);
  const out = makeGray(W, H);
  for (let y = 0; y < H; y++) {
    const row = y * W;
    for (let x = 0; x < W; x++) {
      let s = 0;
      for (let k = -r; k <= r; k++) {
        const xx = x + k < 0 ? 0 : x + k >= W ? W - 1 : x + k;
        s += g.data[row + xx];
      }
      tmp[row + x] = Math.floor((s + half) / n);
    }
  }
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      let s = 0;
      for (let k = -r; k <= r; k++) {
        const yy = y + k < 0 ? 0 : y + k >= H ? H - 1 : y + k;
        s += tmp[yy * W + x];
      }
      out.data[y * W + x] = Math.floor((s + half) / n);
    }
  }
  return out;
}

/** Two box passes ≈ a Gaussian blur. */
export function blur2(g, r) {
  return boxBlur(boxBlur(g, r), r);
}

/** Sobel gradient magnitude, L1 norm, scaled by 1/4 and clamped to 255. */
export function sobel(g) {
  const { width: W, height: H, data: d } = g;
  const out = makeGray(W, H);
  const px = (x, y) => d[(y < 0 ? 0 : y >= H ? H - 1 : y) * W + (x < 0 ? 0 : x >= W ? W - 1 : x)];
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const a = px(x - 1, y - 1), b = px(x, y - 1), c = px(x + 1, y - 1);
      const dd = px(x - 1, y), f = px(x + 1, y);
      const gg = px(x - 1, y + 1), h = px(x, y + 1), i = px(x + 1, y + 1);
      const gx = c + 2 * f + i - a - 2 * dd - gg;
      const gy = gg + 2 * h + i - a - 2 * b - c;
      const m = (Math.abs(gx) + Math.abs(gy)) >> 2;
      out.data[y * W + x] = m > 255 ? 255 : m;
    }
  }
  return out;
}

export function histogram(g) {
  const h = new Array(256).fill(0);
  for (let i = 0; i < g.data.length; i++) h[g.data[i]]++;
  return h;
}

/** Smallest threshold t (>= minT) such that at most `frac` of pixels are >= t. */
export function percentileThreshold(g, frac, minT) {
  const h = histogram(g);
  const target = frac * g.data.length;
  let count = 0;
  for (let t = 255; t >= 0; t--) {
    if (count + h[t] > target) return Math.max(t + 1, minT);
    count += h[t];
  }
  return minT;
}

/** Otsu's threshold. Pixels with value > t form the "bright" class. */
export function otsu(g) {
  const h = histogram(g);
  const total = g.data.length;
  let sumAll = 0;
  for (let i = 0; i < 256; i++) sumAll += i * h[i];
  let wB = 0, sumB = 0, best = -1, bestT = 127;
  for (let t = 0; t < 256; t++) {
    wB += h[t];
    if (wB === 0) continue;
    const wF = total - wB;
    if (wF === 0) break;
    sumB += t * h[t];
    const mB = sumB / wB, mF = (sumAll - sumB) / wF;
    const between = wB * wF * (mB - mF) * (mB - mF);
    if (between > best) { best = between; bestT = t; }
  }
  return bestT;
}

/** Square (Chebyshev) dilation of a 0/1 mask by radius r. */
export function dilate(mask, W, H, r) {
  if (r <= 0) return mask;
  const tmp = new Uint8Array(W * H), out = new Uint8Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let v = 0;
    for (let k = Math.max(0, x - r); k <= Math.min(W - 1, x + r) && !v; k++) v = mask[y * W + k];
    tmp[y * W + x] = v;
  }
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    let v = 0;
    for (let k = Math.max(0, y - r); k <= Math.min(H - 1, y + r) && !v; k++) v = tmp[k * W + x];
    out[y * W + x] = v;
  }
  return out;
}

export function erode(mask, W, H, r) {
  if (r <= 0) return mask;
  const inv = new Uint8Array(W * H);
  for (let i = 0; i < inv.length; i++) inv[i] = mask[i] ? 0 : 1;
  const d = dilate(inv, W, H, r);
  const out = new Uint8Array(W * H);
  for (let i = 0; i < out.length; i++) out[i] = d[i] ? 0 : 1;
  return out;
}

/**
 * 4-connected component labelling (raster-order flood fill, so labels are
 * deterministic). Returns labels (0 = background, 1..n) and per-component stats.
 */
export function labelComponents(mask, W, H) {
  const labels = new Int32Array(W * H);
  const comps = [];
  const stack = new Int32Array(W * H);
  let next = 1;
  for (let start = 0; start < W * H; start++) {
    if (!mask[start] || labels[start]) continue;
    const c = {
      label: next, area: 0, minX: W, minY: H, maxX: -1, maxY: -1,
      sumX: 0, sumY: 0, sumXX: 0, sumYY: 0, sumXY: 0,
      startX: start % W, startY: Math.floor(start / W), borders: 0,
    };
    let sp = 0;
    stack[sp++] = start;
    labels[start] = next;
    while (sp > 0) {
      const p = stack[--sp];
      const x = p % W, y = (p - x) / W;
      c.area++;
      if (x < c.minX) c.minX = x; if (x > c.maxX) c.maxX = x;
      if (y < c.minY) c.minY = y; if (y > c.maxY) c.maxY = y;
      c.sumX += x; c.sumY += y; c.sumXX += x * x; c.sumYY += y * y; c.sumXY += x * y;
      if (x > 0 && mask[p - 1] && !labels[p - 1]) { labels[p - 1] = next; stack[sp++] = p - 1; }
      if (x < W - 1 && mask[p + 1] && !labels[p + 1]) { labels[p + 1] = next; stack[sp++] = p + 1; }
      if (y > 0 && mask[p - W] && !labels[p - W]) { labels[p - W] = next; stack[sp++] = p - W; }
      if (y < H - 1 && mask[p + W] && !labels[p + W]) { labels[p + W] = next; stack[sp++] = p + W; }
    }
    c.borders = (c.minX === 0) + (c.minY === 0) + (c.maxX === W - 1) + (c.maxY === H - 1);
    comps.push(c);
    next++;
  }
  return { labels, comps };
}

// Clockwise neighbour order in image coordinates (y grows downward).
const DX = [1, 1, 0, -1, -1, -1, 0, 1];
const DY = [0, 1, 1, 1, 0, -1, -1, -1];

/**
 * Moore-neighbour boundary trace of the component `label`, starting from its
 * first pixel in raster order. Returns integer pixel points, clockwise.
 */
export function traceBoundary(labels, W, H, label, sx, sy, maxSteps) {
  const pts = [[sx, sy]];
  let cx = sx, cy = sy, dir = 0;
  const limit = maxSteps || 4 * W * H;
  for (let iter = 0; iter < limit; iter++) {
    let found = false;
    const d0 = (dir + 6) % 8;
    for (let k = 0; k < 8; k++) {
      const nd = (d0 + k) % 8;
      const nx = cx + DX[nd], ny = cy + DY[nd];
      if (nx >= 0 && ny >= 0 && nx < W && ny < H && labels[ny * W + nx] === label) {
        cx = nx; cy = ny; dir = nd; found = true;
        break;
      }
    }
    if (!found || (cx === sx && cy === sy)) break;
    pts.push([cx, cy]);
  }
  return pts;
}

function perpDist(p, a, b) {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  if (len2 === 0) return Math.hypot(p[0] - a[0], p[1] - a[1]);
  return Math.abs(dy * p[0] - dx * p[1] + b[0] * a[1] - b[1] * a[0]) / Math.sqrt(len2);
}

/** Ramer–Douglas–Peucker simplification (iterative, deterministic). */
export function douglasPeucker(points, eps, closed) {
  if (points.length < 3) return points.slice();
  const pts = closed ? points.concat([points[0]]) : points;
  const keep = new Uint8Array(pts.length);
  keep[0] = 1; keep[pts.length - 1] = 1;
  const stack = [[0, pts.length - 1]];
  while (stack.length) {
    const [s, e] = stack.pop();
    let maxD = -1, idx = -1;
    for (let i = s + 1; i < e; i++) {
      const d = perpDist(pts[i], pts[s], pts[e]);
      if (d > maxD) { maxD = d; idx = i; }
    }
    if (idx >= 0 && maxD > eps) {
      keep[idx] = 1;
      stack.push([s, idx], [idx, e]);
    }
  }
  const out = [];
  for (let i = 0; i < pts.length; i++) if (keep[i]) out.push(pts[i]);
  if (closed) out.pop();
  return out;
}

/** Quantize gray values into `levels` bands. Returns band index per pixel (0 = darkest). */
export function posterizeLevels(g, levels) {
  const out = new Uint8Array(g.data.length);
  for (let i = 0; i < out.length; i++) out[i] = Math.floor((g.data[i] * levels) / 256);
  return out;
}

/**
 * Apply the project's preparation settings (rotate → flip → crop → resize →
 * brightness/contrast) and return an opaque RGBA image no larger than maxDim.
 */
export function prepareImage(src, prep, maxDim) {
  let img = rotate(src, prep.rotation || 0);
  img = flip(img, !!prep.flipH, !!prep.flipV);
  img = crop(img, prep.crop); // crop rect is relative to the rotated/flipped image
  const [w, h] = fitDims(img.width, img.height, maxDim);
  img = resizeBox(img, w, h);
  return adjust(img, prep.brightness || 0, prep.contrast || 0);
}

/** Split RGBA into three blurred gray channels. */
function blurredChannels(img, r) {
  const W = img.width, H = img.height;
  const ch = [0, 1, 2].map((c) => {
    const g = makeGray(W, H);
    for (let i = 0; i < W * H; i++) g.data[i] = img.data[i * 4 + c];
    return blur2(g, r);
  });
  return ch;
}

/**
 * Foreground = pixels whose colour is far from every dominant border colour.
 * Border colours: deterministic k-means (k=3, 8 iterations) over the outer
 * 2-pixel band, initialised at the 10/50/90% luminance quantiles.
 * Returns { mask, threshold, separation, dist }.
 */
export function foregroundFromBorder(img, r) {
  const W = img.width, H = img.height, N = W * H;
  const [R, G, B] = blurredChannels(img, r);
  const border = [];
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    if (x > 1 && y > 1 && x < W - 2 && y < H - 2) continue;
    const i = y * W + x;
    border.push([R.data[i], G.data[i], B.data[i]]);
  }
  const lum = (p) => 77 * p[0] + 150 * p[1] + 29 * p[2];
  const sorted = border.slice().sort((a, b) => lum(a) - lum(b));
  let centers = [0.1, 0.5, 0.9].map((q) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))].slice());
  for (let it = 0; it < 8; it++) {
    const acc = centers.map(() => [0, 0, 0, 0]);
    for (const p of border) {
      let best = 0, bd = Infinity;
      for (let k = 0; k < centers.length; k++) {
        const d = Math.abs(p[0] - centers[k][0]) + Math.abs(p[1] - centers[k][1]) + Math.abs(p[2] - centers[k][2]);
        if (d < bd) { bd = d; best = k; }
      }
      const a = acc[best]; a[0] += p[0]; a[1] += p[1]; a[2] += p[2]; a[3]++;
    }
    centers = centers.map((c, k) => (acc[k][3] ? [acc[k][0] / acc[k][3], acc[k][1] / acc[k][3], acc[k][2] / acc[k][3]] : c));
  }
  const dist = makeGray(W, H);
  for (let i = 0; i < N; i++) {
    let bd = Infinity;
    for (const c of centers) {
      const d = (Math.abs(R.data[i] - c[0]) + Math.abs(G.data[i] - c[1]) + Math.abs(B.data[i] - c[2])) / 3;
      if (d < bd) bd = d;
    }
    dist.data[i] = bd > 255 ? 255 : Math.round(bd);
  }
  const threshold = Math.max(20, otsu(dist));
  const mask = new Uint8Array(N);
  let sLo = 0, nLo = 0, sHi = 0, nHi = 0;
  for (let i = 0; i < N; i++) {
    const v = dist.data[i];
    if (v > threshold) { mask[i] = 1; sHi += v; nHi++; } else { sLo += v; nLo++; }
  }
  const separation = nHi && nLo ? sHi / nHi - sLo / nLo : 0;
  return { mask, threshold, separation, dist };
}

/** Colour-aware edges: Sobel magnitude per blurred RGB channel, maximum of the three. */
export function colorSobel(img, r) {
  const [R, G, B] = blurredChannels(img, r);
  const a = sobel(R), b = sobel(G), c = sobel(B);
  const out = makeGray(img.width, img.height);
  for (let i = 0; i < out.data.length; i++) out.data[i] = Math.max(a.data[i], b.data[i], c.data[i]);
  return out;
}

/** Morphological clean-up used for subject masks: close(1) then open(2). */
export function cleanMask(mask, W, H) {
  const closed = erode(dilate(mask, W, H, 1), W, H, 1);
  return dilate(erode(closed, W, H, 2), W, H, 2);
}
