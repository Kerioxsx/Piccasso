//! Replica plan for auto-draw: reproduce the picture's shading (tone layers at
//! the right opacity) and, optionally, its lines, at the exact pixel size of
//! the target canvas. Written for Piccasso; no third-party code.
//!
//! Each tone layer is covered with serpentine strokes that stay inside the
//! layer's mask, so one stroke never leaves the shade it belongs to and
//! overlapping rows are part of the same stroke (apps that cap opacity per
//! stroke don't darken overlaps). A simulator renders the plan and reports
//! how closely it matches the source.
use crate::imgproc::{erode, resize_box, to_gray, Gray, Rgba};
use crate::strokes::{strokes_from_lineart, Path};

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicaOptions {
    /// Number of shades including paper white (2..=8).
    pub tones: usize,
    /// Brush diameter in target pixels.
    pub brush_px: f64,
    /// Row spacing as a fraction of the brush (≤ 1 keeps rows overlapping).
    pub overlap: f64,
    /// Draw lines on top of the shading.
    pub lines: bool,
    /// True when the target app can set opacity; otherwise shades are made by
    /// spacing (lighter = sparser rows), the way a single pencil works.
    pub opacity_control: bool,
}
impl Default for ReplicaOptions {
    fn default() -> Self {
        ReplicaOptions { tones: 4, brush_px: 6.0, overlap: 0.85, lines: true, opacity_control: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToneLayer {
    /// 0 = darkest.
    pub tone: usize,
    /// Mean gray of the pixels in this shade.
    pub gray: u8,
    /// Opacity that turns white paper into `gray` (0..1).
    pub opacity: f64,
    /// Strokes in target pixel coordinates.
    pub paths: Vec<Path>,
    /// Fraction of the picture this shade covers.
    pub coverage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplicaPlan {
    pub width: usize,
    pub height: usize,
    /// Lightest drawn shade first, darkest last.
    pub layers: Vec<ToneLayer>,
    pub lines: Vec<Path>,
    /// Quantized target (each pixel = its shade's mean gray).
    pub target: Gray,
}

/// Horizontal runs `[x0, x1]` (inclusive) of `mask` on row `y`.
fn runs(mask: &[u8], w: usize, y: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let row = &mask[y * w..(y + 1) * w];
    let mut x = 0;
    while x < w {
        if row[x] == 0 { x += 1; continue; }
        let s = x;
        while x < w && row[x] != 0 { x += 1; }
        out.push((s, x - 1));
    }
    out
}

/// Serpentine coverage of a mask with rows `spacing` apart (target pixels).
pub fn cover_mask(mask: &[u8], w: usize, h: usize, spacing: f64) -> Vec<Path> {
    let spacing = spacing.max(1.0);
    let mut rows: Vec<usize> = Vec::new();
    let mut y = spacing / 2.0;
    while (y as usize) < h { rows.push(y as usize); y += spacing; }
    if rows.is_empty() && h > 0 { rows.push(h / 2); }
    let mut row_runs: Vec<Vec<(usize, usize)>> = rows.iter().map(|&y| runs(mask, w, y)).collect();
    let mut used: Vec<Vec<bool>> = row_runs.iter().map(|r| vec![false; r.len()]).collect();
    let mut paths = Vec::new();
    for ri in 0..rows.len() {
        for k in 0..row_runs[ri].len() {
            if used[ri][k] { continue; }
            used[ri][k] = true;
            let (a, b) = row_runs[ri][k];
            let yy = rows[ri] as f64 + 0.5;
            let mut path = vec![[a as f64 + 0.5, yy], [b as f64 + 0.5, yy]];
            let mut cur = (a, b);
            let mut at_right = true;
            let mut r = ri;
            // follow overlapping runs downward
            while r + 1 < rows.len() {
                let next = row_runs[r + 1].iter().enumerate().position(|(j, &(c, d))| !used[r + 1][j] && c <= cur.1 && d >= cur.0);
                let Some(j) = next else { break };
                used[r + 1][j] = true;
                let (c, d) = row_runs[r + 1][j];
                let y2 = rows[r + 1] as f64 + 0.5;
                let end = if at_right { cur.1 } else { cur.0 };
                let entry = end.clamp(c, d);
                path.push([entry as f64 + 0.5, y2]);
                // cover the whole run: go to the nearer end first, then the far end
                let (near, far) = if entry - c <= d - entry { (c, d) } else { (d, c) };
                if near != entry { path.push([near as f64 + 0.5, y2]); }
                path.push([far as f64 + 0.5, y2]);
                at_right = far == d;
                cur = (c, d);
                r += 1;
            }
            paths.push(path);
        }
    }
    row_runs.clear();
    paths
}

/// Build the replica plan for a drawing of `tw × th` target pixels.
pub fn plan_replica(img: &Rgba, outline: Option<&Rgba>, tw: usize, th: usize, o: &ReplicaOptions) -> ReplicaPlan {
    let (tw, th) = (tw.max(8), th.max(8));
    // No blur: blurring invents in-between shades along hard edges.
    let g = to_gray(&resize_box(img, tw, th));
    let n = o.tones.clamp(2, 8);
    let band: Vec<usize> = g.data.iter().map(|&v| (v as usize * n) / 256).collect();
    let mut sum = vec![0u64; n];
    let mut cnt = vec![0u64; n];
    for (i, &b) in band.iter().enumerate() { sum[b] += g.data[i] as u64; cnt[b] += 1; }
    let mean: Vec<u8> = (0..n).map(|b| if cnt[b] > 0 { (sum[b] / cnt[b]) as u8 } else { 255 }).collect();
    let mut target = Gray::new(tw, th);
    for (i, &b) in band.iter().enumerate() { target.data[i] = if b == n - 1 { 255 } else { mean[b] }; }
    let base_spacing = o.brush_px * o.overlap;
    let mut layers = Vec::new();
    for b in (0..n - 1).rev() {
        if cnt[b] == 0 { continue; }
        let opacity = 1.0 - mean[b] as f64 / 255.0;
        let mask: Vec<u8> = band.iter().map(|&x| (x == b) as u8).collect();
        let spacing = if o.opacity_control { base_spacing } else { (base_spacing / opacity.max(0.08)).min(o.brush_px * 12.0) };
        // Keep the brush centre one radius inside the shade so its edge lands on the boundary.
        // One pixel less than the radius: real brushes have soft (anti-aliased) edges.
        let r = ((o.brush_px / 2.0).floor() as usize).saturating_sub(1);
        let inner = erode(&mask, tw, th, r);
        let mut paths = cover_mask(&inner, tw, th, spacing);
        // Thin parts the brush can't fit inside: cover what is still missing.
        let mut cov = vec![0u8; tw * th];
        for p in &paths { stamp_path(&mut cov, tw, th, p, o.brush_px / 2.0); }
        let left: Vec<u8> = (0..tw * th).map(|i| (mask[i] != 0 && cov[i] == 0) as u8).collect();
        if o.opacity_control { paths.extend(cover_mask(&left, tw, th, spacing)); }
        layers.push(ToneLayer { tone: b, gray: mean[b], opacity, paths, coverage: cnt[b] as f64 / (tw * th) as f64 });
    }
    let lines = match (o.lines, outline) {
        (true, Some(ol)) => strokes_from_lineart(&resize_box(ol, tw, th), 0.6, 4)
            .into_iter()
            .map(|p| p.into_iter().map(|q| [q[0] * tw as f64, q[1] * th as f64]).collect())
            .collect(),
        _ => vec![],
    };
    ReplicaPlan { width: tw, height: th, layers, lines, target }
}

fn stamp_path(cov: &mut [u8], w: usize, h: usize, p: &[[f64; 2]], r: f64) {
    let ri = r.max(0.5);
    let step = (ri * 0.5).max(0.5);
    let mut put = |x: f64, y: f64| {
        let (x0, x1) = ((x - ri).floor().max(0.0) as usize, ((x + ri).ceil() as usize).min(w.saturating_sub(1)));
        let (y0, y1) = ((y - ri).floor().max(0.0) as usize, ((y + ri).ceil() as usize).min(h.saturating_sub(1)));
        for yy in y0..=y1 {
            for xx in x0..=x1 {
                let (dx, dy) = (xx as f64 + 0.5 - x, yy as f64 + 0.5 - y);
                if dx * dx + dy * dy <= ri * ri { cov[yy * w + xx] = 1; }
            }
        }
    };
    if p.len() == 1 { put(p[0][0], p[0][1]); }
    for s in p.windows(2) {
        let d = (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]);
        let k = (d / step).ceil().max(1.0) as usize;
        for i in 0..=k {
            let t = i as f64 / k as f64;
            put(s[0][0] + (s[1][0] - s[0][0]) * t, s[0][1] + (s[1][1] - s[0][1]) * t);
        }
    }
}

/// Render the plan as a round brush of `brush_px` would, black at each layer's
/// opacity over white (each layer is one pass; overlap inside a layer counts once).
pub fn simulate(plan: &ReplicaPlan, brush_px: f64, opacity_control: bool) -> Gray {
    let (w, h) = (plan.width, plan.height);
    let mut out = vec![255f64; w * h];
    for l in &plan.layers {
        let mut cov = vec![0u8; w * h];
        for p in &l.paths { stamp_path(&mut cov, w, h, p, brush_px / 2.0); }
        let op = if opacity_control { l.opacity } else { 1.0 };
        for i in 0..w * h { if cov[i] != 0 { out[i] *= 1.0 - op; } }
    }
    let mut cov = vec![0u8; w * h];
    for p in &plan.lines { stamp_path(&mut cov, w, h, p, 1.0); }
    for i in 0..w * h { if cov[i] != 0 { out[i] = 30.0; } }
    Gray { width: w, height: h, data: out.iter().map(|&v| v.round().clamp(0.0, 255.0) as u8).collect() }
}

/// Share of pixels whose simulated gray is within one shade step of the target.
pub fn match_score(sim: &Gray, target: &Gray, tones: usize) -> f64 {
    let tol = 255.0 / tones.max(2) as f64 / 2.0 + 1.0;
    let ok = sim.data.iter().zip(&target.data).filter(|(a, b)| (**a as f64 - **b as f64).abs() <= tol).count();
    ok as f64 / sim.data.len().max(1) as f64
}

/// Total stroke length and count (for time estimates).
pub fn plan_stats(plan: &ReplicaPlan) -> (usize, f64) {
    let all = plan.layers.iter().flat_map(|l| l.paths.iter()).chain(plan.lines.iter());
    let (mut n, mut len) = (0usize, 0f64);
    for p in all {
        n += 1;
        for s in p.windows(2) { len += (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]); }
    }
    (n, len)
}
