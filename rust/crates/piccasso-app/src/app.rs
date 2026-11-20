//! Application state, navigation, background jobs and toasts.
use crate::settings::Settings;
use crate::ui::Textures;
use piccasso_core::imgproc::Rgba;
use piccasso_core::lock::View;
use piccasso_core::pipeline::{run_pipeline, PipelineResult};
use piccasso_core::project::Project;
use piccasso_core::shared::shared;
use piccasso_core::store::ProjectStore;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Onboarding,
    Home,
    Prepare,
    Processing,
    Workspace,
    Locked,
    Overlay,
    Compare,
    Settings,
    Learn,
    AutoDraw,
}

/// The project currently open, with decoded source and rendered guides.
pub struct Session {
    pub project: Project,
    pub src: Rgba,
    pub renders: BTreeMap<String, Rgba>,
    pub renders_hash: String,
    pub dirty: bool,
}

pub enum JobMsg {
    Progress(f32, String),
    Done(Box<PipelineResult>),
    Failed(String),
}

pub struct Job {
    pub rx: Receiver<JobMsg>,
    pub cancel: Arc<AtomicBool>,
    pub progress: f32,
    pub label: String,
    /// Keep step text/progress (only refresh renders) when true.
    pub renders_only: bool,
}

pub struct Toast {
    pub text: String,
    pub error: bool,
    pub until: Instant,
}

pub struct PiccassoApp {
    pub screen: Screen,
    pub prev_screen: Screen,
    pub store: Option<ProjectStore>,
    pub store_error: Option<String>,
    pub data_dir: PathBuf,
    pub settings: Settings,
    pub session: Option<Session>,
    pub textures: Textures,
    pub job: Option<Job>,
    pub toasts: Vec<Toast>,
    pub view: View,
    pub started: Instant,
    pub screen_state: crate::screens::ScreenState,
    pub confirm: Option<crate::ui::Confirm>,
}

