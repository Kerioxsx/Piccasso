// Camera tracing ("AR") mode.
//  • WebXR (where supported): real plane detection, tap-to-place, anchors.
//  • Camera overlay (most phones/browsers): live camera + transparent guide,
//    manual move/scale/rotate, 4-corner paper calibration (true perspective),
//    automatic paper detection and an optional "follow paper" tracker.
//  • No camera: align the guide over a photo of your paper.
// The guide is drawn ON SCREEN over the camera image; nothing is projected onto paper.
import { app } from '../../state.js';
import { h, icon, slider, toggle, nav, toast, pickFile, announce } from '../dom.js';
import { composeGuide } from '../canvas.js';
import { probeAr } from '../../core/capabilities.js';
import { cssMatrix3d, homography, applyH, transformedRect, detectPaper, orderCorners } from '../../core/geometry.js';
import { paperLayout } from '../../core/steps.js';
import { toGray } from '../../core/image.js';
import { WakeLock } from '../wakelock.js';
import { Timelapse, timelapseSupported, saveVideo } from '../timelapse.js';
import { frameStats, lightingHints, skinMask, dilate } from '../../core/camera.js';

const LIMITS = 'The camera view is an alignment aid. The guide is drawn on your screen over the camera picture; it is not projected onto the paper. Alignment can drift when the phone or paper moves, and the app cannot correct for every movement. For best results, put the phone on a stand or a stack of books above the paper, use even lighting, and tape the paper down.';

