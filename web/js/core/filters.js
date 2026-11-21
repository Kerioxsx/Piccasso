// Reference conversions for the Studio: classic image-processing filters, no AI.
// All functions take and return RGBA images {width, height, data: Uint8ClampedArray}.
import { makeRGBA, toGray, grayToRGBA, boxBlur, sobel, adjust, flip, foregroundFromBorder, dilate, posterizeLevels } from './image.js';

const lum = (d, i) => (77 * d[i] + 150 * d[i + 1] + 29 * d[i + 2] + 128) >> 8;
const fromGray = (g) => grayToRGBA(g);
const grayOf = (img) => toGray(img);
const mapGray = (img, f) => { const g = grayOf(img); for (let i = 0; i < g.data.length; i++) g.data[i] = f(g.data[i], i); return fromGray(g); };

export const grayscale = (img) => fromGray(grayOf(img));
export const invert = (img) => { const o = makeRGBA(img.width, img.height); for (let i = 0; i < img.data.length; i += 4) { o.data[i] = 255 - img.data[i]; o.data[i + 1] = 255 - img.data[i + 1]; o.data[i + 2] = 255 - img.data[i + 2]; o.data[i + 3] = img.data[i + 3]; } return o; };
export const threshold = (img, t = 128) => mapGray(img, (v) => (v < t ? 0 : 255));

/** Colour or gray posterization to `levels` steps per channel. */
export function posterize(img, levels = 4, gray = false) {
  if (gray) { const g = grayOf(img); const b = posterizeLevels(g, levels); for (let i = 0; i < b.length; i++) g.data[i] = Math.round((b[i] * 255) / (levels - 1)); return fromGray(g); }
  const o = makeRGBA(img.width, img.height); const k = 255 / (levels - 1);
  for (let i = 0; i < img.data.length; i += 4) { for (let c = 0; c < 3; c++) o.data[i + c] = Math.round(Math.round(img.data[i + c] / k) * k); o.data[i + 3] = 255; }
  return o;
}

/** Light / mid / dark value map (the three-value study artists start with). */
export const threeLevels = (img, darkAt = 85, lightAt = 170) => mapGray(img, (v) => (v < darkAt ? 40 : v < lightAt ? 140 : 255));

/** Nine-step value scale (colour to value), for judging how dark each colour really is. */
export const valueScale = (img, steps = 9) => mapGray(img, (v) => Math.round(Math.round((v / 255) * (steps - 1)) * (255 / (steps - 1))));

/** Pencil sketch: colour-dodge of the gray picture with its blurred negative. */
export function pencilSketch(img, radius = 6, darkness = 1) {
  const g = grayOf(img);
  const inv = { width: g.width, height: g.height, data: g.data.map((v) => 255 - v) };
  const b = boxBlur(inv, Math.max(1, radius));
  for (let i = 0; i < g.data.length; i++) {
    const base = g.data[i], blend = b.data[i];
    let v = blend >= 255 ? 255 : Math.min(255, (base * 255) / (255 - blend));
    v = 255 - (255 - v) * darkness;
    g.data[i] = Math.max(0, Math.min(255, v));
  }
  return fromGray(g);
}

/** Edge detection: Sobel magnitude, dark lines on white; `detail` 0..1 lowers the cut-off. */
export function edges(img, detail = 0.5, thickness = 1) {
  const g = grayOf(img);
  const m = sobel(boxBlur(g, 1));
  const mags = m.data;
  let max = 1; for (let i = 0; i < mags.length; i++) if (mags[i] > max) max = mags[i];
  const t = max * (0.45 - 0.4 * detail);
  const mask = new Uint8Array(g.data.length);
  for (let i = 0; i < mask.length; i++) mask[i] = mags[i] > t ? 1 : 0;
  const thick = thickness > 1 ? dilate(mask, g.width, g.height, thickness - 1) : mask;
  for (let i = 0; i < g.data.length; i++) g.data[i] = thick[i] ? 0 : 255;
  return fromGray(g);
}

/** Contour lines: borders between `levels` value bands of the smoothed picture. */
export function contours(img, levels = 5, thickness = 1) {
  const g = boxBlur(grayOf(img), 2);
  const W = g.width, H = g.height, band = new Uint8Array(W * H);
  for (let i = 0; i < band.length; i++) band[i] = Math.min(levels - 1, (g.data[i] * levels) >> 8);
  let mask = new Uint8Array(W * H);
  for (let y = 0; y < H - 1; y++) for (let x = 0; x < W - 1; x++) { const i = y * W + x; if (band[i] !== band[i + 1] || band[i] !== band[i + W]) mask[i] = 1; }
  if (thickness > 1) mask = dilate(mask, W, H, thickness - 1);
  const o = makeRGBA(W, H);
  for (let i = 0; i < W * H; i++) { const v = mask[i] ? 30 : 255; o.data[i * 4] = o.data[i * 4 + 1] = o.data[i * 4 + 2] = v; o.data[i * 4 + 3] = 255; }
  return o;
}

