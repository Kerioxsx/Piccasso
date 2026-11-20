import { app } from '../../state.js';
import { h, icon, toast, nav, dialog, announce, confirmDialog } from '../dom.js';
import { computeRect, fitCanvas, drawLayers, LAYER_COLORS } from '../canvas.js';
import { attachGestures } from '../gestures.js';
import { speak, stopSpeaking, speechSupported } from '../speech.js';
import { gotoStep, completeStep, restartTutorial } from '../../core/project.js';
import { runInBackground } from '../processing.js';
import { exportPNG, exportStepSheet, exportProjectFile } from '../exporter.js';
import { composeGuide } from '../canvas.js';

export function glossaryDialog(termId) {
  const t = app.shared.glossary.terms.find((x) => x.id === termId);
  if (!t) return;
  dialog({ title: t.term, body: t.text, buttons: [{ label: 'Got it', value: true, primary: true }] });
}

export async function renderWorkspace(main) {
  const p = app.project;
  main.classList.add('full');
  let view = { zoom: 1, panX: 0, panY: 0 };
  let showGuide = true, split = false, splitX = 0.5, mirror = false, loading = false;

  const canvas = h('canvas', { role: 'img', 'aria-label': 'Drawing guide for the current step' });
  const splitHandle = h('div.split-handle', { hidden: true, role: 'slider', tabIndex: 0, 'aria-label': 'Compare divider', 'aria-valuemin': 0, 'aria-valuemax': 100 });
  const splitLabels = [h('div.split-label', { hidden: true, style: { left: '10px' } }, 'Original'), h('div.split-label', { hidden: true, style: { right: '64px' } }, 'Guide')];
  const loadingNote = h('div.split-label', { hidden: true, style: { left: '10px', bottom: '10px', top: 'auto' } }, 'Preparing guide images…');
  const tool = (ic, label, fn, pressed) => h('button.icon-btn', { type: 'button', 'aria-label': label, title: label, 'aria-pressed': pressed === undefined ? null : String(pressed), on: { click: fn } }, icon(ic));
  const compareBtn = tool('compare', 'Compare with original (split view)', () => { split = !split; compareBtn.setAttribute('aria-pressed', String(split)); splitHandle.hidden = splitLabels[0].hidden = splitLabels[1].hidden = !split; draw(); }, false);
  const mirrorBtn = tool('flip', 'Mirror view (helps spot mistakes)', () => { mirror = !mirror; mirrorBtn.setAttribute('aria-pressed', String(mirror)); draw(); announce(mirror ? 'Mirrored view on' : 'Mirrored view off'); }, false);
  const stage = h('div.stage', canvas, splitHandle, ...splitLabels, loadingNote,
    h('div.stage-tools',
      tool('zoomin', 'Zoom in', () => zoomBy(1.25)), tool('zoomout', 'Zoom out', () => zoomBy(0.8)), tool('fit', 'Fit to screen', fit),
      compareBtn, mirrorBtn));

  // Panel
  const progressText = h('strong');
  const progressBar = h('i');
  const dots = h('div.step-dots', { role: 'list', 'aria-label': 'Steps' });
  const titleEl = h('h2', { tabIndex: -1 });
  const instrEl = h('p.instruction');
  const tipEl = h('p.tip');
  const termsEl = h('div.chips');
  const checksEl = h('ul.checklist');
  const aiEl = h('div');
  const guideBtn = h('button.btn.small', { type: 'button', 'aria-pressed': 'true', on: { click: () => { showGuide = !showGuide; guideBtn.setAttribute('aria-pressed', String(showGuide)); guideBtn.lastChild.textContent = showGuide ? 'Hide guide' : 'Show guide'; draw(); announce(showGuide ? 'Guide shown' : 'Guide hidden, showing your picture'); } } }, icon('eye'), h('span', 'Hide guide'));
  const prevBtn = h('button.btn', { type: 'button', on: { click: () => go(p.progress.currentStep - 1) } }, icon('prev'), 'Previous');
  const nextBtn = h('button.btn.primary', { type: 'button', on: { click: next } }, 'Done, next', icon('next'));
  const legend = h('div.legend', [['Shapes', LAYER_COLORS.shapes], ['Outline', LAYER_COLORS.silhouette], ['Inner shapes', LAYER_COLORS.internal], ['Subject area', LAYER_COLORS.subject]]
    .map(([t, c]) => h('span', h('i', { style: { background: c } }), t)));

  const exportMenu = async () => {
    const choice = await dialog({ title: 'Export', body: 'Save a copy of your guide or project.', buttons: [
      { label: 'Cancel', value: null }, { label: 'Project file', value: 'project' }, { label: 'Step sheet (print)', value: 'sheet' }, { label: 'Guide image (PNG)', value: 'png', primary: true }] });
    if (choice === 'png') exportPNG(currentComposite(), `${p.name}-step-${p.progress.currentStep + 1}`);
    else if (choice === 'sheet') exportStepSheet(p, (b) => app.baseCanvas(b));
    else if (choice === 'project') exportProjectFile(p);
  };
  const restartMenu = async () => {
    const choice = await dialog({ title: 'Start over?', body: 'Restart just this step, or go back to the very first step.', buttons: [
      { label: 'Cancel', value: null }, { label: 'Restart whole tutorial', value: 'all' }, { label: 'Restart this step', value: 'step', primary: true }] });
    if (choice === 'step') { fit(); checksEl.querySelectorAll('input').forEach((i) => { i.checked = false; }); const id = step().id; p.progress.completed = p.progress.completed.filter((x) => x !== id); p.progress.skipped = p.progress.skipped.filter((x) => x !== id); update(); readAloud(); }
    else if (choice === 'all') { app.project = restartTutorial(p); Object.assign(p, app.project); fit(); update(); }
  };

  const panel = h('aside.panel', { 'aria-label': 'Step instructions' },
    h('div.panel-scroll.stack',
      h('div.step-progress', progressText, h('div.progressbar', { style: { flex: 1 } }, progressBar)),
      dots, titleEl, instrEl,
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: readAloud }, disabled: !speechSupported(), title: speechSupported() ? '' : 'Audio readout is not supported in this browser' }, icon('speak'), 'Read aloud'),
        guideBtn,
        h('button.btn.small', { type: 'button', on: { click: () => next({ skip: true }) } }, icon('skip'), 'Skip'),
        h('button.btn.small', { type: 'button', on: { click: restartMenu } }, icon('restart'), 'Restart')),
      tipEl, aiEl, termsEl,
      h('details', { open: true }, h('summary', h('strong', 'Check your drawing')), h('p.small.muted', 'Look at your paper and the guide side by side. Tap the mirror button to view the guide flipped; holding your drawing up to a mirror the same way makes mistakes easier to spot.'), checksEl),
      legend,
      h('div.mode-bar',
        h('button.btn', { type: 'button', on: { click: () => nav('locked') } }, icon('lock'), 'Locked view'),
        h('button.btn', { type: 'button', on: { click: () => nav('ar') } }, icon('ar'), 'Camera trace'),
        h('button.btn', { type: 'button', on: { click: () => nav('compare') } }, icon('photo'), 'Check photo')),
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: () => nav('prepare') } }, icon('edit'), 'Edit picture'),
        h('button.btn.small', { type: 'button', on: { click: exportMenu } }, icon('export'), 'Export'),
        h('span.small.muted', { style: { marginLeft: 'auto' } }, 'Text size'),
        h('button.btn.small', { type: 'button', 'aria-label': 'Smaller text', on: { click: () => textScale(-0.1) } }, 'A−'),
        h('button.btn.small', { type: 'button', 'aria-label': 'Larger text', on: { click: () => textScale(0.1) } }, 'A+'))),
    h('div.step-nav', prevBtn, nextBtn));

  main.appendChild(h('div.workspace', stage, panel));

  const step = () => p.steps[p.progress.currentStep];
  function textScale(d) { app.settings.textScale = Math.min(1.8, Math.max(0.8, Math.round((app.settings.textScale + d) * 10) / 10)); app.saveSettings(); }
  function readAloud() { const s = step(); if (s) speak(`${s.title}. ${s.instruction} Tip: ${s.tip}`); }
  function fit() { view = { zoom: 1, panX: 0, panY: 0 }; draw(); }
  function zoomBy(f) { view = { ...view, zoom: Math.min(8, Math.max(0.2, view.zoom * f)), panX: view.panX * f, panY: view.panY * f }; draw(); }
  async function persist() { try { await app.saveProject(); Object.assign(p, app.project); } catch (e) { toast(e.message, { error: true }); } }
  function go(i) {
    if (i < 0 || i >= p.steps.length) return;
    Object.assign(p, gotoStep(p, i));
    stopSpeaking(); update(); persist();
    if (app.settings.autoRead) readAloud();
  }
  async function next({ skip = false } = {}) {
    const last = p.progress.currentStep === p.steps.length - 1;
    Object.assign(p, completeStep(p, { skipped: skip }));
    stopSpeaking(); update(); persist();
    if (last && !skip) {
      await dialog({ title: 'You finished all the steps! 🎉', body: 'Great work. You can take a photo of your drawing to compare it with the guide, export a step sheet, or start a new picture.', buttons: [{ label: 'Close', value: 1, primary: true }] });
    } else if (app.settings.autoRead) readAloud();
  }

  function update() {
    const s = step();
    const n = p.steps.length, i = p.progress.currentStep;
    if (!s) { titleEl.textContent = 'No steps yet'; instrEl.textContent = 'Go back to prepare your picture.'; return; }
    progressText.textContent = `Step ${i + 1} of ${n}`;
    progressBar.style.width = `${Math.round(((p.progress.completed.length + p.progress.skipped.length) / n) * 100)}%`;
    dots.innerHTML = '';
    p.steps.forEach((st, k) => dots.appendChild(h('button', {
      type: 'button', role: 'listitem', 'aria-current': k === i ? 'step' : null,
      class: p.progress.completed.includes(st.id) ? 'done' : p.progress.skipped.includes(st.id) ? 'skipped' : '',
      'aria-label': `Step ${k + 1}: ${st.title}${p.progress.completed.includes(st.id) ? ', done' : p.progress.skipped.includes(st.id) ? ', skipped' : ''}`,
      on: { click: () => go(k) } }, String(k + 1))));
    titleEl.textContent = s.title;
    instrEl.textContent = s.instruction;
    tipEl.textContent = s.tip;
    aiEl.innerHTML = '';
    if (s.source === 'ai') {
      const t = h('button.btn.small.ghost', { type: 'button', on: { click: () => { const showing = instrEl.dataset.local === '1'; instrEl.textContent = showing ? s.instruction : s.localInstruction; instrEl.dataset.local = showing ? '0' : '1'; t.textContent = showing ? 'Show built-in wording' : 'Show AI wording'; } } }, 'Show built-in wording');
      aiEl.appendChild(h('p.small.muted', 'Wording written by the AI helper. ', t));
    }
    termsEl.innerHTML = '';
    if (s.terms && s.terms.length) {
      termsEl.appendChild(h('span.small.muted', 'What does it mean? '));
      for (const id of s.terms) {
        const t = app.shared.glossary.terms.find((x) => x.id === id);
        if (t) termsEl.appendChild(h('button.chip', { type: 'button', on: { click: () => glossaryDialog(id) } }, t.term));
      }
    }
    checksEl.innerHTML = '';
    (s.checks || []).forEach((c) => checksEl.appendChild(h('li', h('input', { type: 'checkbox', 'aria-label': c }), h('span', c))));
    prevBtn.disabled = i === 0;
    nextBtn.lastChild.previousSibling.textContent = i === n - 1 ? 'Finish' : 'Done, next';
    announce(`Step ${i + 1} of ${n}: ${s.title}`);
    draw();
  }

  function currentComposite() {
    const s = step();
    const base = showGuide ? app.baseCanvas(s.base) : app.baseCanvas('original');
    return composeGuide(base, p.analysis, showGuide ? s.layers : [], { width: (app.renders.original || app.srcCanvas).width });
  }

  function draw() {
    const { ctx, w, h: hh } = fitCanvas(canvas);
    ctx.clearRect(0, 0, w, hh);
    const s = step();
    if (!s || !p.analysis) return;
    const ref = app.renders.original || app.srcCanvas;
    const rect = computeRect(w, hh, ref.width, ref.height, view);
    ctx.save();
    if (mirror) { ctx.translate(w, 0); ctx.scale(-1, 1); }
    const paintGuide = () => {
      ctx.fillStyle = '#fff'; ctx.fillRect(rect.x, rect.y, rect.w, rect.h);
      const base = showGuide ? app.baseCanvas(s.base) : ref;
      if (base) { ctx.globalAlpha = (p.view.opacity || 100) / 100; ctx.drawImage(base, rect.x, rect.y, rect.w, rect.h); ctx.globalAlpha = 1; }
      if (showGuide) drawLayers(ctx, p.analysis, s.layers, rect);
    };
    ctx.shadowColor = 'rgba(0,0,0,.15)'; ctx.shadowBlur = 12;
    ctx.fillStyle = '#fff'; ctx.fillRect(rect.x, rect.y, rect.w, rect.h);
    ctx.shadowBlur = 0;
    if (split) {
      const sx = splitX * w;
      ctx.save(); ctx.beginPath(); ctx.rect(0, 0, mirror ? w - sx : sx, hh); ctx.clip();
      ctx.drawImage(ref, rect.x, rect.y, rect.w, rect.h); ctx.restore();
      ctx.save(); ctx.beginPath(); ctx.rect(mirror ? w - sx : sx, 0, w, hh); ctx.clip(); paintGuide(); ctx.restore();
      splitHandle.style.left = `${sx - 2}px`;
      splitHandle.setAttribute('aria-valuenow', Math.round(splitX * 100));
    } else paintGuide();
    ctx.restore();
  }

  // Split handle drag
  splitHandle.addEventListener('pointerdown', (e) => { e.stopPropagation(); splitHandle.setPointerCapture(e.pointerId); splitHandle.dataset.drag = '1'; });
  splitHandle.addEventListener('pointermove', (e) => { if (!splitHandle.dataset.drag) return; e.stopPropagation(); const r = stage.getBoundingClientRect(); splitX = Math.min(0.98, Math.max(0.02, (e.clientX - r.left) / r.width)); draw(); });
  splitHandle.addEventListener('pointerup', () => { delete splitHandle.dataset.drag; });
  splitHandle.addEventListener('keydown', (e) => { if (e.key === 'ArrowLeft') { splitX = Math.max(0.02, splitX - 0.05); draw(); } if (e.key === 'ArrowRight') { splitX = Math.min(0.98, splitX + 0.05); draw(); } });

  const detach = attachGestures(stage, { getView: () => view, setView: (v) => { view = v; draw(); }, onReset: fit });
  const ro = new ResizeObserver(() => draw());
  ro.observe(stage);
  const onKey = (e) => {
    if (e.target.closest('input,select,textarea,dialog')) return;
    if (e.key === 'ArrowRight') next();
    else if (e.key === 'ArrowLeft') go(p.progress.currentStep - 1);
    else if (e.key.toLowerCase() === 'g') guideBtn.click();
    else if (e.key.toLowerCase() === 'r') readAloud();
    else if (e.key.toLowerCase() === 'l') nav('locked');
    else if (e.key === '+' || e.key === '=') zoomBy(1.25);
    else if (e.key === '-') zoomBy(0.8);
    else if (e.key === '0') fit();
  };
  document.addEventListener('keydown', onKey);
  let visibleSince = Date.now();
  const timer = setInterval(() => {
    if (document.visibilityState !== 'visible') { visibleSince = Date.now(); return; }
    p.progress.timeSpentSec += Math.round((Date.now() - visibleSince) / 1000); visibleSince = Date.now();
  }, 15000);

  update();

  // Ensure guide renders exist (memory → IndexedDB cache → regenerate in background).
  if (!(await app.loadCachedRenders())) {
    loading = true; loadingNote.hidden = false;
    const job = runInBackground(app.src, structuredClone(p.prep), app.shared, { renderMaxDim: 1280, onProgress: (f) => { loadingNote.textContent = `Preparing guide images… ${Math.round(f * 100)}%`; } });
    try {
      const res = await job.promise;
      if (!p.analysis || p._stale) { p.analysis = res.analysis; }
      await app.setRenders(res.renders);
    } catch (e) { if (e.message !== 'cancelled') toast('Could not prepare guide images: ' + e.message, { error: true }); }
    loading = false; loadingNote.hidden = true;
    draw();
  }
  if (app.settings.autoRead) readAloud();

  return () => {
    detach(); ro.disconnect(); clearInterval(timer); stopSpeaking();
    document.removeEventListener('keydown', onKey);
    p.progress.timeSpentSec += Math.round((Date.now() - visibleSince) / 1000);
    app.saveProject().catch(() => {});
    if (loading) { /* background job result is ignored after leaving */ }
  };
}
