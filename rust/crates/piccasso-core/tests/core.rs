use piccasso_core::capabilities::{evaluate, CameraFacts, OverlayMode};
use piccasso_core::compare::{compare_drawing, guide_mask};
use piccasso_core::imgproc::*;
use piccasso_core::lock::{apply_gesture, Gesture, LockState, ReferenceLock, View};
use piccasso_core::pipeline::{plan_project, run_pipeline};
use piccasso_core::project::*;
use piccasso_core::shared::shared;
use piccasso_core::steps::{fmt1, paper_layout};
use piccasso_core::store::ProjectStore;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const DEMO: &str = include_str!("../../../../samples/demo-project.piccasso.json");

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("piccasso-test-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn web_demo_project_loads_and_round_trips() {
    let p = import_project(DEMO).expect("demo imports");
    assert_eq!(p.format, FORMAT);
    assert!(p.steps.len() >= 5);
    assert!(p.analysis.as_ref().unwrap().objects.len() == 2);
    let again = import_project(&p.to_json()).unwrap();
    assert_eq!(p, again);
}

#[test]
fn unknown_fields_are_preserved() {
    let mut v: serde_json::Value = serde_json::from_str(DEMO).unwrap();
    v["thumb"] = "data:image/jpeg;base64,xyz".into();
    v["ai"] = serde_json::json!({ "used": true, "model": "x" });
    v["steps"][0]["futureField"] = 7.into();
    let p = import_project(&v.to_string()).unwrap();
    let out: serde_json::Value = serde_json::from_str(&p.to_json()).unwrap();
    assert_eq!(out["thumb"], v["thumb"]);
    assert_eq!(out["ai"]["model"], "x");
    assert_eq!(out["steps"][0]["futureField"], 7);
}

