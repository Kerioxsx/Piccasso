//! Piccasso desktop (Rust + egui).
//!
//! The Rust app is a native implementation: decoding, image processing,
//! analysis, step generation, storage, keep-awake and the camera overlay all
//! run in Rust (`piccasso-core`); egui only draws the interface.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod bridge;
mod camera;
mod export;
mod screens;
mod settings;
mod ui;

/// Developer/QA entry points (no window):
///   --autodraw-test X Y W H [speed]  draw the demo's outline strokes into that screen area with the real mouse
///   --serve-phone SECONDS            run the phone bridge and print its URL
fn dev_commands() -> bool {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(feature = "autodraw")]
    if args.get(1).map(|s| s.as_str()) == Some("--autodraw-test") {
        let n: Vec<f64> = args[2..].iter().filter_map(|s| s.parse().ok()).collect();
        let p = piccasso_core::project::import_project(screens::home::DEMO_JSON).expect("demo");
        let a = p.analysis.unwrap();
        let paths = piccasso_core::strokes::order_paths(piccasso_core::strokes::strokes_from_analysis(&a, true), a.aspect);
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let t0 = std::time::Instant::now();
        screens::autodraw::run_drawing(paths, [n[0], n[1], n[2], n[3]], a.aspect, *n.get(4).unwrap_or(&2500.0), 2.0, cancel, tx);
        for m in rx.try_iter() { if let screens::autodraw::Msg::Done(s) = m { println!("{s} ({:.2}s)", t0.elapsed().as_secs_f64()); } }
        return true;
    }
    // --replica-test X Y W H BRUSH TONES SLIDER0_X SLIDER0_Y SLIDER100_X SLIDER100_Y [BRUSH_X BRUSH_Y]
    // Draws an exact replica of the demo picture, then writes the expected target to replica-target.png.
    #[cfg(feature = "autodraw")]
    if args.get(1).map(|s| s.as_str()) == Some("--replica-test") {
        let n: Vec<f64> = args[2..].iter().filter_map(|s| s.parse().ok()).collect();
        let p = piccasso_core::project::import_project(screens::home::DEMO_JSON).expect("demo");
        let src = piccasso_core::decode_source(&p.source).expect("demo image");
        let img = piccasso_core::imgproc::prepare_image(&src, &p.prep.image(), 1280);
        let a = p.analysis.unwrap();
        let f = piccasso_core::strokes::fit_into([n[0], n[1], n[2], n[3]], a.aspect);
        let o = piccasso_core::replica::ReplicaOptions { tones: n[5] as usize, brush_px: n[4], overlap: 0.85, lines: false, opacity_control: true };
        let plan = piccasso_core::replica::plan_replica(&img, None, f[2].round() as usize, f[3].round() as usize, &o);
        let sim = piccasso_core::replica::simulate(&plan, o.brush_px, true);
        println!("plan: {} shades, simulated match {:.1}%, origin {:.0},{:.0} size {}x{}", plan.layers.len(), 100.0 * piccasso_core::replica::match_score(&sim, &plan.target, o.tones), f[0], f[1], plan.width, plan.height);
        let _ = std::fs::write("replica-target.png", piccasso_core::encode_png(&piccasso_core::imgproc::gray_to_rgba(&plan.target)).unwrap());
        let tools = screens::autodraw::Tools { brush: n.get(10).zip(n.get(11)).map(|(x, y)| (*x as i32, *y as i32)), slider: [Some((n[6] as i32, n[7] as i32)), Some((n[8] as i32, n[9] as i32))], buttons: vec![], full: None };
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = std::time::Instant::now();
        screens::autodraw::run_replica(plan, [f[0].round(), f[1].round()], tools, screens::autodraw::OpacityMode::Slider, 6000.0, 2.0, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)), tx);
        for m in rx.try_iter() { if let screens::autodraw::Msg::Done(s) = m { println!("{s} ({:.1}s)", t0.elapsed().as_secs_f64()); } }
        return true;
    }
    #[cfg(feature = "phone")]
    if args.get(1).map(|s| s.as_str()) == Some("--serve-phone") {
        let mut st = bridge::PhoneState::default();
        st.start();
        match st.url() { Some(u) => println!("{u}"), None => { println!("error: {:?}", st.error); return true; } }
        let secs: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
        let t0 = std::time::Instant::now();
        while t0.elapsed().as_secs() < secs {
            if let Some((g, img)) = st.frame_after(st.last_gen) { st.last_gen = g; println!("frame {g}: {}x{}", img.width, img.height); }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        return true;
    }
    let _ = args;
    false
}

fn main() -> eframe::Result<()> {
    if dev_commands() {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Piccasso")
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([420.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("Piccasso", options, Box::new(|cc| Ok(Box::new(app::PiccassoApp::new(cc)))))
}
