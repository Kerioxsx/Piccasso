//! Auto-draw: re-draws the guide's strokes in another drawing program by
//! moving the real mouse (feature `autodraw`). The stroke logic is Piccasso's
//! own (piccasso_core::strokes); the only external piece is the OS mouse API
//! wrapper (enigo). Safety: moving the mouse yourself stops the drawing at once.
use crate::app::{PiccassoApp, Screen};
use crate::ui::{compute_rect, primary};
use piccasso_core::imgproc::{fit_dims, resize_box, Rgba};
use piccasso_core::replica::{match_score, plan_replica, plan_stats, simulate, ReplicaOptions, ReplicaPlan};
use piccasso_core::lock::View;
use piccasso_core::strokes::{fit_into, order_paths, path_stats, strokes_from_analysis, strokes_from_lineart, Path};
use egui::{pos2, vec2, Color32, RichText, Stroke};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target { Corner(usize), Brush, LineBrush, SliderZero, SliderFull, Button(usize), FullButton, SizeMin, SizeMax, HexField, Swatch }

/// What the full-screen picker is choosing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickWhat { Tool(Target), Area, ColorArea, TestArea }

pub struct Pick { pub what: PickWhat, pub shot: Option<Rgba>, pub start: Option<(i32, i32)>, pub candidates: Vec<[i32; 4]> }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OpacityMode { None, Slider, Buttons }

/// How Piccasso chooses a paint colour in the target program.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ColorMode {
    /// Black only (no colour changes).
    #[default]
    None,
    /// Type the colour as hex into a text field.
    Hex,
    /// Click the right spot of colour wheels, squares and sliders, checked on screen.
    Areas,
}

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
    /// Measured brush widths along the size slider (replaces `size_range` when set).
    pub size_curve: piccasso_core::tooldetect::SizeCurve,
    /// Empty part of the canvas for test strokes (calibration).
    pub test_area: Option<[i32; 4]>,
    /// Press Ctrl+Z after calibrating to remove the test strokes.
    pub undo_tests: bool,
    pub color_mode: ColorMode,
    pub hex_field: Option<(i32, i32)>,
    /// Type a leading '#' in the hex field.
    pub hex_hash: bool,
    /// Colour wheels, squares and colour sliders (screen boxes), used in this order.
    pub color_areas: Vec<[i32; 4]>,
    /// Optional "current colour" swatch, read back to check the colour.
    pub swatch: Option<(i32, i32)>,
}

