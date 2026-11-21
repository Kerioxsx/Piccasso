//! Vector strokes for auto-drawing (written for Piccasso, no third-party code):
//! line-art mask → Zhang–Suen thinning → skeleton path tracing →
//! Douglas–Peucker → greedy ordering that minimises pen-up travel.
use crate::analysis::Analysis;
use crate::imgproc::{douglas_peucker, Rgba};
use crate::motion::start_at_top;

/// A stroke in normalized image coordinates (0..1).
pub type Path = Vec<[f64; 2]>;

/// Zhang–Suen thinning of a 0/1 mask (in place semantics, returns the skeleton).
pub fn thin(mask: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut m: Vec<u8> = mask.iter().map(|&v| (v != 0) as u8).collect();
    // clear the 1-px border so neighbour lookups stay in range
    for x in 0..w { m[x] = 0; m[(h - 1) * w + x] = 0; }
    for y in 0..h { m[y * w] = 0; m[y * w + w - 1] = 0; }
    let mut changed = true;
    let mut del = Vec::new();
    while changed {
        changed = false;
        for pass in 0..2 {
            del.clear();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if m[i] == 0 { continue; }
                    // P2..P9 clockwise from north
                    let p = [m[i - w], m[i - w + 1], m[i + 1], m[i + w + 1], m[i + w], m[i + w - 1], m[i - 1], m[i - w - 1]];
                    let b: u32 = p.iter().map(|&v| v as u32).sum();
                    if !(2..=6).contains(&b) { continue; }
                    let a = (0..8).filter(|&k| p[k] == 0 && p[(k + 1) % 8] == 1).count();
                    if a != 1 { continue; }
                    let (c1, c2) = if pass == 0 { (p[0] * p[2] * p[4], p[2] * p[4] * p[6]) } else { (p[0] * p[2] * p[6], p[0] * p[4] * p[6]) };
                    if c1 == 0 && c2 == 0 { del.push(i); }
                }
            }
            if !del.is_empty() { changed = true; }
            for &i in &del { m[i] = 0; }
        }
    }
    m
}

const NB: [(i64, i64); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];

fn neighbours(m: &[u8], w: usize, h: usize, x: usize, y: usize) -> usize {
    NB.iter().filter(|(dx, dy)| {
        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
        nx >= 0 && ny >= 0 && (nx as usize) < w && (ny as usize) < h && m[ny as usize * w + nx as usize] != 0
    }).count()
}

/// Trace a skeleton into pixel polylines. Starts from endpoints first, then loops.
pub fn trace_skeleton(skel: &[u8], w: usize, h: usize, min_len: usize) -> Vec<Vec<[f64; 2]>> {
    let mut left = skel.to_vec();
    let mut paths = Vec::new();
    let mut starts: Vec<usize> = (0..w * h).filter(|&i| left[i] != 0 && neighbours(&left, w, h, i % w, i / w) == 1).collect();
    starts.extend((0..w * h).filter(|&i| left[i] != 0));
    for s in starts {
        if left[s] == 0 { continue; }
        let (mut x, mut y) = (s % w, s / w);
        let mut path = vec![[x as f64, y as f64]];
        left[s] = 0;
        let mut dir: Option<(i64, i64)> = None;
        loop {
            // prefer continuing straight, then the smallest turn
            let mut best: Option<((i64, i64), i64)> = None;
            for &(dx, dy) in &NB {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h || left[ny as usize * w + nx as usize] == 0 { continue; }
                let score = match dir { Some((px, py)) => -(px * dx + py * dy) * 2 + (dx.abs() + dy.abs()), None => dx.abs() + dy.abs() };
                if best.map_or(true, |(_, s0)| score < s0) { best = Some(((dx, dy), score)); }
            }
            let Some(((dx, dy), _)) = best else { break };
            x = (x as i64 + dx) as usize;
            y = (y as i64 + dy) as usize;
            left[y * w + x] = 0;
            path.push([x as f64, y as f64]);
            dir = Some((dx, dy));
        }
        if path.len() >= min_len { paths.push(path); }
    }
    paths
}

/// Line-art strokes from a rendered outline (dark lines on white).
pub fn strokes_from_lineart(img: &Rgba, eps_px: f64, min_len: usize) -> Vec<Path> {
    let (w, h) = (img.width, img.height);
    let mask: Vec<u8> = img.data.chunks_exact(4).map(|p| ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8 < 128) as u8).collect();
    let skel = thin(&mask, w, h);
    trace_skeleton(&skel, w, h, min_len)
        .into_iter()
        .map(|p| douglas_peucker(&p, eps_px, false).into_iter().map(|q| [(q[0] + 0.5) / w as f64, (q[1] + 0.5) / h as f64]).collect())
        .collect()
}

/// Clean vector strokes from the analysis (silhouettes + inner forms).
pub fn strokes_from_analysis(a: &Analysis, with_internal: bool) -> Vec<Path> {
    let mut out: Vec<Path> = a.silhouettes.iter().map(|c| start_at_top(&c.points, c.closed)).collect();
    if with_internal {
        out.extend(a.internals.iter().map(|c| start_at_top(&c.points, c.closed)));
    }
    out
}

/// Greedy nearest-neighbour ordering (paths may be reversed) to minimise travel.
pub fn order_paths(mut paths: Vec<Path>, aspect: f64) -> Vec<Path> {
    let d = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]) * aspect).hypot(a[1] - b[1]);
    let mut out = Vec::with_capacity(paths.len());
    let mut pos = [0.0, 0.0];
    while !paths.is_empty() {
        let (mut bi, mut rev, mut bd) = (0, false, f64::INFINITY);
        for (i, p) in paths.iter().enumerate() {
            let (s, e) = (p[0], p[p.len() - 1]);
            if d(pos, s) < bd { bd = d(pos, s); bi = i; rev = false; }
            if d(pos, e) < bd { bd = d(pos, e); bi = i; rev = true; }
        }
        let mut p = paths.swap_remove(bi);
        if rev { p.reverse(); }
        pos = p[p.len() - 1];
        out.push(p);
    }
    out
}

/// Total pen-down length and pen-up travel (normalized units, x scaled by aspect).
pub fn path_stats(paths: &[Path], aspect: f64) -> (f64, f64) {
    let d = |a: [f64; 2], b: [f64; 2]| ((a[0] - b[0]) * aspect).hypot(a[1] - b[1]);
    let (mut down, mut up, mut pos) = (0.0, 0.0, [0.0, 0.0]);
    for p in paths {
        up += d(pos, p[0]);
        for k in 1..p.len() { down += d(p[k - 1], p[k]); }
        pos = p[p.len() - 1];
    }
    (down, up)
}

/// Map a normalized point into a screen rectangle (x, y, w, h), keeping the drawing's aspect (contain, centred).
pub fn fit_into(area: [f64; 4], aspect: f64) -> [f64; 4] {
    let [x, y, w, h] = area;
    if w / h > aspect { let nw = h * aspect; [x + (w - nw) / 2.0, y, nw, h] } else { let nh = w / aspect; [x, y + (h - nh) / 2.0, w, nh] }
}

/// Densify a polyline so consecutive points are at most `step` pixels apart (in screen space).
pub fn densify(points: &[[f64; 2]], step: f64) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for k in 0..points.len() {
        if k == 0 { out.push(points[0]); continue; }
        let (a, b) = (points[k - 1], points[k]);
        let n = ((b[0] - a[0]).hypot(b[1] - a[1]) / step).ceil().max(1.0) as usize;
        for i in 1..=n {
            let t = i as f64 / n as f64;
            out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        }
    }
    out
}
