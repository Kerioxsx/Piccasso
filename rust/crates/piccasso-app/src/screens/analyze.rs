//! Analyze my drawing (desktop): value histograms with plain advice, symmetry
//! heat map, matching grids and a colour/value sampler. No AI.
use crate::app::PiccassoApp;
use egui::{pos2, Color32, RichText, Stroke};
use piccasso_core::imgproc::{fit_dims, resize_box, Rgba};
use piccasso_core::inspect::{compare_values, sample, symmetry, value_stats};

#[derive(PartialEq, Clone, Copy)]
enum Tool { Values, Symmetry, Grid, Sample }

pub struct AnalyzeState { reference: Option<Rgba>, drawing: Option<Rgba>, tool: Tool, axis: f64, sym_drawing: bool, grid: usize, sampled: Option<(String, u8, &'static str)>, loaded: bool }
impl Default for AnalyzeState { fn default() -> Self { AnalyzeState { reference: None, drawing: None, tool: Tool::Values, axis: 0.5, sym_drawing: true, grid: 4, sampled: None, loaded: false } } }

fn fit(i: &Rgba) -> Rgba { let (w, h) = fit_dims(i.width, i.height, 900); if w == i.width { i.clone() } else { resize_box(i, w, h) } }
fn pick() -> Option<Result<Rgba, String>> {
    rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file().map(|p| std::fs::read(&p).map_err(|e| e.to_string()).and_then(|b| piccasso_core::decode_image_bytes(&b)).map(|i| fit(&i)))
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let mut err = None;
    let st = &mut app.screen_state.analyze;
    if !st.loaded { st.reference = app.session.as_ref().map(|s| fit(&s.src)); st.loaded = true; app.textures.remove("an-ref"); }
    egui::TopBottomPanel::top("an-top").show(ctx, |ui| {
        ui.label(RichText::new("Analyze my drawing").size(24.0).strong());
        ui.label(RichText::new("Check values, symmetry and proportions of your drawing against the reference. Everything is measured, nothing is guessed: no AI.").weak());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Reference: open picture…").clicked() { match pick() { Some(Ok(i)) => { st.reference = Some(i); app.textures.remove("an-ref"); } Some(Err(e)) => err = Some(e), None => {} } }
            if ui.button(RichText::new("Photo of my drawing…").strong()).clicked() { match pick() { Some(Ok(i)) => { st.drawing = Some(i); app.textures.remove("an-drw"); } Some(Err(e)) => err = Some(e), None => {} } }
            ui.separator();
            for (t, l) in [(Tool::Values, "Values"), (Tool::Symmetry, "Symmetry"), (Tool::Grid, "Grid"), (Tool::Sample, "Colour sampler")] { ui.selectable_value(&mut st.tool, t, l); }
        });
        match st.tool {
            Tool::Symmetry => { ui.horizontal(|ui| { ui.selectable_value(&mut st.sym_drawing, true, "My drawing"); ui.selectable_value(&mut st.sym_drawing, false, "Reference"); ui.add(egui::Slider::new(&mut st.axis, 0.2..=0.8).text("Mirror line")); }); }
            Tool::Grid => { ui.add(egui::Slider::new(&mut st.grid, 2..=10).text("Grid cells per side")); }
            Tool::Sample => { ui.label(match &st.sampled { Some((hex, v, w)) => format!("{w}: {hex} · value {v} % (0 = black, 100 = white)"), None => "Click either picture to read its colour and value.".into() }); }
            Tool::Values => {
                if let (Some(r), Some(d)) = (&st.reference, &st.drawing) {
                    let (sr, sd) = (value_stats(r), value_stats(d));
                    ui.label(format!("Reference: brightness {} % · contrast {} · darkest {} · lightest {}     Drawing: brightness {} % · contrast {} · darkest {} · lightest {}",
                        (sr.mean / 2.55).round(), sr.contrast.round(), sr.dark, sr.light, (sd.mean / 2.55).round(), sd.contrast.round(), sd.dark, sd.light));
                    for (lvl, t) in compare_values(&sr, &sd) { ui.label(if lvl == "big" { RichText::new(format!("• {t}")).strong() } else if lvl == "good" { RichText::new(format!("• {t}")).color(crate::ui::ACCENT) } else { RichText::new(format!("• {t}")) }); }
                } else { ui.label(RichText::new("Add both pictures to get advice.").weak()); }
            }
        }
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let half = area.width() / 2.0;
        for (k, (key, label)) in [("an-ref", "Reference"), ("an-drw", "My drawing")].into_iter().enumerate() {
            let slot = egui::Rect::from_min_size(pos2(area.min.x + half * k as f32, area.min.y + 20.0), egui::vec2(half, area.height() - 20.0));
            ui.painter().text(pos2(slot.min.x + 8.0, area.min.y + 2.0), egui::Align2::LEFT_TOP, label, egui::FontId::proportional(14.0), Color32::GRAY);
            let img = if k == 0 { st.reference.clone() } else { st.drawing.clone() };
            let Some(img) = img else { continue };
            let r = crate::ui::compute_rect(slot, img.width as f32, img.height as f32, &Default::default(), 8.0);
            let tex = app.textures.get(ctx, key, &img);
            let resp = ui.allocate_rect(r, egui::Sense::click());
            let p = ui.painter_at(slot);
            p.image(tex.id(), r, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if st.tool == Tool::Values {
                // histogram strip under the picture
                let hs = value_stats(&img).hist;
                let mx = hs[2..254].iter().copied().max().unwrap_or(1).max(1) as f32;
                let base = r.max.y - 4.0;
                let pts: Vec<egui::Pos2> = (0..256usize).map(|v| { let s: u64 = (v.saturating_sub(4)..(v + 5).min(256)).map(|j| hs[j]).sum::<u64>() / 9; pos2(r.min.x + r.width() * v as f32 / 255.0, base - (s as f32 / mx).min(1.0) * r.height() * 0.25) }).collect();
                p.add(egui::Shape::line(pts, Stroke::new(2.0_f32, if k == 0 { Color32::from_gray(90) } else { crate::ui::ACCENT })));
            }
            if st.tool == Tool::Grid {
                for i in 1..st.grid { let f = i as f32 / st.grid as f32; p.line_segment([pos2(r.min.x + r.width() * f, r.min.y), pos2(r.min.x + r.width() * f, r.max.y)], Stroke::new(1.4_f32, Color32::from_rgb(208, 52, 44))); p.line_segment([pos2(r.min.x, r.min.y + r.height() * f), pos2(r.max.x, r.min.y + r.height() * f)], Stroke::new(1.4_f32, Color32::from_rgb(208, 52, 44))); }
            }
            if st.tool == Tool::Symmetry && (k == 1) == st.sym_drawing {
                let (score, diff, w, h) = symmetry(&img, st.axis, 200);
                let (cw, chh) = (r.width() / w as f32, r.height() / h as f32);
                for y in 0..h { for x in 0..w { let d = diff[y * w + x]; if d > 25 { p.rect_filled(egui::Rect::from_min_size(pos2(r.min.x + x as f32 * cw, r.min.y + y as f32 * chh), egui::vec2(cw + 0.5, chh + 0.5)), 0.0, Color32::from_rgba_unmultiplied(230, 40, 40, (d as u32 * 2).min(200) as u8)); } } }
                let ax = r.min.x + r.width() * st.axis as f32;
                p.add(egui::Shape::dashed_line(&[pos2(ax, r.min.y), pos2(ax, r.max.y)], Stroke::new(2.0_f32, crate::ui::ACCENT), 8.0, 6.0));
                p.text(r.min + egui::vec2(8.0, 8.0), egui::Align2::LEFT_TOP, format!("Symmetry: {} %", (score * 100.0).round()), egui::FontId::proportional(16.0), Color32::BLACK);
            }
            if st.tool == Tool::Sample && resp.clicked() {
                if let Some(q) = resp.interact_pointer_pos() {
                    let (x, y) = (((q.x - r.min.x) / r.width() * img.width as f32) as usize, ((q.y - r.min.y) / r.height() * img.height as f32) as usize);
                    let (hex, v) = sample(&img, x, y);
                    st.sampled = Some((hex, v, if k == 0 { "Reference" } else { "Drawing" }));
                }
            }
        }
    });
    if let Some(e) = err { app.error(format!("Couldn't open the picture: {e}")); }
}
