use crate::app::{DrawStepApp, Screen};
use crate::ui::{heading, primary, Confirm, ConfirmAction};
use drawstep_core::project::{import_project, Project};
use drawstep_core::shared::shared;
use drawstep_core::store::Summary;
use egui::RichText;

pub const DEMO_JSON: &str = include_str!("../../../../../samples/demo-project.drawstep.json");

#[derive(Default)]
pub struct HomeState {
    pub list: Option<Vec<Summary>>,
    pub renaming: Option<(String, String)>,
}

pub fn onboarding(app: &mut DrawStepApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(780.0);
            ui.add_space(20.0);
            heading(ui, "Welcome to DrawStep");
            ui.label(RichText::new("Pick any picture and DrawStep will help you draw it on real paper, one easy step at a time. No drawing experience needed.").size(17.0));
            ui.add_space(12.0);
            for (t, d) in [
                ("Guided steps", "The picture is broken into simple shapes, outlines and shading, with plain-language instructions made for your picture."),
                ("Locked reference", "Keep the guide on screen without it moving by accident. The screen stays awake while you draw."),
                ("Camera overlay", "See the guide on top of a camera view (or photo) of your paper to help you trace. Full AR tracking is available in the phone/web version."),
            ] {
                ui.group(|ui| { ui.set_width(ui.available_width()); ui.strong(t); ui.label(d); });
            }
            ui.label(RichText::new("Your pictures stay on this computer. Nothing is uploaded unless you turn on the optional AI helper.").weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary(ui, "Get started").clicked() { finish(app, ctx); }
                if ui.button("Try the demo first").clicked() { finish(app, ctx); load_demo(app); }
            });
        });
    });
}
fn finish(app: &mut DrawStepApp, ctx: &egui::Context) {
    app.settings.onboarded = true;
    app.save_settings(ctx);
    app.go(Screen::Home);
}

pub fn import_image(app: &mut DrawStepApp) {
    let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).set_title("Choose a picture to draw").pick_file() else { return };
    match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| drawstep_core::source_from_bytes(&b)) {
        Ok((source, img)) => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Drawing").to_string();
            start_project(app, Project::new(&name, source), Some(img));
        }
        Err(e) => app.error(e),
    }
}

pub fn start_project(app: &mut DrawStepApp, mut p: Project, _img: Option<drawstep_core::imgproc::Rgba>) {
    p.ar.opacity = app.settings.overlay_opacity as f64;
    if let Some(store) = &app.store {
        if let Err(e) = store.save(&mut p, true) {
            app.error(format!("Could not save: {e}"));
        }
    }
    match app.open_project(p) {
        Ok(()) => app.go(Screen::Prepare),
        Err(e) => app.error(e),
    }
    app.screen_state.home.list = None;
}

pub fn load_demo(app: &mut DrawStepApp) {
    match import_project(DEMO_JSON) {
        Ok(mut p) => {
            p.id = uuid_like();
            p.created_at = drawstep_core::project::now_iso();
            if let Some(store) = &app.store { let _ = store.save(&mut p, true); }
            match app.open_project(p) {
                Ok(()) => { if !app.load_cached_renders() { app.start_job(true); } app.go(Screen::Workspace); }
                Err(e) => app.error(e),
            }
            app.screen_state.home.list = None;
        }
        Err(e) => app.error(e.to_string()),
    }
}
fn uuid_like() -> String {
    drawstep_core::uuid::Uuid::new_v4().to_string()
}

pub fn open(app: &mut DrawStepApp, id: &str) {
    let Some(store) = &app.store else { return };
    match store.load(id) {
        Ok(p) => {
            let has_steps = !p.steps.is_empty();
            match app.open_project(p) {
                Ok(()) => {
                    if has_steps {
                        if !app.load_cached_renders() { app.start_job(true); }
                        app.go(Screen::Workspace);
                    } else { app.go(Screen::Prepare); }
                }
                Err(e) => app.error(e),
            }
        }
        Err(e) => app.error(format!("Could not open the project: {e}")),
    }
}

pub fn import_project_file(app: &mut DrawStepApp) {
    let Some(path) = rfd::FileDialog::new().add_filter("DrawStep project", &["json"]).pick_file() else { return };
    let Some(store) = &app.store else { return };
    match store.import_file(&path) {
        Ok(p) => { app.toast(format!("Imported “{}”", p.name)); let id = p.id.clone(); app.screen_state.home.list = None; open(app, &id); }
        Err(e) => app.error(e),
    }
}

