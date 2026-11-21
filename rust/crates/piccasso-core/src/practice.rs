//! Timed practice and the skill tree (ports of `web/js/core/practice.js` and
//! `web/js/core/skills.js`): session plans, the procedural mannequin, practice
//! statistics, and lesson unlocking over `shared/lessons.en.json`.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// mulberry32: small seeded generator (same sequence as the web version).
pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self { Rng(seed) }
    pub fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64) / 4294967296.0
    }
}

pub fn shuffle<T: Clone>(list: &[T], rng: &mut Rng) -> Vec<T> {
    let mut a = list.to_vec();
    for i in (1..a.len()).rev() { let j = (rng.next() * (i + 1) as f64) as usize; a.swap(i, j); }
    a
}

/// (label, poses, seconds per pose…)
pub const PRESETS: &[(&str, usize, &[u32])] = &[
    ("30-second gestures", 10, &[30]),
    ("1-minute gestures", 8, &[60]),
    ("2-minute poses", 6, &[120]),
    ("5-minute studies", 4, &[300]),
    ("Warm-up ladder (30 s to 5 min)", 8, &[30, 30, 30, 60, 60, 120, 120, 300]),
];

/// Every reference once before any repeats; times from the ladder.
pub fn plan_session(refs: &[String], poses: usize, seconds: &[u32], seed: u32) -> Vec<(Option<String>, u32)> {
    let mut rng = Rng::new(seed);
    let mut pool: Vec<String> = Vec::new();
    (0..poses).map(|i| {
        if pool.is_empty() { pool = shuffle(refs, &mut rng); }
        (pool.pop(), seconds[i.min(seconds.len() - 1)])
    }).collect()
}

// ----- mannequin -----
#[derive(Debug, Clone, Copy, Default)]
pub struct Pose { pub torso: f64, pub neck: f64, pub upper_arm_l: f64, pub fore_arm_l: f64, pub upper_arm_r: f64, pub fore_arm_r: f64, pub thigh_l: f64, pub shin_l: f64, pub thigh_r: f64, pub shin_r: f64 }

pub fn random_pose(rng: &mut Rng) -> Pose {
    let mut r = |lo: f64, hi: f64| lo + (hi - lo) * rng.next();
    let mut p = Pose { torso: r(-20.0, 20.0), neck: r(-20.0, 20.0), upper_arm_l: r(-150.0, 30.0), fore_arm_l: r(0.0, 130.0), upper_arm_r: r(-30.0, 150.0), fore_arm_r: r(-130.0, 0.0),
        thigh_l: r(-70.0, 30.0), shin_l: r(-110.0, 0.0), thigh_r: r(-30.0, 70.0), shin_r: r(0.0, 110.0) };
    if p.thigh_l.abs() > 60.0 && p.thigh_r.abs() > 60.0 { p.thigh_r = r(-10.0, 10.0); }
    p
}

