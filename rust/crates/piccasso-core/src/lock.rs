//! Reference-canvas interaction lock — port of `web/js/core/lock.js`.
//! While locked, gestures that move/zoom the reference are rejected; deliberate
//! controls (unlock, exit, return to step) are always allowed.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    Unlocked,
    Locked,
    Unlocking,
}

/// Interactions that change the reference.
pub const CANVAS_GESTURES: [&str; 6] = ["pan", "zoom", "rotate", "scroll", "doubletap", "tap"];

#[derive(Debug, Clone)]
pub struct ReferenceLock {
    pub hold: Duration,
    pub require_hold: bool,
    pub state: LockState,
    hold_start: Option<Duration>,
}

impl ReferenceLock {
    pub fn new(hold: Duration, require_hold: bool) -> Self {
        ReferenceLock { hold, require_hold, state: LockState::Unlocked, hold_start: None }
    }
    pub fn locked(&self) -> bool {
        self.state != LockState::Unlocked
    }
    pub fn lock(&mut self) {
        self.state = LockState::Locked;
        self.hold_start = None;
    }
    /// `now` is any monotonic timestamp (e.g. time since app start).
    pub fn begin_unlock(&mut self, now: Duration) -> LockState {
        if self.state == LockState::Unlocked {
            return self.state;
        }
        if !self.require_hold {
            self.state = LockState::Unlocked;
        } else {
            self.state = LockState::Unlocking;
            self.hold_start = Some(now);
        }
        self.state
    }
    /// Hold progress 0..1; completes the unlock at 1.
    pub fn tick(&mut self, now: Duration) -> f32 {
        match (self.state, self.hold_start) {
            (LockState::Unlocking, Some(start)) => {
                let p = (now.saturating_sub(start).as_secs_f32() / self.hold.as_secs_f32().max(1e-6)).min(1.0);
                if p >= 1.0 {
                    self.state = LockState::Unlocked;
                    self.hold_start = None;
                }
                p
            }
            (LockState::Unlocked, _) => 1.0,
            _ => 0.0,
        }
    }
    pub fn cancel_unlock(&mut self) {
        if self.state == LockState::Unlocking {
            self.state = LockState::Locked;
            self.hold_start = None;
        }
    }
    pub fn force_unlock(&mut self) {
        self.state = LockState::Unlocked;
        self.hold_start = None;
    }
    pub fn allows(&self, kind: &str) -> bool {
        !self.locked() || !CANVAS_GESTURES.contains(&kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
}
impl Default for View {
    fn default() -> Self {
        View { zoom: 1.0, pan_x: 0.0, pan_y: 0.0 }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Gesture {
    Pan { dx: f64, dy: f64 },
    Zoom { factor: f64, cx: f64, cy: f64 },
}

pub fn apply_gesture(view: View, g: Gesture, lock: Option<&ReferenceLock>) -> View {
    let kind = match g { Gesture::Pan { .. } => "pan", Gesture::Zoom { .. } => "zoom" };
    if let Some(l) = lock {
        if !l.allows(kind) {
            return view;
        }
    }
    match g {
        Gesture::Pan { dx, dy } => View { pan_x: view.pan_x + dx, pan_y: view.pan_y + dy, ..view },
        Gesture::Zoom { factor, cx, cy } => {
            let z = (view.zoom * factor).clamp(0.2, 8.0);
            let f = z / view.zoom;
            View { zoom: z, pan_x: cx - (cx - view.pan_x) * f, pan_y: cy - (cy - view.pan_y) * f }
        }
    }
}
