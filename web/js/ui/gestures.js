// Pointer gestures for a stage: one-finger/mouse pan, two-finger pinch zoom,
// wheel zoom and double-tap reset. Every gesture goes through applyGesture(),
// so a ReferenceLock (when given) can veto it.
import { applyGesture } from '../core/lock.js';

export function attachGestures(el, { getView, setView, lock = null, onReset }) {
  const pts = new Map();
  let last = null, lastTap = 0;
  const apply = (g) => {
    if (lock && !lock.allows(g.type)) return;
    setView(applyGesture(getView(), g, lock));
  };
  const local = (e) => { const r = el.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; };
  const down = (e) => {
    if (lock && lock.locked) { e.preventDefault(); return; }
    el.setPointerCapture(e.pointerId);
    pts.set(e.pointerId, local(e));
    const now = performance.now();
    if (pts.size === 1 && now - lastTap < 300) { if (!lock || lock.allows('doubletap')) onReset && onReset(); lastTap = 0; }
    else lastTap = now;
    last = snapshot();
  };
  const snapshot = () => {
    const a = [...pts.values()];
    if (a.length === 1) return { cx: a[0][0], cy: a[0][1], d: 0 };
    if (a.length >= 2) return { cx: (a[0][0] + a[1][0]) / 2, cy: (a[0][1] + a[1][1]) / 2, d: Math.hypot(a[0][0] - a[1][0], a[0][1] - a[1][1]) };
    return null;
  };
  const move = (e) => {
    if (!pts.has(e.pointerId)) return;
    if (lock && lock.locked) { e.preventDefault(); return; }
    pts.set(e.pointerId, local(e));
    const now = snapshot();
    if (!now || !last) { last = now; return; }
    apply({ type: 'pan', dx: now.cx - last.cx, dy: now.cy - last.cy });
    if (now.d && last.d) apply({ type: 'zoom', factor: now.d / last.d, cx: now.cx - el.clientWidth / 2, cy: now.cy - el.clientHeight / 2 });
    last = now;
  };
  const up = (e) => { pts.delete(e.pointerId); last = snapshot(); };
  const wheel = (e) => {
    e.preventDefault();
    if (lock && lock.locked) return;
    const [x, y] = local(e);
    apply({ type: 'zoom', factor: Math.exp(-e.deltaY * 0.0015), cx: x - el.clientWidth / 2, cy: y - el.clientHeight / 2 });
  };
  const block = (e) => { if (lock && lock.locked) e.preventDefault(); };
  el.addEventListener('pointerdown', down);
  el.addEventListener('pointermove', move);
  el.addEventListener('pointerup', up);
  el.addEventListener('pointercancel', up);
  el.addEventListener('wheel', wheel, { passive: false });
  el.addEventListener('touchmove', block, { passive: false });
  el.addEventListener('gesturestart', block); // iOS Safari pinch
  el.addEventListener('contextmenu', block);
  return () => {
    el.removeEventListener('pointerdown', down); el.removeEventListener('pointermove', move);
    el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', up);
    el.removeEventListener('wheel', wheel); el.removeEventListener('touchmove', block);
    el.removeEventListener('gesturestart', block); el.removeEventListener('contextmenu', block);
  };
}