#[test]
fn invalid_projects_are_rejected() {
    assert!(matches!(import_project("nope"), Err(ProjectError::Json(_))));
    let e = import_project(r#"{"format":"x"}"#).unwrap_err().to_string();
    assert!(e.contains("format must be"), "{e}");
    let mut v: serde_json::Value = serde_json::from_str(DEMO).unwrap();
    v["version"] = 99.into();
    assert!(import_project(&v.to_string()).unwrap_err().to_string().contains("newer Piccasso"));
    v["version"] = 1.into();
    v["prep"]["style"] = "neon".into();
    assert!(import_project(&v.to_string()).unwrap_err().to_string().contains("prep.style"));
}

#[test]
fn step_navigation() {
    let mut p = import_project(DEMO).unwrap();
    let first = p.steps[0].id.clone();
    let second = p.steps[1].id.clone();
    p.complete_step(false);
    p.complete_step(true);
    assert_eq!(p.progress.completed, vec![first]);
    assert_eq!(p.progress.skipped, vec![second]);
    assert_eq!(p.progress.current_step, 2);
    p.goto_step(-3);
    assert_eq!(p.progress.current_step, 0);
    p.goto_step(1000);
    assert_eq!(p.progress.current_step, p.steps.len() - 1);
    p.progress.time_spent_sec = 9.0;
    p.restart_tutorial();
    assert_eq!(p.progress.current_step, 0);
    assert!(p.progress.completed.is_empty());
    assert_eq!(p.progress.time_spent_sec, 9.0);
}

#[test]
fn store_save_list_rename_duplicate_delete_resume() {
    let dir = tmpdir("store");
    let store = ProjectStore::new(&dir).unwrap();
    let mut a = import_project(DEMO).unwrap();
    a.id = "a".into();
    a.name = "A".into();
    store.save(&mut a, true).unwrap();
    std::thread::sleep(Duration::from_millis(5));
    let mut b = a.clone();
    b.id = "b".into();
    b.name = "B".into();
    store.save(&mut b, true).unwrap();
    let names: Vec<_> = store.list().unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec!["B", "A"]);
    store.rename("a", "  Apple ").unwrap();
    assert_eq!(store.load("a").unwrap().name, "Apple");
    let d = store.duplicate("a").unwrap();
    assert_ne!(d.id, "a");
    assert_eq!(d.name, "Apple (copy)");
    let mut r = store.load("a").unwrap();
    r.progress.current_step = 3;
    store.save(&mut r, true).unwrap();
    assert_eq!(store.load("a").unwrap().progress.current_step, 3);
    store.delete("a").unwrap();
    assert!(store.load("a").is_err());
    assert_eq!(store.list().unwrap().len(), 2);
    // import of a clashing id gets a new id
    let f = dir.join("x.json");
    std::fs::write(&f, b.to_json()).unwrap();
    let imported = store.import_file(&f).unwrap();
    assert_ne!(imported.id, "b");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn reference_lock_blocks_gestures_but_not_exit() {
    let mut lock = ReferenceLock::new(Duration::from_millis(1000), true);
    let v = View::default();
    assert_eq!(apply_gesture(v, Gesture::Pan { dx: 5.0, dy: 0.0 }, Some(&lock)).pan_x, 5.0);
    lock.lock();
    for g in ["pan", "zoom", "scroll", "rotate", "tap", "doubletap"] {
        assert!(!lock.allows(g), "{g}");
    }
    for g in ["exit", "unlock", "returnToStep", "brightness"] {
        assert!(lock.allows(g), "{g}");
    }
    assert_eq!(apply_gesture(v, Gesture::Zoom { factor: 2.0, cx: 0.0, cy: 0.0 }, Some(&lock)), v);
    lock.begin_unlock(Duration::from_millis(0));
    assert_eq!(lock.state, LockState::Unlocking);
    assert!((lock.tick(Duration::from_millis(500)) - 0.5).abs() < 1e-6);
    assert!(!lock.allows("pan"));
    lock.cancel_unlock();
    assert_eq!(lock.state, LockState::Locked);
    lock.begin_unlock(Duration::from_millis(1000));
    lock.tick(Duration::from_millis(2000));
    assert_eq!(lock.state, LockState::Unlocked);
    let mut quick = ReferenceLock::new(Duration::from_millis(1000), false);
    quick.lock();
    quick.begin_unlock(Duration::ZERO);
    assert!(!quick.locked());
}

#[test]
fn zoom_is_clamped_and_anchored() {
    let v = apply_gesture(View::default(), Gesture::Zoom { factor: 100.0, cx: 10.0, cy: 10.0 }, None);
    assert_eq!(v.zoom, 8.0);
    assert_eq!(v.pan_x, 10.0 - 10.0 * 8.0);
}

#[test]
fn capability_evaluation() {
    let none = evaluate(&CameraFacts { camera_feature: false, cameras: None, open_error: None });
    assert_eq!(none.mode, OverlayMode::Photo);
    let cam = evaluate(&CameraFacts { camera_feature: true, cameras: Some(1), open_error: None });
    assert_eq!(cam.mode, OverlayMode::Camera);
    assert!(cam.reasons.iter().any(|r| r.contains("not available on desktop")));
    let denied = evaluate(&CameraFacts { camera_feature: true, cameras: Some(1), open_error: Some("permission denied".into()) });
    assert_eq!(denied.mode, OverlayMode::Photo);
    assert!(denied.reasons.iter().any(|r| r.contains("permission")));
}

#[test]
fn transformations() {
    let mut img = Rgba::new(3, 2);
    img.data[0..4].copy_from_slice(&[1, 0, 0, 255]);
    let px = |i: &Rgba, x: usize, y: usize| i.data[(y * i.width + x) * 4..(y * i.width + x) * 4 + 4].to_vec();
    let r = rotate(&img, 90);
    assert_eq!((r.width, r.height), (2, 3));
    assert_eq!(px(&r, 1, 0), vec![1, 0, 0, 255]);
    assert_eq!(px(&rotate(&img, 180), 2, 1), vec![1, 0, 0, 255]);
    assert_eq!(px(&rotate(&img, 270), 0, 2), vec![1, 0, 0, 255]);
    assert_eq!(px(&flip(&img, true, false), 2, 0), vec![1, 0, 0, 255]);
    let c = crop(&Rgba::new(10, 10), &CropRect { x: 0.9, y: 0.9, w: 0.9, h: 0.9 });
    assert_eq!((c.width, c.height), (1, 1));
    assert_eq!(fit_dims(1000, 500, 200), (200, 100));
    assert_eq!(fmt1(12.25), "12.3");
}

#[test]
fn low_contrast_image_falls_back() {
    let img = Rgba::filled(64, 48, 128);
    let (a, steps) = plan_project(&img, &Prep::default(), shared());
    assert!(a.low_contrast && a.subject.whole);
    assert!(steps[0].instruction.contains("low contrast"));
}

#[test]
fn full_pipeline_on_demo_and_cancellation() {
    let p = import_project(DEMO).unwrap();
    let src = piccasso_core::decode_source(&p.source).unwrap();
    let res = run_pipeline(&src, &p.prep, shared(), 400, &|_, _| {}, &AtomicBool::new(false)).unwrap();
    assert_eq!(res.analysis.values.light_dir, "tl");
    for k in ["original", "pencil_sketch", "light", "outline", "values", "sketch"] {
        assert!(res.renders.contains_key(k), "{k}");
    }
    // identical analysis to the one the web app stored in the demo file
    assert_eq!(serde_json::to_value(&res.analysis.objects).unwrap(), serde_json::to_value(&p.analysis.as_ref().unwrap().objects).unwrap());
    let cancelled = run_pipeline(&src, &p.prep, shared(), 400, &|_, _| {}, &AtomicBool::new(true));
    assert_eq!(cancelled.err().as_deref(), Some("cancelled"));
}

#[test]
fn compare_feedback() {
    let sh = shared();
    let mut a = piccasso_core::analysis::Analysis::default();
    a.silhouettes.push(piccasso_core::analysis::Silhouette { object_id: "o".into(), closed: true, points: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]] });
    let layout = paper_layout(&PaperCfg { preset: "square".into(), orientation: "auto".into(), margin_mm: 20.0, ..Default::default() }, &sh.paper.presets, 1.0, 15.0);
    let (w, h) = (200, 200);
    let guide = guide_mask(&a, &layout, w, h);
    let draw = |sx: f64| {
        let mut b = a.clone();
        b.silhouettes[0].points = a.silhouettes[0].points.iter().map(|p| [0.5 + (p[0] - 0.5) * sx, p[1]]).collect();
        let m = guide_mask(&b, &layout, w, h);
        Gray { width: w, height: h, data: m.iter().map(|&v| if v != 0 { 60 } else { 235 }).collect() }
    };
    assert_eq!(compare_drawing(&draw(1.0), &guide, &sh.strings).observations, vec![sh.strings.compare.good.clone()]);
    assert!(compare_drawing(&draw(1.3), &guide, &sh.strings).observations.iter().any(|o| o.contains("wider")));
    let blank = Gray { width: w, height: h, data: vec![235; w * h] };
    assert_eq!(compare_drawing(&blank, &guide, &sh.strings).observations, vec![sh.strings.compare.empty.clone()]);
}

