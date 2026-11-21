//! Where the drawing time goes for a replica plan.
//! cargo run --release -p piccasso-core --example time_budget -- IMAGE W H COLORS [HZ]
use piccasso_core::penpath::*;
use piccasso_core::replica::*;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = piccasso_core::decode_image_bytes(&std::fs::read(&a[1]).unwrap()).unwrap();
    let (pw, ph) = piccasso_core::imgproc::fit_dims(img.width, img.height, 1400);
    let img = piccasso_core::imgproc::resize_box(&img, pw, ph);
    let (w, h): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let colors: usize = a[4].parse().unwrap();
    let hz: f64 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let t0 = std::time::Instant::now();
    let o = ReplicaOptions { brush_px: 5.0, line_brush_px: Some(1.0), lines: false, colors, glaze: if colors == 0 { Some(0.25) } else { None }, ..Default::default() };
    let plan = plan_replica(&img, None, w, h, &o);
    let analysis = t0.elapsed().as_secs_f64();
    let speed = match hz as i64 { 1000 => 24000.0, 500 => 16000.0, 250 => 10000.0, 125 => 7000.0, _ => 5000.0 };
    let lim = PenLimits { speed, hz, tol: 0.5, ramp_px: 40.0 };
    let (mut moving, mut strokes, mut fine_strokes, mut fill_moving, mut fine_moving) = (0.0, 0usize, 0usize, 0.0, 0.0);
    for l in &plan.layers {
        for p in &l.paths {
            let t = timed_points(p, &lim, 0.001).len() as f64 * 0.001;
            moving += t; strokes += 1;
            if l.fine { fine_strokes += 1; fine_moving += t; } else { fill_moving += t; }
        }
    }
    let per_stroke = (1.2 / hz).max(0.002) + 1.2 / hz + 2.0 * (1.2 / hz).min(0.004) + 0.001; // pen down/up waits + travel jump
    let tool = plan.layers.len() as f64 * 1.2; // colour, brush, size, opacity clicks
    let mut hist = [0usize; 5];
    for l in plan.layers.iter().filter(|l| l.fine) { for p in &l.paths {
        let len: f64 = p.windows(2).map(|s| (s[1][0]-s[0][0]).hypot(s[1][1]-s[0][1])).sum();
        hist[if len < 0.5 { 0 } else if len < 3.0 { 1 } else if len < 10.0 { 2 } else if len < 40.0 { 3 } else { 4 }] += 1;
    } }
    println!("fine strokes by length: dot {} | <3px {} | <10px {} | <40px {} | longer {}", hist[0], hist[1], hist[2], hist[3], hist[4]);
    println!("analysis {analysis:.1}s | layers {} | strokes {strokes} ({fine_strokes} fine)", plan.layers.len());
    println!("moving fill {fill_moving:.0}s, moving fine {fine_moving:.0}s, pen up/down {:.0}s, tool changes {tool:.0}s => total {:.0}s", strokes as f64 * per_stroke, moving + strokes as f64 * per_stroke + tool);
}
