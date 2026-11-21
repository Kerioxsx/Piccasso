//! Image-processing primitives. A faithful port of `web/js/core/image.js`;
//! see docs/ALGORITHMS.md. Rounding follows JavaScript's `Math.round`
//! (`floor(x + 0.5)`) so both engines produce identical bytes, verified by
//! `shared/test-vectors.json`.

/// RGBA8 image, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgba {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// 8-bit single-channel image.
#[derive(Clone, Debug, PartialEq)]
pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// 0/1 mask with the same layout as `Gray::data`.
pub type Mask = Vec<u8>;

/// Normalized crop rectangle (0..1).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Default for CropRect {
    fn default() -> Self {
        CropRect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }
    }
}

/// JavaScript `Math.round`.
#[inline]
pub fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

#[inline]
fn clamp255(v: f64) -> u8 {
    if v < 0.0 {
        0
    } else if v > 255.0 {
        255
    } else {
        v as u8
    }
}

impl Rgba {
    pub fn new(width: usize, height: usize) -> Self {
        Rgba { width, height, data: vec![0; width * height * 4] }
    }
    pub fn filled(width: usize, height: usize, v: u8) -> Self {
        Rgba { width, height, data: vec![v; width * height * 4] }
    }
}
impl Gray {
    pub fn new(width: usize, height: usize) -> Self {
        Gray { width, height, data: vec![0; width * height] }
    }
}

pub fn crop(img: &Rgba, rect: &CropRect) -> Rgba {
    let (w0, h0) = (img.width as i64, img.height as i64);
    let mut x0 = js_round(rect.x * w0 as f64) as i64;
    let mut y0 = js_round(rect.y * h0 as f64) as i64;
    x0 = x0.clamp(0, w0 - 1);
    y0 = y0.clamp(0, h0 - 1);
    let mut w = (js_round(rect.w * w0 as f64) as i64).max(1);
    let mut h = (js_round(rect.h * h0 as f64) as i64).max(1);
    w = w.min(w0 - x0);
    h = h.min(h0 - y0);
    if x0 == 0 && y0 == 0 && w == w0 && h == h0 {
        return img.clone();
    }
    let (w, h, x0, y0) = (w as usize, h as usize, x0 as usize, y0 as usize);
    let mut out = Rgba::new(w, h);
    for y in 0..h {
        let s = ((y0 + y) * img.width + x0) * 4;
        out.data[y * w * 4..(y + 1) * w * 4].copy_from_slice(&img.data[s..s + w * 4]);
    }
    out
}

/// Rotate clockwise by 0/90/180/270 degrees.
pub fn rotate(img: &Rgba, deg: i64) -> Rgba {
    let d = ((deg % 360) + 360) % 360;
    if d == 0 {
        return img.clone();
    }
    let (w, h) = (img.width, img.height);
    let mut out = if d == 180 { Rgba::new(w, h) } else { Rgba::new(h, w) };
    let ow = out.width;
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = match d {
                90 => (h - 1 - y, x),
                180 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let si = (y * w + x) * 4;
            let di = (ny * ow + nx) * 4;
            out.data[di..di + 4].copy_from_slice(&img.data[si..si + 4]);
        }
    }
    out
}

pub fn flip(img: &Rgba, horizontal: bool, vertical: bool) -> Rgba {
    if !horizontal && !vertical {
        return img.clone();
    }
    let (w, h) = (img.width, img.height);
    let mut out = Rgba::new(w, h);
    for y in 0..h {
        let sy = if vertical { h - 1 - y } else { y };
        for x in 0..w {
            let sx = if horizontal { w - 1 - x } else { x };
            let si = (sy * w + sx) * 4;
            let di = (y * w + x) * 4;
            out.data[di..di + 4].copy_from_slice(&img.data[si..si + 4]);
        }
    }
    out
}

/// Size that fits within `max_dim`, preserving aspect. Never upscales.
pub fn fit_dims(w: usize, h: usize, max_dim: usize) -> (usize, usize) {
    let m = w.max(h);
    if m <= max_dim {
        return (w, h);
    }
    let s = max_dim as f64 / m as f64;
    ((js_round(w as f64 * s) as usize).max(1), (js_round(h as f64 * s) as usize).max(1))
}