pub fn home(app: &mut DrawStepApp, ctx: &egui::Context) {
    if app.screen_state.home.list.is_none() {
        app.screen_state.home.list = Some(app.store.as_ref().and_then(|s| s.list().ok()).unwrap_or_default());
    }
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            heading(ui, "What would you like to draw?");
            ui.label(RichText::new("Choose a picture. DrawStep turns it into simple steps you can follow on paper.").weak());
            if let Some(e) = &app.store_error { ui.colored_label(egui::Color32::from_rgb(0xa2, 0x4d, 0x1c), e); }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if primary(ui, "Import picture").clicked() { import_image(app); }
                #[cfg(feature = "camera")]
                if ui.add(egui::Button::new("📷 Take a photo (webcam)").min_size(egui::vec2(0.0, 42.0))).clicked() { crate::camera::capture_still_into_project(app); }
                if ui.add(egui::Button::new("✏ Try the demo").min_size(egui::vec2(0.0, 42.0))).clicked() { load_demo(app); }
                if ui.add(egui::Button::new("Open project file").min_size(egui::vec2(0.0, 42.0))).clicked() { import_project_file(app); }
            });
            #[cfg(not(feature = "camera"))]
            ui.label(RichText::new("Camera capture is available when DrawStep is built with the `camera` feature. You can import photos from your camera roll or files.").small().weak());
            ui.add_space(16.0);
            ui.label(RichText::new("Your projects").size(20.0).strong());
            ui.label(RichText::new(format!("Saved on this computer in {}", app.data_dir.display())).small().weak());
            let list = app.screen_state.home.list.clone().unwrap_or_default();
            if list.is_empty() {
                ui.group(|ui| { ui.set_width(ui.available_width()); ui.label("No projects yet. Import a picture or try the demo to begin."); });
            }
            let mut action: Option<(&str, String)> = None;
            for s in &list {
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            if let Some((id, buf)) = app.screen_state.home.renaming.as_mut().filter(|(id, _)| id == &s.id) {
                                let r = ui.text_edit_singleline(buf);
                                if r.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) { action = Some(("rename-save", id.clone())); }
                            } else {
                                ui.strong(&s.name);
                            }
                            ui.label(RichText::new(if s.steps > 0 { format!("Step {} of {}", s.current_step + 1, s.steps) } else { "Not started".into() }).weak());
                            ui.add(egui::ProgressBar::new(s.fraction).desired_width(220.0));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Delete").clicked() { action = Some(("delete", s.id.clone())); }
                            if ui.button("Export").clicked() { action = Some(("export", s.id.clone())); }
                            if ui.button("Duplicate").clicked() { action = Some(("dup", s.id.clone())); }
                            if ui.button("✏ Rename").clicked() { action = Some(("rename", s.id.clone())); }
                            if primary(ui, "Open").clicked() { action = Some(("open", s.id.clone())); }
                        });
                    });
                });
            }
            if let Some((what, id)) = action {
                let store_ok = app.store.is_some();
                match what {
                    "open" => open(app, &id),
                    "rename" => { let name = list.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default(); app.screen_state.home.renaming = Some((id, name)); }
                    "rename-save" if store_ok => {
                        if let Some((id, name)) = app.screen_state.home.renaming.take() {
                            match app.store.as_ref().unwrap().rename(&id, &name) {
                                Ok(p) => { if let Some(sess) = app.session.as_mut().filter(|s| s.project.id == id) { sess.project.name = p.name; } }
                                Err(e) => app.error(e.to_string()),
                            }
                        }
                        app.screen_state.home.list = None;
                    }
                    "dup" if store_ok => { if let Err(e) = app.store.as_ref().unwrap().duplicate(&id) { app.error(e.to_string()); } else { app.toast("Project duplicated"); } app.screen_state.home.list = None; }
                    "export" if store_ok => { if let Ok(p) = app.store.as_ref().unwrap().load(&id) { crate::export::export_project_file(app, &p); } }
                    "delete" => {
                        let name = list.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
                        app.confirm = Some(Confirm { title: "Delete project?".into(), body: format!("“{name}” will be removed from this computer. This can't be undone."), ok: "Delete".into(), action: ConfirmAction::DeleteProject(id) });
                        app.screen_state.home.list = None;
                    }
                    _ => {}
                }
            }
        });
    });
}

pub fn learn(_app: &mut DrawStepApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(760.0);
            heading(ui, "Drawing words, explained");
            ui.label(RichText::new("You don't need to know these to start. Each step links to the words it uses.").weak());
            for t in &shared().glossary.terms {
                ui.add_space(6.0);
                ui.strong(&t.term);
                ui.label(&t.text);
            }
            ui.add_space(12.0);
            ui.label(RichText::new("Good habits for beginners").size(20.0).strong());
            for t in ["Use a regular HB pencil and draw lightly at first.", "Keep an eraser handy; mistakes are part of the process.", "Prop your screen so it stays put while you draw.", "Take breaks and step back to look at your drawing from a distance."] {
                ui.label(format!("• {t}"));
            }
        });
    });
}
