//! Studio (desktop): turn a picture into a drawing reference with the classic
//! conversions in piccasso_core::filters. Same tools as the web Studio.
use crate::app::PiccassoApp;
use egui::{pos2, vec2, Color32, RichText, Stroke};
use piccasso_core::filters::{in_region, pre_adjust, run, upscale, CONVERSIONS};
use piccasso_core::imgproc::{fit_dims, resize_box, Rgba};
use std::collections::HashMap;

pub struct StudioState {
    src: Option<Rgba>,
    key: &'static str,
    params: HashMap<&'static str, Vec<f64>>,
    brightness: f64,
    contrast: f64,
    mirror: bool,
    region: Option<(usize, usize, usize, usize)>,
    selecting: bool,
    drag_from: Option<(usize, usize)>,
    result: Option<Rgba>,
    dirty: bool,
    millis: u128,
}
impl Default for StudioState {
    fn default() -> Self {
        StudioState { src: None, key: "sketch", params: HashMap::new(), brightness: 0.0, contrast: 0.0, mirror: false, region: None, selecting: false, drag_from: None, result: None, dirty: true, millis: 0 }
    }
}

const WORK_MAX: usize = 1200;

fn fit(img: &Rgba) -> Rgba { let (w, h) = fit_dims(img.width, img.height, WORK_MAX); if w == img.width { img.clone() } else { resize_box(img, w, h) } }

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let st = &mut app.screen_state.studio;
    if st.src.is_none() { if let Some(s) = app.session.as_ref() { st.src = Some(fit(&s.src)); st.dirty = true; } }
    if st.dirty {
        if let Some(src) = &st.src {
            let t0 = std::time::Instant::now();
            let base = pre_adjust(src, st.brightness, st.contrast, st.mirror);
            let key = st.key;
            let p = st.params.entry(key).or_insert_with(|| CONVERSIONS.iter().find(|c| c.key == key).map(|c| c.params.iter().map(|q| q.3).collect()).unwrap_or_default()).clone();
            st.result = Some(in_region(&base, st.region, |i| run(key, i, &p)));
            st.millis = t0.elapsed().as_millis();
            app.textures.remove("studio-result");
        }
        st.dirty = false;
    }
    let mut save: Option<f64> = None;
    let mut err: Option<String> = None;
    egui::SidePanel::left("studio-side").resizable(true).default_width(330.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Studio").size(24.0).strong());
            ui.label(RichText::new("Turn a picture into a drawing reference. Classic image processing on this computer: no AI.").weak());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Open a picture…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| piccasso_core::decode_image_bytes(&b)) {
                            Ok(img) => { st.src = Some(fit(&img)); st.region = None; st.dirty = true; }
                            Err(e) => err = Some(format!("Couldn't open the picture: {e}")),
                        }
                    }
                }
                if let Some(s) = app.session.as_ref() { if ui.button("Use current project").clicked() { st.src = Some(fit(&s.src)); st.region = None; st.dirty = true; } }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for c in CONVERSIONS { if ui.selectable_label(st.key == c.key, c.label).clicked() { st.key = c.key; st.dirty = true; } }
            });
            if let Some(c) = CONVERSIONS.iter().find(|c| c.key == st.key) {
                let p = st.params.entry(c.key).or_insert_with(|| c.params.iter().map(|q| q.3).collect());
                for (i, (name, lo, hi, def)) in c.params.iter().enumerate() {
                    let int = lo.fract() == 0.0 && hi.fract() == 0.0 && def.fract() == 0.0;
                    let mut s = egui::Slider::new(&mut p[i], *lo..=*hi).text(*name);
                    if int { s = s.step_by(1.0); }
                    if ui.add(s).changed() { st.dirty = true; }
                }
            }
            ui.separator();
            ui.strong("Adjust the picture first");
            if ui.add(egui::Slider::new(&mut st.brightness, -100.0..=100.0).text("Brightness")).changed() { st.dirty = true; }
            if ui.add(egui::Slider::new(&mut st.contrast, -100.0..=100.0).text("Contrast")).changed() { st.dirty = true; }
            if ui.checkbox(&mut st.mirror, "Mirror (flip left-right)").changed() { st.dirty = true; }
            ui.horizontal(|ui| {
                ui.toggle_value(&mut st.selecting, "Convert only a region (drag)");
                if ui.button("Whole picture").clicked() { st.region = None; st.dirty = true; }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save PNG…").clicked() { save = Some(1.0); }
                if ui.button("Save 2× (sharper print)…").clicked() { save = Some(2.0); }
            });
            if let Some(r) = &st.result { ui.label(RichText::new(format!("{} × {} px · {} ms", r.width, r.height, st.millis)).small().weak()); }
        });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let Some(res) = st.result.clone() else { ui.centered_and_justified(|ui| ui.label("Open a picture to start.")); return; };
        let rect = crate::ui::compute_rect(area, res.width as f32, res.height as f32, &Default::default(), 12.0);
        let tex = app.textures.get(ctx, "studio-result", &res);
        let resp = ui.allocate_rect(rect, egui::Sense::drag());
        ui.painter().image(tex.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let to_img = |p: egui::Pos2| (((p.x - rect.min.x) / rect.width() * res.width as f32).clamp(0.0, res.width as f32) as usize, ((p.y - rect.min.y) / rect.height() * res.height as f32).clamp(0.0, res.height as f32) as usize);
        if st.selecting {
            if resp.drag_started() { st.drag_from = resp.interact_pointer_pos().map(to_img); }
            if let (Some(a), Some(p)) = (st.drag_from, resp.interact_pointer_pos()) {
                let b = to_img(p);
                let r = (a.0.min(b.0), a.1.min(b.1), a.0.abs_diff(b.0), a.1.abs_diff(b.1));
                let k = rect.width() / res.width as f32;
                ui.painter().rect_stroke(egui::Rect::from_min_size(rect.min + vec2(r.0 as f32 * k, r.1 as f32 * k), vec2(r.2 as f32 * k, r.3 as f32 * k)), 0.0, Stroke::new(2.0_f32, crate::ui::ACCENT), egui::StrokeKind::Middle);
                if resp.drag_stopped() { if r.2 > 4 && r.3 > 4 { st.region = Some(r); st.dirty = true; } st.drag_from = None; }
            }
        }
    });
    if let Some(e) = err { app.error(e); }
    let st = &mut app.screen_state.studio;
    if let (Some(f), Some(res)) = (save, st.result.clone()) {
        let key = st.key;
        let img = if f > 1.0 { upscale(&res, f) } else { res };
        if let Some(path) = rfd::FileDialog::new().set_file_name(format!("piccasso-{}{}.png", key, if f > 1.0 { "-2x" } else { "" })).add_filter("PNG image", &["png"]).save_file() {
            match piccasso_core::encode_png(&img).map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
                Ok(()) => app.toast(format!("Saved {}", path.display())),
                Err(e) => app.error(format!("Couldn't save: {e}")),
            }
        }
    }
}
