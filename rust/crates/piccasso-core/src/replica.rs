//! Replica plan for auto-draw: reproduce the picture's shading (tone layers at
//! the right opacity) and, optionally, its lines, at the exact pixel size of
//! the target canvas. Written for Piccasso; no third-party code.
//!
//! Each tone layer is covered with serpentine strokes that stay inside the
//! layer's mask, so one stroke never leaves the shade it belongs to and
//! overlapping rows are part of the same stroke (apps that cap opacity per
//! stroke don't darken overlaps). A simulator renders the plan and reports
//! how closely it matches the source.
use crate::imgproc::{dilate, erode, gray_to_rgba, resize_box, to_gray, Gray, Rgba};
use crate::strokes::{strokes_from_lineart, thin, trace_skeleton, Path};
use crate::imgproc::douglas_peucker;

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
    /// Depth mode: build tone by stacking layers at this one opacity (glazing),
    /// which reproduces smooth gradients far better than flat bands. Pixels darker
    /// than `solid_below` get a single 100 % pass.
    pub glaze: Option<f64>,
    pub solid_below: u8,
    /// Most glaze layers (caps drawing time).
    pub max_layers: usize,
    /// Brush for fine strokes (parts thinner than the fill brush, e.g. ink lines).
    /// None = use the fill brush.
    pub line_brush_px: Option<f64>,
    /// Find ink lines and hatching on the full-resolution picture and draw each
    /// as one smooth stroke at its own darkness and width.
    pub detail: bool,
    /// Colour replica with this many paint colours (0 = black only).
    pub colors: usize,
}
impl Default for ReplicaOptions {
    fn default() -> Self {
        ReplicaOptions { tones: 4, brush_px: 6.0, overlap: 0.85, lines: true, opacity_control: true, glaze: None, solid_below: 40, max_layers: 10, line_brush_px: None, detail: true, colors: 0 }
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
    /// Brush diameter for this pass (target pixels).
    pub brush: f64,
    /// Fine pass: centre-line strokes for parts too thin for the fill brush.
    pub fine: bool,
    /// Paint colour (None = black). Colour layers are painted opaque.
    pub rgb: Option<[u8; 3]>,
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
    /// Colour target (colour plans only).
    pub target_rgb: Option<Rgba>,
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
    // whole-pixel step that never exceeds the requested spacing (a fractional
    // step rounds unevenly and leaves 1-px gaps between some rows)
    let step = (spacing.floor() as usize).max(1);
    let mut rows: Vec<usize> = Vec::new();
    let mut y = step / 2;
    while y < h { rows.push(y); y += step; }
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

/// Strokes for one layer mask: fill strokes whose centre stays one brush radius
/// (minus a pixel, for soft brush edges) inside the mask, plus centre-line
/// strokes for the parts too thin for the fill brush.
fn layer_paths(mask: &[u8], tw: usize, th: usize, brush_px: f64, spacing: f64, fine: bool) -> (Vec<Path>, Vec<Path>) {
    let r = ((brush_px / 2.0).floor() as usize).saturating_sub(1);
    let inner = erode(mask, tw, th, r);
    let paths = region_strokes(&inner, tw, th, spacing, spacing <= brush_px, (brush_px * brush_px / 4.0) as u64);
    if !fine { return (paths, vec![]); }
    let mut cov = vec![0u8; tw * th];
    for p in &paths { stamp_path(&mut cov, tw, th, p, brush_px / 2.0); }
    let left: Vec<u8> = (0..tw * th).map(|i| (mask[i] != 0 && cov[i] == 0) as u8).collect();
    (paths, centre_strokes(&left, tw, th))
}

/// Paint each connected region like a person would: trace its outline first (so
/// the edge is smooth and lands exactly on the boundary), then fill the inside
/// row by row without lifting, travelling between rows only through the region
/// itself. One region = one stroke, so programs that cap opacity per stroke
/// never darken overlaps, and there are no staircase row ends on the edges.
pub fn region_strokes(inner: &[u8], tw: usize, th: usize, spacing: f64, outline: bool, min_area: u64) -> Vec<Path> {
    let (labels, comps) = crate::imgproc::label_components(inner, tw, th);
    let mut out = Vec::new();
    for c in &comps {
        let (x0, y0) = (c.min_x, c.min_y);
        let (bw, bh) = (c.max_x - c.min_x + 1, c.max_y - c.min_y + 1);
        let mut m = vec![0u8; bw * bh];
        for y in 0..bh { for x in 0..bw { m[y * bw + x] = (labels[(y0 + y) * tw + x0 + x] == c.label) as u8; } }
        // specks smaller than the brush would become blobs bigger than themselves;
        // real thin features are drawn by the fine (line brush) pass instead
        if c.area < min_area.max(1) { continue; }
        // pieces: the outline ring first, then serpentine fill pieces
        let mut pieces: Vec<Vec<[f64; 2]>> = Vec::new();
        if outline {
            let core = erode(&m, bw, bh, 1);
            let ring: Vec<u8> = (0..bw * bh).map(|i| (m[i] != 0 && core[i] == 0) as u8).collect();
            for p in trace_skeleton(&ring, bw, bh, 1) {
                let simple = douglas_peucker(&p, 0.6, false);
                pieces.push(crate::lines::chaikin(&simple, 2).into_iter().map(|q| [q[0] + 0.5, q[1] + 0.5]).collect());
            }
        }
        pieces.extend(cover_mask(&m, bw, bh, spacing));
        pieces.retain(|p| !p.is_empty());
        // chain all pieces into one stroke, connecting through the region (BFS)
        let px = |q: [f64; 2]| -> usize { let x = (q[0] as usize).min(bw - 1); let y = (q[1] as usize).min(bh - 1); y * bw + x };
        let mut stroke: Vec<[f64; 2]> = Vec::new();
        let mut cur: Option<usize> = None;
        let mut dist = vec![u32::MAX; bw * bh];
        let mut parent = vec![usize::MAX; bw * bh];
        let mut queue = std::collections::VecDeque::new();
        while !pieces.is_empty() {
            let (bi, rev) = match cur {
                None => (0, false),
                Some(start) => {
                    dist.iter_mut().for_each(|d| *d = u32::MAX);
                    queue.clear();
                    dist[start] = 0;
                    parent[start] = usize::MAX;
                    queue.push_back(start);
                    while let Some(i) = queue.pop_front() {
                        let (x, y) = ((i % bw) as i64, (i / bw) as i64);
                        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)] {
                            let (nx, ny) = (x + dx, y + dy);
                            if nx < 0 || ny < 0 || nx >= bw as i64 || ny >= bh as i64 { continue; }
                            let j = ny as usize * bw + nx as usize;
                            if m[j] == 0 || dist[j] != u32::MAX { continue; }
                            dist[j] = dist[i] + 1;
                            parent[j] = i;
                            queue.push_back(j);
                        }
                    }
                    let mut best = (0usize, false, u32::MAX);
                    for (k, p) in pieces.iter().enumerate() {
                        let (a, b) = (dist[px(p[0])], dist[px(p[p.len() - 1])]);
                        if a < best.2 { best = (k, false, a); }
                        if b < best.2 { best = (k, true, b); }
                    }
                    if best.2 == u32::MAX {
                        // not reachable: finish this stroke, start another
                        if !stroke.is_empty() { out.push(std::mem::take(&mut stroke).into_iter().map(|q| [q[0] + x0 as f64, q[1] + y0 as f64]).collect()); }
                        cur = None;
                        continue;
                    }
                    // connector: parent chain from the chosen endpoint back to `start`
                    let p = &pieces[best.0];
                    let target = if best.1 { px(p[p.len() - 1]) } else { px(p[0]) };
                    let mut chain = Vec::new();
                    let mut i = target;
                    while i != usize::MAX && i != start { chain.push([(i % bw) as f64 + 0.5, (i / bw) as f64 + 0.5]); i = parent[i]; }
                    chain.reverse();
                    let chain = if chain.len() > 2 { douglas_peucker(&chain, 0.5, false) } else { chain };
                    stroke.extend(chain);
                    (best.0, best.1)
                }
            };
            let mut p = pieces.swap_remove(bi);
            if rev { p.reverse(); }
            cur = Some(px(p[p.len() - 1]));
            stroke.extend(p);
        }
        if !stroke.is_empty() { out.push(stroke.into_iter().map(|q| [q[0] + x0 as f64, q[1] + y0 as f64]).collect()); }
    }
    out
}

