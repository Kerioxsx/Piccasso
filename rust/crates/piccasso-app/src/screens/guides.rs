//! Guides (desktop): perspective with draggable vanishing points and horizon,
//! face and figure proportions, composition grids and a line-weight chart,
//! over a picture or blank paper; saved as PNG with the guides drawn in.
use crate::app::PiccassoApp;
use egui::{pos2, vec2, Color32, Pos2, RichText, Stroke};
use piccasso_core::guides::{composition, default_vps, face, figure, perspective, Shape, LINE_WEIGHTS};
use piccasso_core::imgproc::{fit_dims, resize_box, Rgba};

#[derive(PartialEq, Clone, Copy)]
enum Mode { Perspective, Face, Figure, Composition, Weights }

pub struct GuidesState {
    pic: Option<Rgba>,
    w: f64, h: f64,
    mode: Mode,
    points: usize, rays: usize, horizon: f64, vps: Vec<[f64; 2]>,
    heads: usize, comp: &'static str,
    bx: Option<[f64; 4]>,
    color: Color32, width: f32, opacity: f32, pic_opacity: f32, labels: bool,
    drag: Option<(u8, usize, [f64; 2])>,
    loaded: bool,
}
impl Default for GuidesState {
    fn default() -> Self {
        let (horizon, vps) = default_vps(2, 1200.0, 900.0);
        GuidesState { pic: None, w: 1200.0, h: 900.0, mode: Mode::Perspective, points: 2, rays: 14, horizon, vps, heads: 8, comp: "thirds", bx: None,
            color: Color32::from_rgb(0xd0, 0x34, 0x2c), width: 1.5, opacity: 0.9, pic_opacity: 1.0, labels: true, drag: None, loaded: false }
    }
}

impl GuidesState {
    fn set_picture(&mut self, img: Option<&Rgba>) {
        match img {
            Some(i) => { let (w, h) = fit_dims(i.width, i.height, 1400); let p = if w == i.width { i.clone() } else { resize_box(i, w, h) }; self.w = p.width as f64; self.h = p.height as f64; self.pic = Some(p); }
            None => { self.pic = None; self.w = 1200.0; self.h = 900.0; }
        }
        let (hz, v) = default_vps(self.points, self.w, self.h); self.horizon = hz; self.vps = v; self.bx = None;
    }
    /// Margin around the picture for far vanishing points (world pixels).
    fn pad(&self) -> (f64, f64, f64) {
        if self.mode != Mode::Perspective { return (0.0, 0.0, 0.0); }
        (self.w * if self.points == 1 { 0.05 } else { 0.3 }, self.h * 0.1, self.h * if self.points == 3 { 0.6 } else { 0.1 })
    }
    fn default_box(&self) -> [f64; 4] {
        if self.mode == Mode::Face { [self.w * 0.35, self.h * 0.15, self.w * 0.3, self.w * 0.3 * 1.4] } else { [self.w * 0.4, self.h * 0.05, self.w * 0.2, self.h * 0.9] }
    }
    fn shapes(&mut self) -> Vec<Shape> {
        match self.mode {
            Mode::Perspective => perspective(self.points, &self.vps, self.horizon, self.rays, self.w, self.h),
            Mode::Face => { let b = *self.bx.get_or_insert(self.default_box()); face(b[0], b[1], b[2], b[3]) }
            Mode::Figure => { let b = *self.bx.get_or_insert(self.default_box()); figure(b[0], b[1], b[2], b[3], self.heads) }
            Mode::Composition => composition(self.comp, self.w, self.h),
            Mode::Weights => vec![],
        }
    }
}

