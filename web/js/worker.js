// Web Worker: keeps image processing off the UI thread. Cancellation is done by
// the UI terminating the worker (instant) — see ui/processing.js.
import { runPipeline, renderFor } from './core/pipeline.js';

self.onmessage = (ev) => {
  const { id, type, src, prep, shared, renderMaxDim, style, analysis } = ev.data;
  const img = { width: src.width, height: src.height, data: new Uint8ClampedArray(src.data) };
  try {
    if (type === 'run') {
      const res = runPipeline(img, prep, shared, {
        renderMaxDim,
        onProgress: (p, label) => self.postMessage({ id, type: 'progress', p, label }),
      });
      const transfer = [];
      const renders = {};
      for (const [k, r] of Object.entries(res.renders)) {
        renders[k] = { width: r.width, height: r.height, data: r.data.buffer };
        transfer.push(r.data.buffer);
      }
      self.postMessage({ id, type: 'done', analysis: res.analysis, steps: res.steps, renders }, transfer);
    } else if (type === 'render') {
      const r = renderFor(img, prep, shared, style, renderMaxDim, analysis);
      self.postMessage({ id, type: 'done', render: { width: r.width, height: r.height, data: r.data.buffer } }, [r.data.buffer]);
    }
  } catch (err) {
    self.postMessage({ id, type: 'error', message: String(err && err.message || err) });
  }
};
