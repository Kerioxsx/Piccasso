//! Auto-draw: re-draws the guide's strokes in another drawing program by
//! moving the real mouse (feature `autodraw`). The stroke logic is DrawStep's
//! own (drawstep_core::strokes); the only external piece is the OS mouse API
//! wrapper (enigo). Safety: moving the mouse yourself stops the drawing at once.
use crate::app::{DrawStepApp, Screen};
use crate::ui::{compute_rect, primary};
use drawstep_core::imgproc::{fit_dims, resize_box};
use drawstep_core::lock::View;
use drawstep_core::strokes::{fit_into, order_paths, path_stats, strokes_from_analysis, strokes_from_lineart, Path};
use egui::{pos2, vec2, Color32, RichText, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct AutoDrawState {
    source: u8, // 0 outline, 1 outline + inner, 2 detailed line art
    detail: f64,
    speed: f64,
    step_px: f64,
    paths: Option<(String, Vec<Path>)>,
    area: Option<[f64; 4]>,
    corner: [Option<(i32, i32)>; 2],
    capture: Option<(usize, Instant)>,
    run: Option<(Receiver<Msg>, Arc<AtomicBool>)>,
    progress: f32,
    status: String,
}
impl Default for AutoDrawState {
    fn default() -> Self {
        AutoDrawState { source: 1, detail: 1.5, speed: 2500.0, step_px: 2.0, paths: None, area: None, corner: [None, None], capture: None, run: None, progress: 0.0, status: String::new() }
    }
}

pub enum Msg {
    Progress(f32),
    Done(String),
}

#[cfg(feature = "autodraw")]
fn mouse_location() -> Result<(i32, i32), String> {
    use enigo::{Enigo, Mouse, Settings};
    let e = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    e.location().map_err(|e| e.to_string())
}

/// Draw `paths` (normalized) into the screen rectangle `area` (x, y, w, h).
#[cfg(feature = "autodraw")]
pub fn run_drawing(paths: Vec<Path>, area: [f64; 4], aspect: f64, speed: f64, step_px: f64, cancel: Arc<AtomicBool>, tx: std::sync::mpsc::Sender<Msg>) {
    use drawstep_core::strokes::densify;
    use enigo::{Button, Coordinate, Direction, Enigo, Mouse, Settings};
    let mut e = match Enigo::new(&Settings::default()) {
        Ok(e) => e,
        Err(err) => { let _ = tx.send(Msg::Done(format!("Mouse control is not available: {err}"))); return; }
    };
    let r = fit_into(area, aspect);
    let to_screen = |p: [f64; 2]| [r[0] + p[0] * r[2], r[1] + p[1] * r[3]];
    let total = paths.len().max(1);
    let mut expected: Option<(i32, i32)> = None;
    let moved_by_user = |e: &Enigo, exp: Option<(i32, i32)>| -> bool {
        match (exp, e.location()) { (Some((x, y)), Ok((ax, ay))) => (ax - x).abs() > 25 || (ay - y).abs() > 25, _ => false }
    };
    std::thread::sleep(Duration::from_millis(400));
    for (k, path) in paths.iter().enumerate() {
        let pts = densify(&path.iter().map(|p| to_screen(*p)).collect::<Vec<_>>(), step_px);
        if cancel.load(Ordering::Relaxed) || moved_by_user(&e, expected) {
            let _ = tx.send(Msg::Done("Stopped: you moved the mouse.".into()));
            return;
        }
        let (sx, sy) = (pts[0][0].round() as i32, pts[0][1].round() as i32);
        let _ = e.move_mouse(sx, sy, Coordinate::Abs);
        expected = Some((sx, sy));
        std::thread::sleep(Duration::from_millis(12));
        let _ = e.button(Button::Left, Direction::Press);
        let mut t = Instant::now();
        let mut budget = 0.0f64;
        for (i, w) in pts.windows(2).enumerate() {
            budget += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) / speed;
            while t.elapsed().as_secs_f64() < budget { std::hint::spin_loop(); }
            let (x, y) = (w[1][0].round() as i32, w[1][1].round() as i32);
            if i % 12 == 0 && moved_by_user(&e, expected) || cancel.load(Ordering::Relaxed) {
                let _ = e.button(Button::Left, Direction::Release);
                let _ = tx.send(Msg::Done("Stopped: you moved the mouse.".into()));
                return;
            }
            let _ = e.move_mouse(x, y, Coordinate::Abs);
            expected = Some((x, y));
            if budget > 0.5 { t = Instant::now(); budget = 0.0; }
        }
        std::thread::sleep(Duration::from_millis(6));
        let _ = e.button(Button::Left, Direction::Release);
        std::thread::sleep(Duration::from_millis(8));
        let _ = tx.send(Msg::Progress((k + 1) as f32 / total as f32));
    }
    let _ = tx.send(Msg::Done(format!("Finished: {} strokes drawn.", paths.len())));
}

