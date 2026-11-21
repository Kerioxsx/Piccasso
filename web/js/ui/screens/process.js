import { app } from '../../state.js';
import { h, toast, nav, dialog } from '../dom.js';
import { runInBackground } from '../processing.js';
import { defaultProgress } from '../../core/project.js';
import { prepareImage } from '../../core/image.js';
import { rgbaToCanvas } from '../imageio.js';

export function renderProcess(main) {
  const p = app.project;
  const bar = h('i');
  const label = h('p.muted', 'Getting started…');
  let job = null, cancelled = false;
  const cancel = () => {
    cancelled = true;
    if (job) job.cancel();
    nav('prepare');
  };
  main.appendChild(h('div.processing.card',
    h('svg.pencil-anim', { viewBox: '0 0 80 80', 'aria-hidden': 'true', html: '<path d="M10 60c12-2 18-14 26-26s16-20 34-22"/>' }),
    h('h1', 'Planning your drawing'),
    h('div.progressbar', { role: 'progressbar', 'aria-label': 'Progress', 'aria-valuemin': 0, 'aria-valuemax': 100 }, bar),
    label,
    h('p.small.muted', 'Everything runs on your device. This usually takes a few seconds.'),
    h('button.btn', { type: 'button', on: { click: cancel } }, 'Cancel')));
  const pb = main.querySelector('[role=progressbar]');
  const setP = (f, text) => { bar.style.width = `${Math.round(f * 100)}%`; pb.setAttribute('aria-valuenow', Math.round(f * 100)); if (text) label.textContent = text + '…'; };

  (async () => {
    try {
      job = runInBackground(app.src, structuredClone(p.prep), app.shared, { renderMaxDim: 1280, onProgress: setP });
      const res = await job.promise;
      if (cancelled) return;
      const steps = res.steps;
      delete p.ai;
      if (cancelled) return;
      const sameSteps = p.steps.length === steps.length && p.steps.every((s, i) => s.kind === steps[i].kind);
      p.analysis = res.analysis;
      p.steps = steps;
      if (!sameSteps) p.progress = { ...defaultProgress(), timeSpentSec: p.progress.timeSpentSec };
      delete p._stale;
      await app.setRenders(res.renders);
      await app.saveProject();
      nav('workspace');
    } catch (e) {
      if (cancelled || e.message === 'cancelled') return;
      main.innerHTML = '';
      main.appendChild(h('div.processing.card.stack',
        h('h1', 'We couldn\'t analyse this picture'),
        h('p', e.message),
        h('p.muted', 'You can still draw with a simple manual workflow: use the original picture or a filter as your guide, and follow general steps.'),
        h('div.row',
          h('button.btn.primary', { type: 'button', on: { click: () => manualFallback() } }, 'Use manual steps'),
          h('button.btn', { type: 'button', on: { click: () => nav('prepare') } }, 'Back to preparation'))));
    }
  })();

  async function manualFallback() {
    // Minimal plan without analysis: generic steps over the plain picture.
    const S = app.shared.strings.steps;
    const kinds = ['page', 'contours', 'clean', 'values', 'details'];
    p.analysis = { engine: 'manual', width: app.src.width, height: app.src.height, aspect: app.src.width / app.src.height, lowContrast: true,
      subject: { bbox: [0, 0, 1, 1], aspect: app.src.width / app.src.height, whole: true }, objects: [], activeObjects: [], silhouettes: [], internals: [], shapes: [],
      values: { levels: 3, fractions: [], darkPos: 'mc', lightDir: 'even' }, detail: { cells: [], pos: 'mc', density: 0 }, landmarks: [] };
    p.steps = kinds.map((k, i) => ({ id: `step-${i + 1}-${k}`, kind: k, title: S[k].title,
      instruction: app.shared.strings.manualSteps[k], tip: S[k].tip, checks: [], base: k === 'page' ? 'none' : 'original', layers: k === 'page' ? ['frame', 'grid'] : [], terms: [] }));
    p.progress = defaultProgress();
    app.renders = { original: rgbaToCanvas(prepareImage(app.src, p.prep, 1280)) };
    await app.saveProject();
    nav('workspace');
  }
  return () => { cancelled = true; if (job) job.cancel(); };
}
