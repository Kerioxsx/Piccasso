//! Step generation — port of `web/js/core/steps.js`.
use crate::analysis::{pos_code, Analysis, Shape};
use crate::project::{PaperCfg, Prep, Step};
use crate::shared::{PaperPreset, Shared, Strings};
use std::collections::HashMap;

/// Fill `{name}` placeholders; unknown names are left untouched.
pub fn fill(tpl: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(tpl.len());
    let b = tpl.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'{' {
            if let Some(end) = tpl[i + 1..].find('}') {
                let key = &tpl[i + 1..i + 1 + end];
                if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    if let Some(v) = vars.get(key) {
                        out.push_str(v);
                        i += end + 2;
                        continue;
                    }
                }
            }
        }
        let ch = tpl[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn vars<const N: usize>(pairs: [(&'static str, String); N]) -> HashMap<&'static str, String> {
    pairs.into_iter().collect()
}

/// One decimal with symmetric rounding (matches JS `fmt1`).
pub fn fmt1(v: f64) -> String {
    let r = (v.abs() * 10.0 + 0.5).floor() / 10.0;
    format!("{}{:.1}", if v < 0.0 { "-" } else { "" }, r)
}

pub fn join_list(items: &[String]) -> String {
    match items.len() {
        0 => String::new(),
        1 => items[0].clone(),
        n => format!("{} and {}", items[..n - 1].join(", "), items[n - 1]),
    }
}

pub fn aspect_phrase(a: f64, s: &Strings) -> String {
    if a < 0.83 {
        fill(&s.aspect.taller, &vars([("r", fmt1(1.0 / a))]))
    } else if a > 1.2 {
        fill(&s.aspect.wider, &vars([("r", fmt1(a))]))
    } else {
        s.aspect.square.clone()
    }
}

/// Where the drawing goes on the page (mm).
#[derive(Debug, Clone, PartialEq)]
pub struct PaperLayout {
    pub preset: String,
    pub label: String,
    pub orient: String,
    pub paper_w: f64,
    pub paper_h: f64,
    pub margin: f64,
    pub draw_w: f64,
    pub draw_h: f64,
    pub draw_x: f64,
    pub draw_y: f64,
}

pub fn paper_layout(cfg: &PaperCfg, presets: &[PaperPreset], img_aspect: f64, default_margin: f64) -> PaperLayout {
    let custom;
    let mut preset = presets.iter().find(|p| p.id == cfg.preset).unwrap_or(&presets[0]);
    if cfg.preset == "custom" && cfg.width_mm > 0.0 && cfg.height_mm > 0.0 {
        custom = PaperPreset { id: "custom".into(), label: "Custom".into(), width_mm: cfg.width_mm.min(cfg.height_mm), height_mm: cfg.width_mm.max(cfg.height_mm) };
        preset = &custom;
    }
    let margin = if cfg.margin_mm.is_finite() { cfg.margin_mm } else { default_margin };
    let orient = if cfg.orientation == "portrait" || cfg.orientation == "landscape" {
        cfg.orientation.clone()
    } else if img_aspect > 1.05 {
        "landscape".into()
    } else {
        "portrait".into()
    };
    let (mut pw, mut ph) = (preset.width_mm, preset.height_mm);
    if orient == "landscape" {
        std::mem::swap(&mut pw, &mut ph);
    }
    let aw = (pw - 2.0 * margin).max(10.0);
    let ah = (ph - 2.0 * margin).max(10.0);
    let (dw, dh) = if img_aspect > aw / ah { (aw, aw / img_aspect) } else { (ah * img_aspect, ah) };
    PaperLayout {
        preset: preset.id.clone(), label: preset.label.clone(), orient, paper_w: pw, paper_h: ph, margin,
        draw_w: dw, draw_h: dh, draw_x: (pw - dw) / 2.0, draw_y: (ph - dh) / 2.0,
    }
}

fn shape_area(s: &Shape) -> f64 {
    s.rx * s.ry
}

fn size_word<'a>(s: &Shape, largest: &Shape, st: &'a Strings) -> &'a str {
    let r = shape_area(s) / shape_area(largest).max(1e-9);
    if r > 0.5 { &st.sizes[0] } else if r > 0.15 { &st.sizes[1] } else { &st.sizes[2] }
}

/// Vertices where the outline turns by more than 35°.
pub fn count_turns(points: &[[f64; 2]], aspect: f64) -> usize {
    let n = points.len();
    if n < 3 {
        return n;
    }
    let lim = (35f64 * std::f64::consts::PI / 180.0).cos();
    let mut turns = 0;
    for i in 0..n {
        let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
        let (ux, uy) = ((b[0] - a[0]) * aspect, b[1] - a[1]);
        let (vx, vy) = ((c[0] - b[0]) * aspect, c[1] - b[1]);
        let (lu, lv) = (ux.hypot(uy), vx.hypot(vy));
        if lu == 0.0 || lv == 0.0 {
            continue;
        }
        if (ux * vx + uy * vy) / (lu * lv) < lim {
            turns += 1;
        }
    }
    turns
}

pub fn generate_steps(a: &Analysis, prep: &Prep, sh: &Shared) -> Vec<Step> {
    let s = &sh.strings;
    let level = sh.level(&prep.difficulty);
    let p = |code: &str| sh.position(code).to_string();
    let mut kinds: Vec<String> = level.steps.clone();
    if a.internals.len() < 2 {
        kinds.retain(|k| k != "forms");
    }
    if a.internals.is_empty() && a.detail.density < 0.02 {
        kinds.retain(|k| k != "features");
    }
    if prep.style == "contour_only" {
        kinds.retain(|k| k != "values" && k != "details");
    }
    let tech = &a.technique;
    if tech.bands.len() < 2 || prep.style == "contour_only" {
        kinds.retain(|k| k != "pressure");
    }
    if prep.medium == "color" && !tech.palette.is_empty() && prep.style != "contour_only" {
        let at = kinds.iter().position(|k| k == "values").map(|i| i + 1).unwrap_or(kinds.len());
        kinds.insert(at, "color".into());
    }
    if prep.style == "shading_guide" && !kinds.iter().any(|k| k == "values") {
        let at = kinds.iter().position(|k| k == "clean").map(|i| i + 1).unwrap_or(kinds.len());
        kinds.insert(at, "values".into());
    }
    let layout = paper_layout(&prep.paper, &sh.paper.presets, a.aspect, sh.paper.default_margin_mm);
    let mut shapes = a.shapes.clone();
    shapes.sort_by(|x, y| shape_area(y).partial_cmp(&shape_area(x)).unwrap().then_with(|| x.id.cmp(&y.id)));
    let kind_word = |sp: &Shape| s.shape_kinds.get(&sp.kind).cloned().unwrap_or_else(|| sp.kind.clone());
    let largest = shapes[0].clone();
    let phrases: Vec<String> = shapes.iter().take(3).map(|sp| {
        fill(&s.shape_phrase, &vars([("size", size_word(sp, &largest, s).to_string()), ("kind", kind_word(sp)), ("pos", p(&pos_code(sp.cx, sp.cy, [0.0, 0.0, 1.0, 1.0])))]))
    }).collect();
    let aspect_p = aspect_phrase(a.subject.aspect, s);
    let compare_list = if shapes.len() >= 2 {
        let (x, y) = (&shapes[0], &shapes[1]);
        let r = (shape_area(x) / shape_area(y).max(1e-9)).sqrt();
        let ratio = if r < 1.25 { s.ratio_words.same.clone() } else if r < 2.5 { s.ratio_words.twice.clone() } else { fill(&s.ratio_words.x, &vars([("n", format!("{}", crate::imgproc::js_round(r) as i64))])) };
        fill(&s.compare_item, &vars([("a", size_word(x, &largest, s).into()), ("kindA", kind_word(x)), ("b", size_word(y, &largest, s).into()), ("kindB", kind_word(y)), ("ratio", ratio)]))
    } else {
        fill(&s.compare_single, &vars([("aspectPhrase", aspect_p.clone())]))
    };
    let mut sorted_bands = tech.bands.clone();
    sorted_bands.sort_by(|x, y| y.band.cmp(&x.band));
    let pressure_list = join_list(&sorted_bands.iter().map(|b| {
        let e = pressure_for(&tech.bands, b.band, sh);
        fill(&s.pressure_item, &vars([("label", s.pressure_labels.get(&e.label).cloned().unwrap_or(e.label.clone())), ("pct", format!("{}", e.pct)), ("grade", e.grade.clone()), ("pos", p(&b.pos))]))
    }).collect::<Vec<_>>());
    let mut seen: Vec<String> = Vec::new();
    let mut pal_items = Vec::new();
    for c in &tech.palette {
        let name = color_name(c.rgb, &sh.colors.colors);
        if seen.contains(&name) { continue; }
        seen.push(name.clone());
        pal_items.push(fill(&s.palette_item, &vars([("name", name), ("where", p(&c.pos))])));
    }
    let palette_list = join_list(&pal_items);
    let grid_mm = if prep.paper.grid_cm > 0.0 { prep.paper.grid_cm * 10.0 } else { 0.0 };
    let top = a.landmarks.iter().find(|l| l.name == "top");
    let big = a.internals.first();
    let v: HashMap<&str, String> = vars([
        ("orient", layout.orient.clone()),
        ("marginCm", fmt1(layout.margin / 10.0)),
        ("drawWcm", fmt1(layout.draw_w / 10.0)),
        ("drawHcm", fmt1(layout.draw_h / 10.0)),
        ("shapeList", join_list(&phrases)),
        ("aspectPhrase", aspect_p),
        ("compareList", compare_list),
        ("startPos", p(&top.map(|l| pos_code(l.x, l.y, [0.0, 0.0, 1.0, 1.0])).unwrap_or_else(|| "tc".into()))),
        ("cornerCount", format!("{}", a.silhouettes.first().map(|sl| count_turns(&sl.points, a.aspect)).unwrap_or(4))),
        ("internalCount", format!("{}", a.internals.len())),
        ("internalPos", p(&big.map(|b| pos_code(b.centroid[0], b.centroid[1], a.subject.bbox)).unwrap_or_else(|| "mc".into()))),
        ("detailPos", p(&a.detail.pos)),
        ("darkPos", p(&a.values.dark_pos)),
        ("lightSentence", if a.values.light_dir == "even" { s.light.even.clone() } else { fill(&s.light.dir, &vars([("dir", p(&a.values.light_dir))])) }),
        ("bandCount", format!("{}", tech.bands.len())),
        ("pressureList", pressure_list),
        ("highlightTip", s.highlight_tip.clone()),
        ("shadeMethod", s.shade_methods.get(&prep.difficulty).or_else(|| s.shade_methods.get("easy")).cloned().unwrap_or_default()),
        ("paletteList", palette_list),
        ("colorTip", s.color_tip.clone()),
    ]);
    kinds.iter().enumerate().map(|(i, kind)| {
        let t = &s.steps[kind];
        let def = sh.step_kinds.kinds.iter().find(|k| &k.id == kind);
        let mut instruction = fill(&t.instruction, &v);
        if kind == "page" && a.low_contrast {
            instruction.push(' ');
            instruction.push_str(&s.fallback.low_contrast);
        }
        let (mut base, mut layers) = def.map(|d| (d.base.clone(), d.layers.clone())).unwrap_or(("light".into(), vec![]));
        if kind == "page" && grid_mm > 0.0 {
            let cols = (layout.draw_w / grid_mm - 1e-9).ceil() as i64;
            let rows = (layout.draw_h / grid_mm - 1e-9).ceil() as i64;
            instruction.push(' ');
            instruction.push_str(&fill(&s.grid, &vars([("gridCm", fmt1(grid_mm / 10.0)), ("cols", cols.to_string()), ("rows", rows.to_string())])));
            layers.push("cmgrid".into());
        }
        if kind == "contours" && a.silhouettes.is_empty() {
            base = "outline".into();
            layers = vec!["shapes".into()];
        }
        Step {
            id: format!("step-{}-{}", i + 1, kind), kind: kind.clone(), title: t.title.clone(), instruction, tip: t.tip.clone(),
            checks: t.checks.iter().map(|c| fill(c, &v)).collect(), motion: t.motion.clone(), base, layers,
            terms: sh.glossary.step_terms.get(kind).cloned().unwrap_or_default(),
            ..Default::default()
        }
    }).collect()
}

/// Pressure entry for a band by its rank among present bands (darkest first).
pub fn pressure_for(bands: &[crate::analysis::Band], band: u32, sh: &Shared) -> crate::shared::PressureEntry {
    let mut order: Vec<u32> = bands.iter().map(|b| b.band).collect();
    order.sort();
    let n = order.len();
    let rank = order.iter().position(|&b| b == band).unwrap_or(0);
    let table = &sh.pressure.by_levels[&n.clamp(3, 5).to_string()];
    if n < 3 {
        return if rank == 0 { table[0].clone() } else { table[table.len() - 1].clone() };
    }
    table[rank].clone()
}

fn chroma(c: [f64; 3]) -> f64 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// Nearest plain colour name ("redmean" weighted RGB + 2×chroma² difference).
pub fn color_name(rgb: [i64; 3], table: &[(String, [i32; 3])]) -> String {
    let r = [rgb[0] as f64, rgb[1] as f64, rgb[2] as f64];
    let (mut best, mut bd) = (table[0].0.clone(), f64::INFINITY);
    for (name, c) in table {
        let c = [c[0] as f64, c[1] as f64, c[2] as f64];
        let rm = (r[0] + c[0]) / 2.0;
        let (dr, dg, db, dc) = (r[0] - c[0], r[1] - c[1], r[2] - c[2], chroma(r) - chroma(c));
        let d = (2.0 + rm / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rm) / 256.0) * db * db + 2.0 * dc * dc;
        if d < bd { bd = d; best = name.clone(); }
    }
    best
}

/// Grid cell size as fractions of the drawing.
pub fn grid_fractions(l: &PaperLayout, grid_cm: f64) -> (f64, f64) {
    (grid_cm * 10.0 / l.draw_w, grid_cm * 10.0 / l.draw_h)
}
