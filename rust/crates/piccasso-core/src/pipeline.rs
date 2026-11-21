//! End-to-end processing (port of `web/js/core/pipeline.js`).
use crate::analysis::{analyze, AnalyzeOptions, Analysis};
use crate::imgproc::{prepare_image, Rgba};
use crate::project::{Prep, Step};
use crate::render::{render_style, RenderOptions};
use crate::shared::Shared;
use crate::steps::generate_steps;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

pub const STEP_BASES: [&str; 4] = ["light", "outline", "values", "sketch"];

pub fn plan_project(src: &Rgba, prep: &Prep, sh: &Shared) -> (Analysis, Vec<Step>) {
    let level = sh.level(&prep.difficulty);
    let small = prepare_image(src, &prep.image(), sh.difficulty.analysis_max_dim);
    let a = analyze(&small, level, &AnalyzeOptions { excluded: prep.excluded_objects.clone(), isolate: prep.isolate_object.clone() });
    let steps = generate_steps(&a, prep, sh);
    (a, steps)
}

pub fn keep_boxes_for(a: Option<&Analysis>, prep: &Prep) -> Option<Vec<[f64; 4]>> {
    let a = a?;
    if prep.isolate_object.is_none() && prep.excluded_objects.is_empty() {
        return None;
    }
    let boxes: Vec<[f64; 4]> = a.objects.iter().filter(|o| a.active_objects.contains(&o.id)).map(|o| o.bbox).collect();
    if boxes.is_empty() { None } else { Some(boxes) }
}

pub fn render_for(prepared: &Rgba, prep: &Prep, sh: &Shared, style: &str, analysis: Option<&Analysis>) -> Result<Rgba, String> {
    render_style(prepared, style, &RenderOptions {
        level: sh.level(&prep.difficulty), analysis_max_dim: sh.difficulty.analysis_max_dim,
        line_thickness: prep.line_thickness, sketch_contrast: prep.sketch_contrast,
        simplify_background: prep.simplify_background, keep_boxes: keep_boxes_for(analysis, prep),
    })
}

pub struct PipelineResult {
    pub analysis: Analysis,
    pub steps: Vec<Step>,
    pub prepared: Rgba,
    pub renders: BTreeMap<String, Rgba>,
}

/// Full run with progress reporting and cooperative cancellation.
pub fn run_pipeline(src: &Rgba, prep: &Prep, sh: &Shared, render_max_dim: usize, progress: &dyn Fn(f32, &str), cancel: &AtomicBool) -> Result<PipelineResult, String> {
    let check = || if cancel.load(Ordering::Relaxed) { Err("cancelled".to_string()) } else { Ok(()) };
    progress(0.05, "Preparing image");
    let prepared = prepare_image(src, &prep.image(), render_max_dim);
    check()?;
    progress(0.15, "Finding the main shapes");
    let (analysis, steps) = plan_project(src, prep, sh);
    check()?;
    let mut wanted = vec![prep.style.clone(), "original".to_string()];
    wanted.extend(STEP_BASES.iter().map(|s| s.to_string()));
    wanted.dedup();
    let mut renders = BTreeMap::new();
    let total = wanted.len();
    for (i, style) in wanted.into_iter().enumerate() {
        if renders.contains_key(&style) { continue; }
        progress(0.3 + 0.65 * i as f32 / total as f32, &format!("Drawing the {} guide", style.replace('_', " ")));
        let r = render_for(&prepared, prep, sh, &style, Some(&analysis))?;
        renders.insert(style, r);
        check()?;
    }
    progress(1.0, "Done");
    Ok(PipelineResult { analysis, steps, prepared, renders })
}
