//! 3D sandbox (desktop): shapes, orbit camera, perspective/orthographic, light
//! and shadows, vanishing points, solid/wireframe/silhouette; scenes saved in
//! the data folder.
use crate::app::PiccassoApp;
use egui::{pos2, Color32, Pos2, RichText, Stroke};
use piccasso_core::sandbox3d::{build, default_scene, object_axes, project, vanishing_point, Object, Scene};

const COLORS: [(u8, u8, u8); 6] = [(0xc9, 0xb1, 0x8f), (0x9f, 0xb4, 0xc7), (0xb9, 0xc7, 0x9f), (0xd1, 0xa3, 0xa3), (0xc2, 0xb0, 0xd6), (0xe0, 0xc9, 0x8a)];
const SHAPES: [(&str, &str); 5] = [("cube", "Cube"), ("sphere", "Sphere"), ("cylinder", "Cylinder"), ("cone", "Cone"), ("pyramid", "Pyramid")];

#[derive(PartialEq, Clone, Copy)]
enum Look { Solid, Wire, Silhouette }

pub struct SandboxState { scene: Scene, sel: usize, look: Look, construction: bool, vps: bool, shadows: bool, grid: bool }
impl Default for SandboxState { fn default() -> Self { SandboxState { scene: default_scene(), sel: 0, look: Look::Solid, construction: true, vps: false, shadows: true, grid: true } } }

