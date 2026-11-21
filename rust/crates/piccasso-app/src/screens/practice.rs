//! Timed practice (desktop): gesture sessions with your own reference pictures,
//! the current project or the procedural mannequin; silhouette and mirror
//! modes; pause/next/end; history and streak saved in the data folder.
use crate::app::PiccassoApp;
use egui::{pos2, vec2, Color32, RichText, Stroke};
use piccasso_core::filters::silhouette;
use piccasso_core::imgproc::{fit_dims, flip, resize_box, Rgba};
use piccasso_core::practice::{mannequin, mannequin_bounds, plan_session, random_pose, stats, Rng, Session, PRESETS};
use std::time::{Duration, Instant};

#[derive(PartialEq, Clone, Copy)]
enum Source { Mannequin, Pictures, Project }

struct Running { plan: Vec<(Option<String>, u32)>, i: usize, left: f64, last: Instant, paused: bool, drawn: f64, rng: Rng, pose: piccasso_core::practice::Pose, label: String }

pub struct PracticeState {
    source: Source, preset: usize, custom: bool, poses: usize, seconds: u32, silhouette: bool, mirror: bool,
    run: Option<Running>, current: Option<Rgba>,
}
impl Default for PracticeState {
    fn default() -> Self { PracticeState { source: Source::Mannequin, preset: 0, custom: false, poses: 10, seconds: 30, silhouette: false, mirror: false, run: None, current: None } }
}

fn refs_dir(app: &PiccassoApp) -> std::path::PathBuf { app.data_dir.join("practice-refs") }
fn list_refs(app: &PiccassoApp) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(refs_dir(app)).map(|d| d.filter_map(|e| e.ok()).map(|e| e.path().to_string_lossy().to_string()).filter(|p| { let l = p.to_lowercase(); l.ends_with(".png") || l.ends_with(".jpg") || l.ends_with(".jpeg") || l.ends_with(".webp") }).collect()).unwrap_or_default();
    v.sort(); v
}
fn history_path(app: &PiccassoApp) -> std::path::PathBuf { app.data_dir.join("practice-history.json") }
fn load_history(app: &PiccassoApp) -> Vec<Session> { std::fs::read_to_string(history_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default() }
fn today() -> i64 { (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64 }

fn prepare(st: &PracticeState, img: &Rgba) -> Rgba {
    let (w, h) = fit_dims(img.width, img.height, 1400);
    let mut i = if w == img.width { img.clone() } else { resize_box(img, w, h) };
    if st.silhouette { i = silhouette(&i); }
    if st.mirror { i = flip(&i, true, false); }
    i
}

fn load_ref(app: &PiccassoApp, st: &PracticeState, r: &Option<String>) -> Option<Rgba> {
    match st.source {
        Source::Project => app.session.as_ref().map(|s| prepare(st, &s.src)),
        Source::Pictures => r.as_ref().and_then(|p| std::fs::read(p).ok()).and_then(|b| piccasso_core::decode_image_bytes(&b).ok()).map(|i| prepare(st, &i)),
        Source::Mannequin => None,
    }
}

fn finish(app: &mut PiccassoApp) {
    let Some(run) = app.screen_state.practice.run.take() else { return };
    if run.drawn > 5.0 {
        let mut h = load_history(app);
        h.push(Session { day: today(), poses: (run.i as u32).max(1), seconds: run.drawn.round() as u32, label: run.label });
        let _ = std::fs::create_dir_all(&app.data_dir);
        let _ = std::fs::write(history_path(app), serde_json::to_string(&h).unwrap_or_default());
        app.toast(format!("Session saved: {} min of drawing.", (run.drawn / 60.0).round().max(1.0)));
    }
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    if app.screen_state.practice.run.is_some() { return session(app, ctx); }
    let refs = list_refs(app);
    let hist = load_history(app);
    let s = stats(&hist, today());
    let has_project = app.session.is_some();
    let mut start = false;
    let mut add = false;
    let st = &mut app.screen_state.practice;
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(760.0);
            ui.label(RichText::new("Timed practice").size(26.0).strong());
            ui.label("Quick timed poses train your eye to see the whole movement first. Draw the line of action, then the shapes; stop when the time runs out.");
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.strong("What to draw");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut st.source, Source::Mannequin, "Mannequin poses");
                    ui.selectable_value(&mut st.source, Source::Pictures, format!("My pictures ({})", refs.len()));
                    if has_project { ui.selectable_value(&mut st.source, Source::Project, "Current project"); }
                });
                if st.source == Source::Pictures {
                    ui.horizontal(|ui| { if ui.button("Add pictures…").clicked() { add = true; } ui.label(RichText::new("Copied into Piccasso's data folder on this computer.").small().weak()); });
                }
                ui.checkbox(&mut st.silhouette, "Silhouette only");
                ui.checkbox(&mut st.mirror, "Mirror the reference");
            });
            ui.group(|ui| {
                ui.strong("Timing");
                ui.horizontal_wrapped(|ui| {
                    for (i, p) in PRESETS.iter().enumerate() { if ui.selectable_label(!st.custom && st.preset == i, p.0).clicked() { st.preset = i; st.custom = false; } }
                    if ui.selectable_label(st.custom, "Custom").clicked() { st.custom = true; }
                });
                if st.custom {
                    ui.add(egui::Slider::new(&mut st.poses, 1..=40).text("Poses"));
                    ui.add(egui::Slider::new(&mut st.seconds, 10..=900).step_by(10.0).text("Seconds per pose"));
                }
            });
            if crate::ui::primary(ui, "Start practice").clicked() { start = true; }
            ui.add_space(10.0);
            ui.group(|ui| {
                ui.strong("Your practice");
                ui.horizontal(|ui| {
                    for (n, l) in [(s.minutes, "minutes in total"), (s.week_minutes, "minutes this week"), (s.poses, "poses drawn"), (s.streak, "day streak")] {
                        ui.vertical(|ui| { ui.label(RichText::new(n.to_string()).size(24.0).strong()); ui.label(RichText::new(l).small().weak()); });
                        ui.add_space(14.0);
                    }
                });
                for h in hist.iter().rev().take(6) { ui.label(RichText::new(format!("{} poses · {} min · {}", h.poses, (h.seconds as f64 / 60.0).round(), h.label)).small()); }
            });
        });
    });
    if add {
        if let Some(files) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_files() {
            let dir = refs_dir(app);
            let _ = std::fs::create_dir_all(&dir);
            for f in files { if let Some(name) = f.file_name() { let _ = std::fs::copy(&f, dir.join(name)); } }
        }
    }
    if start {
        let st = &app.screen_state.practice;
        let (poses, secs, label): (usize, Vec<u32>, String) = if st.custom { (st.poses, vec![st.seconds], "custom".into()) } else { (PRESETS[st.preset].1, PRESETS[st.preset].2.to_vec(), PRESETS[st.preset].0.into()) };
        let ids: Vec<String> = match st.source { Source::Pictures => refs.clone(), Source::Project => vec!["project".into()], Source::Mannequin => (0..poses).map(|i| format!("pose-{i}")).collect() };
        if ids.is_empty() { app.toast("Add some pictures first, or choose mannequin poses."); return; }
        let seed = (Instant::now().elapsed().as_nanos() as u32) ^ (today() as u32).wrapping_mul(2654435761);
        let mut rng = Rng::new(seed);
        let pose = random_pose(&mut rng);
        let plan = plan_session(&ids, poses, &secs, seed);
        let current = load_ref(app, &app.screen_state.practice, &plan[0].0);
        let left = plan[0].1 as f64;
        app.screen_state.practice.current = current;
        app.textures.remove("practice-ref");
        app.screen_state.practice.run = Some(Running { plan, i: 0, left, last: Instant::now(), paused: false, drawn: 0.0, rng, pose, label });
    }
}

