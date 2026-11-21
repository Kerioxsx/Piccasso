//! Studio conversions (port of `web/js/core/filters.js`): classic image
//! processing that turns a picture into a drawing reference. No AI.
use crate::imgproc::{adjust, box_blur, dilate, flip, foreground_from_border, gray_to_rgba, sobel, to_gray, Gray, Rgba};

fn map_gray(img: &Rgba, f: impl Fn(u8) -> u8) -> Rgba {
    let mut g = to_gray(img);
    for v in g.data.iter_mut() { *v = f(*v); }
    gray_to_rgba(&g)
}

pub fn grayscale(img: &Rgba) -> Rgba { gray_to_rgba(&to_gray(img)) }

pub fn invert(img: &Rgba) -> Rgba {
    let mut o = img.clone();
    for px in o.data.chunks_exact_mut(4) { for c in &mut px[..3] { *c = 255 - *c; } }
    o
}

pub fn threshold(img: &Rgba, t: u8) -> Rgba { map_gray(img, |v| if v < t { 0 } else { 255 }) }

/// Colour posterization to `levels` steps per channel.
pub fn posterize(img: &Rgba, levels: u32) -> Rgba {
    let k = 255.0 / (levels.max(2) - 1) as f64;
    let mut o = img.clone();
    for px in o.data.chunks_exact_mut(4) {
        for c in &mut px[..3] { *c = ((*c as f64 / k).round() * k).round().clamp(0.0, 255.0) as u8; }
        px[3] = 255;
    }
    o
}

/// Light / mid / dark value map.
pub fn three_levels(img: &Rgba, dark_at: u8, light_at: u8) -> Rgba { map_gray(img, |v| if v < dark_at { 40 } else if v < light_at { 140 } else { 255 }) }

/// N-step value scale (colour to value).
pub fn value_scale(img: &Rgba, steps: u32) -> Rgba {
    let s = steps.max(2) as f64 - 1.0;
    map_gray(img, |v| (((v as f64 / 255.0) * s).round() * (255.0 / s)).round() as u8)
}

/// Pencil sketch: colour-dodge of the gray picture with its blurred negative.
pub fn pencil_sketch(img: &Rgba, radius: usize, darkness: f64) -> Rgba {
    let mut g = to_gray(img);
    let inv = Gray { width: g.width, height: g.height, data: g.data.iter().map(|v| 255 - v).collect() };
    let b = box_blur(&inv, radius.max(1));
    for (i, v) in g.data.iter_mut().enumerate() {
        let (base, blend) = (*v as f64, b.data[i] as f64);
        let mut x = if blend >= 255.0 { 255.0 } else { (base * 255.0 / (255.0 - blend)).min(255.0) };
        x = 255.0 - (255.0 - x) * darkness;
        *v = x.clamp(0.0, 255.0) as u8;
    }
    gray_to_rgba(&g)
}

/// Edge detection: Sobel magnitude as dark lines on white; `detail` 0..1 lowers the cut-off.
pub fn edges(img: &Rgba, detail: f64, thickness: usize) -> Rgba {
    let g = to_gray(img);
    let m = sobel(&box_blur(&g, 1));
    let max = m.data.iter().copied().max().unwrap_or(1).max(1) as f64;
    let t = max * (0.45 - 0.4 * detail);
    let mask: Vec<u8> = m.data.iter().map(|&v| (v as f64 > t) as u8).collect();
    let thick = if thickness > 1 { dilate(&mask, g.width, g.height, thickness - 1) } else { mask };
    gray_to_rgba(&Gray { width: g.width, height: g.height, data: thick.iter().map(|&b| if b != 0 { 0 } else { 255 }).collect() })
}

/// Contour lines between `levels` value bands of the smoothed picture.
pub fn contours(img: &Rgba, levels: u32, thickness: usize) -> Rgba {
    let g = box_blur(&to_gray(img), 2);
    let (w, h) = (g.width, g.height);
    let band: Vec<u32> = g.data.iter().map(|&v| ((v as u32 * levels) >> 8).min(levels - 1)).collect();
    let mut mask = vec![0u8; w * h];
    for y in 0..h.saturating_sub(1) { for x in 0..w.saturating_sub(1) { let i = y * w + x; if band[i] != band[i + 1] || band[i] != band[i + w] { mask[i] = 1; } } }
    if thickness > 1 { mask = dilate(&mask, w, h, thickness - 1); }
    gray_to_rgba(&Gray { width: w, height: h, data: mask.iter().map(|&b| if b != 0 { 30 } else { 255 }).collect() })
}

/// Crosshatching: darker values get more hatch directions.
pub fn crosshatch(img: &Rgba, spacing: usize, thickness: usize) -> Rgba {
    let g = box_blur(&to_gray(img), 1);
    let (w, h, s, t) = (g.width, g.height, spacing.max(3) as i64, thickness.max(1) as i64);
    let mut out = Gray::new(w, h);
    for y in 0..h { for x in 0..w {
        let v = g.data[y * w + x];
        let (xi, yi) = (x as i64, y as i64);
        let on = (v < 200 && (xi + yi) % s < t) || (v < 150 && (xi - yi).rem_euclid(s) < t) || (v < 100 && yi % s < t) || (v < 55 && xi % s < t);
        out.data[y * w + x] = if on { 20 } else { 255 };
    } }
    gray_to_rgba(&out)
}