/// Centre-line strokes for a thin mask (skeleton), plus dots for isolated specks.
fn centre_strokes(left: &[u8], tw: usize, th: usize) -> Vec<Path> {
    let skel = thin(left, tw, th);
    let mut fine_paths: Vec<Path> = trace_skeleton(&skel, tw, th, 2)
        .into_iter()
        .map(|p| douglas_peucker(&p, 0.7, false).into_iter().map(|q| [q[0] + 0.5, q[1] + 0.5]).collect())
        .collect();
    // isolated specks (no skeleton) become dots
    let mut dot_cov = vec![0u8; tw * th];
    for p in &fine_paths { stamp_path(&mut dot_cov, tw, th, p, 1.0); }
    let mut y = 0;
    while y < th {
        let mut x = 0;
        while x < tw {
            let i = y * tw + x;
            if left[i] != 0 && dot_cov[i] == 0 && skel[i] == 0 {
                let c = [x as f64 + 0.5, y as f64 + 0.5];
                fine_paths.push(vec![c, c]);
                stamp_path(&mut dot_cov, tw, th, &[c], 1.5);
            }
            x += 1;
        }
        y += 1;
    }
    crate::strokes::order_paths(fine_paths, 1.0)
}

/// Push a fill layer and, when needed, its fine pass.
#[allow(clippy::too_many_arguments)]
fn push_layer(layers: &mut Vec<ToneLayer>, mask: &[u8], tw: usize, th: usize, tone: usize, gray: u8, opacity: f64, o: &ReplicaOptions, spacing: f64, fine: bool) {
    let n = mask.iter().filter(|&&m| m != 0).count();
    if n == 0 { return; }
    let coverage = n as f64 / (tw * th) as f64;
    let (fill, fine_paths) = layer_paths(mask, tw, th, o.brush_px, spacing, fine);
    layers.push(ToneLayer { tone, gray, opacity, paths: fill, coverage, brush: o.brush_px, fine: false, rgb: None });
    if !fine_paths.is_empty() {
        layers.push(ToneLayer { tone, gray, opacity, paths: fine_paths, coverage, brush: o.line_brush_px.unwrap_or(o.brush_px), fine: true, rgb: None });
    }
}

