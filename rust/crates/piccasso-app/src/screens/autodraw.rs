//! Auto-draw: re-draws the guide's strokes in another drawing program by
//! moving the real mouse (feature `autodraw`). The stroke logic is Piccasso's
//! own (piccasso_core::strokes); the only external piece is the OS mouse API
//! wrapper (enigo). Safety: moving the mouse yourself stops the drawing at once.
use crate::app::{PiccassoApp, Screen};
use crate::ui::{compute_rect, primary};
use piccasso_core::imgproc::{fit_dims, gray_to_rgba, resize_box, Rgba};
use piccasso_core::replica::{match_score, plan_replica, plan_stats, simulate, ReplicaOptions, ReplicaPlan};
use piccasso_core::lock::View;
use piccasso_core::strokes::{fit_into, order_paths, path_stats, strokes_from_analysis, strokes_from_lineart, Path};
use egui::{pos2, vec2, Color32, RichText, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target { Corner(usize), Brush, LineBrush, SliderZero, SliderFull, Button(usize), FullButton, SizeMin, SizeMax }

/// What the full-screen picker is choosing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickWhat { Tool(Target), Area }

pub struct Pick { pub what: PickWhat, pub shot: Option<Rgba>, pub start: Option<(i32, i32)> }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpacityMode { None, Slider, Buttons }

/// Where the target program's tools are (screen pixels).
#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub brush: Option<(i32, i32)>,
    /// Optional thin brush for the fine pass (ink lines, hatching).
    pub line_brush: Option<(i32, i32)>,
    pub slider: [Option<(i32, i32)>; 2],
    pub buttons: Vec<Option<(i32, i32)>>,
    pub full: Option<(i32, i32)>,
    /// Brush-size slider ends and the sizes (px) they stand for.
    pub size_slider: [Option<(i32, i32)>; 2],
    pub size_range: (f64, f64),
    /// Size sliders in many programs are logarithmic.
    pub size_log: bool,
}

impl Tools {
    /// Screen point on the brush-size slider for `px`.
    pub fn new_default() -> Self { Tools { size_range: (1.0, 100.0), ..Default::default() } }
    pub fn size_point(&self, px: f64) -> Option<(i32, i32)> {
        let [Some(a), Some(b)] = self.size_slider else { return None };
        let (lo, hi) = (self.size_range.0.max(0.1), self.size_range.1.max(self.size_range.0 + 0.1));
        let px = px.clamp(lo, hi);
        let t = if self.size_log { (px / lo).ln() / (hi / lo).ln() } else { (px - lo) / (hi - lo) };
        let t = 0.005 + t * 0.99; // a click exactly on an end point can miss the slider
        Some(((a.0 as f64 + (b.0 - a.0) as f64 * t).round() as i32, (a.1 as f64 + (b.1 - a.1) as f64 * t).round() as i32))
    }
}

pub struct AutoDrawState {
    source: u8, // 0 outline, 1 outline + inner, 2 detailed line art, 3 exact replica
    tones: usize,
    brush_px: f64,
    depth: bool,
    coat: f64,
    line_px: f64,
    replica_lines: bool,
    opacity_mode: OpacityMode,
    tools: Tools,
    replica: Option<(String, piccasso_core::replica::ReplicaPlan, f64)>,
    replica_rx: Option<(String, Receiver<(piccasso_core::replica::ReplicaPlan, f64, Rgba)>)>,
    sim: Option<Rgba>,
    pub pick: Option<Pick>,
    detail: f64,
    speed: f64,
    step_px: f64,
    paths: Option<(String, Vec<Path>)>,
    area: Option<[f64; 4]>,
    corner: [Option<(i32, i32)>; 2],
    capture: Option<(Target, Instant)>,
    run: Option<(Receiver<Msg>, Arc<AtomicBool>)>,
    progress: f32,
    status: String,
}
impl Default for AutoDrawState {
    fn default() -> Self {
        AutoDrawState { pick: None, tones: 4, brush_px: 6.0, depth: true, coat: 0.25, line_px: 2.0, replica_lines: true, opacity_mode: OpacityMode::Slider, tools: Tools::new_default(), replica: None, replica_rx: None, sim: None,
            source: 1, detail: 1.5, speed: 2500.0, step_px: 2.0, paths: None, area: None, corner: [None, None], capture: None, run: None, progress: 0.0, status: String::new() }
    }
}

