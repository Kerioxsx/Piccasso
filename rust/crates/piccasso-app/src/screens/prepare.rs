use crate::app::{PiccassoApp, Screen};
use crate::ui::{compute_rect, primary};
use piccasso_core::analysis::Analysis;
use piccasso_core::imgproc::{fit_dims, flip, prepare_image, resize_box, rotate, CropRect, Rgba};
use piccasso_core::lock::View;
use piccasso_core::pipeline::{plan_project, render_for};
use piccasso_core::project::Prep;
use piccasso_core::shared::{shared, DIFFICULTY_ORDER};
use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct PrepareState {
    oriented: Option<(String, Rgba)>,
    preview: Option<Rgba>,
    preview_key: String,
    preview_rx: Option<Receiver<(String, Rgba)>>,
    thumbs: Vec<(String, Rgba)>,
    thumbs_key: String,
    thumbs_rx: Option<Receiver<(String, Vec<(String, Rgba)>)>>,
    tab_style: bool,
    aspect: String,
    objects: Option<Analysis>,
    drag: Option<(u8, CropRect, egui::Pos2)>,
    last_change: Option<Instant>,
}

fn orient_key(p: &Prep) -> String {
    format!("{}{}{}", p.rotation, p.flip_h, p.flip_v)
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let sh = shared();
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.prepare;
    let prep_before = sess.project.prep.clone();
    if st.aspect.is_empty() { st.aspect = "free".into(); }

    // Oriented (rotated/flipped) small source for the crop editor.
    let ok = orient_key(&sess.project.prep);
    if st.oriented.as_ref().map_or(true, |(k, _)| k != &ok) {
        let (w, h) = fit_dims(sess.src.width, sess.src.height, 900);
        let small = resize_box(&sess.src, w, h);
        let o = flip(&rotate(&small, sess.project.prep.rotation), sess.project.prep.flip_h, sess.project.prep.flip_v);
        st.oriented = Some((ok.clone(), o));
        app.textures.remove("prep-oriented");
    }
    // Background preview + thumbnails.
    if let Some(rx) = &st.preview_rx {
        if let Ok((k, img)) = rx.try_recv() { st.preview = Some(img); st.preview_key = k; st.preview_rx = None; app.textures.remove("prep-preview"); }
        ctx.request_repaint_after(Duration::from_millis(60));
    }
    if let Some(rx) = &st.thumbs_rx {
        if let Ok((k, t)) = rx.try_recv() { st.thumbs = t; st.thumbs_key = k; st.thumbs_rx = None; for s in &sh.styles.styles { app.textures.remove(&format!("thumb-{}", s.id)); } }
        ctx.request_repaint_after(Duration::from_millis(60));
    }
    let pk = format!("{}", sess.project.prep.render_hash());
    let settled = st.last_change.map_or(true, |t| t.elapsed() > Duration::from_millis(300));
    if st.tab_style && st.preview_key != pk && st.preview_rx.is_none() && settled {
        let (tx, rx) = channel();
        let (src, prep, a, key) = (sess.src.clone(), sess.project.prep.clone(), st.objects.clone(), pk.clone());
        std::thread::spawn(move || {
            let img = prepare_image(&src, &prep.image(), 560);
            if let Ok(r) = render_for(&img, &prep, shared(), &prep.style, a.as_ref()) { let _ = tx.send((key, r)); }
        });
        st.preview_rx = Some(rx);
    }
    let tk = format!("{}|{}|{}|{}|{}|{:?}", ok, sess.project.prep.difficulty, sess.project.prep.brightness, sess.project.prep.contrast, sess.project.prep.sketch_contrast, sess.project.prep.crop);
    if st.thumbs_key != tk && st.thumbs_rx.is_none() && settled {
        let (tx, rx) = channel();
        let (src, prep, key) = (sess.src.clone(), sess.project.prep.clone(), tk.clone());
        std::thread::spawn(move || {
            let small = prepare_image(&src, &prep.image(), 150);
            let mut p2 = prep.clone();
            p2.line_thickness = (prep.line_thickness - 1.0).max(1.0);
            p2.simplify_background = false;
            let out = shared().styles.styles.iter().filter_map(|s| render_for(&small, &p2, shared(), &s.id, None).ok().map(|r| (s.id.clone(), r))).collect();
            let _ = tx.send((key, out));
        });
        st.thumbs_rx = Some(rx);
    }

    let mut start = false;
    egui::SidePanel::right("prep-controls").resizable(true).default_width(380.0).min_width(300.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let p = &mut sess.project;
            ui.add_space(6.0);
            ui.label("Project name");
            ui.text_edit_singleline(&mut p.name);
            ui.add_space(6.0);
            ui.strong("Drawing style");
            egui::Grid::new("styles").num_columns(3).spacing(vec2(6.0, 6.0)).show(ui, |ui| {
                for (i, s) in sh.styles.styles.iter().enumerate() {
                    ui.vertical(|ui| {
                        let selected = p.prep.style == s.id;
                        let size = vec2(100.0, 74.0);
                        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
                        ui.painter().rect_filled(rect, 6.0, Color32::WHITE);
                        if let Some((_, img)) = st.thumbs.iter().find(|(id, _)| id == &s.id) {
                            let tex = app.textures.get(ctx, &format!("thumb-{}", s.id), img);
                            let r = compute_rect(rect, img.width as f32, img.height as f32, &View::default(), 3.0);
                            ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        ui.painter().rect_stroke(rect, 6.0, Stroke::new(if selected { 3.0_f32 } else { 1.0 }, if selected { crate::ui::ACCENT } else { Color32::GRAY }), egui::StrokeKind::Inside);
                        let resp = resp.on_hover_text(&s.description);
                        if resp.clicked() { p.prep.style = s.id.clone(); st.tab_style = true; }
                        if ui.selectable_label(selected, RichText::new(&s.label).small()).clicked() { p.prep.style = s.id.clone(); st.tab_style = true; }
                    });
                    if i % 3 == 2 { ui.end_row(); }
                }
            });
            if let Some(s) = sh.styles.styles.iter().find(|s| s.id == p.prep.style) { ui.label(RichText::new(&s.description).small().weak()); }
            ui.add_space(6.0);
            ui.strong("Difficulty");
            ui.horizontal_wrapped(|ui| {
                for d in DIFFICULTY_ORDER {
                    ui.selectable_value(&mut p.prep.difficulty, d.to_string(), &sh.level(d).label);
                }
            });
            ui.label(RichText::new("Easier levels use fewer, bigger shapes and fewer steps.").small().weak());
            ui.add(egui::Slider::new(&mut p.prep.brightness, -60.0..=60.0).text("Brightness"));
            ui.add(egui::Slider::new(&mut p.prep.contrast, -60.0..=80.0).text("Contrast"));
            ui.add_space(6.0);
            ui.strong("What will you draw with?");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut p.prep.medium, "graphite".to_string(), "One pencil");
                ui.selectable_value(&mut p.prep.medium, "color".to_string(), "Colours");
            });
            ui.label(RichText::new("Pencil adds pressure and shading guidance; Colours adds a palette and a colouring step.").small().weak());
            ui.add_space(6.0);
            ui.strong("Paper");
            let label = if p.prep.paper.preset == "custom" { "Custom size (cm)".to_string() } else { sh.paper.presets.iter().find(|x| x.id == p.prep.paper.preset).map(|x| format!("{} ({} × {} cm)", x.label, x.width_mm / 10.0, x.height_mm / 10.0)).unwrap_or_default() };
            egui::ComboBox::from_label("Paper size").selected_text(label).show_ui(ui, |ui| {
                for pr in &sh.paper.presets { ui.selectable_value(&mut p.prep.paper.preset, pr.id.clone(), format!("{} ({} × {} cm)", pr.label, pr.width_mm / 10.0, pr.height_mm / 10.0)); }
                ui.selectable_value(&mut p.prep.paper.preset, "custom".to_string(), "Custom size (cm)");
            });
            if p.prep.paper.preset == "custom" {
                ui.horizontal(|ui| {
                    let mut wcm = p.prep.paper.width_mm / 10.0;
                    let mut hcm = p.prep.paper.height_mm / 10.0;
                    ui.label("Width");
                    if ui.add(egui::DragValue::new(&mut wcm).range(5.0..=200.0).speed(0.1).suffix(" cm")).changed() { p.prep.paper.width_mm = (wcm * 10.0).round(); }
                    ui.label("Height");
                    if ui.add(egui::DragValue::new(&mut hcm).range(5.0..=200.0).speed(0.1).suffix(" cm")).changed() { p.prep.paper.height_mm = (hcm * 10.0).round(); }
                });
            }
            ui.horizontal(|ui| {
                for (v, l) in [("auto", "Auto"), ("portrait", "Portrait"), ("landscape", "Landscape")] { ui.selectable_value(&mut p.prep.paper.orientation, v.to_string(), l); }
            });
            ui.add(egui::Slider::new(&mut p.prep.paper.margin_mm, 5.0..=40.0).text("Border (mm)"));
            ui.add(egui::Slider::new(&mut p.prep.paper.grid_cm, 0.0..=5.0).step_by(0.5).text("Grid squares (cm, 0 = off)"));
            {
                let o = &st.oriented.as_ref().unwrap().1;
                let asp = (o.width as f64 * p.prep.crop.w) / (o.height as f64 * p.prep.crop.h);
                let l = piccasso_core::steps::paper_layout(&p.prep.paper, &sh.paper.presets, asp, sh.paper.default_margin_mm);
                let grid = if p.prep.paper.grid_cm > 0.0 { format!(" · grid {} × {} squares", (l.draw_w / (p.prep.paper.grid_cm * 10.0) - 1e-9).ceil(), (l.draw_h / (p.prep.paper.grid_cm * 10.0) - 1e-9).ceil()) } else { String::new() };
                egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
                    ui.label(format!("Paper {:.1} × {:.1} cm ({}) → your drawing: {:.1} × {:.1} cm{grid}", l.paper_w / 10.0, l.paper_h / 10.0, l.orient, l.draw_w / 10.0, l.draw_h / 10.0));
                });
            }
            ui.collapsing("Objects and background", |ui| {
                ui.checkbox(&mut p.prep.simplify_background, "Simplify background (hide busy areas behind the subject)");
                ui.label(RichText::new("Find the separate objects, then remove the ones you don't want to draw, or keep only one.").small().weak());
                if ui.button("Find objects").clicked() {
                    st.objects = Some(plan_project(&sess.src, &p.prep, sh).0);
                    st.tab_style = true;
                }
                if let Some(a) = st.objects.clone() {
                    if a.objects.is_empty() { ui.label("No separate objects found. The whole picture will be used."); }
                    let mut refresh = false;
                    for (i, o) in a.objects.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let mut included = !p.prep.excluded_objects.contains(&o.id);
                            if ui.checkbox(&mut included, format!("Object {}", i + 1)).changed() {
                                if included { p.prep.excluded_objects.retain(|x| x != &o.id); } else { p.prep.excluded_objects.push(o.id.clone()); }
                                p.prep.isolate_object = None;
                                refresh = true;
                            }
                            let iso = p.prep.isolate_object.as_deref() == Some(o.id.as_str());
                            if ui.selectable_label(iso, if iso { "Showing only this" } else { "Only this" }).clicked() {
                                p.prep.isolate_object = if iso { None } else { Some(o.id.clone()) };
                                refresh = true;
                            }
                        });
                    }
                    if refresh { st.objects = Some(plan_project(&sess.src, &p.prep, sh).0); }
                }
            });
            ui.collapsing("Line and sketch settings", |ui| {
                ui.add(egui::Slider::new(&mut p.prep.line_thickness, 1.0..=5.0).step_by(1.0).text("Line thickness"));
                ui.add(egui::Slider::new(&mut p.prep.sketch_contrast, -40.0..=100.0).text("Sketch darkness"));
                if ui.add(egui::Slider::new(&mut p.prep.opacity, 20.0..=100.0).text("Guide opacity %")).changed() { p.view.opacity = p.prep.opacity; }
            });
            ui.add_space(10.0);
            if primary(ui, "Create my drawing steps").clicked() { start = true; }
        });
    });

    egui::CentralPanel::default().show(ctx, |ui| {
        let p = &mut sess.project;
        ui.horizontal(|ui| {
            ui.selectable_value(&mut st.tab_style, false, "Crop & rotate");
            ui.selectable_value(&mut st.tab_style, true, "Style preview");
            ui.separator();
            if ui.button("↺ Rotate left").clicked() { p.prep.rotation = (p.prep.rotation + 270) % 360; p.prep.crop = CropRect::default(); }
            if ui.button("↻ Rotate right").clicked() { p.prep.rotation = (p.prep.rotation + 90) % 360; p.prep.crop = CropRect::default(); }
            if ui.selectable_label(p.prep.flip_h, "Mirror").clicked() { p.prep.flip_h = !p.prep.flip_h; p.prep.crop.x = 1.0 - p.prep.crop.x - p.prep.crop.w; }
            if ui.selectable_label(p.prep.flip_v, "Flip upside down").clicked() { p.prep.flip_v = !p.prep.flip_v; p.prep.crop.y = 1.0 - p.prep.crop.y - p.prep.crop.h; }
            if ui.button("Reset crop").clicked() { p.prep.crop = CropRect::default(); st.aspect = "free".into(); }
        });
        let oriented = &st.oriented.as_ref().unwrap().1;
        let img_aspect = oriented.width as f64 / oriented.height as f64;
        ui.horizontal(|ui| {
            ui.label("Crop shape:");
            for (v, l) in [("free", "Free"), ("original", "Original"), ("1", "Square"), ("0.75", "3:4"), ("1.333", "4:3"), ("paper", "Paper")] {
                if ui.selectable_label(st.aspect == v, l).clicked() {
                    st.aspect = v.into();
                    if let Some(a) = target_aspect(&st.aspect, img_aspect, &p.prep) { p.prep.crop = fit_aspect(p.prep.crop, a, img_aspect); }
                }
            }
        });
        let area = ui.available_rect_before_wrap();
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 8.0, ui.visuals().faint_bg_color);
        if !st.tab_style {
            let tex = app.textures.get(ctx, "prep-oriented", oriented);
            let r = compute_rect(area, oriented.width as f32, oriented.height as f32, &View::default(), 16.0);
            let tint = Color32::from_gray((200.0 * (1.0 + p.prep.brightness as f32 / 200.0)).clamp(60.0, 255.0) as u8);
            painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
            let c = p.prep.crop;
            let cr = Rect::from_min_size(pos2(r.min.x + c.x as f32 * r.width(), r.min.y + c.y as f32 * r.height()), vec2(c.w as f32 * r.width(), c.h as f32 * r.height()));
            let shade = Color32::from_black_alpha(110);
            for s in [Rect::from_min_max(r.min, pos2(r.max.x, cr.min.y)), Rect::from_min_max(pos2(r.min.x, cr.max.y), r.max), Rect::from_min_max(pos2(r.min.x, cr.min.y), pos2(cr.min.x, cr.max.y)), Rect::from_min_max(pos2(cr.max.x, cr.min.y), pos2(r.max.x, cr.max.y))] {
                painter.rect_filled(s, 0.0, shade);
            }
            painter.rect_stroke(cr, 0.0, Stroke::new(2.0_f32, Color32::WHITE), egui::StrokeKind::Middle);
            let corners = [cr.left_top(), cr.right_top(), cr.right_bottom(), cr.left_bottom()];
            for k in corners { painter.circle(k, 9.0, Color32::WHITE, Stroke::new(2.0_f32, crate::ui::ACCENT)); }
            let resp = ui.allocate_rect(area, Sense::drag());
            if resp.drag_started() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let handle = corners.iter().position(|k| k.distance(pos) < 18.0).map(|i| i as u8).unwrap_or(if cr.contains(pos) { 9 } else { 255 });
                    if handle != 255 { st.drag = Some((handle, c, pos)); }
                }
            }
            if let (Some((handle, start_c, start_pos)), Some(pos)) = (st.drag, resp.interact_pointer_pos()) {
                let (dx, dy) = (((pos.x - start_pos.x) / r.width()) as f64, ((pos.y - start_pos.y) / r.height()) as f64);
                p.prep.crop = drag_crop(start_c, handle, dx, dy, target_aspect(&st.aspect, img_aspect, &p.prep), img_aspect);
            }
            if resp.drag_stopped() { st.drag = None; }
        } else if let Some(prev) = &st.preview {
            let tex = app.textures.get(ctx, "prep-preview", prev);
            let r = compute_rect(area, prev.width as f32, prev.height as f32, &View::default(), 16.0);
            painter.rect_filled(r, 0.0, Color32::WHITE);
            painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if p.prep.style == "basic_shapes" {
                let a = st.objects.clone().unwrap_or_else(|| plan_project(&sess.src, &p.prep, sh).0);
                crate::ui::draw_layers(&painter, &a, &["shapes".into()], r, 1.0, true);
                st.objects.get_or_insert(a);
            }
            if let Some(a) = &st.objects {
                for (i, o) in a.objects.iter().enumerate() {
                    let active = a.active_objects.contains(&o.id);
                    let br = Rect::from_min_max(pos2(r.min.x + o.bbox[0] as f32 * r.width(), r.min.y + o.bbox[1] as f32 * r.height()), pos2(r.min.x + o.bbox[2] as f32 * r.width(), r.min.y + o.bbox[3] as f32 * r.height()));
                    let col = if active { crate::ui::ACCENT } else { Color32::GRAY };
                    painter.rect_stroke(br, 0.0, Stroke::new(2.0_f32, col), egui::StrokeKind::Middle);
                    painter.text(br.min + vec2(4.0, 2.0), egui::Align2::LEFT_TOP, format!("{}", i + 1), egui::FontId::proportional(16.0), col);
                }
            }
            if st.preview_rx.is_some() { painter.text(area.left_bottom() + vec2(10.0, -10.0), egui::Align2::LEFT_BOTTOM, "Updating preview…", egui::FontId::proportional(14.0), Color32::GRAY); }
        } else {
            ui.put(area, egui::Spinner::new().size(32.0));
        }
    });

    if sess.project.prep != prep_before {
        sess.dirty = true;
        st.last_change = Some(Instant::now());
        if sess.project.prep.isolate_object != prep_before.isolate_object || sess.project.prep.excluded_objects != prep_before.excluded_objects {
            st.preview_key.clear();
        }
        ctx.request_repaint_after(Duration::from_millis(320));
    }
    if start {
        app.save_project();
        app.start_job(false);
        app.go(Screen::Processing);
    }
}

