//! Locked Reference Mode: steady, interaction-locked guide; OS keep-awake;
//! deliberate unlock (press-and-hold) and always-available exit.
use crate::app::{DrawStepApp, Screen};
use crate::ui::compute_rect;
use drawstep_core::imgproc::adjust;
use drawstep_core::lock::{apply_gesture, Gesture, LockState, ReferenceLock, View};
use egui::{vec2, Color32, RichText, Sense};
use std::time::{Duration, Instant};

pub struct LockedState {
    lock: Option<ReferenceLock>,
    layer: String,
    brightness: f64,
    contrast: f64,
    opacity: f64,
    view: View,
    dark: bool,
    started: Instant,
    #[cfg(feature = "keepawake")]
    awake: Option<keepawake::KeepAwake>,
    wake_status: String,
    base_key: String,
    init: bool,
}
impl Default for LockedState {
    fn default() -> Self {
        LockedState {
            lock: None, layer: "step".into(), brightness: 100.0, contrast: 100.0, opacity: 100.0, view: View::default(), dark: false,
            started: Instant::now(),
            #[cfg(feature = "keepawake")]
            awake: None,
            wake_status: "Screen may turn off".into(), base_key: String::new(), init: false,
        }
    }
}

fn request_awake(st: &mut LockedState) {
    #[cfg(feature = "keepawake")]
    {
        match keepawake::Builder::default().display(true).idle(true).reason("Drawing with DrawStep").app_name("DrawStep").app_reverse_domain("app.drawstep.desktop").create() {
            Ok(k) => { st.awake = Some(k); st.wake_status = "Screen stays on".into(); }
            Err(e) => { st.wake_status = format!("Screen may turn off ({e})"); }
        }
    }
    #[cfg(not(feature = "keepawake"))]
    { st.wake_status = "Keep-awake not in this build".into(); }
}

