//! Shared declarative data (`/shared/*.json`), compiled into the binary so both
//! apps use the exact same difficulty tables, styles, paper sizes and texts.
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Level {
    pub label: String,
    pub blur_radius: usize,
    pub edge_fraction: f64,
    pub min_edge: i32,
    pub value_levels: u32,
    pub max_objects: usize,
    pub max_shapes: usize,
    pub max_internal: usize,
    pub min_area_frac: f64,
    pub dp_epsilon: f64,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Difficulty {
    pub analysis_max_dim: usize,
    pub levels: HashMap<String, Level>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StyleDef {
    pub id: String,
    pub label: String,
    pub description: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Styles {
    pub styles: Vec<StyleDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaperPreset {
    pub id: String,
    pub label: String,
    pub width_mm: f64,
    pub height_mm: f64,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paper {
    pub presets: Vec<PaperPreset>,
    pub default_margin_mm: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StepKind {
    pub id: String,
    pub base: String,
    pub layers: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct StepKinds {
    pub kinds: Vec<StepKind>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StepText {
    pub title: String,
    pub instruction: String,
    pub tip: String,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub motion: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PressureEntry {
    pub label: String,
    pub pct: u32,
    pub grade: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pressure {
    pub by_levels: HashMap<String, Vec<PressureEntry>>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Colors {
    pub colors: Vec<(String, [i32; 3])>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AspectText {
    pub taller: String,
    pub wider: String,
    pub square: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct LightText {
    pub even: String,
    pub dir: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct RatioWords {
    pub same: String,
    pub twice: String,
    pub x: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackText {
    pub low_contrast: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareText {
    pub wider: String,
    pub narrower: String,
    pub taller: String,
    pub shorter: String,
    pub shift_left: String,
    pub shift_right: String,
    pub shift_up: String,
    pub shift_down: String,
    pub sparse: String,
    pub good: String,
    pub empty: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Strings {
    pub positions: HashMap<String, String>,
    pub sizes: Vec<String>,
    pub shape_kinds: HashMap<String, String>,
    pub aspect: AspectText,
    pub light: LightText,
    pub steps: HashMap<String, StepText>,
    pub compare_item: String,
    pub compare_single: String,
    pub ratio_words: RatioWords,
    pub fallback: FallbackText,
    pub compare: CompareText,
    pub shape_phrase: String,
    #[serde(default)]
    pub manual_steps: HashMap<String, String>,
    pub pressure_item: String,
    pub pressure_labels: HashMap<String, String>,
    pub highlight_tip: String,
    pub shade_methods: HashMap<String, String>,
    pub palette_item: String,
    pub color_tip: String,
    pub grid: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Term {
    pub id: String,
    pub term: String,
    pub text: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Glossary {
    pub terms: Vec<Term>,
    pub step_terms: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Shared {
    pub difficulty: Difficulty,
    pub styles: Styles,
    pub paper: Paper,
    pub step_kinds: StepKinds,
    pub strings: Strings,
    pub glossary: Glossary,
    pub pressure: Pressure,
    pub colors: Colors,
}

pub const DIFFICULTY_ORDER: [&str; 4] = ["very_easy", "easy", "intermediate", "detailed"];

impl Shared {
    pub fn level(&self, id: &str) -> &Level {
        self.difficulty.levels.get(id).unwrap_or_else(|| &self.difficulty.levels["easy"])
    }
    pub fn position(&self, code: &str) -> &str {
        self.strings.positions.get(code).or_else(|| self.strings.positions.get("mc")).map(|s| s.as_str()).unwrap_or("")
    }
}

static SHARED: OnceLock<Shared> = OnceLock::new();

/// The built-in English data set.
pub fn shared() -> &'static Shared {
    SHARED.get_or_init(|| Shared {
        difficulty: serde_json::from_str(include_str!("../../../../shared/difficulty.json")).expect("difficulty.json"),
        styles: serde_json::from_str(include_str!("../../../../shared/styles.json")).expect("styles.json"),
        paper: serde_json::from_str(include_str!("../../../../shared/paper.json")).expect("paper.json"),
        step_kinds: serde_json::from_str(include_str!("../../../../shared/step-kinds.json")).expect("step-kinds.json"),
        strings: serde_json::from_str(include_str!("../../../../shared/strings.en.json")).expect("strings.en.json"),
        glossary: serde_json::from_str(include_str!("../../../../shared/glossary.en.json")).expect("glossary.en.json"),
        pressure: serde_json::from_str(include_str!("../../../../shared/pressure.json")).expect("pressure.json"),
        colors: serde_json::from_str(include_str!("../../../../shared/colors.json")).expect("colors.json"),
    })
}