fn target_aspect(mode: &str, img_aspect: f64, prep: &Prep) -> Option<f64> {
    match mode {
        "free" => None,
        "original" => Some(img_aspect),
        "paper" => {
            let pr = shared().paper.presets.iter().find(|x| x.id == prep.paper.preset).unwrap_or(&shared().paper.presets[0]);
            let land = prep.paper.orientation == "landscape" || (prep.paper.orientation == "auto" && img_aspect > 1.0);
            Some(if land { pr.height_mm / pr.width_mm } else { pr.width_mm / pr.height_mm })
        }
        v => v.parse().ok(),
    }
}

fn fit_aspect(c: CropRect, a: f64, img_aspect: f64) -> CropRect {
    let (cx, cy) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
    let (mut w, mut h) = (c.w, c.h);
    if (w * img_aspect) / h > a { w = (a * h) / img_aspect; } else { h = (w * img_aspect) / a; }
    w = w.min(1.0);
    h = h.min(1.0);
    CropRect { x: (cx - w / 2.0).clamp(0.0, 1.0 - w), y: (cy - h / 2.0).clamp(0.0, 1.0 - h), w, h }
}

/// handle: 0 TL, 1 TR, 2 BR, 3 BL, 9 move.
fn drag_crop(s: CropRect, handle: u8, dx: f64, dy: f64, aspect: Option<f64>, img_aspect: f64) -> CropRect {
    const MIN: f64 = 0.05;
    if handle == 9 {
        return CropRect { x: (s.x + dx).clamp(0.0, 1.0 - s.w), y: (s.y + dy).clamp(0.0, 1.0 - s.h), ..s };
    }
    let (mut x0, mut y0, mut x1, mut y1) = (s.x, s.y, s.x + s.w, s.y + s.h);
    let left = handle == 0 || handle == 3;
    let top = handle == 0 || handle == 1;
    if left { x0 = (x0 + dx).clamp(0.0, x1 - MIN); } else { x1 = (x1 + dx).clamp(x0 + MIN, 1.0); }
    if top { y0 = (y0 + dy).clamp(0.0, y1 - MIN); } else { y1 = (y1 + dy).clamp(y0 + MIN, 1.0); }
    let mut c = CropRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    if let Some(a) = aspect {
        let nh = (c.w * img_aspect) / a;
        if top { c.y = y1 - nh; }
        c.h = nh;
        if c.y < 0.0 || c.y + c.h > 1.0 {
            c = fit_aspect(c, a, img_aspect);
        }
    }
    c
}
