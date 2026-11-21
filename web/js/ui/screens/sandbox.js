// 3D construction sandbox: place cubes, spheres, cylinders, cones and pyramids,
// orbit the camera (perspective or orthographic), move the light, and see the
// shapes as solid, wireframe or silhouette, with shadows, construction lines and
// vanishing points. Scenes can be saved on the device and exported as PNG.
import { h, slider, segmented, toggle, toast, download, promptDialog } from '../dom.js';
import { buildScene, defaultScene, lightDir, cameraBasis, project, vanishingPoint, objectAxes, v3 } from '../../core/sandbox3d.js';

const SCENES_KEY = 'piccasso.sandbox.scenes';
const loadScenes = () => { try { return JSON.parse(localStorage.getItem(SCENES_KEY)) || {}; } catch { return {}; } };
const saveScenes = (s) => { try { localStorage.setItem(SCENES_KEY, JSON.stringify(s)); } catch { toast('Saving needs browser storage.', { error: true }); } };

const COLORS = ['#c9b18f', '#9fb4c7', '#b9c79f', '#d1a3a3', '#c2b0d6', '#e0c98a'];
const SHAPES = [['cube', 'Cube'], ['sphere', 'Sphere'], ['cylinder', 'Cylinder'], ['cone', 'Cone'], ['pyramid', 'Pyramid']];

