// Drawing-style renderers. Each turns a prepared RGBA image into one of the
// preparation styles from shared/styles.json. Mirrored in
// rust/crates/piccasso-core/src/render.rs.

import {
  makeRGBA, toGray, grayToRGBA, blur2, percentileThreshold,
  dilate, cleanMask, colorSobel, labelComponents, posterizeLevels, foregroundFromBorder,
} from './image.js';

const INK = 25;

/** Graphite "colour dodge" sketch. contrast in -50..100. Returns gray data. */
export function pencilGray(g, contrast, strength = 1) {
  const W = g.width, H = g.height;
  const inv = { width: W, height: H, data: new Uint8Array(W * H) };
  for (let i = 0; i < inv.data.length; i++) inv.data[i] = 255 - g.data[i];
  const bl = blur2(inv, Math.max(2, Math.round(Math.max(W, H) / 60)));
  const k = Math.max(0, 1 + contrast / 50) * strength;
  const out = { width: W, height: H, data: new Uint8Array(W * H) };
  for (let i = 0; i < out.data.length; i++) {
    const b = bl.data[i];
    const s = b >= 255 ? 255 : Math.min(255, Math.floor((g.data[i] * 255) / (255 - b)));
    // keep some of the original tone so the sketch isn't only outlines
    const tone = 255 - Math.floor(((255 - g.data[i]) * 45) / 100);
    const st = Math.floor((s * tone) / 255);
    out.data[i] = 255 - Math.min(255, Math.round((255 - st) * k));
  }
  return out;
}

/** Subject mask at the image's own resolution (same rules as analysis). */
export function foregroundMask(img, level, scale) {
  const W = img.width, H = img.height, N = W * H;
  const fg = foregroundFromBorder(img, Math.max(1, Math.round(level.blurRadius * scale)));
  if (fg.separation < 18) return null;
  const m = cleanMask(fg.mask, W, H);
  const { labels, comps } = labelComponents(m, W, H);
  const minArea = Math.max(4, Math.floor(level.minAreaFrac * N));
  const keep = new Uint8Array(comps.length + 1);
  let any = false;
  for (const c of comps) if (c.area >= minArea && !(c.borders >= 3 && c.area > 0.5 * N)) { keep[c.label] = 1; any = true; }
  const out = new Uint8Array(N);
  if (!any) return null;
  for (let i = 0; i < N; i++) out[i] = keep[labels[i]];
  return out;
}

function boundary(mask, W, H) {
  const out = new Uint8Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x;
    if (!mask[i]) continue;
    if (x === 0 || y === 0 || x === W - 1 || y === H - 1 || !mask[i - 1] || !mask[i + 1] || !mask[i - W] || !mask[i + W]) out[i] = 1;
  }
  return out;
}

function maskToInk(mask, W, H) {
  const out = makeRGBA(W, H);
  for (let i = 0, j = 0; i < mask.length; i++, j += 4) {
    const v = mask[i] ? INK : 255;
    out.data[j] = v; out.data[j + 1] = v; out.data[j + 2] = v; out.data[j + 3] = 255;
  }
  return out;
}

/**
 * @param img    prepared RGBA image
 * @param style  style id (see shared/styles.json) or a step base: light|outline|values|sketch|none
 * @param opt    { level, analysisMaxDim, lineThickness(1..5), sketchContrast(-50..100),
 *                 simplifyBackground, keepBoxes: [[x0,y0,x1,y1]...] | null }
 */
