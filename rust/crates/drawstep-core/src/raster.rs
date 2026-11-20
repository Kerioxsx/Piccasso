//! Software rasterization of guide overlays (for PNG exports and step sheets).
use crate::analysis::Analysis;
use crate::geometry::raster_polyline;
use crate::imgproc::{js_round, Rgba};

pub const C_SHAPES: [u8; 3] = [0xc2, 0x18, 0x5b];
pub const C_SIL: [u8; 3] = [0xd8, 0x43, 0x15];
pub const C_INT: [u8; 3] = [0x15, 0x65, 0xc0];
pub const C_FRAME: [u8; 3] = [0x6d, 0x6a, 0x63];
pub const C_SUBJECT: [u8; 3] = [0xef, 0x6c, 0x00];
pub const C_LANDMARK: [u8; 3] = [0x2e, 0x7d, 0x32];

fn paint(img: &mut Rgba, pts: &[[f64; 2]], closed: bool, r: i64, col: [u8; 3]) {
    let (w, h) = (img.width, img.height);
    let mut m = vec![0u8; w * h];
    raster_polyline(&mut m, w, h, pts, closed, r);
    for (i, &v) in m.iter().enumerate() {
        if v != 0 {
            img.data[i * 4..i * 4 + 3].copy_from_slice(&col);
            img.data[i * 4 + 3] = 255;
        }
    }
}

/// Points of a (rotated) ellipse in normalized coordinates (radii are fractions of width).
pub fn ellipse_points(cx: f64, cy: f64, rx: f64, ry: f64, angle_deg: f64, aspect: f64, n: usize) -> Vec<[f64; 2]> {
    let t = angle_deg.to_radians();
    (0..n).map(|k| {
        let a = k as f64 / n as f64 * std::f64::consts::TAU;
        let (ex, ey) = (a.cos() * rx, a.sin() * ry);
        [cx + ex * t.cos() - ey * t.sin(), cy + (ex * t.sin() + ey * t.cos()) * aspect]
    }).collect()
}

/// Draw the given layers onto `img` (which shows the whole image area).
pub fn draw_layers(img: &mut Rgba, a: &Analysis, layers: &[String]) {
    let (w, h) = (img.width as f64, img.height as f64);
    let r = (js_round(w / 500.0) as i64).max(1);
    let has = |l: &str| layers.iter().any(|x| x == l);
    let px = |p: &[f64; 2]| [p[0] * w, p[1] * h];
    let aspect = w / h;
    if has("frame") {
        paint(img, &[[0.0, 0.0], [w - 1.0, 0.0], [w - 1.0, h - 1.0], [0.0, h - 1.0]], true, r, C_FRAME);
    }
    if has("grid") {
        for f in [1.0 / 3.0, 2.0 / 3.0] {
            paint(img, &[[f * w, 0.0], [f * w, h]], false, 0, C_FRAME);
            paint(img, &[[0.0, f * h], [w, f * h]], false, 0, C_FRAME);
        }
    }
    let b = a.subject.bbox;
    let sb = [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]].map(|p| px(&p));
    if has("subjectBox") || has("measures") {
        paint(img, &sb, true, r, C_SUBJECT);
    }
    if has("measures") {
        let (mx, my) = ((sb[0][0] + sb[1][0]) / 2.0, (sb[0][1] + sb[2][1]) / 2.0);
        paint(img, &[[mx, sb[0][1]], [mx, sb[2][1]]], false, 0, C_SUBJECT);
        paint(img, &[[sb[0][0], my], [sb[1][0], my]], false, 0, C_SUBJECT);
    }
    if has("shapes") {
        for s in &a.shapes {
            let pts = if s.kind == "box" {
                let (x0, x1) = (s.cx - s.rx, s.cx + s.rx);
                let (y0, y1) = (s.cy - s.ry * aspect, s.cy + s.ry * aspect);
                vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
            } else {
                ellipse_points(s.cx, s.cy, s.rx, s.ry, s.angle, aspect, 64)
            };
            paint(img, &pts.iter().map(px).collect::<Vec<_>>(), true, r, C_SHAPES);
        }
    }
    if has("silhouette") {
        for c in &a.silhouettes {
            paint(img, &c.points.iter().map(px).collect::<Vec<_>>(), c.closed, r + 1, C_SIL);
        }
    }
    if has("internal") {
        for c in &a.internals {
            paint(img, &c.points.iter().map(px).collect::<Vec<_>>(), c.closed, r, C_INT);
        }
    }
    if has("landmarks") {
        for l in &a.landmarks {
            let c = px(&[l.x, l.y]);
            paint(img, &[c], false, r + 3, C_LANDMARK);
        }
    }
}
