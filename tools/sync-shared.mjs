// Copies shared/*.json (except test vectors) into web/shared so the web app can fetch them.
import fs from 'node:fs';
import path from 'node:path';
import { ROOT } from './shared.mjs';
const src = path.join(ROOT, 'shared'), dst = path.join(ROOT, 'web/shared');
fs.mkdirSync(dst, { recursive: true });
for (const f of fs.readdirSync(src)) {
  if (!f.endsWith('.json') || f === 'test-vectors.json') continue;
  fs.copyFileSync(path.join(src, f), path.join(dst, f));
}
console.log('synced shared/ -> web/shared/');
