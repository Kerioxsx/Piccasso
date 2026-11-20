//! Portable project format v1 (`schema/drawstep-project.v1.schema.json`).
//! Unknown fields are preserved (`extra`) so files round-trip between the web
//! and desktop apps without losing data either side doesn't understand.
use crate::analysis::Analysis;
use crate::imgproc::{CropRect, PrepImage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const FORMAT: &str = "drawstep-project";
pub const VERSION: u32 = 1;
pub const STYLES: [&str; 8] = ["original", "clean_outline", "simplified_outline", "pencil_sketch", "light_sketch", "contour_only", "shading_guide", "basic_shapes"];
pub const DIFFICULTIES: [&str; 4] = ["very_easy", "easy", "intermediate", "detailed"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct PaperCfg {
    pub preset: String,
    pub orientation: String,
    pub margin_mm: f64,
    /// Used when preset == "custom".
    pub width_mm: f64,
    pub height_mm: f64,
    /// Grid-method square size in cm (0 = off).
    pub grid_cm: f64,
}
impl Default for PaperCfg {
    fn default() -> Self {
        PaperCfg { preset: "a4".into(), orientation: "auto".into(), margin_mm: 15.0, width_mm: 210.0, height_mm: 297.0, grid_cm: 0.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Prep {
    pub crop: CropRect,
    pub rotation: i64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub brightness: f64,
    pub contrast: f64,
    pub style: String,
    pub difficulty: String,
    pub line_thickness: f64,
    pub sketch_contrast: f64,
    pub opacity: f64,
    pub simplify_background: bool,
    pub excluded_objects: Vec<String>,
    pub isolate_object: Option<String>,
    /// "graphite" (one pencil) or "color".
    pub medium: String,
    pub paper: PaperCfg,
}
impl Default for Prep {
    fn default() -> Self {
        Prep {
            crop: CropRect::default(), rotation: 0, flip_h: false, flip_v: false, brightness: 0.0, contrast: 0.0,
            style: "pencil_sketch".into(), difficulty: "easy".into(), line_thickness: 2.0, sketch_contrast: 20.0, opacity: 100.0,
            simplify_background: false, excluded_objects: vec![], isolate_object: None, medium: "graphite".into(), paper: PaperCfg::default(),
        }
    }
}
impl Prep {
    pub fn image(&self) -> PrepImage {
        PrepImage { crop: self.crop, rotation: self.rotation, flip_h: self.flip_h, flip_v: self.flip_v, brightness: self.brightness, contrast: self.contrast }
    }
    /// Hash of the fields that affect renders (cache key).
    pub fn render_hash(&self) -> String {
        let s = serde_json::to_string(&(&self.crop, self.rotation, self.flip_h, self.flip_v, self.brightness, self.contrast, &self.style, &self.difficulty,
            self.line_thickness, self.sketch_contrast, self.simplify_background, &self.excluded_objects, &self.isolate_object)).unwrap_or_default();
        let mut h: u32 = 0x811c9dc5;
        for b in s.bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x01000193);
        }
        format!("{h:08x}")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Step {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub instruction: String,
    pub tip: String,
    pub checks: Vec<String>,
    pub base: String,
    pub layers: Vec<String>,
    pub terms: Vec<String>,
    pub motion: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_instruction: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Progress {
    pub current_step: usize,
    pub completed: Vec<String>,
    pub skipped: Vec<String>,
    pub time_spent_sec: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewState {
    pub zoom: f64,
    pub pan_x: f64,
    pub pan_y: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub opacity: f64,
    pub fit: String,
}
impl Default for ViewState {
    fn default() -> Self {
        ViewState { zoom: 1.0, pan_x: 0.0, pan_y: 0.0, brightness: 100.0, contrast: 100.0, opacity: 100.0, fit: "page".into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ArState {
    pub opacity: f64,
    pub scale: f64,
    pub rotation: f64,
    pub offset_x: f64,
    pub offset_y: f64,
    pub corners: Option<Vec<[f64; 2]>>,
    pub layer: String,
    pub alignment_locked: bool,
}
impl Default for ArState {
    fn default() -> Self {
        ArState { opacity: 0.5, scale: 1.0, rotation: 0.0, offset_x: 0.0, offset_y: 0.0, corners: None, layer: "outline".into(), alignment_locked: false }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Source {
    pub mime: String,
    pub width: u32,
    pub height: u32,
    /// Base64 image bytes (no data: prefix).
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_with: Option<Value>,
    pub source: Source,
    pub prep: Prep,
    pub analysis: Option<Analysis>,
    pub steps: Vec<Step>,
    pub progress: Progress,
    pub view: ViewState,
    pub ar: ArState,
    /// Fields from other versions/apps (e.g. the web app's `thumb`, `ai`) — preserved.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
impl Default for Project {
    fn default() -> Self {
        Project {
            format: FORMAT.into(), version: VERSION, id: String::new(), name: "Untitled drawing".into(),
            created_at: String::new(), updated_at: String::new(), created_with: None, source: Source::default(), prep: Prep::default(),
            analysis: None, steps: vec![], progress: Progress::default(), view: ViewState::default(), ar: ArState::default(), extra: Map::new(),
        }
    }
}

/// RFC 3339 UTC timestamp without external crates.
pub fn now_iso() -> String {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    iso_from_unix(d.as_secs() as i64, d.subsec_millis())
}
pub fn iso_from_unix(secs: i64, millis: u32) -> String {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60, millis)
}

impl Project {
    pub fn new(name: &str, source: Source) -> Self {
        let now = now_iso();
        Project {
            id: uuid::Uuid::new_v4().to_string(), name: if name.is_empty() { "Untitled drawing".into() } else { name.into() },
            created_at: now.clone(), updated_at: now,
            created_with: Some(serde_json::json!({ "app": "drawstep-desktop", "version": env!("CARGO_PKG_VERSION") })),
            source, ..Default::default()
        }
    }
    pub fn current_step(&self) -> Option<&Step> {
        self.steps.get(self.progress.current_step)
    }
    pub fn goto_step(&mut self, i: i64) {
        let max = self.steps.len().saturating_sub(1) as i64;
        self.progress.current_step = i.clamp(0, max) as usize;
    }
    pub fn complete_step(&mut self, skipped: bool) {
        let Some(id) = self.current_step().map(|s| s.id.clone()) else { return };
        self.progress.completed.retain(|x| x != &id);
        self.progress.skipped.retain(|x| x != &id);
        if skipped { self.progress.skipped.push(id) } else { self.progress.completed.push(id) }
        self.goto_step(self.progress.current_step as i64 + 1);
    }
    pub fn restart_tutorial(&mut self) {
        let t = self.progress.time_spent_sec;
        self.progress = Progress { time_spent_sec: t, ..Default::default() };
    }
    pub fn fraction_done(&self) -> f32 {
        if self.steps.is_empty() { 0.0 } else { (self.progress.completed.len() + self.progress.skipped.len()) as f32 / self.steps.len() as f32 }
    }
    /// Clamp/repair values after loading.
    pub fn normalize(&mut self) {
        let max = self.steps.len().saturating_sub(1);
        self.progress.current_step = self.progress.current_step.min(max);
        if self.created_at.is_empty() { self.created_at = now_iso(); }
        if self.updated_at.is_empty() { self.updated_at = self.created_at.clone(); }
    }
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("project serializes")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("This file is not valid JSON: {0}")]
    Json(String),
    #[error("This is not a valid DrawStep project:\n• {}", .0.join("\n• "))]
    Invalid(Vec<String>),
}

/// Structural validation identical to the web app's `validateProject`.
pub fn validate(v: &Value) -> Vec<String> {
    let mut e = Vec::new();
    let Some(o) = v.as_object() else { return vec!["Project must be a JSON object.".into()] };
    if o.get("format").and_then(Value::as_str) != Some(FORMAT) { e.push(format!("format must be \"{FORMAT}\".")); }
    match o.get("version").and_then(Value::as_u64) {
        Some(ver) if ver >= 1 => { if ver > VERSION as u64 { e.push(format!("This project was made with a newer DrawStep (format v{ver}). Please update the app.")); } }
        _ => e.push("version must be a positive integer.".into()),
    }
    if o.get("id").and_then(Value::as_str).map_or(true, str::is_empty) { e.push("id is required.".into()); }
    if !o.get("name").map_or(false, Value::is_string) { e.push("name must be a string.".into()); }
    match o.get("source").and_then(Value::as_object) {
        None => e.push("source is required.".into()),
        Some(s) => {
            if s.get("data").and_then(Value::as_str).map_or(true, str::is_empty) { e.push("source.data (base64 image) is required.".into()); }
            let mime = s.get("mime").and_then(Value::as_str).unwrap_or("");
            if !["image/png", "image/jpeg", "image/webp", "image/gif", "image/bmp"].contains(&mime) { e.push("source.mime must be an image type.".into()); }
            let pos = |k: &str| s.get(k).and_then(Value::as_f64).map_or(false, |x| x > 0.0);
            if !pos("width") || !pos("height") { e.push("source.width/height must be positive.".into()); }
        }
    }
    if let Some(p) = o.get("prep") {
        match p.as_object() {
            None => e.push("prep must be an object.".into()),
            Some(p) => {
                if let Some(s) = p.get("style").and_then(Value::as_str) { if !STYLES.contains(&s) { e.push(format!("prep.style \"{s}\" is unknown.")); } }
                if let Some(d) = p.get("difficulty").and_then(Value::as_str) { if !DIFFICULTIES.contains(&d) { e.push(format!("prep.difficulty \"{d}\" is unknown.")); } }
                if let Some(r) = p.get("rotation") { if ![0, 90, 180, 270].contains(&r.as_i64().unwrap_or(-1)) { e.push("prep.rotation must be 0, 90, 180 or 270.".into()); } }
            }
        }
    }
    if let Some(s) = o.get("steps") {
        match s.as_array() {
            None => e.push("steps must be an array.".into()),
            Some(a) => for (i, st) in a.iter().enumerate() {
                let ok = st.get("id").map_or(false, Value::is_string) && st.get("kind").map_or(false, Value::is_string) && st.get("instruction").map_or(false, Value::is_string);
                if !ok { e.push(format!("steps[{i}] needs id, kind and instruction.")); }
            },
        }
    }
    e
}

/// Parse + validate + normalize.
pub fn import_project(text: &str) -> Result<Project, ProjectError> {
    let v: Value = serde_json::from_str(text).map_err(|e| ProjectError::Json(e.to_string()))?;
    let errs = validate(&v);
    if !errs.is_empty() {
        return Err(ProjectError::Invalid(errs));
    }
    let mut p: Project = serde_json::from_value(v).map_err(|e| ProjectError::Invalid(vec![e.to_string()]))?;
    p.normalize();
    Ok(p)
}