#[test]
fn iso_dates() {
    assert_eq!(iso_from_unix(0, 0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_from_unix(1_795_132_800, 5), "2026-11-20T00:00:00.005Z");
}

#[test]
fn technique_and_motion() {
    use piccasso_core::motion::motion_paths;
    use piccasso_core::steps::{color_name, grid_fractions, pressure_for};
    let sh = shared();
    assert_eq!(color_name([83, 121, 176], &sh.colors.colors), "blue");
    assert_eq!(color_name([52, 73, 104], &sh.colors.colors), "navy");
    let p = import_project(DEMO).unwrap();
    let src = piccasso_core::decode_source(&p.source).unwrap();
    let mut prep = Prep { medium: "color".into(), ..Default::default() };
    prep.paper.grid_cm = 2.0;
    let (a, steps) = plan_project(&src, &prep, sh);
    assert!(steps.iter().any(|s| s.kind == "pressure"));
    assert!(steps.iter().find(|s| s.kind == "color").unwrap().instruction.contains("blue"));
    assert!(steps[0].instruction.contains("12 across and 9 down"));
    let b = &a.technique.bands;
    assert_eq!(pressure_for(b, b[0].band, sh).grade, "4B");
    let l = paper_layout(&PaperCfg { preset: "custom".into(), width_mm: 300.0, height_mm: 200.0, orientation: "landscape".into(), margin_mm: 10.0, grid_cm: 0.0 }, &sh.paper.presets, 1.5, 15.0);
    assert_eq!((l.paper_w, l.draw_w, l.draw_h), (300.0, 270.0, 180.0));
    assert!((grid_fractions(&l, 2.0).0 - 20.0 / 270.0).abs() < 1e-12);
    let shapes = motion_paths(&a, steps.iter().find(|s| s.kind == "shapes").unwrap());
    assert!(shapes.len() >= 2 && shapes.iter().all(|s| s.kind == "shape"));
    assert!(motion_paths(&a, steps.iter().find(|s| s.kind == "values").unwrap()).iter().all(|s| s.kind == "hatch"));
}

#[test]
fn autodraw_strokes() {
    use piccasso_core::strokes::*;
    // An "L" drawn 3 px thick thins to one stroke of roughly the right length.
    let (w, h) = (40usize, 40usize);
    let mut img = Rgba::filled(w, h, 255);
    for y in 5..35 { for x in 9..12 { img.data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[0, 0, 0]); } }
    for x in 9..32 { for y in 32..35 { img.data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[0, 0, 0]); } }
    let paths = strokes_from_lineart(&img, 1.0, 5);
    assert_eq!(paths.len(), 1, "{paths:?}");
    let (down, _) = path_stats(&paths, 1.0);
    assert!((down - 50.0 / 40.0).abs() < 0.15, "length {down}");
    assert!(paths[0].len() <= 5, "simplified to a few points: {:?}", paths[0]);
    // ordering reduces travel
    let scattered = vec![vec![[0.9, 0.9], [0.95, 0.95]], vec![[0.0, 0.0], [0.05, 0.05]], vec![[0.5, 0.5], [0.55, 0.5]]];
    let (_, before) = path_stats(&scattered, 1.0);
    let (_, after) = path_stats(&order_paths(scattered, 1.0), 1.0);
    assert!(after < before);
    assert_eq!(fit_into([0.0, 0.0, 200.0, 100.0], 1.0), [50.0, 0.0, 100.0, 100.0]);
    let d = densify(&[[0.0, 0.0], [10.0, 0.0]], 2.0);
    assert_eq!(d.len(), 6);
    // demo outline strokes exist
    let p = import_project(DEMO).unwrap();
    assert_eq!(strokes_from_analysis(p.analysis.as_ref().unwrap(), false).len(), 2);
}

