use piccasso_core::practice::*;

#[test]
fn session_plans_and_mannequin() {
    let refs: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
    let p = plan_session(&refs, 7, PRESETS[4].2, 3);
    let first: std::collections::BTreeSet<_> = p[..3].iter().map(|x| x.0.clone().unwrap()).collect();
    assert_eq!(first.len(), 3);
    assert_eq!(p.iter().map(|x| x.1).collect::<Vec<_>>(), vec![30, 30, 30, 60, 60, 120, 120]);
    let stand = Pose { upper_arm_l: 10.0, upper_arm_r: -10.0, ..Default::default() };
    let b = mannequin_bounds(&mannequin(&stand, [0.0, 0.0], 400.0));
    assert!(b[3] > 360.0 && b[3] < 440.0, "{}", b[3]);
    let mut rng = Rng::new(7);
    for _ in 0..50 {
        let m = mannequin(&random_pose(&mut rng), [0.0, 0.0], 400.0);
        let (h, k) = (m.joints["hipL"], m.joints["kneeL"]);
        assert!(((h[0] - k[0]).hypot(h[1] - k[1]) - 95.0).abs() < 1e-9);
    }
}

#[test]
fn stats_and_skill_tree() {
    let s = |day, poses, seconds| Session { day, poses, seconds, label: String::new() };
    let st = stats(&[s(98, 8, 600), s(99, 4, 1200), s(100, 10, 300), s(70, 5, 60)], 100);
    assert_eq!(st, Stats { sessions: 4, poses: 27, minutes: 36, week_minutes: 35, streak: 3 });
    assert_eq!(stats(&[s(99, 1, 60)], 100).streak, 1);
    let d = lessons();
    assert_eq!(d.categories.len(), 10);
    assert_eq!(d.lessons.len(), 30);
    let mut p = SkillProgress::default();
    assert_eq!(next_lesson(d, &p).unwrap().level, "beginner");
    let boxes = d.lessons.iter().find(|l| l.id == "shapes-boxes").unwrap();
    assert!(!unlocked(boxes, &p));
    p.done.insert("shapes-circles".into(), "2026-11-21".into());
    assert!(unlocked(boxes, &p));
    assert_eq!(milestones(d, &p), vec!["First lesson done".to_string()]);
}

#[test]
fn sandbox_vanishing_points_and_shadows() {
    use piccasso_core::sandbox3d::*;
    let sc = default_scene();
    let (w, h) = (800.0, 600.0);
    let cube = &sc.objects[0];
    let b = basis(&sc.cam);
    let sv: Vec<_> = world_verts(cube, &mesh("cube")).iter().map(|p| project(*p, &sc.cam, &b, w, h)).collect();
    let vp = vanishing_point(object_axes(cube)[0], &sc.cam, &b, w, h).unwrap();
    for i in 0..4 { let (a, c) = (sv[i], sv[i + 4]); let cr = (c[0] - a[0]) * (vp[1] - a[1]) - (c[1] - a[1]) * (vp[0] - a[0]); assert!(cr.abs() / ((c[0] - a[0]).hypot(c[1] - a[1]) * (vp[0] - a[0]).hypot(vp[1] - a[1])) < 1e-6); }
    let s = shadow_point([0.0, 1.0, 0.0], light_dir(0.0, 45.0)).unwrap();
    assert!(s[1].abs() < 1e-9 && s[2] < 0.0);
    let (faces, shadows, _) = build(&sc, w, h);
    assert!(faces.len() > 20 && shadows.len() > 20);
    assert!(faces.windows(2).all(|f| f[0].depth >= f[1].depth));
}

#[test]
fn inspect_advice_and_symmetry() {
    use piccasso_core::inspect::*;
    use piccasso_core::imgproc::Rgba;
    let mk = |f: &dyn Fn(usize) -> u8| { let (w, h) = (100usize, 100usize); let mut d = vec![255u8; w * h * 4]; for y in 0..h { for x in 0..w { let v = f(x); d[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[v, v, v]); } } Rgba { width: w, height: h, data: d } };
    let r = value_stats(&mk(&|x| if x < 50 { 10 } else { 245 }));
    let d = value_stats(&mk(&|x| if x < 50 { 90 } else { 220 }));
    let tips: String = compare_values(&r, &d).iter().map(|t| t.1.clone()).collect();
    assert!(tips.contains("darkest darks are lighter") && tips.contains("less contrast"));
    assert_eq!(compare_values(&r, &r)[0].0, "good");
    assert!(symmetry(&mk(&|x| ((x as f64 - 49.5).abs() * 3.0) as u8), 0.5, 400).0 > 0.99);
    assert_eq!(sample(&mk(&|_| 128), 1, 1), ("#808080".to_string(), 50));
}
