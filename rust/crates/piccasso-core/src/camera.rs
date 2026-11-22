//! Camera helpers without AI (port of `web/js/core/camera.js`): lighting and
//! focus hints from image statistics, and a classical skin-colour mask so the
//! hand can be shown in front of the guide.
use crate::imgproc::{Gray, Rgba};

/// Variance of the 4-neighbour Laplacian (higher = sharper).
pub fn sharpness(g: &Gray) -> f64 {
    let (w, h, d) = (g.width, g.height, &g.data);
    let (mut s, mut s2, mut n) = (0f64, 0f64, 0f64);
    for y in 1..h.saturating_sub(1) { for x in 1..w.saturating_sub(1) {
        let i = y * w + x;
        let l = 4.0 * d[i] as f64 - d[i - 1] as f64 - d[i + 1] as f64 - d[i - w] as f64 - d[i + w] as f64;
        s += l; s2 += l * l; n += 1.0;
    } }
    if n > 0.0 { s2 / n - (s / n).powi(2) } else { 0.0 }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameStats { pub mean: f64, pub contrast: f64, pub glare: f64, pub uneven_x: f64, pub uneven_y: f64, pub sharpness: f64 }

pub fn frame_stats(g: &Gray) -> FrameStats {
    let (w, h) = (g.width, g.height);
    let (mut s, mut s2, mut glare) = (0f64, 0f64, 0f64);
    let (mut half, mut cnt) = ([0f64; 4], [0f64; 4]);
    for y in 0..h { for x in 0..w {
        let v = g.data[y * w + x] as f64;
        s += v; s2 += v * v; if v >= 250.0 { glare += 1.0; }
        let lr = if x < w / 2 { 0 } else { 1 }; let tb = if y < h / 2 { 2 } else { 3 };
        half[lr] += v; cnt[lr] += 1.0; half[tb] += v; cnt[tb] += 1.0;
    } }
    let n = ((w * h) as f64).max(1.0);
    let mean = s / n;
    let m: Vec<f64> = (0..4).map(|i| half[i] / cnt[i].max(1.0)).collect();
    FrameStats { mean, contrast: (s2 / n - mean * mean).max(0.0).sqrt(), glare: glare / n, uneven_x: (m[0] - m[1]).abs(), uneven_y: (m[2] - m[3]).abs(), sharpness: sharpness(g) }
}

/// Plain-language hints, most important first: (id, text). Empty = looks fine.
pub fn lighting_hints(st: &FrameStats) -> Vec<(&'static str, String)> {
    let mut t = vec![];
    if st.mean < 55.0 { t.push(("dark", "It's dark. Add a lamp or move to a brighter spot.".to_string())); }
    else if st.mean > 230.0 { t.push(("bright", "The picture is washed out. Move away from direct light.".into())); }
    if st.glare > 0.04 { t.push(("glare", "There is glare on the paper. Tilt the lamp or the camera a little.".into())); }
    if st.uneven_x.max(st.uneven_y) > 45.0 { t.push(("uneven", format!("One side is much darker ({}). Light the paper evenly so the guide stays visible.", if st.uneven_x >= st.uneven_y { "left/right" } else { "top/bottom" }))); }
    if st.sharpness < 25.0 && st.contrast > 12.0 { t.push(("blur", "The picture looks blurry. Keep the camera still and check its focus.".into())); }
    t
}

/// Skin-colour mask (YCbCr box) with speckle removal: 0/255 per pixel, plus the skin share.
pub fn skin_mask(img: &Rgba, min_neighbours: u32) -> (Gray, f64) {
    let (w, h) = (img.width, img.height);
    let mut raw = vec![0u8; w * h];
    for (p, px) in img.data.chunks_exact(4).enumerate() {
        let (r, g, b) = (px[0] as f64, px[1] as f64, px[2] as f64);
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
        let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
        raw[p] = (y > 45.0 && (77.0..=127.0).contains(&cb) && (137.0..=177.0).contains(&cr) && r > g && r > b) as u8;
    }
    let mut out = Gray { width: w, height: h, data: vec![0; w * h] };
    let mut count = 0usize;
    for y in 1..h.saturating_sub(1) { for x in 1..w.saturating_sub(1) {
        let p = y * w + x;
        if raw[p] == 0 { continue; }
        let mut nb = 0u32;
        for dy in -1i64..=1 { for dx in -1i64..=1 { nb += raw[(p as i64 + dy * w as i64 + dx) as usize] as u32; } }
        if nb - 1 >= min_neighbours { out.data[p] = 255; count += 1; }
    } }
    (out, count as f64 / ((w * h) as f64).max(1.0))
}

/// Grow a 0/255 mask by r pixels.
pub fn dilate(m: &Gray, r: usize) -> Gray {
    let (w, h) = (m.width, m.height);
    let mut src = m.data.clone();
    for _ in 0..r {
        let mut dst = vec![0u8; w * h];
        for y in 0..h { for x in 0..w { if src[y * w + x] != 0 {
            for yy in y.saturating_sub(1)..(y + 2).min(h) { for xx in x.saturating_sub(1)..(x + 2).min(w) { dst[yy * w + xx] = 255; } }
        } } }
        src = dst;
    }
    Gray { width: w, height: h, data: src }
}

/// The frame's pixels where the (grown) skin mask is set, transparent elsewhere.
pub fn hand_cutout(img: &Rgba) -> Option<Rgba> {
    let (m, share) = skin_mask(img, 5);
    if share < 0.004 { return None; }
    let m = dilate(&m, 2);
    let mut out = img.clone();
    for (p, px) in out.data.chunks_exact_mut(4).enumerate() { px[3] = m.data[p]; }
    Some(out)
}
