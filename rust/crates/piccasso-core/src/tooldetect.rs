//! Reading another program's tools from a screenshot: finding colour pickers
//! (wheels, hue strips, saturation/brightness squares, colour sliders), finding
//! the pixel of a picker closest to a wanted colour, measuring a test brush
//! stroke, and turning measured brush widths into a slider curve (for size
//! sliders that show no pixel numbers). Written for Piccasso; no third-party code.
use crate::imgproc::Rgba;
use crate::lines::color_dist;

/// A screen rectangle: x, y, width, height (screen pixels).
pub type Area = [i32; 4];

fn px(img: &Rgba, x: i32, y: i32) -> [f64; 3] {
    let i = (y as usize * img.width + x as usize) * 4;
    [img.data[i] as f64, img.data[i + 1] as f64, img.data[i + 2] as f64]
}

fn clip(img: &Rgba, a: Area) -> (i32, i32, i32, i32) {
    let x0 = a[0].clamp(0, img.width as i32 - 1);
    let y0 = a[1].clamp(0, img.height as i32 - 1);
    let x1 = (a[0] + a[2]).clamp(x0 + 1, img.width as i32);
    let y1 = (a[1] + a[3]).clamp(y0 + 1, img.height as i32);
    (x0, y0, x1, y1)
}

fn hsv(c: [f64; 3]) -> (f64, f64, f64) {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    let h = if d < 1e-9 { 0.0 } else if mx == c[0] { ((c[1] - c[2]) / d).rem_euclid(6.0) } else if mx == c[1] { (c[2] - c[0]) / d + 2.0 } else { (c[0] - c[1]) / d + 4.0 };
    (h * 60.0, if mx < 1e-9 { 0.0 } else { d / mx }, mx / 255.0)
}

/// How a picker area responds: a hue area (wheel, hue strip) shows many hues
/// at full brightness, so it is matched on hue and saturation only; a shade
/// area (saturation/brightness square, brightness slider) is matched on the
/// whole colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind { Hue, Shade }

pub fn picker_kind(img: &Rgba, a: Area) -> PickerKind {
    let (x0, y0, x1, y1) = clip(img, a);
    let mut bins = [0usize; 12];
    let mut n = 0usize;
    let step = (((x1 - x0) * (y1 - y0)) as f64 / 4000.0).sqrt().max(1.0) as i32;
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let (h, s, v) = hsv(px(img, x, y));
            if s > 0.35 && v > 0.35 { bins[((h / 30.0) as usize).min(11)] += 1; }
            n += 1;
            x += step;
        }
        y += step;
    }
    let present = bins.iter().filter(|&&b| b * 60 > n).count();
    if present >= 5 { PickerKind::Hue } else { PickerKind::Shade }
}

/// Pixel of `a` closest to `want`, and how close it is (colour distance; 0 = exact).
pub fn best_pixel(img: &Rgba, a: Area, want: [u8; 3], kind: PickerKind) -> ((i32, i32), f64) {
    let (x0, y0, x1, y1) = clip(img, a);
    let w = [want[0] as f64, want[1] as f64, want[2] as f64];
    let (wh, ws, _) = hsv(w);
    let mut best = ((x0, y0), f64::INFINITY);
    // the panel around a round wheel or a thin slider shows in the box's corners:
    // never click it
    let corners = [px(img, x0, y0), px(img, x1 - 1, y0), px(img, x0, y1 - 1), px(img, x1 - 1, y1 - 1)];
    let bg = if corners.iter().all(|c| (0..3).all(|k| (c[k] - corners[0][k]).abs() < 3.0)) { Some(corners[0]) } else { None };
    for y in y0..y1 {
        for x in x0..x1 {
            let c = px(img, x, y);
            if let Some(b) = bg { if (0..3).all(|k| (c[k] - b[k]).abs() < 3.0) { continue; } }
            let d = match kind {
                PickerKind::Shade => color_dist(c, w),
                PickerKind::Hue => {
                    let (h, s, v) = hsv(c);
                    if v < 0.2 { f64::INFINITY } else {
                        let dh = ((h - wh).abs()).min(360.0 - (h - wh).abs()) / 180.0;
                        // gray targets: any hue will do, only saturation counts
                        (dh * ws.min(0.5) * 2.0).powi(2) * 4.0 + (s - ws).powi(2)
                    }
                }
            };
            // ties: prefer the point nearest the area's centre (away from borders)
            if d < best.1 - 1e-9 { best = ((x, y), d); }
        }
    }
    best
}