/// Halftone: one dot per cell, sized by darkness.
pub fn halftone(img: &Rgba, cell: usize) -> Rgba {
    let g = to_gray(img);
    let (w, h, c) = (g.width, g.height, cell.max(3));
    let mut out = Gray { width: w, height: h, data: vec![255; w * h] };
    for cy in (0..h).step_by(c) { for cx in (0..w).step_by(c) {
        let (x1, y1) = ((cx + c).min(w), (cy + c).min(h));
        let mut sum = 0u64; let mut n = 0u64;
        for y in cy..y1 { for x in cx..x1 { sum += g.data[y * w + x] as u64; n += 1; } }
        let r = (c as f64 / 2.0) * (1.0 - sum as f64 / n as f64 / 255.0).sqrt() * 1.15;
        let (mx, my) = (cx as f64 + c as f64 / 2.0, cy as f64 + c as f64 / 2.0);
        for y in cy..y1 { for x in cx..x1 { if (x as f64 + 0.5 - mx).powi(2) + (y as f64 + 0.5 - my).powi(2) <= r * r { out.data[y * w + x] = 0; } } }
    } }
    gray_to_rgba(&out)
}

pub fn subject_mask(img: &Rgba) -> Vec<u8> { foreground_from_border(img, 2).mask }

pub fn silhouette(img: &Rgba) -> Rgba {
    let m = subject_mask(img);
    gray_to_rgba(&Gray { width: img.width, height: img.height, data: m.iter().map(|&b| if b != 0 { 0 } else { 255 }).collect() })
}

/// Empty space around the subject shaded, the subject left white.
pub fn negative_space(img: &Rgba) -> Rgba {
    let m = subject_mask(img);
    let edge = dilate(&m, img.width, img.height, 1);
    let mut o = Rgba::new(img.width, img.height);
    for i in 0..m.len() {
        let v = if m[i] != 0 { 255 } else if edge[i] != 0 { 20 } else { 150 };
        let b = if m[i] != 0 { 255 } else { (v + 40).min(255) };
        o.data[i * 4..i * 4 + 4].copy_from_slice(&[v as u8, v as u8, b as u8, 255]);
    }
    o
}

/// Background removal: outside the subject becomes white.
pub fn remove_background(img: &Rgba) -> Rgba {
    let m = subject_mask(img);
    let mut o = img.clone();
    for (i, &b) in m.iter().enumerate() { if b == 0 { o.data[i * 4..i * 4 + 4].copy_from_slice(&[255, 255, 255, 255]); } }
    o
}

/// Anime-style simplification: smoothed, flattened colours with bold dark outlines.
pub fn anime_simplify(img: &Rgba, levels: u32, line_detail: f64) -> Rgba {
    let (w, h) = (img.width, img.height);
    let mut soft = Rgba::new(w, h);
    for c in 0..3 {
        let mut g = Gray::new(w, h);
        for i in 0..w * h { g.data[i] = img.data[i * 4 + c]; }
        let b = box_blur(&g, 2);
        for i in 0..w * h { soft.data[i * 4 + c] = b.data[i]; soft.data[i * 4 + 3] = 255; }
    }
    let mut flat = posterize(&soft, levels);
    let lines = edges(img, line_detail, 2);
    for i in 0..w * h { if lines.data[i * 4] == 0 { flat.data[i * 4..i * 4 + 3].copy_from_slice(&[25, 25, 25]); } }
    flat
}

/// Bicubic upscale by `factor` (conventional interpolation).
pub fn upscale(img: &Rgba, factor: f64) -> Rgba {
    let (sw, sh) = (img.width as i64, img.height as i64);
    let (w, h) = ((sw as f64 * factor).round() as usize, (sh as f64 * factor).round() as usize);
    let cub = |t: f64| { let a = -0.5; let x = t.abs(); if x <= 1.0 { (a + 2.0) * x.powi(3) - (a + 3.0) * x * x + 1.0 } else if x < 2.0 { a * x.powi(3) - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a } else { 0.0 } };
    let mut o = Rgba::new(w, h);
    for y in 0..h {
        let sy = (y as f64 + 0.5) / factor - 0.5; let y0 = sy.floor() as i64;
        for x in 0..w {
            let sx = (x as f64 + 0.5) / factor - 0.5; let x0 = sx.floor() as i64;
            let (mut acc, mut ws) = ([0f64; 4], 0f64);
            for m in -1..=2 { let yy = (y0 + m).clamp(0, sh - 1); let wy = cub(sy - (y0 + m) as f64);
                for n in -1..=2 { let xx = (x0 + n).clamp(0, sw - 1); let wgt = wy * cub(sx - (x0 + n) as f64); let j = ((yy * sw + xx) * 4) as usize;
                    for c in 0..4 { acc[c] += img.data[j + c] as f64 * wgt; } ws += wgt; } }
            let i = (y * w + x) * 4;
            for c in 0..4 { o.data[i + c] = (acc[c] / ws).round().clamp(0.0, 255.0) as u8; }
        }
    }
    o
}