impl Tools {
    /// Screen point on the brush-size slider for `px`.
    pub fn new_default() -> Self { Tools { size_range: (1.0, 100.0), undo_tests: true, ..Default::default() } }
    /// Screen point at position `t` (0..1) of the size slider.
    pub fn size_at(&self, t: f64) -> Option<(i32, i32)> {
        let [Some(a), Some(b)] = self.size_slider else { return None };
        let t = 0.005 + t.clamp(0.0, 1.0) * 0.99; // a click exactly on an end point can miss the slider
        Some(((a.0 as f64 + (b.0 - a.0) as f64 * t).round() as i32, (a.1 as f64 + (b.1 - a.1) as f64 * t).round() as i32))
    }
    pub fn size_point(&self, px: f64) -> Option<(i32, i32)> {
        if let Some(t) = self.size_curve.position_for(px) { return self.size_at(t); }
        let (lo, hi) = (self.size_range.0.max(0.1), self.size_range.1.max(self.size_range.0 + 0.1));
        let px = px.clamp(lo, hi);
        let t = if self.size_log { (px / lo).ln() / (hi / lo).ln() } else { (px - lo) / (hi - lo) };
        self.size_at(t)
    }
    pub fn can_color(&self) -> bool {
        match self.color_mode { ColorMode::None => false, ColorMode::Hex => self.hex_field.is_some(), ColorMode::Areas => !self.color_areas.is_empty() }
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
    colors: usize,
    fine_detail: bool,
    cal_rx: Option<Receiver<Result<piccasso_core::tooldetect::SizeCurve, String>>>,
    speed_rx: Option<Receiver<Result<(f64, f64), String>>>,
    cal_log: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    opacity_mode: OpacityMode,
    tools: Tools,
    replica: Option<(String, piccasso_core::replica::ReplicaPlan, f64)>,
    replica_rx: Option<(String, Receiver<(piccasso_core::replica::ReplicaPlan, f64, Rgba)>)>,
    sim: Option<Rgba>,
    pub pick: Option<Pick>,
    detail: f64,
    speed: f64,
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
        AutoDrawState { pick: None, tones: 4, brush_px: 6.0, depth: true, coat: 0.25, line_px: 2.0, replica_lines: true, colors: 0, fine_detail: true, cal_rx: None, speed_rx: None, cal_log: Default::default(), opacity_mode: OpacityMode::Slider, tools: Tools::new_default(), replica: None, replica_rx: None, sim: None,
            source: 1, detail: 1.5, speed: 2500.0, paths: None, area: None, corner: [None, None], capture: None, run: None, progress: 0.0, status: String::new() }
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
            Target::HexField => self.tools.hex_field = Some(p),
            Target::Swatch => self.tools.swatch = Some(p),
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
            PickWhat::ColorArea => "Drag a box around a colour wheel, colour square or colour slider (or click a highlighted one).".to_string(),
            PickWhat::TestArea => "Drag a box over an empty part of the canvas for test strokes.".to_string(),
            PickWhat::Tool(t) => format!("Click on: {}", match t {
                Target::Brush => "the fill brush button", Target::LineBrush => "the line brush button",
                Target::SliderZero => "the opacity slider at 0%", Target::SliderFull => "the opacity slider at 100%",
                Target::SizeMin => "the brush-size slider at its smallest size", Target::SizeMax => "the brush-size slider at its largest size",
                Target::FullButton => "the 100% opacity button", Target::Button(_) => "this shade's opacity button", Target::Corner(_) => "the corner",
                Target::HexField => "the colour's hex code field", Target::Swatch => "the swatch that shows the current colour",
            }),
        };
        // detected colour widgets, highlighted so one click picks them
        if pk.what == PickWhat::ColorArea {
            if let Some(shot) = &pk.shot {
                let (sx, sy) = (area.width() / shot.width as f32, area.height() / shot.height as f32);
                for c in &pk.candidates {
                    let r = egui::Rect::from_min_size(pos2(area.min.x + c[0] as f32 * sx, area.min.y + c[1] as f32 * sy), vec2(c[2] as f32 * sx, c[3] as f32 * sy));
                    painter.rect_stroke(r, 3.0, Stroke::new(2.5_f32, Color32::from_rgb(0xf2, 0xc1, 0x4e)), egui::StrokeKind::Outside);
                }
            }
        }
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
        if let (PickWhat::Area | PickWhat::ColorArea | PickWhat::TestArea, Some(a), Some(p0)) = (pk.what, pk.start, resp.interact_pointer_pos()) {
            if let Some(start_pt) = ctx.input(|i| i.pointer.press_origin()) {
                let r = egui::Rect::from_two_pos(start_pt, p0);
                painter.rect_stroke(r, 0.0, Stroke::new(2.0_f32, Color32::from_rgb(0x7c, 0xc4, 0xb3)), egui::StrokeKind::Middle);
                let _ = a;
            }
        }
        if resp.drag_stopped() || resp.clicked() {
            // a quick click can press and release within one frame: fall back to the current position
            let b = crate::screen_pick::mouse_position();
            if let (Some(a), Some(b)) = (pk.start.or(b), b) {
                // a click (not a drag) inside a highlighted widget picks that widget
                let hit = pk.candidates.iter().find(|c| b.0 >= c[0] && b.1 >= c[1] && b.0 < c[0] + c[2] && b.1 < c[1] + c[3]).copied();
                match hit {
                    Some(c) if (a.0 - b.0).abs() + (a.1 - b.1).abs() < 6 => done = Some(Some((pk.what, (c[0], c[1]), (c[0] + c[2], c[1] + c[3])))),
                    _ => done = Some(Some((pk.what, a, b))),
                }
            }
        }
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) { done = Some(None); }
    if let Some(res) = done {
        st.pick = None;
        tex.remove("screen-shot");
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        match res {
            Some((PickWhat::Tool(t), _, b)) => st.set_target(t, b),
            Some((PickWhat::ColorArea, a, b)) | Some((PickWhat::TestArea, a, b)) => {
                let (x0, y0, x1, y1) = (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1));
                if x1 - x0 >= 4 && y1 - y0 >= 4 {
                    let r = [x0, y0, x1 - x0, y1 - y0];
                    if matches!(res, Some((PickWhat::ColorArea, _, _))) { st.tools.color_areas.push(r); st.tools.color_mode = ColorMode::Areas; } else { st.tools.test_area = Some(r); }
                } else { st.status = "The box was too small; drag a bigger one.".into(); }
            }
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
    let shot = crate::screen_pick::capture_screen();
    let candidates = match (&shot, what) { (Some(s), PickWhat::ColorArea) => piccasso_core::tooldetect::detect_color_widgets(s), _ => vec![] };
    st.pick = Some(Pick { what, shot, start: None, candidates });
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
pub static DWELL_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// How many times a second the drawing program reads the mouse (60 is safe for most).
pub static READ_HZ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(60);

#[cfg(feature = "autodraw")]
mod pen {
    use enigo::{Button, Coordinate, Direction, Enigo, Mouse};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    pub const STOPPED: &str = "Stopped: you moved the mouse.";

    pub struct Pen<'a> { pub e: Enigo, pub expected: Option<(i32, i32)>, pub cancel: &'a AtomicBool, pub speed: f64, pub dwell: Duration }

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
            std::thread::sleep(Duration::from_millis(40));
            let _ = self.e.button(Button::Left, Direction::Press);
            std::thread::sleep(Duration::from_millis(40));
            let _ = self.e.button(Button::Left, Direction::Release);
            std::thread::sleep(Duration::from_millis(150));
            Ok(())
        }
        /// Press a key combination with the platform's command key (Ctrl, or Cmd on macOS).
        pub fn shortcut(&mut self, ch: char) {
            use enigo::{Key, Keyboard};
            let m = if cfg!(target_os = "macos") { Key::Meta } else { Key::Control };
            let _ = self.e.key(m, Direction::Press);
            let _ = self.e.key(Key::Unicode(ch), Direction::Click);
            let _ = self.e.key(m, Direction::Release);
            std::thread::sleep(Duration::from_millis(60));
        }
        /// Choose a paint colour. Hex: click the field, select all, type, Enter.
        /// Areas: click the closest pixel of each colour wheel/square/slider, looking
        /// at the screen again after every click (the other pickers change), and
        /// repeat until the swatch (if given) shows the colour. Returns the colour
        /// the screen shows, when it could be read.
        pub fn select_color(&mut self, tools: &super::Tools, want: [u8; 3]) -> Result<Option<[u8; 3]>, String> {
            use piccasso_core::tooldetect::{best_pixel, picker_kind, sample};
            match tools.color_mode {
                super::ColorMode::None => Ok(None),
                super::ColorMode::Hex => {
                    let Some(f) = tools.hex_field else { return Ok(None) };
                    use enigo::{Key, Keyboard};
                    self.click(f)?;
                    self.shortcut('a');
                    let code = format!("{}{:02x}{:02x}{:02x}", if tools.hex_hash { "#" } else { "" }, want[0], want[1], want[2]);
                    let _ = self.e.text(&code);
                    std::thread::sleep(Duration::from_millis(40));
                    let _ = self.e.key(Key::Return, Direction::Click);
                    std::thread::sleep(Duration::from_millis(200));
                    Ok(tools.swatch.and_then(|sw| crate::screen_pick::capture_screen().map(|img| sample(&img, sw, 2))))
                }
                super::ColorMode::Areas => {
                    let f = |c: [u8; 3]| [c[0] as f64, c[1] as f64, c[2] as f64];
                    let rounds = if tools.swatch.is_some() { 4 } else { 2 };
                    let mut shown = None;
                    for _ in 0..rounds {
                        for a in &tools.color_areas {
                            let Some(img) = crate::screen_pick::capture_screen() else { return Err("Can't read the screen to find colours here; use the hex field instead.".into()) };
                            let kind = picker_kind(&img, *a);
                            let (p, _) = best_pixel(&img, *a, want, kind);
                            self.click(p)?;
                        }
                        if let Some(sw) = tools.swatch {
                            if let Some(img) = crate::screen_pick::capture_screen() {
                                let c = sample(&img, sw, 2);
                                shown = Some(c);
                                if piccasso_core::lines::color_dist(f(c), f(want)) < 150.0 { break; }
                            }
                        }
                    }
                    Ok(shown)
                }
            }
        }
        /// One pen-down stroke through screen points, moved continuously: the
        /// speed along the path is planned so the program (which reads the mouse
        /// only `READ_HZ` times a second and joins readings with straight lines)
        /// records the curve within half a pixel, pausing one reading only at
        /// real corners. The mouse is updated every millisecond, so there are no
        /// point-to-point jumps and no gaps.
        pub fn stroke(&mut self, pts: &[[f64; 2]]) -> Result<(), String> {
            self.check()?;
            let hz = super::READ_HZ.load(Ordering::Relaxed).max(20) as f64;
            let lim = piccasso_core::penpath::PenLimits { speed: self.speed, hz, tol: 0.5, ramp_px: 40.0 };
            let dt = 0.001;
            let timed = piccasso_core::penpath::timed_points(pts, &lim, dt);
            // pen down: the program must read the new position once (one reading
            // period), then the button press is an event it gets at once
            let read = Duration::from_secs_f64(1.2 / hz);
            self.go(pts[0][0].round() as i32, pts[0][1].round() as i32);
            std::thread::sleep(read.max(Duration::from_millis(2)) + self.dwell.min(Duration::from_millis(80)));
            let _ = self.e.button(Button::Left, Direction::Press);
            let tick = Duration::from_secs_f64((1.2 / hz).min(0.004));
            std::thread::sleep(tick);
            let t0 = Instant::now();
            let mut last = (i32::MIN, i32::MIN);
            for (k, p) in timed.iter().enumerate() {
                let due = Duration::from_secs_f64(k as f64 * dt);
                loop {
                    let el = t0.elapsed();
                    if el >= due { break; }
                    if due - el > Duration::from_micros(1500) { std::thread::sleep(Duration::from_micros(500)); } else { std::hint::spin_loop(); }
                }
                let q = (p[0].round() as i32, p[1].round() as i32);
                if q != last {
                    self.go(q.0, q.1);
                    last = q;
                }
                if k % 25 == 0 && (self.moved() || self.cancel.load(Ordering::Relaxed)) {
                    let _ = self.e.button(Button::Left, Direction::Release);
                    return Err(STOPPED.into());
                }
            }
            // pen up: hold the last point for one reading so the stroke reaches its end
            std::thread::sleep(read);
            let _ = self.e.button(Button::Left, Direction::Release);
            std::thread::sleep(tick);
            Ok(())
        }
    }
}

