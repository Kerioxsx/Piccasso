// Studio: turn any picture into a drawing reference with classic image
// processing (sketch, edges, contours, values, hatching, halftone, silhouette,
// negative space, background removal…). Optional: apply only inside a region,
// mirror, brightness/contrast, 2× resolution, export as PNG.
import { app } from '../../state.js';
import { h, slider, toggle, toast, pickFile, download, announce } from '../dom.js';
import { CONVERSIONS, preAdjust, inRegion, upscale } from '../../core/filters.js';
import { fitDims, resizeBox } from '../../core/image.js';
import { fileToCanvas, canvasToRGBA, rgbaToCanvas } from '../imageio.js';

const WORK_MAX = 1200; // processing size (export can be doubled)

export async function renderStudio(main) {
  let src = null;            // working RGBA
  let mode = 'sketch';
  const params = {};          // per conversion
  const pre = { brightness: 0, contrast: 0, mirror: false };
  let region = null;          // {x, y, w, h} in working pixels
  let selecting = false, dragStart = null;
  let result = null;

  const canvas = h('canvas.studio-canvas', { 'aria-label': 'Converted reference' });
  const box = h('div.studio-stage', canvas);
  const sel = h('div.studio-sel', { hidden: true });
  box.appendChild(sel);
  const paramBox = h('div.stack');
  const chips = h('div.chips', { role: 'group', 'aria-label': 'Conversion' });
  const status = h('p.small.muted');

  const setSrc = (rgba) => {
    const [w, hh] = fitDims(rgba.width, rgba.height, WORK_MAX);
    src = w === rgba.width ? rgba : resizeBox(rgba, w, hh);
    region = null; sel.hidden = true; run();
  };

  const params0 = (key) => {
    const c = CONVERSIONS[key];
    if (!params[key]) params[key] = Object.fromEntries(Object.entries(c.params || {}).map(([k, v]) => [k, v[2]]));
    return params[key];
  };

  function renderParams() {
    const c = CONVERSIONS[mode], p = params0(mode);
    paramBox.replaceChildren(...Object.entries(c.params || {}).map(([k, [min, max, def]]) => {
      const step = [min, max, def].every(Number.isInteger) ? 1 : 0.05;
      return slider({ label: k.replace(/([A-Z])/g, ' $1').replace(/^./, (s) => s.toUpperCase()), min, max, step, value: p[k],
        format: (v) => (step < 1 ? (+v).toFixed(2) : v), onInput: (v) => { p[k] = v; schedule(); } });
    }));
  }

  function renderChips() {
    chips.replaceChildren(...Object.entries(CONVERSIONS).map(([k, c]) => h('button.chip', { type: 'button', 'aria-pressed': String(k === mode),
      on: { click: () => { mode = k; renderChips(); renderParams(); run(); announce(c.label); } } }, c.label)));
  }

  let timer = 0;
  const schedule = () => { clearTimeout(timer); timer = setTimeout(run, 120); };
  function run() {
    if (!src) return;
    const t0 = performance.now();
    const base = preAdjust(src, pre);
    const c = CONVERSIONS[mode];
    result = inRegion(base, region, (img) => c.run(img, params0(mode)));
    rgbaToCanvas(result, canvas);
    status.textContent = `${c.label} · ${result.width}×${result.height} px · ${Math.round(performance.now() - t0)} ms`;
  }

  // region selection: drag on the picture
  const toImg = (e) => {
    const r = canvas.getBoundingClientRect();
    return [Math.round(((e.clientX - r.left) / r.width) * canvas.width), Math.round(((e.clientY - r.top) / r.height) * canvas.height)];
  };
  const showSel = () => {
    if (!region) { sel.hidden = true; return; }
    const r = canvas.getBoundingClientRect(), b = box.getBoundingClientRect();
    const k = r.width / canvas.width;
    Object.assign(sel.style, { left: `${r.left - b.left + region.x * k}px`, top: `${r.top - b.top + region.y * k}px`, width: `${region.w * k}px`, height: `${region.h * k}px` });
    sel.hidden = false;
  };
  canvas.addEventListener('pointerdown', (e) => { if (!selecting) return; dragStart = toImg(e); canvas.setPointerCapture(e.pointerId); });
  canvas.addEventListener('pointermove', (e) => {
    if (!dragStart) return;
    const [x, y] = toImg(e);
    region = { x: Math.min(x, dragStart[0]), y: Math.min(y, dragStart[1]), w: Math.abs(x - dragStart[0]), h: Math.abs(y - dragStart[1]) };
    showSel();
  });
  canvas.addEventListener('pointerup', () => { if (!dragStart) return; dragStart = null; if (region && region.w > 4 && region.h > 4) run(); else { region = null; showSel(); } });

  const regionBtn = h('button.btn.small', { type: 'button', 'aria-pressed': 'false', on: { click: () => {
    selecting = !selecting; regionBtn.setAttribute('aria-pressed', String(selecting));
    if (selecting) toast('Drag over the part of the picture to convert.');
  } } }, 'Convert only a region');
  const clearRegion = h('button.btn.small', { type: 'button', on: { click: () => { region = null; showSel(); run(); } } }, 'Whole picture');

  const exportPng = async (factor) => {
    if (!result) return;
    const out = factor > 1 ? upscale(result, factor) : result;
    const c = rgbaToCanvas(out);
    const blob = await new Promise((r) => c.toBlob(r, 'image/png'));
    download(blob, `piccasso-${mode}${factor > 1 ? '-2x' : ''}.png`);
  };

  const openImage = async () => {
    const f = await pickFile('image/*');
    if (!f) return;
    try { setSrc(canvasToRGBA(await fileToCanvas(f))); } catch (e) { toast(e.message, { error: true }); }
  };

  main.appendChild(h('div.studio',
    h('div.studio-side.stack',
      h('h1', 'Studio'),
      h('p.small.muted', 'Turn a picture into a drawing reference. Everything is classic image processing on this device: no AI.'),
      h('div.row', h('button.btn.small', { type: 'button', on: { click: openImage } }, 'Open a picture'),
        app.src ? h('button.btn.small', { type: 'button', on: { click: () => setSrc(app.src) } }, 'Use current project') : null),
      chips,
      paramBox,
      h('details', { open: true }, h('summary', 'Adjust the picture first'),
        slider({ label: 'Brightness', min: -100, max: 100, value: 0, onInput: (v) => { pre.brightness = v; schedule(); } }),
        slider({ label: 'Contrast', min: -100, max: 100, value: 0, onInput: (v) => { pre.contrast = v; schedule(); } }),
        toggle('Mirror (flip left-right)', false, (v) => { pre.mirror = v; run(); }, 'Drawing the mirrored picture shows proportion mistakes your eye stopped noticing.')),
      h('div.row', regionBtn, clearRegion),
      h('div.row',
        h('button.btn.small.primary', { type: 'button', on: { click: () => exportPng(1) } }, 'Save PNG'),
        h('button.btn.small', { type: 'button', on: { click: () => exportPng(2) } }, 'Save 2× (sharper print)'),
        h('button.btn.small', { type: 'button', on: { click: () => window.print() } }, 'Print')),
      status),
    box));

  renderChips(); renderParams();
  if (app.src) setSrc(app.src);
  else status.textContent = 'Open a picture to start.';
  const onResize = () => showSel();
  window.addEventListener('resize', onResize);
  return () => window.removeEventListener('resize', onResize);
}
