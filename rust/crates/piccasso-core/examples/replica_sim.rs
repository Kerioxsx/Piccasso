//! Simulate replica plans for any picture without moving the mouse.
//! cargo run --release -p piccasso-core --example replica_sim -- IMAGE OUT_DIR HEIGHT_PX
//! Writes one simulated PNG per configuration and prints accuracy against the
//! original picture and the expected drawing time.
use piccasso_core::imgproc::{fit_dims, gray_to_rgba, resize_box};
use piccasso_core::replica::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = piccasso_core::decode_image_bytes(&std::fs::read(&a[1]).expect("read image")).expect("decode");
    let out = std::path::Path::new(&a[2]);
    std::fs::create_dir_all(out).unwrap();
    let height: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(560);
    let w = (img.width as f64 * height as f64 / img.height as f64).round() as usize;
    let (pw, ph) = fit_dims(img.width, img.height, 1400);
    let img = resize_box(&img, pw, ph);
    let src = source_gray(&img, w, height);
    let _ = std::fs::write(out.join("original-gray.png"), piccasso_core::encode_png(&gray_to_rgba(&src)).unwrap());
    let base = ReplicaOptions { lines: false, ..Default::default() };
    let configs: Vec<(&str, ReplicaOptions)> = vec![
        ("bands-4-brush6", ReplicaOptions { tones: 4, brush_px: 6.0, ..base.clone() }),
        ("bands-8-brush4", ReplicaOptions { tones: 8, brush_px: 4.0, ..base.clone() }),
        ("depth-25-brush4", ReplicaOptions { brush_px: 4.0, glaze: Some(0.25), max_layers: 10, ..base.clone() }),
        ("depth-20-brush3", ReplicaOptions { brush_px: 3.0, glaze: Some(0.20), max_layers: 12, ..base.clone() }),
        ("depth-25-b5-line2", ReplicaOptions { brush_px: 5.0, glaze: Some(0.25), max_layers: 10, line_brush_px: Some(2.0), ..base.clone() }),
    ];
    println!("{:<18} {:>6} {:>9} {:>11} {:>10} {:>9}", "config", "layers", "strokes", "err vs src", "<=16 err", "time*");
    for (name, o) in configs {
        let plan = plan_replica(&img, None, w, height, &o);
        let sim = simulate(&plan, o.brush_px, o.opacity_control);
        let err = mean_error(&sim, &src);
        let close = sim.data.iter().zip(&src.data).filter(|(x, y)| (**x as i32 - **y as i32).abs() <= 16).count() as f64 / src.data.len() as f64;
        let (n, len) = plan_stats(&plan);
        let corners: usize = plan.layers.iter().flat_map(|l| l.paths.iter()).map(|p| p.len()).sum();
        let secs = len / 6000.0 + corners as f64 * 0.025 + n as f64 * 0.06 + plan.layers.len() as f64 * 0.7;
        println!("{name:<18} {:>6} {:>9} {:>10.1} {:>9.1}% {:>8.0}s", plan.layers.len(), n, err, close * 100.0, secs);
        let _ = std::fs::write(out.join(format!("{name}.png")), piccasso_core::encode_png(&gray_to_rgba(&sim)).unwrap());
    }
    println!("* estimated at 6000 px/s with 25 ms corner pauses");
}