#[test]
fn replica_plan_is_exact_and_stays_inside_shades() {
    use piccasso_core::replica::*;
    // three flat shades: white | mid gray | dark
    let (w, h) = (90usize, 60usize);
    let mut img = Rgba::filled(w, h, 255);
    for y in 0..h { for x in 30..90 { let v = if x < 60 { 128 } else { 40 }; img.data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[v, v, v]); } }
    let o = ReplicaOptions { tones: 4, brush_px: 4.0, overlap: 0.85, lines: false, opacity_control: true, ..Default::default() };
    let plan = plan_replica(&img, None, w, h, &o);
    assert_eq!(plan.layers.len(), 2, "two shades to draw, white is paper");
    assert!(plan.layers[0].gray > plan.layers[1].gray, "lightest first");
    for l in &plan.layers {
        let expect = 1.0 - l.gray as f64 / 255.0;
        assert!((l.opacity - expect).abs() < 1e-9);
        // every stroke point lies in this shade's band of the quantized target
        for p in &l.paths { for q in p {
            let (x, y) = ((q[0] as usize).min(w - 1), (q[1] as usize).min(h - 1));
            assert_eq!(plan.target.data[y * w + x], l.gray, "stroke left its shade at {x},{y}");
        } }
    }
    let sim = simulate(&plan, o.brush_px, true);
    let score = match_score(&sim, &plan.target, o.tones);
    assert!(score > 0.97, "simulated match {score}");
    // without opacity control the shading is made by spacing
    let sparse = plan_replica(&img, None, w, h, &ReplicaOptions { opacity_control: false, ..o.clone() });
    let rows = |p: &ReplicaPlan| p.layers[0].paths.iter().map(|x| x.len()).sum::<usize>();
    assert!(rows(&sparse) < rows(&plan));
    let (n, len) = plan_stats(&plan);
    assert!(n >= 2 && len > 0.0);
}

#[test]
fn legacy_drawstep_files_still_open() {
    let mut v: serde_json::Value = serde_json::from_str(DEMO).unwrap();
    v["format"] = "drawstep-project".into();
    let p = import_project(&v.to_string()).unwrap();
    assert_eq!(p.format, FORMAT);
}

