//! Drawing analysis without AI (port of `web/js/core/inspect.js`): value
//! statistics, rule-based advice, symmetry, measurements, colour sampling.
use crate::imgproc::{fit_dims, histogram, resize_box, to_gray, Rgba};

#[derive(Debug, Clone)]
pub struct ValueStats { pub mean: f64, pub contrast: f64, pub dark: u8, pub light: u8, pub bands: [f64; 5], pub hist: [u64; 256] }

pub fn value_stats(img: &Rgba) -> ValueStats {
    let g = to_gray(img);
    let hist = histogram(&g);
    let n = g.data.len().max(1) as f64;
    let mean = (0..256).map(|v| v as f64 * hist[v] as f64).sum::<f64>() / n;
    let var = (0..256).map(|v| hist[v] as f64 * (v as f64 - mean).powi(2)).sum::<f64>() / n;
    let pct = |q: f64| { let mut acc = 0.0; for v in 0..256 { acc += hist[v] as f64; if acc >= q * n { return v as u8; } } 255 };
    let mut bands = [0.0; 5];
    for v in 0..256 { bands[((v as f64 / 51.2) as usize).min(4)] += hist[v] as f64 / n; }
    ValueStats { mean, contrast: var.sqrt(), dark: pct(0.02), light: pct(0.98), bands, hist }
}

const BAND_NAMES: [&str; 5] = ["darkest darks", "dark values", "middle values", "light values", "lightest lights"];

/// (level: "big" | "small" | "good", text)
pub fn compare_values(r: &ValueStats, d: &ValueStats) -> Vec<(&'static str, String)> {
    let mut t = vec![];
    if d.dark as i32 - r.dark as i32 > 25 { t.push(("big", format!("Your darkest darks are lighter than the reference ({} vs {} of 255). Use a softer pencil (4B–6B) or more layers in the darkest areas.", d.dark, r.dark))); }
    if r.light as i32 - d.light as i32 > 25 { t.push(("big", "Your lightest areas are greyer than the reference. Keep the paper white there, or lift graphite with a kneaded eraser.".to_string())); }
    if r.contrast - d.contrast > 12.0 { t.push(("big", format!("Your drawing has less contrast ({} vs {}). Push the darks darker and keep the lights clean.", d.contrast.round(), r.contrast.round()))); }
    else if d.contrast - r.contrast > 15.0 { t.push(("small", "Your drawing is more contrasty than the reference. Soften the middle values.".into())); }
    if (d.mean - r.mean).abs() > 20.0 { t.push(("small", if d.mean > r.mean { "Overall your drawing is lighter than the reference.".into() } else { "Overall your drawing is darker than the reference.".into() })); }
    for i in 0..5 { let x = d.bands[i] - r.bands[i]; if x.abs() > 0.12 { t.push(("small", format!("{} {} than the reference ({} % vs {} % of the picture).", if x > 0.0 { "More" } else { "Less" }, BAND_NAMES[i], (d.bands[i] * 100.0).round(), (r.bands[i] * 100.0).round()))); } }
    if t.is_empty() { t.push(("good", "The values match the reference closely. Nice work!".into())); }
    t
}

/// Left/right symmetry about a vertical axis (0..1 of the width): (score 0..1, diff map, w, h).
pub fn symmetry(img: &Rgba, axis: f64, max_dim: usize) -> (f64, Vec<u8>, usize, usize) {
    let (w, h) = fit_dims(img.width, img.height, max_dim);
    let g = to_gray(&if w == img.width { img.clone() } else { resize_box(img, w, h) });
    let ax = axis * w as f64;
    let mut diff = vec![0u8; w * h];
    let (mut sum, mut n) = (0f64, 0f64);
    for y in 0..h { for x in 0..ax.floor() as usize {
        let xm = (2.0 * ax - x as f64 - 1.0).round() as i64;
        if xm < 0 || xm >= w as i64 { continue; }
        let dv = (g.data[y * w + x] as i32 - g.data[y * w + xm as usize] as i32).unsigned_abs() as u8;
        diff[y * w + x] = dv; sum += dv as f64; n += 1.0;
    } }
    (if n > 0.0 { 1.0 - sum / n / 255.0 } else { 1.0 }, diff, w, h)
}

/// Colour at a pixel: (hex, value 0..100).
pub fn sample(img: &Rgba, x: usize, y: usize) -> (String, u8) {
    let i = (y.min(img.height - 1) * img.width + x.min(img.width - 1)) * 4;
    let (r, g, b) = (img.data[i], img.data[i + 1], img.data[i + 2]);
    (format!("#{r:02x}{g:02x}{b:02x}"), ((77.0 * r as f64 + 150.0 * g as f64 + 29.0 * b as f64) / 256.0 / 255.0 * 100.0).round() as u8)
}
