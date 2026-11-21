//! Drawing guides (port of `web/js/core/guides.js`): perspective rays, face
//! and figure proportions, composition grids and the line-weight chart.

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line { a: [f64; 2], b: [f64; 2], w: f64, kind: &'static str },
    Ellipse { c: [f64; 2], rx: f64, ry: f64 },
    Label { at: [f64; 2], text: String },
    Vp { c: [f64; 2], label: String },
}

/// Clip segment a→b to the rectangle [0, 0, w, h] (Liang–Barsky).
pub fn clip_segment(a: [f64; 2], b: [f64; 2], w: f64, h: f64) -> Option<([f64; 2], [f64; 2])> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    for (p, q) in [(-dx, a[0]), (dx, w - a[0]), (-dy, a[1]), (dy, h - a[1])] {
        if p == 0.0 { if q < 0.0 { return None; } continue; }
        let r = q / p;
        if p < 0.0 { if r > t1 { return None; } if r > t0 { t0 = r; } } else { if r < t0 { return None; } if r < t1 { t1 = r; } }
    }
    Some(([a[0] + t0 * dx, a[1] + t0 * dy], [a[0] + t1 * dx, a[1] + t1 * dy]))
}

/// Rays from a vanishing point across the picture, aimed at evenly spaced border points.
pub fn rays_from(vp: [f64; 2], w: f64, h: f64, count: usize) -> Vec<Shape> {
    let far = (w + h) * 4.0;
    let per = ((count as f64 / 4.0).round() as usize).max(2);
    let mut angles: Vec<f64> = Vec::new();
    for i in 0..=per {
        let t = i as f64 / per as f64;
        for (x, y) in [(t * w, 0.0), (t * w, h), (0.0, t * h), (w, t * h)] {
            let a = ((y - vp[1]).atan2(x - vp[0]) * 1000.0).round() / 1000.0;
            if !angles.contains(&a) { angles.push(a); }
        }
    }
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    angles.into_iter().filter_map(|ang| {
        let b = [vp[0] + ang.cos() * far, vp[1] + ang.sin() * far];
        clip_segment(vp, b, w, h).map(|(a, b)| Shape::Line { a, b, w: 1.0, kind: "ray" })
    }).collect()
}

/// Perspective guide with `points` (1–3) vanishing points.
pub fn perspective(points: usize, vps: &[[f64; 2]], horizon_y: f64, count: usize, w: f64, h: f64) -> Vec<Shape> {
    let mut s = Vec::new();
    if let Some((a, b)) = clip_segment([-w * 10.0, horizon_y], [w * 11.0, horizon_y], w, h) {
        s.push(Shape::Line { a, b, w: 2.0, kind: "horizon" });
        s.push(Shape::Label { at: [8.0, horizon_y - 6.0], text: "Horizon (eye level)".into() });
    }
    for (i, vp) in vps.iter().take(points).enumerate() {
        s.extend(rays_from(*vp, w, h, count));
        s.push(Shape::Vp { c: *vp, label: format!("VP{}", i + 1) });
    }
    if points == 1 {
        let n = ((count as f64 / 3.0).round() as usize).max(2);
        for i in 1..n { let x = w * i as f64 / n as f64; s.push(Shape::Line { a: [x, 0.0], b: [x, h], w: 1.0, kind: "vertical" }); }
    }
    s
}

/// Default (horizon, vanishing points) for 1/2/3-point perspective.
pub fn default_vps(points: usize, w: f64, h: f64) -> (f64, Vec<[f64; 2]>) {
    let hy = h * 0.42;
    match points {
        1 => (hy, vec![[w / 2.0, hy]]),
        2 => (hy, vec![[-w * 0.15, hy], [w * 1.15, hy]]),
        _ => (hy, vec![[-w * 0.15, hy], [w * 1.15, hy], [w / 2.0, h * 1.9]]),
    }
}

