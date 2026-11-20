//! Shared egui helpers: textures, guide painting, toasts and confirmations.
use crate::app::DrawStepApp;
use drawstep_core::analysis::Analysis;
use drawstep_core::geometry::{apply_h, homography};
use drawstep_core::imgproc::Rgba;
use drawstep_core::lock::View;
use egui::{pos2, vec2, Color32, Pos2, Rect, Shape, Stroke};
use std::collections::HashMap;
use std::time::Instant;

pub const ACCENT: Color32 = Color32::from_rgb(0x2f, 0x74, 0x66);
pub const C_SHAPES: Color32 = Color32::from_rgb(0xc2, 0x18, 0x5b);
pub const C_SIL: Color32 = Color32::from_rgb(0xd8, 0x43, 0x15);
pub const C_INT: Color32 = Color32::from_rgb(0x15, 0x65, 0xc0);
pub const C_FRAME: Color32 = Color32::from_rgb(0x6d, 0x6a, 0x63);
pub const C_SUBJECT: Color32 = Color32::from_rgb(0xef, 0x6c, 0x00);
pub const C_LANDMARK: Color32 = Color32::from_rgb(0x2e, 0x7d, 0x32);
pub const C_LIGHT: Color32 = Color32::from_rgb(0xf9, 0xa8, 0x25);

pub fn style(ctx: &egui::Context) {
    ctx.all_styles_mut(|s| {
        s.spacing.button_padding = vec2(12.0, 8.0);
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.interact_size.y = 36.0;
        s.visuals.selection.bg_fill = ACCENT;
        s.visuals.hyperlink_color = ACCENT;
        for (_, f) in s.text_styles.iter_mut() {
            f.size *= 1.12;
        }
    });
}

/// GPU textures keyed by name; cleared when renders change.
#[derive(Default)]
pub struct Textures {
    map: HashMap<String, egui::TextureHandle>,
}
impl Textures {
    pub fn clear(&mut self) {
        self.map.clear();
    }
    pub fn id(&self, key: &str) -> Option<egui::TextureId> {
        self.map.get(key).map(|t| t.id())
    }
    pub fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }
    pub fn get(&mut self, ctx: &egui::Context, key: &str, img: &Rgba) -> egui::TextureHandle {
        self.map
            .entry(key.to_string())
            .or_insert_with(|| ctx.load_texture(key, to_color_image(img), egui::TextureOptions::LINEAR))
            .clone()
    }
    pub fn set(&mut self, ctx: &egui::Context, key: &str, img: &Rgba) -> egui::TextureHandle {
        let t = ctx.load_texture(key, to_color_image(img), egui::TextureOptions::LINEAR);
        self.map.insert(key.to_string(), t.clone());
        t
    }
}

pub fn to_color_image(img: &Rgba) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.data)
}

/// Contain-fit image of size (w,h) inside `area`, then apply zoom/pan.
pub fn compute_rect(area: Rect, w: f32, h: f32, view: &View, pad: f32) -> Rect {
    let s = ((area.width() - 2.0 * pad) / w).min((area.height() - 2.0 * pad) / h).max(0.01);
    let size = vec2(w * s * view.zoom as f32, h * s * view.zoom as f32);
    let c = area.center() + vec2(view.pan_x as f32, view.pan_y as f32);
    Rect::from_center_size(c, size)
}

fn map(rect: Rect, p: [f64; 2]) -> Pos2 {
    pos2(rect.min.x + p[0] as f32 * rect.width(), rect.min.y + p[1] as f32 * rect.height())
}

fn dashed(painter: &egui::Painter, pts: &[Pos2], closed: bool, stroke: Stroke, dash: f32) {
    let mut v = pts.to_vec();
    if closed && !pts.is_empty() {
        v.push(pts[0]);
    }
    painter.extend(Shape::dashed_line(&v, stroke, dash, dash * 0.7));
}

