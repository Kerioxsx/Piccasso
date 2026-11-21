// Guides: perspective (1/2/3-point with draggable vanishing points and horizon),
// face and figure proportions, composition grids, a line-weight chart and a
// magnifier, over your picture or blank paper. Export or print with the guides.
import { app } from '../../state.js';
import { h, slider, segmented, toggle, toast, pickFile, download } from '../dom.js';
import { perspectiveGuide, defaultVps, faceGuide, figureGuide, compositionGuide, LINE_WEIGHTS, mmToPx } from '../../core/guides.js';
import { fitDims, resizeBox } from '../../core/image.js';
import { fileToCanvas, canvasToRGBA, rgbaToCanvas } from '../imageio.js';

const MAX = 1400;

export async function renderGuides(main) {
  let pic = null;                         // canvas of the picture (or null = blank paper)
  let W = 1200, H = 900;                  // picture size in px
  let mode = 'perspective';
  const st = {
    points: 2, rays: 14, ...defaultVps(2, W, H),
    heads: 8, comp: 'thirds',
    box: null,                              // face/figure template box
    color: '#d0342c', width: 1.5, guideOpacity: 0.9, picOpacity: 1, labels: true, loupe: false,
  };
  // room around the picture for vanishing points that lie outside it (the third one sits far below)
  const pad = () => ({ x: Math.round(W * (st.points === 1 ? 0.05 : 0.3)), top: Math.round(H * 0.1), bottom: Math.round(H * (st.points === 3 ? 0.6 : 0.1)) });

  const canvas = h('canvas.guides-canvas', { 'aria-label': 'Picture with drawing guides' });
  const loupe = h('canvas.guides-loupe', { width: 220, height: 220, hidden: true, 'aria-hidden': 'true' });
  const stage = h('div.guides-stage', canvas, loupe);
  const ctl = h('div.stack');
  const g = canvas.getContext('2d');

  const setPicture = (rgba) => {
    if (rgba) {
      const [w, hh] = fitDims(rgba.width, rgba.height, MAX);
      pic = rgbaToCanvas(w === rgba.width ? rgba : resizeBox(rgba, w, hh));
      W = pic.width; H = pic.height;
    } else { pic = null; W = 1200; H = 900; }
    Object.assign(st, defaultVps(st.points, W, H));
    st.box = null;
    draw();
  };

  const defaultBox = () => (mode === 'face' ? { x: W * 0.35, y: H * 0.15, w: W * 0.3, h: W * 0.3 * 1.4 } : { x: W * 0.4, y: H * 0.05, w: W * 0.2, h: H * 0.9 });

  function shapes() {
    if (mode === 'perspective') return perspectiveGuide({ points: st.points, vps: st.vps, horizonY: st.horizonY, count: st.rays }, W, H);
    if (mode === 'face') return faceGuide(st.box || (st.box = defaultBox()));
    if (mode === 'figure') return figureGuide(st.box || (st.box = defaultBox()), st.heads);
    if (mode === 'composition') return compositionGuide(st.comp, W, H);
    return [];
  }

  function draw() {
    const p = pad();
    const showPad = mode === 'perspective';
    const ox = showPad ? p.x : 0, oy = showPad ? p.top : 0;
    canvas.width = W + (showPad ? 2 * p.x : 0);
    canvas.height = H + (showPad ? p.top + p.bottom : 0);
    g.fillStyle = getComputedStyle(document.body).getPropertyValue('--bg') || '#eee';
    g.fillRect(0, 0, canvas.width, canvas.height);
    g.save(); g.translate(ox, oy);
    g.fillStyle = '#fff'; g.fillRect(0, 0, W, H);
    if (mode === 'weights') { drawWeights(); g.restore(); return; }
    if (pic) { g.globalAlpha = st.picOpacity; g.drawImage(pic, 0, 0); g.globalAlpha = 1; }
    g.strokeStyle = 'rgba(0,0,0,.25)'; g.strokeRect(0.5, 0.5, W - 1, H - 1);
    g.globalAlpha = st.guideOpacity;
    g.strokeStyle = st.color; g.fillStyle = st.color;
    const k = Math.max(1, W / 900);
    for (const s of shapes()) {
      g.lineWidth = st.width * (s.w || 1) * k;
      g.setLineDash(s.kind === 'measure' ? [6 * k, 5 * k] : []);
      if (s.type === 'line') { g.beginPath(); g.moveTo(...s.a); g.lineTo(...s.b); g.stroke(); }
      else if (s.type === 'ellipse') { g.beginPath(); g.ellipse(s.c[0], s.c[1], s.rx, s.ry, 0, 0, Math.PI * 2); g.stroke(); }
      else if (s.type === 'label' && st.labels) { g.font = `${Math.round(13 * k)}px system-ui, sans-serif`; halo(s.text, s.at, k); }
      else if (s.type === 'vp') {
        g.setLineDash([]); g.lineWidth = 2 * k;
        g.beginPath(); g.arc(s.c[0], s.c[1], 11 * k, 0, Math.PI * 2); g.stroke();
        g.beginPath(); g.arc(s.c[0], s.c[1], 3 * k, 0, Math.PI * 2); g.fill();
        if (st.labels) { g.font = `bold ${Math.round(13 * k)}px system-ui, sans-serif`; halo(s.label, [s.c[0] + 14 * k, s.c[1] - 10 * k], k); }
      }
    }
    g.setLineDash([]);
    // handles for the template box
    if ((mode === 'face' || mode === 'figure') && st.box) {
      const b = st.box; g.lineWidth = 1.5 * k; g.strokeRect(b.x, b.y, b.w, b.h);
      g.fillRect(b.x + b.w - 7 * k, b.y + b.h - 7 * k, 14 * k, 14 * k);
    }
    g.globalAlpha = 1;
    g.restore();
  }

  // text with a white outline, readable over any picture
  function halo(text, at, k) {
    g.save(); g.lineWidth = 3.5 * k; g.strokeStyle = 'rgba(255,255,255,.9)'; g.lineJoin = 'round'; g.setLineDash([]);
    g.strokeText(text, ...at); g.restore(); g.fillText(text, ...at);
  }

  function drawWeights() {
    const k = Math.min(W / 900, H / 800);
    g.fillStyle = '#222'; g.font = `bold ${Math.round(22 * k)}px system-ui, sans-serif`;
    g.fillText('Line weights', 40 * k, 60 * k);
    g.font = `${Math.round(14 * k)}px system-ui, sans-serif`;
    g.fillText('Thin, light lines recede; thick, dark lines come forward. Print at 100 % to see real widths.', 40 * k, 90 * k);
    LINE_WEIGHTS.forEach((r, i) => {
      const y = (150 + i * 130) * k;
      g.strokeStyle = '#222'; g.lineCap = 'round'; g.lineWidth = Math.max(1, mmToPx(r.mm) * k * 2);
      g.beginPath(); g.moveTo(40 * k, y); g.bezierCurveTo(200 * k, y - 40 * k, 300 * k, y + 40 * k, 460 * k, y); g.stroke();
      g.fillStyle = '#222'; g.font = `bold ${Math.round(16 * k)}px system-ui, sans-serif`;
      g.fillText(`${r.mm} mm · ${r.grade}`, 500 * k, y - 6 * k);
      g.font = `${Math.round(14 * k)}px system-ui, sans-serif`;
      g.fillText(r.use, 500 * k, y + 16 * k);
    });
  }

  // ----- dragging: vanishing points, horizon, template box -----
  const toWorld = (e) => {
    const r = canvas.getBoundingClientRect(), p = pad(), showPad = mode === 'perspective';
    const sx = canvas.width / r.width;
    return [(e.clientX - r.left) * sx - (showPad ? p.x : 0), (e.clientY - r.top) * sx - (showPad ? p.top : 0)];
  };
  let drag = null;
  canvas.addEventListener('pointerdown', (e) => {
    const [x, y] = toWorld(e), near = 22 * Math.max(1, W / 900);
    drag = null;
    if (mode === 'perspective') {
      const i = st.vps.slice(0, st.points).findIndex((v) => Math.hypot(v[0] - x, v[1] - y) < near * 1.4);
      if (i >= 0) drag = { kind: 'vp', i };
      else if (Math.abs(y - st.horizonY) < near) drag = { kind: 'horizon' };
    } else if ((mode === 'face' || mode === 'figure') && st.box) {
      const b = st.box;
      if (Math.hypot(x - (b.x + b.w), y - (b.y + b.h)) < near * 1.4) drag = { kind: 'resize' };
      else if (x > b.x && x < b.x + b.w && y > b.y && y < b.y + b.h) drag = { kind: 'move', dx: x - b.x, dy: y - b.y };
    }
    if (drag) { canvas.setPointerCapture(e.pointerId); e.preventDefault(); }
  });
  canvas.addEventListener('pointermove', (e) => {
    if (st.loupe) showLoupe(e);
    if (!drag) return;
    const [x, y] = toWorld(e);
    if (drag.kind === 'vp') {
      const v = st.vps[drag.i];
      v[0] = x;
      // VP1/VP2 sit on the horizon; moving one vertically moves the horizon too
      if (drag.i < 2) { st.horizonY = y; st.vps.forEach((p, j) => { if (j < 2) p[1] = y; }); } else v[1] = y;
    } else if (drag.kind === 'horizon') { st.horizonY = y; st.vps.forEach((p, j) => { if (j < 2) p[1] = y; }); }
    else if (drag.kind === 'move') { st.box.x = x - drag.dx; st.box.y = y - drag.dy; }
    else if (drag.kind === 'resize') {
      const b = st.box, aspect = mode === 'face' ? 1.4 : null;
      b.w = Math.max(30, x - b.x); b.h = aspect ? b.w * aspect : Math.max(60, y - b.y);
    }
    draw();
  });
  canvas.addEventListener('pointerup', () => { drag = null; });
  canvas.addEventListener('pointerleave', () => { loupe.hidden = true; });

  function showLoupe(e) {
    const r = canvas.getBoundingClientRect(), sr = stage.getBoundingClientRect(), sx = canvas.width / r.width;
    const cx = (e.clientX - r.left) * sx, cy = (e.clientY - r.top) * sx, z = 3, L = loupe.width;
    const lg = loupe.getContext('2d');
    lg.fillStyle = '#fff'; lg.fillRect(0, 0, L, L);
    lg.imageSmoothingEnabled = false;
    lg.drawImage(canvas, cx - L / (2 * z), cy - L / (2 * z), L / z, L / z, 0, 0, L, L);
    lg.strokeStyle = 'rgba(0,0,0,.5)'; lg.beginPath(); lg.moveTo(L / 2, L / 2 - 10); lg.lineTo(L / 2, L / 2 + 10); lg.moveTo(L / 2 - 10, L / 2); lg.lineTo(L / 2 + 10, L / 2); lg.stroke();
    loupe.style.left = `${e.clientX - sr.left + 24}px`; loupe.style.top = `${e.clientY - sr.top - L - 12}px`;
    loupe.hidden = false;
  }

  // ----- controls -----
  function renderControls() {
    const per = mode === 'perspective' ? [
      segmented('Vanishing points', [['1', '1-point'], ['2', '2-point'], ['3', '3-point']], String(st.points), (v) => { st.points = +v; Object.assign(st, defaultVps(st.points, W, H)); draw(); }),
      slider({ label: 'Lines from each point', min: 4, max: 40, value: st.rays, onInput: (v) => { st.rays = v; draw(); } }),
      h('p.small.muted', 'Drag the round vanishing points and the horizon line. Points may sit outside the picture, like in real scenes.'),
    ] : mode === 'face' ? [
      h('p.small.muted', 'Classical front-view proportions: eyes halfway down the head, brows · nose · chin in thirds, five eye-widths across. Drag the box onto the face; drag its corner to resize.'),
    ] : mode === 'figure' ? [
      segmented('Figure height', [['6', '6 heads (child)'], ['7', '7 heads'], ['8', '8 heads (ideal)']], String(st.heads), (v) => { st.heads = +v; draw(); }),
      h('p.small.muted', 'Drag the box from head to feet; drag its corner to resize.'),
    ] : mode === 'composition' ? [
      segmented('Grid', [['thirds', 'Thirds'], ['golden', 'Golden ratio'], ['symmetry', 'Symmetry'], ['diagonals', 'Diagonals']], st.comp, (v) => { st.comp = v; draw(); }),
    ] : [h('p.small.muted', 'Practise each weight a few times: same pencil, different pressure and grade.')];
    ctl.replaceChildren(...per);
  }

  const exportPng = async (guidesOnly) => {
    const keep = st.picOpacity;
    if (guidesOnly) st.picOpacity = 0;
    draw();
    // export only the picture area (not the margin used for far vanishing points)
    const p = pad(), showPad = mode === 'perspective';
    const out = document.createElement('canvas'); out.width = W; out.height = H;
    out.getContext('2d').drawImage(canvas, showPad ? p.x : 0, showPad ? p.top : 0, W, H, 0, 0, W, H);
    st.picOpacity = keep; draw();
    download(await new Promise((r) => out.toBlob(r, 'image/png')), `piccasso-guide-${mode}${guidesOnly ? '-only' : ''}.png`);
  };

  const openImage = async () => {
    const f = await pickFile('image/*');
    if (!f) return;
    try { setPicture(canvasToRGBA(await fileToCanvas(f))); } catch (e) { toast(e.message, { error: true }); }
  };

  const modes = h('div.chips', { role: 'group', 'aria-label': 'Guide' });
  const renderModes = () => modes.replaceChildren(...[['perspective', 'Perspective'], ['face', 'Face proportions'], ['figure', 'Figure proportions'], ['composition', 'Composition'], ['weights', 'Line weights']].map(([k, l]) =>
    h('button.chip', { type: 'button', 'aria-pressed': String(k === mode), on: { click: () => { mode = k; st.box = null; renderModes(); renderControls(); draw(); } } }, l)));

  main.appendChild(h('div.studio',
    h('div.studio-side.stack',
      h('h1', 'Guides'),
      h('p.small.muted', 'Construction guides over your picture or blank paper.'),
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: openImage } }, 'Open a picture'),
        app.src ? h('button.btn.small', { type: 'button', on: { click: () => setPicture(app.src) } }, 'Use current project') : null,
        h('button.btn.small', { type: 'button', on: { click: () => setPicture(null) } }, 'Blank paper')),
      modes, ctl,
      h('details', h('summary', 'Look'),
        h('label.field', 'Guide colour', h('input', { type: 'color', value: st.color, on: { input: (e) => { st.color = e.target.value; draw(); } } })),
        slider({ label: 'Guide thickness', min: 0.5, max: 5, step: 0.5, value: st.width, onInput: (v) => { st.width = v; draw(); } }),
        slider({ label: 'Guide opacity', min: 10, max: 100, value: st.guideOpacity * 100, format: (v) => v + '%', onInput: (v) => { st.guideOpacity = v / 100; draw(); } }),
        slider({ label: 'Picture opacity', min: 0, max: 100, value: 100, format: (v) => v + '%', onInput: (v) => { st.picOpacity = v / 100; draw(); } }),
        toggle('Labels', true, (v) => { st.labels = v; draw(); })),
      toggle('Magnifier', false, (v) => { st.loupe = v; if (!v) loupe.hidden = true; }, 'Hover or drag over the picture to see details 3× bigger.'),
      h('div.row',
        h('button.btn.small.primary', { type: 'button', on: { click: () => exportPng(false) } }, 'Save PNG'),
        h('button.btn.small', { type: 'button', on: { click: () => exportPng(true) } }, 'Guides only'),
        h('button.btn.small', { type: 'button', on: { click: () => window.print() } }, 'Print'))),
    stage));

  renderModes(); renderControls();
  setPicture(app.src || null);
}