/// Area-average resize.
pub fn resize_box(img: &Rgba, dw: usize, dh: usize) -> Rgba {
    let (sw, sh) = (img.width, img.height);
    if dw == sw && dh == sh {
        return img.clone();
    }
    let mut out = Rgba::new(dw, dh);
    for y in 0..dh {
        let y0 = y * sh / dh;
        let y1 = (y0 + 1).max((y + 1) * sh / dh);
        for x in 0..dw {
            let x0 = x * sw / dw;
            let x1 = (x0 + 1).max((x + 1) * sw / dw);
            let mut acc = [0u64; 4];
            for yy in y0..y1 {
                let mut i = (yy * sw + x0) * 4;
                for _ in x0..x1 {
                    for c in 0..4 {
                        acc[c] += img.data[i + c] as u64;
                    }
                    i += 4;
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u64;
            let half = n >> 1;
            let o = (y * dw + x) * 4;
            for c in 0..4 {
                out.data[o + c] = ((acc[c] + half) / n) as u8;
            }
        }
    }
    out
}

/// Brightness/contrast in -100..100; transparent pixels are composited on white.
pub fn adjust(img: &Rgba, brightness: f64, contrast: f64) -> Rgba {
    let mut out = Rgba::new(img.width, img.height);
    let cf = (100.0 + contrast) / 100.0;
    let bo = brightness * 2.55;
    for i in (0..img.data.len()).step_by(4) {
        let a = img.data[i + 3] as u32;
        for c in 0..3 {
            let v = (img.data[i + c] as u32 * a + 255 * (255 - a) + 127) / 255;
            out.data[i + c] = clamp255(js_round((v as f64 - 128.0) * cf + 128.0 + bo));
        }
        out.data[i + 3] = 255;
    }
    out
}

/// Integer luma `(77R + 150G + 29B + 128) >> 8`.
pub fn to_gray(img: &Rgba) -> Gray {
    let mut out = Gray::new(img.width, img.height);
    for (j, px) in img.data.chunks_exact(4).enumerate() {
        out.data[j] = ((77 * px[0] as u32 + 150 * px[1] as u32 + 29 * px[2] as u32 + 128) >> 8) as u8;
    }
    out
}

pub fn gray_to_rgba(g: &Gray) -> Rgba {
    let mut out = Rgba::new(g.width, g.height);
    for (j, &v) in g.data.iter().enumerate() {
        out.data[j * 4..j * 4 + 4].copy_from_slice(&[v, v, v, 255]);
    }
    out
}

/// One separable box-blur pass (edge clamped).
pub fn box_blur(g: &Gray, r: usize) -> Gray {
    if r == 0 {
        return g.clone();
    }
    let (w, h) = (g.width, g.height);
    let ri = r as i64;
    let n = (2 * r + 1) as u32;
    let half = n >> 1;
    let mut tmp = vec![0u8; w * h];
    let mut out = Gray::new(w, h);
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let mut s = 0u32;
            for k in -ri..=ri {
                let xx = (x as i64 + k).clamp(0, w as i64 - 1) as usize;
                s += g.data[row + xx] as u32;
            }
            tmp[row + x] = ((s + half) / n) as u8;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut s = 0u32;
            for k in -ri..=ri {
                let yy = (y as i64 + k).clamp(0, h as i64 - 1) as usize;
                s += tmp[yy * w + x] as u32;
            }
            out.data[y * w + x] = ((s + half) / n) as u8;
        }
    }
    out
}

pub fn blur2(g: &Gray, r: usize) -> Gray {
    box_blur(&box_blur(g, r), r)
}