impl AutoDrawState {
    /// Transparent window needed (picker active without a screenshot underneath).
    pub fn picking(&self) -> bool { self.pick.as_ref().map_or(false, |p| p.shot.is_none()) }
    fn set_target(&mut self, t: Target, p: (i32, i32)) {
        match t {
            Target::Corner(i) => self.corner[i] = Some(p),
            Target::Brush => self.tools.brush = Some(p),
            Target::LineBrush => self.tools.line_brush = Some(p),
            Target::SliderZero => self.tools.slider[0] = Some(p),
            Target::SliderFull => self.tools.slider[1] = Some(p),
            Target::Button(i) => { if self.tools.buttons.len() <= i { self.tools.buttons.resize(i + 1, None); } self.tools.buttons[i] = Some(p); }
            Target::FullButton => self.tools.full = Some(p),
            Target::SizeMin => self.tools.size_slider[0] = Some(p),
            Target::SizeMax => self.tools.size_slider[1] = Some(p),
        }
        if let (Some(a0), Some(b0), Target::Corner(_)) = (self.corner[0], self.corner[1], t) {
            let (x0, y0, x1, y1) = (a0.0.min(b0.0), a0.1.min(b0.1), a0.0.max(b0.0), a0.1.max(b0.1));
            if x1 - x0 > 20 && y1 - y0 > 20 { self.area = Some([x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]); }
        }
    }
}

