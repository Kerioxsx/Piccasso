import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const j = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, 'shared', f), 'utf8'));
export function loadShared(lang = 'en') {
  return {
    difficulty: j('difficulty.json'), styles: j('styles.json'), paper: j('paper.json'),
    stepKinds: j('step-kinds.json'), pressure: j('pressure.json'), colors: j('colors.json'), strings: j(`strings.${lang}.json`), glossary: j(`glossary.${lang}.json`),
  };
}