/** Crosshatching reference: darker values get more hatch directions (/, \, —, |). */
export function crosshatch(img, spacing = 6, thickness = 1) {
  const g = boxBlur(grayOf(img), 1), W = g.width, H = g.height, o = makeRGBA(W, H);
  const s = Math.max(3, spacing), t = Math.max(1, thickness);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const v = g.data[y * W + x];
    const on = (v < 200 && (x + y) % s < t) || (v < 150 && ((x - y) % s + s) % s < t) || (v < 100 && y % s < t) || (v < 55 && x % s < t);
    const i = (y * W + x) * 4; const c = on ? 20 : 255;
    o.data[i] = o.data[i + 1] = o.data[i + 2] = c; o.data[i + 3] = 255;
  }
  return o;
}

/** Halftone: a dot per cell, its size following the darkness. */
export function halftone(img, cell = 8) {
  const g = grayOf(img), W = g.width, H = g.height, o = makeRGBA(W, H);
  o.data.fill(255);
  const c = Math.max(3, cell);
  for (let cy = 0; cy < H; cy += c) for (let cx = 0; cx < W; cx += c) {
    let sum = 0, n = 0;
    for (let y = cy; y < Math.min(H, cy + c); y++) for (let x = cx; x < Math.min(W, cx + c); x++) { sum += g.data[y * W + x]; n++; }
    const r = (c / 2) * Math.sqrt(1 - sum / n / 255) * 1.15;
    const mx = cx + c / 2, my = cy + c / 2;
    for (let y = cy; y < Math.min(H, cy + c); y++) for (let x = cx; x < Math.min(W, cx + c); x++) {
      if ((x + 0.5 - mx) ** 2 + (y + 0.5 - my) ** 2 <= r * r) { const i = (y * W + x) * 4; o.data[i] = o.data[i + 1] = o.data[i + 2] = 0; }
    }
  }
  return o;
}

/** Subject mask (conventional segmentation: the subject differs from the picture's border). */
export function subjectMask(img) { return foregroundFromBorder(img, 2).mask; }

/** Silhouette: the subject filled black on white. */
export function silhouette(img) {
  const m = subjectMask(img), o = makeRGBA(img.width, img.height);
  for (let i = 0; i < m.length; i++) { const v = m[i] ? 0 : 255; o.data[i * 4] = o.data[i * 4 + 1] = o.data[i * 4 + 2] = v; o.data[i * 4 + 3] = 255; }
  return o;
}

/** Negative space: the empty space around the subject shaded, the subject left white. */
export function negativeSpace(img) {
  const m = subjectMask(img), W = img.width, H = img.height, o = makeRGBA(W, H);
  const edge = dilate(m, W, H, 1);
  for (let i = 0; i < m.length; i++) {
    const v = m[i] ? 255 : edge[i] ? 20 : 150;
    o.data[i * 4] = v; o.data[i * 4 + 1] = m[i] ? 255 : v; o.data[i * 4 + 2] = m[i] ? 255 : Math.min(255, v + 40); o.data[i * 4 + 3] = 255;
  }
  return o;
}

/** Background removal: pixels outside the subject become white (or transparent). */
export function removeBackground(img, transparent = false) {
  const m = subjectMask(img), o = makeRGBA(img.width, img.height);
  for (let i = 0; i < m.length; i++) {
    const j = i * 4;
    if (m[i]) { o.data[j] = img.data[j]; o.data[j + 1] = img.data[j + 1]; o.data[j + 2] = img.data[j + 2]; o.data[j + 3] = 255; }
    else { o.data[j] = o.data[j + 1] = o.data[j + 2] = 255; o.data[j + 3] = transparent ? 0 : 255; }
  }
  return o;
}

/** Anime-style simplification: flattened colours (median-cut-like posterize after smoothing) with bold dark outlines. */
export function animeSimplify(img, levels = 5, lineDetail = 0.5) {
  const W = img.width, H = img.height;
  const soft = makeRGBA(W, H);
  // smooth each channel, keeping edges by blending only where the picture is flat
  const chans = [0, 1, 2].map((c) => { const g = { width: W, height: H, data: new Uint8ClampedArray(W * H) }; for (let i = 0; i < W * H; i++) g.data[i] = img.data[i * 4 + c]; return boxBlur(g, 2); });
  for (let i = 0; i < W * H; i++) { for (let c = 0; c < 3; c++) soft.data[i * 4 + c] = chans[c].data[i]; soft.data[i * 4 + 3] = 255; }
  const flat = posterize(soft, levels);
  const lines = edges(img, lineDetail, 2);
  for (let i = 0; i < W * H; i++) if (lines.data[i * 4] === 0) { flat.data[i * 4] = flat.data[i * 4 + 1] = flat.data[i * 4 + 2] = 25; }
  return flat;
}