/// Full-screen picker: click a tool, or drag a box for the drawing area.
fn show_picker(st: &mut AutoDrawState, tex: &mut crate::ui::Textures, ctx: &egui::Context) {
    let Some(pk) = st.pick.as_mut() else { return };
    let mut done: Option<Option<(PickWhat, (i32, i32), (i32, i32))>> = None;
    egui::CentralPanel::default().frame(egui::Frame::new().fill(if pk.shot.is_some() { Color32::BLACK } else { Color32::from_black_alpha(70) })).show(ctx, |ui| {
        let area = ui.max_rect();
        let resp = ui.allocate_rect(area, egui::Sense::click_and_drag());
        let painter = ui.painter_at(area);
        if let Some(shot) = &pk.shot {
            let t = tex.get(ctx, "screen-shot", shot);
            painter.image(t.id(), area, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            painter.rect_filled(area, 0.0, Color32::from_black_alpha(40));
        }
        let what = match pk.what {
            PickWhat::Area => "Drag a box around the area to draw in (your program's canvas).".to_string(),
            PickWhat::Tool(t) => format!("Click on: {}", match t {
                Target::Brush => "the fill brush button", Target::LineBrush => "the line brush button",
                Target::SliderZero => "the opacity slider at 0%", Target::SliderFull => "the opacity slider at 100%",
                Target::SizeMin => "the brush-size slider at its smallest size", Target::SizeMax => "the brush-size slider at its largest size",
                Target::FullButton => "the 100% opacity button", Target::Button(_) => "this shade's opacity button", Target::Corner(_) => "the corner",
            }),
        };
        let banner = egui::Rect::from_center_size(pos2(area.center().x, area.min.y + 40.0), vec2(area.width().min(760.0), 54.0));
        painter.rect_filled(banner, 10.0, Color32::from_black_alpha(200));
        painter.text(banner.center(), egui::Align2::CENTER_CENTER, format!("{what}   ·   Esc to cancel"), egui::FontId::proportional(18.0), Color32::WHITE);
        if let Some(p) = ctx.input(|i| i.pointer.hover_pos()) {
            let st_ = Stroke::new(1.0_f32, Color32::from_rgb(0x7c, 0xc4, 0xb3));
            painter.line_segment([pos2(area.min.x, p.y), pos2(area.max.x, p.y)], st_);
            painter.line_segment([pos2(p.x, area.min.y), pos2(p.x, area.max.y)], st_);
        }
        if resp.drag_started() || (resp.is_pointer_button_down_on() && pk.start.is_none()) {
            pk.start = crate::screen_pick::mouse_position();
        }
        if let (PickWhat::Area, Some(a), Some(p0)) = (pk.what, pk.start, resp.interact_pointer_pos()) {
            if let Some(start_pt) = ctx.input(|i| i.pointer.press_origin()) {
                let r = egui::Rect::from_two_pos(start_pt, p0);
                painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(0x7c, 0xc4, 0xb3)), egui::StrokeKind::Middle);
                let _ = a;
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            // a quick click can press and release within one frame: fall back to the current position
            let b = crate::screen_pick::mouse_position();
            if let (Some(a), Some(b)) = (pk.start.or(b), b) { done = Some(Some((pk.what, a, b))); }
        }
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { done = Some(None); }
    if let Some(res) = done {
        st.pick = None;
        tex.remove("screen-shot");
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        match res {
            Some((PickWhat::Tool(t), _, b)) => st.set_target(t, b),
            Some((PickWhat::Area, a, b)) => {
                let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
                if x1 - x0 > 20 && y1 - y0 > 20 {
                    st.corner = [Some((x0, y0)), Some((x1, y1))];
                    st.area = Some([x0 as f64, y0 as f64, (x1 - x0) as f64, (y1 - y0) as f64]);
                } else { st.status = "The box was too small; drag a bigger one.".into(); }
            }
            None => {}
        }
    }
    ctx.request_repaint();
}

fn start_pick(st: &mut AutoDrawState, what: PickWhat, ctx: &egui::Context) {
    st.pick = Some(Pick { what, shot: crate::screen_pick::capture_screen(), start: None });
    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
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

/// Pause at corners (ms) so the target program registers every turn.
pub static DWELL_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(25);

#[cfg(feature = "autodraw")]
mod pen {
    use enigo::{Button, Coordinate, Direction, Enigo, Mouse};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    pub const STOPPED: &str = "Stopped: you moved the mouse.";

    pub struct Pen<'a> { pub e: Enigo, pub expected: Option<(i32, i32)>, pub cancel: &'a AtomicBool, pub speed: f64, pub step: f64, pub dwell: Duration }

    impl Pen<'_> {
        fn moved(&self) -> bool {
            match (self.expected, self.e.location()) { (Some((x, y)), Ok((ax, ay))) => (ax - x).abs() > 25 || (ay - y).abs() > 25, _ => false }
        }
        fn check(&self) -> Result<(), String> {
            if self.cancel.load(Ordering::Relaxed) || self.moved() { Err(STOPPED.into()) } else { Ok(()) }
        }
        fn go(&mut self, x: i32, y: i32) { let _ = self.e.move_mouse(x, y, Coordinate::Abs); self.expected = Some((x, y)); }
        /// Click a tool in the target program and give it time to react.
        pub fn click(&mut self, p: (i32, i32)) -> Result<(), String> {
            self.check()?;
            self.go(p.0, p.1);
            std::thread::sleep(Duration::from_millis(70));
            let _ = self.e.button(Button::Left, Direction::Press);
            std::thread::sleep(Duration::from_millis(40));
            let _ = self.e.button(Button::Left, Direction::Release);
            std::thread::sleep(Duration::from_millis(250));
            Ok(())
        }
        /// One pen-down stroke through screen points. Drawing programs sample the
        /// mouse only ~60-120 times a second, so the pen waits `dwell` at the
        /// start, at every corner and before lifting; straight runs between
        /// corners stay straight even if samples are dropped.
        pub fn stroke(&mut self, pts: &[[f64; 2]]) -> Result<(), String> {
            self.check()?;
            self.go(pts[0][0].round() as i32, pts[0][1].round() as i32);
            std::thread::sleep(self.dwell.max(Duration::from_millis(12)));
            let _ = self.e.button(Button::Left, Direction::Press);
            std::thread::sleep(self.dwell);
            let mut t = Instant::now();
            let mut budget = 0.0f64;
            let mut n = 0usize;
            for (k, seg) in pts.windows(2).enumerate() {
                let dense = piccasso_core::strokes::densify(seg, self.step);
                for w in dense.windows(2) {
                    budget += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]) / self.speed;
                    while t.elapsed().as_secs_f64() < budget { std::hint::spin_loop(); }
                    n += 1;
                    if n % 12 == 0 && (self.moved() || self.cancel.load(Ordering::Relaxed)) {
                        let _ = self.e.button(Button::Left, Direction::Release);
                        return Err(STOPPED.into());
                    }
                    self.go(w[1][0].round() as i32, w[1][1].round() as i32);
                }
                // corner: let the program register this point before turning
                let turn = if k + 2 < pts.len() {
                    let (a, b, c) = (pts[k], pts[k + 1], pts[k + 2]);
                    let (ux, uy, vx, vy) = (b[0] - a[0], b[1] - a[1], c[0] - b[0], c[1] - b[1]);
                    let l = (ux.hypot(uy) * vx.hypot(vy)).max(1e-9);
                    (ux * vx + uy * vy) / l < 0.94
                } else { false };
                if turn { std::thread::sleep(self.dwell); t = Instant::now(); budget = 0.0; }
                if budget > 0.5 { t = Instant::now(); budget = 0.0; }
            }
            std::thread::sleep(self.dwell);
            let _ = self.e.button(Button::Left, Direction::Release);
            std::thread::sleep(Duration::from_millis(8));
            Ok(())
        }
    }
}