fn session(app: &mut PiccassoApp, ctx: &egui::Context) {
    let mut next = false;
    let mut end = false;
    {
        let run = app.screen_state.practice.run.as_mut().unwrap();
        let now = Instant::now();
        if !run.paused { let d = now.duration_since(run.last).as_secs_f64(); run.left -= d; run.drawn += d; }
        run.last = now;
        if run.left <= 0.0 && !run.paused { next = true; }
    }
    ctx.input(|i| { if i.key_pressed(egui::Key::Space) { if let Some(r) = app.screen_state.practice.run.as_mut() { r.paused = !r.paused; } } if i.key_pressed(egui::Key::ArrowRight) { next = true; } if i.key_pressed(egui::Key::Escape) { end = true; } });
    let st = &mut app.screen_state.practice;
    let run = st.run.as_mut().unwrap();
    egui::TopBottomPanel::bottom("practice-bar").show(ctx, |ui| {
        ui.horizontal(|ui| {
            let l = run.left.max(0.0).ceil() as u32;
            ui.label(RichText::new(if run.paused { "Paused".to_string() } else { format!("{}:{:02}", l / 60, l % 60) }).size(30.0).strong().color(if l <= 5 && !run.paused { Color32::from_rgb(0xb5, 0x46, 0x2b) } else { ui.visuals().text_color() }));
            ui.label(format!("Pose {} of {} · {} s", run.i + 1, run.plan.len(), run.plan[run.i].1));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("End").clicked() { end = true; }
                if ui.button("Next pose").clicked() { next = true; }
                if ui.button(if run.paused { "Resume" } else { "Pause" }).clicked() { run.paused = !run.paused; }
            });
        });
    });
    let (sil, mirror) = (st.silhouette, st.mirror);
    let current = st.current.clone();
    let pose = run.pose;
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 8.0, Color32::from_rgb(0xfb, 0xfa, 0xf6));
        if let Some(img) = &current {
            let tex = app.textures.get(ctx, "practice-ref", img);
            let r = crate::ui::compute_rect(area, img.width as f32, img.height as f32, &Default::default(), 10.0);
            painter.image(tex.id(), r, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            return;
        }
        let m = mannequin(&pose, [0.0, 0.0], 800.0);
        let b = mannequin_bounds(&m);
        let k = ((area.width() as f64 * 0.85) / b[2]).min((area.height() as f64 * 0.88) / b[3]).min(1.2);
        let (cx, cy) = (area.center().x as f64, area.center().y as f64);
        let pt = |p: [f64; 2]| { let x = (p[0] - (b[0] + b[2] / 2.0)) * k; pos2((cx + if mirror { -x } else { x }) as f32, (cy + (p[1] - (b[1] + b[3] / 2.0)) * k) as f32) };
        let tone = |side: i8| if sil { Color32::from_gray(34) } else if side == 0 { Color32::from_rgb(0xc4, 0xad, 0x8c) } else if (side > 0) != mirror { Color32::from_rgb(0xd9, 0xc8, 0xad) } else { Color32::from_rgb(0xa8, 0x8f, 0x6c) };
        let far = |s: i8| s != 0 && (s > 0) != mirror;
        let limb = |bone: &piccasso_core::practice::Bone| { painter.line_segment([pt(bone.a), pt(bone.b)], Stroke::new((bone.w * k) as f32, tone(bone.side))); painter.circle_filled(pt(bone.a), (bone.w * k / 2.0) as f32, tone(bone.side)); painter.circle_filled(pt(bone.b), (bone.w * k / 2.0) as f32, tone(bone.side)); };
        for bone in m.bones.iter().filter(|b| far(b.side)) { limb(bone); }
        painter.add(egui::Shape::convex_polygon(m.torso.iter().map(|p| pt(*p)).collect(), tone(0), Stroke::new((0.18 * m.unit * k) as f32, tone(0))));
        for bone in m.bones.iter().filter(|b| b.side == 0) { limb(bone); }
        painter.circle_filled(pt(m.joints["head"]), (m.head_r * k * 0.85) as f32, tone(0));
        for bone in m.bones.iter().filter(|b| !far(b.side) && b.side != 0) { limb(bone); }
        if !sil {
            let foot = if m.joints["footL"][1] > m.joints["footR"][1] { m.joints["footL"] } else { m.joints["footR"] };
            let pts: Vec<egui::Pos2> = (0..=24).map(|i| { let t = i as f64 / 24.0; let (p0, p1, p2, p3) = (m.joints["head"], m.joints["chest"], m.joints["hip"], foot);
                let c = |a: f64, b: f64, c2: f64, d: f64| (1.0 - t).powi(3) * a + 3.0 * (1.0 - t).powi(2) * t * b + 3.0 * (1.0 - t) * t * t * c2 + t.powi(3) * d;
                pt([c(p0[0], p1[0], p2[0], p3[0]), c(p0[1], p1[1], p2[1], p3[1])]) }).collect();
            painter.add(egui::Shape::dashed_line(&pts, Stroke::new(3.0_f32, crate::ui::ACCENT), 10.0, 8.0));
            painter.line_segment([pos2(area.min.x + area.width() * 0.1, pt(foot).y + 10.0), pos2(area.max.x - area.width() * 0.1, pt(foot).y + 10.0)], Stroke::new(2.0_f32, Color32::from_black_alpha(40)));
        }
        let _ = vec2(0.0, 0.0);
    });
    ctx.request_repaint_after(Duration::from_millis(200));
    if end { finish(app); return; }
    if next {
        let more = { let run = app.screen_state.practice.run.as_mut().unwrap(); run.i += 1; run.i < run.plan.len() };
        if !more { finish(app); return; }
        let (r, secs) = { let run = app.screen_state.practice.run.as_ref().unwrap(); run.plan[run.i].clone() };
        let img = load_ref(app, &app.screen_state.practice, &r);
        let st = &mut app.screen_state.practice;
        st.current = img;
        let run = st.run.as_mut().unwrap();
        run.left = secs as f64; run.last = Instant::now();
        run.pose = random_pose(&mut run.rng);
        app.textures.remove("practice-ref");
    }
}
