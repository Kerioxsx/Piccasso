//! Hand-motion paths (port of `web/js/core/motion.js`), also used as the
//! stroke source for the desktop auto-draw feature.
use crate::analysis::{Analysis, Shape};
use crate::project::Step;

#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub kind: &'static str,
    pub points: Vec<[f64; 2]>,
}

pub fn shape_path(s: &Shape, aspect: f64, n: usize) -> Vec<[f64; 2]> {
    if s.kind == "box" {
        let (x0, x1, y0, y1) = (s.cx - s.rx, s.cx + s.rx, s.cy - s.ry * aspect, s.cy + s.ry * aspect);
        return vec![[s.cx, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0], [s.cx, y0]];
    }
    let t = s.angle.to_radians();
    (0..=n).map(|k| {
        let a = -std::f64::consts::FRAC_PI_2 + k as f64 / n as f64 * std::f64::consts::TAU;
        let (ex, ey) = (a.cos() * s.rx, a.sin() * s.ry);
        [s.cx + ex * t.cos() - ey * t.sin(), s.cy + (ex * t.sin() + ey * t.cos()) * aspect]
    }).collect()
}

pub fn start_at_top(points: &[[f64; 2]], closed: bool) -> Vec<[f64; 2]> {
    if !closed || points.len() < 3 {
        return points.to_vec();
    }
    let mut k = 0;
    for i in 1..points.len() {
        if points[i][1] < points[k][1] || (points[i][1] == points[k][1] && points[i][0] < points[k][0]) { k = i; }
    }
    let mut out: Vec<[f64; 2]> = points[k..].iter().chain(points[..k].iter()).copied().collect();
    out.push(out[0]);
    out
}

pub fn hatch_paths(b: [f64; 4], light_dir: &str, count: usize) -> Vec<Vec<[f64; 2]>> {
    let from_left = light_dir.as_bytes().get(1) == Some(&b'l');
    let (w, h) = (b[2] - b[0], b[3] - b[1]);
    (0..count).map(|i| {
        let f = (i as f64 + 0.5) / count as f64;
        let ya = b[1] + h * (f - 0.18).max(0.0);
        let yb = b[1] + h * (f + 0.18).min(1.0);
        let xa = if from_left { b[0] + w * 0.55 } else { b[0] + w * 0.45 };
        let xb = if from_left { b[2] - w * 0.02 } else { b[0] + w * 0.02 };
        let mut seg = vec![[xa, yb], [xb, ya]];
        if i % 2 == 1 { seg.reverse(); }
        seg
    }).collect()
}

pub fn motion_paths(a: &Analysis, step: &Step) -> Vec<Stroke> {
    let has = |l: &str| step.layers.iter().any(|x| x == l);
    let aspect = if a.aspect > 0.0 { a.aspect } else { 1.0 };
    let mut out = Vec::new();
    if step.kind == "page" {
        for p in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            out.push(Stroke { kind: "dot", points: vec![p, p] });
        }
        return out;
    }
    let shading = ["pressure", "values", "color", "details", "clean"].contains(&step.kind.as_str());
    if !shading && has("shapes") {
        let mut shapes = a.shapes.clone();
        shapes.sort_by(|x, y| (y.rx * y.ry).partial_cmp(&(x.rx * x.ry)).unwrap());
        for s in &shapes { out.push(Stroke { kind: "shape", points: shape_path(s, aspect, 48) }); }
    }
    if !shading && has("silhouette") {
        for c in &a.silhouettes { out.push(Stroke { kind: "outline", points: start_at_top(&c.points, c.closed) }); }
    }
    if !shading && has("internal") {
        for c in &a.internals { out.push(Stroke { kind: "inner", points: start_at_top(&c.points, c.closed) }); }
    }
    if out.is_empty() {
        let b = a.subject.bbox;
        let (cx, cy) = a.technique.bands.first().map(|d| (d.x, d.y)).unwrap_or(((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0));
        let r = (b[2] - b[0]).min(b[3] - b[1]) * 0.22;
        for seg in hatch_paths([cx - r, cy - r, cx + r, cy + r], &a.values.light_dir, 9) {
            out.push(Stroke { kind: "hatch", points: seg });
        }
    }
    out
}