#[test]
fn depth_mode_reproduces_gradients_and_fine_lines() {
    use piccasso_core::replica::*;
    // a smooth left-to-right gradient with a thin black line through it
    let (w, h) = (120usize, 80usize);
    let mut img = Rgba::filled(w, h, 255);
    for y in 0..h { for x in 0..w {
        let mut v = 255 - (x * 200 / w) as u8;
        if y == 40 { v = 0; }
        img.data[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[v, v, v]);
    } }
    let src = source_gray(&img, w, h);
    let bands = ReplicaOptions { tones: 3, brush_px: 4.0, lines: false, ..Default::default() };
    let depth = ReplicaOptions { brush_px: 4.0, glaze: Some(0.2), max_layers: 12, line_brush_px: Some(1.0), lines: false, ..Default::default() };
    let err = |o: &ReplicaOptions| { let p = plan_replica(&img, None, w, h, o); mean_error(&simulate(&p, o.brush_px, true), &src) };
    let (eb, ed) = (err(&bands), err(&depth));
    assert!(ed < eb * 0.75, "depth {ed:.1} should clearly beat 3 flat bands {eb:.1}");
    // the 1-px line is traced as continuous fine strokes, not dotted rows
    let plan = plan_replica(&img, None, w, h, &depth);
    let fine: Vec<_> = plan.layers.iter().filter(|l| l.fine && l.opacity == 1.0).collect();
    assert!(!fine.is_empty(), "solid fine pass exists");
    let longest = fine.iter().flat_map(|l| l.paths.iter()).map(|p| (p[p.len() - 1][0] - p[0][0]).abs()).fold(0.0, f64::max);
    assert!(longest > w as f64 * 0.8, "line drawn as one long stroke, got {longest}");
    assert_eq!(fine[0].brush, 1.0);
}

#[test]
fn pen_timing_keeps_sampled_strokes_on_the_path() {
    use piccasso_core::penpath::*;
    // a circle of radius 30 px, a zigzag with sharp corners, and a serpentine fill
    let circle: Vec<[f64; 2]> = (0..=120).map(|i| { let a = i as f64 / 120.0 * std::f64::consts::TAU; [100.0 + 30.0 * a.cos(), 100.0 + 30.0 * a.sin()] }).collect();
    let zig: Vec<[f64; 2]> = (0..12).map(|i| [i as f64 * 15.0, if i % 2 == 0 { 0.0 } else { 40.0 }]).collect();
    let mut serp = Vec::new();
    for r in 0..10 { let y = r as f64 * 4.0; if r % 2 == 0 { serp.push([0.0, y]); serp.push([200.0, y]); } else { serp.push([200.0, y]); serp.push([0.0, y]); } }
    let lim = PenLimits { speed: 4000.0, hz: 60.0, tol: 0.5, ramp_px: 40.0 };
    for path in [&circle, &zig, &serp] {
        let t = timed_points(path, &lim, 0.001);
        let err = sampled_error(path, &t, 0.001, lim.hz);
        assert!(err <= 1.0, "sampled error {err}");
        // straight runs still go fast: total time well below a fixed-dwell pen
        assert!(t.len() as f64 * 0.001 < 3.0, "too slow");
    }
    // a corner whose point is listed twice must still be held (it was skipped once)
    let dup = vec![[0.0, 0.0], [70.0, 0.0], [70.0, 0.0], [70.0, 4.0], [0.0, 4.0], [0.0, 4.0], [0.0, 8.0], [70.0, 8.0]];
    let t = timed_points(&dup, &lim, 0.001);
    assert!(sampled_error(&dup, &t, 0.001, 60.0) <= 1.0, "duplicate corner cut: {}", sampled_error(&dup, &t, 0.001, 60.0));
    // naive constant-speed motion at the same top speed would cut corners badly
    let naive: Vec<[f64; 2]> = piccasso_core::strokes::densify(&zig, 4000.0 * 0.001);
    assert!(sampled_error(&zig, &naive, 0.001, 60.0) > 5.0);
}

