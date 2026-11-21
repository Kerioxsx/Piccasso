// Analyze: check your own drawing against the reference without AI: value
// histograms with plain advice, symmetry, matching grids, measurements,
// colour/value sampling and annotation pins.
import { app } from '../../state.js';
import { h, slider, toast, pickFile } from '../dom.js';
import { valueStats, compareValues, symmetry, measure, sample } from '../../core/inspect.js';
import { fitDims, resizeBox } from '../../core/image.js';
import { fileToCanvas, canvasToRGBA, rgbaToCanvas } from '../imageio.js';

const MAX = 900;
const PINS_KEY = 'piccasso.analyze.pins';

export async function renderAnalyze(main) {
  const imgs = { ref: null, drw: null };
  let tool = 'values';
  const opts = { axis: 0.5, symOf: 'drw', grid: 4, paperCm: 21 };
  let points = []; const measures = [];
  let pins = (() => { try { return JSON.parse(localStorage.getItem(PINS_KEY))?.[app.project?.id || 'none'] || []; } catch { return []; } })();
  const savePins = () => { try { const all = JSON.parse(localStorage.getItem(PINS_KEY) || '{}'); all[app.project?.id || 'none'] = pins; localStorage.setItem(PINS_KEY, JSON.stringify(all)); } catch { /* storage off */ } };

  const cv = { ref: h('canvas.an-canvas', { 'aria-label': 'Reference' }), drw: h('canvas.an-canvas', { 'aria-label': 'My drawing' }) };
  const out = h('div.stack');
  const slot = (k, label) => h('figure.an-slot', h('figcaption.small.muted', label), cv[k]);

  const fit = (rgba) => { const [w, hh] = fitDims(rgba.width, rgba.height, MAX); return w === rgba.width ? rgba : resizeBox(rgba, w, hh); };
  const load = async (k, capture) => {
    const f = await pickFile('image/*', capture);
    if (!f) return;
    try { imgs[k] = fit(canvasToRGBA(await fileToCanvas(f))); points = []; render(); } catch (e) { toast(e.message, { error: true }); }
  };

  function drawBase(k) {
    const c = cv[k], img = imgs[k];
    if (!img) { c.width = 400; c.height = 300; const g = c.getContext('2d'); g.fillStyle = '#f2efe8'; g.fillRect(0, 0, 400, 300); g.fillStyle = '#888'; g.font = '16px system-ui'; g.fillText(k === 'ref' ? 'Choose the reference' : 'Add a photo of your drawing', 40, 150); return null; }
    rgbaToCanvas(img, c);
    return c.getContext('2d');
  }

  function render() {
    for (const k of ['ref', 'drw']) {
      const g = drawBase(k); if (!g) continue;
      const img = imgs[k], k2 = img.width / 600;
      if (tool === 'grid') {
        g.strokeStyle = 'rgba(208,52,44,.85)'; g.lineWidth = Math.max(1, k2 * 1.4);
        for (let i = 1; i < opts.grid; i++) {
          const x = (img.width * i) / opts.grid, y = (img.height * i) / opts.grid;
          g.beginPath(); g.moveTo(x, 0); g.lineTo(x, img.height); g.moveTo(0, y); g.lineTo(img.width, y); g.stroke();
        }
        g.fillStyle = 'rgba(208,52,44,.9)'; g.font = `${Math.round(12 * Math.max(1, k2))}px system-ui`;
        for (let r = 0; r < opts.grid; r++) for (let c = 0; c < opts.grid; c++) g.fillText(`${String.fromCharCode(65 + r)}${c + 1}`, (img.width * c) / opts.grid + 4, (img.height * r) / opts.grid + 14 * Math.max(1, k2));
      }
      if (tool === 'symmetry' && k === opts.symOf) {
        const s = symmetry(img, opts.axis, 300);
        const heat = new ImageData(s.width, s.height);
        for (let i = 0; i < s.diff.length; i++) { const d = s.diff[i]; heat.data[i * 4] = 230; heat.data[i * 4 + 1] = 40; heat.data[i * 4 + 2] = 40; heat.data[i * 4 + 3] = Math.min(220, d * 2); }
        const tmp = h('canvas', { width: s.width, height: s.height }); tmp.getContext('2d').putImageData(heat, 0, 0);
        g.drawImage(tmp, 0, 0, img.width, img.height);
        g.strokeStyle = '#2a7a6c'; g.lineWidth = 2 * Math.max(1, k2); g.setLineDash([8, 6]);
        g.beginPath(); g.moveTo(img.width * opts.axis, 0); g.lineTo(img.width * opts.axis, img.height); g.stroke(); g.setLineDash([]);
        g.fillStyle = '#fff'; g.fillRect(8, 8, 200 * Math.max(1, k2), 30 * Math.max(1, k2)); g.fillStyle = '#222'; g.font = `bold ${Math.round(15 * Math.max(1, k2))}px system-ui`;
        g.fillText(`Symmetry: ${Math.round(s.score * 100)} %`, 16, 30 * Math.max(1, k2));
      }
      if (tool === 'measure') {
        g.strokeStyle = '#2a7a6c'; g.fillStyle = '#2a7a6c'; g.lineWidth = 2 * Math.max(1, k2);
        for (const m of measures.filter((m) => m.k === k)) { g.beginPath(); g.moveTo(...m.a); g.lineTo(...m.b); g.stroke(); g.font = `${Math.round(13 * Math.max(1, k2))}px system-ui`; g.fillText(m.label, (m.a[0] + m.b[0]) / 2 + 6, (m.a[1] + m.b[1]) / 2 - 6); }
        for (const p of points.filter((p) => p.k === k)) { g.beginPath(); g.arc(p.x, p.y, 5 * Math.max(1, k2), 0, Math.PI * 2); g.fill(); }
      }
      if (tool === 'annotate' && k === 'drw') {
        pins.forEach((p, i) => {
          const r = 13 * Math.max(1, k2), x = p.x * img.width, y = p.y * img.height;
          g.fillStyle = '#d0342c'; g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill();
          g.fillStyle = '#fff'; g.font = `bold ${Math.round(r * 1.1)}px system-ui`; g.textAlign = 'center'; g.textBaseline = 'middle'; g.fillText(String(i + 1), x, y); g.textAlign = 'start'; g.textBaseline = 'alphabetic';
        });
      }
    }
    renderOut();
  }

  function histCanvas(sr, sd) {
    const c = h('canvas.an-hist', { width: 512, height: 140, 'aria-label': 'Value histograms: reference in grey, drawing in green' }), g = c.getContext('2d');
    g.fillStyle = '#fff'; g.fillRect(0, 0, 512, 140);
    const grad = g.createLinearGradient(0, 0, 512, 0); grad.addColorStop(0, '#000'); grad.addColorStop(1, '#fff'); g.fillStyle = grad; g.fillRect(0, 128, 512, 12);
    const smooth = (hst) => hst.map((_, i) => { let a = 0, n = 0; for (let k = -4; k <= 4; k++) { const j = i + k; if (j >= 0 && j < 256) { a += hst[j]; n++; } } return a / n; });
    const plot = (raw, color) => { const hist = smooth(raw); const mx = Math.max(...hist.slice(2, 254)) || 1; g.strokeStyle = color; g.lineWidth = 2; g.beginPath(); hist.forEach((v, i) => { const y = 124 - Math.min(1, v / mx) * 118; i ? g.lineTo(i * 2, y) : g.moveTo(0, y); }); g.stroke(); };
    if (sr) plot(sr.hist, 'rgba(90,90,90,.9)');
    if (sd) plot(sd.hist, 'rgba(42,122,108,1)');
    return c;
  }

  function renderOut() {
    const sr = imgs.ref ? valueStats(imgs.ref) : null, sd = imgs.drw ? valueStats(imgs.drw) : null;
    if (tool === 'values') {
      const row = (name, f) => h('tr', h('th', name), h('td', sr ? f(sr) : '–'), h('td', sd ? f(sd) : '–'));
      out.replaceChildren(
        histCanvas(sr, sd),
        h('p.small.muted', 'Grey: reference · Green: your drawing. Left is black, right is white.'),
        h('table.an-table', h('thead', h('tr', h('th', ''), h('th', 'Reference'), h('th', 'Drawing'))),
          h('tbody', row('Average brightness', (s) => `${Math.round((s.mean / 255) * 100)} %`), row('Contrast', (s) => Math.round(s.contrast)), row('Darkest dark', (s) => s.dark), row('Lightest light', (s) => s.light),
            row('Dark | mid | light', (s) => `${Math.round((s.bands[0] + s.bands[1]) * 100)} | ${Math.round(s.bands[2] * 100)} | ${Math.round((s.bands[3] + s.bands[4]) * 100)} %`))),
        sr && sd ? h('ul.an-tips', ...compareValues(sr, sd).map((t) => h('li', { 'data-level': t.level }, t.text))) : h('p.small.muted', 'Add both pictures to get advice.'));
    } else if (tool === 'symmetry') {
      out.replaceChildren(
        h('div.chips', ...[['drw', 'My drawing'], ['ref', 'Reference']].map(([k, l]) => h('button.chip', { type: 'button', 'aria-pressed': String(opts.symOf === k), on: { click: () => { opts.symOf = k; render(); } } }, l))),
        slider({ label: 'Mirror line', min: 20, max: 80, value: opts.axis * 100, format: (v) => `${v} %`, onInput: (v) => { opts.axis = v / 100; render(); } }),
        h('p.small.muted', 'Red shows where the left side differs from the mirrored right side. Faces, vases and buildings are usually close to symmetric; big red areas show where to look.'));
    } else if (tool === 'grid') {
      out.replaceChildren(slider({ label: 'Grid cells per side', min: 2, max: 10, value: opts.grid, onInput: (v) => { opts.grid = v; render(); } }),
        h('p.small.muted', 'The same grid on both. Compare cell by cell: does each shape cross the same lines in your drawing as in the reference?'));
    } else if (tool === 'measure') {
      const last2 = measures.slice(-2);
      out.replaceChildren(
        h('p.small.muted', 'Click two points on either picture to measure. Measure the same thing on both to compare.'),
        h('label.field', 'Paper width (cm)', h('input', { type: 'number', min: 1, max: 200, value: opts.paperCm, on: { change: (e) => { opts.paperCm = +e.target.value || null; } } })),
        h('ol.an-measures', ...measures.map((m) => h('li', `${m.k === 'ref' ? 'Reference' : 'Drawing'}: ${m.label}`))),
        last2.length === 2 ? h('p', h('strong', 'Ratio of the last two: '), (measure(last2[1].a, last2[1].b, 1).px / Math.max(1e-9, measure(last2[0].a, last2[0].b, 1).px) * (imgs[last2[0].k].width / imgs[last2[1].k].width)).toFixed(2), ' (1.00 = same proportion)') : null,
        measures.length ? h('button.btn.small', { type: 'button', on: { click: () => { measures.length = 0; points = []; render(); } } }, 'Clear') : null);
    } else if (tool === 'sample') {
      out.replaceChildren(h('p.small.muted', 'Click anywhere on either picture to read its colour and value.'), sampleBox);
    } else if (tool === 'annotate') {
      out.replaceChildren(h('p.small.muted', 'Click on your drawing to pin a note: things to fix, things that worked. Pins are saved with this project on this device.'),
        h('ol.an-pins', ...pins.map((p, i) => h('li', h('input', { type: 'text', value: p.text, 'aria-label': `Note ${i + 1}`, on: { input: (e) => { p.text = e.target.value; savePins(); } } }),
          h('button.icon-btn.small', { type: 'button', 'aria-label': `Remove note ${i + 1}`, on: { click: () => { pins.splice(i, 1); savePins(); render(); } } }, '×')))));
    }
  }
  const sampleBox = h('div.an-sample');

  for (const k of ['ref', 'drw']) {
    cv[k].addEventListener('click', (e) => {
      const img = imgs[k]; if (!img) return;
      const r = cv[k].getBoundingClientRect(), x = ((e.clientX - r.left) / r.width) * img.width, y = ((e.clientY - r.top) / r.height) * img.height;
      if (tool === 'measure') {
        points.push({ k, x, y });
        const mine = points.filter((p) => p.k === k);
        if (mine.length === 2) {
          const m = measure([mine[0].x, mine[0].y], [mine[1].x, mine[1].y], img.width, opts.paperCm);
          measures.push({ k, a: [mine[0].x, mine[0].y], b: [mine[1].x, mine[1].y], label: `${Math.round(m.ofWidth * 100)} % of the width${m.cm ? ` · ${m.cm.toFixed(1)} cm` : ''}` });
          points = points.filter((p) => p.k !== k);
        }
        render();
      } else if (tool === 'sample') {
        const s = sample(img, x, y);
        sampleBox.replaceChildren(h('span.an-swatch', { style: { background: s.hex } }), h('div', h('strong', s.hex), h('div.small', `Value ${s.value} % (0 = black, 100 = white) · RGB ${s.r}, ${s.g}, ${s.b}`), h('div.small.muted', k === 'ref' ? 'Reference' : 'Drawing')));
      } else if (tool === 'annotate' && k === 'drw') {
        pins.push({ x: x / img.width, y: y / img.height, text: '' }); savePins(); render();
        setTimeout(() => { const ins = out.querySelectorAll('.an-pins input'); if (ins.length) ins[ins.length - 1].focus(); }, 30);
      }
    });
  }

  const tools = h('div.chips', { role: 'group', 'aria-label': 'Tool' });
  const renderTools = () => tools.replaceChildren(...[['values', 'Values'], ['symmetry', 'Symmetry'], ['grid', 'Grid'], ['measure', 'Measure'], ['sample', 'Colour sampler'], ['annotate', 'Notes']].map(([k, l]) =>
    h('button.chip', { type: 'button', 'aria-pressed': String(k === tool), on: { click: () => { tool = k; points = []; renderTools(); render(); } } }, l)));

  main.appendChild(h('div.stack',
    h('h1', 'Analyze my drawing'),
    h('p.muted', 'Check values, symmetry and proportions of your drawing against the reference. Everything is measured, nothing is guessed: no AI.'),
    h('div.row',
      app.src ? h('button.btn.small', { type: 'button', on: { click: () => { imgs.ref = fit(app.src); render(); } } }, 'Reference: current project') : null,
      h('button.btn.small', { type: 'button', on: { click: () => load('ref') } }, 'Reference: open picture'),
      h('button.btn.small.primary', { type: 'button', on: { click: () => load('drw', 'environment') } }, 'Photo of my drawing')),
    tools,
    h('div.an-grid', slot('ref', 'Reference'), slot('drw', 'My drawing')),
    out));

  if (app.src) imgs.ref = fit(app.src);
  renderTools(); render();
}
