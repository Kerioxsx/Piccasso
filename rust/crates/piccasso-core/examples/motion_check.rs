//! Replay a replica plan as a program that reads the mouse `hz` times a second
//! would draw it, and report pixels the plan covers but the replay misses.
//! cargo run --release -p piccasso-core --example motion_check -- IMAGE W H COLORS
use piccasso_core::penpath::*;
use piccasso_core::replica::*;
fn stamp(cov: &mut [u8], w: usize, h: usize, p: &[[f64; 2]], r: f64) {
    for s in p.windows(2) {
        let d = (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1]);
        let k = (d / 0.3).ceil().max(1.0) as usize;
        for i in 0..=k {
            let t = i as f64 / k as f64;
            let (x, y) = (s[0][0] + (s[1][0] - s[0][0]) * t, s[0][1] + (s[1][1] - s[0][1]) * t);
            let ri = r.ceil() as i64;
            for yy in (y as i64 - ri)..=(y as i64 + ri) { for xx in (x as i64 - ri)..=(x as i64 + ri) {
                if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 { continue; }
                let (dx, dy) = (xx as f64 + 0.5 - x, yy as f64 + 0.5 - y);
                if dx * dx + dy * dy <= r * r { cov[yy as usize * w + xx as usize] = 1; }
            } }
        }
    }
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = piccasso_core::decode_image_bytes(&std::fs::read(&a[1]).unwrap()).unwrap();
    let (pw, ph) = piccasso_core::imgproc::fit_dims(img.width, img.height, 1400);
    let img = piccasso_core::imgproc::resize_box(&img, pw, ph);
    let (w, h): (usize, usize) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let colors: usize = a[4].parse().unwrap();
    let o = ReplicaOptions { brush_px: 5.0, line_brush_px: Some(1.0), lines: false, colors, glaze: if colors == 0 { Some(0.25) } else { None }, ..Default::default() };
    let plan = plan_replica(&img, None, w, h, &o);
    let lim = PenLimits { speed: 6000.0, hz: 60.0, tol: 0.5, ramp_px: 40.0 };
    // full replay: paint each layer from the 60 Hz readings, like the program would
    let mut out = vec![255f64; w * h * 3];
    let (mut last_r, mut last_p) = (vec![usize::MAX; w * h], vec![usize::MAX; w * h]);
    for (li, l) in plan.layers.iter().enumerate() {
        let mut pc = vec![0u8; w * h];
        for p in &l.paths { stamp(&mut pc, w, h, p, l.brush / 2.0); }
        for i in 0..w * h { if pc[i] != 0 { last_p[i] = li; } }
        let mut cov = vec![0u8; w * h];
        for p in &l.paths {
            let t = timed_points(p, &lim, 0.001);
            let mut samples = vec![t[0]];
            samples.extend(t.iter().skip(5).step_by(16).cloned());
            samples.push(*t.last().unwrap());
            stamp(&mut cov, w, h, &samples, l.brush / 2.0);
        }
        let c = l.rgb.unwrap_or([0, 0, 0]);
        for i in 0..w * h { if cov[i] != 0 { last_r[i] = li; } }
        for i in 0..w * h { if cov[i] != 0 { for k in 0..3 { out[i * 3 + k] = out[i * 3 + k] * (1.0 - l.opacity) + c[k] as f64 * l.opacity; } } }
    }
    let sim = simulate_rgb(&plan, true);
    let mut img = piccasso_core::imgproc::Rgba { width: w, height: h, data: vec![255; w * h * 4] };
    let mut bad = 0;
    for i in 0..w * h {
        let d = (0..3).map(|k| (out[i * 3 + k] - sim.data[i * 4 + k] as f64).abs()).fold(0.0, f64::max);
        if d > 60.0 { bad += 1; img.data[i * 4] = 255; img.data[i * 4 + 1] = 0; img.data[i * 4 + 2] = 0; }
        else { for k in 0..3 { img.data[i * 4 + k] = out[i * 3 + k] as u8; } }
    }
    println!("pixels differing from the plan by > 60: {bad} of {}", w * h);
    let l = &plan.layers[14];
    let p = &l.paths[0];
    let t = timed_points(p, &lim, 0.001);
    let samples: Vec<[f64; 2]> = std::iter::once(t[0]).chain(t.iter().skip(5).step_by(16).cloned()).collect();
    for (x, y) in [(130usize, 250usize), (140, 251)] { let i = y * w + x; println!("out {:?} sim {:?}", &out[i*3..i*3+3], &sim.data[i*4..i*4+3]); println!("pixel {x},{y}: plan layer {} (fine {}, rgb {:?}), replay layer {}", last_p[i], plan.layers[last_p[i]].fine, plan.layers[last_p[i]].rgb, last_r[i]); }
    for k in 2188..2200 { println!("v{k}: ({:.2},{:.2})", p[k][0], p[k][1]); }
    let mut u: Vec<[f64; 2]> = vec![];
    for &q in p.iter() { if u.last().map_or(true, |r: &[f64; 2]| (r[0]-q[0]).hypot(r[1]-q[1]) > 0.05) { u.push(q); } }
    let (v, c) = vertex_speeds(&u, &lim);
    for k in 0..u.len() { if (u[k][1]-251.5).abs() < 0.01 && (u[k][0]-108.5).abs() < 0.01 { for j in k-3..k+3 { println!("u{j}: ({:.2},{:.2}) v {:.0} corner {}", u[j][0], u[j][1], v[j], c[j]); } } }
    for (k, q) in samples.iter().enumerate() { if (q[1] - 251.0).abs() < 3.0 && q[0] > 90.0 && q[0] < 180.0 { println!("sample {k}: ({:.1},{:.1})", q[0], q[1]); } }
    std::fs::write(&a[5], piccasso_core::encode_png(&img).unwrap()).unwrap();
}
