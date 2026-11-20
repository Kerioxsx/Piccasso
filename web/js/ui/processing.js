// Runs the core pipeline in a Web Worker (responsive UI + instant cancel).
// Falls back to the main thread if workers are unavailable.
import { runPipeline, renderFor } from '../core/pipeline.js';

let seq = 0;

function startWorker() {
  try { return new Worker(new URL('../worker.js', import.meta.url), { type: 'module' }); } catch { return null; }
}

/**
 * @returns {{ promise: Promise<{analysis, steps, renders}>, cancel: () => void }}
 */
export function runInBackground(src, prep, shared, { renderMaxDim = 1024, onProgress = () => {} } = {}) {
  const worker = startWorker();
  let cancelled = false;
  if (!worker) {
    const promise = new Promise((resolve, reject) => setTimeout(() => {
      try { resolve(runPipeline(src, prep, shared, { renderMaxDim, onProgress, isCancelled: () => cancelled })); } catch (e) { reject(e); }
    }, 30));
    return { promise, cancel: () => { cancelled = true; } };
  }
  const id = ++seq;
  let rejectFn;
  const promise = new Promise((resolve, reject) => {
    rejectFn = reject;
    worker.onmessage = (ev) => {
      const m = ev.data;
      if (m.id !== id) return;
      if (m.type === 'progress') onProgress(m.p, m.label);
      else if (m.type === 'done') {
        worker.terminate();
        const renders = {};
        for (const [k, r] of Object.entries(m.renders)) renders[k] = { width: r.width, height: r.height, data: new Uint8ClampedArray(r.data) };
        resolve({ analysis: m.analysis, steps: m.steps, renders });
      } else if (m.type === 'error') { worker.terminate(); reject(new Error(m.message)); }
    };
    worker.onerror = (e) => { worker.terminate(); reject(new Error(e.message || 'Image processing failed.')); };
    const copy = new Uint8ClampedArray(src.data);
    worker.postMessage({ id, type: 'run', src: { width: src.width, height: src.height, data: copy.buffer }, prep, shared, renderMaxDim }, [copy.buffer]);
  });
  return { promise, cancel: () => { if (!cancelled) { cancelled = true; worker.terminate(); rejectFn(new Error('cancelled')); } } };
}

/** Render a single style (used for live style thumbnails and setting tweaks). */
export function renderInBackground(src, prep, shared, style, renderMaxDim, analysis) {
  const worker = startWorker();
  if (!worker) return Promise.resolve(renderFor(src, prep, shared, style, renderMaxDim, analysis));
  const id = ++seq;
  return new Promise((resolve, reject) => {
    worker.onmessage = (ev) => {
      const m = ev.data;
      worker.terminate();
      if (m.type === 'done') resolve({ width: m.render.width, height: m.render.height, data: new Uint8ClampedArray(m.render.data) });
      else reject(new Error(m.message));
    };
    worker.onerror = (e) => { worker.terminate(); reject(new Error(e.message)); };
    const copy = new Uint8ClampedArray(src.data);
    worker.postMessage({ id, type: 'render', src: { width: src.width, height: src.height, data: copy.buffer }, prep, shared, renderMaxDim, style, analysis }, [copy.buffer]);
  });
}
