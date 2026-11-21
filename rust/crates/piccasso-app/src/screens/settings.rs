use crate::app::PiccassoApp;
use crate::ui::heading;
use egui::RichText;

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let mut changed = false;
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(720.0);
            heading(ui, "Settings");
            let s = &mut app.settings;
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Display");
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    for (v, l) in [("system", "Match system"), ("light", "Light"), ("dark", "Dark")] {
                        changed |= ui.selectable_value(&mut s.theme, v.to_string(), l).changed();
                    }
                });
                changed |= ui.add(egui::Slider::new(&mut s.ui_scale, 0.8..=1.8).text("Interface and text size")).drag_stopped();
                ui.label(RichText::new("Animations are minimal in the desktop app.").small().weak());
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Locked reference");
                changed |= ui.checkbox(&mut s.keep_awake, "Keep the screen awake while drawing").changed();
                ui.label(RichText::new(if cfg!(feature = "keepawake") { "Uses the operating system's sleep-prevention API." } else { "This build has no keep-awake support; adjust your power settings instead." }).small().weak());
                changed |= ui.checkbox(&mut s.hold_to_unlock, "Press and hold to unlock (prevents accidental unlocking)").changed();
                let mut secs = s.hold_ms as f32 / 1000.0;
                if ui.add(egui::Slider::new(&mut secs, 0.6..=3.0).text("Hold time (s)")).changed() { s.hold_ms = (secs * 1000.0) as u64; changed = true; }
                changed |= ui.checkbox(&mut s.lock_timer, "Show session timer").changed();
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Camera overlay");
                changed |= ui.add(egui::Slider::new(&mut s.overlay_opacity, 0.1..=1.0).text("Default guide opacity")).changed();
                let cap = crate::camera::capability();
                ui.label(format!("This computer: {}.", cap.status));
                for r in cap.reasons { ui.label(RichText::new(r).small().weak()); }
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Language");
                ui.label("English. More languages can be added by translating shared/strings.<lang>.json.");
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Privacy");
                ui.label("Everything happens on this computer. Pictures and projects are never uploaded. Piccasso uses no AI: every guide comes from image-processing, geometry and computer-vision algorithms.");
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Updates");
                ui.label(format!("This is Piccasso {}.", crate::updater::current_version()));
                changed |= ui.checkbox(&mut s.check_updates, "Check for a new version when Piccasso starts").changed();
                ui.label(RichText::new("Asks GitHub for the latest release; nothing about you or your pictures is sent. Updates are only installed when you choose to.").small().weak());
                use crate::updater::Status;
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!app.updater.busy(), egui::Button::new("Check now")).clicked() { app.update_dismissed = false; app.updater.start_check(); }
                    match app.updater.status.clone() {
                        Status::Checking => { ui.spinner(); ui.label("Checking…"); }
                        Status::UpToDate => { ui.label("You have the latest version."); }
                        Status::Available(i) => { ui.label(format!("Version {} is available.", i.version)); if ui.button("Install and restart").clicked() { app.updater.start_install(); } }
                        Status::Downloading(f) => { ui.add(egui::ProgressBar::new(f).desired_width(200.0).show_percentage()); }
                        Status::Ready => { ui.label("Installed: restart Piccasso to use it."); }
                        Status::Failed(e) => { ui.label(RichText::new(e).small()); }
                        Status::Idle => {}
                    }
                });
            });
            ui.group(|ui| {
                ui.set_width(ui.available_width());
                ui.strong("Storage");
                ui.label(format!("Projects folder: {}", app.data_dir.join("projects").display()));
                ui.label(RichText::new("Each project is a portable .piccasso.json file that the web app can open too.").small().weak());
            });
            ui.label(RichText::new(format!("Piccasso desktop {} · project format v{}", env!("CARGO_PKG_VERSION"), piccasso_core::project::VERSION)).small().weak());
        });
    });
    if changed {
        app.save_settings(ctx);
    }
}
