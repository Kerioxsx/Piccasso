//! Ink-line and hatching detection at full resolution, curve smoothing, and a
//! colour palette planner. Written for Piccasso; no third-party code.
use crate::imgproc::{douglas_peucker, Gray, Rgba};
use crate::strokes::{thin, trace_skeleton, Path};

/// Grayscale max (dilate) or min (erode) filter with a square window.
fn rank_filter(g: &Gray, r: usize, max: bool) -> Gray {
    let (w, h) = (g.width, g.height);
    let pick = |a: u8, b: u8| if max { a.max(b) } else { a.min(b) };
    let mut tmp = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let (lo, hi) = (x.saturating_sub(r), (x + r).min(w - 1));
            let mut v = g.data[y * w + lo];
            for k in lo + 1..=hi { v = pick(v, g.data[y * w + k]); }
            tmp[y * w + x] = v;
        }
    }
    let mut out = Gray::new(w, h);
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(r), (y + r).min(h - 1));
        for x in 0..w {
            let mut v = tmp[lo * w + x];
            for k in lo + 1..=hi { v = pick(v, tmp[k * w + x]); }
            out.data[y * w + x] = v;
        }
    }
    out
}

/// One detected line, in the caller's target pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct InkLine {
    /// Smoothed centre line.
    pub points: Path,
    /// Darkness of the ink core, 0 (paper) .. 1 (black).
    pub darkness: f64,
    /// Opacity that turns the paper/shading under the line into the line's darkness.
    pub over: f64,
    /// Stroke width in target pixels.
    pub width: f64,
    /// Ink colour (mean of the darkest pixels along the line).
    pub rgb: [u8; 3],
}

/// Chaikin corner cutting (keeps endpoints): turns pixel polylines into smooth curves.
pub fn chaikin(p: &[[f64; 2]], iterations: usize) -> Path {
    let mut cur = p.to_vec();
    for _ in 0..iterations {
        if cur.len() < 3 { break; }
        let mut next = Vec::with_capacity(cur.len() * 2);
        next.push(cur[0]);
        for w in cur.windows(2) {
            let (a, b) = (w[0], w[1]);
            next.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
            next.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
        }
        next.push(*cur.last().unwrap());
        cur = next;
    }
    cur
}

/// Thin dark marks (ink lines, pencil hatching) found with a black-hat filter
/// on the full-resolution gray picture: closing(g) − g is high only where a
/// mark is narrower than the window. Returns the lines in target coordinates
/// (`scale` = target / source) and the picture with those marks lifted out
/// (`base`), so the shading pass doesn't paint them a second time.
pub fn ink_lines(g: &Gray, rgb: Option<&Rgba>, scale: f64, max_width_px: usize, min_contrast: u8) -> (Vec<InkLine>, Gray) {
    let (w, h) = (g.width, g.height);
    let r = (max_width_px / 2).max(1);
    let closing = rank_filter(&rank_filter(g, r, true), r, false);
    let mask: Vec<u8> = (0..w * h).map(|i| (closing.data[i].saturating_sub(g.data[i]) >= min_contrast) as u8).collect();
    // grow the mask by one pixel so anti-aliased edges leave with the line
    let grown = crate::imgproc::dilate(&mask, w, h, 1);
    let mut base = g.clone();
    for i in 0..w * h { if grown[i] != 0 { base.data[i] = closing.data[i].max(g.data[i]); } }
    let skel = thin(&mask, w, h);
    // width of each connected mark = area / centre-line length
    let (labels, comps) = crate::imgproc::label_components(&mask, w, h);
    let mut skel_len = vec![0usize; comps.len() + 1];
    for i in 0..w * h { if skel[i] != 0 && labels[i] > 0 { skel_len[labels[i] as usize] += 1; } }
    let mut lines = Vec::new();
    for p in trace_skeleton(&skel, w, h, 3) {
        let (mut dark, mut over, mut col) = (0.0, 0.0, [0f64; 3]);
        let mut lab = 0usize;
        for q in &p {
            let (x, y) = (q[0] as usize, q[1] as usize);
            let (mut m, mut mi) = (255u8, y * w + x);
            for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    if g.data[yy * w + xx] < m { m = g.data[yy * w + xx]; mi = yy * w + xx; }
                }
            }
            if lab == 0 && labels[y * w + x] > 0 { lab = labels[y * w + x] as usize; }
            dark += 1.0 - m as f64 / 255.0;
            over += (1.0 - m as f64 / base.data[y * w + x].max(1) as f64).clamp(0.0, 1.0);
            if let Some(c) = rgb { for k in 0..3 { col[k] += c.data[mi * 4 + k] as f64; } }
        }
        let n = p.len() as f64;
        let width_src = if lab > 0 && skel_len[lab] > 0 { comps[lab - 1].area as f64 / skel_len[lab] as f64 } else { 1.0 };
        let simple = douglas_peucker(&p, 0.6, false);
        let smooth = chaikin(&simple, 2);
        let pts: Path = smooth.into_iter().map(|q| [(q[0] + 0.5) * scale, (q[1] + 0.5) * scale]).collect();
        let rgbv = if rgb.is_some() { [(col[0] / n).round() as u8, (col[1] / n).round() as u8, (col[2] / n).round() as u8] } else { let v = ((1.0 - dark / n) * 255.0).round() as u8; [v, v, v] };
        lines.push(InkLine { points: pts, darkness: dark / n, over: over / n, width: (width_src * scale).max(0.5), rgb: rgbv });
    }
    (lines, base)
}

