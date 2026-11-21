use crate::app::{PiccassoApp, Screen, Session};
use crate::ui::{compute_rect, primary, Confirm, ConfirmAction, Textures};
use piccasso_core::lock::{apply_gesture, Gesture, View};
use piccasso_core::project::Step;
use piccasso_core::shared::shared;
use egui::{pos2, vec2, Color32, Rect, RichText, Sense};
use std::collections::HashSet;

#[derive(Default)]
pub struct WorkspaceState {
    pub hide_guide: bool,
    pub split: Option<f32>,
    pub mirror: bool,
    pub checks: HashSet<String>,
    pub term: Option<String>,
    pub motion_start: Option<std::time::Instant>,
    pub motion_step: String,
}

const UV: Rect = Rect { min: egui::Pos2 { x: 0.0, y: 0.0 }, max: egui::Pos2 { x: 1.0, y: 1.0 } };

/// Paint the raster base + vector layers of a step into `rect`.
pub fn paint_step(painter: &egui::Painter, ctx: &egui::Context, tex: &mut Textures, sess: &Session, step: Option<&Step>, rect: Rect, show_guide: bool, opacity: f32, base_override: Option<&str>) {
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    let base = base_override.map(String::from).unwrap_or_else(|| if show_guide { step.map(|s| s.base.clone()).unwrap_or_else(|| "light".into()) } else { "original".into() });
    let img = if base == "none" { None } else { sess.renders.get(&base).or_else(|| sess.renders.get(&sess.project.prep.style)).or_else(|| sess.renders.get("original")) };
    if let Some(img) = img {
        let key = format!("render-{base}");
        let t = tex.get(ctx, &key, img);
        painter.image(t.id(), rect, UV, Color32::WHITE.gamma_multiply(opacity));
    } else if sess.renders.is_empty() && base != "none" {
        let t = tex.get(ctx, "source", &sess.src);
        painter.image(t.id(), rect, UV, Color32::WHITE.gamma_multiply(0.35));
    }
    if show_guide && base_override.is_none() {
        if let (Some(a), Some(s)) = (&sess.project.analysis, step) {
            crate::ui::draw_layers_ex(painter, a, &s.layers, rect, opacity.max(0.5), true, crate::ui::grid_for(&sess.project));
        }
    }
}

