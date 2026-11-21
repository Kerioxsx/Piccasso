//! Persistent user settings (JSON in the app data directory).
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: String, // system | light | dark
    pub ui_scale: f32,
    pub hold_to_unlock: bool,
    pub hold_ms: u64,
    pub keep_awake: bool,
    pub lock_timer: bool,
    pub overlay_opacity: f32,
    pub onboarded: bool,
    /// Look for a new version on start (GitHub releases).
    pub check_updates: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "system".into(), ui_scale: 1.0, hold_to_unlock: true, hold_ms: 1200, keep_awake: true, lock_timer: true,
            overlay_opacity: 0.5, onboarded: false, check_updates: true,
        }
    }
}
impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
    pub fn save(&self, path: &PathBuf) {
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, t);
        }
    }
}