/// Run a conversion only inside the pixel rectangle (x, y, w, h).
pub fn in_region(img: &Rgba, rect: Option<(usize, usize, usize, usize)>, f: impl Fn(&Rgba) -> Rgba) -> Rgba {
    let out = f(img);
    let Some((rx, ry, rw, rh)) = rect else { return out };
    let mut o = img.clone();
    for y in ry.min(img.height)..(ry + rh).min(img.height) {
        let (s, e) = ((y * img.width + rx.min(img.width)) * 4, (y * img.width + (rx + rw).min(img.width)) * 4);
        o.data[s..e].copy_from_slice(&out.data[s..e]);
    }
    o
}

/// Brightness/contrast (-100..100) and mirror before a conversion.
pub fn pre_adjust(img: &Rgba, brightness: f64, contrast: f64, mirror: bool) -> Rgba {
    let o = if brightness != 0.0 || contrast != 0.0 { adjust(img, brightness, contrast) } else { img.clone() };
    if mirror { flip(&o, true, false) } else { o }
}

/// The Studio's conversions: key, label, parameters (name, min, max, default).
pub struct Conversion { pub key: &'static str, pub label: &'static str, pub params: &'static [(&'static str, f64, f64, f64)] }

pub const CONVERSIONS: &[Conversion] = &[
    Conversion { key: "original", label: "Original", params: &[] },
    Conversion { key: "grayscale", label: "Grayscale", params: &[] },
    Conversion { key: "sketch", label: "Pencil sketch", params: &[("Radius", 2.0, 20.0, 6.0), ("Darkness", 0.3, 2.0, 1.0)] },
    Conversion { key: "edges", label: "Edge detection", params: &[("Detail", 0.0, 1.0, 0.5), ("Thickness", 1.0, 6.0, 1.0)] },
    Conversion { key: "contours", label: "Contours", params: &[("Levels", 2.0, 12.0, 5.0), ("Thickness", 1.0, 6.0, 1.0)] },
    Conversion { key: "three", label: "Three values", params: &[("Dark below", 20.0, 140.0, 85.0), ("Light above", 120.0, 240.0, 170.0)] },
    Conversion { key: "values", label: "Colour to value", params: &[("Steps", 3.0, 11.0, 9.0)] },
    Conversion { key: "posterize", label: "Posterize", params: &[("Levels", 2.0, 8.0, 4.0)] },
    Conversion { key: "threshold", label: "Threshold", params: &[("Level", 10.0, 245.0, 128.0)] },
    Conversion { key: "hatch", label: "Crosshatching", params: &[("Spacing", 3.0, 16.0, 6.0), ("Thickness", 1.0, 4.0, 1.0)] },
    Conversion { key: "halftone", label: "Halftone", params: &[("Cell", 4.0, 24.0, 8.0)] },
    Conversion { key: "silhouette", label: "Silhouette", params: &[] },
    Conversion { key: "negative", label: "Negative space", params: &[] },
    Conversion { key: "nobg", label: "Remove background", params: &[] },
    Conversion { key: "anime", label: "Anime simplify", params: &[("Levels", 3.0, 8.0, 5.0), ("Lines", 0.0, 1.0, 0.5)] },
    Conversion { key: "invert", label: "Invert", params: &[] },
];

/// Run conversion `key` with parameter values `p` (in the order of its params).
pub fn run(key: &str, img: &Rgba, p: &[f64]) -> Rgba {
    let g = |i: usize, d: f64| p.get(i).copied().unwrap_or(d);
    match key {
        "grayscale" => grayscale(img),
        "sketch" => pencil_sketch(img, g(0, 6.0) as usize, g(1, 1.0)),
        "edges" => edges(img, g(0, 0.5), g(1, 1.0) as usize),
        "contours" => contours(img, g(0, 5.0) as u32, g(1, 1.0) as usize),
        "three" => three_levels(img, g(0, 85.0) as u8, g(1, 170.0) as u8),
        "values" => value_scale(img, g(0, 9.0) as u32),
        "posterize" => posterize(img, g(0, 4.0) as u32),
        "threshold" => threshold(img, g(0, 128.0) as u8),
        "hatch" => crosshatch(img, g(0, 6.0) as usize, g(1, 1.0) as usize),
        "halftone" => halftone(img, g(0, 8.0) as usize),
        "silhouette" => silhouette(img),
        "negative" => negative_space(img),
        "nobg" => remove_background(img),
        "anime" => anime_simplify(img, g(0, 5.0) as u32, g(1, 0.5)),
        "invert" => invert(img),
        _ => img.clone(),
    }
}
