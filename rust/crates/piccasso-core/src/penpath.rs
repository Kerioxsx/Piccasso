//! Timing for real mouse strokes. Drawing programs read the mouse only `hz`
//! times a second and join the readings with straight lines, so a fast pen
//! cuts corners and flattens curves. This plans a speed for every point of a
//! stroke: full speed on straight runs, just slow enough on curves and corners
//! that the program's straight segments stay within `tol` pixels of the path,
//! with smooth acceleration in between. The pen then moves continuously (no
//! stops, no point-by-point jumps). Written for Piccasso; no third-party code.

/// Speed limits for a stroke.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenLimits {
    /// Top speed in px/s.
    pub speed: f64,
    /// How many times a second the target program reads the mouse.
    pub hz: f64,
    /// Allowed deviation in px.
    pub tol: f64,
    /// Distance over which the pen goes from stop to full speed (px).
    pub ramp_px: f64,
}
impl Default for PenLimits {
    fn default() -> Self { PenLimits { speed: 3000.0, hz: 60.0, tol: 0.5, ramp_px: 40.0 } }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 { (b[0] - a[0]).hypot(b[1] - a[1]) }

/// Highest speed allowed at each vertex, and which vertices are corners where
/// the pen pauses for one reading of the program (so the corner is recorded
/// exactly and the straight runs on both sides need no slowing down).
pub fn vertex_speeds(pts: &[[f64; 2]], lim: &PenLimits) -> (Vec<f64>, Vec<bool>) {
    let n = pts.len();
    let mut v = vec![lim.speed; n];
    let mut corner = vec![false; n];
    for i in 1..n.saturating_sub(1) {
        let (a, b, c) = (pts[i - 1], pts[i], pts[i + 1]);
        let (ux, uy, wx, wy) = (b[0] - a[0], b[1] - a[1], c[0] - b[0], c[1] - b[1]);
        let (lu, lw) = (ux.hypot(uy), wx.hypot(wy));
        if lu < 1e-9 || lw < 1e-9 { continue; }
        let cos = ((ux * wx + uy * wy) / (lu * lw)).clamp(-1.0, 1.0);
        let turn = cos.acos();
        let half = (turn / 2.0).sin();
        if half < 1e-6 { continue; }
        // the most a straight reading could miss this vertex by is min(len)·sin(turn/2)
        if lu.min(lw) * half > lim.tol {
            corner[i] = true;
            continue;
        }
        // part of a curve of radius ≈ len / turn: the program's chord sagitta
        // s²/8R stays ≤ tol when the pen moves s = v/hz per reading
        let r = lu.min(lw) / turn.max(1e-6);
        v[i] = lim.speed.min(lim.hz * (8.0 * r * lim.tol).sqrt());
    }
    // acceleration limit in both directions: v² ≤ v_prev² + 2·a·d
    let acc = lim.speed * lim.speed / (2.0 * lim.ramp_px.max(1.0));
    for i in 1..n { let d = dist(pts[i - 1], pts[i]); v[i] = v[i].min((v[i - 1] * v[i - 1] + 2.0 * acc * d).sqrt()); }
    for i in (0..n.saturating_sub(1)).rev() { let d = dist(pts[i], pts[i + 1]); v[i] = v[i].min((v[i + 1] * v[i + 1] + 2.0 * acc * d).sqrt()); }
    (v, corner)
}

/// Positions of the pen every `dt` seconds along the stroke. Corners appear as
/// the same point repeated for a little over one reading period.
pub fn timed_points(pts: &[[f64; 2]], lim: &PenLimits, dt: f64) -> Vec<[f64; 2]> {
    // repeated points would hide the corner they sit on (no turn can be measured
    // across a zero-length segment), so drop them first
    let mut uniq: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for &p in pts { if uniq.last().map_or(true, |q| dist(*q, p) > 0.05) { uniq.push(p); } }
    if uniq.len() == 1 && pts.len() > 1 { uniq.push(*pts.last().unwrap()); }
    let pts = &uniq[..];
    let n = pts.len();
    if n < 2 { return pts.to_vec(); }
    let (v, corner) = vertex_speeds(pts, lim);
    let floor = 20.0;
    let acc = lim.speed * lim.speed / (2.0 * lim.ramp_px.max(1.0));
    let hold = ((1.3 / lim.hz) / dt).ceil() as usize;
    let mut out = vec![pts[0]];
    let mut t_next = dt;
    let mut t = 0.0;
    for i in 0..n - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let len = dist(a, b);
        if len >= 1e-9 {
            // integrate in small slices: accelerate from v0, cap at top speed, brake into v1
            let slices = (len / 0.25).ceil().max(1.0) as usize;
            let (v0, v1) = (v[i].max(floor), v[i + 1].max(floor));
            let speed_at = |sl: f64| (v0 * v0 + 2.0 * acc * sl).sqrt().min((v1 * v1 + 2.0 * acc * (len - sl)).sqrt()).min(lim.speed).max(floor);
            for k in 0..slices {
                let s0 = k as f64 / slices as f64;
                let s1 = (k + 1) as f64 / slices as f64;
                let vm = speed_at(len * (s0 + s1) / 2.0);
                let t1 = t + len * (s1 - s0) / vm;
                while t_next <= t1 {
                    let f = s0 + (s1 - s0) * (t_next - t) / (t1 - t);
                    out.push([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]);
                    t_next += dt;
                }
                t = t1;
            }
        }
        if i + 1 < n - 1 && corner[i + 1] {
            for _ in 0..hold { out.push(b); }
            t = 0.0;
            t_next = dt;
        }
    }
    if out.last() != pts.last() { out.push(pts[n - 1]); }
    out
}

/// Duration of a stroke in seconds.
pub fn stroke_time(pts: &[[f64; 2]], lim: &PenLimits) -> f64 {
    timed_points(pts, lim, 0.001).len() as f64 * 0.001
}

/// What a program that reads the mouse `hz` times a second would draw from a
/// pen moving through `timed` (one point every `dt`): the largest distance of
/// its straight segments from the intended path.
pub fn sampled_error(path: &[[f64; 2]], timed: &[[f64; 2]], dt: f64, hz: f64) -> f64 {
    let every = ((1.0 / hz) / dt).round().max(1.0) as usize;
    let mut worst: f64 = 0.0;
    for phase in (0..every).step_by((every / 4).max(1)) {
        let mut samples: Vec<[f64; 2]> = vec![timed[0]];
        samples.extend(timed.iter().skip(phase).step_by(every).cloned());
        if samples.last() != timed.last() { samples.push(*timed.last().unwrap()); }
        worst = worst.max(chord_error(path, &samples));
    }
    worst
}

fn chord_error(path: &[[f64; 2]], samples: &[[f64; 2]]) -> f64 {
    let seg_d = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    };
    // every point of the intended path must lie near the drawn polyline
    let dense = crate::strokes::densify(path, 0.5);
    let mut worst: f64 = 0.0;
    for p in dense {
        let d = samples.windows(2).map(|s| seg_d(p, s[0], s[1])).fold(f64::INFINITY, f64::min);
        worst = worst.max(d);
    }
    worst
}