pub fn show(app: &mut DrawStepApp, ctx: &egui::Context) {
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.autodraw;
    let Some(a) = sess.project.analysis.clone() else { return };
    let aspect = a.aspect;
    let mut back = false;
    let mut minimize: Option<bool> = None;

    // Build strokes when the source or detail changes.
    let key = format!("{}|{}|{}", st.source, st.detail, sess.renders_hash);
    if st.paths.as_ref().map_or(true, |(k, _)| k != &key) {
        let raw: Vec<Path> = match st.source {
            0 => strokes_from_analysis(&a, false),
            1 => strokes_from_analysis(&a, true),
            _ => match sess.renders.get("outline") {
                Some(img) => { let (w, h) = fit_dims(img.width, img.height, 700); strokes_from_lineart(&resize_box(img, w, h), st.detail, 6) }
                None => vec![],
            },
        };
        st.paths = Some((key, order_paths(raw.into_iter().filter(|p| p.len() >= 2).collect(), aspect)));
    }
    // Corner capture countdown (hover the target corner, wait 3 s).
    if let Some((i, t0)) = st.capture {
        if t0.elapsed() >= Duration::from_secs(3) {
            #[cfg(feature = "autodraw")]
            match mouse_location() {
                Ok(p) => { st.corner[i] = Some(p); }
                Err(e) => st.status = format!("Could not read the mouse position: {e}"),
            }
            st.capture = None;
            minimize = Some(false);
            if let (Some(a0), Some(b0)) = (st.corner[0], st.corner[1]) {
                let (x0, y0, x1, y1) = (a0.0.min(b0.0), a0.1.min(b0.1), a0.0.max(b0.0), a0.1.max(b0.1));
                if x1 - x0 > 20 && y1 - y0 > 20 { st.area = Some([x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]); }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    if let Some((rx, _)) = &st.run {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Progress(p) => st.progress = p,
                Msg::Done(s) => { st.status = s; minimize = Some(false); }
            }
        }
        if minimize == Some(false) { st.run = None; }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    let paths = st.paths.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
    let (down, up) = path_stats(&paths, aspect);
    egui::SidePanel::right("ad-panel").resizable(true).default_width(380.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Auto-draw").size(22.0).strong());
            ui.label("DrawStep moves your real mouse to draw these lines in any drawing program (Paint, Krita, Photoshop, a web whiteboard…).");
            if !cfg!(feature = "autodraw") {
                ui.colored_label(Color32::from_rgb(0xa2, 0x4d, 0x1c), "This build has no mouse control (enable the `autodraw` feature).");
            }
            ui.add_space(6.0);
            ui.strong("1. What to draw");
            ui.radio_value(&mut st.source, 0, "Main outline only (fastest)");
            ui.radio_value(&mut st.source, 1, "Outline and inner shapes");
            ui.radio_value(&mut st.source, 2, "Detailed line art");
            if st.source == 2 { ui.add(egui::Slider::new(&mut st.detail, 0.6..=4.0).text("Simplify (px)")); }
            ui.label(RichText::new(format!("{} strokes", paths.len())).weak());
            ui.add_space(6.0);
            ui.strong("2. Where to draw");
            ui.label(RichText::new("Open your drawing program next to DrawStep. Click a button, then hover the mouse over that corner of its canvas and keep it still for 3 seconds.").small());
            ui.horizontal(|ui| {
                for (i, l) in [(0usize, "Set top-left corner"), (1, "Set bottom-right corner")] {
                    let label = match (st.capture, st.corner[i]) {
                        (Some((c, t0)), _) if c == i => format!("Hover now… {}", 3 - t0.elapsed().as_secs().min(3)),
                        (_, Some(p)) => format!("{l}: {}, {}", p.0, p.1),
                        _ => l.to_string(),
                    };
                    if ui.add_enabled(st.capture.is_none() && st.run.is_none() && cfg!(feature = "autodraw"), egui::Button::new(label)).clicked() { st.capture = Some((i, Instant::now())); }
                }
            });
            if let Some(ar) = st.area.as_mut() {
                ui.horizontal(|ui| {
                    ui.label("Area:");
                    ui.add(egui::DragValue::new(&mut ar[0]).prefix("x ").speed(1.0));
                    ui.add(egui::DragValue::new(&mut ar[1]).prefix("y ").speed(1.0));
                    ui.add(egui::DragValue::new(&mut ar[2]).prefix("w ").speed(1.0).range(20.0..=10000.0));
                    ui.add(egui::DragValue::new(&mut ar[3]).prefix("h ").speed(1.0).range(20.0..=10000.0));
                });
                let f = fit_into(*ar, aspect);
                ui.label(RichText::new(format!("The drawing will be {:.0} × {:.0} px, centred in the area.", f[2], f[3])).small().weak());
            }
            ui.add_space(6.0);
            ui.strong("3. Speed and precision");
            ui.add(egui::Slider::new(&mut st.speed, 300.0..=8000.0).logarithmic(true).text("Speed (px/s)"));
            ui.add(egui::Slider::new(&mut st.step_px, 1.0..=6.0).text("Point spacing (px)"));
            if let Some(ar) = st.area {
                let scale = fit_into(ar, aspect)[3];
                let secs = (down * scale) / st.speed + paths.len() as f64 * 0.03 + (up * scale) * 0.0;
                ui.label(RichText::new(format!("Estimated time: about {:.0} s", secs.max(1.0))).weak());
            }
            ui.add_space(6.0);
            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.label(RichText::new("Safety: DrawStep minimises itself while drawing. To stop at any moment, just move the mouse — it stops instantly and releases the button. Use a blank canvas and pick a brush first.").small());
            });
            ui.add_space(6.0);
            if st.run.is_some() {
                ui.add(egui::ProgressBar::new(st.progress).show_percentage());
                if ui.button("Stop").clicked() { if let Some((_, c)) = &st.run { c.store(true, Ordering::Relaxed); } }
            } else if ui.add_enabled(st.area.is_some() && !paths.is_empty() && cfg!(feature = "autodraw"), egui::Button::new(RichText::new("Start drawing").color(Color32::WHITE).strong()).fill(crate::ui::ACCENT).min_size(vec2(0.0, 42.0))).clicked() {
                #[cfg(feature = "autodraw")]
                {
                    let (tx, rx) = channel();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let (p2, ar, sp, stp, c2) = (paths.clone(), st.area.unwrap(), st.speed, st.step_px, cancel.clone());
                    std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(900)); run_drawing(p2, ar, aspect, sp, stp, c2, tx) });
                    st.run = Some((rx, cancel));
                    st.progress = 0.0;
                    st.status = "Drawing… move the mouse to stop.".into();
                    minimize = Some(true);
                }
            }
            if !st.status.is_empty() { ui.label(RichText::new(&st.status).strong()); }
            ui.add_space(8.0);
            if ui.button("◀ Back to steps").clicked() { back = true; }
            let _ = primary;
        });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let rect = compute_rect(area, 1000.0, (1000.0 / aspect) as f32, &View::default(), 16.0);
        let painter = ui.painter_at(area);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        let n = paths.len().max(1);
        let done = (st.progress * n as f32) as usize;
        let map = |p: [f64; 2]| pos2(rect.min.x + p[0] as f32 * rect.width(), rect.min.y + p[1] as f32 * rect.height());
        let mut last = pos2(rect.min.x, rect.min.y);
        for (i, p) in paths.iter().enumerate() {
            let pts: Vec<_> = p.iter().map(|q| map(*q)).collect();
            painter.line_segment([last, pts[0]], Stroke::new(0.8_f32, Color32::from_rgba_unmultiplied(120, 120, 120, 60)));
            let col = if i < done { crate::ui::ACCENT } else { Color32::from_rgb(0x24, 0x22, 0x1e) };
            painter.add(egui::Shape::line(pts.clone(), Stroke::new(1.6_f32, col)));
            last = *pts.last().unwrap();
        }
        painter.text(area.left_bottom() + vec2(12.0, -12.0), egui::Align2::LEFT_BOTTOM, format!("Preview: {n} strokes in drawing order (grey lines = pen lifted)"), egui::FontId::proportional(14.0), Color32::GRAY);
    });
    if let Some(m) = minimize { ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(m)); if !m { ctx.send_viewport_cmd(egui::ViewportCommand::Focus); } }
    if st.capture.is_some() && st.capture.unwrap().1.elapsed() < Duration::from_millis(150) { /* keep window visible so the user sees the countdown */ }
    if back {
        if let Some((_, c)) = &st.run { c.store(true, Ordering::Relaxed); }
        app.screen_state.autodraw = Default::default();
        app.go(Screen::Workspace);
    }
}