/// Colour picker widgets on a screenshot: places where colours change smoothly
/// in every pixel (gradients), unlike icons and text, which have few colours.
/// Returns boxes, biggest first.
pub fn detect_color_widgets(img: &Rgba) -> Vec<Area> {
    const B: usize = 8;
    let (bw, bh) = (img.width / B, img.height / B);
    if bw == 0 || bh == 0 { return vec![]; }
    let mut on = vec![0u8; bw * bh];
    for by in 0..bh {
        for bx in 0..bw {
            let (mut lo, mut hi) = ([255f64; 3], [0f64; 3]);
            let mut jumps = 0usize;
            for y in 0..B {
                for x in 0..B {
                    let (xx, yy) = ((bx * B + x) as i32, (by * B + y) as i32);
                    let c = px(img, xx, yy);
                    for k in 0..3 { lo[k] = lo[k].min(c[k]); hi[k] = hi[k].max(c[k]); }
                    for (ox, oy) in [(1, 0), (0, 1)] {
                        if (x as i32) < ox || (y as i32) < oy { continue; }
                        let p = px(img, xx - ox, yy - oy);
                        if (0..3).map(|k| (c[k] - p[k]).abs()).fold(0.0, f64::max) > 40.0 { jumps += 1; }
                    }
                }
            }
            let range = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
            // a gradient: colours keep changing but never jump (icons and text have hard edges)
            if range >= 6.0 && jumps == 0 { on[by * bw + bx] = 1; }
        }
    }
    let (labels, comps) = crate::imgproc::label_components(&on, bw, bh);
    // the gradient itself must span a wide range of colours (subtle shading on
    // toolbars and buttons doesn't), and fill most of its box (a real widget)
    let span = |c: &crate::imgproc::Component| {
        let (mut lo, mut hi) = ([255f64; 3], [0f64; 3]);
        for by in c.min_y..=c.max_y {
            for bx in c.min_x..=c.max_x {
                if labels[by * bw + bx] != c.label { continue; }
                for y in (by * B..(by + 1) * B).step_by(2) {
                    for x in (bx * B..(bx + 1) * B).step_by(2) {
                        let p = px(img, x as i32, y as i32);
                        for k in 0..3 { lo[k] = lo[k].min(p[k]); hi[k] = hi[k].max(p[k]); }
                    }
                }
            }
        }
        (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max)
    };
    let fill = |c: &crate::imgproc::Component| c.area as f64 / ((c.max_x - c.min_x + 1) * (c.max_y - c.min_y + 1)) as f64;
    let mut out: Vec<(u64, Area)> = comps.iter()
        .filter(|c| c.area >= 6 && (c.max_x - c.min_x + 1).max(c.max_y - c.min_y + 1) >= 4 && fill(c) >= 0.35 && span(c) >= 120.0)
        .map(|c| (c.area, [(c.min_x * B) as i32, (c.min_y * B) as i32, ((c.max_x - c.min_x + 1) * B) as i32, ((c.max_y - c.min_y + 1) * B) as i32]))
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, a)| a).take(8).collect()
}

/// Mean colour of a small square around a point (to read a "current colour" swatch).
pub fn sample(img: &Rgba, p: (i32, i32), r: i32) -> [u8; 3] {
    let mut s = [0f64; 3];
    let mut n = 0.0;
    for y in p.1 - r..=p.1 + r {
        for x in p.0 - r..=p.0 + r {
            if x < 0 || y < 0 || x >= img.width as i32 || y >= img.height as i32 { continue; }
            let c = px(img, x, y);
            for k in 0..3 { s[k] += c[k]; }
            n += 1.0;
        }
    }
    let n: f64 = if n > 0.0 { n } else { 1.0 };
    [(s[0] / n).round() as u8, (s[1] / n).round() as u8, (s[2] / n).round() as u8]
}

/// Thickness of a horizontal test stroke at column `x` around row `y`, in
/// pixels of ink: soft (anti-aliased) edge pixels count by how much ink they
/// hold, so a 3.8 px brush measures 3.8, not the 5-6 pixels it touches. `ink`
/// is the paint colour used for the test (a full-ink pixel counts as 1).
pub fn stroke_width(img: &Rgba, x: i32, y: i32, paper: [u8; 3], ink: [u8; 3], max: i32) -> f64 {
    let p = [paper[0] as f64, paper[1] as f64, paper[2] as f64];
    let ink_d = (0..3).map(|k| (p[k] - ink[k] as f64).abs()).fold(0.0, f64::max);
    let diff = |yy: i32| -> f64 {
        if yy < 0 || yy >= img.height as i32 { return 0.0; }
        let c = px(img, x, yy);
        (0..3).map(|k| (c[k] - p[k]).abs()).fold(0.0, f64::max)
    };
    if diff(y) <= 60.0 { return 0.0; }
    let mut lo = y;
    while y - lo < max && diff(lo - 1) > 12.0 { lo -= 1; }
    let mut hi = y;
    while hi - y < max && diff(hi + 1) > 12.0 { hi += 1; }
    let full = (lo..=hi).map(diff).fold(0.0, f64::max).max(ink_d);
    (lo..=hi).map(|yy| (diff(yy) / full).min(1.0)).sum()
}

/// Measured brush widths along a size slider: (slider position 0..1, width px),
/// sorted by position. Works for any slider shape (linear, squared, logarithmic)
/// because nothing is assumed about the numbers it shows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SizeCurve { pub points: Vec<(f64, f64)> }

