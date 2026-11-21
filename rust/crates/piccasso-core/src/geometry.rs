//! Perspective geometry — port of `web/js/core/geometry.js`.
use crate::imgproc::*;

/// Homography (row-major, h[8] = 1) mapping src[i] → dst[i]; None if degenerate.
pub fn homography(src: &[[f64; 2]; 4], dst: &[[f64; 2]; 4]) -> Option<[f64; 9]> {
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 {
        let ([x, y], [u, v]) = (src[i], dst[i]);
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for c in 0..8 {
        let mut p = c;
        for r in c + 1..8 {
            if a[r][c].abs() > a[p][c].abs() {
                p = r;
            }
        }
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        for r in 0..8 {
            if r == c {
                continue;
            }
            let f = a[r][c] / a[c][c];
            for k in c..9 {
                a[r][k] -= f * a[c][k];
            }
        }
    }
    let mut h = [0f64; 9];
    for i in 0..8 {
        h[i] = a[i][8] / a[i][i];
    }
    h[8] = 1.0;
    Some(h)
}

pub fn apply_h(h: &[f64; 9], x: f64, y: f64) -> [f64; 2] {
    let w = h[6] * x + h[7] * y + h[8];
    [(h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w]
}

pub fn transformed_rect(cx: f64, cy: f64, w: f64, h: f64, scale: f64, rot_deg: f64) -> [[f64; 2]; 4] {
    let a = rot_deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    let pts = [[-w / 2.0, -h / 2.0], [w / 2.0, -h / 2.0], [w / 2.0, h / 2.0], [-w / 2.0, h / 2.0]];
    pts.map(|[x, y]| [cx + (x * c - y * s) * scale, cy + (x * s + y * c) * scale])
}

pub fn order_corners(pts: &[[f64; 2]; 4]) -> [[f64; 2]; 4] {
    let pick = |f: &dyn Fn(&[f64; 2]) -> f64, max: bool| {
        let mut best = pts[0];
        for p in pts.iter().skip(1) {
            if (max && f(p) > f(&best)) || (!max && f(p) < f(&best)) {
                best = *p;
            }
        }
        best
    };
    let sum = |p: &[f64; 2]| p[0] + p[1];
    let diff = |p: &[f64; 2]| p[0] - p[1];
    [pick(&sum, false), pick(&diff, true), pick(&sum, true), pick(&diff, false)]
}

pub fn polygon_area(q: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;
    for i in 0..q.len() {
        let ([x1, y1], [x2, y2]) = (q[i], q[(i + 1) % q.len()]);
        a += x1 * y2 - x2 * y1;
    }
    a.abs() / 2.0
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaperDetection {
    pub corners: [[f64; 2]; 4],
    pub confidence: f64,
}

/// Largest bright region approximated by its four extreme corners.
pub fn detect_paper(gray: &Gray) -> Option<PaperDetection> {
    let (w, h) = (gray.width, gray.height);
    let n = w * h;
    let b = blur2(gray, 2);
    let t = otsu(&b);
    let m: Mask = b.data.iter().map(|&v| (v as i32 > t) as u8).collect();
    let (labels, comps) = label_components(&m, w, h);
    let mut best = comps.first()?;
    for c in &comps {
        if c.area > best.area {
            best = c;
        }
    }
    if (best.area as f64) < 0.12 * n as f64 {
        return None;
    }
    let (mut tl, mut tr, mut br, mut bl): (Option<[f64; 2]>, Option<[f64; 2]>, Option<[f64; 2]>, Option<[f64; 2]>) = (None, None, None, None);
    for y in best.min_y..=best.max_y {
        for x in best.min_x..=best.max_x {
            if labels[y * w + x] != best.label {
                continue;
            }
            let p = [x as f64, y as f64];
            if tl.map_or(true, |q| p[0] + p[1] < q[0] + q[1]) { tl = Some(p); }
            if br.map_or(true, |q| p[0] + p[1] > q[0] + q[1]) { br = Some(p); }
            if tr.map_or(true, |q| p[0] - p[1] > q[0] - q[1]) { tr = Some(p); }
            if bl.map_or(true, |q| p[0] - p[1] < q[0] - q[1]) { bl = Some(p); }
        }
    }
    let quad = [tl?, tr?, br?, bl?];
    let qa = polygon_area(&quad);
    if qa < 1.0 {
        return None;
    }
    let fill = (best.area as f64 / qa).min(1.0);
    let confidence = js_round(((best.area as f64 / n as f64) * 2.0).min(1.0) * fill * fill * 100.0) / 100.0;
    Some(PaperDetection { corners: quad.map(|[x, y]| [(x + 0.5) / w as f64, (y + 0.5) / h as f64]), confidence })
}

/// Warp a quad (normalized TL,TR,BR,BL) into an out_w × out_h rectangle (bilinear).
pub fn warp_gray(gray: &Gray, corners: &[[f64; 2]; 4], out_w: usize, out_h: usize) -> Gray {
    let (w, h) = (gray.width, gray.height);
    let quad = corners.map(|[x, y]| [x * w as f64 - 0.5, y * h as f64 - 0.5]);
    let (ow, oh) = ((out_w - 1) as f64, (out_h - 1) as f64);
    let mut out = Gray::new(out_w, out_h);
    let Some(hm) = homography(&[[0.0, 0.0], [ow, 0.0], [ow, oh], [0.0, oh]], &quad) else { return out };
    for v in 0..out_h {
        for u in 0..out_w {
            let [mut x, mut y] = apply_h(&hm, u as f64, v as f64);
            x = x.clamp(0.0, (w - 1) as f64);
            y = y.clamp(0.0, (h - 1) as f64);
            let (x0, y0) = (x.floor() as usize, y.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
            let (fx, fy) = (x - x0 as f64, y - y0 as f64);
            let d = &gray.data;
            let top = d[y0 * w + x0] as f64 * (1.0 - fx) + d[y0 * w + x1] as f64 * fx;
            let bot = d[y1 * w + x0] as f64 * (1.0 - fx) + d[y1 * w + x1] as f64 * fx;
            out.data[v * out_w + u] = js_round(top * (1.0 - fy) + bot * fy) as u8;
        }
    }
    out
}

/// Rasterize a polyline (pixel coords) into a mask with a square brush of radius r.
pub fn raster_polyline(mask: &mut [u8], w: usize, h: usize, pts: &[[f64; 2]], closed: bool, r: i64) {
    let mut stamp = |x: i64, y: i64| {
        for yy in y - r..=y + r {
            for xx in x - r..=x + r {
                if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < h {
                    mask[yy as usize * w + xx as usize] = 1;
                }
            }
        }
    };
    let n = pts.len();
    if n == 0 {
        return;
    }
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (mut x0, mut y0) = (js_round(pts[i][0]) as i64, js_round(pts[i][1]) as i64);
        let (x1, y1) = (js_round(pts[(i + 1) % n][0]) as i64, js_round(pts[(i + 1) % n][1]) as i64);
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut err = dx + dy;
        loop {
            stamp(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x0 += sx; }
            if e2 <= dx { err += dx; y0 += sy; }
        }
    }
    if n == 1 {
        stamp(js_round(pts[0][0]) as i64, js_round(pts[0][1]) as i64);
    }
}
