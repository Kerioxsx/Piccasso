//! Local project storage on disk. Each project is stored as its portable
//! `.piccasso.json` file (the same format the web app exports), plus a render
//! cache of PNGs keyed by the prep hash.
use crate::project::{import_project, now_iso, Project};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub struct ProjectStore {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub steps: usize,
    pub current_step: usize,
    pub fraction: f32,
}

impl ProjectStore {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("projects"))?;
        fs::create_dir_all(root.join("cache"))?;
        Ok(ProjectStore { root })
    }
    fn path(&self, id: &str) -> PathBuf {
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        self.root.join("projects").join(format!("{safe}.piccasso.json"))
    }
    pub fn cache_dir(&self, id: &str, hash: &str) -> PathBuf {
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        self.root.join("cache").join(safe).join(hash)
    }

    /// Atomic save (write temp + rename). Updates `updated_at` when `touch`.
    pub fn save(&self, p: &mut Project, touch: bool) -> io::Result<()> {
        if touch {
            p.updated_at = now_iso();
        }
        let path = self.path(&p.id);
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, p.to_json())?;
        fs::rename(&tmp, &path)
    }
    pub fn load(&self, id: &str) -> io::Result<Project> {
        let text = fs::read_to_string(self.path(id))?;
        import_project(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
    }
    pub fn exists(&self, id: &str) -> bool {
        self.path(id).exists()
    }
    /// Summaries, most recently updated first. Unreadable files are skipped.
    pub fn list(&self) -> io::Result<Vec<Summary>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(self.root.join("projects"))? {
            let path = entry?.path();
            if !path.to_string_lossy().ends_with(".piccasso.json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(p) = import_project(&text) {
                    out.push(Summary { id: p.id.clone(), name: p.name.clone(), updated_at: p.updated_at.clone(), steps: p.steps.len(), current_step: p.progress.current_step, fraction: p.fraction_done() });
                }
            }
        }
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(out)
    }
    pub fn rename(&self, id: &str, name: &str) -> io::Result<Project> {
        let mut p = self.load(id)?;
        let n = name.trim();
        if !n.is_empty() {
            p.name = n.into();
        }
        self.save(&mut p, true)?;
        Ok(p)
    }
    pub fn duplicate(&self, id: &str) -> io::Result<Project> {
        let mut p = self.load(id)?;
        p.id = uuid::Uuid::new_v4().to_string();
        p.name = format!("{} (copy)", p.name);
        p.created_at = now_iso();
        self.save(&mut p, true)?;
        Ok(p)
    }
    pub fn delete(&self, id: &str) -> io::Result<()> {
        let path = self.path(id);
        if path.exists() {
            fs::remove_file(path)?;
        }
        let cache = self.root.join("cache").join(id);
        if cache.exists() {
            let _ = fs::remove_dir_all(cache);
        }
        Ok(())
    }
    /// Import a file exported from either app. Gets a fresh id if it clashes.
    pub fn import_file(&self, file: &Path) -> Result<Project, String> {
        let text = fs::read_to_string(file).map_err(|e| format!("Could not read the file: {e}"))?;
        let mut p = import_project(&text).map_err(|e| e.to_string())?;
        if self.exists(&p.id) {
            p.id = uuid::Uuid::new_v4().to_string();
        }
        self.save(&mut p, false).map_err(|e| e.to_string())?;
        Ok(p)
    }
}
