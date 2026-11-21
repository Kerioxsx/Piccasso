// Optional camera-assisted comparison. Runs entirely on this device: the photo
// is never uploaded. The user confirms the paper corners, the photo is warped
// flat, pencil lines are extracted and compared with the guide.
import { app } from '../../state.js';
import { h, icon, nav, pickFile, toast, slider, segmented, announce } from '../dom.js';
import { fileToCanvas, canvasToRGBA, rgbaToCanvas } from '../imageio.js';
import { detectPaper, warpGray, orderCorners } from '../../core/geometry.js';
import { toGray, grayToRGBA } from '../../core/image.js';
import { paperLayout } from '../../core/steps.js';
import { compareDrawing, guideMask } from '../../core/compare.js';
import { drawLayers } from '../canvas.js';

export function renderCompare(main) {
  const p = app.project;
  const layout = paperLayout(p.prep.paper, app.shared.paper.presets, p.analysis.aspect, app.shared.paper.defaultMarginMm);
  const body = h('div.stack');
  main.appendChild(h('div.stack', { style: { maxWidth: '900px', margin: '0 auto' } },
    h('div.row', h('button.btn.small', { type: 'button', on: { click: () => nav('workspace') } }, icon('prev'), 'Back to steps'), h('h1', { style: { margin: 0 } }, 'Check your drawing')),
    body));

  function intro() {
    body.innerHTML = '';
    body.appendChild(h('div.card.stack',
      h('p', 'Take a photo of your paper and Piccasso will point out a few things to check, such as size and placement. It\'s friendly feedback about structure, not a score.'),
      h('p.notice.info', '🔒 Your photo stays on this device. It is not uploaded or saved unless you export it.'),
      h('ul.small.muted', h('li', 'Hold the phone straight above the paper.'), h('li', 'Make sure all four corners of the paper are visible.'), h('li', 'A darker table behind the paper helps.')),
      h('div.row',
        h('button.btn.primary', { type: 'button', on: { click: async () => load(await pickFile('image/*', 'environment')) } }, icon('camera'), 'Take a photo'),
        h('button.btn', { type: 'button', on: { click: async () => load(await pickFile('image/*')) } }, icon('image'), 'Choose a photo'))));
  }

  async function load(file) {
    if (!file) return;
    let canvas;
    try { canvas = await fileToCanvas(file); } catch (e) { toast(e.message, { error: true }); return; }
    const rgba = canvasToRGBA(canvas);
    const gray = toGray(rgba);
    const s = 240 / Math.max(gray.width, gray.height);
    const small = toGray(canvasToRGBA((() => { const c = document.createElement('canvas'); c.width = Math.round(canvas.width * s); c.height = Math.round(canvas.height * s); c.getContext('2d').drawImage(canvas, 0, 0, c.width, c.height); return c; })()));
    const det = detectPaper(small);
    let corners = det && det.confidence > 0.3 ? det.corners : [[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]];
    corners_ui(canvas, gray, corners, !!(det && det.confidence > 0.3));
  }

  function corners_ui(photo, gray, corners, auto) {
    body.innerHTML = '';
    const view = h('canvas', { 'aria-label': 'Your photo with paper corners' });
    const stage = h('div.compare-stage', view);
    const hs = corners.map((_, i) => h('div.ar-handle', { role: 'slider', tabIndex: 0, 'aria-label': `Paper corner ${['top left', 'top right', 'bottom right', 'bottom left'][i]}` }, ['TL', 'TR', 'BR', 'BL'][i]));
    stage.append(...hs);
    const s = Math.min(1, 1100 / Math.max(photo.width, photo.height));
    view.width = Math.round(photo.width * s); view.height = Math.round(photo.height * s);
    const redraw = () => {
      const ctx = view.getContext('2d');
      ctx.drawImage(photo, 0, 0, view.width, view.height);
      ctx.strokeStyle = '#7cc4b3'; ctx.lineWidth = 3; ctx.beginPath();
      corners.forEach(([x, y], i) => (i ? ctx.lineTo : ctx.moveTo).call(ctx, x * view.width, y * view.height));
      ctx.closePath(); ctx.stroke();
      const r = view.getBoundingClientRect();
      hs.forEach((hd, i) => { hd.style.left = corners[i][0] * r.width + 'px'; hd.style.top = corners[i][1] * r.height + 'px'; });
    };
    hs.forEach((hd, i) => {
      hd.addEventListener('pointerdown', (e) => { e.preventDefault(); hd.setPointerCapture(e.pointerId); hd.dataset.d = '1'; });
      hd.addEventListener('pointermove', (e) => { if (!hd.dataset.d) return; const r = view.getBoundingClientRect(); corners[i] = [Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)), Math.min(1, Math.max(0, (e.clientY - r.top) / r.height))]; redraw(); });
      hd.addEventListener('pointerup', () => { delete hd.dataset.d; });
      hd.addEventListener('keydown', (e) => { const d = { ArrowLeft: [-0.01, 0], ArrowRight: [0.01, 0], ArrowUp: [0, -0.01], ArrowDown: [0, 0.01] }[e.key]; if (d) { e.preventDefault(); corners[i] = [corners[i][0] + d[0], corners[i][1] + d[1]]; redraw(); } });
    });
    body.append(
      h('p', auto ? 'We found the paper. Check that the four round handles sit on the paper\'s corners.' : 'Drag the four round handles onto the corners of your paper.'),
      stage,
      h('div.row',
        h('button.btn.primary', { type: 'button', on: { click: () => results(gray, orderCorners(corners.map((c) => c.slice()))) } }, icon('check'), 'Compare'),
        h('button.btn', { type: 'button', on: { click: intro } }, 'Different photo')));
    requestAnimationFrame(redraw);
    const ro = new ResizeObserver(redraw); ro.observe(stage);
  }

  function results(gray, corners) {
    const L = 600;
    const W = layout.paperW >= layout.paperH ? L : Math.round((L * layout.paperW) / layout.paperH);
    const H = layout.paperW >= layout.paperH ? Math.round((L * layout.paperH) / layout.paperW) : L;
    const flat = warpGray(gray, corners, W, H);
    const guide = guideMask(p.analysis, layout, W, H);
    const res = compareDrawing(flat, guide, app.shared.strings);
    let modeV = 'overlay', alpha = 0.6;
    const photoC = rgbaToCanvas(grayToRGBA(flat));
    const out = h('canvas', { width: W, height: H, 'aria-label': 'Your drawing with the guide on top' });
    const draw = () => {
      const ctx = out.getContext('2d');
      if (modeV === 'side') {
        out.width = W * 2 + 12; out.height = H;
        ctx.fillStyle = '#222'; ctx.fillRect(0, 0, out.width, H);
        ctx.drawImage(photoC, 0, 0);
        ctx.fillStyle = '#fff'; ctx.fillRect(W + 12, 0, W, H);
        drawLayers(ctx, p.analysis, ['silhouette', 'internal'], { x: W + 12 + (layout.drawX / layout.paperW) * W, y: (layout.drawY / layout.paperH) * H, w: (layout.drawW / layout.paperW) * W, h: (layout.drawH / layout.paperH) * H });
      } else {
        out.width = W; out.height = H;
        ctx.drawImage(photoC, 0, 0);
        drawLayers(ctx, p.analysis, ['silhouette', 'internal'], { x: (layout.drawX / layout.paperW) * W, y: (layout.drawY / layout.paperH) * H, w: (layout.drawW / layout.paperW) * W, h: (layout.drawH / layout.paperH) * H }, { alpha, labels: false });
      }
    };
    body.innerHTML = '';
    body.append(
      h('div.card.stack',
        h('h2', 'Things to look at'),
        h('ul.observations', res.observations.map((o) => h('li', o))),
        res.metrics ? h('p.small.muted', `Line overlap with the guide: about ${Math.round(res.metrics.coverage * 100)}%. This is only a rough guide; small differences are completely normal.`) : null),
      segmented('View', [['overlay', 'Guide on top'], ['side', 'Side by side']], modeV, (v) => { modeV = v; draw(); }),
      slider({ label: 'Guide strength', min: 0, max: 100, value: 60, format: (v) => v + '%', onInput: (v) => { alpha = v / 100; draw(); } }),
      h('div.compare-stage', out),
      h('div.row',
        h('button.btn', { type: 'button', on: { click: intro } }, 'Try another photo'),
        h('button.btn.primary', { type: 'button', on: { click: () => nav('workspace') } }, 'Back to my steps')));
    draw();
    announce('Comparison ready. ' + res.observations.join(' '));
  }

  intro();
}
