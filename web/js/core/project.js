// DrawStep portable project format, version 1.
// The schema lives in schema/drawstep-project.v1.schema.json; the Rust app
// reads and writes the same structure (rust/crates/drawstep-core/src/project.rs).

export const FORMAT = 'drawstep-project';
export const VERSION = 1;
export const STYLES = ['original', 'clean_outline', 'simplified_outline', 'pencil_sketch', 'light_sketch', 'contour_only', 'shading_guide', 'basic_shapes'];
export const DIFFICULTIES = ['very_easy', 'easy', 'intermediate', 'detailed'];

export function uuid() {
  if (globalThis.crypto && globalThis.crypto.randomUUID) return globalThis.crypto.randomUUID();
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === 'x' ? r : (r & 3) | 8).toString(16);
  });
}

export function defaultPrep() {
  return {
    crop: { x: 0, y: 0, w: 1, h: 1 }, rotation: 0, flipH: false, flipV: false,
    brightness: 0, contrast: 0,
    style: 'pencil_sketch', difficulty: 'easy',
    lineThickness: 2, sketchContrast: 20, opacity: 100,
    simplifyBackground: false, excludedObjects: [], isolateObject: null,
    paper: { preset: 'a4', orientation: 'auto', marginMm: 15 },
  };
}
export const defaultProgress = () => ({ currentStep: 0, completed: [], skipped: [], timeSpentSec: 0 });
export const defaultView = () => ({ zoom: 1, panX: 0, panY: 0, brightness: 100, contrast: 100, opacity: 100, fit: 'page' });
export const defaultAr = () => ({ opacity: 0.5, scale: 1, rotation: 0, offsetX: 0, offsetY: 0, corners: null, layer: 'outline', alignmentLocked: false });

export function createProject({ name, source, app = { app: 'drawstep-web', version: '0.1.0' } }) {
  const now = new Date().toISOString();
  return {
    format: FORMAT, version: VERSION, id: uuid(), name: name || 'Untitled drawing',
    createdAt: now, updatedAt: now, createdWith: app,
    source, prep: defaultPrep(), analysis: null, steps: [],
    progress: defaultProgress(), view: defaultView(), ar: defaultAr(),
  };
}

const isObj = (v) => v && typeof v === 'object' && !Array.isArray(v);

/** Structural validation. Returns { ok, errors[] }. */
export function validateProject(p) {
  const e = [];
  if (!isObj(p)) return { ok: false, errors: ['Project must be a JSON object.'] };
  if (p.format !== FORMAT) e.push(`format must be "${FORMAT}".`);
  if (!Number.isInteger(p.version) || p.version < 1) e.push('version must be a positive integer.');
  else if (p.version > VERSION) e.push(`This project was made with a newer DrawStep (format v${p.version}). Please update the app.`);
  if (typeof p.id !== 'string' || !p.id) e.push('id is required.');
  if (typeof p.name !== 'string') e.push('name must be a string.');
  if (!isObj(p.source)) e.push('source is required.');
  else {
    if (typeof p.source.data !== 'string' || !p.source.data) e.push('source.data (base64 image) is required.');
    if (!/^image\/(png|jpeg|webp|gif|bmp)$/.test(p.source.mime || '')) e.push('source.mime must be an image type.');
    if (!(p.source.width > 0) || !(p.source.height > 0)) e.push('source.width/height must be positive.');
  }
  if (p.prep !== undefined) {
    if (!isObj(p.prep)) e.push('prep must be an object.');
    else {
      if (p.prep.style !== undefined && !STYLES.includes(p.prep.style)) e.push(`prep.style "${p.prep.style}" is unknown.`);
      if (p.prep.difficulty !== undefined && !DIFFICULTIES.includes(p.prep.difficulty)) e.push(`prep.difficulty "${p.prep.difficulty}" is unknown.`);
      if (p.prep.rotation !== undefined && ![0, 90, 180, 270].includes(p.prep.rotation)) e.push('prep.rotation must be 0, 90, 180 or 270.');
    }
  }
  if (p.steps !== undefined && !Array.isArray(p.steps)) e.push('steps must be an array.');
  if (Array.isArray(p.steps)) p.steps.forEach((s, i) => {
    if (!isObj(s) || typeof s.id !== 'string' || typeof s.kind !== 'string' || typeof s.instruction !== 'string') e.push(`steps[${i}] needs id, kind and instruction.`);
  });
  return { ok: e.length === 0, errors: e };
}

/** Fill in defaults for optional sections so older/partial files load cleanly. */
export function normalizeProject(p) {
  const out = { ...p };
  out.prep = { ...defaultPrep(), ...(p.prep || {}) };
  out.prep.crop = { ...defaultPrep().crop, ...(p.prep && p.prep.crop) };
  out.prep.paper = { ...defaultPrep().paper, ...(p.prep && p.prep.paper) };
  out.progress = { ...defaultProgress(), ...(p.progress || {}) };
  out.view = { ...defaultView(), ...(p.view || {}) };
  out.ar = { ...defaultAr(), ...(p.ar || {}) };
  out.steps = Array.isArray(p.steps) ? p.steps : [];
  out.analysis = p.analysis || null;
  out.createdAt = p.createdAt || new Date().toISOString();
  out.updatedAt = p.updatedAt || out.createdAt;
  const max = Math.max(0, out.steps.length - 1);
  out.progress.currentStep = Math.min(Math.max(0, out.progress.currentStep | 0), max);
  return out;
}

/** Portable export: strips runtime-only keys (prefixed with "_"). */
export function exportProject(p) {
  return JSON.stringify(p, (k, v) => (k.startsWith('_') ? undefined : v), 2);
}

/** Parse + validate + normalize. Throws Error with a readable message. */
export function importProject(text, { newId = false } = {}) {
  let obj;
  try { obj = JSON.parse(text); } catch { throw new Error('This file is not valid JSON.'); }
  const v = validateProject(obj);
  if (!v.ok) throw new Error('This is not a valid DrawStep project:\n• ' + v.errors.join('\n• '));
  const p = normalizeProject(obj);
  if (newId) p.id = uuid();
  return p;
}

/** Step navigation helpers (pure). */
export function gotoStep(p, index) {
  const max = Math.max(0, p.steps.length - 1);
  return { ...p, progress: { ...p.progress, currentStep: Math.min(Math.max(0, index), max) } };
}
export function completeStep(p, { skipped = false } = {}) {
  const step = p.steps[p.progress.currentStep];
  if (!step) return p;
  const completed = p.progress.completed.filter((id) => id !== step.id);
  const skippedL = p.progress.skipped.filter((id) => id !== step.id);
  if (skipped) skippedL.push(step.id); else completed.push(step.id);
  return gotoStep({ ...p, progress: { ...p.progress, completed, skipped: skippedL } }, p.progress.currentStep + 1);
}
export function restartTutorial(p) {
  return { ...p, progress: { ...defaultProgress(), timeSpentSec: p.progress.timeSpentSec } };
}
