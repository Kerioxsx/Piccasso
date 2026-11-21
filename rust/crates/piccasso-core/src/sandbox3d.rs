//! 3D construction sandbox (port of `web/js/core/sandbox3d.js`): meshes, an
//! orbit camera, perspective/orthographic projection, vanishing points,
//! ground shadows and back-to-front face lists.
use serde::{Deserialize, Serialize};

pub type V3 = [f64; 3];
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] }
fn mul(a: V3, k: f64) -> V3 { [a[0] * k, a[1] * k, a[2] * k] }
fn dot(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn norm(a: V3) -> V3 { let l = (dot(a, a)).sqrt().max(1e-12); mul(a, 1.0 / l) }

/// Rotation in degrees, applied X then Y then Z.
pub fn rotate(p: V3, r: V3) -> V3 {
    let [mut x, mut y, mut z] = p;
    let (c, s) = (r[0].to_radians().cos(), r[0].to_radians().sin()); (y, z) = (y * c - z * s, y * s + z * c);
    let (c, s) = (r[1].to_radians().cos(), r[1].to_radians().sin()); (x, z) = (x * c + z * s, -x * s + z * c);
    let (c, s) = (r[2].to_radians().cos(), r[2].to_radians().sin()); (x, y) = (x * c - y * s, x * s + y * c);
    [x, y, z]
}

pub struct Mesh { pub v: Vec<V3>, pub f: Vec<Vec<usize>> }

/// Unit-size mesh resting on y = 0: "cube", "pyramid", "cylinder", "cone", "sphere".
pub fn mesh(kind: &str) -> Mesh {
    let seg = 24;
    let ring = |y: f64, r: f64| -> Vec<V3> { (0..seg).map(|i| { let a = i as f64 / seg as f64 * std::f64::consts::TAU; [a.cos() * r, y, a.sin() * r] }).collect() };
    let (mut v, mut f): (Vec<V3>, Vec<Vec<usize>>) = (vec![], vec![]);
    match kind {
        "cube" => {
            for x in [-0.5, 0.5] { for y in [0.0, 1.0] { for z in [-0.5, 0.5] { v.push([x, y, z]); } } }
            f = vec![vec![0, 1, 3, 2], vec![4, 6, 7, 5], vec![0, 4, 5, 1], vec![2, 3, 7, 6], vec![0, 2, 6, 4], vec![1, 5, 7, 3]];
        }
        "pyramid" => { v = vec![[-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [0.5, 0.0, 0.5], [-0.5, 0.0, 0.5], [0.0, 1.0, 0.0]]; f = vec![vec![0, 1, 2, 3], vec![0, 4, 1], vec![1, 4, 2], vec![2, 4, 3], vec![3, 4, 0]]; }
        "cylinder" | "cone" => {
            v.extend(ring(0.0, 0.5));
            if kind == "cylinder" {
                v.extend(ring(1.0, 0.5));
                for i in 0..seg { let j = (i + 1) % seg; f.push(vec![i, seg + i, seg + j, j]); }
                f.push((0..seg).map(|i| seg + i).collect());
            } else {
                v.push([0.0, 1.0, 0.0]);
                for i in 0..seg { f.push(vec![i, seg, (i + 1) % seg]); }
            }
            f.push((0..seg).rev().collect());
        }
        _ => {
            let rows = seg / 2;
            for r in 0..=rows { let phi = r as f64 / rows as f64 * std::f64::consts::PI; let (y, rr) = (0.5 - phi.cos() * 0.5, phi.sin() * 0.5);
                for i in 0..seg { let a = i as f64 / seg as f64 * std::f64::consts::TAU; v.push([a.cos() * rr, y, a.sin() * rr]); } }
            for r in 0..rows { for i in 0..seg { let j = (i + 1) % seg; f.push(vec![r * seg + i, (r + 1) * seg + i, (r + 1) * seg + j, r * seg + j]); } }
        }
    }
    Mesh { v, f }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Object { pub kind: String, pub pos: V3, pub rot: V3, pub scale: V3 }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Camera { pub target: V3, pub yaw: f64, pub pitch: f64, pub dist: f64, pub focal: f64, pub ortho: bool, pub ortho_size: f64 }
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Scene { pub objects: Vec<Object>, pub cam: Camera, pub light_az: f64, pub light_el: f64 }

pub fn default_scene() -> Scene {
    Scene {
        objects: vec![
            Object { kind: "cube".into(), pos: [-1.2, 0.0, 0.0], rot: [0.0, 25.0, 0.0], scale: [1.4, 1.4, 1.4] },
            Object { kind: "sphere".into(), pos: [0.9, 0.0, 0.6], rot: [0.0; 3], scale: [1.2, 1.2, 1.2] },
            Object { kind: "cylinder".into(), pos: [0.6, 0.0, -1.4], rot: [0.0; 3], scale: [0.9, 1.8, 0.9] },
        ],
        cam: Camera { target: [0.0, 0.6, 0.0], yaw: 35.0, pitch: 18.0, dist: 5.6, focal: 1.6, ortho: false, ortho_size: 6.0 },
        light_az: -50.0, light_el: 45.0,
    }
}

pub fn world_verts(o: &Object, m: &Mesh) -> Vec<V3> { m.v.iter().map(|p| add(rotate([p[0] * o.scale[0], p[1] * o.scale[1], p[2] * o.scale[2]], o.rot), o.pos)).collect() }

pub struct Basis { pub eye: V3, pub fwd: V3, pub right: V3, pub up: V3 }
pub fn basis(c: &Camera) -> Basis {
    let (yaw, pitch) = (c.yaw.to_radians(), c.pitch.to_radians());
    let dir = [pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()];
    let eye = add(c.target, mul(dir, c.dist));
    let fwd = norm(sub(c.target, eye));
    let right = norm(cross(fwd, [0.0, 1.0, 0.0]));
    Basis { eye, fwd, right, up: cross(right, fwd) }
}

/// Screen point [x, y, depth] for a world point.
pub fn project(p: V3, c: &Camera, b: &Basis, w: f64, h: f64) -> [f64; 3] {
    let d = sub(p, b.eye);
    let (x, y, z) = (dot(d, b.right), dot(d, b.up), dot(d, b.fwd));
    let s = w.min(h);
    if c.ortho { let k = s / c.ortho_size; return [w / 2.0 + x * k, h / 2.0 - y * k, z]; }
    let f = c.focal * s / 2.0;
    [w / 2.0 + x / z.max(1e-3) * f, h / 2.0 - y / z.max(1e-3) * f, z]
}

/// Where lines in direction `dir` meet on screen (None for orthographic or picture-parallel directions).
pub fn vanishing_point(dir: V3, c: &Camera, b: &Basis, w: f64, h: f64) -> Option<[f64; 2]> {
    if c.ortho { return None; }
    let (x, y, z) = (dot(dir, b.right), dot(dir, b.up), dot(dir, b.fwd));
    if z.abs() < 1e-6 { return None; }
    let f = c.focal * w.min(h) / 2.0;
    Some([w / 2.0 + x / z * f, h / 2.0 - y / z * f])
}

pub fn object_axes(o: &Object) -> [V3; 3] { [rotate([1.0, 0.0, 0.0], o.rot), rotate([0.0, 1.0, 0.0], o.rot), rotate([0.0, 0.0, 1.0], o.rot)] }
pub fn light_dir(az: f64, el: f64) -> V3 { let (a, e) = (az.to_radians(), el.to_radians()); norm([e.cos() * a.sin(), e.sin(), e.cos() * a.cos()]) }
pub fn shadow_point(p: V3, l: V3) -> Option<V3> { if l[1] <= 1e-3 { return None; } let t = p[1] / l[1]; Some([p[0] - l[0] * t, 0.0, p[2] - l[2] * t]) }

pub struct Face { pub obj: usize, pub pts: Vec<[f64; 3]>, pub shade: f64, pub depth: f64 }

/// Visible faces back to front, plus ground-shadow polygons, in screen space.
pub fn build(scene: &Scene, w: f64, h: f64) -> (Vec<Face>, Vec<Vec<[f64; 3]>>, Basis) {
    let b = basis(&scene.cam);
    let l = light_dir(scene.light_az, scene.light_el);
    let (mut faces, mut shadows) = (vec![], vec![]);
    for (oi, o) in scene.objects.iter().enumerate() {
        let m = mesh(&o.kind);
        let wv = world_verts(o, &m);
        let sv: Vec<[f64; 3]> = wv.iter().map(|p| project(*p, &scene.cam, &b, w, h)).collect();
        let centre = mul(wv.iter().fold([0.0; 3], |a, p| add(a, *p)), 1.0 / wv.len() as f64);
        for f in &m.f {
            let mut n = [0.0; 3];
            for q in 0..f.len() { let (a, c) = (wv[f[q]], wv[f[(q + 1) % f.len()]]); n[0] += (a[1] - c[1]) * (a[2] + c[2]); n[1] += (a[2] - c[2]) * (a[0] + c[0]); n[2] += (a[0] - c[0]) * (a[1] + c[1]); }
            if dot(n, n) < 1e-24 { continue; }
            let mut n = norm(n);
            let fc = mul(f.iter().fold([0.0; 3], |a, i| add(a, wv[*i])), 1.0 / f.len() as f64);
            if dot(n, sub(fc, centre)) < 0.0 { n = mul(n, -1.0); }
            let to_eye = if scene.cam.ortho { mul(b.fwd, -1.0) } else { sub(b.eye, fc) };
            if dot(n, to_eye) <= 0.0 { continue; }
            faces.push(Face { obj: oi, pts: f.iter().map(|i| sv[*i]).collect(), shade: dot(n, l).max(0.0), depth: f.iter().map(|i| sv[*i][2]).sum::<f64>() / f.len() as f64 });
        }
        for f in &m.f {
            let pts: Option<Vec<V3>> = f.iter().map(|i| shadow_point(wv[*i], l)).collect();
            if let Some(p) = pts { shadows.push(p.iter().map(|q| project(*q, &scene.cam, &b, w, h)).collect()); }
        }
    }
    faces.sort_by(|a, c| c.depth.partial_cmp(&a.depth).unwrap());
    (faces, shadows, b)
}
