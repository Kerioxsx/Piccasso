import { app } from '../../state.js';
import { h, icon, slider, segmented, toggle, toast, nav } from '../dom.js';
import { rotate, flip, fitDims, resizeBox, prepareImage } from '../../core/image.js';
import { renderStyle } from '../../core/render.js';
import { planProject } from '../../core/pipeline.js';
import { rgbaToCanvas } from '../imageio.js';
import { renderInBackground } from '../processing.js';
import { drawLayers } from '../canvas.js';
import { paperLayout } from '../../core/steps.js';
import { dialog } from '../dom.js';

const ASPECTS = [['free', 'Free'], ['original', 'Original'], ['1', 'Square'], ['0.75', '3:4'], ['1.333', '4:3'], ['paper', 'Paper']];

export function renderPrepare(main) {
  const p = app.project;
  const prep = p.prep;
  const shared = app.shared;
  let tab = 'crop';
  let aspectMode = 'free';
  let objectsAnalysis = null;
  let previewTimer = 0, previewSeq = 0;
  let oriented = null; // small rotated/flipped source for the crop editor

  const cropCanvas = h('canvas', { 'aria-label': 'Picture with crop area' });
  const styleCanvas = h('canvas', { 'aria-label': 'Style preview', hidden: true });
  const cropBox = h('div.crop-box', { role: 'group', 'aria-label': 'Crop area. Drag to move, drag the corners to resize.' },
    ['tl', 'tr', 'bl', 'br'].map((c) => h('div.h.' + c, { 'data-h': c })));
  const previewWrap = h('div.preview-wrap', cropCanvas, styleCanvas, cropBox);
  const busy = h('span.small.muted', { 'aria-live': 'polite' });

  function buildOriented() {
    const [w, hh] = fitDims(app.src.width, app.src.height, 900);
    oriented = flip(rotate(resizeBox(app.src, w, hh), prep.rotation), prep.flipH, prep.flipV);
    rgbaToCanvas(oriented, cropCanvas);
    cropCanvas.style.filter = `brightness(${1 + prep.brightness / 100}) contrast(${1 + prep.contrast / 100})`;
    requestAnimationFrame(placeCropBox);
  }

  function placeCropBox() {
    if (tab !== 'crop') { cropBox.hidden = true; return; }
    cropBox.hidden = false;
    const wr = previewWrap.getBoundingClientRect(), cr = cropCanvas.getBoundingClientRect();
    const c = prep.crop;
    Object.assign(cropBox.style, {
      left: `${cr.left - wr.left + c.x * cr.width}px`, top: `${cr.top - wr.top + c.y * cr.height}px`,
      width: `${c.w * cr.width}px`, height: `${c.h * cr.height}px`,
    });
  }

  function targetAspect() {
    if (aspectMode === 'free') return null;
    if (aspectMode === 'original') return oriented.width / oriented.height;
    if (aspectMode === 'paper') {
      const pr = shared.paper.presets.find((x) => x.id === prep.paper.preset) || shared.paper.presets[0];
      const land = prep.paper.orientation === 'landscape' || (prep.paper.orientation === 'auto' && oriented.width > oriented.height);
      return land ? pr.heightMm / pr.widthMm : pr.widthMm / pr.heightMm;
    }
    return +aspectMode;
  }

  /** Make the crop match the aspect (in pixels), centred on the current crop. */
  function applyAspect() {
    const a = targetAspect();
    if (!a) return;
    const imgA = oriented.width / oriented.height;
    let { x, y, w, h: hh } = prep.crop;
    const cx = x + w / 2, cy = y + hh / 2;
    // normalized w/h relationship: (w*W)/(h*H) = a  →  w = a*h/imgA
    if ((w * imgA) / hh > a) w = (a * hh) / imgA; else hh = (w * imgA) / a;
    w = Math.min(w, 1); hh = Math.min(hh, 1);
    x = Math.min(Math.max(0, cx - w / 2), 1 - w); y = Math.min(Math.max(0, cy - hh / 2), 1 - hh);
    prep.crop = { x, y, w, h: hh };
    placeCropBox(); changed();
  }

  // Crop dragging
  let drag = null;
  cropBox.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    cropBox.setPointerCapture(e.pointerId);
    drag = { handle: e.target.dataset.h || 'move', sx: e.clientX, sy: e.clientY, start: { ...prep.crop } };
  });
  cropBox.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const cr = cropCanvas.getBoundingClientRect();
    const dx = (e.clientX - drag.sx) / cr.width, dy = (e.clientY - drag.sy) / cr.height;
    let { x, y, w, h: hh } = drag.start;
    const MIN = 0.05;
    if (drag.handle === 'move') {
      x = Math.min(Math.max(0, x + dx), 1 - w); y = Math.min(Math.max(0, y + dy), 1 - hh);
    } else {
      let x1 = x + w, y1 = y + hh;
      if (drag.handle.includes('l')) x = Math.min(Math.max(0, x + dx), x1 - MIN); else x1 = Math.max(Math.min(1, x1 + dx), x + MIN);
      if (drag.handle.includes('t')) y = Math.min(Math.max(0, y + dy), y1 - MIN); else y1 = Math.max(Math.min(1, y1 + dy), y + MIN);
      w = x1 - x; hh = y1 - y;
      const a = targetAspect();
      if (a) {
        const imgA = oriented.width / oriented.height;
        const nh = (w * imgA) / a;
        if (drag.handle.includes('t')) y = y1 - nh;
        hh = nh;
        if (y < 0 || y + hh > 1) { hh = drag.handle.includes('t') ? y1 : 1 - y; y = drag.handle.includes('t') ? 0 : y; w = (a * hh) / imgA; if (drag.handle.includes('l')) x = x1 - w; }
      }
    }
    prep.crop = { x, y, w, h: hh };
    placeCropBox();
  });
  const endDrag = () => { if (drag) { drag = null; changed(); } };
  cropBox.addEventListener('pointerup', endDrag);
  cropBox.addEventListener('pointercancel', endDrag);
  cropBox.addEventListener('keydown', (e) => {
    const step = e.shiftKey ? 0.05 : 0.01;
    const k = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
    if (!k) return;
    e.preventDefault();
    const c = prep.crop;
    prep.crop = { ...c, x: Math.min(Math.max(0, c.x + k[0]), 1 - c.w), y: Math.min(Math.max(0, c.y + k[1]), 1 - c.h) };
    placeCropBox(); changed();
  });
  cropBox.tabIndex = 0;

  function setTab(t) {
    tab = t;
    cropCanvas.hidden = t !== 'crop';
    styleCanvas.hidden = t !== 'style';
    tabs.querySelectorAll('button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.v === t)));
    placeCropBox();
    if (t === 'style') schedulePreview(0);
  }

  function schedulePreview(delay = 250) {
    clearTimeout(previewTimer);
    if (tab !== 'style') return;
    previewTimer = setTimeout(async () => {
      const seq = ++previewSeq;
      busy.textContent = 'Updating preview…';
      try {
        const r = await renderInBackground(app.src, structuredClone(prep), shared, prep.style, 640, objectsAnalysis);
        if (seq !== previewSeq) return;
        rgbaToCanvas(r, styleCanvas);
        if (prep.style === 'basic_shapes' || objectsAnalysis) {
          const ctx = styleCanvas.getContext('2d');
          const a = objectsAnalysis || planProject(app.src, prep, shared).analysis;
          const rect = { x: 0, y: 0, w: styleCanvas.width, h: styleCanvas.height };
          if (prep.style === 'basic_shapes') drawLayers(ctx, a, ['shapes'], rect);
          if (objectsAnalysis) drawObjectBoxes(ctx, a, rect);
        }
        busy.textContent = '';
      } catch (e) { busy.textContent = ''; toast('Preview failed: ' + e.message, { error: true }); }
    }, delay);
  }

  function drawObjectBoxes(ctx, a, rect) {
    const act = new Set(a.activeObjects);
    a.objects.forEach((o, i) => {
      const [x0, y0, x1, y1] = o.bbox;
      ctx.save();
      ctx.strokeStyle = act.has(o.id) ? '#2f7466' : '#999'; ctx.lineWidth = Math.max(2, rect.w / 300);
      ctx.setLineDash(act.has(o.id) ? [] : [6, 6]);
      ctx.strokeRect(x0 * rect.w, y0 * rect.h, (x1 - x0) * rect.w, (y1 - y0) * rect.h);
      ctx.fillStyle = act.has(o.id) ? '#2f7466' : '#999';
      ctx.fillRect(x0 * rect.w, y0 * rect.h, 26, 22);
      ctx.fillStyle = '#fff'; ctx.font = '600 14px system-ui'; ctx.fillText(String(i + 1), x0 * rect.w + 8, y0 * rect.h + 16);
      ctx.restore();
    });
  }

  let saveTimer = 0;
  async function measureTool() {
    // Calibrate CSS px per mm with a standard ID-1 card (85.60 × 53.98 mm), then show a cm ruler.
    let pxPerMm = app.settings.pxPerMm || 3.78;
    const card = h('div.card-outline');
    const ruler = h('canvas.ruler', { width: 640, height: 60, 'aria-label': 'On-screen ruler in centimetres' });
    const drawRuler = () => {
      card.style.width = `${85.6 * pxPerMm}px`; card.style.height = `${53.98 * pxPerMm}px`;
      const cssW = Math.min(window.innerWidth - 80, 560);
      ruler.style.width = cssW + 'px'; ruler.style.height = '60px';
      const dpr = window.devicePixelRatio || 1; ruler.width = cssW * dpr; ruler.height = 60 * dpr;
      const ctx = ruler.getContext('2d'); ctx.scale(dpr, dpr); ctx.clearRect(0, 0, cssW, 60);
      ctx.strokeStyle = getComputedStyle(document.body).color; ctx.fillStyle = ctx.strokeStyle; ctx.font = '12px system-ui';
      for (let mm = 0; mm * pxPerMm <= cssW; mm++) {
        const x = mm * pxPerMm + 0.5, len = mm % 10 === 0 ? 26 : mm % 5 === 0 ? 18 : 10;
        ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, len); ctx.stroke();
        if (mm % 10 === 0) ctx.fillText(String(mm / 10), x + 2, 42);
      }
    };
    const body = h('div.stack',
      h('p', 'Hold a bank card (or any ID-sized card) against the screen and move the slider until the outline matches it exactly. Then use the ruler to measure your paper or check a drawing size.'),
      card,
      slider({ label: 'Card size', min: 2, max: 8, step: 0.01, value: pxPerMm, format: (v) => v.toFixed(2) + ' px/mm', onInput: (v) => { pxPerMm = v; drawRuler(); } }),
      ruler,
      h('p.small.muted', 'Paper bigger than your screen? Measure along one edge in steps, or use the size printed on the paper pack. Common sizes: A4 21 × 29.7 cm, A5 14.8 × 21 cm, Letter 21.6 × 27.9 cm.'));
    setTimeout(drawRuler, 30);
    const ok = await dialog({ title: 'Measure with a card', body, buttons: [{ label: 'Close', value: false }, { label: 'Save calibration', value: true, primary: true }] });
    if (ok) { app.settings.pxPerMm = pxPerMm; app.saveSettings(); toast('Screen ruler calibrated'); }
  }

  function changed({ thumbs = false } = {}) {
    p.analysis && (p._stale = true);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => app.saveProject().catch((e) => toast(e.message, { error: true })), 600);
    schedulePreview();
    if (thumbs) renderThumbs();
  }

  // Style thumbnails (small, main thread, staggered).
  const styleButtons = {};
  const styleGrid = h('div.style-grid', { role: 'group', 'aria-label': 'Drawing style' },
    shared.styles.styles.map((s) => {
      const c = h('canvas', { width: 160, height: 120 });
      const b = h('button.style-opt', { type: 'button', title: s.description, 'aria-pressed': String(prep.style === s.id),
        on: { click: () => { prep.style = s.id; Object.values(styleButtons).forEach((x) => x.b.setAttribute('aria-pressed', String(x.id === s.id))); styleDesc.textContent = s.description; changed(); if (tab !== 'style') setTab('style'); } } },
        c, h('span', s.label));
      styleButtons[s.id] = { b, c, id: s.id };
      return b;
    }));
  const styleDesc = h('p.small.muted', (shared.styles.styles.find((s) => s.id === prep.style) || {}).description || '');
  let thumbToken = 0;
  function renderThumbs() {
    const token = ++thumbToken;
    const small = prepareImage(app.src, prep, 160);
    const level = shared.difficulty.levels[prep.difficulty];
    const ids = Object.keys(styleButtons);
    let i = 0;
    const next = () => {
      if (token !== thumbToken || i >= ids.length) return;
      const id = ids[i++];
      const r = renderStyle(small, id, { level, lineThickness: Math.max(1, prep.lineThickness - 1), sketchContrast: prep.sketchContrast });
      const c = styleButtons[id].c;
      const tmp = rgbaToCanvas(r);
      const ctx = c.getContext('2d');
      ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, c.width, c.height);
      const s = Math.min(c.width / tmp.width, c.height / tmp.height);
      ctx.drawImage(tmp, (c.width - tmp.width * s) / 2, (c.height - tmp.height * s) / 2, tmp.width * s, tmp.height * s);
      setTimeout(next, 0);
    };
    next();
  }

  // Objects (remove / isolate)
  const objChips = h('div.chips');
  function renderObjectChips() {
    objChips.innerHTML = '';
    if (!objectsAnalysis) return;
    if (!objectsAnalysis.objects.length) { objChips.appendChild(h('p.small.muted', 'No separate objects found. The whole picture will be used.')); return; }
    objectsAnalysis.objects.forEach((o, i) => {
      const removed = prep.excludedObjects.includes(o.id);
      const isolated = prep.isolateObject === o.id;
      objChips.appendChild(h('div.row', { style: { gap: '6px' } },
        h('button.chip' + (removed ? '.removed' : ''), { type: 'button', 'aria-pressed': String(!removed),
          'aria-label': `Object ${i + 1}: ${removed ? 'removed' : 'included'}. Tap to ${removed ? 'include' : 'remove'}.`,
          on: { click: () => { prep.excludedObjects = removed ? prep.excludedObjects.filter((x) => x !== o.id) : [...prep.excludedObjects, o.id]; prep.isolateObject = null; refreshObjects(); } } },
          `Object ${i + 1}`),
        h('button.chip', { type: 'button', 'aria-pressed': String(isolated),
          on: { click: () => { prep.isolateObject = isolated ? null : o.id; refreshObjects(); } } }, isolated ? 'Showing only this' : 'Only this')));
    });
  }
  function refreshObjects() {
    objectsAnalysis = planProject(app.src, prep, shared).analysis;
    renderObjectChips(); changed();
  }

  const tabs = segmented('Preview', [['crop', 'Crop & rotate'], ['style', 'Style preview']], tab, setTab);
  const nameInput = h('input', { type: 'text', value: p.name, 'aria-label': 'Project name', on: { change: () => { p.name = nameInput.value.trim() || p.name; changed(); } } });
  const paperSel = h('select', { 'aria-label': 'Paper size', on: { change: () => { prep.paper.preset = paperSel.value; customRow.hidden = paperSel.value !== 'custom'; if (aspectMode === 'paper') applyAspect(); updateSize(); changed(); } } },
    [...shared.paper.presets.map((pr) => h('option', { value: pr.id, selected: pr.id === prep.paper.preset }, `${pr.label} (${pr.widthMm / 10} × ${pr.heightMm / 10} cm)`)),
      h('option', { value: 'custom', selected: prep.paper.preset === 'custom' }, 'Custom size (cm)')]);
  const cmInput = (key, label) => h('label.field', label, h('input', { type: 'number', min: 5, max: 200, step: 0.1, inputmode: 'decimal', value: ((prep.paper[key] || 0) / 10) || '',
    on: { input: (e) => { const v = parseFloat(e.target.value); if (v > 0) { prep.paper[key] = Math.round(v * 10); updateSize(); changed(); } } } }));
  const customRow = h('div.grid.two', { hidden: prep.paper.preset !== 'custom' }, cmInput('widthMm', 'Paper width (cm)'), cmInput('heightMm', 'Paper height (cm)'));
  const sizeOut = h('p.size-readout', { 'aria-live': 'polite' });
  function updateSize() {
    const L = paperLayout(prep.paper, shared.paper.presets, oriented ? (oriented.width * prep.crop.w) / (oriented.height * prep.crop.h) : 1, shared.paper.defaultMarginMm);
    const grid = prep.paper.gridCm > 0 ? ` · grid ${Math.ceil(L.drawW / (prep.paper.gridCm * 10) - 1e-9)} × ${Math.ceil(L.drawH / (prep.paper.gridCm * 10) - 1e-9)} squares` : '';
    sizeOut.textContent = `Paper ${(L.paperW / 10).toFixed(1)} × ${(L.paperH / 10).toFixed(1)} cm (${L.orient}) → your drawing: ${(L.drawW / 10).toFixed(1)} × ${(L.drawH / 10).toFixed(1)} cm${grid}`;
  }

  main.appendChild(h('div.stack',
    h('div.row', h('h1', { style: { margin: 0 } }, 'Prepare your picture'), busy),
    h('div.prepare',
      h('div.stack', tabs, previewWrap,
        h('div.row',
          h('button.btn.small', { type: 'button', on: { click: () => { prep.rotation = (prep.rotation + 270) % 360; prep.crop = { x: 0, y: 0, w: 1, h: 1 }; buildOriented(); changed({ thumbs: true }); } } }, icon('rotl'), 'Rotate left'),
          h('button.btn.small', { type: 'button', on: { click: () => { prep.rotation = (prep.rotation + 90) % 360; prep.crop = { x: 0, y: 0, w: 1, h: 1 }; buildOriented(); changed({ thumbs: true }); } } }, icon('rotr'), 'Rotate right'),
          h('button.btn.small', { type: 'button', 'aria-pressed': String(prep.flipH), on: { click: (e) => { prep.flipH = !prep.flipH; e.currentTarget.setAttribute('aria-pressed', String(prep.flipH)); prep.crop = { ...prep.crop, x: 1 - prep.crop.x - prep.crop.w }; buildOriented(); changed({ thumbs: true }); } } }, icon('flip'), 'Mirror'),
          h('button.btn.small', { type: 'button', 'aria-pressed': String(prep.flipV), on: { click: (e) => { prep.flipV = !prep.flipV; e.currentTarget.setAttribute('aria-pressed', String(prep.flipV)); prep.crop = { ...prep.crop, y: 1 - prep.crop.y - prep.crop.h }; buildOriented(); changed({ thumbs: true }); } } }, 'Flip upside down'),
          h('button.btn.small', { type: 'button', on: { click: () => { prep.crop = { x: 0, y: 0, w: 1, h: 1 }; aspectMode = 'free'; placeCropBox(); changed({ thumbs: true }); } } }, 'Reset crop')),
        segmented('Crop shape', ASPECTS, aspectMode, (v) => { aspectMode = v; applyAspect(); })),
      h('div.card.stack',
        h('label.field', 'Project name', nameInput),
        h('div', h('h3', 'Drawing style'), styleGrid, styleDesc),
        segmented('Difficulty', Object.entries(shared.difficulty.levels).map(([k, v]) => [k, v.label]), prep.difficulty, (v) => { prep.difficulty = v; changed({ thumbs: true }); }),
        h('p.small.muted', 'Easier levels use fewer, bigger shapes and fewer steps.'),
        slider({ label: 'Brightness', min: -60, max: 60, value: prep.brightness, onInput: (v) => { prep.brightness = v; cropCanvas.style.filter = `brightness(${1 + v / 100}) contrast(${1 + prep.contrast / 100})`; changed(); } }),
        slider({ label: 'Contrast', min: -60, max: 80, value: prep.contrast, onInput: (v) => { prep.contrast = v; cropCanvas.style.filter = `brightness(${1 + prep.brightness / 100}) contrast(${1 + v / 100})`; changed(); } }),
        segmented('What will you draw with?', [['graphite', 'One pencil'], ['color', 'Colours']], prep.medium || 'graphite', (v) => { prep.medium = v; changed(); }),
        h('p.small.muted', 'Pencil adds pressure and shading guidance. Colours adds a palette and a colouring step.'),
        h('div.stack', h('h3', 'Paper'), h('label.field', 'Paper size', paperSel), customRow,
          h('button.btn.small', { type: 'button', on: { click: () => measureTool() } }, 'Not sure? Measure with a bank card'),
          segmented('Orientation', [['auto', 'Auto'], ['portrait', 'Portrait'], ['landscape', 'Landscape']], prep.paper.orientation, (v) => { prep.paper.orientation = v; if (aspectMode === 'paper') applyAspect(); updateSize(); changed(); }),
          slider({ label: 'Border (mm)', min: 5, max: 40, value: prep.paper.marginMm, onInput: (v) => { prep.paper.marginMm = v; updateSize(); changed(); } }),
          slider({ label: 'Grid squares (cm, 0 = off)', min: 0, max: 5, step: 0.5, value: prep.paper.gridCm || 0, format: (v) => (v ? v + ' cm' : 'off'), onInput: (v) => { prep.paper.gridCm = v; updateSize(); changed(); } }),
          sizeOut),
        h('details.adv', h('summary', 'Objects and background'),
          h('div.stack',
            toggle('Simplify background', prep.simplifyBackground, (v) => { prep.simplifyBackground = v; changed(); }, 'Hide busy backgrounds behind the main subject.'),
            h('p.small.muted', 'Find the separate objects in your picture, then remove ones you don\'t want to draw, or keep only one.'),
            h('button.btn.small', { type: 'button', on: { click: () => { refreshObjects(); setTab('style'); } } }, 'Find objects'),
            objChips)),
        h('details.adv', h('summary', 'Line and sketch settings'),
          h('div.stack',
            slider({ label: 'Line thickness', min: 1, max: 5, value: prep.lineThickness, onInput: (v) => { prep.lineThickness = v; changed(); } }),
            slider({ label: 'Sketch darkness', min: -40, max: 100, value: prep.sketchContrast, onInput: (v) => { prep.sketchContrast = v; changed(); } }),
            slider({ label: 'Guide opacity', min: 20, max: 100, value: prep.opacity, format: (v) => v + '%', onInput: (v) => { prep.opacity = v; p.view.opacity = v; changed(); } }))),
        h('button.btn.primary.block', { type: 'button', on: { click: async () => { clearTimeout(saveTimer); await app.saveProject(); nav('process'); } } }, 'Create my drawing steps')))));

  buildOriented();
  renderThumbs();
  updateSize();
  const ro = new ResizeObserver(placeCropBox);
  ro.observe(previewWrap);
  return () => { ro.disconnect(); clearTimeout(previewTimer); clearTimeout(saveTimer); thumbToken++; app.saveProject().catch(() => {}); };
}