/// Draw `paths` (normalized) into the screen rectangle `area` (x, y, w, h).
#[cfg(feature = "autodraw")]
pub fn run_drawing(paths: Vec<Path>, area: [f64; 4], aspect: f64, speed: f64, cancel: Arc<AtomicBool>, tx: std::sync::mpsc::Sender<Msg>) {
    let e = match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(e) => e,
        Err(err) => { let _ = tx.send(Msg::Done(format!("Mouse control is not available: {err}"))); return; }
    };
    let mut pen = pen::Pen { e, expected: None, cancel: &cancel, speed, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
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

/// Measure the brush along the size slider (for sliders that show no pixel
/// numbers): set black at full opacity, then for several slider positions draw a
/// short line in the test area and measure its thickness on screen.
#[cfg(feature = "autodraw")]
pub fn calibrate_size(tools: &Tools, cancel: &AtomicBool, log: &dyn Fn(String)) -> Result<piccasso_core::tooldetect::SizeCurve, String> {
    use piccasso_core::tooldetect::{calibration_positions, sample, stroke_width, SizeCurve};
    let e = enigo::Enigo::new(&enigo::Settings::default()).map_err(|e| format!("Mouse control is not available: {e}"))?;
    let mut pen = pen::Pen { e, expected: None, cancel, speed: 1500.0, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
    let Some(ta) = tools.test_area else { return Err("Choose an empty test area first.".into()) };
    if tools.size_slider.iter().any(|p| p.is_none()) { return Err("Pick both ends of the brush-size slider first.".into()) }
    if tools.can_color() { pen.select_color(tools, [0, 0, 0])?; }
    if let Some(b) = tools.brush { pen.click(b)?; }
    if let [Some(z), Some(f)] = tools.slider { pen.click((z.0 + ((f.0 - z.0) as f64 * 0.995) as i32, z.1 + ((f.1 - z.1) as f64 * 0.995) as i32))?; }
    let shot = crate::screen_pick::capture_screen().ok_or("Can't read the screen here, so the brush can't be measured.")?;
    let paper = sample(&shot, (ta[0] + 3, ta[1] + 3), 1);
    let (cy, half) = (ta[1] + ta[3] / 2, ta[3] / 2 - 1);
    let (x0, x1) = (ta[0] + ta[2] / 5, ta[0] + ta[2] * 4 / 5);
    let mut pts = Vec::new();
    let mut strokes = 0;
    for t in calibration_positions() {
        pen.click(tools.size_at(t).unwrap())?;
        pen.stroke(&[[x0 as f64, cy as f64], [x1 as f64, cy as f64]])?;
        strokes += 1;
        std::thread::sleep(Duration::from_millis(150));
        let img = crate::screen_pick::capture_screen().ok_or("screen capture failed")?;
        let w = stroke_width(&img, (x0 + x1) / 2, cy, paper, [0, 0, 0], half);
        log(format!("slider {:>3.0}% -> {w:.0} px", t * 100.0));
        if w <= 0.0 { continue; }
        pts.push((t, w));
        if w >= (2 * half) as f64 { break; } // wider than the test area: larger sizes can't be measured here
    }
    if tools.undo_tests { for _ in 0..strokes { pen.shortcut('z'); std::thread::sleep(Duration::from_millis(80)); } }
    let c = SizeCurve::new(pts);
    if c.is_empty() { return Err("No test stroke could be seen: check the brush, colour and test area.".into()); }
    Ok(c)
}

/// Find the fastest setting this program draws exactly: draw a test zigzag and
/// wave in the test spot, fastest setting first, check it on screen (every
/// corner reached, no shortcuts), undo it, and step down until one passes.
/// Returns (mouse readings per second, top speed px/s).
#[cfg(feature = "autodraw")]
pub fn measure_speed(tools: &Tools, cancel: &AtomicBool, log: &dyn Fn(String)) -> Result<(f64, f64), String> {
    use piccasso_core::tooldetect::{score_stroke, speed_test_path, speed_trials};
    let Some(ta) = tools.test_area else { return Err("Choose an empty test spot first.".into()) };
    let e = enigo::Enigo::new(&enigo::Settings::default()).map_err(|e| format!("Mouse control is not available: {e}"))?;
    let mut pen = pen::Pen { e, expected: None, cancel, speed: 1000.0, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
    if tools.can_color() { pen.select_color(tools, [0, 0, 0])?; }
    if let Some(b) = tools.brush { pen.click(b)?; }
    let size = 3.0;
    if let Some(p) = tools.size_point(size) { pen.click(p)?; }
    if let [Some(z), Some(f)] = tools.slider { pen.click((z.0 + ((f.0 - z.0) as f64 * 0.995) as i32, z.1 + ((f.1 - z.1) as f64 * 0.995) as i32))?; }
    let path = speed_test_path(ta);
    let band = tools.size_curve.width_at(tools.size_curve.position_for(size).unwrap_or(0.0)).unwrap_or(size) / 2.0 + 2.5;
    let old_hz = READ_HZ.load(Ordering::Relaxed);
    let mut found = None;
    for (hz, speed) in speed_trials() {
        let before = crate::screen_pick::capture_screen().ok_or("Can't read the screen here, so the speed can't be tested.")?;
        READ_HZ.store(hz as u64, Ordering::Relaxed);
        pen.speed = speed;
        let r = pen.stroke(&path);
        std::thread::sleep(Duration::from_millis(200));
        let after = crate::screen_pick::capture_screen().ok_or("screen capture failed")?;
        if tools.undo_tests { pen.shortcut('z'); std::thread::sleep(Duration::from_millis(150)); }
        if let Err(e) = r { READ_HZ.store(old_hz, Ordering::Relaxed); return Err(e); }
        let (hit, stray) = score_stroke(&before, &after, &path, band);
        let pass = hit >= 0.985 && stray <= 0.02;
        log(format!("{hz:.0} readings/s, {speed:.0} px/s: line {:.1}% covered, {:.1}% off the line -> {}", hit * 100.0, stray * 100.0, if pass { "exact" } else { "not exact" }));
        if pass { found = Some((hz, speed)); break; }
    }
    match found {
        Some(f) => { READ_HZ.store(f.0 as u64, Ordering::Relaxed); Ok(f) }
        None => { READ_HZ.store(old_hz, Ordering::Relaxed); Err("Even the slowest setting wasn't exact: check the brush (no smoothing/stabiliser) and the test spot.".into()) }
    }
}

/// Draw a replica plan (pixel coordinates) at screen origin `org`: before each
/// layer, set its colour, brush, size and opacity with the recorded tools.
#[cfg(feature = "autodraw")]
pub fn run_replica(plan: ReplicaPlan, org: [f64; 2], tools: Tools, mode: OpacityMode, speed: f64, cancel: Arc<AtomicBool>, tx: std::sync::mpsc::Sender<Msg>) {
    let e = match enigo::Enigo::new(&enigo::Settings::default()) {
        Ok(e) => e,
        Err(err) => { let _ = tx.send(Msg::Done(format!("Mouse control is not available: {err}"))); return; }
    };
    let mut pen = pen::Pen { e, expected: None, cancel: &cancel, speed, dwell: Duration::from_millis(DWELL_MS.load(Ordering::Relaxed)) };
    let (total, _) = plan_stats(&plan);
    let mut done = 0usize;
    let mut state: (Option<[u8; 3]>, Option<bool>, Option<i64>, Option<i64>) = (None, None, None, None); // colour, fine, size, opacity
    let mut set_layer = |pen: &mut pen::Pen, rgb: Option<[u8; 3]>, opacity: f64, layer_index: Option<usize>, fine: bool, size_px: Option<f64>| -> Result<(), String> {
        let want = rgb.unwrap_or([0, 0, 0]);
        if tools.can_color() && state.0 != Some(want) {
            pen.select_color(&tools, want)?;
            state.0 = Some(want);
        }
        let brush = if fine { tools.line_brush.or(tools.brush) } else { tools.brush };
        if state.1 != Some(fine) || tools.line_brush.is_none() {
            if let Some(b) = brush { pen.click(b)?; }
            state.1 = Some(fine);
        }
        if let Some(px) = size_px {
            let key = (px * 10.0).round() as i64;
            if state.2 != Some(key) { if let Some(p) = tools.size_point(px) { pen.click(p)?; } state.2 = Some(key); }
        }
        let okey = (opacity * 1000.0).round() as i64;
        if state.3 != Some(okey) || mode == OpacityMode::Buttons {
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
            state.3 = Some(okey);
        }
        Ok(())
    };
    std::thread::sleep(Duration::from_millis(400));
    let mut run = || -> Result<(), String> {
        for (li, layer) in plan.layers.iter().enumerate() {
            // fills ask for half a pixel more than planned: they stay inside their area
            // by a safety margin, and the extra overlap closes hairline gaps between rows
            let size = if layer.fine { layer.brush } else { layer.brush + 0.5 };
            set_layer(&mut pen, layer.rgb, layer.opacity, Some(li), layer.fine, Some(size))?;
            for p in &layer.paths {
                let pts: Vec<[f64; 2]> = p.iter().map(|q| [org[0] + q[0], org[1] + q[1]]).collect();
                pen.stroke(&pts)?;
                done += 1;
                let _ = tx.send(Msg::Progress(done as f32 / total.max(1) as f32));
            }
        }
        if !plan.lines.is_empty() {
            set_layer(&mut pen, None, 1.0, None, true, None)?;
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
    let _ = tx.send(Msg::Done(match res { Ok(()) => format!("Finished: {} layers and {} lines drawn.", plan.layers.len(), plan.lines.len()), Err(m) => m }));
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
        let rkey = format!("{tw}x{th}|{}|{}|{}|{:?}|{}|{}|{}|{}|{}|{}", st.tones, st.brush_px, st.replica_lines, st.opacity_mode, sess.renders_hash, st.depth, st.coat, st.line_px, st.colors, st.fine_detail);
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
                    glaze: (st.depth && st.opacity_mode != OpacityMode::Buttons).then_some(st.coat), line_brush_px: Some(st.line_px), colors: st.colors, detail: st.fine_detail, ..Default::default() };
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let plan = plan_replica(&src, outline.as_ref(), tw, th, &o);
                    let sim_rgb = piccasso_core::replica::simulate_rgb(&plan, o.opacity_control);
                    let sim = simulate(&plan, o.brush_px, o.opacity_control);
                    let score = match_score(&sim, &plan.target, o.tones);
                    let _ = tx.send((plan, score, sim_rgb));
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
    if let Some(rx) = &st.cal_rx {
        if let Ok(r) = rx.try_recv() {
            match r {
                Ok(c) => { st.status = format!("Brush measured: {:.0} to {:.0} px along the slider.", c.range().unwrap().0, c.range().unwrap().1); st.tools.size_curve = c; }
                Err(e) => st.status = e,
            }
            st.cal_rx = None;
            minimize = Some(false);
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    if let Some(rx) = &st.speed_rx {
        if let Ok(r) = rx.try_recv() {
            match r {
                Ok((hz, sp)) => { st.speed = sp; st.status = format!("This program draws exactly at {sp:.0} px/s (it reads the mouse about {hz:.0} times a second)."); }
                Err(e) => st.status = e,
            }
            st.speed_rx = None;
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
                ui.checkbox(&mut st.fine_detail, "Copy every line and hatching stroke (found at full resolution)");
                ui.horizontal(|ui| {
                    ui.label("Colour:");
                    let mut col = st.colors > 0;
                    if ui.selectable_label(!col, "Black only").clicked() { col = false; }
                    if ui.selectable_label(col, "Colour").clicked() { col = true; }
                    if col && st.colors == 0 { st.colors = 8; }
                    if !col { st.colors = 0; }
                });
                if st.colors > 0 {
                    ui.add(egui::Slider::new(&mut st.colors, 2..=16).text("Paint colours"));
                    ui.label(RichText::new("Each colour is one opaque layer, lightest first. Set the colour tools below.").small().weak());
                }
                ui.checkbox(&mut st.replica_lines, "Draw the outlines on top");
                match &st.replica {
                    Some((_, plan, score)) => {
                        let (n, _) = plan_stats(plan);
                        ui.label(RichText::new(format!("Simulated match: {:.1}% of pixels within one shade · {} layers · {n} strokes", score * 100.0, plan.layers.len())).strong());
                        let pal = piccasso_core::replica::plan_palette(plan);
                        if !pal.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Paints:");
                                for c in pal {
                                    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                                    ui.painter().rect_filled(r, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
                                    ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0_f32, Color32::GRAY), egui::StrokeKind::Inside);
                                }
                            });
                        }
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
                let mut pick_box: Option<PickWhat> = None;
                let mut calibrate = false;
                let mut detect = false;
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
                ui.label(RichText::new("Slider has no pixel numbers? Let Piccasso measure it: it draws a few short test lines in an empty spot and measures them on screen.").small());
                ui.horizontal(|ui| {
                    let label = match st.tools.test_area { Some(a) => format!("Test spot ✔ ({}×{})", a[2], a[3]), None => "Choose empty test spot".into() };
                    if ui.add_enabled(can, egui::Button::new(label)).clicked() { pick_box = Some(PickWhat::TestArea); }
                    let ready = st.tools.test_area.is_some() && st.tools.size_slider.iter().all(|p| p.is_some()) && st.cal_rx.is_none();
                    if ui.add_enabled(can && ready, egui::Button::new("Measure brush sizes")).clicked() { calibrate = true; }
                });
                ui.checkbox(&mut st.tools.undo_tests, "Undo the test lines afterwards (Ctrl+Z)");
                if let Some((lo, hi)) = st.tools.size_curve.range() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Measured: {lo:.0}–{hi:.0} px · fill {:.0}% · line {:.0}% of the slider", 100.0 * st.tools.size_curve.position_for(st.brush_px).unwrap_or(0.0), 100.0 * st.tools.size_curve.position_for(st.line_px).unwrap_or(0.0))).small().strong());
                        if ui.small_button("Forget").clicked() { st.tools.size_curve = Default::default(); }
                    });
                }
                if st.colors > 0 {
                    ui.add_space(4.0);
                    ui.strong("Colour tools");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut st.tools.color_mode, ColorMode::Areas, "Colour wheel / sliders");
                        ui.selectable_value(&mut st.tools.color_mode, ColorMode::Hex, "Hex code field");
                    });
                    match st.tools.color_mode {
                        ColorMode::Hex | ColorMode::None => {
                            tool_btn(ui, Target::HexField, "Hex field", st.tools.hex_field, st.capture);
                            ui.checkbox(&mut st.tools.hex_hash, "Type a leading #");
                            ui.label(RichText::new("Piccasso clicks the field, selects all, types the code (e.g. 3fa34d) and presses Enter.").small().weak());
                        }
                        ColorMode::Areas => {
                            ui.horizontal(|ui| {
                                if ui.add_enabled(can, egui::Button::new("Find colour tools automatically")).on_hover_text("Looks at your screen for colour wheels, squares and colour sliders").clicked() { detect = true; }
                                if ui.add_enabled(can, egui::Button::new("Add one by hand")).clicked() { pick_box = Some(PickWhat::ColorArea); }
                            });
                            let mut remove = None;
                            for (i, a) in st.tools.color_areas.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("{}. {}×{} at {}, {}", i + 1, a[2], a[3], a[0], a[1]));
                                    if ui.small_button("✖").clicked() { remove = Some(i); }
                                });
                            }
                            if let Some(i) = remove { st.tools.color_areas.remove(i); }
                            ui.label(RichText::new("Order matters: the big wheel or hue picker first, then its brightness/colour sliders. Piccasso clicks the closest colour in each, looks again, and repeats until it matches.").small().weak());
                        }
                    }
                    tool_btn(ui, Target::Swatch, "Current-colour swatch (optional, checks the colour)", st.tools.swatch, st.capture);
                }
                if let Some(t) = cap { st.capture = Some((t, Instant::now())); }
                if let Some(t) = pick_tool { start_pick(st, PickWhat::Tool(t), ctx); }
                if let Some(w) = pick_box { start_pick(st, w, ctx); }
                if detect {
                    match crate::screen_pick::capture_screen() {
                        Some(shot) => {
                            let found = piccasso_core::tooldetect::detect_color_widgets(&shot);
                            st.status = if found.is_empty() { "No colour wheel or colour slider found on screen: open your program's colour panel, or add one by hand.".into() } else { format!("Found {} colour tools. Check the list (biggest first) and remove any that aren't colour pickers.", found.len()) };
                            st.tools.color_areas = found;
                        }
                        None => st.status = "Can't read the screen here: add colour tools by hand or use the hex field.".into(),
                    }
                }
                #[cfg(feature = "autodraw")]
                if calibrate {
                    let (tx, rx) = channel();
                    let tools = st.tools.clone();
                    let log = st.cal_log.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(900));
                        let cancel = AtomicBool::new(false);
                        let r = calibrate_size(&tools, &cancel, &|m| { if let Ok(mut l) = log.lock() { l.push(m); } });
                        let _ = tx.send(r);
                    });
                    st.cal_rx = Some(rx);
                    st.status = "Measuring the brush… move the mouse to stop.".into();
                    minimize = Some(true);
                }
                #[cfg(not(feature = "autodraw"))]
                let _ = calibrate;
            }
            ui.add_space(6.0);
            ui.strong("3. Speed and precision");
            ui.add(egui::Slider::new(&mut st.speed, 300.0..=24000.0).logarithmic(true).text("Speed (px/s)"));
            let mut speed_test = false;
            ui.horizontal(|ui| {
                let ok = st.tools.test_area.is_some() && st.run.is_none() && st.speed_rx.is_none() && cfg!(feature = "autodraw");
                if ui.add_enabled(ok, egui::Button::new("Test this program's speed")).on_hover_text("Draws a small zigzag in the test spot at the fastest setting, checks it on screen, undoes it and steps down until it is exact").clicked() { speed_test = true; }
                if st.tools.test_area.is_none() { ui.label(RichText::new("(choose a test spot above first)").small().weak()); }
            });
            ui.label(RichText::new("Programs that read every mouse movement (most desktop paint programs) can be drawn in 5-20x faster than ones that read it once per screen frame.").small().weak());
            #[cfg(feature = "autodraw")]
            if speed_test {
                let (tx, rx) = channel();
                let tools = st.tools.clone();
                let log = st.cal_log.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    let cancel = AtomicBool::new(false);
                    let _ = tx.send(measure_speed(&tools, &cancel, &|m| { if let Ok(mut l) = log.lock() { l.push(m); } }));
                });
                st.speed_rx = Some(rx);
                st.status = "Testing the drawing speed… move the mouse to stop.".into();
                minimize = Some(true);
            }
            #[cfg(not(feature = "autodraw"))]
            let _ = speed_test;
            let mut hz = READ_HZ.load(Ordering::Relaxed);
            if ui.add(egui::Slider::new(&mut hz, 30..=240).text("Program reads mouse (times/s)")).on_hover_text("Drawing programs read the mouse 60-240 times a second and join the readings with straight lines. Piccasso moves the mouse continuously and slows down on curves just enough that nothing is cut. Lower this if curves look flattened; raise it for a faster drawing.").changed() { READ_HZ.store(hz, Ordering::Relaxed); }
            let mut dwell = DWELL_MS.load(Ordering::Relaxed);
            if ui.add(egui::Slider::new(&mut dwell, 0..=80).text("Extra pause at pen down (ms)")).on_hover_text("Extra time for programs that miss the start of strokes. 0 is right for most.").changed() { DWELL_MS.store(dwell, Ordering::Relaxed); }
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
                    let (plan, tools, mode, sp, c2) = (st.replica.as_ref().unwrap().1.clone(), st.tools.clone(), st.opacity_mode, st.speed, cancel.clone());
                    std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(900)); run_replica(plan, [f[0].round(), f[1].round()], tools, mode, sp, c2, tx) });
                    st.run = Some((rx, cancel));
                    st.progress = 0.0;
                    st.status = "Drawing the replica… move the mouse to stop.".into();
                    minimize = Some(true);
                } else {
                    let (tx, rx) = channel();
                    let cancel = Arc::new(AtomicBool::new(false));
                    let (p2, ar, sp, c2) = (paths.clone(), st.area.unwrap(), st.speed, cancel.clone());
                    std::thread::spawn(move || { std::thread::sleep(Duration::from_millis(900)); run_drawing(p2, ar, aspect, sp, c2, tx) });
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
