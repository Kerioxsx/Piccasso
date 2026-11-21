//! On-device drawing comparison: photo → confirm paper corners → flatten →
//! compare pencil lines with the guide. The photo never leaves this computer.
use crate::app::{PiccassoApp, Screen};
use crate::ui::{compute_rect, draw_layers, primary};
use piccasso_core::compare::{compare_drawing, guide_mask, Metrics};
use piccasso_core::geometry::{detect_paper, order_corners, warp_gray};
use piccasso_core::imgproc::{fit_dims, gray_to_rgba, resize_box, to_gray, Rgba};
use piccasso_core::lock::View;
use piccasso_core::shared::shared;
use piccasso_core::steps::paper_layout;
use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};

#[derive(Default)]
pub struct CompareState {
    photo: Option<Rgba>,
    corners: [[f64; 2]; 4],
    auto: bool,
    drag: Option<usize>,
    result: Option<(Vec<String>, Option<Metrics>, Rgba)>,
    side: bool,
    alpha: f32,
}

const UV: Rect = Rect { min: egui::Pos2 { x: 0.0, y: 0.0 }, max: egui::Pos2 { x: 1.0, y: 1.0 } };

fn load(st: &mut CompareState, img: Rgba) {
    let (w, h) = fit_dims(img.width, img.height, 1600);
    let img = resize_box(&img, w, h);
    let (sw, sh) = fit_dims(w, h, 240);
    let det = detect_paper(&to_gray(&resize_box(&img, sw, sh)));
    st.auto = det.as_ref().map_or(false, |d| d.confidence > 0.3);
    st.corners = det.filter(|d| d.confidence > 0.3).map(|d| d.corners).unwrap_or([[0.1, 0.1], [0.9, 0.1], [0.9, 0.9], [0.1, 0.9]]);
    st.photo = Some(img);
    st.result = None;
    st.alpha = 0.6;
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.compare;
    let mut back = false;
    let mut err: Option<String> = None;
    let Some(analysis) = sess.project.analysis.clone() else { return };
    let layout = paper_layout(&sess.project.prep.paper, &shared().paper.presets, analysis.aspect, shared().paper.default_margin_mm);
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            if ui.button("◀ Back to steps").clicked() { back = true; }
            crate::ui::heading(ui, "Check your drawing");
        });
        if st.photo.is_none() {
            ui.label("Take a photo of your paper and Piccasso will point out a few things to check, such as size and placement. It's friendly feedback about structure, not a score.");
            ui.label(RichText::new("🔒 Your photo stays on this computer. It is not uploaded or saved.").strong());
            ui.label(RichText::new("Tips: photograph straight from above, keep all four corners visible, and put the paper on a darker surface.").weak());
            ui.horizontal(|ui| {
                if primary(ui, "Choose a photo").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| piccasso_core::decode_image_bytes(&b)) {
                            Ok(img) => { load(st, img); app.textures.remove("cmp-photo"); }
                            Err(e) => err = Some(e),
                        }
                    }
                }
                #[cfg(feature = "camera")]
                if ui.button("📷 Use the webcam").clicked() {
                    let feed = crate::camera::CameraFeed::start();
                    let t0 = std::time::Instant::now();
                    loop {
                        if let Some((n, img)) = feed.latest() { if n > 5 { load(st, img); app.textures.remove("cmp-photo"); break; } }
                        if let Some(e) = feed.error() { err = Some(format!("Camera: {e}")); break; }
                        if t0.elapsed().as_secs() > 5 { err = Some("The camera did not deliver a picture in time.".into()); break; }
                        std::thread::sleep(std::time::Duration::from_millis(30));
                    }
                }
            });
            return;
        }
        let photo = st.photo.clone().unwrap();
        if let Some((obs, metrics, flat)) = st.result.clone() {
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Things to look at");
                for o in &obs { ui.label(format!("• {o}")); }
                if let Some(m) = &metrics { ui.label(RichText::new(format!("Line overlap with the guide: about {}%. This is only a rough guide; small differences are completely normal.", (m.coverage * 100.0).round())).small().weak()); }
            });
            ui.horizontal(|ui| {
                ui.selectable_value(&mut st.side, false, "Guide on top");
                ui.selectable_value(&mut st.side, true, "Side by side");
                ui.add(egui::Slider::new(&mut st.alpha, 0.0..=1.0).text("Guide strength"));
                if ui.button("Try another photo").clicked() { st.photo = None; st.result = None; }
            });
            let area = ui.available_rect_before_wrap();
            let tex = app.textures.get(ctx, "cmp-flat", &flat);
            let (fw, fh) = (flat.width as f32, flat.height as f32);
            let draw_area = |r: Rect| Rect::from_min_size(pos2(r.min.x + (layout.draw_x / layout.paper_w) as f32 * r.width(), r.min.y + (layout.draw_y / layout.paper_h) as f32 * r.height()), vec2((layout.draw_w / layout.paper_w) as f32 * r.width(), (layout.draw_h / layout.paper_h) as f32 * r.height()));
            let painter = ui.painter_at(area);
            if st.side {
                let left = compute_rect(Rect::from_min_max(area.min, pos2(area.center().x - 6.0, area.max.y)), fw, fh, &View::default(), 8.0);
                let right = compute_rect(Rect::from_min_max(pos2(area.center().x + 6.0, area.min.y), area.max), fw, fh, &View::default(), 8.0);
                painter.image(tex.id(), left, UV, Color32::WHITE);
                painter.rect_filled(right, 0.0, Color32::WHITE);
                draw_layers(&painter, &analysis, &["silhouette".into(), "internal".into()], draw_area(right), 1.0, false);
            } else {
                let r = compute_rect(area, fw, fh, &View::default(), 8.0);
                painter.image(tex.id(), r, UV, Color32::WHITE);
                draw_layers(&painter, &analysis, &["silhouette".into(), "internal".into()], draw_area(r), st.alpha, false);
            }
            return;
        }
        ui.label(if st.auto { "We found the paper. Check that the four round handles sit on the paper's corners." } else { "Drag the four round handles onto the corners of your paper." });
        ui.horizontal(|ui| {
            if primary(ui, "✔ Compare").clicked() {
                let corners = order_corners(&st.corners);
                let l = 600usize;
                let (w, h) = if layout.paper_w >= layout.paper_h { (l, (l as f64 * layout.paper_h / layout.paper_w).round() as usize) } else { ((l as f64 * layout.paper_w / layout.paper_h).round() as usize, l) };
                let flat = warp_gray(&to_gray(&photo), &corners, w, h);
                let guide = guide_mask(&analysis, &layout, w, h);
                let res = compare_drawing(&flat, &guide, &shared().strings);
                app.textures.remove("cmp-flat");
                st.result = Some((res.observations, res.metrics, gray_to_rgba(&flat)));
            }
            if ui.button("Different photo").clicked() { st.photo = None; }
        });
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, Sense::drag());
        let r = compute_rect(area, photo.width as f32, photo.height as f32, &View::default(), 8.0);
        let tex = app.textures.get(ctx, "cmp-photo", &photo);
        let painter = ui.painter_at(area);
        painter.image(tex.id(), r, UV, Color32::WHITE);
        let to_s = |p: [f64; 2]| pos2(r.min.x + p[0] as f32 * r.width(), r.min.y + p[1] as f32 * r.height());
        if resp.drag_started() { if let Some(pos) = resp.interact_pointer_pos() { st.drag = st.corners.iter().position(|c| to_s(*c).distance(pos) < 28.0); } }
        if let (Some(i), Some(pos)) = (st.drag, resp.interact_pointer_pos()) {
            st.corners[i] = [(((pos.x - r.min.x) / r.width()) as f64).clamp(0.0, 1.0), (((pos.y - r.min.y) / r.height()) as f64).clamp(0.0, 1.0)];
        }
        if resp.drag_stopped() { st.drag = None; }
        let pts: Vec<_> = st.corners.iter().map(|c| to_s(*c)).collect();
        painter.add(egui::Shape::closed_line(pts.clone(), Stroke::new(3.0_f32, Color32::from_rgb(0x7c, 0xc4, 0xb3))));
        for (i, p) in pts.iter().enumerate() {
            painter.circle(*p, 16.0, crate::ui::ACCENT.gamma_multiply(0.7), Stroke::new(3.0_f32, Color32::WHITE));
            painter.text(*p, egui::Align2::CENTER_CENTER, ["TL", "TR", "BR", "BL"][i], egui::FontId::proportional(12.0), Color32::WHITE);
        }
    });
    if let Some(e) = err { app.error(e); }
    if back { app.screen_state.compare = Default::default(); app.go(Screen::Workspace); }
}
