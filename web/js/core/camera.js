// Camera helpers without AI: lighting/focus hints from simple image statistics,
// and a classical skin-colour mask so your hand can be shown in front of the guide.

/** Sharpness: variance of the 4-neighbour Laplacian (higher = sharper). */
export function sharpness(gray) {
  const { width: w, height: h, data: d } = gray;
  let sum = 0, sum2 = 0, n = 0;
  for (let y = 1; y < h - 1; y++) for (let x = 1; x < w - 1; x++) {
    const i = y * w + x;
    const l = 4 * d[i] - d[i - 1] - d[i + 1] - d[i - w] - d[i + w];
    sum += l; sum2 += l * l; n++;
  }
  return n ? sum2 / n - (sum / n) ** 2 : 0;
}

/** Brightness statistics of a grey frame: mean, contrast (std dev), glare share, left/right/top/bottom balance. */
export function frameStats(gray) {
  const { width: w, height: h, data: d } = gray;
  let s = 0, s2 = 0, glare = 0;
  const half = [0, 0, 0, 0], cnt = [0, 0, 0, 0]; // left, right, top, bottom
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const v = d[y * w + x];
    s += v; s2 += v * v; if (v >= 250) glare++;
    const lr = x < w / 2 ? 0 : 1, tb = y < h / 2 ? 2 : 3;
    half[lr] += v; cnt[lr]++; half[tb] += v; cnt[tb]++;
  }
  const n = w * h || 1, mean = s / n;
  const m = half.map((v, i) => v / (cnt[i] || 1));
  return { mean, contrast: Math.sqrt(Math.max(0, s2 / n - mean * mean)), glare: glare / n, unevenX: Math.abs(m[0] - m[1]), unevenY: Math.abs(m[2] - m[3]), sharpness: sharpness(gray) };
}

/** Plain-language hints for the camera picture, most important first. Empty = looks fine. */
export function lightingHints(st) {
  const t = [];
  if (st.mean < 55) t.push({ id: 'dark', text: 'It\'s dark. Add a lamp or move to a brighter spot.' });
  else if (st.mean > 230) t.push({ id: 'bright', text: 'The picture is washed out. Move away from direct light.' });
  if (st.glare > 0.04) t.push({ id: 'glare', text: 'There is glare on the paper. Tilt the lamp or the phone a little.' });
  if (Math.max(st.unevenX, st.unevenY) > 45) t.push({ id: 'uneven', text: `One side is much darker (${st.unevenX >= st.unevenY ? 'left/right' : 'top/bottom'}). Light the paper evenly so the guide stays visible.` });
  if (st.sharpness < 25 && st.contrast > 12) t.push({ id: 'blur', text: 'The picture looks blurry. Hold the phone still, or tap the paper on your screen to focus.' });
  return t;
}

/**
 * Skin-colour mask (YCbCr box, a classic rule) with a light clean-up: pixels need
 * enough skin-coloured neighbours, which drops speckles. Returns { width, height, data: 0/255 }.
 */
export function skinMask(rgba, { minNeighbours = 5 } = {}) {
  const { width: w, height: h, data: d } = rgba;
  const raw = new Uint8Array(w * h);
  for (let i = 0, p = 0; p < w * h; p++, i += 4) {
    const r = d[i], g = d[i + 1], b = d[i + 2];
    const y = 0.299 * r + 0.587 * g + 0.114 * b;
    const cb = 128 - 0.168736 * r - 0.331264 * g + 0.5 * b;
    const cr = 128 + 0.5 * r - 0.418688 * g - 0.081312 * b;
    raw[p] = y > 45 && cb >= 77 && cb <= 127 && cr >= 137 && cr <= 177 && r > g && r > b ? 1 : 0;
  }
  const out = new Uint8Array(w * h);
  let count = 0;
  for (let y = 1; y < h - 1; y++) for (let x = 1; x < w - 1; x++) {
    const p = y * w + x;
    if (!raw[p]) continue;
    let nb = 0;
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) nb += raw[p + dy * w + dx];
    if (nb - 1 >= minNeighbours) { out[p] = 255; count++; }
  }
  return { width: w, height: h, data: out, share: count / (w * h || 1) };
}

/** Grow a 0/255 mask by r pixels (square), so the hand cut-out covers its edges. */
export function dilate(mask, r = 1) {
  const { width: w, height: h, data } = mask;
  let src = data;
  for (let k = 0; k < r; k++) {
    const dst = new Uint8Array(w * h);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      if (src[y * w + x]) { for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) { const yy = y + dy, xx = x + dx; if (yy >= 0 && yy < h && xx >= 0 && xx < w) dst[yy * w + xx] = 255; } }
    }
    src = dst;
  }
  return { ...mask, data: src };
}