/// Draw `paths` (normalized) into the screen rectangle `area` (x, y, w, h).
#[cfg(feature = "autodraw")]
pub fn run_drawing(paths: Vec<Path>, area: [f64; 4], aspect: f64, speed: f64, step_px: f64, cancel: Arc<AtomicBool>, tx: std::sync::mpsc::Sender<Msg>) {
    let e = match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(e) => e,
        Err(err) => { let _ = tx.send(Msg::Done(format!("Mouse control is not available: {err}"))); return; }
    };
    let mut pen = pen::Pen { e, expected: None, cancel: &cancel, speed, step: step_px, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
    let r = fit_into(area, aspect);
    let total = paths.len().max(1);
    std::thread::sleep(Duration::from_millis(400));
    for (k, path) in paths.iter().enumerate() {
        let pts: Vec<[f64; 2]> = path.iter().map(|p| [r[0] + p[0] * r[2], r[1] + p[1] * r[3]]).collect();
        if let Err(m) = pen.stroke(&pts) { let _ = tx.send(Msg::Done(m)); return; }
        let _ = tx.send(Msg::Progress((k + 1) as f32 / total as f32));
    }
    let _ = tx.send(Msg::Done(format!("Finished: {} strokes drawn.", paths.len())));
}

/// Draw a replica plan (pixel coordinates) at screen origin `org`, selecting the
/// brush and the opacity for each shade with the recorded tool positions.
#[cfg(feature = "autodraw")]
#[allow(clippy::too_many_arguments)]
pub fn run_replica(plan: ReplicaPlan, org: [f64; 2], tools: Tools, mode: OpacityMode, speed: f64, step_px: f64, cancel: Arc<AtomicBool>, tx: std::sync::mpsc::Sender<Msg>) {
    let e = match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(e) => e,
        Err(err) => { let _ = tx.send(Msg::Done(format!("Mouse control is not available: {err}"))); return; }
    };
    let mut pen = pen::Pen { e, expected: None, cancel: &cancel, speed, step: step_px, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
    let (total, _) = plan_stats(&plan);
    let mut done = 0usize;
    let set_strength = |pen: &mut pen::Pen, opacity: f64, layer_index: Option<usize>, fine: bool, size_px: Option<f64>| -> Result<(), String> {
        let brush = if fine { tools.line_brush.or(tools.brush) } else { tools.brush };
        if let Some(b) = brush { pen.click(b)?; }
        if let Some(px) = size_px { if let Some(p) = tools.size_point(px) { pen.click(p)?; } }
        match mode {
            OpacityMode::Slider => if let [Some(z), Some(f)] = tools.slider {
                // stay a hair inside the ends: a click exactly on an end point can miss the slider
                let t = 0.005 + opacity.clamp(0.0, 1.0) * 0.99;
                pen.click(((z.0 as f64 + (f.0 - z.0) as f64 * t).round() as i32, (z.1 as f64 + (f.1 - z.1) as f64 * t).round() as i32))?;
            },
            OpacityMode::Buttons => {
                let b = match layer_index { Some(i) => tools.buttons.get(i).copied().flatten(), None => tools.full };
                if let Some(b) = b { pen.click(b)?; }
            }
            OpacityMode::None => {}
        }
        Ok(())
    };
    std::thread::sleep(Duration::from_millis(400));
    let mut run = || -> Result<(), String> {
        for (li, layer) in plan.layers.iter().enumerate() {
            set_strength(&mut pen, layer.opacity, Some(li), layer.fine, Some(layer.brush))?;
            for p in &layer.paths {
                let pts: Vec<[f64; 2]> = p.iter().map(|q| [org[0] + q[0], org[1] + q[1]]).collect();
                pen.stroke(&pts)?;
                done += 1;
                let _ = tx.send(Msg::Progress(done as f32 / total.max(1) as f32));
            }
        }
        if !plan.lines.is_empty() {
            set_strength(&mut pen, 1.0, None, true, None)?;
            for p in &plan.lines {
                let pts: Vec<[f64; 2]> = p.iter().map(|q| [org[0] + q[0], org[1] + q[1]]).collect();
                pen.stroke(&pts)?;
                done += 1;
                let _ = tx.send(Msg::Progress(done as f32 / total.max(1) as f32));
            }
        }
        Ok(())
    };
    let res = run();
    let _ = tx.send(Msg::Done(match res { Ok(()) => format!("Finished: {} shades and {} lines drawn.", plan.layers.len(), plan.lines.len()), Err(m) => m }));
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.autodraw;
    if st.pick.is_some() { show_picker(st, &mut app.textures, ctx); return; }
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
    // Replica plan at the exact pixel size of the drawing area (background thread).
    if st.source == 3 {
        let (tw, th) = match st.area { Some(ar) => { let f = fit_into(ar, aspect); (f[2].round() as usize, f[3].round() as usize) } None => (600, (600.0 / aspect).round() as usize) };
        let rkey = format!("{tw}x{th}|{}|{}|{}|{:?}|{}|{}|{}|{}", st.tones, st.brush_px, st.replica_lines, st.opacity_mode, sess.renders_hash, st.depth, st.coat, st.line_px);
        if let Some((k, rx)) = &st.replica_rx {
            if let Ok((plan, score, sim)) = rx.try_recv() {
                let k = k.clone();
                st.replica = Some((k, plan, score));
                app.textures.remove("replica-sim");
                st.sim = Some(sim);
                st.replica_rx = None;
            } else { ctx.request_repaint_after(Duration::from_millis(100)); }
        }
        let current = st.replica.as_ref().map(|(k, _, _)| k == &rkey).unwrap_or(false) || st.replica_rx.as_ref().map(|(k, _)| k == &rkey).unwrap_or(false);
        if !current && st.replica_rx.is_none() {
            if let Some(src) = sess.renders.get("original").cloned() {
                let outline = sess.renders.get("outline").cloned();
                let o = ReplicaOptions { tones: st.tones, brush_px: st.brush_px, overlap: 0.85, lines: st.replica_lines, opacity_control: st.opacity_mode != OpacityMode::None,
                    glaze: (st.depth && st.opacity_mode != OpacityMode::Buttons).then_some(st.coat), line_brush_px: Some(st.line_px), ..Default::default() };
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let plan = plan_replica(&src, outline.as_ref(), tw, th, &o);
                    let sim = simulate(&plan, o.brush_px, o.opacity_control);
                    let score = match_score(&sim, &plan.target, o.tones);
                    let _ = tx.send((plan, score, gray_to_rgba(&sim)));
                });
                st.replica_rx = Some((rkey, rx));
            }
        }
    }
    // Capture countdown (hover the corner or tool, keep still for 3 s).
    if let Some((target, t0)) = st.capture {
        if t0.elapsed() >= Duration::from_secs(3) {
            #[cfg(feature = "autodraw")]
            match mouse_location() {
                Ok(p) => st.set_target(target, p),
                Err(e) => st.status = format!("Could not read the mouse position: {e}"),
            }
            #[cfg(not(feature = "autodraw"))]
            let _ = target;
            st.capture = None;
            minimize = Some(false);
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
            ui.label("Piccasso moves your real mouse to draw these lines in any drawing program (Paint, Krita, Photoshop, a web whiteboard…).");
            if !cfg!(feature = "autodraw") {
                ui.colored_label(Color32::from_rgb(0xa2, 0x4d, 0x1c), "This build has no mouse control (enable the `autodraw` feature).");
            }
            ui.add_space(6.0);
            ui.strong("1. What to draw");
            ui.radio_value(&mut st.source, 0, "Main outline only (fastest)");
            ui.radio_value(&mut st.source, 1, "Outline and inner shapes");
            ui.radio_value(&mut st.source, 2, "Detailed line art");
            ui.radio_value(&mut st.source, 3, "Exact replica (shading + lines)");
            if st.source == 2 { ui.add(egui::Slider::new(&mut st.detail, 0.6..=4.0).text("Simplify (px)")); }
            if st.source == 3 {
                ui.checkbox(&mut st.depth, "Depth mode: build smooth shading from light coats (best for 3D-looking pictures)");
                if st.depth {
                    ui.add(egui::Slider::new(&mut st.coat, 0.1..=0.5).text("Opacity of each coat"));
                    ui.label(RichText::new("Darker areas get more coats; pure black gets one solid pass. Needs the opacity slider (not per-shade buttons).").small().weak());
                } else {
                    ui.add(egui::Slider::new(&mut st.tones, 2..=8).text("Shades (incl. paper white)"));
                }
                ui.add(egui::Slider::new(&mut st.brush_px, 1.0..=40.0).text("Fill brush size (px)"));
                ui.add(egui::Slider::new(&mut st.line_px, 1.0..=10.0).text("Line brush size (px)"));
                ui.label(RichText::new("Thin parts (ink lines, hatching) are traced along their centre with the line brush.").small().weak());
                ui.label(RichText::new("Set the same brush size in your drawing program, with hardness 100% for the cleanest result.").small().weak());
                ui.checkbox(&mut st.replica_lines, "Draw the outlines on top");
                match &st.replica {
                    Some((_, plan, score)) => {
                        let (n, _) = plan_stats(plan);
                        ui.label(RichText::new(format!("Simulated match: {:.1}% of pixels within one shade · {} shades · {n} strokes", score * 100.0, plan.layers.len())).strong());
                    }
                    None => { ui.label(RichText::new("Working out the shading…").weak()); }
                }
            } else {
                ui.label(RichText::new(format!("{} strokes", paths.len())).weak());
            }
            ui.add_space(6.0);
            ui.strong("2. Where to draw");
            ui.label(RichText::new("Open your drawing program next to Piccasso. Click a button, then hover the mouse over that corner of its canvas and keep it still for 3 seconds.").small());
            ui.horizontal(|ui| {
                for (i, l) in [(0usize, "Set top-left corner"), (1, "Set bottom-right corner")] {
                    let label = match (st.capture, st.corner[i]) {
                        (Some((Target::Corner(c), t0)), _) if c == i => format!("Hover now… {}", 3 - t0.elapsed().as_secs().min(3)),
                        (_, Some(p)) => format!("{l}: {}, {}", p.0, p.1),
                        _ => l.to_string(),
                    };
                    if ui.add_enabled(st.capture.is_none() && st.run.is_none() && cfg!(feature = "autodraw"), egui::Button::new(label)).clicked() { st.capture = Some((Target::Corner(i), Instant::now())); }
                }
            });
            if ui.add_enabled(st.run.is_none() && cfg!(feature = "autodraw"), egui::Button::new("Select area on screen (drag a box)")).clicked() { start_pick(st, PickWhat::Area, ctx); }
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
            if st.source == 3 {
                ui.add_space(6.0);
                ui.strong("Tools in your drawing program");
                ui.label(RichText::new("Piccasso clicks these before each shade. Record them the same way: click, then hover over the tool for 3 seconds.").small());
                let can = st.capture.is_none() && st.run.is_none() && cfg!(feature = "autodraw");
                let mut cap: Option<Target> = None;
                let mut pick_tool: Option<Target> = None;
                let mut tool_btn = |ui: &mut egui::Ui, t: Target, label: &str, val: Option<(i32, i32)>, capture: Option<(Target, Instant)>| {
                    let text = match (capture, val) {
                        (Some((c, t0)), _) if c == t => format!("Hover now… {}", 3 - t0.elapsed().as_secs().min(3)),
                        (_, Some(p)) => format!("{label} ✔ ({}, {})", p.0, p.1),
                        _ => label.to_string(),
                    };
                    ui.horizontal(|ui| {
                        if ui.add_enabled(can, egui::Button::new(text)).on_hover_text("Click, then hold the mouse over the tool for 3 seconds").clicked() { cap = Some(t); }
                        if ui.add_enabled(can, egui::Button::new("Pick on screen")).on_hover_text("Shows your screen full-size: click the tool").clicked() { pick_tool = Some(t); }
                    });
                };
                tool_btn(ui, Target::Brush, "Fill brush button (optional)", st.tools.brush, st.capture);
                tool_btn(ui, Target::LineBrush, "Line brush button (optional)", st.tools.line_brush, st.capture);
                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    ui.selectable_value(&mut st.opacity_mode, OpacityMode::Slider, "Slider");
                    ui.selectable_value(&mut st.opacity_mode, OpacityMode::Buttons, "A button per shade");
                    ui.selectable_value(&mut st.opacity_mode, OpacityMode::None, "No opacity control");
                });
                match st.opacity_mode {
                    OpacityMode::Slider => {
                        ui.horizontal(|ui| {
                            tool_btn(ui, Target::SliderZero, "Slider at 0%", st.tools.slider[0], st.capture);
                            tool_btn(ui, Target::SliderFull, "Slider at 100%", st.tools.slider[1], st.capture);
                        });
                        ui.label(RichText::new("Piccasso clicks along the slider at each shade's opacity.").small().weak());
                    }
                    OpacityMode::Buttons => {
                        if let Some((_, plan, _)) = &st.replica {
                            for (i, l) in plan.layers.iter().enumerate() {
                                let label = format!("Shade {} · {:.0}% opacity", i + 1, l.opacity * 100.0);
                                tool_btn(ui, Target::Button(i), &label, st.tools.buttons.get(i).copied().flatten(), st.capture);
                            }
                        }
                        if st.replica_lines { tool_btn(ui, Target::FullButton, "100% (for outlines)", st.tools.full, st.capture); }
                        ui.label(RichText::new("Use preset buttons, swatches or brushes you set up at these opacities.").small().weak());
                    }
                    OpacityMode::None => {
                        ui.label(RichText::new("Shades are made by spacing the strokes: lighter shades get sparser rows, like shading with one pencil.").small().weak());
                    }
                }
                ui.add_space(4.0);
                ui.strong("Brush size slider (optional)");
                ui.horizontal(|ui| {
                    tool_btn(ui, Target::SizeMin, "Smallest end", st.tools.size_slider[0], st.capture);
                });
                ui.horizontal(|ui| {
                    tool_btn(ui, Target::SizeMax, "Largest end", st.tools.size_slider[1], st.capture);
                });
                ui.horizontal(|ui| {
                    ui.label("Sizes at the ends:");
                    ui.add(egui::DragValue::new(&mut st.tools.size_range.0).range(0.1..=500.0).suffix(" px"));
                    ui.label("to");
                    ui.add(egui::DragValue::new(&mut st.tools.size_range.1).range(1.0..=2000.0).suffix(" px"));
                    ui.checkbox(&mut st.tools.size_log, "logarithmic");
                });
                ui.label(RichText::new("When set, Piccasso sets the fill size before fill passes and the line size before fine passes by clicking the slider.").small().weak());
                if let Some(t) = cap { st.capture = Some((t, Instant::now())); }
                if let Some(t) = pick_tool { start_pick(st, PickWhat::Tool(t), ctx); }
            }
            ui.add_space(6.0);
            ui.strong("3. Speed and precision");
            ui.add(egui::Slider::new(&mut st.speed, 300.0..=8000.0).logarithmic(true).text("Speed (px/s)"));
            ui.add(egui::Slider::new(&mut st.step_px, 1.0..=6.0).text("Point spacing (px)"));
            let mut dwell = DWELL_MS.load(Ordering::Relaxed);
            if ui.add(egui::Slider::new(&mut dwell, 5..=80).text("Corner pause (ms)")).on_hover_text("Drawing programs read the mouse 60-120 times a second. Pausing at each corner makes sure no corner is cut. Raise it if you see diagonal shortcuts.").changed() { DWELL_MS.store(dwell, Ordering::Relaxed); }
            if let Some(ar) = st.area {
                let scale = fit_into(ar, aspect)[3];
                let secs = (down * scale) / st.speed + paths.len() as f64 * 0.03 + (up * scale) * 0.0;
                ui.label(RichText::new(format!("Estimated time: about {:.0} s", secs.max(1.0))).weak());
            }
            ui.add_space(6.0);
            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(8.0).inner_margin(8.0).show(ui, |ui| {
                ui.label(RichText::new("Safety: Piccasso minimises itself while drawing. To stop at any moment, just move the mouse — it stops instantly and releases the button. Use a blank canvas and pick a brush first.").small());
            });
            ui.add_space(6.0);
            if st.run.is_some() {
                ui.add(egui::ProgressBar::new(st.progress).show_percentage());
                if ui.button("Stop").clicked() { if let Some((_, c)) = &st.run { c.store(true, Ordering::Relaxed); } }
            } else if ui.add_enabled(st.area.is_some() && (if st.source == 3 { st.replica.is_some() && st.replica_rx.is_none() } else { !paths.is_empty() }) && cfg!(feature = "autodraw"), egui::Button::new(RichText::new("Start drawing").color(Color32::WHITE).strong()).fill(crate::ui::ACCENT).min_size(vec2(0.0, 42.0))).clicked() {
                #[cfg(feature = "autodraw")]
                if st.source == 3 {
                    let (tx, rx) = channel();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let f = fit_into(st.area.unwrap(), aspect);
                    let (plan, tools, mode, sp, stp, c2) = (st.replica.as_ref().unwrap().1.clone(), st.tools.clone(), st.opacity_mode, st.speed, st.step_px, cancel.clone());
                    std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(900)); run_replica(plan, [f[0].round(), f[1].round()], tools, mode, sp, stp, c2, tx) });
                    st.run = Some((rx, cancel));
                    st.progress = 0.0;
                    st.status = "Drawing the replica… move the mouse to stop.".into();
                    minimize = Some(true);
                } else {
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
        if st.source == 3 {
            if let Some(sim) = &st.sim {
                let tex = app.textures.get(ctx, "replica-sim", sim);
                painter.image(tex.id(), rect, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                painter.text(area.left_bottom() + vec2(12.0, -12.0), egui::Align2::LEFT_BOTTOM, "Preview: simulation of the finished replica with your brush size", egui::FontId::proportional(14.0), Color32::GRAY);
            }
            return;
        }
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
