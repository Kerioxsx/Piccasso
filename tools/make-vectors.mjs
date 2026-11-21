import fs from 'node:fs';
import path from 'node:path';
import { computeVectors } from './vectors.mjs';
import { loadShared, ROOT } from './shared.mjs';
fs.writeFileSync(path.join(ROOT, 'shared/test-vectors.json'), JSON.stringify(computeVectors(loadShared()), null, 1));
console.log('wrote shared/test-vectors.json');
