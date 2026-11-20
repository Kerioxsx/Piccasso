// End-to-end processing: source image + prep → analysis, steps and renders.
import { prepareImage } from './image.js';
import { analyze } from './analysis.js';
import { generateSteps } from './steps.js';
import { renderStyle } from './render.js';

export const STEP_BASES = ['light', 'outline', 'values', 'sketch'];

/** Analysis + steps only (fast; analysis runs at shared.difficulty.analysisMaxDim). */
export function planProject(src, prep, shared) {
  const level = shared.difficulty.levels[prep.difficulty] || shared.difficulty.levels.easy;
  const small = prepareImage(src, prep, shared.difficulty.analysisMaxDim);
  const analysis = analyze(small, level, { excluded: prep.excludedObjects, isolate: prep.isolateObject });
  const steps = generateSteps(analysis, prep, shared);
  return { analysis, steps };
}

export function keepBoxesFor(analysis, prep) {
  if (!analysis || (!prep.isolateObject && !(prep.excludedObjects || []).length)) return null;
  const act = new Set(analysis.activeObjects);
  const boxes = analysis.objects.filter((o) => act.has(o.id)).map((o) => o.bbox);
  return boxes.length ? boxes : null;
}

/** Render one style/base at up to renderMaxDim. `prepared` may be passed to avoid re-preparing. */
export function renderFor(src, prep, shared, style, renderMaxDim, analysis, prepared) {
  const level = shared.difficulty.levels[prep.difficulty] || shared.difficulty.levels.easy;
  const img = prepared || prepareImage(src, prep, renderMaxDim);
  return renderStyle(img, style, {
    level, analysisMaxDim: shared.difficulty.analysisMaxDim,
    lineThickness: prep.lineThickness, sketchContrast: prep.sketchContrast,
    simplifyBackground: prep.simplifyBackground, keepBoxes: keepBoxesFor(analysis, prep),
  });
}

/**
 * Full run with progress callback and cooperative cancellation.
 * @param onProgress (fraction, label)
 * @param isCancelled () => boolean
 */
export function runPipeline(src, prep, shared, { renderMaxDim = 1024, onProgress = () => {}, isCancelled = () => false } = {}) {
  const check = () => { if (isCancelled()) throw new Error('cancelled'); };
  onProgress(0.05, 'Preparing image');
  const prepared = prepareImage(src, prep, renderMaxDim);
  check();
  onProgress(0.15, 'Finding the main shapes');
  const { analysis, steps } = planProject(src, prep, shared);
  check();
  const renders = {};
  const wanted = [prep.style, ...STEP_BASES.filter((b) => b !== prep.style)];
  wanted.forEach((style, i) => {
    onProgress(0.3 + (0.65 * i) / wanted.length, `Drawing the ${style.replace(/_/g, ' ')} guide`);
    renders[style] = renderFor(src, prep, shared, style, renderMaxDim, analysis, prepared);
    check();
  });
  onProgress(1, 'Done');
  return { analysis, steps, renders };
}