/// Map the paper colour to white and the darkest ink to black (photos of
/// drawings are never pure white), so "paper" is left untouched.
pub fn normalize_levels(g: &Gray) -> Gray {
    let mut v = g.data.clone();
    v.sort_unstable();
    let pick = |q: f64| v[((v.len() - 1) as f64 * q) as usize] as f64;
    let (lo, hi) = (pick(0.01), pick(0.90).max(pick(0.01) + 32.0));
    Gray { width: g.width, height: g.height, data: g.data.iter().map(|&x| (((x as f64 - lo) / (hi - lo)) * 255.0).round().clamp(0.0, 255.0) as u8).collect() }
}

/// Depth (glazing) plan: layer k covers every pixel that needs at least k coats
/// of opacity `a`; coats multiply, so tones = 255·(1−a)^n give many smooth levels
/// with a single opacity setting. Darkest pixels get one solid pass instead.
fn plan_glaze(g: &Gray, a: f64, o: &ReplicaOptions) -> (Vec<ToneLayer>, Gray) {
    let (tw, th) = (g.width, g.height);
    let a = a.clamp(0.05, 0.9);
    let max_n = o.max_layers.max(1);
    let coats: Vec<usize> = g.data.iter().map(|&v| {
        if v < o.solid_below { return usize::MAX; }
        let x = (v.max(1) as f64 / 255.0).ln() / (1.0 - a).ln();
        (x.round() as usize).min(max_n)
    }).collect();
    let mut target = Gray::new(tw, th);
    for i in 0..tw * th {
        target.data[i] = if coats[i] == usize::MAX { 0 } else { (255.0 * (1.0 - a).powi(coats[i] as i32)).round() as u8 };
    }
    let spacing = o.brush_px * o.overlap;
    let mut layers = Vec::new();
    // Thin parts (narrower than the fill brush: ink lines, hatching) are drawn
    // once, along their centre, at the darkness they need. Only areas the fill
    // brush can reach are built up in coats.
    let dark: Vec<u8> = coats.iter().map(|&c| (c >= 1) as u8).collect();
    let r = ((o.brush_px / 2.0).floor() as usize).saturating_sub(1);
    let reach = dilate(&erode(&dark, tw, th, r), tw, th, r);
    // black parts are judged on their own: a thin ink line inside a dark area
    // would otherwise be dropped by the solid pass
    let solid_all: Vec<u8> = coats.iter().map(|&c| (c == usize::MAX) as u8).collect();
    let solid_reach = dilate(&erode(&solid_all, tw, th, r), tw, th, r);
    let thin_px: Vec<bool> = (0..tw * th).map(|i| (dark[i] != 0 && reach[i] == 0) || (solid_all[i] != 0 && solid_reach[i] == 0)).collect();
    for k in 1..=max_n {
        let mask: Vec<u8> = (0..tw * th).map(|i| (coats[i] >= k && coats[i] != usize::MAX && !thin_px[i]) as u8).collect();
        if !mask.iter().any(|&m| m != 0) { break; }
        push_layer(&mut layers, &mask, tw, th, k, (255.0 * (1.0 - a).powi(k as i32)).round() as u8, a, o, spacing, false);
    }
    let solid: Vec<u8> = (0..tw * th).map(|i| (coats[i] == usize::MAX && !thin_px[i]) as u8).collect();
    push_layer(&mut layers, &solid, tw, th, 0, 0, 1.0, o, spacing, false);
    // fine passes: each connected thin part (a whole ink line, a hatching stroke)
    // keeps one darkness, its average, so a line is never split into dashes of
    // different opacity. Lightest group first, darkest (ink) last.
    let thin_mask: Vec<u8> = thin_px.iter().map(|&b| b as u8).collect();
    let (labels, comps) = crate::imgproc::label_components(&thin_mask, tw, th);
    let mut comp_d = vec![0.0f64; comps.len() + 1];
    {
        let mut sum = vec![0.0f64; comps.len() + 1];
        let mut cnt = vec![0usize; comps.len() + 1];
        for i in 0..tw * th {
            let l = labels[i] as usize;
            if l == 0 { continue; }
            sum[l] += 1.0 - target.data[i] as f64 / 255.0;
            cnt[l] += 1;
        }
        for l in 1..=comps.len() { if cnt[l] > 0 { comp_d[l] = sum[l] / cnt[l] as f64; } }
    }
    let groups = [(0.0, 0.3), (0.3, 0.55), (0.55, 0.8), (0.8, 1.01)];
    for (lo, hi) in groups {
        let mut sum = 0.0;
        let mask: Vec<u8> = (0..tw * th).map(|i| {
            let l = labels[i] as usize;
            let on = l != 0 && comp_d[l] > lo && comp_d[l] <= hi;
            if on { sum += comp_d[l]; }
            on as u8
        }).collect();
        let n = mask.iter().filter(|&&m| m != 0).count();
        if n == 0 { continue; }
        let opacity = if hi > 1.0 { 1.0 } else { sum / n as f64 };
        let paths = centre_strokes(&mask, tw, th);
        layers.push(ToneLayer { tone: 0, gray: (255.0 * (1.0 - opacity)).round() as u8, opacity, paths, coverage: n as f64 / (tw * th) as f64,
            brush: o.line_brush_px.unwrap_or(o.brush_px), fine: true, rgb: None });
    }
    (layers, target)
}