/// Sobel magnitude (L1), scaled by 1/4, clamped to 255.
pub fn sobel(g: &Gray) -> Gray {
    let (w, h) = (g.width as i64, g.height as i64);
    let px = |x: i64, y: i64| -> i32 { g.data[(y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize] as i32 };
    let mut out = Gray::new(g.width, g.height);
    for y in 0..h {
        for x in 0..w {
            let (a, b, c) = (px(x - 1, y - 1), px(x, y - 1), px(x + 1, y - 1));
            let (d, f) = (px(x - 1, y), px(x + 1, y));
            let (gg, hh, i) = (px(x - 1, y + 1), px(x, y + 1), px(x + 1, y + 1));
            let gx = c + 2 * f + i - a - 2 * d - gg;
            let gy = gg + 2 * hh + i - a - 2 * b - c;
            let m = (gx.abs() + gy.abs()) >> 2;
            out.data[(y * w + x) as usize] = m.min(255) as u8;
        }
    }
    out
}

pub fn histogram(g: &Gray) -> [u64; 256] {
    let mut h = [0u64; 256];
    for &v in &g.data {
        h[v as usize] += 1;
    }
    h
}

/// Smallest threshold t (>= min_t) such that at most `frac` of pixels are >= t.
pub fn percentile_threshold(g: &Gray, frac: f64, min_t: i32) -> i32 {
    let h = histogram(g);
    let target = frac * g.data.len() as f64;
    let mut count = 0u64;
    for t in (0..256).rev() {
        if (count + h[t]) as f64 > target {
            return (t as i32 + 1).max(min_t);
        }
        count += h[t];
    }
    min_t
}

/// Otsu threshold; pixels > t are the bright class.
pub fn otsu(g: &Gray) -> i32 {
    let h = histogram(g);
    let total = g.data.len() as f64;
    let mut sum_all = 0f64;
    for (i, &c) in h.iter().enumerate() {
        sum_all += i as f64 * c as f64;
    }
    let (mut wb, mut sum_b, mut best, mut best_t) = (0f64, 0f64, -1f64, 127i32);
    for t in 0..256usize {
        wb += h[t] as f64;
        if wb == 0.0 {
            continue;
        }
        let wf = total - wb;
        if wf == 0.0 {
            break;
        }
        sum_b += t as f64 * h[t] as f64;
        let mb = sum_b / wb;
        let mf = (sum_all - sum_b) / wf;
        let between = wb * wf * (mb - mf) * (mb - mf);
        if between > best {
            best = between;
            best_t = t as i32;
        }
    }
    best_t
}

/// Square dilation of a 0/1 mask.
pub fn dilate(mask: &[u8], w: usize, h: usize, r: usize) -> Mask {
    if r == 0 {
        return mask.to_vec();
    }
    let mut tmp = vec![0u8; w * h];
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            tmp[y * w + x] = (lo..=hi).any(|k| mask[y * w + k] != 0) as u8;
        }
    }
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            out[y * w + x] = (lo..=hi).any(|k| tmp[k * w + x] != 0) as u8;
        }
    }
    out
}

pub fn erode(mask: &[u8], w: usize, h: usize, r: usize) -> Mask {
    if r == 0 {
        return mask.to_vec();
    }
    let inv: Vec<u8> = mask.iter().map(|&v| (v == 0) as u8).collect();
    dilate(&inv, w, h, r).iter().map(|&v| (v == 0) as u8).collect()
}

/// Connected-component statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub label: i32,
    pub area: u64,
    pub min_x: usize,
    pub min_y: usize,
    pub max_x: usize,
    pub max_y: usize,
    pub sum_x: f64,
    pub sum_y: f64,
    pub sum_xx: f64,
    pub sum_yy: f64,
    pub sum_xy: f64,
    pub start_x: usize,
    pub start_y: usize,
    pub borders: u32,
}

/// 4-connected labelling (raster-order flood fill; deterministic).
pub fn label_components(mask: &[u8], w: usize, h: usize) -> (Vec<i32>, Vec<Component>) {
    let mut labels = vec![0i32; w * h];
    let mut comps = Vec::new();
    let mut stack: Vec<usize> = Vec::with_capacity(1024);
    let mut next = 1i32;
    for start in 0..w * h {
        if mask[start] == 0 || labels[start] != 0 {
            continue;
        }
        let mut c = Component {
            label: next, area: 0, min_x: w, min_y: h, max_x: 0, max_y: 0,
            sum_x: 0.0, sum_y: 0.0, sum_xx: 0.0, sum_yy: 0.0, sum_xy: 0.0,
            start_x: start % w, start_y: start / w, borders: 0,
        };
        stack.clear();
        stack.push(start);
        labels[start] = next;
        while let Some(p) = stack.pop() {
            let (x, y) = (p % w, p / w);
            c.area += 1;
            // max_* start at 0, which is correct because coordinates are never negative
            if x < c.min_x { c.min_x = x; }
            if x > c.max_x { c.max_x = x; }
            if y < c.min_y { c.min_y = y; }
            if y > c.max_y { c.max_y = y; }
            let (xf, yf) = (x as f64, y as f64);
            c.sum_x += xf; c.sum_y += yf; c.sum_xx += xf * xf; c.sum_yy += yf * yf; c.sum_xy += xf * yf;
            if x > 0 && mask[p - 1] != 0 && labels[p - 1] == 0 { labels[p - 1] = next; stack.push(p - 1); }
            if x < w - 1 && mask[p + 1] != 0 && labels[p + 1] == 0 { labels[p + 1] = next; stack.push(p + 1); }
            if y > 0 && mask[p - w] != 0 && labels[p - w] == 0 { labels[p - w] = next; stack.push(p - w); }
            if y < h - 1 && mask[p + w] != 0 && labels[p + w] == 0 { labels[p + w] = next; stack.push(p + w); }
        }
        c.borders = (c.min_x == 0) as u32 + (c.min_y == 0) as u32 + (c.max_x == w - 1) as u32 + (c.max_y == h - 1) as u32;
        comps.push(c);
        next += 1;
    }
    (labels, comps)
}