fn scenes_path(app: &PiccassoApp) -> std::path::PathBuf { app.data_dir.join("sandbox-scenes.json") }

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let mut saved: std::collections::BTreeMap<String, Scene> = std::fs::read_to_string(scenes_path(app)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut save_now = false;
    let st = &mut app.screen_state.sandbox;
    egui::SidePanel::left("sandbox-side").resizable(true).default_width(330.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("3D sandbox").size(24.0).strong());
            ui.label(RichText::new("Build simple 3D shapes to understand form, light and perspective. Drag to turn the view, scroll to zoom, click a shape to select it.").weak());
            ui.horizontal_wrapped(|ui| { for (k, l) in SHAPES { if ui.button(format!("+ {l}")).clicked() { st.scene.objects.push(Object { kind: k.into(), pos: [0.0, 0.0, 0.0], rot: [0.0; 3], scale: [1.0; 3] }); st.sel = st.scene.objects.len() - 1; } } });
            ui.horizontal_wrapped(|ui| { for (i, o) in st.scene.objects.iter().enumerate() { let name = SHAPES.iter().find(|s| s.0 == o.kind).map(|s| s.1).unwrap_or("?"); if ui.selectable_label(st.sel == i, format!("{}. {name}", i + 1)).clicked() { st.sel = i; } } });
            let mut action = 0;
            if let Some(o) = st.scene.objects.get_mut(st.sel) {
                let [px, py, pz] = &mut o.pos; let [rx, ry, rz] = &mut o.rot; let [sx, sy, sz] = &mut o.scale;
                for (label, v, lo, hi) in [("Left / right", px, -5.0, 5.0), ("Front / back", pz, -5.0, 5.0), ("Lift", py, 0.0, 4.0), ("Turn", ry, -180.0, 180.0), ("Tilt forward", rx, -90.0, 90.0), ("Tilt sideways", rz, -90.0, 90.0), ("Width", sx, 0.2, 5.0), ("Height", sy, 0.2, 5.0), ("Depth", sz, 0.2, 5.0)] {
                    ui.add(egui::Slider::new(v, lo..=hi).text(label));
                }
                ui.horizontal(|ui| { if ui.button("Duplicate").clicked() { action = 1; } if ui.button("Remove").clicked() { action = 2; } });
            }
            match action {
                1 => { let mut c = st.scene.objects[st.sel].clone(); c.pos[0] += 0.8; c.pos[2] += 0.8; st.scene.objects.push(c); st.sel = st.scene.objects.len() - 1; }
                2 => { st.scene.objects.remove(st.sel); st.sel = st.sel.saturating_sub(1); }
                _ => {}
            }
            ui.separator();
            ui.horizontal(|ui| { ui.selectable_value(&mut st.look, Look::Solid, "Solid"); ui.selectable_value(&mut st.look, Look::Wire, "Wireframe"); ui.selectable_value(&mut st.look, Look::Silhouette, "Silhouette"); });
            ui.horizontal(|ui| { ui.selectable_value(&mut st.scene.cam.ortho, false, "Perspective"); ui.selectable_value(&mut st.scene.cam.ortho, true, "Orthographic"); });
            if ui.add(egui::Slider::new(&mut st.scene.cam.focal, 0.8..=4.0).text("Lens (wide to long)")).changed() { st.scene.cam.dist = 3.5 * st.scene.cam.focal; }
            ui.checkbox(&mut st.construction, "Construction lines (each shape's axes)");
            ui.checkbox(&mut st.vps, "Vanishing points of the selected shape");
            ui.checkbox(&mut st.shadows, "Shadows");
            ui.checkbox(&mut st.grid, "Ground grid");
            ui.add(egui::Slider::new(&mut st.scene.light_az, -180.0..=180.0).text("Light direction"));
            ui.add(egui::Slider::new(&mut st.scene.light_el, 5.0..=90.0).text("Light height"));
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save scene").clicked() { save_now = true; }
                let names: Vec<String> = saved.keys().cloned().collect();
                egui::ComboBox::from_id_salt("scenes").selected_text("Open a saved scene").show_ui(ui, |ui| { for n in names { if ui.button(&n).clicked() { st.scene = saved[&n].clone(); st.sel = 0; } } });
                if ui.button("Reset").clicked() { st.scene = default_scene(); st.sel = 0; }
            });
        });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, egui::Sense::click_and_drag());
        let d = resp.drag_delta();
        if d.length() > 0.0 { st.scene.cam.yaw -= d.x as f64 * 0.4; st.scene.cam.pitch = (st.scene.cam.pitch + d.y as f64 * 0.3).clamp(-5.0, 85.0); }
        if resp.hovered() { let s = ui.input(|i| i.smooth_scroll_delta.y); if s != 0.0 { let f = if s < 0.0 { 1.08 } else { 1.0 / 1.08 }; st.scene.cam.dist = (st.scene.cam.dist * f).clamp(2.5, 30.0); st.scene.cam.ortho_size = (st.scene.cam.ortho_size * f).clamp(2.0, 25.0); } }
        let (w, h) = (area.width() as f64, area.height() as f64);
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 8.0, Color32::from_rgb(0xf7, 0xf4, 0xec));
        let (faces, shadows, b) = build(&st.scene, w, h);
        let to = |p: [f64; 3]| -> Pos2 { pos2(area.min.x + p[0] as f32, area.min.y + p[1] as f32) };
        if st.grid {
            for i in -6..=6 { for (a, c) in [([i as f64, 0.0, -6.0], [i as f64, 0.0, 6.0]), ([-6.0, 0.0, i as f64], [6.0, 0.0, i as f64])] {
                let (pa, pc) = (project(a, &st.scene.cam, &b, w, h), project(c, &st.scene.cam, &b, w, h));
                if pa[2] > 0.05 && pc[2] > 0.05 { painter.line_segment([to(pa), to(pc)], Stroke::new(1.0_f32, Color32::from_black_alpha(22))); }
            } }
        }
        if st.shadows && st.look != Look::Wire {
            // shadow polygons drawn faintly; overlaps are rare enough at this opacity
            for poly in &shadows { if poly.iter().any(|p| p[2] <= 0.05) { continue; } painter.add(egui::Shape::convex_polygon(poly.iter().map(|p| to(*p)).collect(), Color32::from_black_alpha(14), Stroke::NONE)); }
        }
        for f in &faces {
            if f.pts.iter().any(|p| p[2] <= 0.05) { continue; }
            let pts: Vec<Pos2> = f.pts.iter().map(|p| to(*p)).collect();
            match st.look {
                Look::Solid => { let c = COLORS[f.obj % 6]; let lit = 0.38 + 0.62 * f.shade; let col = Color32::from_rgb((c.0 as f64 * lit) as u8, (c.1 as f64 * lit) as u8, (c.2 as f64 * lit) as u8); painter.add(egui::Shape::convex_polygon(pts, col, Stroke::new(1.0_f32, col))); }
                Look::Silhouette => { painter.add(egui::Shape::convex_polygon(pts, Color32::from_gray(29), Stroke::new(1.0_f32, Color32::from_gray(29)))); }
                Look::Wire => { let mut p2 = pts.clone(); p2.push(pts[0]); painter.add(egui::Shape::line(p2, Stroke::new(1.2_f32, if f.obj == st.sel { crate::ui::ACCENT } else { Color32::from_gray(50) }))); }
            }
        }
        let axis_col = [Color32::from_rgb(200, 60, 50), Color32::from_rgb(40, 130, 70), Color32::from_rgb(40, 90, 190)];
        if st.construction && st.look != Look::Silhouette {
            for o in &st.scene.objects {
                let c = [o.pos[0], o.pos[1] + o.scale[1] / 2.0, o.pos[2]];
                for (ai, ax) in object_axes(o).iter().enumerate() {
                    let len = o.scale[ai] * 0.75;
                    let (pa, pc) = (project([c[0] - ax[0] * len, c[1] - ax[1] * len, c[2] - ax[2] * len], &st.scene.cam, &b, w, h), project([c[0] + ax[0] * len, c[1] + ax[1] * len, c[2] + ax[2] * len], &st.scene.cam, &b, w, h));
                    if pa[2] > 0.05 && pc[2] > 0.05 { painter.line_segment([to(pa), to(pc)], Stroke::new(1.5_f32, axis_col[ai].gamma_multiply(0.8))); }
                }
            }
        }
        if st.vps {
            if let Some(o) = st.scene.objects.get(st.sel) {
                if st.scene.cam.ortho { painter.text(area.min + egui::vec2(10.0, 10.0), egui::Align2::LEFT_TOP, "Orthographic view: parallel edges stay parallel (no vanishing points).", egui::FontId::proportional(14.0), Color32::from_gray(80)); }
                let axes = object_axes(o);
                for (ai, ax) in axes.iter().enumerate() {
                    let Some(vp) = vanishing_point(*ax, &st.scene.cam, &b, w, h) else { continue };
                    let vps = to([vp[0], vp[1], 1.0]);
                    for sx in [-0.5, 0.5] { for sz in [-0.5, 0.5] { for sy in [0.0, 1.0] {
                        let l = [sx * o.scale[0], sy * o.scale[1], sz * o.scale[2]];
                        let wp = [o.pos[0] + axes[0][0] * l[0] + axes[1][0] * l[1] + axes[2][0] * l[2], o.pos[1] + axes[0][1] * l[0] + axes[1][1] * l[1] + axes[2][1] * l[2], o.pos[2] + axes[0][2] * l[0] + axes[1][2] * l[1] + axes[2][2] * l[2]];
                        let p = project(wp, &st.scene.cam, &b, w, h);
                        if p[2] > 0.05 { painter.line_segment([to(p), vps], Stroke::new(1.0_f32, axis_col[ai].gamma_multiply(0.55))); }
                    } } }
                    painter.circle_filled(vps, 6.0, axis_col[ai]);
                }
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let q = [(p.x - area.min.x) as f64, (p.y - area.min.y) as f64];
                let inside = |poly: &[[f64; 3]]| { let mut c = false; let n = poly.len(); let mut j = n - 1; for i in 0..n { let (a, bb) = (poly[i], poly[j]); if (a[1] > q[1]) != (bb[1] > q[1]) && q[0] < (bb[0] - a[0]) * (q[1] - a[1]) / (bb[1] - a[1]) + a[0] { c = !c; } j = i; } c };
                if let Some(f) = faces.iter().rev().find(|f| inside(&f.pts)) { st.sel = f.obj; }
            }
        }
    });
    if save_now {
        let name = format!("Scene {}", saved.len() + 1);
        saved.insert(name.clone(), app.screen_state.sandbox.scene.clone());
        let _ = std::fs::create_dir_all(&app.data_dir);
        let _ = std::fs::write(scenes_path(app), serde_json::to_string(&saved).unwrap_or_default());
        app.toast(format!("Saved as “{name}”."));
    }
}