/// A limb: from, to, width, side (0 centre, 1 figure's left, -1 right).
pub struct Bone { pub a: [f64; 2], pub b: [f64; 2], pub w: f64, pub side: i8 }
pub struct Mannequin { pub joints: BTreeMap<&'static str, [f64; 2]>, pub bones: Vec<Bone>, pub torso: [[f64; 2]; 4], pub head_r: f64, pub unit: f64 }

fn step(from: [f64; 2], ang: f64, len: f64) -> [f64; 2] { let r = ang.to_radians(); [from[0] + r.sin() * len, from[1] + r.cos() * len] }

/// Skeleton for a pose: about 8 heads tall (`h` px) with the hips at `hip`.
pub fn mannequin(p: &Pose, hip: [f64; 2], h: f64) -> Mannequin {
    let u = h / 8.0;
    let mut j = BTreeMap::new();
    j.insert("hip", hip);
    let chest = step(hip, 180.0 + p.torso, 3.0 * u); j.insert("chest", chest);
    let neck = step(chest, 180.0 + p.torso + p.neck, 0.4 * u); j.insert("neck", neck);
    j.insert("head", step(neck, 180.0 + p.torso + p.neck, 0.5 * u));
    let sl = step(chest, p.torso + 90.0, 0.9 * u); let sr = step(chest, p.torso - 90.0, 0.9 * u);
    let el = step(sl, p.upper_arm_l, 1.4 * u); let er = step(sr, p.upper_arm_r, 1.4 * u);
    let hl = step(hip, p.torso + 90.0, 0.55 * u); let hr = step(hip, p.torso - 90.0, 0.55 * u);
    let kl = step(hl, p.thigh_l, 1.9 * u); let kr = step(hr, p.thigh_r, 1.9 * u);
    for (k, v) in [("shoulderL", sl), ("shoulderR", sr), ("elbowL", el), ("elbowR", er), ("handL", step(el, p.upper_arm_l + p.fore_arm_l, 1.3 * u)), ("handR", step(er, p.upper_arm_r + p.fore_arm_r, 1.3 * u)),
        ("hipL", hl), ("hipR", hr), ("kneeL", kl), ("kneeR", kr), ("footL", step(kl, p.thigh_l + p.shin_l, 1.8 * u)), ("footR", step(kr, p.thigh_r + p.shin_r, 1.8 * u))] { j.insert(k, v); }
    let bones = [("chest", "neck", 0.22, 0), ("shoulderL", "elbowL", 0.24, 1), ("elbowL", "handL", 0.19, 1), ("shoulderR", "elbowR", 0.24, -1), ("elbowR", "handR", 0.19, -1),
        ("hipL", "kneeL", 0.32, 1), ("kneeL", "footL", 0.24, 1), ("hipR", "kneeR", 0.32, -1), ("kneeR", "footR", 0.24, -1)]
        .iter().map(|(a, b, w, s)| Bone { a: j[a], b: j[b], w: w * u, side: *s }).collect();
    Mannequin { torso: [sl, sr, hr, hl], joints: j, bones, head_r: 0.5 * u, unit: u }
}

pub fn mannequin_bounds(m: &Mannequin) -> [f64; 4] {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for b in &m.bones { for p in [b.a, b.b] { x0 = x0.min(p[0] - b.w); y0 = y0.min(p[1] - b.w); x1 = x1.max(p[0] + b.w); y1 = y1.max(p[1] + b.w); } }
    let hd = m.joints["head"];
    x0 = x0.min(hd[0] - m.head_r); y0 = y0.min(hd[1] - m.head_r); x1 = x1.max(hd[0] + m.head_r); y1 = y1.max(hd[1] + m.head_r);
    [x0, y0, x1 - x0, y1 - y0]
}

// ----- history and statistics -----
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session { pub day: i64, pub poses: u32, pub seconds: u32, pub label: String }

#[derive(Debug, Default, PartialEq)]
pub struct Stats { pub sessions: usize, pub poses: u32, pub minutes: u32, pub week_minutes: u32, pub streak: u32 }

/// `today` and `Session::day` are days since 1970-01-01.
pub fn stats(s: &[Session], today: i64) -> Stats {
    let days: std::collections::BTreeSet<i64> = s.iter().map(|x| x.day).collect();
    let mut streak = 0u32;
    let mut d = if days.contains(&today) { today } else { today - 1 };
    while days.contains(&d) { streak += 1; d -= 1; }
    Stats {
        sessions: s.len(), poses: s.iter().map(|x| x.poses).sum(),
        minutes: (s.iter().map(|x| x.seconds as f64).sum::<f64>() / 60.0).round() as u32,
        week_minutes: (s.iter().filter(|x| today - x.day < 7).map(|x| x.seconds as f64).sum::<f64>() / 60.0).round() as u32,
        streak,
    }
}

// ----- skill tree -----
#[derive(Debug, Clone, Deserialize)]
pub struct Category { pub id: String, pub title: String, pub summary: String }
#[derive(Debug, Clone, Deserialize)]
pub struct Tool { pub route: String, pub label: String }
#[derive(Debug, Clone, Deserialize)]
pub struct Lesson { pub id: String, pub cat: String, pub level: String, pub minutes: u32, pub title: String, pub goal: String, pub steps: Vec<String>, pub tool: Option<Tool>, #[serde(default)] pub requires: Vec<String> }
#[derive(Debug, Clone, Deserialize)]
pub struct Lessons { pub categories: Vec<Category>, pub lessons: Vec<Lesson> }

pub fn lessons() -> &'static Lessons {
    static L: std::sync::OnceLock<Lessons> = std::sync::OnceLock::new();
    L.get_or_init(|| serde_json::from_str(include_str!("../../../../shared/lessons.en.json")).expect("lessons.en.json"))
}

/// Progress: done lesson id → date (YYYY-MM-DD), saved lessons, notes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillProgress { #[serde(default)] pub done: BTreeMap<String, String>, #[serde(default)] pub bookmarks: Vec<String>, #[serde(default)] pub notes: BTreeMap<String, String> }

pub const LEVELS: [&str; 3] = ["beginner", "intermediate", "advanced"];

pub fn unlocked(l: &Lesson, p: &SkillProgress) -> bool { l.requires.iter().all(|r| p.done.contains_key(r)) }

pub fn next_lesson<'a>(d: &'a Lessons, p: &SkillProgress) -> Option<&'a Lesson> {
    let mut open: Vec<&Lesson> = d.lessons.iter().filter(|l| !p.done.contains_key(&l.id) && unlocked(l, p)).collect();
    open.sort_by_key(|l| LEVELS.iter().position(|x| *x == l.level).unwrap_or(9));
    open.first().copied()
}

pub fn milestones(d: &Lessons, p: &SkillProgress) -> Vec<String> {
    let n = p.done.len();
    let mut out = Vec::new();
    for (k, t) in [(1, "First lesson done"), (5, "Five lessons done"), (15, "Fifteen lessons done")] { if n >= k { out.push(t.to_string()); } }
    for c in &d.categories {
        let ls: Vec<&Lesson> = d.lessons.iter().filter(|l| l.cat == c.id).collect();
        if !ls.is_empty() && ls.iter().all(|l| p.done.contains_key(&l.id)) { out.push(format!("{} complete", c.title)); }
    }
    if n == d.lessons.len() && n > 0 { out.push("Every lesson done".into()); }
    out
}