const DX: [i64; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DY: [i64; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

/// Moore-neighbour boundary trace (clockwise), from the component's first raster pixel.
pub fn trace_boundary(labels: &[i32], w: usize, h: usize, label: i32, sx: usize, sy: usize) -> Vec<[i64; 2]> {
    let mut pts = vec![[sx as i64, sy as i64]];
    let (mut cx, mut cy, mut dir) = (sx as i64, sy as i64, 0usize);
    let limit = 4 * w * h;
    for _ in 0..limit {
        let mut found = false;
        let d0 = (dir + 6) % 8;
        for k in 0..8 {
            let nd = (d0 + k) % 8;
            let (nx, ny) = (cx + DX[nd], cy + DY[nd]);
            if nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && labels[ny as usize * w + nx as usize] == label {
                cx = nx; cy = ny; dir = nd; found = true;
                break;
            }
        }
        if !found || (cx == sx as i64 && cy == sy as i64) {
            break;
        }
        pts.push([cx, cy]);
    }
    pts
}

fn perp_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    (dy * p[0] - dx * p[1] + b[0] * a[1] - b[1] * a[0]).abs() / len2.sqrt()
}

/// Ramer–Douglas–Peucker (iterative, same order as the JS version).
pub fn douglas_peucker(points: &[[f64; 2]], eps: f64, closed: bool) -> Vec<[f64; 2]> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut pts = points.to_vec();
    if closed {
        pts.push(points[0]);
    }
    let f = |p: [f64; 2]| p;
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    let last = pts.len() - 1;
    keep[last] = true;
    let mut stack = vec![(0usize, last)];
    while let Some((s, e)) = stack.pop() {
        let (mut max_d, mut idx) = (-1.0f64, usize::MAX);
        for i in s + 1..e {
            let d = perp_dist(f(pts[i]), f(pts[s]), f(pts[e]));
            if d > max_d {
                max_d = d;
                idx = i;
            }
        }
        if idx != usize::MAX && max_d > eps {
            keep[idx] = true;
            stack.push((s, idx));
            stack.push((idx, e));
        }
    }
    let mut out: Vec<[f64; 2]> = pts.iter().zip(&keep).filter(|(_, &k)| k).map(|(p, _)| *p).collect();
    if closed {
        out.pop();
    }
    out
}

pub fn posterize_levels(g: &Gray, levels: u32) -> Vec<u8> {
    g.data.iter().map(|&v| ((v as u32 * levels) / 256) as u8).collect()
}

fn blurred_channels(img: &Rgba, r: usize) -> [Gray; 3] {
    let mk = |c: usize| {
        let mut g = Gray::new(img.width, img.height);
        for i in 0..img.width * img.height {
            g.data[i] = img.data[i * 4 + c];
        }
        blur2(&g, r)
    };
    [mk(0), mk(1), mk(2)]
}

/// Result of border-colour foreground detection.
pub struct Foreground {
    pub mask: Mask,
    pub threshold: i32,
    pub separation: f64,
    pub dist: Gray,
}