export function renderStyle(img, style, opt) {
  const level = opt.level;
  const W = img.width, H = img.height, N = W * H;
  const scale = Math.max(W, H) / (opt.analysisMaxDim || 320);
  const rs = (r) => Math.max(1, Math.round(r * scale));
  const thick = Math.max(0, (opt.lineThickness || 2) - 1);
  const contrast = opt.sketchContrast == null ? 20 : opt.sketchContrast;
  const g = toGray(img);
  let out;
  switch (style) {
    case 'none': {
      out = makeRGBA(W, H); out.data.fill(255); break;
    }
    case 'original': {
      out = makeRGBA(W, H); out.data.set(img.data); break;
    }
    case 'pencil_sketch': case 'sketch':
      out = grayToRGBA(pencilGray(g, contrast)); break;
    case 'light_sketch': case 'light':
      out = grayToRGBA(pencilGray(g, contrast, 0.4)); break;
    case 'basic_shapes':
      out = grayToRGBA(pencilGray(g, contrast, 0.25)); break;
    case 'clean_outline': case 'outline': {
      const e = colorSobel(img, rs(level.blurRadius));
      const t = percentileThreshold(e, level.edgeFraction, level.minEdge);
      const m = new Uint8Array(N);
      for (let i = 0; i < N; i++) m[i] = e.data[i] >= t ? 1 : 0;
      out = maskToInk(dilate(m, W, H, thick), W, H);
      break;
    }
    case 'simplified_outline': {
      const lv = posterizeLevels(blur2(g, rs(level.blurRadius * 2 + 1)), level.valueLevels);
      const m = new Uint8Array(N);
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const i = y * W + x;
        if ((x < W - 1 && lv[i] !== lv[i + 1]) || (y < H - 1 && lv[i] !== lv[i + W])) m[i] = 1;
      }
      out = maskToInk(dilate(m, W, H, thick), W, H);
      break;
    }
    case 'contour_only': {
      const fg = foregroundMask(img, level, scale);
      out = fg ? maskToInk(dilate(boundary(fg, W, H), W, H, thick), W, H)
               : renderStyle(img, 'clean_outline', opt);
      break;
    }
    case 'shading_guide': case 'values': {
      const L = level.valueLevels;
      const lv = posterizeLevels(blur2(g, rs(level.blurRadius * 2 + 1)), L);
      out = makeRGBA(W, H);
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const i = y * W + x;
        let v = 60 + Math.round((lv[i] * 195) / (L - 1));
        if ((x < W - 1 && lv[i] !== lv[i + 1]) || (y < H - 1 && lv[i] !== lv[i + W])) v = Math.max(0, v - 60);
        const j = i * 4;
        out.data[j] = v; out.data[j + 1] = v; out.data[j + 2] = v; out.data[j + 3] = 255;
      }
      break;
    }
    default:
      throw new Error(`Unknown style: ${style}`);
  }
  if (style !== 'none' && (opt.simplifyBackground || opt.keepBoxes)) applyBackground(out, img, g, opt, scale, style === 'original');
  return out;
}

function applyBackground(out, img, g, opt, scale, soft) {
  const W = img.width, H = img.height, N = W * H;
  let keep = new Uint8Array(N).fill(1);
  if (opt.simplifyBackground) {
    const fg = foregroundMask(img, opt.level, scale);
    if (fg) keep = dilate(fg, W, H, Math.max(1, Math.round(Math.max(W, H) * 0.02)));
  }
  if (opt.keepBoxes && opt.keepBoxes.length) {
    const boxMask = new Uint8Array(N);
    for (const [x0, y0, x1, y1] of opt.keepBoxes) {
      const ax = Math.max(0, Math.floor((x0 - 0.03) * W)), ay = Math.max(0, Math.floor((y0 - 0.03) * H));
      const bx = Math.min(W, Math.ceil((x1 + 0.03) * W)), by = Math.min(H, Math.ceil((y1 + 0.03) * H));
      for (let y = ay; y < by; y++) for (let x = ax; x < bx; x++) boxMask[y * W + x] = 1;
    }
    for (let i = 0; i < N; i++) keep[i] = keep[i] & boxMask[i];
  }
  for (let i = 0; i < N; i++) {
    if (keep[i]) continue;
    const j = i * 4;
    for (let c = 0; c < 3; c++) out.data[j + c] = soft ? Math.round(out.data[j + c] * 0.15 + 255 * 0.85) : 255;
  }
}
