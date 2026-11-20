// Locked Reference Mode: a steady, touch-locked guide for drawing on paper.
// The lock applies to the reference canvas only; Unlock, Exit and Return-to-step
// remain deliberate, accessible controls. The screen is kept awake with the
// Screen Wake Lock API where the browser allows it.
import { app } from '../../state.js';
import { h, icon, slider, segmented, toggle, nav, announce, toast, fmtTime } from '../dom.js';
import { computeRect, fitCanvas, drawLayers } from '../canvas.js';
import { attachGestures } from '../gestures.js';
import { ReferenceLock, LOCKED, UNLOCKED, UNLOCKING } from '../../core/lock.js';
import { WakeLock } from '../wakelock.js';

export function renderLocked(main) {
  const p = app.project;
  document.body.classList.add('immersive');
  const lock = new ReferenceLock({ holdMs: app.settings.holdMs, requireHold: app.settings.holdToUnlock });
  let view = { zoom: p.view.zoom || 1, panX: p.view.panX || 0, panY: p.view.panY || 0 };
  let layer = 'step';
  let darkBg = false;
  const settings = { brightness: p.view.brightness || 100, contrast: p.view.contrast || 100, opacity: p.view.opacity || 100 };
  let timerOn = app.settings.lockTimer, startedAt = Date.now(), raf = 0;

  const canvas = h('canvas', { role: 'img', 'aria-label': 'Drawing reference' });
  const canvasWrap = h('div.locked-canvas', canvas);
  const status = h('span.lock-status', { role: 'status' });
  const wakeStatus = h('span.pill', { role: 'status' });
  const timerEl = h('span.pill.timer', { 'aria-label': 'Session time' });
  const fill = h('span.fill');
  const unlockBtn = h('button.btn.hold-btn', { type: 'button' }, fill, icon('unlock'), h('span', 'Hold to unlock'));
  const lockBtn = h('button.btn.primary', { type: 'button', on: { click: doLock } }, icon('lock'), 'Lock screen');
  const exitBtn = h('button.btn', { type: 'button', on: { click: exit } }, icon('close'), 'Exit');
  const backBtn = h('button.btn', { type: 'button', on: { click: () => nav('workspace') } }, icon('steps'), 'Back to step');
  const step = p.steps[p.progress.currentStep];

  const wake = new WakeLock((st) => {
    if (st.active) { wakeStatus.className = 'pill ok'; wakeStatus.textContent = 'Screen stays on'; }
    else if (st.reason === 'unsupported') { wakeStatus.className = 'pill warn'; wakeStatus.textContent = 'Can\'t keep screen on'; wakeStatus.title = 'This browser does not support keeping the screen awake. Increase your device\'s auto-lock time in system settings while drawing.'; }
    else if (st.reason === 'denied') { wakeStatus.className = 'pill warn'; wakeStatus.textContent = 'Screen may turn off'; wakeStatus.title = 'The browser refused to keep the screen on (battery saver may be on). ' + (st.message || ''); }
    else { wakeStatus.className = 'pill'; wakeStatus.textContent = 'Screen may turn off'; }
  });

  const screen = h('div.locked-screen',
    h('div.lock-bar', status, wakeStatus, timerOn ? timerEl : null, backBtn, lockBtn, unlockBtn, exitBtn),
    canvasWrap,
    h('div.lock-controls',
      segmented('Show', [['step', 'Current step'], ['reference', 'Original'], ['outline', 'Outline'], ['sketch', 'Sketch'], ['values', 'Shading']], layer, (v) => { layer = v; draw(); }),
      slider({ label: 'Brightness', min: 50, max: 150, value: settings.brightness, format: (v) => v + '%', onInput: (v) => { settings.brightness = v; draw(); } }),
      slider({ label: 'Contrast', min: 50, max: 200, value: settings.contrast, format: (v) => v + '%', onInput: (v) => { settings.contrast = v; draw(); } }),
      slider({ label: 'Opacity', min: 10, max: 100, value: settings.opacity, format: (v) => v + '%', onInput: (v) => { settings.opacity = v; draw(); } }),
      h('div.stack',
        h('div.row', h('button.btn.small', { type: 'button', on: { click: () => { view = { zoom: 1, panX: 0, panY: 0 }; draw(); } } }, icon('fit'), 'Fit to page'),
          h('button.btn.small', { type: 'button', on: { click: () => { view = { ...view, zoom: Math.min(8, view.zoom * 1.25) }; draw(); } } }, icon('zoomin'), 'Zoom'),
          h('button.btn.small', { type: 'button', on: { click: () => { view = { ...view, zoom: Math.max(0.2, view.zoom * 0.8) }; draw(); } } }, icon('zoomout'))),
        h('p.small.muted', 'Drag or pinch to position, then lock.')),
      toggle('Dark surround', darkBg, (v) => { darkBg = v; screen.classList.toggle('dark-bg', v); }),
      toggle('Session timer', timerOn, (v) => { timerOn = v; app.settings.lockTimer = v; app.saveSettings(); timerEl.hidden = !v; }),
      h('p.small.muted', { style: { gridColumn: '1 / -1' } }, 'Brightness here only changes the picture. To make the whole screen brighter, use your device\'s brightness control.')));
  if (!timerOn) timerEl.hidden = true;
  main.appendChild(screen);

  function baseFor() {
    if (layer === 'step') return app.baseCanvas(step ? step.base : 'light');
    if (layer === 'reference') return app.baseCanvas('original');
    return app.baseCanvas(layer);
  }

  function draw() {
    const { ctx, w, h: hh } = fitCanvas(canvas);
    ctx.clearRect(0, 0, w, hh);
    const ref = app.renders.original || app.srcCanvas;
    const rect = computeRect(w, hh, ref.width, ref.height, view, 8);
    ctx.save();
    ctx.filter = `brightness(${settings.brightness}%) contrast(${settings.contrast}%)`;
    ctx.fillStyle = '#fff'; ctx.fillRect(rect.x, rect.y, rect.w, rect.h);
    const base = baseFor();
    if (base) { ctx.globalAlpha = settings.opacity / 100; ctx.drawImage(base, rect.x, rect.y, rect.w, rect.h); ctx.globalAlpha = 1; }
    ctx.filter = 'none';
    if (layer === 'step' && step) drawLayers(ctx, p.analysis, step.layers, rect, { alpha: settings.opacity / 100 });
    ctx.restore();
  }

  function renderState(state) {
    const locked = state !== UNLOCKED;
    screen.classList.toggle('is-locked', locked);
    status.innerHTML = '';
    status.append(icon(locked ? 'lock' : 'unlock'), document.createTextNode(state === UNLOCKING ? 'Keep holding…' : locked ? 'Locked: the picture won\'t move' : 'Unlocked: adjust, then lock'));
    lockBtn.hidden = locked; unlockBtn.hidden = !locked;
    unlockBtn.setAttribute('aria-label', app.settings.holdToUnlock ? 'Unlock. Press and hold for about one second.' : 'Unlock');
    unlockBtn.lastChild.textContent = app.settings.holdToUnlock ? 'Hold to unlock' : 'Unlock';
    canvas.setAttribute('aria-label', locked ? 'Drawing reference, locked' : 'Drawing reference. Drag to move, pinch to zoom.');
    if (state === LOCKED) fill.style.transform = 'scaleX(0)';
  }
  lock.on((s) => { renderState(s); if (s === LOCKED) announce('Reference locked'); if (s === UNLOCKED) announce('Reference unlocked'); });

  async function doLock() {
    lock.lock();
    if (app.settings.keepAwake) await wake.request();
    try { if (!document.fullscreenElement && screen.requestFullscreen) await screen.requestFullscreen({ navigationUI: 'hide' }); } catch { /* optional */ }
    p.view = { ...p.view, ...view, ...settings };
    app.saveProject().catch(() => {});
  }

  // Hold-to-unlock (pointer, keyboard Space/Enter held, or a click when hold is off)
  let holdRaf = 0;
  const holdTick = () => {
    const prog = lock.tick(performance.now());
    fill.style.transform = `scaleX(${prog})`;
    if (lock.state === UNLOCKING) holdRaf = requestAnimationFrame(holdTick);
  };
  const startHold = (e) => { e.preventDefault(); if (lock.beginUnlock(performance.now()) === UNLOCKING) holdRaf = requestAnimationFrame(holdTick); };
  const endHold = () => { cancelAnimationFrame(holdRaf); if (lock.state === UNLOCKING) { lock.cancelUnlock(); toast('Keep holding the button to unlock.'); } };
  unlockBtn.addEventListener('pointerdown', startHold);
  unlockBtn.addEventListener('pointerup', endHold);
  unlockBtn.addEventListener('pointerleave', endHold);
  unlockBtn.addEventListener('pointercancel', endHold);
  unlockBtn.addEventListener('keydown', (e) => { if ((e.key === ' ' || e.key === 'Enter') && !e.repeat) startHold(e); });
  unlockBtn.addEventListener('keyup', (e) => { if (e.key === ' ' || e.key === 'Enter') endHold(); });
  // Screen readers activate with a synthetic click (no hold possible): allow a confirmed unlock instead.
  unlockBtn.addEventListener('click', (e) => { if (e.detail === 0 && lock.locked && lock.state !== UNLOCKING) lock.forceUnlock(); });

  async function exit() {
    await cleanup();
    nav('workspace');
  }

  const detach = attachGestures(canvasWrap, { getView: () => view, setView: (v) => { view = v; draw(); }, lock, onReset: () => { view = { zoom: 1, panX: 0, panY: 0 }; draw(); } });
  const tickTimer = () => { timerEl.textContent = fmtTime((Date.now() - startedAt) / 1000); raf = setTimeout(tickTimer, 1000); };
  tickTimer();
  const ro = new ResizeObserver(draw); ro.observe(canvasWrap);
  const onKey = (e) => { if (e.key === 'Escape' && !lock.locked) exit(); };
  document.addEventListener('keydown', onKey);
  renderState(lock.state);
  draw();
  if (!app.renders.original) app.loadCachedRenders().then(draw);

  let cleaned = false;
  async function cleanup() {
    if (cleaned) return; cleaned = true;
    detach(); ro.disconnect(); clearTimeout(raf); cancelAnimationFrame(holdRaf);
    document.removeEventListener('keydown', onKey);
    await wake.release();
    try { if (document.fullscreenElement) await document.exitFullscreen(); } catch { /* ignore */ }
    p.view = { ...p.view, ...view, ...settings };
    p.progress.timeSpentSec += Math.round((Date.now() - startedAt) / 1000);
    app.saveProject().catch(() => {});
  }
  return () => { cleanup(); };
}
