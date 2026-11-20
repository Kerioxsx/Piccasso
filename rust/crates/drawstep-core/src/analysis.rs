//! Local analysis engine "local-v1" — port of `web/js/core/analysis.js`.
use crate::imgproc::*;
use crate::shared::Level;
use serde::{Deserialize, Serialize};

/// Symmetric rounding to 4 decimals (identical to JS `r4`).
pub fn r4(v: f64) -> f64 {
    let s = if v < 0.0 { -1.0 } else { 1.0 };
    s * (v.abs() * 10000.0 + 0.5).floor() / 10000.0
}

/// 3×3 position code (row t/m/b + column l/c/r) of a point inside a box.
pub fn pos_code(x: f64, y: f64, b: [f64; 4]) -> String {
    let w = if b[2] - b[0] == 0.0 { 1.0 } else { b[2] - b[0] };
    let h = if b[3] - b[1] == 0.0 { 1.0 } else { b[3] - b[1] };
    let (fx, fy) = ((x - b[0]) / w, (y - b[1]) / h);
    let col = if fx < 1.0 / 3.0 { 'l' } else if fx < 2.0 / 3.0 { 'c' } else { 'r' };
    let row = if fy < 1.0 / 3.0 { 't' } else if fy < 2.0 / 3.0 { 'm' } else { 'b' };
    format!("{row}{col}")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Subject {
    pub bbox: [f64; 4],
    pub aspect: f64,
    pub whole: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_box: Option<[i64; 4]>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Object {
    pub id: String,
    pub bbox: [f64; 4],
    pub centroid: [f64; 2],
    pub area_frac: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Silhouette {
    pub object_id: String,
    pub closed: bool,
    pub points: Vec<[f64; 2]>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Internal {
    pub id: String,
    pub tone: String,
    pub bbox: [f64; 4],
    pub area_frac: f64,
    pub centroid: [f64; 2],
    pub closed: bool,
    pub points: Vec<[f64; 2]>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Shape {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub cx: f64,
    pub cy: f64,
    pub rx: f64,
    pub ry: f64,
    pub angle: f64,
    pub area_frac: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Values {
    pub levels: u32,
    pub fractions: Vec<f64>,
    pub dark_pos: String,
    pub light_dir: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Detail {
    pub cells: Vec<u64>,
    pub pos: String,
    pub density: f64,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Landmark {
    pub name: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Analysis {
    pub engine: String,
    pub width: usize,
    pub height: usize,
    pub aspect: f64,
    pub threshold: i32,
    pub low_contrast: bool,
    pub subject: Subject,
    pub objects: Vec<Object>,
    pub active_objects: Vec<String>,
    pub silhouettes: Vec<Silhouette>,
    pub internals: Vec<Internal>,
    pub shapes: Vec<Shape>,
    pub values: Values,
    pub detail: Detail,
    pub landmarks: Vec<Landmark>,
}

#[derive(Debug, Clone, Default)]
pub struct AnalyzeOptions {
    pub excluded: Vec<String>,
    pub isolate: Option<String>,
}

fn fit_shape(c: &Component, w: usize, h: usize, id: String, source: &str) -> Shape {
    let a = c.area as f64;
    let (mx, my) = (c.sum_x / a, c.sum_y / a);
    let cxx = c.sum_xx / a - mx * mx;
    let cyy = c.sum_yy / a - my * my;
    let cxy = c.sum_xy / a - mx * my;
    let tr = cxx + cyy;
    let det = cxx * cyy - cxy * cxy;
    let disc = (0f64).max((tr * tr) / 4.0 - det).sqrt();
    let l1 = tr / 2.0 + disc;
    let l2 = (0f64).max(tr / 2.0 - disc);
    let (mut rx, mut ry) = (2.0 * l1.sqrt(), 2.0 * l2.sqrt());
    let mut angle = (0.5 * (2.0 * cxy).atan2(cxx - cyy) * 180.0) / std::f64::consts::PI;
    let bw = (c.max_x - c.min_x + 1) as f64;
    let bh = (c.max_y - c.min_y + 1) as f64;
    let fill = a / (bw * bh);
    let (mut kind, mut cx, mut cy) = ("ellipse", mx + 0.5, my + 0.5);
    if fill > 0.85 {
        kind = "box";
        rx = bw / 2.0;
        ry = bh / 2.0;
        angle = 0.0;
        cx = c.min_x as f64 + bw / 2.0;
        cy = c.min_y as f64 + bh / 2.0;
    } else if ry / rx.max(1e-9) > 0.85 {
        kind = "circle";
        let m = (rx + ry) / 2.0;
        rx = m;
        ry = m;
        angle = 0.0;
    }
    let (wf, hf) = (w as f64, h as f64);
    Shape {
        id, kind: kind.into(), source: source.into(),
        cx: r4(cx / wf), cy: r4(cy / hf), rx: r4(rx.max(0.5) / wf), ry: r4(ry.max(0.5) / wf),
        angle: r4(angle), area_frac: r4(a / (wf * hf)),
    }
}

fn norm_pts(pts: &[[i64; 2]], w: usize, h: usize) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [r4((p[0] as f64 + 0.5) / w as f64), r4((p[1] as f64 + 0.5) / h as f64)]).collect()
}
fn norm_box(c: &Component, w: usize, h: usize) -> [f64; 4] {
    let (wf, hf) = (w as f64, h as f64);
    [r4(c.min_x as f64 / wf), r4(c.min_y as f64 / hf), r4((c.max_x + 1) as f64 / wf), r4((c.max_y + 1) as f64 / hf)]
}
fn centroid(c: &Component, w: usize, h: usize) -> [f64; 2] {
    [r4((c.sum_x / c.area as f64 + 0.5) / w as f64), r4((c.sum_y / c.area as f64 + 0.5) / h as f64)]
}

pub fn analyze(img: &Rgba, level: &Level, opts: &AnalyzeOptions) -> Analysis {
    let (w, h) = (img.width, img.height);
    let n = w * h;
    let nf = n as f64;
    let g = to_gray(img);
    let fg = foreground_from_border(img, level.blur_radius);
    let t = fg.threshold;
    let separation = fg.separation;
    let mask = clean_mask(&fg.mask, w, h);
    let (labels, comps) = label_components(&mask, w, h);
    let min_area = 4u64.max((level.min_area_frac * nf).floor() as u64);
    let mut candidates: Vec<&Component> = comps
        .iter()
        .filter(|c| c.area >= min_area && !(c.borders >= 3 && c.area as f64 > 0.5 * nf))
        .collect();
    candidates.sort_by(|a, b| b.area.cmp(&a.area).then(a.label.cmp(&b.label)));
    let low_contrast = separation < 18.0 || candidates.is_empty();

    struct Obj<'a> {
        id: String,
        c: &'a Component,
    }
    let objects: Vec<Obj> = if low_contrast {
        vec![]
    } else {
        candidates.iter().take(level.max_objects).enumerate().map(|(i, c)| Obj { id: format!("obj-{}", i + 1), c }).collect()
    };
    let active: Vec<&Obj> = objects
        .iter()
        .filter(|o| match &opts.isolate {
            Some(iso) => &o.id == iso,
            None => !opts.excluded.contains(&o.id),
        })
        .collect();
    let use_whole = active.is_empty();

    let mut subj = vec![0u8; n];
    let (mut sx0, mut sy0, mut sx1, mut sy1);
    if use_whole {
        subj.fill(1);
        sx0 = 0; sy0 = 0; sx1 = w - 1; sy1 = h - 1;
    } else {
        let act: Vec<i32> = active.iter().map(|o| o.c.label).collect();
        for i in 0..n {
            if act.contains(&labels[i]) {
                subj[i] = 1;
            }
        }
        sx0 = w; sy0 = h; sx1 = 0; sy1 = 0;
        for o in &active {
            sx0 = sx0.min(o.c.min_x); sy0 = sy0.min(o.c.min_y);
            sx1 = sx1.max(o.c.max_x); sy1 = sy1.max(o.c.max_y);
        }
    }
    let (wf, hf) = (w as f64, h as f64);
    let subj_box = [r4(sx0 as f64 / wf), r4(sy0 as f64 / hf), r4((sx1 + 1) as f64 / wf), r4((sy1 + 1) as f64 / hf)];
    let (subj_w, subj_h) = (sx1 - sx0 + 1, sy1 - sy0 + 1);
    let eps = level.dp_epsilon * w.max(h) as f64;

    let silhouettes: Vec<Silhouette> = if use_whole {
        vec![]
    } else {
        active.iter().take(3).map(|o| {
            let raw = trace_boundary(&labels, w, h, o.c.label, o.c.start_x, o.c.start_y);
            Silhouette { object_id: o.id.clone(), closed: true, points: norm_pts(&douglas_peucker(&raw_f(&raw), eps, true).iter().map(|p| [p[0] as i64, p[1] as i64]).collect::<Vec<_>>(), w, h) }
        }).collect()
    };

    // Value bands
    let lv_count = level.value_levels as usize;
    let vb = blur2(&g, level.blur_radius * 2 + 1);
    let lv = posterize_levels(&vb, level.value_levels);
    let mut counts = vec![0u64; lv_count];
    let (mut subj_count, mut bxs, mut bys, mut bws, mut dxs, mut dys, mut dws) = (0u64, 0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if subj[i] == 0 {
                continue;
            }
            subj_count += 1;
            counts[lv[i] as usize] += 1;
            let v = vb.data[i] as f64;
            bxs += x as f64 * v; bys += y as f64 * v; bws += v;
            dxs += x as f64 * (255.0 - v); dys += y as f64 * (255.0 - v); dws += 255.0 - v;
        }
    }
    let sc = subj_count as f64;
    let mut dl = 0usize;
    while dl < lv_count - 1 && (counts[dl] as f64) < 0.01 * sc { dl += 1; }
    let mut hl = lv_count - 1;
    while hl > 0 && (counts[hl] as f64) < 0.01 * sc { hl -= 1; }

    let mut light_dir = "even".to_string();
    if bws > 0.0 && dws > 0.0 {
        let m0 = subj_w.max(subj_h) as f64;
        let vx = (bxs / bws - dxs / dws) / m0;
        let vy = (bys / bws - dys / dws) / m0;
        let m = vx.hypot(vy);
        if m >= 0.04 {
            let col = if vx > 0.38 * m { 'r' } else if vx < -0.38 * m { 'l' } else { 'c' };
            let row = if vy > 0.38 * m { 'b' } else if vy < -0.38 * m { 't' } else { 'm' };
            let code = format!("{row}{col}");
            light_dir = if code == "mc" { "even".into() } else { code };
        }
    }

    // Internal forms
    struct Raw {
        tone: &'static str,
        c: Component,
        labels: std::rc::Rc<Vec<i32>>,
    }
    let mut raws: Vec<Raw> = Vec::new();
    let (mut dark_sx, mut dark_sy, mut dark_n) = (0f64, 0f64, 0f64);
    for (tone, band) in [("dark", dl), ("light", hl)] {
        if tone == "light" && hl == dl {
            continue;
        }
        let mut m2 = vec![0u8; n];
        for i in 0..n {
            if subj[i] != 0 && lv[i] as usize == band {
                m2[i] = 1;
                if tone == "dark" {
                    dark_sx += (i % w) as f64;
                    dark_sy += (i / w) as f64;
                    dark_n += 1.0;
                }
            }
        }
        let (l2, cs) = label_components(&m2, w, h);
        let l2 = std::rc::Rc::new(l2);
        for c in cs {
            if c.area < 4u64.max(min_area >> 1) || c.area as f64 > 0.9 * sc {
                continue;
            }
            raws.push(Raw { tone, c, labels: l2.clone() });
        }
    }
    raws.sort_by(|a, b| {
        b.c.area.cmp(&a.c.area).then_with(|| {
            if a.tone == b.tone { a.c.label.cmp(&b.c.label) } else if a.tone == "dark" { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
        })
    });
    raws.truncate(level.max_internal);
    let internals: Vec<Internal> = raws.iter().enumerate().map(|(i, r)| {
        let raw = trace_boundary(&r.labels, w, h, r.c.label, r.c.start_x, r.c.start_y);
        Internal {
            id: format!("f-{}", i + 1), tone: r.tone.into(), bbox: norm_box(&r.c, w, h), area_frac: r4(r.c.area as f64 / nf),
            centroid: centroid(&r.c, w, h), closed: true,
            points: norm_pts(&douglas_peucker(&raw_f(&raw), eps, true).iter().map(|p| [p[0] as i64, p[1] as i64]).collect::<Vec<_>>(), w, h),
        }
    }).collect();

    // Construction shapes
    let mut shapes: Vec<Shape> = Vec::new();
    for o in &active {
        if shapes.len() >= level.max_shapes { break; }
        let id = format!("s-{}", shapes.len() + 1);
        shapes.push(fit_shape(o.c, w, h, id, "object"));
    }
    for r in &raws {
        if shapes.len() >= level.max_shapes { break; }
        let s = fit_shape(&r.c, w, h, format!("s-{}", shapes.len() + 1), "form");
        if s.rx >= 0.02 { shapes.push(s); }
    }
    if shapes.is_empty() {
        shapes.push(Shape {
            id: "s-1".into(), kind: "box".into(), source: "frame".into(),
            cx: r4((subj_box[0] + subj_box[2]) / 2.0), cy: r4((subj_box[1] + subj_box[3]) / 2.0),
            rx: r4((subj_box[2] - subj_box[0]) / 2.0), ry: r4(((subj_box[3] - subj_box[1]) * hf) / wf / 2.0),
            angle: 0.0, area_frac: r4((subj_w * subj_h) as f64 / nf),
        });
    }

    // Detail cells
    let edges = color_sobel(img, level.blur_radius);
    let et = percentile_threshold(&edges, level.edge_fraction, level.min_edge);
    let mut cells = vec![0u64; 9];
    let mut edge_count = 0u64;
    for y in sy0..=sy1 {
        for x in sx0..=sx1 {
            if (edges.data[y * w + x] as i32) < et { continue; }
            let col = 2.min(((x - sx0) * 3) / subj_w);
            let row = 2.min(((y - sy0) * 3) / subj_h);
            cells[row * 3 + col] += 1;
            edge_count += 1;
        }
    }
    let mut best = 4usize;
    for i in 0..9 { if cells[i] > cells[best] { best = i; } }
    let detail_pos = format!("{}{}", ['t', 'm', 'b'][best / 3], ['l', 'c', 'r'][best % 3]);

    // Landmarks
    let (mut top, mut bottom, mut left, mut right): (Option<[f64; 2]>, Option<[f64; 2]>, Option<[f64; 2]>, Option<[f64; 2]>) = (None, None, None, None);
    for y in 0..h {
        for x in 0..w {
            if subj[y * w + x] == 0 { continue; }
            let p = [x as f64, y as f64];
            if top.is_none() { top = Some(p); }
            bottom = Some(p);
            if left.map_or(true, |l| p[0] < l[0]) { left = Some(p); }
            if right.map_or(true, |r| p[0] > r[0]) { right = Some(p); }
        }
    }
    let lm = |name: &str, p: [f64; 2]| Landmark { name: name.into(), x: r4((p[0] + 0.5) / wf), y: r4((p[1] + 0.5) / hf) };
    let mut landmarks = Vec::new();
    if let (Some(t0), Some(b0), Some(l0), Some(r0)) = (top, bottom, left, right) {
        landmarks.extend([lm("top", t0), lm("bottom", b0), lm("left", l0), lm("right", r0)]);
    }
    if dark_n > 0.0 { landmarks.push(lm("darkest", [dark_sx / dark_n, dark_sy / dark_n])); }
    let dark_pos = if dark_n > 0.0 { pos_code((dark_sx / dark_n + 0.5) / wf, (dark_sy / dark_n + 0.5) / hf, subj_box) } else { "mc".into() };

    Analysis {
        engine: "local-v1".into(), width: w, height: h, aspect: r4(wf / hf), threshold: t, low_contrast,
        subject: Subject { bbox: subj_box, aspect: r4(subj_w as f64 / subj_h as f64), whole: use_whole, pixel_box: Some([sx0 as i64, sy0 as i64, sx1 as i64 + 1, sy1 as i64 + 1]) },
        objects: objects.iter().map(|o| Object { id: o.id.clone(), bbox: norm_box(o.c, w, h), centroid: centroid(o.c, w, h), area_frac: r4(o.c.area as f64 / nf) }).collect(),
        active_objects: active.iter().map(|o| o.id.clone()).collect(),
        silhouettes, internals, shapes,
        values: Values { levels: level.value_levels, fractions: counts.iter().map(|&c| r4(if subj_count > 0 { c as f64 / sc } else { 0.0 })).collect(), dark_pos, light_dir },
        detail: Detail { cells, pos: detail_pos, density: r4(if subj_count > 0 { edge_count as f64 / (subj_w * subj_h) as f64 } else { 0.0 }) },
        landmarks,
    }
}

fn raw_f(p: &[[i64; 2]]) -> Vec<[f64; 2]> {
    p.iter().map(|q| [q[0] as f64, q[1] as f64]).collect()
}