pub fn ref_size(sess: &Session) -> (f32, f32) {
    if let Some(o) = sess.renders.get("original") { return (o.width as f32, o.height as f32); }
    let a = sess.project.analysis.as_ref().map(|a| a.aspect).unwrap_or(sess.src.width as f64 / sess.src.height as f64);
    (1000.0, (1000.0 / a) as f32)
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let sh = shared();
    let narrow = ctx.screen_rect().width() < 820.0;
    let mut nav: Option<i64> = None;
    let mut complete: Option<bool> = None;
    let mut go: Option<Screen> = None;
    let mut export: Option<&str> = None;
    let mut confirm_restart = false;
    let Some(sess) = app.session.as_mut() else { return };
    let ws = &mut app.screen_state.workspace;
    let job = app.job.as_ref().map(|j| (j.progress, j.label.clone()));

    let panel = |ui: &mut egui::Ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let p = &sess.project;
            let n = p.steps.len();
            let i = p.progress.current_step;
            let Some(step) = p.current_step().cloned() else { ui.label("No steps yet. Go back and prepare your picture."); return };
            ui.horizontal(|ui| {
                ui.strong(format!("Step {} of {}", i + 1, n));
                ui.add(egui::ProgressBar::new(p.fraction_done()).desired_width(ui.available_width()));
            });
            ui.horizontal_wrapped(|ui| {
                for (k, s) in p.steps.iter().enumerate() {
                    let done = p.progress.completed.contains(&s.id);
                    let skipped = p.progress.skipped.contains(&s.id);
                    let txt = RichText::new(format!("{}", k + 1)).color(if k == i { Color32::WHITE } else { ui.visuals().text_color() });
                    let mut b = egui::Button::new(txt).min_size(vec2(30.0, 30.0)).corner_radius(15.0);
                    if k == i { b = b.fill(crate::ui::ACCENT); } else if done { b = b.fill(crate::ui::ACCENT.gamma_multiply(0.3)); }
                    let hover = format!("Step {}: {}{}", k + 1, s.title, if done { " (done)" } else if skipped { " (skipped)" } else { "" });
                    if ui.add(b).on_hover_text(hover).clicked() { nav = Some(k as i64); }
                }
            });
            ui.label(RichText::new(&step.title).size(22.0).strong());
            // projects made with the old AI helper show the built-in wording
            let instr = if step.source.as_deref() == Some("ai") { step.local_instruction.clone().unwrap_or(step.instruction.clone()) } else { step.instruction.clone() };
            ui.label(RichText::new(instr).size(17.0));
            ui.horizontal_wrapped(|ui| {
                ui.add_enabled(false, egui::Button::new("🔊 Read aloud")).on_disabled_hover_text("Audio readout is available in the web/phone version. Use your system's screen reader to hear instructions here.");
                if ui.selectable_label(!ws.hide_guide, if ws.hide_guide { "👁 Show guide" } else { "👁 Hide guide" }).clicked() { ws.hide_guide = !ws.hide_guide; }
                if ui.button("⏭ Skip").clicked() { complete = Some(true); }
                if ui.button("↺ Restart step").clicked() { ws.checks.retain(|c| !c.starts_with(&step.id)); nav = Some(i as i64); }
                if ui.button("↺ Restart all").clicked() { confirm_restart = true; }
            });
            egui::Frame::new().fill(crate::ui::ACCENT.gamma_multiply(0.15)).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(format!("Tip · {}", step.tip)).size(15.0));
            });
            if !step.motion.is_empty() {
                egui::Frame::new().stroke(egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color)).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.strong("Hand movement");
                    ui.label(RichText::new(&step.motion).size(15.0));
                    let on = ws.motion_start.is_some() && ws.motion_step == step.id;
                    if ui.selectable_label(on, if on { "⏹ Stop the motion" } else { "▶ Show me the motion" }).clicked() {
                        if on { ws.motion_start = None; } else { ws.motion_start = Some(std::time::Instant::now()); ws.motion_step = step.id.clone(); }
                    }
                });
            }
            if let Some(a) = &p.analysis {
                let tech = &a.technique;
                if step.kind == "pressure" && !tech.bands.is_empty() {
                    let mut order = tech.bands.clone();
                    order.sort_by(|x, y| y.band.cmp(&x.band));
                    for (k, b) in order.iter().enumerate() {
                        let e = piccasso_core::steps::pressure_for(&tech.bands, b.band, sh);
                        let shade = (235.0 - 195.0 * k as f32 / (order.len().max(2) - 1) as f32) as u8;
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                            ui.painter().circle(r.center(), 12.0, Color32::from_gray(shade), egui::Stroke::new(1.5_f32, ui.visuals().text_color()));
                            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, format!("{}", k + 1), egui::FontId::proportional(13.0), if shade < 128 { Color32::WHITE } else { Color32::BLACK });
                            ui.label(RichText::new(sh.strings.pressure_labels.get(&e.label).cloned().unwrap_or(e.label.clone())).strong());
                            ui.label(format!("about {}% pressure · {} pencil", e.pct, e.grade));
                        });
                        ui.add(egui::ProgressBar::new(e.pct as f32 / 100.0).desired_height(6.0));
                    }
                }
                if step.kind == "color" {
                    ui.horizontal_wrapped(|ui| {
                        let mut seen = Vec::new();
                        for c in &tech.palette {
                            let name = piccasso_core::steps::color_name(c.rgb, &sh.colors.colors);
                            if seen.contains(&name) { continue; }
                            seen.push(name.clone());
                            let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                            ui.painter().circle(r.center(), 8.0, Color32::from_rgb(c.rgb[0] as u8, c.rgb[1] as u8, c.rgb[2] as u8), egui::Stroke::new(1.0_f32, ui.visuals().text_color()));
                            ui.label(name);
                        }
                    });
                }
            }
            if !step.terms.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("What does it mean?").weak());
                    for t in &step.terms {
                        if let Some(term) = sh.glossary.terms.iter().find(|x| &x.id == t) {
                            if ui.button(&term.term).clicked() { ws.term = Some(term.id.clone()); }
                        }
                    }
                });
            }
            ui.collapsing(RichText::new("Check your drawing").strong(), |ui| {
                ui.label(RichText::new("Compare your paper with the guide. The mirror button flips the guide; looking at your drawing in a mirror the same way makes mistakes easier to spot.").small().weak());
                for (k, c) in step.checks.iter().enumerate() {
                    let key = format!("{}#{k}", step.id);
                    let mut on = ws.checks.contains(&key);
                    if ui.checkbox(&mut on, c).changed() { if on { ws.checks.insert(key); } else { ws.checks.remove(&key); } }
                }
            });
            ui.horizontal_wrapped(|ui| {
                for (t, c) in [("Shapes", crate::ui::C_SHAPES), ("Outline", crate::ui::C_SIL), ("Inner shapes", crate::ui::C_INT), ("Subject area", crate::ui::C_SUBJECT)] {
                    ui.label(RichText::new("■").color(c));
                    ui.label(RichText::new(t).small());
                }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.add(egui::Button::new("🔒 Locked view").min_size(vec2(0.0, 40.0))).clicked() { go = Some(Screen::Locked); }
                if ui.add(egui::Button::new("📷 Camera overlay").min_size(vec2(0.0, 40.0))).clicked() { go = Some(Screen::Overlay); }
                if ui.add(egui::Button::new("✔ Check a photo").min_size(vec2(0.0, 40.0))).clicked() { go = Some(Screen::Compare); }
                if ui.add(egui::Button::new("Analyze values").min_size(vec2(0.0, 40.0))).clicked() { go = Some(Screen::Analyze); }
                if ui.add(egui::Button::new("✏ Auto-draw on PC").min_size(vec2(0.0, 40.0))).on_hover_text("Let Piccasso move the mouse to draw the lines in another program").clicked() { go = Some(Screen::AutoDraw); }
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("✏ Edit picture").clicked() { go = Some(Screen::Prepare); }
                ui.menu_button("💾 Export", |ui| {
                    if ui.button("Guide image (PNG)").clicked() { export = Some("png"); ui.close_menu(); }
                    if ui.button("Step sheet (HTML, printable)").clicked() { export = Some("sheet"); ui.close_menu(); }
                    if ui.button("Project file (.piccasso.json)").clicked() { export = Some("project"); ui.close_menu(); }
                });
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(i > 0, egui::Button::new("◀ Previous").min_size(vec2(120.0, 42.0))).clicked() { nav = Some(i as i64 - 1); }
                if primary(ui, if i + 1 == n { "Finish ✔" } else { "Done, next ▶" }).clicked() { complete = Some(false); }
            });
        });
    };
    if narrow {
        egui::TopBottomPanel::bottom("ws-panel").resizable(true).default_height(320.0).show(ctx, panel);
    } else {
        egui::SidePanel::right("ws-panel").resizable(true).default_width(400.0).min_width(320.0).show(ctx, panel);
    }

    egui::CentralPanel::default().frame(egui::Frame::new().fill(ctx.style().visuals.faint_bg_color)).show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        // Gestures (unlocked here): drag = pan, scroll/pinch = zoom, double-click = fit.
        if resp.dragged() { let d = resp.drag_delta(); app.view = apply_gesture(app.view, Gesture::Pan { dx: d.x as f64, dy: d.y as f64 }, None); }
        if resp.hovered() {
            let (zoom, scroll) = ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta.y));
            let f = zoom as f64 * (scroll as f64 * 0.0015).exp();
            if (f - 1.0).abs() > 1e-4 {
                let c = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(area.center()) - area.center();
                app.view = apply_gesture(app.view, Gesture::Zoom { factor: f, cx: c.x as f64, cy: c.y as f64 }, None);
            }
        }
        if resp.double_clicked() { app.view = View::default(); }
        let (w, h) = ref_size(sess);
        let rect = compute_rect(area, w, h, &app.view, 16.0);
        let painter = ui.painter_at(area);
        let step = sess.project.current_step().cloned();
        let opacity = (sess.project.view.opacity / 100.0) as f32;
        let target = painter.clone();
        // Mirror: a rect with swapped x edges maps every point (raster and vectors) mirrored.
        let draw_rect = if ws.mirror { Rect::from_min_max(pos2(rect.max.x, rect.min.y), pos2(rect.min.x, rect.max.y)) } else { rect };
        if let Some(sx) = ws.split {
            let cut = area.min.x + sx * area.width();
            let left = target.with_clip_rect(Rect::from_min_max(area.min, pos2(cut, area.max.y)));
            paint_step(&left, ctx, &mut app.textures, sess, step.as_ref(), draw_rect, false, 1.0, Some("original"));
            let right = target.with_clip_rect(Rect::from_min_max(pos2(cut, area.min.y), area.max));
            paint_step(&right, ctx, &mut app.textures, sess, step.as_ref(), draw_rect, !ws.hide_guide, opacity, None);
            painter.line_segment([pos2(cut, area.min.y), pos2(cut, area.max.y)], egui::Stroke::new(3.0_f32, crate::ui::ACCENT));
            painter.text(area.min + vec2(10.0, 10.0), egui::Align2::LEFT_TOP, "Original", egui::FontId::proportional(14.0), Color32::from_gray(90));
            painter.text(pos2(area.max.x - 70.0, area.min.y + 10.0), egui::Align2::RIGHT_TOP, "Guide", egui::FontId::proportional(14.0), Color32::from_gray(90));
        } else {
            paint_step(&target, ctx, &mut app.textures, sess, step.as_ref(), draw_rect, !ws.hide_guide, opacity, None);
        }
        if let (Some(t0), Some(a), Some(st)) = (ws.motion_start, &sess.project.analysis, &step) {
            if ws.motion_step == st.id {
                let strokes = piccasso_core::motion::motion_paths(a, st);
                let total: f32 = strokes.iter().map(|s| s.points.windows(2).map(|w| (((w[1][0] - w[0][0]) * draw_rect.width() as f64).hypot((w[1][1] - w[0][1]) * draw_rect.height() as f64)) as f32).sum::<f32>().max(30.0)).sum();
                let dur = (total / (draw_rect.width().abs() * 0.45).max(120.0)).max(2.5);
                let t = t0.elapsed().as_secs_f32() % (dur + 1.2);
                crate::ui::draw_motion(&target, &strokes, draw_rect, (t / dur).min(1.0));
                ctx.request_repaint();
            } else { ws.motion_start = None; }
        }
        if let Some((p, l)) = &job {
            painter.text(area.left_bottom() + vec2(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("Preparing guide images… {}% ({l})", (p * 100.0) as i32), egui::FontId::proportional(14.0), Color32::from_gray(80));
            ctx.request_repaint();
        }
        // Stage tools
        let tools = Rect::from_min_size(pos2(area.max.x - 76.0, area.min.y + 10.0), vec2(66.0, 300.0));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tools), |ui| {
            if ui.add_sized([64.0, 36.0], egui::Button::new("+")).on_hover_text("Zoom in").clicked() { app.view = apply_gesture(app.view, Gesture::Zoom { factor: 1.25, cx: 0.0, cy: 0.0 }, None); }
            if ui.add_sized([64.0, 36.0], egui::Button::new("-")).on_hover_text("Zoom out").clicked() { app.view = apply_gesture(app.view, Gesture::Zoom { factor: 0.8, cx: 0.0, cy: 0.0 }, None); }
            if ui.add_sized([64.0, 36.0], egui::Button::new("⛶")).on_hover_text("Fit to screen").clicked() { app.view = View::default(); }
            if ui.add_sized([64.0, 36.0], egui::SelectableLabel::new(ws.split.is_some(), "Split")).on_hover_text("Compare with the original (split view)").clicked() { ws.split = if ws.split.is_some() { None } else { Some(0.5) }; }
            if ui.add_sized([64.0, 36.0], egui::SelectableLabel::new(ws.mirror, "Mirror")).on_hover_text("Mirror view (helps spot mistakes)").clicked() { ws.mirror = !ws.mirror; }
        });
        if let Some(sx) = ws.split.as_mut() {
            let r = Rect::from_min_size(pos2(area.min.x + 20.0, area.max.y - 40.0), vec2(area.width().min(360.0), 30.0));
            ui.allocate_new_ui(egui::UiBuilder::new().max_rect(r), |ui| { ui.add(egui::Slider::new(sx, 0.02..=0.98).text("Divider").show_value(false)); });
        }
    });

    if let Some(id) = ws.term.clone() {
        let mut open = true;
        if let Some(t) = sh.glossary.terms.iter().find(|x| x.id == id) {
            egui::Window::new(&t.term).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, vec2(0.0, 0.0)).show(ctx, |ui| { ui.set_max_width(380.0); ui.label(&t.text); });
        }
        if !open { ws.term = None; }
    }

    // Keyboard shortcuts
    let (right, left, gkey) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowRight), i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::G)));
    let typing = ctx.memory(|m| m.focused().is_some());
    if !typing {
        if right { complete = Some(false); }
        if left { nav = Some(sess.project.progress.current_step as i64 - 1); }
        if gkey { ws.hide_guide = !ws.hide_guide; }
    }

    let mut finished = false;
    if let Some(k) = nav { sess.project.goto_step(k); sess.dirty = true; }
    if let Some(skip) = complete {
        finished = !skip && sess.project.progress.current_step + 1 == sess.project.steps.len();
        sess.project.complete_step(skip);
        sess.dirty = true;
    }
    if confirm_restart {
        app.confirm = Some(Confirm { title: "Restart the whole tutorial?".into(), body: "You'll go back to step 1. Your picture and settings are kept.".into(), ok: "Restart".into(), action: ConfirmAction::RestartTutorial });
    }
    if nav.is_some() || complete.is_some() { app.save_project(); }
    if finished { app.toast("You finished all the steps! 🎉 Check a photo of your drawing or export a step sheet."); }
    match export {
        Some("png") => crate::export::export_png(app),
        Some("sheet") => crate::export::export_step_sheet(app),
        Some("project") => { if let Some(s) = app.session.as_ref() { let p = s.project.clone(); crate::export::export_project_file(app, &p); } }
        _ => {}
    }
    if let Some(s) = go { app.save_project(); app.go(s); }
}