/// Colour distance that keeps hue apart: luma plus two chroma axes, with
/// chroma weighted up so a small green eye never merges into a gray of the
/// same brightness.
pub fn color_dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    let y = |c: [f64; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let (ya, yb) = (y(a), y(b));
    let (cba, cbb) = (a[2] - ya, b[2] - yb);
    let (cra, crb) = (a[0] - ya, b[0] - yb);
    (ya - yb).powi(2) + 1.5 * ((cba - cbb).powi(2) + (cra - crb).powi(2))
}

fn hue_of(c: [f64; 3]) -> f64 {
    let (r, g, b) = (c[0], c[1], c[2]);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = (mx - mn).max(1e-9);
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    h * 60.0
}

/// Deterministic colour palette over the non-paper pixels: one centre per
/// clearly present hue (so small vivid details like an eye or a tear keep
/// their colour), the rest spread over brightness, refined with k-means.
/// Returns the colours and each pixel's index (usize::MAX = paper, left blank).
pub fn palette(img: &Rgba, k: usize, paper_lum: u8) -> (Vec<[u8; 3]>, Vec<usize>) {
    let n = img.width * img.height;
    let col = |i: usize| [img.data[i * 4] as f64, img.data[i * 4 + 1] as f64, img.data[i * 4 + 2] as f64];
    let chroma = |c: [f64; 3]| c.iter().cloned().fold(0.0, f64::max) - c.iter().cloned().fold(255.0, f64::min);
    let lum = |c: [f64; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let px: Vec<usize> = (0..n).filter(|&i| { let c = col(i); lum(c) < paper_lum as f64 || chroma(c) > 40.0 }).collect();
    let mut assign = vec![usize::MAX; n];
    if px.is_empty() { return (vec![], assign); }
    let k = k.max(1);
    // hue centres: 12 hue bins of clearly coloured pixels
    let mut bins = vec![([0f64; 3], 0usize); 12];
    for &i in &px {
        let c = col(i);
        if chroma(c) < 50.0 { continue; }
        let b = ((hue_of(c) / 30.0) as usize).min(11);
        for q in 0..3 { bins[b].0[q] += c[q]; }
        bins[b].1 += 1;
    }
    let min_count = (px.len() / 2000).max(12);
    let mut hues: Vec<([f64; 3], usize)> = bins.into_iter().filter(|b| b.1 >= min_count).map(|b| ([b.0[0] / b.1 as f64, b.0[1] / b.1 as f64, b.0[2] / b.1 as f64], b.1)).collect();
    hues.sort_by(|a, b| b.1.cmp(&a.1));
    let mut centers: Vec<[f64; 3]> = Vec::new();
    for (c, _) in hues.iter().take((k / 2).max(1)) {
        if centers.iter().all(|d| color_dist(*c, *d) > 900.0) { centers.push(*c); }
    }
    // the rest over brightness (quantiles of the low-chroma pixels)
    let mut grays: Vec<usize> = px.iter().cloned().filter(|&i| chroma(col(i)) < 50.0).collect();
    if grays.is_empty() { grays = px.clone(); }
    grays.sort_by(|a, b| lum(col(*a)).partial_cmp(&lum(col(*b))).unwrap().then(a.cmp(b)));
    let m = k - centers.len().min(k);
    for j in 0..m { centers.push(col(grays[((j as f64 + 0.5) / m as f64 * (grays.len() - 1) as f64) as usize])); }
    let k = centers.len();
    for _ in 0..12 {
        let mut acc = vec![[0f64; 4]; k];
        for &i in &px {
            let c = col(i);
            let (mut best, mut bd) = (0, f64::INFINITY);
            for (m, ce) in centers.iter().enumerate() {
                let d = color_dist(c, *ce);
                if d < bd { bd = d; best = m; }
            }
            assign[i] = best;
            let a = &mut acc[best];
            a[0] += c[0]; a[1] += c[1]; a[2] += c[2]; a[3] += 1.0;
        }
        for (m, ce) in centers.iter_mut().enumerate() { if acc[m][3] > 0.0 { *ce = [acc[m][0] / acc[m][3], acc[m][1] / acc[m][3], acc[m][2] / acc[m][3]]; } }
    }
    (centers.iter().map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8]).collect(), assign)
}
