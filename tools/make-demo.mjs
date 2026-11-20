// Builds samples/demo-project.drawstep.json (and the web copy) from the
// procedural demo image, running the real pipeline.
import fs from 'node:fs';
import path from 'node:path';
import { encodePNG } from './png.mjs';
import { paintDemo } from './demo-image.mjs';
import { loadShared, ROOT } from './shared.mjs';
import { createProject } from '../web/js/core/project.js';
import { planProject } from '../web/js/core/pipeline.js';

const shared = loadShared();
const img = paintDemo();
const png = encodePNG(img);
const p = createProject({
  name: 'Demo: pear and mug',
  source: { mime: 'image/png', width: img.width, height: img.height, data: png.toString('base64') },
  app: { app: 'drawstep-tools', version: '0.1.0' },
});
p.id = 'demo-pear-and-mug';
p.createdAt = p.updatedAt = '2026-11-20T00:00:00.000Z';
const { analysis, steps } = planProject(img, p.prep, shared);
p.analysis = analysis;
p.steps = steps;
const json = JSON.stringify(p, null, 2);
for (const out of ['samples/demo-project.drawstep.json', 'web/demo/demo-project.drawstep.json']) {
  fs.mkdirSync(path.dirname(path.join(ROOT, out)), { recursive: true });
  fs.writeFileSync(path.join(ROOT, out), json);
}
fs.writeFileSync(path.join(ROOT, 'samples/demo-image.png'), png);
console.log(`demo: ${img.width}x${img.height}, ${steps.length} steps, ${analysis.objects.length} objects, ${analysis.shapes.length} shapes, lowContrast=${analysis.lowContrast}`);
for (const s of steps) console.log(`- ${s.title}: ${s.instruction}`);