/** Bicubic upscale (conventional interpolation) by `factor`. */
export function upscale(img, factor = 2) {
  const W = img.width, H = img.height, w = Math.round(W * factor), h = Math.round(H * factor), o = makeRGBA(w, h);
  const cub = (t) => { const a = -0.5, x = Math.abs(t); return x <= 1 ? (a + 2) * x ** 3 - (a + 3) * x ** 2 + 1 : x < 2 ? a * x ** 3 - 5 * a * x ** 2 + 8 * a * x - 4 * a : 0; };
  for (let y = 0; y < h; y++) {
    const sy = (y + 0.5) / factor - 0.5, y0 = Math.floor(sy);
    for (let x = 0; x < w; x++) {
      const sx = (x + 0.5) / factor - 0.5, x0 = Math.floor(sx);
      const acc = [0, 0, 0, 0]; let wsum = 0;
      for (let m = -1; m <= 2; m++) {
        const yy = Math.min(H - 1, Math.max(0, y0 + m)), wy = cub(sy - (y0 + m));
        for (let n = -1; n <= 2; n++) {
          const xx = Math.min(W - 1, Math.max(0, x0 + n)), wgt = wy * cub(sx - (x0 + n)), j = (yy * W + xx) * 4;
          for (let c = 0; c < 4; c++) acc[c] += img.data[j + c] * wgt; wsum += wgt;
        }
      }
      const i = (y * w + x) * 4; for (let c = 0; c < 4; c++) o.data[i + c] = acc[c] / wsum;
    }
  }
  return o;
}

/** Run `fn` only inside rect {x, y, w, h} (pixels); the rest stays as it was. */
export function inRegion(img, rect, fn) {
  const out = fn(img);
  if (!rect) return out;
  const o = makeRGBA(img.width, img.height); o.data.set(img.data);
  for (let y = Math.max(0, rect.y); y < Math.min(img.height, rect.y + rect.h); y++) {
    const s = (y * img.width + Math.max(0, rect.x)) * 4, e = (y * img.width + Math.min(img.width, rect.x + rect.w)) * 4;
    o.data.set(out.data.subarray(s, e), s);
  }
  return o;
}

/** All Studio conversions with their adjustable parameters. */
export const CONVERSIONS = {
  original: { label: 'Original', run: (i) => i },
  grayscale: { label: 'Grayscale', run: grayscale },
  sketch: { label: 'Pencil sketch', params: { radius: [2, 20, 6], darkness: [0.3, 2, 1] }, run: (i, p) => pencilSketch(i, p.radius, p.darkness) },
  edges: { label: 'Edge detection', params: { detail: [0, 1, 0.5], thickness: [1, 6, 1] }, run: (i, p) => edges(i, p.detail, p.thickness) },
  contours: { label: 'Contours', params: { levels: [2, 12, 5], thickness: [1, 6, 1] }, run: (i, p) => contours(i, p.levels, p.thickness) },
  three: { label: 'Three values', params: { darkAt: [20, 140, 85], lightAt: [120, 240, 170] }, run: (i, p) => threeLevels(i, p.darkAt, p.lightAt) },
  values: { label: 'Colour → value', params: { steps: [3, 11, 9] }, run: (i, p) => valueScale(i, p.steps) },
  posterize: { label: 'Posterize', params: { levels: [2, 8, 4] }, run: (i, p) => posterize(i, p.levels) },
  threshold: { label: 'Threshold', params: { level: [10, 245, 128] }, run: (i, p) => threshold(i, p.level) },
  hatch: { label: 'Crosshatching', params: { spacing: [3, 16, 6], thickness: [1, 4, 1] }, run: (i, p) => crosshatch(i, p.spacing, p.thickness) },
  halftone: { label: 'Halftone', params: { cell: [4, 24, 8] }, run: (i, p) => halftone(i, p.cell) },
  silhouette: { label: 'Silhouette', run: silhouette },
  negative: { label: 'Negative space', run: negativeSpace },
  nobg: { label: 'Remove background', run: (i) => removeBackground(i) },
  anime: { label: 'Anime simplify', params: { levels: [3, 8, 5], lines: [0, 1, 0.5] }, run: (i, p) => animeSimplify(i, p.levels, p.lines) },
  invert: { label: 'Invert', run: invert },
};

/** Before the conversion: brightness/contrast (-100..100) and mirror. */
export function preAdjust(img, { brightness = 0, contrast = 0, mirror = false } = {}) {
  let o = brightness || contrast ? adjust(img, brightness, contrast) : img;
  if (mirror) o = flip(o, true, false);
  return o;
}

export { lum };