#[test]
fn replica_detail_and_colour() {
    use piccasso_core::replica::*;
    use piccasso_core::imgproc::Rgba;
    // white paper, a 1 px dark line, a green disc and a gray block
    let (w, h) = (240, 240);
    let mut d = vec![255u8; w * h * 4];
    for y in 0..h { for x in 0..w {
        let i = (y * w + x) * 4;
        if x == 60 && y > 20 && y < 220 { d[i] = 20; d[i + 1] = 20; d[i + 2] = 20; }
        if (x as i32 - 160).pow(2) + (y as i32 - 70).pow(2) < 900 { d[i] = 60; d[i + 1] = 190; d[i + 2] = 50; }
        if x > 120 && x < 220 && y > 140 && y < 220 { d[i] = 120; d[i + 1] = 120; d[i + 2] = 120; }
    } }
    let img = Rgba { width: w, height: h, data: d };
    // black: the thin line becomes one smooth line stroke at near-full darkness
    let o = ReplicaOptions { brush_px: 4.0, glaze: Some(0.25), line_brush_px: Some(1.0), lines: false, ..Default::default() };
    let plan = plan_replica(&img, None, 80, 80, &o);
    let line_layers: Vec<_> = plan.layers.iter().filter(|l| l.fine && l.opacity >= 0.8).collect();
    assert!(!line_layers.is_empty());
    let longest = line_layers.iter().flat_map(|l| l.paths.iter()).map(|p| p.windows(2).map(|s| (s[1][0]-s[0][0]).hypot(s[1][1]-s[0][1])).sum::<f64>()).fold(0.0, f64::max);
    assert!(longest > 55.0, "line length {longest}");
    // colour: the green disc keeps a green paint
    let oc = ReplicaOptions { brush_px: 4.0, line_brush_px: Some(1.0), colors: 4, lines: false, ..Default::default() };
    let pc = plan_replica(&img, None, 80, 80, &oc);
    let pal = plan_palette(&pc);
    assert!(pal.iter().any(|c| c[1] as i32 - c[0] as i32 > 80 && c[1] as i32 - c[2] as i32 > 80), "{pal:?}");
    let sim = simulate_rgb(&pc, true);
    let src = piccasso_core::imgproc::resize_box(&img, 80, 80);
    assert!(mean_error_rgb(&sim, &src) < 12.0, "{}", mean_error_rgb(&sim, &src));
}

fn hsv_rgb(h: f64, s: f64, v: f64) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as i32 { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
    [((r + m) * 255.0).round() as u8, ((g + m) * 255.0).round() as u8, ((b + m) * 255.0).round() as u8]
}

#[test]
fn colour_pickers_are_found_and_read() {
    use piccasso_core::tooldetect::*;
    use piccasso_core::imgproc::Rgba;
    // a fake screen: gray UI, a few icons, an HSV wheel (100 px) at (40,40) and a
    // brightness slider (200x16) at (40,170) showing the wheel's chosen hue
    let (w, h) = (400usize, 260usize);
    let mut d = vec![0u8; w * h * 4];
    let mut put = |x: usize, y: usize, c: [u8; 3]| { let i = (y * w + x) * 4; d[i..i + 3].copy_from_slice(&c); d[i + 3] = 255; };
    for y in 0..h { for x in 0..w { put(x, y, [236, 236, 236]); } }
    for y in 10..30 { for x in 300..320 { put(x, y, [200, 40, 40]); } } // a red icon
    for y in 0..100 { for x in 0..100 {
        let (dx, dy) = (x as f64 - 50.0, y as f64 - 50.0);
        let r = dx.hypot(dy);
        if r <= 50.0 { put(40 + x, 40 + y, hsv_rgb((dy.atan2(dx).to_degrees() + 360.0) % 360.0, r / 50.0, 1.0)); }
    } }
    for y in 0..16 { for x in 0..200 { put(40 + x, 170 + y, hsv_rgb(120.0, 0.8, x as f64 / 199.0)); } }
    let img = Rgba { width: w, height: h, data: d };
    let found = detect_color_widgets(&img);
    let covers = |a: &Area, x: i32, y: i32| x >= a[0] && y >= a[1] && x < a[0] + a[2] && y < a[1] + a[3];
    assert!(found.iter().any(|a| covers(a, 90, 90)), "wheel not found: {found:?}");
    assert!(found.iter().any(|a| covers(a, 140, 178)), "slider not found: {found:?}");
    assert!(!found.iter().any(|a| covers(a, 310, 20)), "icon taken for a picker: {found:?}");
    let wheel = [40, 40, 100, 100];
    let slider = [40, 170, 200, 16];
    assert_eq!(picker_kind(&img, wheel), PickerKind::Hue);
    assert_eq!(picker_kind(&img, slider), PickerKind::Shade);
    // a dark green: the wheel gives its hue/saturation, the slider its darkness
    let want = hsv_rgb(120.0, 0.8, 0.4);
    let ((x, y), _) = best_pixel(&img, wheel, want, PickerKind::Hue);
    let c = sample(&img, (x, y), 0);
    assert!(c[1] > c[0] + 100 && c[1] > c[2] + 100, "{c:?}");
    let ((sx, _), d) = best_pixel(&img, slider, want, PickerKind::Shade);
    assert!((sx - (40 + 80)).abs() <= 3, "slider x {sx}");
    assert!(d < 30.0);
}