/// Paint vector layers of the analysis over `rect` (where the whole image maps).
pub fn draw_layers(painter: &egui::Painter, a: &Analysis, layers: &[String], rect: Rect, alpha: f32, labels: bool) {
    let has = |l: &str| layers.iter().any(|x| x == l);
    let lw = (rect.width() / 420.0).max(1.5);
    let c = |col: Color32| col.gamma_multiply(alpha);
    let b = a.subject.bbox;
    let sb = Rect::from_min_max(map(rect, [b[0], b[1]]), map(rect, [b[2], b[3]]));
    if has("frame") {
        let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
        dashed(painter, &corners, true, Stroke::new(lw, c(C_FRAME)), lw * 4.0);
        for p in corners {
            painter.circle_filled(p, lw * 2.5, c(C_FRAME));
        }
    }
    if has("grid") {
        for f in [1.0 / 3.0, 2.0 / 3.0] {
            let s = Stroke::new(lw * 0.7, c(C_FRAME).gamma_multiply(0.5));
            painter.line_segment([map(rect, [f, 0.0]), map(rect, [f, 1.0])], s);
            painter.line_segment([map(rect, [0.0, f]), map(rect, [1.0, f])], s);
        }
    }
    if has("subjectBox") {
        dashed(painter, &[sb.left_top(), sb.right_top(), sb.right_bottom(), sb.left_bottom()], true, Stroke::new(lw, c(C_SUBJECT)), lw * 3.0);
    }
    if has("measures") {
        let off = lw * 6.0;
        let s = Stroke::new(lw, c(C_SUBJECT));
        painter.line_segment([pos2(sb.min.x, sb.min.y - off), pos2(sb.max.x, sb.min.y - off)], s);
        painter.line_segment([pos2(sb.min.x - off, sb.min.y), pos2(sb.min.x - off, sb.max.y)], s);
        dashed(painter, &[pos2(sb.center().x, sb.min.y), pos2(sb.center().x, sb.max.y)], false, Stroke::new(lw, c(C_SUBJECT).gamma_multiply(0.5)), lw * 2.0);
        dashed(painter, &[pos2(sb.min.x, sb.center().y), pos2(sb.max.x, sb.center().y)], false, Stroke::new(lw, c(C_SUBJECT).gamma_multiply(0.5)), lw * 2.0);
        if labels {
            let asp = a.subject.aspect;
            let (wt, ht) = if asp >= 1.0 { (format!("{} wide", drawstep_core::steps::fmt1(asp)), "1 tall".to_string()) } else { ("1 wide".to_string(), format!("{} tall", drawstep_core::steps::fmt1(1.0 / asp))) };
            label(painter, &wt, pos2(sb.center().x, sb.min.y - off), C_SUBJECT);
            label(painter, &ht, pos2(sb.min.x - off, sb.center().y), C_SUBJECT);
        }
    }
    if has("shapes") {
        for (i, s) in a.shapes.iter().enumerate() {
            let center = map(rect, [s.cx, s.cy]);
            let (rx, ry) = (s.rx as f32 * rect.width(), s.ry as f32 * rect.width());
            let stroke = Stroke::new(lw * 1.2, c(C_SHAPES));
            if s.kind == "box" {
                painter.rect_stroke(Rect::from_center_size(center, vec2(rx * 2.0, ry * 2.0)), 0.0, stroke, egui::StrokeKind::Middle);
            } else {
                let t = (s.angle as f32).to_radians();
                let pts: Vec<Pos2> = (0..72).map(|k| {
                    let a = k as f32 / 72.0 * std::f32::consts::TAU;
                    let (ex, ey) = (a.cos() * rx, a.sin() * ry);
                    pos2(center.x + ex * t.cos() - ey * t.sin(), center.y + ex * t.sin() + ey * t.cos())
                }).collect();
                painter.add(Shape::closed_line(pts, stroke));
            }
            painter.line_segment([center - vec2(lw * 3.0, 0.0), center + vec2(lw * 3.0, 0.0)], stroke);
            painter.line_segment([center - vec2(0.0, lw * 3.0), center + vec2(0.0, lw * 3.0)], stroke);
            if labels {
                label(painter, &format!("{}", i + 1), center - vec2(0.0, ry + lw * 6.0), C_SHAPES);
            }
        }
    }
    if has("silhouette") {
        for s in &a.silhouettes {
            let pts: Vec<Pos2> = s.points.iter().map(|p| map(rect, *p)).collect();
            painter.add(Shape::closed_line(pts, Stroke::new(lw * 1.6, c(C_SIL))));
        }
    }
    if has("internal") {
        for s in &a.internals {
            let pts: Vec<Pos2> = s.points.iter().map(|p| map(rect, *p)).collect();
            dashed(painter, &pts, s.closed, Stroke::new(lw * 1.1, c(C_INT)), lw * 3.0);
        }
    }
    if has("landmarks") {
        for l in &a.landmarks {
            let p = map(rect, [l.x, l.y]);
            painter.circle(p, lw * 2.6, c(C_LANDMARK), Stroke::new(lw * 0.8, Color32::WHITE));
        }
    }
    if has("light") && a.values.light_dir != "even" {
        let code = a.values.light_dir.as_bytes();
        let dx = match code[1] { b'l' => -1.0, b'r' => 1.0, _ => 0.0 };
        let dy = match code[0] { b't' => -1.0, b'b' => 1.0, _ => 0.0 };
        let len = sb.width().max(sb.height()) * 0.62;
        let sun = sb.center() + vec2(dx, dy) * len;
        painter.circle_filled(sun, lw * 6.0, c(C_LIGHT));
        painter.arrow(sun - vec2(dx, dy) * lw * 8.0, -vec2(dx, dy) * len * 0.55, Stroke::new(lw * 1.6, c(C_LIGHT)));
    }
}

