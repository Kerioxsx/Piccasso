// Writes every style + an analysis overlay for an image to /tmp-ish folder for visual inspection.
// Usage: node tools/debug-render.mjs <outDir> [difficulty]
import fs from 'node:fs';
import path from 'node:path';
import { encodePNG } from './png.mjs';
import { paintDemo } from './demo-image.mjs';
import { loadShared } from './shared.mjs';
import { defaultPrep } from '../web/js/core/project.js';
import { planProject, renderFor } from '../web/js/core/pipeline.js';
import { prepareImage } from '../web/js/core/image.js';
import { rasterPolyline } from '../web/js/core/geometry.js';

const out = process.argv[2]; const diff = process.argv[3] || 'easy';
fs.mkdirSync(out, { recursive: true });
const shared = loadShared();
const src = paintDemo();
const prep = { ...defaultPrep(), difficulty: diff };
const { analysis } = planProject(src, prep, shared);
for (const s of shared.styles.styles) {
  const r = renderFor(src, prep, shared, s.id, 640, analysis);
  fs.writeFileSync(path.join(out, `${s.id}.png`), encodePNG(r));
}
const base = prepareImage(src, prep, 640);
const W = base.width, H = base.height;
const paint = (pts, closed, col) => {
  const m = new Uint8Array(W * H);
  rasterPolyline(m, W, H, pts.map(([x, y]) => [x * W, y * H]), closed, 1);
  for (let i = 0; i < m.length; i++) if (m[i]) base.data.set(col, i * 4);
};
for (const s of analysis.shapes) {
  const pts = [];
  for (let k = 0; k < 48; k++) {
    const a = (k / 48) * 2 * Math.PI, t = (s.angle * Math.PI) / 180;
    const ex = Math.cos(a) * s.rx * W, ey = Math.sin(a) * s.ry * W;
    pts.push([(s.cx * W + ex * Math.cos(t) - ey * Math.sin(t)) / W, (s.cy * H + ex * Math.sin(t) + ey * Math.cos(t)) / H]);
  }
  paint(pts, true, [255, 0, 255, 255]);
}
for (const c of analysis.silhouettes) paint(c.points, true, [255, 0, 0, 255]);
for (const c of analysis.internals) paint(c.points, true, c.tone === 'dark' ? [0, 0, 255, 255] : [0, 200, 0, 255]);
const b = analysis.subject.bbox; paint([[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]], true, [255, 140, 0, 255]);
fs.writeFileSync(path.join(out, `overlay.png`), encodePNG(base));
console.log(JSON.stringify({ subject: analysis.subject, objects: analysis.objects, values: analysis.values, detail: analysis.detail }, null, 1));