/// Face proportions inside the box (x, y, w, h).
pub fn face(x: f64, y: f64, w: f64, h: f64) -> Vec<Shape> {
    let cx = x + w / 2.0;
    let mut s = vec![Shape::Ellipse { c: [cx, y + h / 2.0], rx: w / 2.0, ry: h / 2.0 }, Shape::Line { a: [cx, y], b: [cx, y + h], w: 1.0, kind: "axis" }];
    for (f, label) in [(0.1, "Hairline"), (0.4, "Brows"), (0.5, "Eyes (half the head)"), (0.7, "Nose base"), (0.8, "Mouth"), (1.0, "Chin")] {
        let yy = y + h * f;
        s.push(Shape::Line { a: [x, yy], b: [x + w, yy], w: 1.0, kind: "measure" });
        s.push(Shape::Label { at: [x + w + 6.0, yy + 4.0], text: label.into() });
    }
    let ew = w / 5.0;
    for k in [1.0, 3.0] { s.push(Shape::Ellipse { c: [x + ew * (k + 0.5), y + h * 0.5], rx: ew * 0.45, ry: ew * 0.2 }); }
    s.push(Shape::Line { a: [x, y + h * 0.4], b: [x, y + h * 0.7], w: 2.0, kind: "feature" });
    s.push(Shape::Line { a: [x + w, y + h * 0.4], b: [x + w, y + h * 0.7], w: 2.0, kind: "feature" });
    s
}

/// Figure `heads` head-units tall in the box, with landmarks.
pub fn figure(x: f64, y: f64, w: f64, h: f64, heads: usize) -> Vec<Shape> {
    let (u, cx) = (h / heads as f64, x + w / 2.0);
    let marks8 = ["Chin", "Nipples", "Navel", "Crotch (half)", "Mid-thigh", "Knees", "Mid-shin", "Feet"];
    let mut s = vec![Shape::Line { a: [cx, y], b: [cx, y + h], w: 1.0, kind: "axis" }];
    for i in 1..=heads {
        let yy = y + u * i as f64;
        let m = if heads == 8 { marks8[i - 1].to_string() } else if i == heads { "Feet".into() } else if i == 1 { "Chin".into() } else { format!("{i} heads") };
        s.push(Shape::Line { a: [x, yy], b: [x + w, yy], w: 1.0, kind: "measure" });
        s.push(Shape::Label { at: [x + w + 6.0, yy + 4.0], text: format!("{i}  {m}") });
    }
    s.push(Shape::Ellipse { c: [cx, y + u / 2.0], rx: u * 0.38, ry: u / 2.0 });
    s.push(Shape::Line { a: [cx - u, y + u * 1.4], b: [cx + u, y + u * 1.4], w: 2.0, kind: "feature" });
    s.push(Shape::Line { a: [cx - u * 0.8, y + u * 4.0], b: [cx + u * 0.8, y + u * 4.0], w: 2.0, kind: "feature" });
    s
}

/// Composition grid: "thirds", "golden", "symmetry" or "diagonals".
pub fn composition(kind: &str, w: f64, h: f64) -> Vec<Shape> {
    let v = |x: f64| Shape::Line { a: [x, 0.0], b: [x, h], w: 1.0, kind: "grid" };
    let hz = |y: f64| Shape::Line { a: [0.0, y], b: [w, y], w: 1.0, kind: "grid" };
    match kind {
        "thirds" => vec![v(w / 3.0), v(2.0 * w / 3.0), hz(h / 3.0), hz(2.0 * h / 3.0)],
        "golden" => { let g = 1.0 / 1.618034; vec![v(w * (1.0 - g)), v(w * g), hz(h * (1.0 - g)), hz(h * g)] }
        "symmetry" => vec![Shape::Line { a: [w / 2.0, 0.0], b: [w / 2.0, h], w: 2.0, kind: "axis" }, Shape::Line { a: [0.0, h / 2.0], b: [w, h / 2.0], w: 1.0, kind: "axis" }],
        _ => vec![Shape::Line { a: [0.0, 0.0], b: [w, h], w: 1.0, kind: "grid" }, Shape::Line { a: [w, 0.0], b: [0.0, h], w: 1.0, kind: "grid" }, v(w / 2.0), hz(h / 2.0)],
    }
}

/// Line-weight chart: (mm, grade, use).
pub const LINE_WEIGHTS: &[(f64, &str, &str)] = &[
    (0.2, "2H, light hand", "Construction lines and first sketch (erase later)"),
    (0.35, "HB", "Outlines in light and far areas"),
    (0.5, "B / 2B", "Main outlines"),
    (0.8, "4B", "Edges in shadow, overlaps, near objects"),
    (1.2, "6B, side of the lead", "Darkest accents, where objects touch the ground"),
];