/// Draw the guide lines into a copy of the picture (for saving).
fn rasterize(st: &mut GuidesState) -> Rgba {
    let (w, h) = (st.w as usize, st.h as usize);
    let mut img = match &st.pic { Some(p) => p.clone(), None => Rgba { width: w, height: h, data: vec![255; w * h * 4] } };
    let c = st.color.to_array();
    let a = st.opacity as f64;
    let k = (st.w / 900.0).max(1.0);
    let mut dot = |img: &mut Rgba, x: f64, y: f64, r: f64| {
        let ri = r.ceil() as i64;
        for yy in (y as i64 - ri)..=(y as i64 + ri) { for xx in (x as i64 - ri)..=(x as i64 + ri) {
            if xx < 0 || yy < 0 || xx >= w as i64 || yy >= h as i64 || ((xx as f64 + 0.5 - x).powi(2) + (yy as f64 + 0.5 - y).powi(2)) > r * r { continue; }
            let i = (yy as usize * w + xx as usize) * 4;
            for ch in 0..3 { img.data[i + ch] = (img.data[i + ch] as f64 * (1.0 - a) + c[ch] as f64 * a).round() as u8; }
        } }
    };
    let line = |img: &mut Rgba, p: [f64; 2], q: [f64; 2], r: f64, dot: &mut dyn FnMut(&mut Rgba, f64, f64, f64)| {
        let n = ((q[0] - p[0]).hypot(q[1] - p[1]) / (r * 0.7).max(0.5)).ceil().max(1.0) as usize;
        for i in 0..=n { let t = i as f64 / n as f64; dot(img, p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t, r); }
    };
    for s in st.shapes() {
        match s {
            Shape::Line { a: p, b: q, w: lw, .. } => line(&mut img, p, q, (st.width as f64 * lw * k / 2.0).max(0.6), &mut dot),
            Shape::Ellipse { c: cc, rx, ry } => { let n = 180; let mut prev = [cc[0] + rx, cc[1]]; for i in 1..=n { let t = i as f64 / n as f64 * std::f64::consts::TAU; let p = [cc[0] + rx * t.cos(), cc[1] + ry * t.sin()]; line(&mut img, prev, p, (st.width as f64 * k / 2.0).max(0.6), &mut dot); prev = p; } }
            _ => {}
        }
    }
    img
}