export async function renderSandbox(main) {
  let scene = defaultScene();
  let sel = 0;
  const view = { mode: 'solid', construction: true, vps: false, shadows: true, grid: true };
  const canvas = h('canvas.sandbox-canvas', { 'aria-label': '3D scene' });
  const stage = h('div.sandbox-stage', canvas);
  const objBox = h('div.stack');
  const g = canvas.getContext('2d');
  const shadowLayer = document.createElement('canvas');

  function size() {
    // measure the free width without the canvas, or its old size would keep the column wide
    canvas.style.width = '0px';
    const r = stage.getBoundingClientRect(), dpr = Math.min(2, window.devicePixelRatio || 1);
    const w = Math.max(240, Math.round(r.width - 18)), hh = Math.max(280, Math.round(Math.min(r.width * 0.75, window.innerHeight - 160)));
    canvas.style.width = `${w}px`; canvas.style.height = `${hh}px`;
    canvas.width = Math.round(w * dpr); canvas.height = Math.round(hh * dpr);
    shadowLayer.width = canvas.width; shadowLayer.height = canvas.height;
  }

  function draw() {
    const W = canvas.width, H = canvas.height, k = W / 900;
    g.fillStyle = '#f7f4ec'; g.fillRect(0, 0, W, H);
    const L = lightDir(scene.light.az, scene.light.el);
    const { faces, shadows, basis } = buildScene(scene.objects, scene.cam, L, W, H);
    const P = (p) => project(p, scene.cam, basis, W, H);
    // ground grid
    if (view.grid) {
      g.strokeStyle = 'rgba(0,0,0,.09)'; g.lineWidth = 1;
      for (let i = -6; i <= 6; i++) {
        for (const [a, b] of [[[i, 0, -6], [i, 0, 6]], [[-6, 0, i], [6, 0, i]]]) {
          const pa = P(a), pb = P(b); if (pa[2] <= 0.05 || pb[2] <= 0.05) continue;
          g.beginPath(); g.moveTo(pa[0], pa[1]); g.lineTo(pb[0], pb[1]); g.stroke();
        }
      }
    }
    // shadows: drawn opaque on a layer, then laid down once (no double-dark overlaps)
    if (view.shadows && view.mode !== 'wireframe') {
      const sg = shadowLayer.getContext('2d');
      sg.clearRect(0, 0, W, H); sg.fillStyle = '#000';
      for (const poly of shadows) { if (poly.some((p) => p[2] <= 0.05)) continue; sg.beginPath(); poly.forEach((p, i) => (i ? sg.lineTo(p[0], p[1]) : sg.moveTo(p[0], p[1]))); sg.closePath(); sg.fill(); }
      g.globalAlpha = 0.22; g.drawImage(shadowLayer, 0, 0); g.globalAlpha = 1;
    }
    // faces back to front
    for (const f of faces) {
      if (f.pts.some((p) => p[2] <= 0.05)) continue;
      g.beginPath(); f.pts.forEach((p, i) => (i ? g.lineTo(p[0], p[1]) : g.moveTo(p[0], p[1]))); g.closePath();
      if (view.mode === 'solid') {
        const base = hex(COLORS[f.obj % COLORS.length]), lit = 0.38 + 0.62 * f.shade;
        g.fillStyle = `rgb(${base.map((c) => Math.round(c * lit)).join(',')})`; g.fill();
        g.strokeStyle = g.fillStyle; g.lineWidth = 1; g.stroke(); // hide seams between facets
      } else if (view.mode === 'silhouette') { g.fillStyle = '#1d1d1b'; g.fill(); g.strokeStyle = '#1d1d1b'; g.stroke(); }
      else { g.strokeStyle = f.obj === sel ? '#2a7a6c' : '#333'; g.lineWidth = 1.2 * k; g.stroke(); }
    }
    // construction lines: each object's own axes through its centre
    if (view.construction && view.mode !== 'silhouette') {
      g.lineWidth = 1.5 * k;
      scene.objects.forEach((obj, oi) => {
        const c = v3.add(obj.pos, [0, obj.scale[1] / 2, 0]);
        objectAxes(obj).forEach((ax, ai) => {
          const len = obj.scale[ai] * 0.75, a = P(v3.add(c, v3.mul(ax, -len))), b = P(v3.add(c, v3.mul(ax, len)));
          if (a[2] <= 0.05 || b[2] <= 0.05) return;
          g.strokeStyle = ['rgba(200,60,50,.8)', 'rgba(40,130,70,.8)', 'rgba(40,90,190,.8)'][ai];
          g.setLineDash(oi === sel ? [] : [5 * k, 4 * k]);
          g.beginPath(); g.moveTo(a[0], a[1]); g.lineTo(b[0], b[1]); g.stroke();
        });
      });
      g.setLineDash([]);
    }
    // vanishing points of the selected object's edges, and the horizon
    if (view.vps && scene.objects[sel]) {
      const obj = scene.objects[sel], cBase = obj.pos;
      const hz = [vanishingPoint([1, 0, 0], scene.cam, basis, W, H), vanishingPoint([0, 0, 1], scene.cam, basis, W, H)].filter(Boolean);
      g.strokeStyle = 'rgba(42,122,108,.9)'; g.lineWidth = 2 * k;
      if (hz.length === 2) { const [a, b] = hz, dx = b[0] - a[0], dy = b[1] - a[1], t = 20; g.beginPath(); g.moveTo(a[0] - dx * t, a[1] - dy * t); g.lineTo(b[0] + dx * t, b[1] + dy * t); g.stroke(); }
      objectAxes(obj).forEach((ax, ai) => {
        const vp = vanishingPoint(ax, scene.cam, basis, W, H);
        if (!vp) return;
        g.strokeStyle = ['rgba(200,60,50,.55)', 'rgba(40,130,70,.55)', 'rgba(40,90,190,.55)'][ai]; g.lineWidth = 1 * k;
        // lines from the box corners toward this vanishing point
        for (const sx of [-0.5, 0.5]) for (const sz of [-0.5, 0.5]) for (const sy of [0, 1]) {
          const local = [sx * obj.scale[0], sy * obj.scale[1], sz * obj.scale[2]];
          const wp = v3.add(cBase, objectAxes(obj).reduce((acc, a2, i2) => v3.add(acc, v3.mul(a2, local[i2])), [0, 0, 0]));
          const p = P(wp); if (p[2] <= 0.05) continue;
          g.beginPath(); g.moveTo(p[0], p[1]); g.lineTo(vp[0], vp[1]); g.stroke();
        }
        g.fillStyle = g.strokeStyle.replace('.55', '1'); g.beginPath(); g.arc(vp[0], vp[1], 6 * k, 0, Math.PI * 2); g.fill();
      });
      if (scene.cam.ortho) { g.fillStyle = '#555'; g.font = `${14 * k}px system-ui`; g.fillText('Orthographic view: parallel edges stay parallel (no vanishing points).', 12 * k, 22 * k); }
    }
  }

  const hex = (c) => [1, 3, 5].map((i) => parseInt(c.slice(i, i + 2), 16));

  // ----- interaction: drag to orbit, wheel / pinch to zoom, tap to select -----
  const pointers = new Map(); let lastPinch = 0, moved = false;
  canvas.addEventListener('pointerdown', (e) => { pointers.set(e.pointerId, [e.clientX, e.clientY]); canvas.setPointerCapture(e.pointerId); moved = false; });
  canvas.addEventListener('pointermove', (e) => {
    if (!pointers.has(e.pointerId)) return;
    const prev = pointers.get(e.pointerId); pointers.set(e.pointerId, [e.clientX, e.clientY]);
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()], d = Math.hypot(a[0] - b[0], a[1] - b[1]);
      if (lastPinch) zoom(lastPinch / d); lastPinch = d; moved = true; return;
    }
    const dx = e.clientX - prev[0], dy = e.clientY - prev[1];
    if (Math.abs(dx) + Math.abs(dy) > 1) moved = true;
    scene.cam.yaw -= dx * 0.4; scene.cam.pitch = Math.max(-5, Math.min(85, scene.cam.pitch + dy * 0.3));
    draw();
  });
  const up = (e) => {
    pointers.delete(e.pointerId); if (pointers.size < 2) lastPinch = 0;
    if (!moved) pick(e);
  };
  canvas.addEventListener('pointerup', up); canvas.addEventListener('pointercancel', (e) => { pointers.delete(e.pointerId); lastPinch = 0; });
  canvas.addEventListener('wheel', (e) => { e.preventDefault(); zoom(e.deltaY > 0 ? 1.1 : 1 / 1.1); }, { passive: false });
  function zoom(f) { scene.cam.dist = Math.max(2.5, Math.min(30, scene.cam.dist * f)); scene.cam.orthoSize = Math.max(2, Math.min(25, scene.cam.orthoSize * f)); draw(); }
  function pick(e) {
    const r = canvas.getBoundingClientRect(), x = ((e.clientX - r.left) / r.width) * canvas.width, y = ((e.clientY - r.top) / r.height) * canvas.height;
    const { faces } = buildScene(scene.objects, scene.cam, lightDir(scene.light.az, scene.light.el), canvas.width, canvas.height);
    for (let i = faces.length - 1; i >= 0; i--) if (inside([x, y], faces[i].pts)) { sel = faces[i].obj; renderObj(); draw(); return; }
  }
  const inside = (p, poly) => { let c = false; for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) { const a = poly[i], b = poly[j]; if ((a[1] > p[1]) !== (b[1] > p[1]) && p[0] < ((b[0] - a[0]) * (p[1] - a[1])) / (b[1] - a[1]) + a[0]) c = !c; } return c; };

  // ----- panel -----
  function renderObj() {
    const o = scene.objects[sel];
    const list = h('div.chips', ...scene.objects.map((ob, i) => h('button.chip', { type: 'button', 'aria-pressed': String(i === sel), style: { borderLeft: `6px solid ${COLORS[i % COLORS.length]}` },
      on: { click: () => { sel = i; renderObj(); draw(); } } }, `${i + 1}. ${SHAPES.find((s) => s[0] === ob.kind)[1]}`)));
    if (!o) { objBox.replaceChildren(list, h('p.small.muted', 'Add a shape to start.')); return; }
    const sl = (label, min, max, step, get, set) => slider({ label, min, max, step, value: get(), format: (v) => (+v).toFixed(step < 1 ? 1 : 0), onInput: (v) => { set(v); draw(); } });
    objBox.replaceChildren(list,
      h('div.grid', { style: { gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))' } },
        sl('Left / right', -5, 5, 0.1, () => o.pos[0], (v) => { o.pos[0] = v; }),
        sl('Front / back', -5, 5, 0.1, () => o.pos[2], (v) => { o.pos[2] = v; }),
        sl('Lift', 0, 4, 0.1, () => o.pos[1], (v) => { o.pos[1] = v; }),
        sl('Turn', -180, 180, 1, () => o.rot[1], (v) => { o.rot[1] = v; }),
        sl('Tilt forward', -90, 90, 1, () => o.rot[0], (v) => { o.rot[0] = v; }),
        sl('Tilt sideways', -90, 90, 1, () => o.rot[2], (v) => { o.rot[2] = v; }),
        sl('Width', 0.2, 5, 0.1, () => o.scale[0], (v) => { o.scale[0] = v; }),
        sl('Height', 0.2, 5, 0.1, () => o.scale[1], (v) => { o.scale[1] = v; }),
        sl('Depth', 0.2, 5, 0.1, () => o.scale[2], (v) => { o.scale[2] = v; })),
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: () => { scene.objects.push(structuredClone({ ...o, pos: [o.pos[0] + 0.8, o.pos[1], o.pos[2] + 0.8] })); sel = scene.objects.length - 1; renderObj(); draw(); } } }, 'Duplicate'),
        h('button.btn.small.danger', { type: 'button', on: { click: () => { scene.objects.splice(sel, 1); sel = Math.max(0, sel - 1); renderObj(); draw(); } } }, 'Remove')));
  }

  const scenesSel = h('select', { 'aria-label': 'Saved scenes' });
  const renderScenes = () => { const s = loadScenes(); scenesSel.replaceChildren(h('option', { value: '' }, Object.keys(s).length ? 'Open a saved scene…' : 'No saved scenes'), ...Object.keys(s).map((n) => h('option', { value: n }, n))); };
  scenesSel.addEventListener('change', () => { const s = loadScenes()[scenesSel.value]; if (s) { scene = structuredClone(s); sel = 0; renderObj(); draw(); } });

  main.appendChild(h('div.studio',
    h('div.studio-side.stack',
      h('h1', '3D sandbox'),
      h('p.small.muted', 'Build simple 3D shapes to understand form, light and perspective. Drag to turn the view, scroll or pinch to zoom, tap a shape to select it.'),
      h('div.row', ...SHAPES.map(([k, l]) => h('button.btn.small', { type: 'button', on: { click: () => {
        scene.objects.push({ kind: k, pos: [(Math.random() - 0.5) * 2, 0, (Math.random() - 0.5) * 2], rot: [0, 0, 0], scale: [1, 1, 1] }); sel = scene.objects.length - 1; renderObj(); draw();
      } } }, `+ ${l}`))),
      objBox,
      h('details', { open: true }, h('summary', 'View'),
        segmented('Look', [['solid', 'Solid'], ['wireframe', 'Wireframe'], ['silhouette', 'Silhouette']], view.mode, (v) => { view.mode = v; draw(); }),
        segmented('Projection', [['p', 'Perspective'], ['o', 'Orthographic']], 'p', (v) => { scene.cam.ortho = v === 'o'; draw(); }),
        slider({ label: 'Lens (wide → long)', min: 0.8, max: 4, step: 0.1, value: scene.cam.focal, format: (v) => (+v).toFixed(1), onInput: (v) => { scene.cam.focal = v; scene.cam.dist = 3.5 * v; draw(); } }),
        toggle('Construction lines (each shape\'s axes)', view.construction, (v) => { view.construction = v; draw(); }),
        toggle('Vanishing points of the selected shape', view.vps, (v) => { view.vps = v; draw(); }, 'Lines along each edge direction meet at one point on the horizon (or above/below it for tilted edges).'),
        toggle('Shadows', view.shadows, (v) => { view.shadows = v; draw(); }),
        toggle('Ground grid', view.grid, (v) => { view.grid = v; draw(); })),
      h('details', h('summary', 'Light'),
        slider({ label: 'Light direction', min: -180, max: 180, value: scene.light.az, format: (v) => `${v}°`, onInput: (v) => { scene.light.az = v; draw(); } }),
        slider({ label: 'Light height', min: 5, max: 90, value: scene.light.el, format: (v) => `${v}°`, onInput: (v) => { scene.light.el = v; draw(); } })),
      h('div.row',
        h('button.btn.small', { type: 'button', on: { click: async () => { const n = await promptDialog('Save scene', `Scene ${Object.keys(loadScenes()).length + 1}`, 'Name'); if (!n) return; const s = loadScenes(); s[n] = scene; saveScenes(s); renderScenes(); toast('Scene saved on this device.'); } } }, 'Save scene'),
        scenesSel),
      h('div.row',
        h('button.btn.small.primary', { type: 'button', on: { click: async () => download(await new Promise((r) => canvas.toBlob(r, 'image/png')), 'piccasso-3d-scene.png') } }, 'Save PNG'),
        h('button.btn.small', { type: 'button', on: { click: () => { scene = defaultScene(); sel = 0; renderObj(); draw(); } } }, 'Reset'))),
    stage));

  renderObj(); renderScenes();
  size(); draw();
  const onResize = () => { size(); draw(); };
  window.addEventListener('resize', onResize);
  return () => window.removeEventListener('resize', onResize);
}
