// Drawing analysis without AI: value statistics, histogram comparison with
// plain-language advice, symmetry, measurements and colour sampling.
import { toGray, histogram, resizeBox, fitDims } from './image.js';

/** Value statistics of a picture: mean, contrast (std dev), dark/light ends, 5 value bands. */
export function valueStats(img) {
  const g = img.data.length === img.width * img.height ? img : toGray(img);
  const hist = histogram(g), n = g.data.length;
  let mean = 0; for (let v = 0; v < 256; v++) mean += v * hist[v]; mean /= n;
  let varr = 0; for (let v = 0; v < 256; v++) varr += hist[v] * (v - mean) ** 2;
  const pct = (q) => { let acc = 0; for (let v = 0; v < 256; v++) { acc += hist[v]; if (acc >= q * n) return v; } return 255; };
  const bands = [0, 0, 0, 0, 0];
  for (let v = 0; v < 256; v++) bands[Math.min(4, Math.floor(v / 51.2))] += hist[v] / n;
  return { mean, contrast: Math.sqrt(varr / n), dark: pct(0.02), light: pct(0.98), bands, hist };
}

const BAND_NAMES = ['darkest darks', 'dark values', 'middle values', 'light values', 'lightest lights'];

/** Compare a drawing with its reference and say what to change (deterministic rules). */
export function compareValues(ref, drw) {
  const tips = [];
  if (drw.dark - ref.dark > 25) tips.push({ level: 'big', text: `Your darkest darks are lighter than the reference (${drw.dark} vs ${ref.dark} of 255). Use a softer pencil (4B–6B) or more layers in the darkest areas.` });
  if (ref.light - drw.light > 25) tips.push({ level: 'big', text: 'Your lightest areas are greyer than the reference. Keep the paper white there, or lift graphite with a kneaded eraser.' });
  if (ref.contrast - drw.contrast > 12) tips.push({ level: 'big', text: `Your drawing has less contrast (${Math.round(drw.contrast)} vs ${Math.round(ref.contrast)}). Push the darks darker and keep the lights clean.` });
  else if (drw.contrast - ref.contrast > 15) tips.push({ level: 'small', text: 'Your drawing is more contrasty than the reference. Soften the middle values to get the gentle transitions back.' });
  if (Math.abs(drw.mean - ref.mean) > 20) tips.push({ level: 'small', text: drw.mean > ref.mean ? 'Overall your drawing is lighter than the reference.' : 'Overall your drawing is darker than the reference.' });
  ref.bands.forEach((r, i) => {
    const d = drw.bands[i] - r;
    if (Math.abs(d) > 0.12) tips.push({ level: 'small', text: `${d > 0 ? 'More' : 'Less'} ${BAND_NAMES[i]} than the reference (${Math.round(drw.bands[i] * 100)} % vs ${Math.round(r * 100)} % of the picture).` });
  });
  if (!tips.length) tips.push({ level: 'good', text: 'The values match the reference closely. Nice work!' });
  return tips;
}

/**
 * Left/right symmetry about a vertical axis at `axis` (0..1 of the width):
 * score 0..1 (1 = perfectly mirrored) and a difference map (0..255 per pixel, left half).
 */
export function symmetry(img, axis = 0.5, maxDim = 400) {
  const [w, h] = fitDims(img.width, img.height, maxDim);
  const g = toGray(w === img.width ? img : resizeBox(img, w, h));
  const ax = axis * w, diff = new Uint8Array(w * h);
  let sum = 0, n = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < Math.floor(ax); x++) {
    const xm = Math.round(2 * ax - x - 1);
    if (xm < 0 || xm >= w) continue;
    const d = Math.abs(g.data[y * w + x] - g.data[y * w + xm]);
    diff[y * w + x] = d; sum += d; n++;
  }
  return { score: n ? 1 - sum / n / 255 : 1, diff, width: w, height: h };
}

/** Distance between two points as a fraction of the picture width, and in cm if the paper width is known. */
export function measure(a, b, imgWidth, paperWidthCm = null) {
  const px = Math.hypot(b[0] - a[0], b[1] - a[1]);
  return { px, ofWidth: px / imgWidth, cm: paperWidthCm ? (px / imgWidth) * paperWidthCm : null };
}

/** Colour at a pixel: hex, RGB and its value (0 = black … 100 = white). */
export function sample(img, x, y) {
  const xi = Math.max(0, Math.min(img.width - 1, Math.round(x))), yi = Math.max(0, Math.min(img.height - 1, Math.round(y)));
  const i = (yi * img.width + xi) * 4, [r, g, b] = [img.data[i], img.data[i + 1], img.data[i + 2]];
  const value = Math.round(((77 * r + 150 * g + 29 * b) / 256 / 255) * 100);
  return { r, g, b, hex: '#' + [r, g, b].map((c) => c.toString(16).padStart(2, '0')).join(''), value };
}
