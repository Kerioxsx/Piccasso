use piccasso_core::filters::*;
use piccasso_core::guides::*;
use piccasso_core::imgproc::Rgba;

fn picture() -> Rgba {
    let (w, h) = (64usize, 48usize);
    let mut d = vec![0u8; w * h * 4];
    for y in 0..h { for x in 0..w {
        let v = if (x as i64 - 32).pow(2) + (y as i64 - 24).pow(2) < 100 { 20 } else if x > 44 && x < 58 && y > 8 && y < 20 { 128 } else { 240 };
        d[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
    } }
    Rgba { width: w, height: h, data: d }
}
fn at(i: &Rgba, x: usize, y: usize) -> u8 { i.data[(y * i.width + x) * 4] }

#[test]
fn every_conversion_keeps_size_and_matches_the_web_behaviour() {
    let img = picture();
    for c in CONVERSIONS {
        let p: Vec<f64> = c.params.iter().map(|q| q.3).collect();
        let o = run(c.key, &img, &p);
        assert_eq!((o.width, o.height, o.data.len()), (img.width, img.height, img.data.len()), "{}", c.key);
    }
    let t = threshold(&img, 128);
    assert_eq!(at(&t, 32, 24), 0); assert_eq!(at(&t, 2, 2), 255);
    let s = silhouette(&img);
    assert_eq!(at(&s, 32, 24), 0); assert_eq!(at(&s, 2, 2), 255);
    let e = edges(&img, 0.5, 1);
    assert_eq!(at(&e, 32, 24), 255);
    assert!((20..32).any(|x| at(&e, x, 24) == 0));
    let hf = halftone(&img, 8);
    let dark = (16..24).flat_map(|y| (24..32).map(move |x| (x, y))).filter(|&(x, y)| at(&hf, x, y) == 0).count();
    let light = (32..40).flat_map(|y| (0..8).map(move |x| (x, y))).filter(|&(x, y)| at(&hf, x, y) == 0).count();
    assert!(dark > light * 3);
    let r = in_region(&img, Some((0, 0, 32, 48)), |i| threshold(i, 128));
    assert_eq!(at(&r, 50, 12), 128);
    let u = upscale(&img, 2.0);
    assert_eq!(u.width, 128);
    assert_eq!(at(&pre_adjust(&img, 0.0, 0.0, true), 63 - 50, 12), at(&img, 50, 12));
}

#[test]
fn guides_geometry() {
    assert_eq!(clip_segment([-10.0, 50.0], [110.0, 50.0], 100.0, 100.0), Some(([0.0, 50.0], [100.0, 50.0])));
    assert_eq!(clip_segment([-10.0, -10.0], [-5.0, -20.0], 100.0, 100.0), None);
    let (w, h) = (400.0, 300.0);
    for points in 1..=3 {
        let (hy, vps) = default_vps(points, w, h);
        let s = perspective(points, &vps, hy, 12, w, h);
        let rays: Vec<_> = s.iter().filter_map(|x| if let Shape::Line { a, b, kind: "ray", .. } = x { Some((*a, *b)) } else { None }).collect();
        assert!(rays.len() >= points * 6);
        for (a, b) in rays {
            for p in [a, b] { assert!(p[0] >= -1e-6 && p[0] <= w + 1e-6 && p[1] >= -1e-6 && p[1] <= h + 1e-6); }
            assert!(vps.iter().take(points).any(|vp| ((b[0] - a[0]) * (vp[1] - a[1]) - (b[1] - a[1]) * (vp[0] - a[0])).abs() < 1e-3 * (w + h).powi(2)));
        }
    }
    let f = face(0.0, 0.0, 100.0, 140.0);
    assert!(f.iter().any(|s| matches!(s, Shape::Label { at, text } if text.starts_with("Eyes") && (at[1] - 74.0).abs() < 1e-9)));
    let g = figure(0.0, 0.0, 100.0, 800.0, 8);
    assert!(g.iter().any(|s| matches!(s, Shape::Label { at, text } if text.contains("Crotch") && (at[1] - 404.0).abs() < 1e-9)));
    assert_eq!(composition("thirds", 300.0, 90.0).len(), 4);
}
