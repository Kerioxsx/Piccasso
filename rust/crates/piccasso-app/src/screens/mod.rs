//! Screen dispatch, the top bar and per-screen transient state.
use crate::app::{PiccassoApp, Screen};
use egui::RichText;

pub mod autodraw;
pub mod compare;
pub mod home;
pub mod locked;
pub mod overlay;
pub mod prepare;
pub mod settings;
pub mod workspace;

#[derive(Default)]
pub struct ScreenState {
    pub prepare: prepare::PrepareState,
    pub locked: locked::LockedState,
    pub overlay: overlay::OverlayState,
    pub compare: compare::CompareState,
    pub home: home::HomeState,
    pub workspace: workspace::WorkspaceState,
    pub autodraw: autodraw::AutoDrawState,
    pub phone: crate::bridge::PhoneState,
}
impl ScreenState {
    pub fn on_enter(&mut self, s: Screen) {
        match s {
            Screen::Locked => self.locked = Default::default(),
            Screen::Overlay => self.overlay = Default::default(),
            Screen::Compare => self.compare = Default::default(),
            Screen::Prepare => self.prepare = Default::default(),
            _ => {}
        }
    }
}

fn top_bar(app: &mut PiccassoApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            if !matches!(app.screen, Screen::Home | Screen::Onboarding) && ui.button("◀ Back").clicked() {
                let target = match app.screen {
                    Screen::Workspace | Screen::Settings | Screen::Learn => Screen::Home,
                    Screen::Prepare => if app.session.as_ref().map_or(false, |s| !s.project.steps.is_empty()) { Screen::Workspace } else { Screen::Home },
                    _ => Screen::Workspace,
                };
                if app.screen == Screen::Prepare { app.save_project(); }
                app.go(target);
            }
            ui.label(RichText::new("✏ Piccasso").size(22.0).strong().color(crate::ui::ACCENT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.selectable_label(app.screen == Screen::Settings, "Settings").clicked() { app.go(Screen::Settings); }
                if ui.selectable_label(app.screen == Screen::Learn, "Learn").clicked() { app.go(Screen::Learn); }
                if ui.selectable_label(app.screen == Screen::Home, "Projects").clicked() { app.save_project(); app.go(Screen::Home); }
            });
        });
    });
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let needs_project = matches!(app.screen, Screen::Prepare | Screen::Processing | Screen::Workspace | Screen::Locked | Screen::Overlay | Screen::Compare | Screen::AutoDraw);
    if needs_project && app.session.is_none() {
        app.screen = Screen::Home;
    }
    if !matches!(app.screen, Screen::Locked | Screen::Overlay) {
        top_bar(app, ctx);
        update_banner(app, ctx);
    }
    match app.screen {
        Screen::Onboarding => home::onboarding(app, ctx),
        Screen::Home => home::home(app, ctx),
        Screen::Learn => home::learn(app, ctx),
        Screen::Settings => settings::show(app, ctx),
        Screen::Prepare => prepare::show(app, ctx),
        Screen::Processing => processing(app, ctx),
        Screen::Workspace => workspace::show(app, ctx),
        Screen::Locked => locked::show(app, ctx),
        Screen::Overlay => overlay::show(app, ctx),
        Screen::Compare => compare::show(app, ctx),
        Screen::AutoDraw => autodraw::show(app, ctx),
    }
}

/// "A new version is available" bar under the top bar.
fn update_banner(app: &mut PiccassoApp, ctx: &egui::Context) {
    use crate::updater::Status;
    let show = match &app.updater.status { Status::Available(_) => !app.update_dismissed, Status::Downloading(_) | Status::Ready => true, Status::Failed(_) => !app.update_dismissed && app.updater.status != Status::Idle && matches!(app.screen, Screen::Home), _ => false };
    if !show { return; }
    egui::TopBottomPanel::top("update-banner").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            match app.updater.status.clone() {
                Status::Available(i) => {
                    ui.label(egui::RichText::new(format!("Piccasso {} is available (you have {}).", i.version, crate::updater::current_version())).strong());
                    if ui.button("Install and restart").clicked() { app.save_project(); app.updater.start_install(); }
                    if ui.button("Later").clicked() { app.update_dismissed = true; }
                    if !i.page.is_empty() { ui.hyperlink_to("What's new", i.page); }
                }
                Status::Downloading(f) => { ui.label("Downloading the update…"); ui.add(egui::ProgressBar::new(f).desired_width(220.0).show_percentage()); }
                Status::Ready => {
                    ui.label(egui::RichText::new("The update is installed.").strong());
                    #[cfg(feature = "update")]
                    if ui.button("Restart now").clicked() { app.save_project(); crate::updater::restart(); }
                }
                Status::Failed(e) => { ui.label(format!("Update: {e}")); if ui.button("Dismiss").clicked() { app.update_dismissed = true; } }
                _ => {}
            }
        });
    });
}

fn processing(app: &mut PiccassoApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.25);
            crate::ui::heading(ui, "Planning your drawing");
            let (p, l) = app.job.as_ref().map(|j| (j.progress, j.label.clone())).unwrap_or((1.0, "Finishing".into()));
            ui.add(egui::ProgressBar::new(p).desired_width(420.0).show_percentage());
            ui.label(format!("{l}…"));
            ui.label(RichText::new("Everything runs on this computer.").weak());
            ui.add_space(8.0);
            if ui.button("Cancel").clicked() {
                app.cancel_job();
                app.go(Screen::Prepare);
            }
        });
    });
}
