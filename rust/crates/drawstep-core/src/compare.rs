//! On-device drawing comparison — port of `web/js/core/compare.js`.
use crate::analysis::Analysis;
use crate::geometry::raster_polyline;
use crate::imgproc::*;
use crate::shared::Strings;
use crate::steps::{fill, vars, PaperLayout};

pub fn extract_lines(g: &Gray) -> Mask {
    let (w, h) = (g.width, g.height);
    let mean = blur2(g, (js_round(w.max(h) as f64 / 40.0) as usize).max(2));
    let mut m = vec![0u8; w * h];
    let (bx, by) = (js_round(w as f64 * 0.03) as usize, js_round(h as f64 * 0.03) as usize);
    for y in by..h.saturating_sub(by) {
        for x in bx..w.saturating_sub(bx) {
            let i = y * w + x;
            if g.data[i] < 200 && (g.data[i] as i32 + 18) < mean.data[i] as i32 {
                m[i] = 1;
            }
        }
    }
    m
}

pub fn guide_mask(a: &Analysis, l: &PaperLayout, w: usize, h: usize) -> Mask {
    let mut m = vec![0u8; w * h];
    let to_px = |p: &[f64; 2]| [((l.draw_x + p[0] * l.draw_w) / l.paper_w) * w as f64, ((l.draw_y + p[1] * l.draw_h) / l.paper_h) * h as f64];
    let r = (js_round(w.max(h) as f64 / 250.0) as i64).max(1);
    for c in &a.silhouettes {
        raster_polyline(&mut m, w, h, &c.points.iter().map(to_px).collect::<Vec<_>>(), c.closed, r);
    }
    for c in &a.internals {
        raster_polyline(&mut m, w, h, &c.points.iter().map(to_px).collect::<Vec<_>>(), c.closed, r);
    }
    m
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RBox {
    pub x0: usize,
    pub x1: usize,
    pub y0: usize,
    pub y1: usize,
    pub count: u64,
}

pub fn robust_box(mask: &[u8], w: usize, h: usize) -> Option<RBox> {
    let (mut xs, mut ys) = (vec![0u64; w], vec![0u64; h]);
    let mut n = 0u64;
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] != 0 {
                xs[x] += 1;
                ys[y] += 1;
                n += 1;
            }
        }
    }
    if n == 0 {
        return None;
    }
    let pct = |hist: &[u64], p: f64| {
        let mut acc = 0u64;
        for (i, &v) in hist.iter().enumerate() {
            acc += v;
            if acc as f64 >= p * n as f64 {
                return i;
            }
        }
        hist.len() - 1
    };
    Some(RBox { x0: pct(&xs, 0.02), x1: pct(&xs, 0.98), y0: pct(&ys, 0.02), y1: pct(&ys, 0.98), count: n })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub width_ratio: f64,
    pub height_ratio: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub coverage: f64,
}

pub struct CompareResult {
    pub observations: Vec<String>,
    pub metrics: Option<Metrics>,
    pub drawn: Mask,
}

pub fn compare_drawing(drawn_gray: &Gray, guide: &[u8], strings: &Strings) -> CompareResult {
    let (w, h) = (drawn_gray.width, drawn_gray.height);
    let n = (w * h) as f64;
    let s = &strings.compare;
    let drawn = extract_lines(drawn_gray);
    let (db, gb) = (robust_box(&drawn, w, h), robust_box(guide, w, h));
    let (Some(db), Some(gb)) = (db, gb) else {
        return CompareResult { observations: vec![s.empty.clone()], metrics: None, drawn };
    };
    if (db.count as f64) < 0.002 * n {
        return CompareResult { observations: vec![s.empty.clone()], metrics: None, drawn };
    }
    let mut obs = Vec::new();
    let (gw, gh) = ((gb.x1 - gb.x0 + 1) as f64, (gb.y1 - gb.y0 + 1) as f64);
    let (dw, dh) = ((db.x1 - db.x0 + 1) as f64, (db.y1 - db.y0 + 1) as f64);
    let (wr, hr) = (dw / gw, dh / gh);
    let off_x = ((db.x0 + db.x1) as f64 / 2.0 - (gb.x0 + gb.x1) as f64 / 2.0) / gw;
    let off_y = ((db.y0 + db.y1) as f64 / 2.0 - (gb.y0 + gb.y1) as f64 / 2.0) / gh;
    let pct = |r: f64| format!("{}", js_round((r - 1.0).abs() * 100.0) as i64);
    if wr > 1.08 { obs.push(fill(&s.wider, &vars([("pct", pct(wr))]))); } else if wr < 0.92 { obs.push(fill(&s.narrower, &vars([("pct", pct(wr))]))); }
    if hr > 1.08 { obs.push(fill(&s.taller, &vars([("pct", pct(hr))]))); } else if hr < 0.92 { obs.push(fill(&s.shorter, &vars([("pct", pct(hr))]))); }
    if off_x < -0.08 { obs.push(s.shift_left.clone()); } else if off_x > 0.08 { obs.push(s.shift_right.clone()); }
    if off_y < -0.08 { obs.push(s.shift_up.clone()); } else if off_y > 0.08 { obs.push(s.shift_down.clone()); }
    let near = dilate(&drawn, w, h, (js_round(w.max(h) as f64 / 60.0) as usize).max(2));
    let (mut tot, mut hit) = ([0u64; 9], [0u64; 9]);
    for y in gb.y0..=gb.y1 {
        for x in gb.x0..=gb.x1 {
            let i = y * w + x;
            if guide[i] == 0 { continue; }
            let c = 2.min(((y - gb.y0) * 3) / gh as usize) * 3 + 2.min(((x - gb.x0) * 3) / gw as usize);
            tot[c] += 1;
            if near[i] != 0 { hit[c] += 1; }
        }
    }
    let total: u64 = tot.iter().sum();
    let min_cell = 8f64.max(0.04 * total as f64);
    let (mut worst, mut worst_cov, mut gtot, mut ghit) = (None, 1.0f64, 0u64, 0u64);
    for c in 0..9 {
        gtot += tot[c];
        ghit += hit[c];
        if (tot[c] as f64) < min_cell { continue; }
        let cov = hit[c] as f64 / tot[c] as f64;
        if cov < worst_cov { worst_cov = cov; worst = Some(c); }
    }
    if let Some(c) = worst {
        if worst_cov < 0.35 {
            let code = format!("{}{}", ['t', 'm', 'b'][c / 3], ['l', 'c', 'r'][c % 3]);
            obs.push(fill(&s.sparse, &vars([("pos", strings.positions.get(&code).cloned().unwrap_or_default())])));
        }
    }
    if obs.is_empty() { obs.push(s.good.clone()); }
    CompareResult {
        observations: obs,
        metrics: Some(Metrics { width_ratio: wr, height_ratio: hr, offset_x: off_x, offset_y: off_y, coverage: if gtot > 0 { ghit as f64 / gtot as f64 } else { 0.0 } }),
        drawn,
    }
}