#[test]
fn size_curve_without_numbers() {
    use piccasso_core::tooldetect::SizeCurve;
    // a slider whose size grows with the square of the position (1..100 px)
    let f = |t: f64| 1.0 + 99.0 * t * t;
    let pts = piccasso_core::tooldetect::calibration_positions().into_iter().map(|t| (t, f(t).round())).collect();
    let c = SizeCurve::new(pts);
    for want in [2.0, 6.0, 12.0, 30.0, 70.0] {
        let t = c.position_for(want).unwrap();
        let got = f(t);
        assert!((got - want).abs() / want < 0.2, "want {want} got {got}");
    }
}

#[test]
fn toolbars_and_plain_sliders_are_not_colour_pickers() {
    use piccasso_core::tooldetect::detect_color_widgets;
    use piccasso_core::imgproc::Rgba;
    // a pale toolbar with a barely visible gradient, dark text/icons on it, and a
    // flat gray slider with a dark knob
    let (w, h) = (400usize, 200usize);
    let mut d = vec![255u8; w * h * 4];
    for y in 0..h { for x in 0..w {
        let i = (y * w + x) * 4;
        let mut c = [236u8, 236, 236];
        if y < 60 { let t = (x as f64 / w as f64 * 10.0) as u8; c = [205 + t, 225 + t / 2, 245]; }
        if y >= 20 && y < 40 && x % 30 < 12 { c = [30, 30, 30]; }
        if y >= 120 && y < 136 && x >= 40 && x < 300 { c = [204, 204, 204]; }
        if y >= 116 && y < 140 && x >= 150 && x < 160 { c = [50, 50, 50]; }
        d[i..i + 3].copy_from_slice(&c);
    } }
    let found = detect_color_widgets(&Rgba { width: w, height: h, data: d });
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn soft_edges_count_by_ink() {
    use piccasso_core::tooldetect::stroke_width;
    use piccasso_core::imgproc::Rgba;
    // a 3.6 px line drawn with anti-aliasing: rows 10..13 full, 14 at 60 %
    let (w, h) = (20usize, 30usize);
    let mut d = vec![255u8; w * h * 4];
    for y in 0..h { for x in 0..w {
        let cov: f64 = match y { 10..=12 => 1.0, 13 => 0.6, 9 => 0.0, _ => 0.0 };
        let v = (255.0 * (1.0 - cov)).round() as u8;
        let i = (y * w + x) * 4; d[i] = v; d[i + 1] = v; d[i + 2] = v;
    } }
    let wdt = stroke_width(&Rgba { width: w, height: h, data: d }, 10, 11, [255, 255, 255], [0, 0, 0], 20);
    assert!((wdt - 3.6).abs() < 0.1, "{wdt}");
}

#[test]
fn speed_test_scoring_spots_cut_corners() {
    use piccasso_core::tooldetect::*;
    use piccasso_core::imgproc::Rgba;
    let a = [0, 0, 240, 160];
    let path = speed_test_path(a);
    let blank = Rgba { width: 260, height: 180, data: vec![255; 260 * 180 * 4] };
    let draw = |pts: &[[f64; 2]]| {
        let mut img = blank.clone();
        for q in piccasso_core::strokes::densify(pts, 0.3) {
            for dy in -1..=1 { for dx in -1..=1 {
                let (x, y) = (q[0] as i32 + dx, q[1] as i32 + dy);
                if x >= 0 && y >= 0 && x < 260 && y < 180 { let i = (y as usize * 260 + x as usize) * 4; img.data[i] = 0; img.data[i + 1] = 0; img.data[i + 2] = 0; }
            } }
        }
        img
    };
    let (hit, stray) = score_stroke(&blank, &draw(&path), &path, 3.0);
    assert!(hit > 0.99 && stray < 0.01, "exact: {hit} {stray}");
    // a program that read the mouse too rarely: every other point lost
    let cut: Vec<[f64; 2]> = path.iter().step_by(2).cloned().collect();
    let (hit2, _) = score_stroke(&blank, &draw(&cut), &path, 3.0);
    assert!(hit2 < 0.97, "cut corners not noticed: {hit2}");
}