impl PiccassoApp {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        // PICCASSO_DATA_DIR overrides the location (portable installs, tests).
        let data_dir = std::env::var_os("PICCASSO_DATA_DIR").map(PathBuf::from).or_else(|| {
            directories::ProjectDirs::from("app", "Piccasso", "Piccasso").map(|d| d.data_dir().to_path_buf())
        }).unwrap_or_else(|| PathBuf::from("piccasso-data"));
        let (store, store_error) = match ProjectStore::new(&data_dir) {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(format!("Projects can't be saved: {e} ({})", data_dir.display()))),
        };
        let settings = Settings::load(&data_dir.join("settings.json"));
        let mut app = PiccassoApp {
            screen: if settings.onboarded { Screen::Home } else { Screen::Onboarding },
            prev_screen: Screen::Home,
            store, store_error, data_dir, settings, session: None, textures: Textures::default(), job: None,
            toasts: vec![], view: View::default(), started: Instant::now(), screen_state: Default::default(), confirm: None,
        };
        app.apply_settings(&cc.egui_ctx);
        // Developer convenience: PICCASSO_START=<screen> opens the demo on that screen.
        if let Ok(start) = std::env::var("PICCASSO_START") {
            crate::screens::home::load_demo(&mut app);
            app.screen = match start.as_str() {
                "prepare" => Screen::Prepare, "locked" => Screen::Locked, "overlay" => Screen::Overlay,
                "compare" => Screen::Compare, "autodraw" => Screen::AutoDraw, "settings" => Screen::Settings, "home" => Screen::Home, "learn" => Screen::Learn,
                _ => Screen::Workspace,
            };
            app.screen_state.on_enter(app.screen);
            if let (Ok(n), Some(sess)) = (std::env::var("PICCASSO_STEP"), app.session.as_mut()) { sess.project.goto_step(n.parse().unwrap_or(0)); }
        }
        app
    }

    pub fn apply_settings(&self, ctx: &egui::Context) {
        ctx.set_theme(match self.settings.theme.as_str() {
            "light" => egui::ThemePreference::Light,
            "dark" => egui::ThemePreference::Dark,
            _ => egui::ThemePreference::System,
        });
        ctx.set_zoom_factor(self.settings.ui_scale);
        crate::ui::style(ctx);
    }
    pub fn save_settings(&self, ctx: &egui::Context) {
        self.settings.save(&self.data_dir.join("settings.json"));
        self.apply_settings(ctx);
    }

    pub fn go(&mut self, s: Screen) {
        if self.screen != s {
            self.prev_screen = self.screen;
            self.screen = s;
            self.screen_state.on_enter(s);
        }
    }

    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), error: false, until: Instant::now() + Duration::from_secs(4) });
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), error: true, until: Instant::now() + Duration::from_secs(7) });
    }

    /// Persist the current project (if any).
    pub fn save_project(&mut self) {
        let Some(sess) = self.session.as_mut() else { return };
        let Some(store) = self.store.as_ref() else { return };
        if let Err(e) = store.save(&mut sess.project, true) {
            let msg = format!("Could not save the project: {e}");
            self.error(msg);
        } else {
            sess.dirty = false;
        }
    }

    pub fn open_project(&mut self, project: Project) -> Result<(), String> {
        let src = piccasso_core::decode_source(&project.source)?;
        self.textures.clear();
        self.view = View::default();
        self.session = Some(Session { project, src, renders: BTreeMap::new(), renders_hash: String::new(), dirty: false });
        self.load_cached_renders();
        Ok(())
    }

    /// Load renders from the on-disk cache when they match the current prep.
    pub fn load_cached_renders(&mut self) -> bool {
        let (Some(sess), Some(store)) = (self.session.as_mut(), self.store.as_ref()) else { return false };
        let hash = sess.project.prep.render_hash();
        if sess.renders_hash == hash && !sess.renders.is_empty() {
            return true;
        }
        let dir = store.cache_dir(&sess.project.id, &hash);
        let mut renders = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let path = e.path();
                if let (Some(stem), Ok(bytes)) = (path.file_stem().and_then(|s| s.to_str()).map(String::from), std::fs::read(&path)) {
                    if let Ok(img) = piccasso_core::decode_image_bytes(&bytes) {
                        renders.insert(stem, img);
                    }
                }
            }
        }
        if renders.contains_key(&sess.project.prep.style) && renders.contains_key("original") {
            sess.renders = renders;
            sess.renders_hash = hash;
            self.textures.clear();
            true
        } else {
            false
        }
    }

    fn write_render_cache(&self) {
        let (Some(sess), Some(store)) = (self.session.as_ref(), self.store.as_ref()) else { return };
        let dir = store.cache_dir(&sess.project.id, &sess.renders_hash);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        for (k, img) in &sess.renders {
            if let Ok(png) = piccasso_core::encode_png(img) {
                let _ = std::fs::write(dir.join(format!("{k}.png")), png);
            }
        }
    }

    /// Start analysis + rendering in a background thread.
    pub fn start_job(&mut self, renders_only: bool) {
        let Some(sess) = self.session.as_ref() else { return };
        if let Some(j) = &self.job {
            j.cancel.store(true, Ordering::Relaxed);
        }
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (src, prep, c2) = (sess.src.clone(), sess.project.prep.clone(), cancel.clone());
        std::thread::spawn(move || {
            let txp = tx.clone();
            let res = run_pipeline(&src, &prep, shared(), 1280, &|p, l| { let _ = txp.send(JobMsg::Progress(p, l.to_string())); }, &c2);
            let _ = tx.send(match res {
                Ok(r) => JobMsg::Done(Box::new(r)),
                Err(e) => JobMsg::Failed(e),
            });
        });
        self.job = Some(Job { rx, cancel, progress: 0.0, label: "Starting".into(), renders_only });
    }

    pub fn cancel_job(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = self.job.as_mut() else { return };
        let mut finished = None;
        while let Ok(m) = job.rx.try_recv() {
            match m {
                JobMsg::Progress(p, l) => { job.progress = p; job.label = l; }
                other => { finished = Some(other); break; }
            }
        }
        ctx.request_repaint_after(Duration::from_millis(50));
        let Some(msg) = finished else { return };
        let renders_only = job.renders_only;
        self.job = None;
        match msg {
            JobMsg::Done(r) => {
                let ai_on = self.settings.ai_enabled;
                if let Some(sess) = self.session.as_mut() {
                    let r = *r;
                    if !renders_only {
                        let same = sess.project.steps.len() == r.steps.len() && sess.project.steps.iter().zip(&r.steps).all(|(a, b)| a.kind == b.kind);
                        sess.project.steps = r.steps;
                        if !same {
                            sess.project.restart_tutorial();
                        }
                        sess.project.analysis = Some(r.analysis);
                    } else if sess.project.analysis.is_none() {
                        sess.project.analysis = Some(r.analysis);
                    }
                    sess.renders = r.renders;
                    sess.renders_hash = sess.project.prep.render_hash();
                }
                self.textures.clear();
                self.write_render_cache();
                if !renders_only && ai_on {
                    crate::ai::maybe_rewrite(self);
                }
                self.save_project();
                if self.screen == Screen::Processing {
                    self.go(Screen::Workspace);
                }
            }
            JobMsg::Failed(e) => {
                if e != "cancelled" {
                    self.error(format!("Processing failed: {e}"));
                    if self.screen == Screen::Processing {
                        self.go(Screen::Prepare);
                    }
                }
            }
            JobMsg::Progress(..) => {}
        }
    }
}

impl eframe::App for PiccassoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job(ctx);
        crate::ai::poll(self);
        crate::screens::show(self, ctx);
        crate::ui::show_toasts(self, ctx);
        crate::ui::show_confirm(self, ctx);
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_project();
    }
}