/// Foreground = pixels far (in colour) from every dominant border colour.
pub fn foreground_from_border(img: &Rgba, r: usize) -> Foreground {
    let (w, h) = (img.width, img.height);
    let n = w * h;
    let [rc, gc, bc] = blurred_channels(img, r);
    let mut border: Vec<[f64; 3]> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if x > 1 && y > 1 && x + 2 < w && y + 2 < h {
                continue;
            }
            let i = y * w + x;
            border.push([rc.data[i] as f64, gc.data[i] as f64, bc.data[i] as f64]);
        }
    }
    let lum = |p: &[f64; 3]| 77.0 * p[0] + 150.0 * p[1] + 29.0 * p[2];
    let mut sorted = border.clone();
    sorted.sort_by(|a, b| lum(a).partial_cmp(&lum(b)).unwrap());
    let mut centers: Vec<[f64; 3]> = [0.1, 0.5, 0.9]
        .iter()
        .map(|q| sorted[((q * sorted.len() as f64).floor() as usize).min(sorted.len() - 1)])
        .collect();
    for _ in 0..8 {
        let mut acc = vec![[0f64; 4]; centers.len()];
        for p in &border {
            let (mut best, mut bd) = (0usize, f64::INFINITY);
            for (k, c) in centers.iter().enumerate() {
                let d = (p[0] - c[0]).abs() + (p[1] - c[1]).abs() + (p[2] - c[2]).abs();
                if d < bd {
                    bd = d;
                    best = k;
                }
            }
            let a = &mut acc[best];
            a[0] += p[0]; a[1] += p[1]; a[2] += p[2]; a[3] += 1.0;
        }
        for (k, c) in centers.iter_mut().enumerate() {
            if acc[k][3] != 0.0 {
                *c = [acc[k][0] / acc[k][3], acc[k][1] / acc[k][3], acc[k][2] / acc[k][3]];
            }
        }
    }
    let mut dist = Gray::new(w, h);
    for i in 0..n {
        let mut bd = f64::INFINITY;
        for c in &centers {
            let d = ((rc.data[i] as f64 - c[0]).abs() + (gc.data[i] as f64 - c[1]).abs() + (bc.data[i] as f64 - c[2]).abs()) / 3.0;
            if d < bd {
                bd = d;
            }
        }
        dist.data[i] = if bd > 255.0 { 255 } else { js_round(bd) as u8 };
    }
    let threshold = otsu(&dist).max(20);
    let mut mask = vec![0u8; n];
    let (mut s_lo, mut n_lo, mut s_hi, mut n_hi) = (0f64, 0f64, 0f64, 0f64);
    for i in 0..n {
        let v = dist.data[i] as f64;
        if dist.data[i] as i32 > threshold {
            mask[i] = 1;
            s_hi += v;
            n_hi += 1.0;
        } else {
            s_lo += v;
            n_lo += 1.0;
        }
    }
    let separation = if n_hi != 0.0 && n_lo != 0.0 { s_hi / n_hi - s_lo / n_lo } else { 0.0 };
    Foreground { mask, threshold, separation, dist }
}

/// Colour-aware edges: max of per-channel Sobel on blurred channels.
pub fn color_sobel(img: &Rgba, r: usize) -> Gray {
    let [a, b, c] = blurred_channels(img, r);
    let (sa, sb, sc) = (sobel(&a), sobel(&b), sobel(&c));
    let mut out = Gray::new(img.width, img.height);
    for i in 0..out.data.len() {
        out.data[i] = sa.data[i].max(sb.data[i]).max(sc.data[i]);
    }
    out
}

/// close(1) then open(2).
pub fn clean_mask(mask: &[u8], w: usize, h: usize) -> Mask {
    let closed = erode(&dilate(mask, w, h, 1), w, h, 1);
    dilate(&erode(&closed, w, h, 2), w, h, 2)
}

/// Preparation settings that affect the image itself.
#[derive(Clone, Debug)]
pub struct PrepImage {
    pub crop: CropRect,
    pub rotation: i64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub brightness: f64,
    pub contrast: f64,
}

/// rotate → flip → crop → resize → brightness/contrast.
pub fn prepare_image(src: &Rgba, p: &PrepImage, max_dim: usize) -> Rgba {
    let img = rotate(src, p.rotation);
    let img = flip(&img, p.flip_h, p.flip_v);
    let img = crop(&img, &p.crop);
    let (w, h) = fit_dims(img.width, img.height, max_dim);
    let img = resize_box(&img, w, h);
    adjust(&img, p.brightness, p.contrast)
}
