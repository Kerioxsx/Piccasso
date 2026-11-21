//! Skill tree (desktop): the shared 30 lessons in 10 categories, unlocked in
//! order, with steps, a button that opens the matching tool, notes, bookmarks,
//! milestones; progress saved in the data folder.
use crate::app::{PiccassoApp, Screen};
use egui::RichText;
use piccasso_core::practice::{lessons, milestones, next_lesson, unlocked, SkillProgress, LEVELS};

#[derive(Default)]
pub struct SkillsState { open: Option<String>, filter: Option<&'static str>, progress: Option<SkillProgress> }

fn path(app: &PiccassoApp) -> std::path::PathBuf { app.data_dir.join("skills.json") }
fn save(app: &PiccassoApp, p: &SkillProgress) { let _ = std::fs::create_dir_all(&app.data_dir); let _ = std::fs::write(path(app), serde_json::to_string(p).unwrap_or_default()); }
fn level_text(l: &str) -> &'static str { match l { "beginner" => "Beginner", "intermediate" => "Intermediate", _ => "Advanced" } }
fn date() -> String {
    let d = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) / 86400) as i64;
    let z = d + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097; let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153; let dd = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", yoe + era * 400 + i64::from(m <= 2), m, dd)
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    if app.screen_state.skills.progress.is_none() {
        let p = std::fs::read_to_string(path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        app.screen_state.skills.progress = Some(p);
    }
    let d = lessons();
    let mut go: Option<Screen> = None;
    let mut changed = false;
    let st = &mut app.screen_state.skills;
    let p = st.progress.as_mut().unwrap();
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(id) = st.open.clone() {
                let Some(l) = d.lessons.iter().find(|l| l.id == id) else { st.open = None; return };
                let cat = d.categories.iter().find(|c| c.id == l.cat).map(|c| c.title.clone()).unwrap_or_default();
                ui.set_max_width(760.0);
                if ui.button("◀ Skill tree").clicked() { st.open = None; }
                ui.label(RichText::new(format!("{cat} · {} · about {} minutes", level_text(&l.level), l.minutes)).weak());
                ui.label(RichText::new(&l.title).size(26.0).strong());
                ui.label(RichText::new(&l.goal).size(17.0));
                if !unlocked(l, p) {
                    let need: Vec<String> = l.requires.iter().filter(|r| !p.done.contains_key(*r)).filter_map(|r| d.lessons.iter().find(|x| &x.id == r).map(|x| x.title.clone())).collect();
                    ui.colored_label(egui::Color32::from_rgb(0xa2, 0x4d, 0x1c), format!("First finish: {}", need.join(", ")));
                }
                ui.add_space(6.0);
                for (i, s) in l.steps.iter().enumerate() { ui.label(format!("{}. {s}", i + 1)); ui.add_space(2.0); }
                if let Some(t) = &l.tool {
                    let target = match t.route.as_str() { "guides" => Some(Screen::Guides), "studio" => Some(Screen::Studio), "practice" => Some(Screen::Practice), "sandbox" => Some(Screen::Sandbox), "home" => Some(Screen::Home), _ => None };
                    if let Some(sc) = target { if ui.button(&t.label).clicked() { go = Some(sc); } }
                }
                ui.add_space(6.0);
                ui.label("My notes");
                let note = p.notes.entry(l.id.clone()).or_default();
                if ui.add(egui::TextEdit::multiline(note).desired_rows(4).desired_width(f32::INFINITY).hint_text("What worked, what to try next time…")).changed() { changed = true; }
                ui.horizontal(|ui| {
                    if let Some(when) = p.done.get(&l.id).cloned() {
                        if ui.button(format!("Done on {when} · undo")).clicked() { p.done.remove(&l.id); changed = true; }
                    } else if ui.add_enabled(unlocked(l, p), egui::Button::new(RichText::new("Mark as done").strong())).clicked() {
                        p.done.insert(l.id.clone(), date()); changed = true;
                        st.open = next_lesson(d, p).map(|n| n.id.clone());
                    }
                    let saved = p.bookmarks.contains(&l.id);
                    if ui.button(if saved { "★ Saved to my practice set" } else { "☆ Save to my practice set" }).clicked() {
                        if saved { p.bookmarks.retain(|b| b != &l.id); } else { p.bookmarks.push(l.id.clone()); } changed = true;
                    }
                });
                return;
            }
            ui.horizontal(|ui| { ui.label(RichText::new("Skill tree").size(26.0).strong()); if ui.button("Drawing words").clicked() { go = Some(Screen::Learn); } });
            ui.label(format!("{} of {} lessons done. Lessons open up as you finish the ones before them.", p.done.len(), d.lessons.len()));
            if let Some(n) = next_lesson(d, p) {
                ui.group(|ui| { ui.horizontal(|ui| {
                    ui.vertical(|ui| { ui.label(RichText::new("Next up").small().weak()); ui.label(RichText::new(&n.title).size(18.0).strong()); ui.label(&n.goal); });
                    if ui.button("Start lesson").clicked() { st.open = Some(n.id.clone()); }
                }); });
            }
            ui.horizontal(|ui| {
                if ui.selectable_label(st.filter.is_none(), "All").clicked() { st.filter = None; }
                for l in LEVELS { if ui.selectable_label(st.filter == Some(l), level_text(l)).clicked() { st.filter = Some(l); } }
                if ui.selectable_label(st.filter == Some("saved"), format!("Saved ({})", p.bookmarks.len())).clicked() { st.filter = Some("saved"); }
            });
            let ms = milestones(d, p);
            if !ms.is_empty() { ui.label(RichText::new(format!("★ {}", ms.join("  ★ "))).color(crate::ui::ACCENT)); }
            ui.add_space(6.0);
            egui::Grid::new("skills-grid").num_columns(2).spacing([16.0, 16.0]).show(ui, |ui| {
                for (ci, c) in d.categories.iter().enumerate() {
                    let ls: Vec<_> = d.lessons.iter().filter(|l| l.cat == c.id && match st.filter { None => true, Some("saved") => p.bookmarks.contains(&l.id), Some(f) => l.level == f }).collect();
                    ui.group(|ui| { ui.vertical(|ui| {
                        ui.set_width(330.0);
                        let done = d.lessons.iter().filter(|l| l.cat == c.id && p.done.contains_key(&l.id)).count();
                        let total = d.lessons.iter().filter(|l| l.cat == c.id).count();
                        ui.label(RichText::new(format!("{}. {}", ci + 1, c.title)).strong());
                        ui.label(RichText::new(&c.summary).small().weak());
                        ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_height(6.0));
                        for l in ls {
                            let mark = if p.done.contains_key(&l.id) { "✔" } else if unlocked(l, p) { "○" } else { "·" };
                            if ui.add(egui::Button::new(format!("{mark}  {}  ·  {} · {} min", l.title, level_text(&l.level), l.minutes)).frame(false)).clicked() { st.open = Some(l.id.clone()); }
                        }
                    }); });
                    if ci % 2 == 1 { ui.end_row(); }
                }
            });
        });
    });
    if changed { let p = app.screen_state.skills.progress.clone().unwrap(); save(app, &p); }
    if let Some(sc) = go { app.go(sc); }
}