fn label(painter: &egui::Painter, text: &str, at: Pos2, color: Color32) {
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(13.0), Color32::WHITE);
    let r = Rect::from_center_size(at, galley.size() + vec2(8.0, 4.0));
    painter.rect_filled(r, 3.0, color.gamma_multiply(0.92));
    painter.galley(r.min + vec2(4.0, 2.0), galley, Color32::WHITE);
}

/// Draw a texture warped onto a quad (TL,TR,BR,BL) with a subdivided mesh (≈ perspective).
pub fn paint_quad(painter: &egui::Painter, tex: egui::TextureId, quad: [[f64; 2]; 4], tint: Color32) {
    let Some(h) = homography(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], &quad) else { return };
    let n = 20u32;
    let mut mesh = egui::Mesh::with_texture(tex);
    for j in 0..=n {
        for i in 0..=n {
            let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
            let p = apply_h(&h, u, v);
            mesh.vertices.push(egui::epaint::Vertex { pos: pos2(p[0] as f32, p[1] as f32), uv: pos2(u as f32, v as f32), color: tint });
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            mesh.add_triangle(a, a + 1, a + n + 2);
            mesh.add_triangle(a, a + n + 2, a + n + 1);
        }
    }
    painter.add(Shape::mesh(mesh));
}

pub fn show_toasts(app: &mut DrawStepApp, ctx: &egui::Context) {
    let now = Instant::now();
    app.toasts.retain(|t| t.until > now);
    if app.toasts.is_empty() {
        return;
    }
    egui::Area::new(egui::Id::new("toasts")).anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -24.0)).order(egui::Order::Foreground).show(ctx, |ui| {
        for t in &app.toasts {
            let fill = if t.error { Color32::from_rgb(0xb3, 0x26, 0x1e) } else { Color32::from_gray(35) };
            egui::Frame::new().fill(fill).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                ui.set_max_width(460.0);
                ui.label(egui::RichText::new(&t.text).color(Color32::WHITE));
            });
        }
    });
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}

/// A pending confirmation dialog.
pub struct Confirm {
    pub title: String,
    pub body: String,
    pub ok: String,
    pub action: ConfirmAction,
}
pub enum ConfirmAction {
    DeleteProject(String),
    RestartTutorial,
}

pub fn show_confirm(app: &mut DrawStepApp, ctx: &egui::Context) {
    let Some(c) = app.confirm.as_ref() else { return };
    let mut result = None;
    egui::Modal::new(egui::Id::new("confirm")).show(ctx, |ui| {
        ui.set_max_width(420.0);
        ui.heading(&c.title);
        ui.label(&c.body);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() { result = Some(false); }
            if ui.add(egui::Button::new(egui::RichText::new(&c.ok).color(Color32::WHITE)).fill(Color32::from_rgb(0xb3, 0x26, 0x1e))).clicked() { result = Some(true); }
        });
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) { result = Some(false); }
    });
    match result {
        Some(true) => {
            let c = app.confirm.take().unwrap();
            match c.action {
                ConfirmAction::DeleteProject(id) => {
                    if let Some(store) = &app.store {
                        match store.delete(&id) {
                            Ok(()) => {
                                if app.session.as_ref().map_or(false, |s| s.project.id == id) { app.session = None; }
                                app.toast("Project deleted");
                            }
                            Err(e) => app.error(format!("Could not delete: {e}")),
                        }
                    }
                }
                ConfirmAction::RestartTutorial => {
                    if let Some(s) = app.session.as_mut() { s.project.restart_tutorial(); }
                    app.save_project();
                }
            }
        }
        Some(false) => app.confirm = None,
        None => {}
    }
}

/// Large primary button.
pub fn primary(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(text).color(Color32::WHITE).strong()).fill(ACCENT).min_size(vec2(0.0, 42.0)))
}

pub fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(26.0).strong());
}
