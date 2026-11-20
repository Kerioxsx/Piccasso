// Reference-canvas interaction lock. A pure state machine so the behaviour is
// testable without a DOM: while locked, gestures that would move/zoom/scroll
// the reference are rejected; deliberate controls (unlock, exit, step return,
// brightness) stay available. Unlock can require a press-and-hold.

export const LOCKED = 'locked';
export const UNLOCKED = 'unlocked';
export const UNLOCKING = 'unlocking';

/** Interactions that change the reference and are blocked while locked. */
export const CANVAS_GESTURES = ['pan', 'zoom', 'rotate', 'scroll', 'doubletap', 'tap'];

export class ReferenceLock {
  constructor({ holdMs = 1200, requireHold = true } = {}) {
    this.holdMs = holdMs;
    this.requireHold = requireHold;
    this.state = UNLOCKED;
    this.holdStart = 0;
    this.listeners = new Set();
  }
  on(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
  _emit() { for (const fn of this.listeners) fn(this.state); }
  get locked() { return this.state !== UNLOCKED; }

  lock() { this.state = LOCKED; this.holdStart = 0; this._emit(); }

  /** Start an unlock. Without requireHold it unlocks immediately. */
  beginUnlock(now) {
    if (this.state === UNLOCKED) return this.state;
    if (!this.requireHold) { this.state = UNLOCKED; this._emit(); return this.state; }
    this.state = UNLOCKING; this.holdStart = now; this._emit();
    return this.state;
  }
  /** Progress 0..1 of an ongoing hold; completes the unlock when it reaches 1. */
  tick(now) {
    if (this.state !== UNLOCKING) return this.state === UNLOCKED ? 1 : 0;
    const p = Math.min(1, (now - this.holdStart) / this.holdMs);
    if (p >= 1) { this.state = UNLOCKED; this._emit(); }
    return p;
  }
  cancelUnlock() {
    if (this.state === UNLOCKING) { this.state = LOCKED; this.holdStart = 0; this._emit(); }
  }
  /** Emergency/accessibility path (e.g. screen-reader double-tap on the unlock button, Escape key). */
  forceUnlock() { this.state = UNLOCKED; this._emit(); }

  allows(kind) {
    if (!this.locked) return true;
    return !CANVAS_GESTURES.includes(kind);
  }
}

/** Apply a view gesture ({type:'pan',dx,dy} | {type:'zoom',factor,cx,cy}) unless locked. */
export function applyGesture(view, gesture, lock) {
  if (lock && !lock.allows(gesture.type)) return view;
  if (gesture.type === 'pan') return { ...view, panX: view.panX + gesture.dx, panY: view.panY + gesture.dy };
  if (gesture.type === 'zoom') {
    const z = Math.min(8, Math.max(0.2, view.zoom * gesture.factor));
    const f = z / view.zoom;
    const cx = gesture.cx || 0, cy = gesture.cy || 0;
    return { ...view, zoom: z, panX: cx - (cx - view.panX) * f, panY: cy - (cy - view.panY) * f };
  }
  return view;
}
