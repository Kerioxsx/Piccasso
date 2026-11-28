use piccasso_core::camera::*;
use piccasso_core::imgproc::{Gray, Rgba};

fn gray(w: usize, h: usize, f: impl Fn(usize, usize) -> u8) -> Gray { let mut g = Gray { width: w, height: h, data: vec![0; w * h] }; for y in 0..h { for x in 0..w { g.data[y * w + x] = f(x, y); } } g }
fn rgb(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Rgba { let mut d = vec![255u8; w * h * 4]; for y in 0..h { for x in 0..w { let c = f(x, y); let i = (y * w + x) * 4; d[i..i + 3].copy_from_slice(&c); } } Rgba { width: w, height: h, data: d } }
fn ids(g: &Gray) -> Vec<&'static str> { lighting_hints(&frame_stats(g)).into_iter().map(|h| h.0).collect() }

#[test]
fn lighting_hints_match_the_web() {
    let paper = |b: i32| gray(80, 60, move |x, y| (b + if ((x >> 2) + (y >> 2)) % 2 == 1 { 20 } else { -20 }) as u8);
    assert!(ids(&paper(170)).is_empty());
    assert!(ids(&paper(30)).contains(&"dark"));
    assert!(ids(&gray(80, 60, |x, y| if x > 60 && y < 20 { 255 } else { (170 + ((x + y) % 2) * 30) as u8 })).contains(&"glare"));
    assert!(ids(&gray(80, 60, |x, y| ((if x < 40 { 80 } else { 200 }) + ((x + y) % 2) * 20) as u8)).contains(&"uneven"));
    assert!(ids(&gray(80, 60, |x, _| (100.0 + (x as f64 * 1.2).round()) as u8)).contains(&"blur"));
}

#[test]
fn skin_mask_and_cutout() {
    let img = rgb(60, 40, |x, y| if x > 10 && x < 30 && y > 8 && y < 32 { [210, 150, 120] } else if x > 40 && x < 50 { [40, 60, 180] } else { [235, 233, 228] });
    let (m, share) = skin_mask(&img, 5);
    assert_eq!(m.data[20 * 60 + 20], 255);
    assert_eq!(m.data[20 * 60 + 45], 0);
    assert!(share > 0.15 && share < 0.25);
    let c = hand_cutout(&img).unwrap();
    assert_eq!(c.data[(20 * 60 + 20) * 4 + 3], 255);
    assert_eq!(c.data[(20 * 60 + 45) * 4 + 3], 0);
    assert!(hand_cutout(&rgb(20, 20, |_, _| [235, 233, 228])).is_none());
    let one = Gray { width: 5, height: 5, data: (0..25).map(|i| if i == 12 { 255 } else { 0 }).collect() };
    assert_eq!(dilate(&one, 1).data.iter().filter(|v| **v > 0).count(), 9);
}

#[test]
fn canvas_under_the_cursor() {
    use piccasso_core::tooldetect::canvas_at;
    // grey app window with a white canvas (with a black stroke on it) and a toolbar
    let img = rgb(1000, 700, |x, y| {
        if (200..900).contains(&x) && (100..600).contains(&y) { if (400..402).contains(&x) && (200..500).contains(&y) { [0, 0, 0] } else { [255, 255, 255] } }
        else if y < 40 { [60, 60, 60] } else { [200, 200, 200] }
    });
    let c = canvas_at(&img, 500, 300).expect("canvas");
    assert!((c[0] - 200).abs() <= 3 && (c[1] - 100).abs() <= 3 && (c[2] - 700).abs() <= 4 && (c[3] - 500).abs() <= 4, "{c:?}");
    assert!(canvas_at(&img, 10, 10).is_none(), "the toolbar strip is too thin to be a canvas");
}
