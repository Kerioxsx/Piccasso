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
    let preset = presets.iter().find(|p| p.id == cfg.preset).unwrap_or(&presets[0]);
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
        if kind == "contours" && a.silhouettes.is_empty() {
            base = "outline".into();
            layers = vec!["shapes".into()];
        }
        Step {
            id: format!("step-{}-{}", i + 1, kind), kind: kind.clone(), title: t.title.clone(), instruction, tip: t.tip.clone(),
            checks: t.checks.iter().map(|c| fill(c, &v)).collect(), base, layers,
            terms: sh.glossary.step_terms.get(kind).cloned().unwrap_or_default(),
            ..Default::default()
        }
    }).collect()
}