pub fn show(app: &mut DrawStepApp, ctx: &egui::Context) {
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.locked;
    if !st.init {
        st.init = true;
        st.lock = Some(ReferenceLock::new(Duration::from_millis(app.settings.hold_ms), app.settings.hold_to_unlock));
        st.brightness = sess.project.view.brightness;
        st.contrast = sess.project.view.contrast;
        st.opacity = sess.project.view.opacity;
        st.view = View { zoom: sess.project.view.zoom, pan_x: sess.project.view.pan_x, pan_y: sess.project.view.pan_y };
    }
    let now = app.started.elapsed();
    let mut lock = st.lock.take().unwrap();
    let mut exit: Option<Screen> = None;
    let locked = lock.locked();

    egui::TopBottomPanel::top("lock-bar").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            let status = match lock.state { LockState::Unlocked => "🔓 Unlocked: adjust, then lock", LockState::Locked => "🔒 Locked: the picture won't move", LockState::Unlocking => "Keep holding…" };
            ui.label(RichText::new(status).strong().size(17.0));
            ui.separator();
            ui.label(RichText::new(&st.wake_status).small());
            if app.settings.lock_timer {
                let secs = st.started.elapsed().as_secs();
                ui.label(RichText::new(format!("Time {}:{:02}", secs / 60, secs % 60)).monospace());
                ctx.request_repaint_after(Duration::from_millis(500));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("✖ Exit").min_size(vec2(0.0, 40.0))).clicked() { exit = Some(Screen::Workspace); }
                if locked {
                    let label = if lock.require_hold { "Hold to unlock" } else { "Unlock" };
                    let resp = ui.add(egui::Button::new(format!("🔓 {label}")).min_size(vec2(0.0, 40.0)));
                    let resp = resp.on_hover_text(if lock.require_hold { "Press and hold for a moment. (You can turn holding off in Settings.)" } else { "Unlock the reference" });
                    let held = resp.is_pointer_button_down_on() || (resp.has_focus() && ui.input(|i| i.key_down(egui::Key::Space) || i.key_down(egui::Key::Enter)));
                    if lock.require_hold {
                        if held {
                            if lock.state == LockState::Locked { lock.begin_unlock(now); }
                            let p = lock.tick(now);
                            ui.painter().rect_filled(egui::Rect::from_min_size(resp.rect.min, vec2(resp.rect.width() * p, resp.rect.height())), 6.0, crate::ui::ACCENT.gamma_multiply(0.4));
                            ctx.request_repaint();
                        } else if lock.state == LockState::Unlocking {
                            lock.cancel_unlock();
                        }
                    } else if resp.clicked() {
                        lock.begin_unlock(now);
                    }
                } else if ui.add(egui::Button::new(RichText::new("🔒 Lock screen").color(Color32::WHITE)).fill(crate::ui::ACCENT).min_size(vec2(0.0, 40.0))).clicked() {
                    lock.lock();
                    if app.settings.keep_awake { request_awake(st); }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                }
                if ui.add(egui::Button::new("◀ Back to step").min_size(vec2(0.0, 40.0))).clicked() { exit = Some(Screen::Workspace); }
            });
        });
    });
    if !lock.locked() {
        egui::TopBottomPanel::bottom("lock-controls").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Show:");
                for (v, l) in [("step", "Current step"), ("original", "Original"), ("outline", "Outline"), ("sketch", "Sketch"), ("values", "Shading")] {
                    ui.selectable_value(&mut st.layer, v.to_string(), l);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Slider::new(&mut st.brightness, 50.0..=150.0).text("Brightness %"));
                ui.add(egui::Slider::new(&mut st.contrast, 50.0..=200.0).text("Contrast %"));
                ui.add(egui::Slider::new(&mut st.opacity, 10.0..=100.0).text("Opacity %"));
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("⛶ Fit to page").clicked() { st.view = View::default(); }
                if ui.button("+ Zoom").clicked() { st.view = apply_gesture(st.view, Gesture::Zoom { factor: 1.25, cx: 0.0, cy: 0.0 }, None); }
                if ui.button("-").clicked() { st.view = apply_gesture(st.view, Gesture::Zoom { factor: 0.8, cx: 0.0, cy: 0.0 }, None); }
                ui.checkbox(&mut st.dark, "Dark surround");
                ui.label(RichText::new("Drag or scroll to position, then lock. Brightness here changes only the picture; use your monitor's brightness for the whole screen.").small().weak());
            });
        });
    }

    let bg = if st.dark { Color32::from_gray(17) } else { Color32::WHITE };
    egui::CentralPanel::default().frame(egui::Frame::new().fill(bg)).show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let lock = &lock;
        if resp.dragged() { let d = resp.drag_delta(); st.view = apply_gesture(st.view, Gesture::Pan { dx: d.x as f64, dy: d.y as f64 }, Some(lock)); }
        if resp.hovered() {
            let (zoom, scroll) = ctx.input(|i| (i.zoom_delta(), i.smooth_scroll_delta.y));
            let f = zoom as f64 * (scroll as f64 * 0.0015).exp();
            if (f - 1.0).abs() > 1e-4 {
                let c = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(area.center()) - area.center();
                st.view = apply_gesture(st.view, Gesture::Zoom { factor: f, cx: c.x as f64, cy: c.y as f64 }, Some(lock));
            }
        }
        let (w, h) = super::workspace::ref_size(sess);
        let rect = compute_rect(area, w, h, &st.view, 8.0);
        let painter = ui.painter_at(area);
        let step = sess.project.current_step().cloned();
        // Brightness/contrast are applied on the CPU to the chosen raster (cached by key).
        let base_name = if st.layer == "step" { step.as_ref().map(|s| s.base.clone()).unwrap_or_else(|| "light".into()) } else { st.layer.clone() };
        let key = format!("{base_name}|{}|{}", st.brightness, st.contrast);
        if st.base_key != key {
            if let Some(img) = sess.renders.get(&base_name).or_else(|| sess.renders.get("original")) {
                let adj = adjust(img, st.brightness - 100.0, st.contrast - 100.0);
                let _ = app.textures.set(ctx, "locked-base", &adj);
                st.base_key = key;
            }
        }
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        if base_name != "none" {
            let t = app.textures.get(ctx, "locked-base", sess.renders.get("original").unwrap_or(&sess.src));
            painter.image(t.id(), rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE.gamma_multiply((st.opacity / 100.0) as f32));
        }
        if st.layer == "step" {
            if let (Some(a), Some(s)) = (&sess.project.analysis, &step) { crate::ui::draw_layers(&painter, a, &s.layers, rect, (st.opacity / 100.0) as f32, true); }
        }
    });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !lock.locked() { exit = Some(Screen::Workspace); }
    st.lock = Some(lock);
    if let Some(s) = exit {
        sess.project.view.zoom = st.view.zoom;
        sess.project.view.pan_x = st.view.pan_x;
        sess.project.view.pan_y = st.view.pan_y;
        sess.project.view.brightness = st.brightness;
        sess.project.view.contrast = st.contrast;
        sess.project.view.opacity = st.opacity;
        sess.project.progress.time_spent_sec += st.started.elapsed().as_secs_f64();
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        app.textures.remove("locked-base");
        app.screen_state.locked = Default::default(); // drops the keep-awake guard
        app.save_project();
        app.go(s);
    }
}