/// Build the replica plan for a drawing of `tw × th` target pixels.
pub fn plan_replica(img: &Rgba, outline: Option<&Rgba>, tw: usize, th: usize, o: &ReplicaOptions) -> ReplicaPlan {
    let (tw, th) = (tw.max(8), th.max(8));
    // Detail pass: lines are found on a picture up to 3x the target size, where a
    // 1 px ink line is still a solid line (at target size it is a faint smudge).
    let (ink, g) = if o.detail {
        let s = (img.width as f64 / tw as f64).clamp(1.0, 3.0);
        let (hw, hh) = ((tw as f64 * s).round() as usize, (th as f64 * s).round() as usize);
        let hi_rgb = resize_box(img, hw, hh);
        let hi = normalize_levels(&to_gray(&hi_rgb));
        let max_w = ((o.brush_px * s).ceil() as usize).max(3);
        let (lines, base) = crate::lines::ink_lines(&hi, Some(&hi_rgb), tw as f64 / hw as f64, max_w, 48);
        (lines, to_gray(&resize_box(&gray_to_rgba(&base), tw, th)))
    } else {
        (vec![], normalize_levels(&to_gray(&resize_box(img, tw, th))))
    };
    if o.colors > 0 {
        return plan_color(img, &ink, tw, th, o, outline);
    }
    let (mut layers, mut target) = if let (Some(a), true) = (o.glaze, o.opacity_control) {
        plan_glaze(&g, a, o)
    } else {
        plan_bands(&g, o)
    };
    // lines on top, lightest group first; every line is one smooth stroke
    let line_brush = o.line_brush_px.unwrap_or(o.brush_px).max(1.0);
    let levels = if o.opacity_control { vec![0.3, 0.55, 0.8, 1.0] } else { vec![1.0] };
    let mut groups: std::collections::BTreeMap<(usize, usize), Vec<Path>> = Default::default();
    for l in &ink {
        if l.over < 0.12 && o.opacity_control { continue; }
        let lv = levels.iter().enumerate().min_by(|a, b| (a.1 - l.over).abs().partial_cmp(&(b.1 - l.over).abs()).unwrap()).unwrap().0;
        let wk = width_class(l.width, line_brush, o.brush_px);
        groups.entry((lv, wk)).or_default().push(l.points.clone());
    }
    for ((lv, wk), paths) in groups {
        let opacity = levels[lv];
        let brush = line_brush * wk as f64;
        let paths = crate::strokes::order_paths(paths, 1.0);
        let mut cov = vec![0u8; tw * th];
        for p in &paths { stamp_path(&mut cov, tw, th, p, brush / 2.0); }
        for i in 0..tw * th { if cov[i] != 0 { target.data[i] = (target.data[i] as f64 * (1.0 - opacity)).round() as u8; } }
        let coverage = cov.iter().filter(|&&c| c != 0).count() as f64 / (tw * th) as f64;
        layers.push(ToneLayer { tone: 0, gray: (255.0 * (1.0 - opacity)).round() as u8, opacity, paths, coverage, brush, fine: true, rgb: None });
    }
    let lines = replica_lines(outline, tw, th, o);
    ReplicaPlan { width: tw, height: th, layers, lines, target, target_rgb: None }
}

