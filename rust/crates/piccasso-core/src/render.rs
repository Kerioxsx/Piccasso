//! Drawing-style renderers — port of `web/js/core/render.js`.
use crate::imgproc::*;
use crate::shared::Level;

const INK: u8 = 25;

pub struct RenderOptions<'a> {
    pub level: &'a Level,
    pub analysis_max_dim: usize,
    pub line_thickness: f64,
    pub sketch_contrast: f64,
    pub simplify_background: bool,
    pub keep_boxes: Option<Vec<[f64; 4]>>,
}

pub fn pencil_gray(g: &Gray, contrast: f64, strength: f64) -> Gray {
    let (w, h) = (g.width, g.height);
    let inv = Gray { width: w, height: h, data: g.data.iter().map(|&v| 255 - v).collect() };
    let bl = blur2(&inv, (js_round(w.max(h) as f64 / 60.0) as usize).max(2));
    let k = (1.0 + contrast / 50.0).max(0.0) * strength;
    let mut out = Gray::new(w, h);
    for i in 0..out.data.len() {
        let b = bl.data[i] as u32;
        let s = if b >= 255 { 255 } else { ((g.data[i] as u32 * 255) / (255 - b)).min(255) };
        let tone = 255 - ((255 - g.data[i] as u32) * 45) / 100;
        let st = (s * tone) / 255;
        out.data[i] = 255 - js_round((255.0 - st as f64) * k).min(255.0) as u8;
    }
    out
}

/// Subject mask at render resolution (None when the image is low-contrast).
pub fn foreground_mask(img: &Rgba, level: &Level, scale: f64) -> Option<Mask> {
    let (w, h) = (img.width, img.height);
    let n = w * h;
    let fg = foreground_from_border(img, (js_round(level.blur_radius as f64 * scale) as usize).max(1));
    if fg.separation < 18.0 {
        return None;
    }
    let m = clean_mask(&fg.mask, w, h);
    let (labels, comps) = label_components(&m, w, h);
    let min_area = 4u64.max((level.min_area_frac * n as f64).floor() as u64);
    let mut keep = vec![false; comps.len() + 1];
    let mut any = false;
    for c in &comps {
        if c.area >= min_area && !(c.borders >= 3 && c.area as f64 > 0.5 * n as f64) {
            keep[c.label as usize] = true;
            any = true;
        }
    }
    if !any {
        return None;
    }
    Some(labels.iter().map(|&l| keep[l as usize] as u8).collect())
}

fn boundary(mask: &[u8], w: usize, h: usize) -> Mask {
    let mut out = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if mask[i] == 0 {
                continue;
            }
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 || mask[i - 1] == 0 || mask[i + 1] == 0 || mask[i - w] == 0 || mask[i + w] == 0 {
                out[i] = 1;
            }
        }
    }
    out
}

fn mask_to_ink(mask: &[u8], w: usize, h: usize) -> Rgba {
    let mut out = Rgba::new(w, h);
    for (i, &m) in mask.iter().enumerate() {
        let v = if m != 0 { INK } else { 255 };
        out.data[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, 255]);
    }
    out
}

fn level_edges(lv: &[u8], w: usize, h: usize) -> Mask {
    let mut m = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if (x < w - 1 && lv[i] != lv[i + 1]) || (y < h - 1 && lv[i] != lv[i + w]) {
                m[i] = 1;
            }
        }
    }
    m
}

/// Render a preparation style or step base (`light|outline|values|sketch|none`).
pub fn render_style(img: &Rgba, style: &str, opt: &RenderOptions) -> Result<Rgba, String> {
    let level = opt.level;
    let (w, h) = (img.width, img.height);
    let n = w * h;
    let scale = w.max(h) as f64 / opt.analysis_max_dim as f64;
    let rs = |r: f64| (js_round(r * scale) as usize).max(1);
    let thick = (opt.line_thickness as i64 - 1).max(0) as usize;
    let contrast = opt.sketch_contrast;
    let g = to_gray(img);
    let out = match style {
        "none" => Rgba::filled(w, h, 255),
        "original" => img.clone(),
        "pencil_sketch" | "sketch" => gray_to_rgba(&pencil_gray(&g, contrast, 1.0)),
        "light_sketch" | "light" => gray_to_rgba(&pencil_gray(&g, contrast, 0.4)),
        "basic_shapes" => gray_to_rgba(&pencil_gray(&g, contrast, 0.25)),
        "clean_outline" | "outline" => {
            let e = color_sobel(img, rs(level.blur_radius as f64));
            let t = percentile_threshold(&e, level.edge_fraction, level.min_edge);
            let m: Mask = e.data.iter().map(|&v| (v as i32 >= t) as u8).collect();
            mask_to_ink(&dilate(&m, w, h, thick), w, h)
        }
        "simplified_outline" => {
            let lv = posterize_levels(&blur2(&g, rs((level.blur_radius * 2 + 1) as f64)), level.value_levels);
            mask_to_ink(&dilate(&level_edges(&lv, w, h), w, h, thick), w, h)
        }
        "contour_only" => match foreground_mask(img, level, scale) {
            Some(fg) => mask_to_ink(&dilate(&boundary(&fg, w, h), w, h, thick), w, h),
            None => return render_style(img, "clean_outline", opt),
        },
        "shading_guide" | "values" => {
            let l = level.value_levels;
            let lv = posterize_levels(&blur2(&g, rs((level.blur_radius * 2 + 1) as f64)), l);
            let edges = level_edges(&lv, w, h);
            let mut o = Rgba::new(w, h);
            for i in 0..n {
                let mut v = 60 + js_round((lv[i] as f64 * 195.0) / (l - 1) as f64) as i32;
                if edges[i] != 0 {
                    v = (v - 60).max(0);
                }
                o.data[i * 4..i * 4 + 4].copy_from_slice(&[v as u8, v as u8, v as u8, 255]);
            }
            o
        }
        other => return Err(format!("Unknown style: {other}")),
    };
    let mut out = out;
    if style != "none" && (opt.simplify_background || opt.keep_boxes.is_some()) {
        apply_background(&mut out, img, opt, scale, style == "original");
    }
    Ok(out)
}

fn apply_background(out: &mut Rgba, img: &Rgba, opt: &RenderOptions, scale: f64, soft: bool) {
    let (w, h) = (img.width, img.height);
    let n = w * h;
    let mut keep = vec![1u8; n];
    if opt.simplify_background {
        if let Some(fg) = foreground_mask(img, opt.level, scale) {
            keep = dilate(&fg, w, h, (js_round(w.max(h) as f64 * 0.02) as usize).max(1));
        }
    }
    if let Some(boxes) = &opt.keep_boxes {
        if !boxes.is_empty() {
            let mut bm = vec![0u8; n];
            for b in boxes {
                let ax = ((b[0] - 0.03) * w as f64).floor().max(0.0) as usize;
                let ay = ((b[1] - 0.03) * h as f64).floor().max(0.0) as usize;
                let bx = ((b[2] + 0.03) * w as f64).ceil().min(w as f64) as usize;
                let by = ((b[3] + 0.03) * h as f64).ceil().min(h as f64) as usize;
                for y in ay..by {
                    for x in ax..bx {
                        bm[y * w + x] = 1;
                    }
                }
            }
            for i in 0..n {
                keep[i] &= bm[i];
            }
        }
    }
    for i in 0..n {
        if keep[i] != 0 {
            continue;
        }
        for c in 0..3 {
            let j = i * 4 + c;
            out.data[j] = if soft { js_round(out.data[j] as f64 * 0.15 + 255.0 * 0.85) as u8 } else { 255 };
        }
    }
}
