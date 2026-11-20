// Rasterizes the app icon to PNG sizes (simple software renderer, no deps).
import fs from 'node:fs';
import path from 'node:path';
import { encodePNG } from './png.mjs';
import { ROOT } from './shared.mjs';

function icon(size, maskable) {
  const img = { width: size, height: size, data: new Uint8ClampedArray(size * size * 4) };
  const S = size / 512, pad = maskable ? 0 : 0;
  const bezier = (t) => { // cubic approximating the brand stroke
    const p0 = [110, 380], p1 = [180, 368], p2 = [300, 120], p3 = [412, 94];
    const u = 1 - t;
    return [0, 1].map((k) => u * u * u * p0[k] + 3 * u * u * t * p1[k] + 3 * u * t * t * p2[k] + t * t * t * p3[k]);
  };
  const curve = Array.from({ length: 200 }, (_, i) => bezier(i / 199));
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const X = x / S, Y = y / S;
    const r = maskable ? 0 : 112;
    const inRect = (() => { const cx = Math.min(Math.max(X, r), 512 - r), cy = Math.min(Math.max(Y, r), 512 - r); return Math.hypot(X - cx, Y - cy) <= r; })();
    let c = inRect ? [47, 116, 102, 255] : [0, 0, 0, 0];
    const sc = maskable ? 0.72 : 1, MX = 256 + (X - 256) / sc, MY = 256 + (Y - 256) / sc;
    let dmin = 1e9; for (const [px, py] of curve) dmin = Math.min(dmin, Math.hypot(MX - px, MY - py));
    if (inRect && dmin < 17) c = [255, 255, 255, 255];
    if (inRect && Math.hypot(MX - 126, MY - 372) < 30) c = [246, 243, 237, 255];
    img.data.set(c, (y * size + x) * 4);
  }
  return img;
}
for (const [name, size, m] of [['icon-192.png', 192, false], ['icon-512.png', 512, false], ['icon-maskable-512.png', 512, true]]) {
  fs.writeFileSync(path.join(ROOT, 'web/icons', name), encodePNG(icon(size, m)));
}
console.log('icons written');
