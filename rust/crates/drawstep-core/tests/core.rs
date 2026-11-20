use drawstep_core::capabilities::{evaluate, CameraFacts, OverlayMode};
use drawstep_core::compare::{compare_drawing, guide_mask};
use drawstep_core::imgproc::*;
use drawstep_core::lock::{apply_gesture, Gesture, LockState, ReferenceLock, View};
use drawstep_core::pipeline::{plan_project, run_pipeline};
use drawstep_core::project::*;
use drawstep_core::shared::shared;
use drawstep_core::steps::{fmt1, paper_layout};
use drawstep_core::store::ProjectStore;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const DEMO: &str = include_str!("../../../../samples/demo-project.drawstep.json");

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("drawstep-test-{tag}-{}", uuid::Uuid::new_v4()));
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
    assert!(import_project(&v.to_string()).unwrap_err().to_string().contains("newer DrawStep"));
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
    let src = drawstep_core::decode_source(&p.source).unwrap();
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
    let mut a = drawstep_core::analysis::Analysis::default();
    a.silhouettes.push(drawstep_core::analysis::Silhouette { object_id: "o".into(), closed: true, points: vec![[0.2, 0.2], [0.8, 0.2], [0.8, 0.8], [0.2, 0.8]] });
    let layout = paper_layout(&PaperCfg { preset: "square".into(), orientation: "auto".into(), margin_mm: 20.0 }, &sh.paper.presets, 1.0, 15.0);
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
