//! Exports: guide PNG, printable HTML step sheet, portable project file.
use crate::app::DrawStepApp;
use base64::Engine;
use drawstep_core::imgproc::{fit_dims, resize_box, Rgba};
use drawstep_core::project::Project;
use drawstep_core::raster;

fn safe(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == ' ' { c } else { '-' }).collect();
    let s = s.trim().replace(' ', "-");
    if s.is_empty() { "drawing".into() } else { s.chars().take(60).collect() }
}

/// Raster base + vector layers for a step, at up to `max_dim`.
pub fn step_image(app: &DrawStepApp, step_index: usize, max_dim: usize) -> Option<Rgba> {
    let sess = app.session.as_ref()?;
    let step = sess.project.steps.get(step_index)?;
    let reference = sess.renders.get("original")?;
    let (w, h) = fit_dims(reference.width, reference.height, max_dim);
    let mut img = match sess.renders.get(&step.base) {
        Some(b) if step.base != "none" => resize_box(b, w, h),
        _ => Rgba::filled(w, h, 255),
    };
    if let Some(a) = &sess.project.analysis {
        let grid = a.aspect.is_finite().then(|| crate::ui::grid_for(&sess.project)).flatten();
        raster::draw_layers_grid(&mut img, a, &step.layers, grid);
    }
    Some(img)
}

pub fn export_png(app: &mut DrawStepApp) {
    let Some(sess) = app.session.as_ref() else { return };
    let i = sess.project.progress.current_step;
    let name = format!("{}-step-{}.png", safe(&sess.project.name), i + 1);
    let Some(img) = step_image(app, i, 2048) else { app.error("Guide images are not ready yet."); return };
    let Some(path) = rfd::FileDialog::new().set_file_name(&name).add_filter("PNG image", &["png"]).save_file() else { return };
    match drawstep_core::encode_png(&img).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
        Ok(()) => app.toast(format!("Saved {}", path.display())),
        Err(e) => app.error(format!("Could not save: {e}")),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn export_step_sheet(app: &mut DrawStepApp) {
    let Some(sess) = app.session.as_ref() else { return };
    let p = &sess.project;
    let mut items = String::new();
    for (i, s) in p.steps.iter().enumerate() {
        let img = step_image(app, i, 520).and_then(|im| drawstep_core::encode_png(&im).ok()).map(|b| base64::engine::general_purpose::STANDARD.encode(b)).unwrap_or_default();
        items.push_str(&format!("<section><img src=\"data:image/png;base64,{img}\" alt=\"Guide for step {}\"><div><h2>{}. {}</h2><p>{}</p><p class=\"tip\"><b>Tip:</b> {}</p></div></section>\n", i + 1, i + 1, esc(&s.title), esc(&s.instruction), esc(&s.tip)));
    }
    let html = format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>{name} – DrawStep step sheet</title><style>body{{font:15px/1.5 system-ui,sans-serif;color:#222;max-width:900px;margin:24px auto;padding:0 16px}}h1{{font-family:Georgia,serif}}section{{display:grid;grid-template-columns:260px 1fr;gap:16px;align-items:start;padding:14px 0;border-top:1px solid #ddd;break-inside:avoid}}img{{width:100%;border:1px solid #ccc;border-radius:8px}}h2{{font-size:1.05rem;margin:0 0 6px}}.tip{{color:#555}}button{{font:inherit;padding:10px 16px}}@media print{{button{{display:none}}}}@media(max-width:600px){{section{{grid-template-columns:1fr}}}}</style></head><body><button onclick=\"print()\">Print / Save as PDF</button><h1>{name}</h1><p>{n} steps · difficulty: {d} · made with DrawStep</p>{items}</body></html>",
        name = esc(&p.name), n = p.steps.len(), d = p.prep.difficulty.replace('_', " "));
    let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}-steps.html", safe(&p.name))).add_filter("HTML", &["html"]).save_file() else { return };
    match std::fs::write(&path, html) {
        Ok(()) => app.toast(format!("Saved {}. Open it in a browser to print.", path.display())),
        Err(e) => app.error(format!("Could not save: {e}")),
    }
}

pub fn export_project_file(app: &mut DrawStepApp, p: &Project) {
    let Some(path) = rfd::FileDialog::new().set_file_name(format!("{}.drawstep.json", safe(&p.name))).add_filter("DrawStep project", &["json"]).save_file() else { return };
    match std::fs::write(&path, p.to_json()) {
        Ok(()) => app.toast(format!("Saved {}", path.display())),
        Err(e) => app.error(format!("Could not save: {e}")),
    }
}