export async function renderAr(main) {
  const p = app.project;
  document.body.classList.add('immersive');
  const ar = p.ar;
  const step = p.steps[p.progress.currentStep];
  const layout = paperLayout(p.prep.paper, app.shared.paper.presets, p.analysis.aspect, app.shared.paper.defaultMarginMm);
  const layers = new Set([ar.layer === 'reference' ? 'reference' : ar.layer === 'step' ? 'step' : 'outline']);
  let mode = 'none'; // webxr | camera | photo | none
  let stream = null, xr = null, follow = false, calibrating = false, trackTimer = 0;
  let paperCorners = null; // screen-space TL,TR,BR,BL of the physical paper (calibration)
  let simple = { cx: 0, cy: 0, scale: ar.scale || 1, rotation: ar.rotation || 0, ox: ar.offsetX || 0, oy: ar.offsetY || 0 };
  const wake = new WakeLock(() => {});
  let facing = 'environment', frozen = false, zoom = 1, pan = [0, 0], showHand = false, handTimer = 0, hintTimer = 0, hintId = '', hintDismissed = '';

  const video = h('video', { playsInline: true, muted: true, autoplay: true, 'aria-label': 'Camera view' });
  video.setAttribute('playsinline', '');
  const bgPhoto = h('img.bg-photo', { alt: 'Photo of your paper', hidden: true });
  const freezeCv = h('canvas.ar-freeze', { hidden: true, 'aria-label': 'Frozen camera picture' });
  const overlay = h('canvas.ar-overlay', { 'aria-hidden': 'true' });
  const handCv = h('canvas.ar-hand', { 'aria-hidden': 'true', hidden: true });
  const surface = h('div.ar-drag-surface', { 'aria-label': 'Drag to move the guide; pinch to resize and rotate' });
  const handles = ['TL', 'TR', 'BR', 'BL'].map((n, i) => h('div.ar-handle', { hidden: true, role: 'slider', tabIndex: 0, 'aria-label': `Paper corner ${n}. Drag onto the matching corner of your paper.`, 'data-i': i }, n));
  const statusEl = h('span.ar-status', { 'data-state': 'unavailable', role: 'status' }, h('span.dot'), h('span', 'Checking…'));
  const msg = h('div.ar-msg', { hidden: true, role: 'alert' });
  const xrDom = h('div', { style: { position: 'absolute', inset: 0, pointerEvents: 'none' } });
  const hint = h('div.ar-hint', { hidden: true, role: 'status', 'aria-live': 'polite' });

  const setStatus = (state, text) => { statusEl.dataset.state = state; statusEl.lastChild.textContent = text; };
  const showMsg = (...content) => { msg.innerHTML = ''; msg.append(...content); msg.hidden = false; };
  const hideMsg = () => { msg.hidden = true; };

  // ----- controls -----
  const layerChip = (id, text) => h('button.btn.small', { type: 'button', 'aria-pressed': String(layers.has(id)), on: { click: (e) => {
    if (layers.has(id)) layers.delete(id); else layers.add(id);
    if (id === 'reference' && layers.has(id)) layers.delete('outline');
    e.currentTarget.setAttribute('aria-pressed', String(layers.has(id)));
    rebuildOverlay();
  } } }, text);
  const lockToggle = toggle('Lock alignment', !!ar.alignmentLocked, (v) => { ar.alignmentLocked = v; if (v) { follow = false; followToggleInput.checked = false; } updateHandles(); setModeStatus(); save(); announce(v ? 'Alignment locked' : 'Alignment unlocked'); }, 'Stops accidental moves and tracking updates.');
  const followToggle = toggle('Follow paper (experimental)', false, (v) => { follow = v; if (v) { ar.alignmentLocked = false; lockToggle.querySelector('input').checked = false; startTracking(); } else stopTracking(); setModeStatus(); }, 'Re-detects the paper a few times per second. Works best on a plain, contrasting table.');
  const followToggleInput = followToggle.querySelector('input');
  const nudge = (dx, dy) => { if (ar.alignmentLocked) return; if (paperCorners) paperCorners = paperCorners.map(([x, y]) => [x + dx, y + dy]); else { simple.ox += dx; simple.oy += dy; } place(); save(); };

  // ----- timelapse of the real drawing (a still every few seconds) -----
  const tl = new Timelapse(video, { everyMs: 2000 });
  const tlStatus = h('span.small.muted');
  const tlSave = h('button.btn.small', { type: 'button', hidden: true, on: { click: async () => {
    tlSave.disabled = true;
    try {
      const blob = await tl.render({ seconds: 15, onProgress: (f) => { tlStatus.textContent = `Making the video… ${Math.round(f * 100)}%`; } });
      await saveVideo(blob, `piccasso-timelapse-${(app.project.name || 'drawing').replace(/[^\w-]+/g, '-').slice(0, 40)}`);
      tlStatus.textContent = 'Timelapse saved.';
    } catch (e) { toast(e.message, { error: true }); tlStatus.textContent = ''; }
    tlSave.disabled = false;
  } } }, 'Save timelapse video');
  const tlBox = h('input', { type: 'checkbox', on: { change: (e) => {
    if (e.target.checked) { if (tl.count === 0) tl.clear(); tl.start(); tlSave.hidden = true; announce('Recording a timelapse'); }
    else { tl.stop(); tlSave.hidden = tl.count === 0; }
    tlTick();
  } } });
  const tlTick = () => { tlStatus.textContent = tl.recording ? `Recording… ${tl.count} photos (one every 2 s)` : tl.count ? `${tl.count} photos recorded` : ''; };
  const tlTimer = setInterval(() => { if (tl.recording) tlTick(); }, 1000);
  const tlRow = h('div.row', { style: { marginTop: '6px', alignItems: 'center' } },
    h('label.row', { style: { gap: '6px' } }, tlBox, 'Record timelapse'), tlStatus, tlSave);
  if (!timelapseSupported()) { tlBox.disabled = true; tlStatus.textContent = 'This browser can\'t make videos.'; }

  const freezeBtn = h('button.btn.small', { type: 'button', 'aria-pressed': 'false', on: { click: () => setFrozen(!frozen) } }, 'Freeze frame');
  const switchBtn = h('button.btn.small', { type: 'button', hidden: true, on: { click: switchCamera } }, 'Switch camera');
  const handBox = h('input', { type: 'checkbox', on: { change: (e) => { showHand = e.target.checked; handCv.hidden = !showHand; if (showHand) startHand(); else stopHand(); announce(showHand ? 'Your hand is shown in front of the guide' : 'Hand view off'); } } });
  const zoomSlider = slider({ label: 'Zoom (magnify the view)', min: 100, max: 400, step: 10, value: 100, format: (v) => v + '%', onInput: (v) => { zoom = v / 100; if (zoom === 1) pan = [0, 0]; applyZoom(); } });
  const camOnly = [];
  const panel = h('div.ar-panel', { role: 'region', 'aria-label': 'Tracing controls' },
    h('div.grid', { style: { gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))' } },
      slider({ label: 'Guide opacity', min: 5, max: 100, value: Math.round(ar.opacity * 100), format: (v) => v + '%', onInput: (v) => { ar.opacity = v / 100; overlay.style.opacity = ar.opacity; save(); } }),
      slider({ label: 'Size', min: 20, max: 300, value: Math.round(simple.scale * 100), format: (v) => v + '%', onInput: (v) => { if (ar.alignmentLocked) return; simple.scale = v / 100; paperCorners = null; place(); save(); } }),
      slider({ label: 'Rotation', min: -180, max: 180, value: simple.rotation, format: (v) => v + '°', onInput: (v) => { if (ar.alignmentLocked) return; simple.rotation = v; paperCorners = null; place(); save(); } })),
    h('div.row', { style: { marginTop: '8px' } },
      h('span.small', 'Show:'), layerChip('reference', 'Picture'), layerChip('outline', 'Outline'), layerChip('shapes', 'Shapes'),
      layerChip('contours', 'Contours'), layerChip('shading', 'Shading'), layerChip('step', 'Current step')),
    h('div.row', { style: { marginTop: '8px' } },
      h('button.btn.small', { type: 'button', 'aria-label': 'Move left', on: { click: () => nudge(-8, 0) } }, '←'),
      h('button.btn.small', { type: 'button', 'aria-label': 'Move up', on: { click: () => nudge(0, -8) } }, '↑'),
      h('button.btn.small', { type: 'button', 'aria-label': 'Move down', on: { click: () => nudge(0, 8) } }, '↓'),
      h('button.btn.small', { type: 'button', 'aria-label': 'Move right', on: { click: () => nudge(8, 0) } }, '→'),
      h('button.btn.small', { type: 'button', on: { click: recenter } }, icon('target'), 'Recenter'),
      h('button.btn.small', { type: 'button', 'aria-pressed': 'false', on: { click: (e) => { calibrating = !calibrating; e.currentTarget.setAttribute('aria-pressed', String(calibrating)); if (calibrating && !paperCorners) paperCorners = defaultPaperCorners(); updateHandles(); place(); if (calibrating) announce('Drag the four round handles onto the corners of your paper'); } } }, 'Calibrate corners'),
      (camOnly[0] = h('button.btn.small', { type: 'button', on: { click: detectNow } }, 'Detect paper'))),
    h('div.grid', { style: { gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', marginTop: '6px' } }, lockToggle, (camOnly[1] = followToggle)),
    (camOnly[2] = h('div.row', { style: { marginTop: '8px', alignItems: 'center' } },
      freezeBtn, switchBtn,
      h('label.row', { style: { gap: '6px' } }, handBox, 'Show my hand in front of the guide'))),
    h('div.grid', { style: { gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', marginTop: '6px' } }, zoomSlider),
    tlRow,
    h('details', { style: { marginTop: '6px' } }, h('summary', 'About camera tracing'), h('p.small', LIMITS)));

  const xrBtn = h('button.btn.small', { type: 'button', hidden: true, on: { click: startXrMode } }, icon('ar'), 'Start AR');
  const stage = h('div.ar-stage', video, bgPhoto, freezeCv, overlay, handCv, ...handles);
  const screen = h('div.ar-screen', stage, surface, hint,
    h('div.ar-top',
      h('button.icon-btn', { type: 'button', 'aria-label': 'Exit camera tracing', on: { click: () => nav('workspace') } }, icon('close')),
      statusEl, h('span', { style: { flex: 1 } }), xrBtn,
      h('button.btn.small', { type: 'button', on: { click: usePhoto } }, icon('image'), 'Use a photo')),
    msg, panel, xrDom);
  main.appendChild(screen);

  function save() { clearTimeout(save.t); save.t = setTimeout(() => app.saveProject().catch(() => {}), 500); }

  // ----- overlay composition -----
  function rebuildOverlay() {
    const ref = app.renders.original || app.srcCanvas;
    const W = Math.min(1400, ref.width), H = Math.round(W / p.analysis.aspect);
    const c = document.createElement('canvas'); c.width = W; c.height = H;
    const ctx = c.getContext('2d');
    if (layers.has('reference')) ctx.drawImage(composeGuide(app.baseCanvas('original'), null, [], { width: W, height: H }), 0, 0);
    const add = (base, lys) => ctx.drawImage(composeGuide(base, p.analysis, lys, { width: W, height: H, transparentPaper: true }), 0, 0);
    if (layers.has('shading')) add(app.baseCanvas('values'), []);
    if (layers.has('outline')) add(app.baseCanvas('outline'), []);
    if (layers.has('step') && step) add(app.baseCanvas(step.base), step.layers);
    if (layers.has('shapes')) add(null, ['shapes']);
    if (layers.has('contours')) add(null, ['silhouette', 'internal']);
    overlay.width = W; overlay.height = H;
    overlay.style.width = W + 'px'; overlay.style.height = H + 'px';
    overlay.getContext('2d').drawImage(c, 0, 0);
    overlay.style.opacity = ar.opacity;
    ar.layer = layers.has('reference') ? 'reference' : layers.has('step') ? 'step' : 'outline';
    if (xr) xr.refreshTexture();
    place();
  }

  // ----- placement -----
  function defaultPaperCorners() {
    const sw = screen.clientWidth, sh = screen.clientHeight;
    const pa = layout.paperW / layout.paperH;
    let w = sw * 0.8, hh = w / pa;
    if (hh > sh * 0.6) { hh = sh * 0.6; w = hh * pa; }
    return transformedRect(sw / 2 + simple.ox, sh * 0.42 + simple.oy, w, hh, 1, 0);
  }
  function quad() {
    const W = overlay.width, H = overlay.height;
    if (paperCorners) {
      // map the drawing area inside the paper (mm) onto the calibrated paper corners
      const hm = homography([[0, 0], [layout.paperW, 0], [layout.paperW, layout.paperH], [0, layout.paperH]], paperCorners);
      if (hm) return [[layout.drawX, layout.drawY], [layout.drawX + layout.drawW, layout.drawY], [layout.drawX + layout.drawW, layout.drawY + layout.drawH], [layout.drawX, layout.drawY + layout.drawH]].map(([x, y]) => applyH(hm, x, y));
    }
    const sw = screen.clientWidth, sh = screen.clientHeight;
    const fit = Math.min((sw * 0.8) / W, (sh * 0.55) / H);
    return transformedRect(sw / 2 + simple.ox, sh * 0.42 + simple.oy, W, H, fit * simple.scale, simple.rotation);
  }
  function place() {
    if (!overlay.width) return;
    const q = quad();
    overlay.style.transform = cssMatrix3d(overlay.width, overlay.height, q);
    if (paperCorners) handles.forEach((hd, i) => { hd.style.left = paperCorners[i][0] + 'px'; hd.style.top = paperCorners[i][1] + 'px'; });
    ar.corners = paperCorners ? paperCorners.map(([x, y]) => [x / screen.clientWidth, y / screen.clientHeight]) : null;
    ar.scale = simple.scale; ar.rotation = simple.rotation; ar.offsetX = simple.ox; ar.offsetY = simple.oy;
  }
  function updateHandles() { handles.forEach((hd) => { hd.hidden = !(calibrating && paperCorners) || ar.alignmentLocked; }); }
  function recenter() {
    if (ar.alignmentLocked) { toast('Unlock alignment first.'); return; }
    simple = { cx: 0, cy: 0, scale: 1, rotation: 0, ox: 0, oy: 0 };
    paperCorners = calibrating ? defaultPaperCorners() : null;
    if (xr) xr.recenter();
    place(); updateHandles(); save();
    announce('Guide recentered');
  }

  // Handle dragging (calibration)
  handles.forEach((hd, i) => {
    hd.addEventListener('pointerdown', (e) => { if (ar.alignmentLocked) return; e.preventDefault(); hd.setPointerCapture(e.pointerId); hd.dataset.drag = '1'; });
    hd.addEventListener('pointermove', (e) => {
      if (!hd.dataset.drag) return;
      paperCorners[i] = toStage(e.clientX, e.clientY);
      place();
    });
    hd.addEventListener('pointerup', () => { delete hd.dataset.drag; paperCorners = orderCorners(paperCorners); place(); save(); });
    hd.addEventListener('keydown', (e) => {
      const d = { ArrowLeft: [-4, 0], ArrowRight: [4, 0], ArrowUp: [0, -4], ArrowDown: [0, 4] }[e.key];
      if (!d || ar.alignmentLocked) return;
      e.preventDefault(); paperCorners[i] = [paperCorners[i][0] + d[0], paperCorners[i][1] + d[1]]; place(); save();
    });
  });

  // Simple-mode gestures on the surface: drag = move, pinch = scale + rotate.
  const touches = new Map();
  let g0 = null;
  const gsnap = () => {
    const a = [...touches.values()];
    if (a.length === 1) return { x: a[0][0], y: a[0][1] };
    const [p1, p2] = a;
    return { x: (p1[0] + p2[0]) / 2, y: (p1[1] + p2[1]) / 2, d: Math.hypot(p1[0] - p2[0], p1[1] - p2[1]), a: Math.atan2(p2[1] - p1[1], p2[0] - p1[0]) };
  };
  surface.addEventListener('pointerdown', (e) => { if ((ar.alignmentLocked && zoom === 1) || mode === 'webxr') return; surface.setPointerCapture(e.pointerId); touches.set(e.pointerId, [e.clientX, e.clientY]); g0 = gsnap(); });
  surface.addEventListener('pointermove', (e) => {
    if (!touches.has(e.pointerId)) return;
    touches.set(e.pointerId, [e.clientX, e.clientY]);
    const g = gsnap();
    if (ar.alignmentLocked) { pan = [pan[0] + g.x - g0.x, pan[1] + g.y - g0.y]; g0 = g; applyZoom(); return; } // locked + zoomed: drag moves the view
    const k = 1 / zoom;
    g.x = g0.x + (g.x - g0.x) * k; g.y = g0.y + (g.y - g0.y) * k;
    if (paperCorners) paperCorners = paperCorners.map(([x, y]) => [x + g.x - g0.x, y + g.y - g0.y]);
    else {
      simple.ox += g.x - g0.x; simple.oy += g.y - g0.y;
      if (g.d && g0.d) { simple.scale = Math.min(3, Math.max(0.2, simple.scale * (g.d / g0.d))); simple.rotation += ((g.a - g0.a) * 180) / Math.PI; }
    }
    g0 = gsnap(); place();
  });
  const gEnd = (e) => { touches.delete(e.pointerId); g0 = touches.size ? gsnap() : null; save(); };
  surface.addEventListener('pointerup', gEnd);
  surface.addEventListener('pointercancel', gEnd);

  // ----- paper detection / tracking -----
  const grab = document.createElement('canvas');
  function frameGray() {
    const src = mode === 'photo' ? bgPhoto : frozen ? freezeCv : video;
    const vw = mode === 'photo' ? bgPhoto.naturalWidth : frozen ? freezeCv.width : video.videoWidth, vh = mode === 'photo' ? bgPhoto.naturalHeight : frozen ? freezeCv.height : video.videoHeight;
    if (!vw || !vh) return null;
    const s = 200 / Math.max(vw, vh);
    grab.width = Math.round(vw * s); grab.height = Math.round(vh * s);
    const ctx = grab.getContext('2d', { willReadFrequently: true });
    ctx.drawImage(src, 0, 0, grab.width, grab.height);
    const d = ctx.getImageData(0, 0, grab.width, grab.height);
    return { gray: toGray({ width: d.width, height: d.height, data: d.data }), vw, vh };
  }
  /** Map normalized media coords to screen coords (object-fit: cover for video, contain for photo). */
  function mediaToScreen([nx, ny], vw, vh) {
    const sw = screen.clientWidth, sh = screen.clientHeight;
    const s = mode === 'photo' ? Math.min(sw / vw, sh / vh) : Math.max(sw / vw, sh / vh);
    return [(sw - vw * s) / 2 + nx * vw * s, (sh - vh * s) / 2 + ny * vh * s];
  }
  function detectOnce() {
    const f = frameGray();
    if (!f) return null;
    let mean = 0; for (const v of f.gray.data) mean += v; mean /= f.gray.data.length;
    const res = detectPaper(f.gray);
    return { res, mean, corners: res ? res.corners.map((c) => mediaToScreen(c, f.vw, f.vh)) : null };
  }
  function detectNow() {
    if (ar.alignmentLocked) { toast('Unlock alignment first.'); return; }
    const d = detectOnce();
    if (!d) { toast('The camera is not ready yet.'); return; }
    if (d.mean < 50) toast('It\'s quite dark. More light will help the camera see the paper.');
    if (!d.res || d.res.confidence < 0.35) { toast('Couldn\'t find the paper. Make sure all four corners are visible on a darker surface, or calibrate the corners by hand.', { ms: 6000 }); return; }
    paperCorners = d.corners; calibrating = true; updateHandles(); place(); save();
    announce('Paper found. Check the corners and adjust if needed.');
  }
  function startTracking() {
    stopTracking();
    trackTimer = setInterval(() => {
      if (!follow || ar.alignmentLocked || document.hidden) return;
      const d = detectOnce();
      if (d && d.res && d.res.confidence >= 0.5) {
        paperCorners = paperCorners ? paperCorners.map(([x, y], i) => [x * 0.6 + d.corners[i][0] * 0.4, y * 0.6 + d.corners[i][1] * 0.4]) : d.corners;
        place(); setStatus('tracking', 'Following paper');
      } else setStatus('searching', 'Searching for paper… (holding last position)');
    }, 200);
  }
  function stopTracking() { clearInterval(trackTimer); trackTimer = 0; }
  function setModeStatus() {
    if (mode === 'webxr') return;
    if (mode === 'none') setStatus('unavailable', 'AR unavailable');
    else if (ar.alignmentLocked) setStatus('tracking', 'Alignment locked');
    else if (follow) setStatus('searching', 'Searching for paper…');
    else setStatus('limited', mode === 'photo' ? 'Photo alignment (manual)' : 'Manual alignment (no tracking)');
  }

  // ----- modes -----
  async function startCamera() {
    hideMsg();
    try {
      stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: { ideal: facing }, width: { ideal: 1920 }, height: { ideal: 1080 } }, audio: false });
    } catch (e) {
      mode = 'none'; setModeStatus();
      const why = e.name === 'NotAllowedError' ? 'Camera permission was denied. To use tracing, allow camera access for this site in your browser settings, then try again.'
        : e.name === 'NotFoundError' || e.name === 'OverconstrainedError' ? 'No camera was found on this device.'
          : e.name === 'NotReadableError' ? 'The camera is being used by another app. Close it and try again.'
            : 'The camera could not be started: ' + e.message;
      showMsg(h('p', why), h('div.row', { style: { justifyContent: 'center' } },
        h('button.btn.small', { type: 'button', on: { click: startCamera } }, 'Try again'),
        h('button.btn.small', { type: 'button', on: { click: usePhoto } }, 'Use a photo of my paper'),
        h('button.btn.small', { type: 'button', on: { click: () => nav('locked') } }, 'Use locked reference instead')));
      return false;
    }
    video.srcObject = stream; video.hidden = false; bgPhoto.hidden = true; tlRow.hidden = false;
    try { await video.play(); } catch { /* autoplay policies; muted inline usually OK */ }
    mode = 'camera';
    camOnly.forEach((el) => { el.hidden = false; });
    navigator.mediaDevices.enumerateDevices?.().then((ds) => { switchBtn.hidden = ds.filter((d) => d.kind === 'videoinput').length < 2; }).catch(() => {});
    startHints();
    stream.getVideoTracks()[0].addEventListener('ended', () => {
      setStatus('lost', 'Camera stopped');
      showMsg(h('p', 'The camera stopped (another app may have taken it, or the device went to sleep).'), h('button.btn.small', { type: 'button', on: { click: startCamera } }, 'Restart camera'));
    });
    setModeStatus();
    if (app.settings.keepAwake) wake.request();
    return true;
  }
  function stopCamera() { if (stream) { stream.getTracks().forEach((t) => t.stop()); stream = null; } stopHints(); }

  // ----- zoom, freeze, camera switch -----
  /** Screen (client) point → stage point, undoing the zoom and pan. */
  function toStage(x, y) {
    const r = screen.getBoundingClientRect(), cx = r.width / 2, cy = r.height / 2;
    return [cx + (x - r.left - cx - pan[0]) / zoom, cy + (y - r.top - cy - pan[1]) / zoom];
  }
  function applyZoom() {
    const lim = [(zoom - 1) * screen.clientWidth / 2, (zoom - 1) * screen.clientHeight / 2];
    pan = pan.map((v, i) => Math.max(-lim[i], Math.min(lim[i], v)));
    stage.style.transform = zoom === 1 ? '' : `translate(${pan[0]}px, ${pan[1]}px) scale(${zoom})`;
  }
  function setFrozen(v) {
    if (v && !video.videoWidth) { toast('The camera is not ready yet.'); return; }
    frozen = v;
    if (v) {
      freezeCv.width = video.videoWidth; freezeCv.height = video.videoHeight;
      freezeCv.getContext('2d').drawImage(video, 0, 0);
      clearHand();
    }
    freezeCv.hidden = !v; video.style.visibility = v ? 'hidden' : '';
    freezeBtn.setAttribute('aria-pressed', String(v)); freezeBtn.textContent = v ? 'Back to live' : 'Freeze frame';
    announce(v ? 'Picture frozen. Zoom in to check details.' : 'Live camera');
  }
  async function switchCamera() {
    facing = facing === 'environment' ? 'user' : 'environment';
    if (frozen) setFrozen(false);
    stopCamera();
    if (await startCamera()) toast(facing === 'user' ? 'Front camera' : 'Back camera');
  }

  // ----- lighting / focus hints -----
  function startHints() {
    stopHints();
    hintTimer = setInterval(() => {
      if (mode !== 'camera' || frozen || document.hidden) return;
      const f = frameGray(); if (!f) return;
      const hs = lightingHints(frameStats(f.gray));
      const top = hs.find((x) => x.id !== hintDismissed);
      if (!top) { hint.hidden = true; hintId = ''; if (!hs.length) hintDismissed = ''; return; }
      if (top.id === hintId) return;
      hintId = top.id;
      hint.replaceChildren(h('span', top.text), h('button.icon-btn', { type: 'button', 'aria-label': 'Dismiss hint', on: { click: () => { hintDismissed = hintId; hint.hidden = true; hintId = ''; } } }, '×'));
      hint.hidden = false;
    }, 1500);
  }
  function stopHints() { clearInterval(hintTimer); hintTimer = 0; hint.hidden = true; hintId = ''; }

  // ----- hand in front of the guide (classical skin-colour mask) -----
  const handGrab = document.createElement('canvas'), maskCv = document.createElement('canvas');
  function clearHand() { handCv.getContext('2d').clearRect(0, 0, handCv.width, handCv.height); }
  function startHand() {
    stopHand();
    handTimer = setInterval(() => {
      if (mode !== 'camera' || frozen || document.hidden || !video.videoWidth) return;
      const vw = video.videoWidth, vh = video.videoHeight, k = 160 / Math.max(vw, vh);
      handGrab.width = Math.round(vw * k); handGrab.height = Math.round(vh * k);
      const g = handGrab.getContext('2d', { willReadFrequently: true });
      g.drawImage(video, 0, 0, handGrab.width, handGrab.height);
      const d = g.getImageData(0, 0, handGrab.width, handGrab.height);
      const m = skinMask({ width: d.width, height: d.height, data: d.data });
      const sw = screen.clientWidth, sh = screen.clientHeight;
      if (handCv.width !== sw || handCv.height !== sh) { handCv.width = sw; handCv.height = sh; }
      const ctx = handCv.getContext('2d');
      ctx.clearRect(0, 0, sw, sh);
      if (m.share < 0.004) return;
      const big = dilate(m, 2);
      maskCv.width = big.width; maskCv.height = big.height;
      const md = maskCv.getContext('2d').createImageData(big.width, big.height);
      for (let i = 0; i < big.data.length; i++) md.data[i * 4 + 3] = big.data[i];
      maskCv.getContext('2d').putImageData(md, 0, 0);
      const s = Math.max(sw / vw, sh / vh), dw = vw * s, dh = vh * s, dx = (sw - dw) / 2, dy = (sh - dh) / 2;
      ctx.filter = 'blur(3px)'; ctx.drawImage(maskCv, dx, dy, dw, dh); ctx.filter = 'none';
      ctx.globalCompositeOperation = 'source-in'; ctx.drawImage(video, dx, dy, dw, dh); ctx.globalCompositeOperation = 'source-over';
    }, 120);
  }
  function stopHand() { clearInterval(handTimer); handTimer = 0; clearHand(); }

  async function usePhoto() {
    const f = await pickFile('image/*', 'environment');
    if (!f) return;
    stopCamera(); stopTracking(); follow = false;
    bgPhoto.src = URL.createObjectURL(f);
    await bgPhoto.decode().catch(() => {});
    if (frozen) setFrozen(false);
    bgPhoto.hidden = false; video.hidden = true; mode = 'photo'; hideMsg(); tl.stop(); tlBox.checked = false; tlRow.hidden = true;
    camOnly[1].hidden = true; camOnly[2].hidden = true; camOnly[0].hidden = false;
    setModeStatus();
    toast('Align the guide over your photo. Tip: tap Detect paper.');
  }

  async function startXrMode() {
    try {
      stopCamera(); stopTracking();
      const sizeMeters = { w: layout.drawW / 1000, h: layout.drawH / 1000 };
      overlay.hidden = true; surface.hidden = true; video.hidden = true;
      xr = await startXR({
        overlayCanvas: overlay, domRoot: screen, sizeMeters,
        get: () => ({ opacity: ar.opacity, scale: simple.scale, rotationDeg: simple.rotation, locked: !!ar.alignmentLocked }),
        onStatus: (s, t) => setStatus(s, t),
        onEnd: () => { xr = null; overlay.hidden = false; surface.hidden = false; mode = 'none'; toast('AR session ended.'); startCamera(); },
      });
      mode = 'webxr';
      camOnly.forEach((el) => { el.hidden = true; });
      if (!xr.hasDomOverlay) toast('Tap the screen to place the guide. Use your phone\'s Back button to leave AR.', { ms: 6000 });
    } catch (e) {
      overlay.hidden = false; surface.hidden = false;
      toast('AR could not start: ' + e.message + ' Using the camera overlay instead.', { error: true, ms: 6000 });
      startCamera();
    }
  }
  let startXR = null;

  // ----- boot -----
  if (!app.renders.original) await app.loadCachedRenders();
  rebuildOverlay();
  if (ar.corners) paperCorners = ar.corners.map(([x, y]) => [x * screen.clientWidth, y * screen.clientHeight]);
  updateHandles();
  const cap = await probeAr();
  if (cap.mode === 'webxr') {
    startXR = (await import('../webxr.js')).startXR;
    xrBtn.hidden = false;
    toast('This device supports AR with surface tracking. Tap “Start AR” to try it.', { ms: 5000 });
  }
  if (cap.mode === 'camera' || cap.mode === 'webxr') await startCamera();
  else {
    mode = 'none'; setModeStatus();
    camOnly.forEach((el) => { el.hidden = true; });
    showMsg(h('p', h('strong', 'AR unavailable. '), cap.reasons.join(' ')),
      h('div.row', { style: { justifyContent: 'center' } },
        h('button.btn.small', { type: 'button', on: { click: usePhoto } }, 'Align over a photo of my paper'),
        h('button.btn.small', { type: 'button', on: { click: () => nav('locked') } }, 'Use locked reference')));
  }

  const onVis = () => {
    if (document.hidden) return;
    if (mode === 'camera' && stream && stream.getVideoTracks()[0].readyState === 'ended') startCamera();
  };
  document.addEventListener('visibilitychange', onVis);
  const ro = new ResizeObserver(() => { place(); applyZoom(); }); ro.observe(screen);
  return () => {
    tl.stop(); clearInterval(tlTimer); stopHand(); stopHints();
    stopTracking(); stopCamera(); wake.release(); ro.disconnect();
    document.removeEventListener('visibilitychange', onVis);
    if (xr) xr.end();
    app.saveProject().catch(() => {});
  };
}
