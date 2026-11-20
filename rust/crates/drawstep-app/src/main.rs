//! DrawStep desktop (Rust + egui).
//!
//! The Rust app is a native implementation: decoding, image processing,
//! analysis, step generation, storage, keep-awake and the camera overlay all
//! run in Rust (`drawstep-core`); egui only draws the interface.
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
        let p = drawstep_core::project::import_project(screens::home::DEMO_JSON).expect("demo");
        let a = p.analysis.unwrap();
        let paths = drawstep_core::strokes::order_paths(drawstep_core::strokes::strokes_from_analysis(&a, true), a.aspect);
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let t0 = std::time::Instant::now();
        screens::autodraw::run_drawing(paths, [n[0], n[1], n[2], n[3]], a.aspect, *n.get(4).unwrap_or(&2500.0), 2.0, cancel, tx);
        for m in rx.try_iter() { if let screens::autodraw::Msg::Done(s) = m { println!("{s} ({:.2}s)", t0.elapsed().as_secs_f64()); } }
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
            .with_title("DrawStep")
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([420.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("DrawStep", options, Box::new(|cc| Ok(Box::new(app::DrawStepApp::new(cc)))))
}
