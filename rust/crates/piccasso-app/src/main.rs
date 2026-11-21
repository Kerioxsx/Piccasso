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
mod screen_pick;
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
        screens::autodraw::run_drawing(paths, [n[0], n[1], n[2], n[3]], a.aspect, *n.get(4).unwrap_or(&2500.0), cancel, tx);
        for m in rx.try_iter() { if let screens::autodraw::Msg::Done(s) = m { println!("{s} ({:.2}s)", t0.elapsed().as_secs_f64()); } }
        return true;
    }
    // --replica-test X Y W H BRUSH TONES SLIDER0_X SLIDER0_Y SLIDER100_X SLIDER100_Y [BRUSH_X BRUSH_Y]
    // Draws an exact replica of the demo picture, then writes the expected target to replica-target.png.
    #[cfg(feature = "autodraw")]
    if args.get(1).map(|s| s.as_str()) == Some("--replica-test") {
        let n: Vec<f64> = args[2..].iter().filter_map(|s| s.parse().ok()).collect();
        // PICCASSO_IMG=<file> uses another picture; PICCASSO_GLAZE=<coat opacity> enables depth mode;
        // PICCASSO_LINE=<px> sets the line brush; PICCASSO_LINE_BTN=x,y is the line-brush tool position.
        let img = match std::env::var("PICCASSO_IMG") {
            Ok(path) => { let i = piccasso_core::decode_image_bytes(&std::fs::read(path).expect("image")).expect("decode"); let (w, h) = piccasso_core::imgproc::fit_dims(i.width, i.height, 1400); piccasso_core::imgproc::resize_box(&i, w, h) }
            Err(_) => {
                let p = piccasso_core::project::import_project(screens::home::DEMO_JSON).expect("demo");
                let src = piccasso_core::decode_source(&p.source).expect("demo image");
                piccasso_core::imgproc::prepare_image(&src, &p.prep.image(), 1280)
            }
        };
        let aspect = img.width as f64 / img.height as f64;
        let f = piccasso_core::strokes::fit_into([n[0], n[1], n[2], n[3]], aspect);
        let env_f = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
        let o = piccasso_core::replica::ReplicaOptions { tones: n[5] as usize, brush_px: n[4], overlap: 0.85, lines: false, opacity_control: true,
            glaze: env_f("PICCASSO_GLAZE"), line_brush_px: env_f("PICCASSO_LINE"), colors: env_f("PICCASSO_COLORS").unwrap_or(0.0) as usize, ..Default::default() };
        let plan = piccasso_core::replica::plan_replica(&img, None, f[2].round() as usize, f[3].round() as usize, &o);
        let sim = piccasso_core::replica::simulate(&plan, o.brush_px, true);
        let sim_rgb = piccasso_core::replica::simulate_rgb(&plan, true);
        let _ = std::fs::write("replica-sim-rgb.png", piccasso_core::encode_png(&sim_rgb).unwrap());
        let _ = std::fs::write("replica-source-rgb.png", piccasso_core::encode_png(&piccasso_core::imgproc::resize_box(&img, plan.width, plan.height)).unwrap());
        println!("plan: {} shades, simulated match {:.1}%, origin {:.0},{:.0} size {}x{}", plan.layers.len(), 100.0 * piccasso_core::replica::match_score(&sim, &plan.target, o.tones), f[0], f[1], plan.width, plan.height);
        let _ = std::fs::write("replica-target.png", piccasso_core::encode_png(&piccasso_core::imgproc::gray_to_rgba(&plan.target)).unwrap());
        let _ = std::fs::write("replica-source.png", piccasso_core::encode_png(&piccasso_core::imgproc::gray_to_rgba(&piccasso_core::replica::source_gray(&img, plan.width, plan.height))).unwrap());
        let _ = std::fs::write("replica-sim.png", piccasso_core::encode_png(&piccasso_core::imgproc::gray_to_rgba(&sim)).unwrap());
        let line_btn = std::env::var("PICCASSO_LINE_BTN").ok().and_then(|v| { let mut it = v.split(',').filter_map(|x| x.parse::<i32>().ok()); Some((it.next()?, it.next()?)) });
        // PICCASSO_SIZE_SLIDER=x0,y0,x1,y1 and PICCASSO_SIZE_RANGE=min,max set the brush-size slider.
        let nums = |k: &str| std::env::var(k).ok().map(|v| v.split(',').filter_map(|x| x.parse::<f64>().ok()).collect::<Vec<_>>()).unwrap_or_default();
        let ss = nums("PICCASSO_SIZE_SLIDER");
        let sr = nums("PICCASSO_SIZE_RANGE");
        let pt = |k: &str| { let v = nums(k); (v.len() >= 2).then(|| (v[0] as i32, v[1] as i32)) };
        let mut tools = screens::autodraw::Tools::new_default();
        tools.size_slider = if ss.len() == 4 { [Some((ss[0] as i32, ss[1] as i32)), Some((ss[2] as i32, ss[3] as i32))] } else { [None, None] };
        if sr.len() == 2 { tools.size_range = (sr[0], sr[1]); }
        tools.line_brush = line_btn;
        tools.brush = n.get(10).zip(n.get(11)).map(|(x, y)| (*x as i32, *y as i32));
        tools.slider = [Some((n[6] as i32, n[7] as i32)), Some((n[8] as i32, n[9] as i32))];
        // colour: PICCASSO_HEX=x,y (hex field) or PICCASSO_COLOR_AREAS="x,y,w,h;..." or PICCASSO_DETECT=1;
        // PICCASSO_SWATCH=x,y reads the current colour back
        tools.hex_field = pt("PICCASSO_HEX");
        tools.swatch = pt("PICCASSO_SWATCH");
        if tools.hex_field.is_some() { tools.color_mode = screens::autodraw::ColorMode::Hex; }
        if let Ok(v) = std::env::var("PICCASSO_COLOR_AREAS") {
            tools.color_areas = v.split(';').filter_map(|b| { let n: Vec<i32> = b.split(',').filter_map(|x| x.parse().ok()).collect(); (n.len() == 4).then(|| [n[0], n[1], n[2], n[3]]) }).collect();
            tools.color_mode = screens::autodraw::ColorMode::Areas;
        }
        if std::env::var("PICCASSO_DETECT").is_ok() {
            let shot = screen_pick::capture_screen().expect("screen capture");
            tools.color_areas = piccasso_core::tooldetect::detect_color_widgets(&shot);
            println!("detected colour tools: {:?}", tools.color_areas);
            tools.color_areas.sort_by_key(|a| std::cmp::Reverse(a[2] as i64 * a[3] as i64)); // the big picker first, then its sliders
            tools.color_mode = screens::autodraw::ColorMode::Areas;
        }
        // PICCASSO_CALIBRATE=x,y,w,h: measure the brush-size slider in that empty area first
        let cal = nums("PICCASSO_CALIBRATE");
        if cal.len() == 4 {
            tools.test_area = Some([cal[0] as i32, cal[1] as i32, cal[2] as i32, cal[3] as i32]);
            let cancel = std::sync::atomic::AtomicBool::new(false);
            match screens::autodraw::calibrate_size(&tools, &cancel, &|m| println!("  {m}")) {
                Ok(c) => { println!("size curve: {:?}", c.points); tools.size_curve = c; }
                Err(e) => { println!("calibration failed: {e}"); return true; }
            }
        }
        // PICCASSO_SPEEDTEST=1: find the fastest exact setting first (uses the test spot)
        let mut speed = env_f("PICCASSO_SPEED").unwrap_or(6000.0);
        if std::env::var("PICCASSO_SPEEDTEST").is_ok() {
            let cancel = std::sync::atomic::AtomicBool::new(false);
            match screens::autodraw::measure_speed(&tools, &cancel, &|m| println!("  {m}")) {
                Ok((hz, sp)) => { println!("speed: {sp:.0} px/s at {hz:.0} readings/s"); speed = sp; }
                Err(e) => { println!("speed test failed: {e}"); return true; }
            }
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let t0 = std::time::Instant::now();
        screens::autodraw::run_replica(plan, [f[0].round(), f[1].round()], tools, screens::autodraw::OpacityMode::Slider, speed, std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)), tx);
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
            .with_min_inner_size([420.0, 520.0])
            // see-through when picking places on the screen (normal screens are opaque)
            .with_transparent(true),
        ..Default::default()
    };
    eframe::run_native("Piccasso", options, Box::new(|cc| Ok(Box::new(app::PiccassoApp::new(cc)))))
}