impl SizeCurve {
    pub fn new(mut points: Vec<(f64, f64)>) -> Self {
        points.retain(|p| p.1 > 0.0);
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        // widths must not shrink as the slider grows (measurement noise)
        for i in 1..points.len() { if points[i].1 < points[i - 1].1 { points[i].1 = points[i - 1].1; } }
        SizeCurve { points }
    }
    pub fn is_empty(&self) -> bool { self.points.len() < 2 }
    pub fn range(&self) -> Option<(f64, f64)> { Some((self.points.first()?.1, self.points.last()?.1)) }
    /// Slider position for a brush of `px` (interpolated in log width).
    pub fn position_for(&self, px: f64) -> Option<f64> {
        if self.is_empty() { return None; }
        let p = &self.points;
        if px <= p[0].1 { return Some(p[0].0); }
        for w in p.windows(2) {
            let ((t0, a), (t1, b)) = (w[0], w[1]);
            if px <= b {
                if (b - a).abs() < 1e-9 { return Some(t0); }
                let f = (px.ln() - a.ln()) / (b.ln() - a.ln());
                return Some(t0 + (t1 - t0) * f);
            }
        }
        Some(p[p.len() - 1].0)
    }
    /// Width the slider gives at position `t` (for previews).
    pub fn width_at(&self, t: f64) -> Option<f64> {
        if self.is_empty() { return None; }
        let p = &self.points;
        if t <= p[0].0 { return Some(p[0].1); }
        for w in p.windows(2) {
            let ((t0, a), (t1, b)) = (w[0], w[1]);
            if t <= t1 { let f = (t - t0) / (t1 - t0).max(1e-9); return Some((a.ln() + (b.ln() - a.ln()) * f).exp()); }
        }
        Some(p[p.len() - 1].1)
    }
}

/// Slider positions to test when calibrating (denser at the small end, where
/// most sliders put the useful sizes).
pub fn calibration_positions() -> Vec<f64> { vec![0.0, 0.04, 0.1, 0.18, 0.3, 0.45, 0.65, 0.85, 1.0] }

/// Test pattern for measuring how fast a program can be drawn in: a zigzag
/// with sharp corners followed by a tight wave, inside `a` (screen box).
pub fn speed_test_path(a: Area) -> Vec<[f64; 2]> {
    let (x0, y0, w, h) = (a[0] as f64 + 10.0, a[1] as f64 + 10.0, a[2] as f64 - 20.0, a[3] as f64 - 20.0);
    let mut p = Vec::new();
    let teeth = 6;
    for i in 0..=teeth { p.push([x0 + w * 0.55 * i as f64 / teeth as f64, if i % 2 == 0 { y0 + h } else { y0 }]); }
    // a wave of radius ≈ h/6 (tight curves)
    let (wx, ww) = (x0 + w * 0.6, w * 0.4);
    for k in 1..=60 {
        let t = k as f64 / 60.0;
        p.push([wx + ww * t, y0 + h * 0.5 - (t * std::f64::consts::TAU * 2.0).sin() * h * 0.4]);
    }
    p
}

/// How well a test stroke came out, from screenshots before and after it:
/// (share of the intended centre line that got ink, share of the new ink that
/// lies farther than `band` px from the intended line).
pub fn score_stroke(before: &Rgba, after: &Rgba, path: &[[f64; 2]], band: f64) -> (f64, f64) {
    let changed = |x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= after.width as i32 || y >= after.height as i32 { return false; }
        let (a, b) = (px(after, x, y), px(before, x, y));
        (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max) > 50.0
    };
    let dense = crate::strokes::densify(path, 1.0);
    // centre line: count a point as inked if any pixel within 1 px changed
    let hit = dense.iter().filter(|q| {
        let (x, y) = (q[0].round() as i32, q[1].round() as i32);
        (-1..=1).any(|dy| (-1..=1).any(|dx| changed(x + dx, y + dy)))
    }).count() as f64 / dense.len().max(1) as f64;
    // stray ink: new ink far from every intended segment
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in path { minx = minx.min(q[0]); miny = miny.min(q[1]); maxx = maxx.max(q[0]); maxy = maxy.max(q[1]); }
    let m = band + 12.0;
    let seg_d = |p: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 < 1e-12 { 0.0 } else { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) };
        (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
    };
    let (mut ink, mut stray) = (0usize, 0usize);
    for y in (miny - m) as i32..=(maxy + m) as i32 {
        for x in (minx - m) as i32..=(maxx + m) as i32 {
            if !changed(x, y) { continue; }
            ink += 1;
            let p = [x as f64 + 0.5, y as f64 + 0.5];
            if path.windows(2).all(|s| seg_d(p, s[0], s[1]) > band) { stray += 1; }
        }
    }
    (hit, if ink == 0 { 1.0 } else { stray as f64 / ink as f64 })
}

/// Settings tried by the speed test, fastest first: (mouse readings per second
/// assumed, top speed in px/s).
pub fn speed_trials() -> Vec<(f64, f64)> {
    vec![(1000.0, 24000.0), (500.0, 16000.0), (250.0, 10000.0), (125.0, 7000.0), (60.0, 5000.0), (30.0, 3000.0)]
}