/// Line widths come in whole multiples of the line brush (each needs a brush
/// size change), never wider than the fill brush.
fn width_class(width: f64, line_brush: f64, brush: f64) -> usize {
    let max_k = ((brush / line_brush).floor() as usize).max(1);
    ((width / line_brush).round() as usize).clamp(1, max_k)
}

/// Flat bands: each shade is one layer at the opacity that gives its mean gray.
fn plan_bands(g: &Gray, o: &ReplicaOptions) -> (Vec<ToneLayer>, Gray) {
    let (tw, th) = (g.width, g.height);
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
        // Keep the brush edge on the shade boundary; thin parts get centre-line strokes.
        push_layer(&mut layers, &mask, tw, th, b, mean[b], opacity, o, spacing, o.opacity_control);
    }
    (layers, target)
}

/// Colour plan: a small palette of opaque paints, lightest first. Each colour's
/// area reaches one brush radius under the darker colours painted after it, so
/// neighbouring colours meet with no white gaps; small parts get the line brush.
fn plan_color(img: &Rgba, ink: &[crate::lines::InkLine], tw: usize, th: usize, o: &ReplicaOptions, outline: Option<&Rgba>) -> ReplicaPlan {
    let small = resize_box(img, tw, th);
    // paper level from the picture itself (photos of paper are never pure white)
    let mut lum: Vec<u8> = small.data.chunks_exact(4).map(|p| ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8) as u8).collect();
    lum.sort_unstable();
    let paper = lum[(lum.len() - 1) * 9 / 10].saturating_sub(18).clamp(120, 245);
    let (pal, assign) = crate::lines::palette(&small, o.colors.clamp(1, 16), paper);
    let l = |c: [u8; 3]| 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
    let mut order: Vec<usize> = (0..pal.len()).collect();
    order.sort_by(|a, b| l(pal[*b]).partial_cmp(&l(pal[*a])).unwrap());
    let mut rank = vec![0usize; pal.len()];
    for (r, &c) in order.iter().enumerate() { rank[c] = r; }
    let n = tw * th;
    let mut target = Rgba { width: tw, height: th, data: vec![255; n * 4] };
    for i in 0..n { if assign[i] != usize::MAX { let c = pal[assign[i]]; target.data[i * 4..i * 4 + 3].copy_from_slice(&c); } }
    let rb = (o.brush_px / 2.0).ceil() as usize;
    let spacing = o.brush_px * o.overlap;
    let mut layers = Vec::new();
    for (r, &c) in order.iter().enumerate() {
        let own: Vec<u8> = assign.iter().map(|&a| (a == c) as u8).collect();
        if !own.iter().any(|&v| v != 0) { continue; }
        let halo = dilate(&own, tw, th, rb);
        let mask: Vec<u8> = (0..n).map(|i| (own[i] != 0 || (halo[i] != 0 && assign[i] != usize::MAX && rank[assign[i]] > r)) as u8).collect();
        let before = layers.len();
        push_layer(&mut layers, &mask, tw, th, r, l(pal[c]).round() as u8, 1.0, o, spacing, true);
        for t in &mut layers[before..] { t.rgb = Some(pal[c]); }
    }
    // ink lines in their nearest paint colour, darkest last
    let line_brush = o.line_brush_px.unwrap_or(o.brush_px).max(1.0);
    let mut groups: std::collections::BTreeMap<(usize, usize), Vec<Path>> = Default::default();
    for li in ink {
        if li.over < 0.15 { continue; }
        let f = |c: [u8; 3]| [c[0] as f64, c[1] as f64, c[2] as f64];
        let d = |p: [u8; 3]| crate::lines::color_dist(f(p), f(li.rgb));
        let ci = (0..pal.len()).min_by(|a, b| d(pal[*a]).partial_cmp(&d(pal[*b])).unwrap()).unwrap_or(0);
        groups.entry((rank[ci], width_class(li.width, line_brush, o.brush_px))).or_default().push(li.points.clone());
    }
    for ((r, wk), paths) in groups {
        let c = pal[order[r]];
        let brush = line_brush * wk as f64;
        let paths = crate::strokes::order_paths(paths, 1.0);
        let mut cov = vec![0u8; n];
        for p in &paths { stamp_path(&mut cov, tw, th, p, brush / 2.0); }
        for i in 0..n { if cov[i] != 0 { target.data[i * 4..i * 4 + 3].copy_from_slice(&c); } }
        let coverage = cov.iter().filter(|&&v| v != 0).count() as f64 / n as f64;
        layers.push(ToneLayer { tone: r, gray: l(c).round() as u8, opacity: 1.0, paths, coverage, brush, fine: true, rgb: Some(c) });
    }
    let tg = to_gray(&target);
    ReplicaPlan { width: tw, height: th, layers, lines: replica_lines(outline, tw, th, o), target: tg, target_rgb: Some(target) }
}

