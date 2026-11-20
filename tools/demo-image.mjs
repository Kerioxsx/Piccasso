// Procedurally painted still life (pear + mug on a table) used as the built-in
// demo project. Deterministic, no external assets, no copyright concerns.
import { makeRGBA } from '../web/js/core/image.js';

export function paintDemo(W = 640, H = 480) {
  const img = makeRGBA(W, H);
  const L = [-0.6, -0.7, 0.6]; // light from upper left, towards viewer
  const ln = Math.hypot(...L); const lx = L[0] / ln, ly = L[1] / ln, lz = L[2] / ln;
  const tableY = H * 0.68;
  const pear = { cx: W * 0.36, cy: H * 0.56, r: H * 0.17 };
  const pearTop = { cx: W * 0.355, cy: H * 0.38, r: H * 0.095 };
  const mug = { x0: W * 0.58, x1: W * 0.8, y0: H * 0.36, y1: H * 0.7 };
  const set = (x, y, r, g, b) => { const i = (y * W + x) * 4; img.data[i] = r; img.data[i + 1] = g; img.data[i + 2] = b; img.data[i + 3] = 255; };
  const sphere = (x, y, s) => {
    const dx = (x - s.cx) / s.r, dy = (y - s.cy) / s.r, d2 = dx * dx + dy * dy;
    if (d2 > 1) return null;
    const dz = Math.sqrt(1 - d2);
    return Math.max(0, dx * lx + dy * ly + dz * lz);
  };
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    // wall / table
    let r, g, b;
    if (y < tableY) { const t = y / tableY; r = 236 - 10 * t; g = 230 - 10 * t; b = 218 - 8 * t; }
    else { const t = (y - tableY) / (H - tableY); r = 196 - 30 * t; g = 168 - 28 * t; b = 132 - 24 * t; }
    // cast shadows on the table (to the lower right of objects)
    const sh1 = ((x - (pear.cx + 40)) / (pear.r * 1.25)) ** 2 + ((y - (tableY + 10)) / (pear.r * 0.32)) ** 2;
    const sh2 = ((x - ((mug.x0 + mug.x1) / 2 + 45)) / ((mug.x1 - mug.x0) * 0.75)) ** 2 + ((y - (tableY + 6)) / 20) ** 2;
    if (y > tableY - 4 && (sh1 < 1 || sh2 < 1)) { r *= 0.7; g *= 0.7; b *= 0.72; }
    // mug body (cylinder shading) + handle
    const inBody = x >= mug.x0 && x <= mug.x1 && y >= mug.y0 && y <= mug.y1;
    const hx = (x - mug.x1) / 34, hy = (y - (mug.y0 + mug.y1) / 2) / 48, hd = Math.hypot(hx, hy);
    const inHandle = x > mug.x1 - 4 && hd < 1 && hd > 0.58;
    if (inBody || inHandle) {
      const u = (x - mug.x0) / (mug.x1 - mug.x0) * 2 - 1;
      const shade = inBody ? Math.max(0.25, Math.min(1, 0.95 - 0.55 * (u + 0.35) ** 2)) : 0.45;
      r = 70 * shade + 20; g = 110 * shade + 25; b = 170 * shade + 30;
      if (inBody && y < mug.y0 + 10) { r = 60; g = 80; b = 120; } // rim
    }
    // pear: two blended spheres
    const s1 = sphere(x, y, pear), s2 = sphere(x, y, pearTop);
    const s = s1 != null && s2 != null ? Math.max(s1, s2) : s1 != null ? s1 : s2;
    if (s != null) {
      const k = 0.25 + 0.75 * s;
      r = 120 * k + 40; g = 160 * k + 35; b = 40 * k + 10;
      if (s > 0.97) { r = 245; g = 245; b = 215; } // highlight
    }
    // stem
    if (x > pearTop.cx - 3 && x < pearTop.cx + 3 && y > pearTop.cy - pearTop.r - 26 && y < pearTop.cy - pearTop.r + 6) { r = 80; g = 55; b = 30; }
    set(x, y, Math.round(r), Math.round(g), Math.round(b));
  }
  return img;
}