pub fn show(app: &mut PiccassoApp, ctx: &egui::Context) {
    let st = &mut app.screen_state.guides;
    if !st.loaded { let src = app.session.as_ref().map(|s| s.src.clone()); st.set_picture(src.as_ref()); st.loaded = true; app.textures.remove("guides-pic"); }
    let mut save = false;
    let mut err: Option<String> = None;
    egui::SidePanel::left("guides-side").resizable(true).default_width(320.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Guides").size(24.0).strong());
            ui.label(RichText::new("Construction guides over your picture or blank paper.").weak());
            ui.horizontal_wrapped(|ui| {
                if ui.button("Open a picture…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| piccasso_core::decode_image_bytes(&b)) { Ok(i) => { st.set_picture(Some(&i)); app.textures.remove("guides-pic"); } Err(e) => err = Some(e) }
                    }
                }
                if let Some(s) = app.session.as_ref() { if ui.button("Use current project").clicked() { let src = s.src.clone(); st.set_picture(Some(&src)); app.textures.remove("guides-pic"); } }
                if ui.button("Blank paper").clicked() { st.set_picture(None); }
            });
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                for (m, l) in [(Mode::Perspective, "Perspective"), (Mode::Face, "Face proportions"), (Mode::Figure, "Figure proportions"), (Mode::Composition, "Composition"), (Mode::Weights, "Line weights")] {
                    if ui.selectable_label(st.mode == m, l).clicked() { st.mode = m; st.bx = None; }
                }
            });
            match st.mode {
                Mode::Perspective => {
                    ui.horizontal(|ui| { for n in 1..=3 { if ui.selectable_label(st.points == n, format!("{n}-point")).clicked() { st.points = n; let (hz, v) = default_vps(n, st.w, st.h); st.horizon = hz; st.vps = v; } } });
                    ui.add(egui::Slider::new(&mut st.rays, 4..=40).text("Lines from each point"));
                    ui.label(RichText::new("Drag the round vanishing points and the horizon line. They may sit outside the picture.").small().weak());
                }
                Mode::Face => { ui.label(RichText::new("Eyes halfway down the head; brows · nose · chin in thirds; five eye-widths across. Drag the box onto the face, its corner to resize.").small().weak()); }
                Mode::Figure => {
                    ui.horizontal(|ui| { for n in [6, 7, 8] { if ui.selectable_label(st.heads == n, format!("{n} heads")).clicked() { st.heads = n; } } });
                    ui.label(RichText::new("Drag the box from head to feet, its corner to resize.").small().weak());
                }
                Mode::Composition => { ui.horizontal_wrapped(|ui| { for (k, l) in [("thirds", "Thirds"), ("golden", "Golden ratio"), ("symmetry", "Symmetry"), ("diagonals", "Diagonals")] { if ui.selectable_label(st.comp == k, l).clicked() { st.comp = k; } } }); }
                Mode::Weights => { ui.label(RichText::new("Thin, light lines recede; thick, dark lines come forward.").small().weak()); }
            }
            ui.separator();
            ui.horizontal(|ui| { ui.label("Guide colour"); ui.color_edit_button_srgba(&mut st.color); });
            ui.add(egui::Slider::new(&mut st.width, 0.5..=5.0).text("Guide thickness"));
            ui.add(egui::Slider::new(&mut st.opacity, 0.1..=1.0).text("Guide opacity"));
            ui.add(egui::Slider::new(&mut st.pic_opacity, 0.0..=1.0).text("Picture opacity"));
            ui.checkbox(&mut st.labels, "Labels");
            ui.separator();
            if ui.add_enabled(st.mode != Mode::Weights, egui::Button::new("Save PNG with guides…")).clicked() { save = true; }
        });
    });
    egui::CentralPanel::default().show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let (px, ptop, pbot) = st.pad();
        let (ww, wh) = (st.w + 2.0 * px, st.h + ptop + pbot);
        let rect = crate::ui::compute_rect(area, ww as f32, wh as f32, &Default::default(), 10.0);
        let k = rect.width() as f64 / ww;
        let to_s = |p: [f64; 2]| -> Pos2 { pos2(rect.min.x + ((p[0] + px) * k) as f32, rect.min.y + ((p[1] + ptop) * k) as f32) };
        let to_w = |p: Pos2| -> [f64; 2] { [(p.x - rect.min.x) as f64 / k - px, (p.y - rect.min.y) as f64 / k - ptop] };
        let resp = ui.allocate_rect(rect, egui::Sense::drag());
        let painter = ui.painter_at(area);
        let pr = egui::Rect::from_min_max(to_s([0.0, 0.0]), to_s([st.w, st.h]));
        painter.rect_filled(pr, 0.0, Color32::WHITE);
        if st.mode == Mode::Weights {
            for (i, (mm, grade, usage)) in LINE_WEIGHTS.iter().enumerate() {
                let y = pr.min.y + pr.height() * (0.12 + i as f32 * 0.17);
                let wpx = (*mm as f32 / 25.4 * 96.0 * 2.0 * pr.width() / 900.0).max(1.0);
                painter.line_segment([pos2(pr.min.x + pr.width() * 0.05, y), pos2(pr.min.x + pr.width() * 0.5, y)], Stroke::new(wpx, Color32::from_gray(30)));
                painter.text(pos2(pr.min.x + pr.width() * 0.55, y - 10.0), egui::Align2::LEFT_CENTER, format!("{mm} mm · {grade}"), egui::FontId::proportional(15.0), Color32::from_gray(30));
                painter.text(pos2(pr.min.x + pr.width() * 0.55, y + 10.0), egui::Align2::LEFT_CENTER, *usage, egui::FontId::proportional(13.0), Color32::from_gray(80));
            }
            return;
        }
        if let Some(p) = &st.pic {
            let tex = app.textures.get(ctx, "guides-pic", p);
            painter.image(tex.id(), pr, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::from_white_alpha((st.pic_opacity * 255.0) as u8));
        }
        let col = st.color.gamma_multiply(st.opacity);
        let kw = (st.w / 900.0).max(1.0) as f32 * k as f32;
        for s in st.shapes() {
            match s {
                Shape::Line { a, b, w, kind } => {
                    let stroke = Stroke::new(st.width * w as f32 * kw.max(0.6), col);
                    if kind == "measure" { painter.add(egui::Shape::dashed_line(&[to_s(a), to_s(b)], stroke, 7.0, 5.0)); } else { painter.line_segment([to_s(a), to_s(b)], stroke); }
                }
                Shape::Ellipse { c, rx, ry } => {
                    let pts: Vec<Pos2> = (0..=72).map(|i| { let t = i as f64 / 72.0 * std::f64::consts::TAU; to_s([c[0] + rx * t.cos(), c[1] + ry * t.sin()]) }).collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(st.width * kw.max(0.6), col)));
                }
                Shape::Label { at, text } if st.labels => { painter.text(to_s(at), egui::Align2::LEFT_BOTTOM, text, egui::FontId::proportional(13.0), col); }
                Shape::Vp { c, label } => {
                    painter.circle_stroke(to_s(c), 10.0, Stroke::new(2.0_f32, col)); painter.circle_filled(to_s(c), 3.0, col);
                    if st.labels { painter.text(to_s(c) + vec2(13.0, -9.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(13.0), col); }
                }
                _ => {}
            }
        }
        if let (Mode::Face | Mode::Figure, Some(b)) = (st.mode, st.bx) {
            painter.rect_stroke(egui::Rect::from_min_max(to_s([b[0], b[1]]), to_s([b[0] + b[2], b[1] + b[3]])), 0.0, Stroke::new(1.5_f32, col), egui::StrokeKind::Middle);
            painter.rect_filled(egui::Rect::from_center_size(to_s([b[0] + b[2], b[1] + b[3]]), vec2(12.0, 12.0)), 0.0, col);
        }
        // dragging
        let near = 22.0 / k;
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos().map(to_w) {
                st.drag = None;
                if st.mode == Mode::Perspective {
                    if let Some(i) = st.vps.iter().take(st.points).position(|v| (v[0] - p[0]).hypot(v[1] - p[1]) < near * 1.4) { st.drag = Some((0, i, [0.0, 0.0])); }
                    else if (p[1] - st.horizon).abs() < near { st.drag = Some((1, 0, [0.0, 0.0])); }
                } else if let Some(b) = st.bx {
                    if (p[0] - b[0] - b[2]).hypot(p[1] - b[1] - b[3]) < near * 1.4 { st.drag = Some((3, 0, [0.0, 0.0])); }
                    else if p[0] > b[0] && p[0] < b[0] + b[2] && p[1] > b[1] && p[1] < b[1] + b[3] { st.drag = Some((2, 0, [p[0] - b[0], p[1] - b[1]])); }
                }
            }
        }
        if let (Some((kind, i, off)), Some(p)) = (st.drag, resp.interact_pointer_pos().map(to_w)) {
            match kind {
                0 => { st.vps[i][0] = p[0]; if i < 2 { st.horizon = p[1]; for v in st.vps.iter_mut().take(2) { v[1] = p[1]; } } else { st.vps[i][1] = p[1]; } }
                1 => { st.horizon = p[1]; for v in st.vps.iter_mut().take(2) { v[1] = p[1]; } }
                2 => { if let Some(b) = st.bx.as_mut() { b[0] = p[0] - off[0]; b[1] = p[1] - off[1]; } }
                _ => { let face = st.mode == Mode::Face; if let Some(b) = st.bx.as_mut() { b[2] = (p[0] - b[0]).max(30.0); b[3] = if face { b[2] * 1.4 } else { (p[1] - b[1]).max(60.0) }; } }
            }
        }
        if resp.drag_stopped() { st.drag = None; }
    });
    if let Some(e) = err { app.error(format!("Couldn't open the picture: {e}")); }
    if save {
        let img = rasterize(&mut app.screen_state.guides);
        if let Some(path) = rfd::FileDialog::new().set_file_name("piccasso-guide.png").add_filter("PNG image", &["png"]).save_file() {
            match piccasso_core::encode_png(&img).map_err(|e| e.to_string()).and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
                Ok(()) => app.toast(format!("Saved {}", path.display())),
                Err(e) => app.error(format!("Couldn't save: {e}")),
            }
        }
    }
}