/// Paint colours of a plan, in the order they are used (for the colour picker).
pub fn plan_palette(plan: &ReplicaPlan) -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = Vec::new();
    for l in &plan.layers { if let Some(c) = l.rgb { if !out.contains(&c) { out.push(c); } } }
    out
}

fn replica_lines(outline: Option<&Rgba>, tw: usize, th: usize, o: &ReplicaOptions) -> Vec<Path> {
    match (o.lines, outline) {
        (true, Some(ol)) => strokes_from_lineart(&resize_box(ol, tw, th), 0.6, 4)
            .into_iter()
            .map(|p| p.into_iter().map(|q| [q[0] * tw as f64, q[1] * th as f64]).collect())
            .collect(),
        _ => vec![],
    }
}

/// Mean absolute difference between two gray images (0..255).
pub fn mean_error(a: &Gray, b: &Gray) -> f64 {
    a.data.iter().zip(&b.data).map(|(x, y)| (*x as f64 - *y as f64).abs()).sum::<f64>() / a.data.len().max(1) as f64
}

/// Source picture as gray at the plan's size (to judge fidelity to the original, not only to the plan).
pub fn source_gray(img: &Rgba, w: usize, h: usize) -> Gray {
    normalize_levels(&to_gray(&resize_box(img, w, h)))
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

/// Render the plan in colour as a round brush would: each layer is one pass of
/// its colour (black when none) at its opacity; overlap inside a layer counts once.
pub fn simulate_rgb(plan: &ReplicaPlan, opacity_control: bool) -> Rgba {
    let (w, h) = (plan.width, plan.height);
    let mut out = vec![255f64; w * h * 3];
    for l in &plan.layers {
        let mut cov = vec![0u8; w * h];
        for p in &l.paths { stamp_path(&mut cov, w, h, p, l.brush / 2.0); }
        let op = if opacity_control || l.rgb.is_some() { l.opacity } else { 1.0 };
        let c = l.rgb.unwrap_or([0, 0, 0]);
        for i in 0..w * h {
            if cov[i] == 0 { continue; }
            for k in 0..3 { out[i * 3 + k] = out[i * 3 + k] * (1.0 - op) + c[k] as f64 * op; }
        }
    }
    let mut cov = vec![0u8; w * h];
    for p in &plan.lines { stamp_path(&mut cov, w, h, p, 1.0); }
    let mut data = vec![255u8; w * h * 4];
    for i in 0..w * h {
        for k in 0..3 { data[i * 4 + k] = if cov[i] != 0 { 30 } else { out[i * 3 + k].round().clamp(0.0, 255.0) as u8 }; }
    }
    Rgba { width: w, height: h, data }
}

/// Gray version of [`simulate_rgb`].
pub fn simulate(plan: &ReplicaPlan, _brush_px: f64, opacity_control: bool) -> Gray {
    to_gray(&simulate_rgb(plan, opacity_control))
}

/// Mean colour difference per channel (0..255).
pub fn mean_error_rgb(a: &Rgba, b: &Rgba) -> f64 {
    let mut s = 0.0;
    for i in 0..a.width * a.height { for k in 0..3 { s += (a.data[i * 4 + k] as f64 - b.data[i * 4 + k] as f64).abs(); } }
    s / (a.width * a.height * 3).max(1) as f64
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
