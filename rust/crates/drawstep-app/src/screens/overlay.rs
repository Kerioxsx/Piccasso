//! Camera / photo overlay ("AR-like" tracing aid on desktop).
//! Desktop has no ARKit/ARCore: the guide is drawn over a live camera view
//! (with the `camera` feature) or a photo of the paper, aligned manually,
//! by 4-corner calibration, or by automatic paper detection. Nothing is projected.
use crate::app::{DrawStepApp, Screen, Session};
use crate::ui::{compute_rect, paint_quad};
use drawstep_core::capabilities::OverlayMode;
use drawstep_core::geometry::{apply_h, detect_paper, homography, order_corners, transformed_rect};
use drawstep_core::imgproc::{fit_dims, resize_box, to_gray, Rgba};
use drawstep_core::lock::View;
use drawstep_core::raster;
use drawstep_core::shared::shared;
use drawstep_core::steps::paper_layout;
use egui::{pos2, vec2, Color32, Pos2, Rect, RichText, Sense, Stroke};
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

const LIMITS: &str = "The camera view is an alignment aid: the guide is drawn on your screen over the camera picture, not projected onto the paper. Alignment drifts if the camera or paper moves, and the app cannot correct for every movement. Fix the webcam above the paper, use even light and tape the paper down. Desktop computers have no AR plane tracking; for true AR use the phone/web version on a supported device.";

pub struct OverlayState {
    photo: Option<Rgba>,
    photo_gen: u64,
    #[cfg(feature = "camera")]
    feed: Option<crate::camera::CameraFeed>,
    #[cfg_attr(not(feature = "camera"), allow(dead_code))]
    frame_gen: u64,
    media: Option<Rgba>,
    corners: Option<[[f64; 2]; 4]>, // paper corners, normalized to the media rect
    scale: f64,
    rotation: f64,
    offset: [f64; 2],
    opacity: f32,
    layers: BTreeSet<&'static str>,
    composite_key: String,
    calibrating: bool,
    follow: bool,
    locked: bool,
    last_detect: Option<Instant>,
    status: (String, Color32),
    drag_handle: Option<usize>,
    init: bool,
    composite: Option<Rgba>,
    phone_source: bool,
}
impl Default for OverlayState {
    fn default() -> Self {
        OverlayState {
            photo: None, photo_gen: 0,
            #[cfg(feature = "camera")]
            feed: None,
            frame_gen: 0, media: None, corners: None, scale: 1.0, rotation: 0.0, offset: [0.0, 0.0], opacity: 0.5,
            layers: ["outline"].into_iter().collect(), composite_key: String::new(), calibrating: false, follow: false, locked: false,
            last_detect: None, status: ("Starting…".into(), Color32::GRAY), drag_handle: None, init: false, composite: None, phone_source: false,
        }
    }
}

/// Transparent overlay raster: rasters become ink (luminance → alpha), vectors are rasterized.
fn compose(sess: &Session, layers: &BTreeSet<&'static str>) -> Rgba {
    let base = sess.renders.get("original").cloned().unwrap_or_else(|| sess.src.clone());
    let (w, h) = fit_dims(base.width, base.height, 1280);
    let mut out = Rgba::new(w, h);
    let blend = |out: &mut Rgba, img: &Rgba, as_ink: bool| {
        let img = resize_box(img, w, h);
        for i in 0..w * h {
            let p = &img.data[i * 4..i * 4 + 4];
            let (src_rgb, a) = if as_ink {
                let l = (77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8;
                ([p[0], p[1], p[2]], (255 - l) as f32 / 255.0)
            } else { ([p[0], p[1], p[2]], 1.0) };
            if a <= 0.0 { continue; }
            let o = &mut out.data[i * 4..i * 4 + 4];
            let oa = o[3] as f32 / 255.0;
            let na = a + oa * (1.0 - a);
            for c in 0..3 { o[c] = ((src_rgb[c] as f32 * a + o[c] as f32 * oa * (1.0 - a)) / na.max(1e-6)) as u8; }
            o[3] = (na * 255.0) as u8;
        }
    };
    if layers.contains("reference") { blend(&mut out, &base, false); }
    for (k, key) in [("shading", "values"), ("outline", "outline")] {
        if layers.contains(k) { if let Some(r) = sess.renders.get(key) { blend(&mut out, r, true); } }
    }
    let step = sess.project.current_step();
    if layers.contains("step") {
        if let Some(r) = step.and_then(|s| sess.renders.get(&s.base)) { blend(&mut out, r, true); }
    }
    if let Some(a) = &sess.project.analysis {
        let mut v: Vec<String> = vec![];
        if layers.contains("shapes") { v.push("shapes".into()); }
        if layers.contains("contours") { v.extend(["silhouette".into(), "internal".into()]); }
        if layers.contains("step") { if let Some(s) = step { v.extend(s.layers.iter().cloned()); } }
        raster::draw_layers(&mut out, a, &v);
    }
    out
}

fn status(st: &mut OverlayState, text: &str, c: Color32) { st.status = (text.to_string(), c); }

pub fn show(app: &mut DrawStepApp, ctx: &egui::Context) {
    let cap = crate::camera::capability();
    let Some(sess) = app.session.as_mut() else { return };
    let st = &mut app.screen_state.overlay;
    let ph = &mut app.screen_state.phone;
    let mut exit = false;
    if !st.init {
        st.init = true;
        st.opacity = sess.project.ar.opacity as f32;
        st.scale = sess.project.ar.scale;
        st.rotation = sess.project.ar.rotation;
        if let Some(c) = &sess.project.ar.corners { if c.len() == 4 { st.corners = Some([c[0], c[1], c[2], c[3]]); } }
        #[cfg(feature = "camera")]
        if cap.mode == OverlayMode::Camera { st.feed = Some(crate::camera::CameraFeed::start()); }
    }
    // Media: latest camera frame or the chosen photo.
    #[cfg(feature = "camera")]
    if let Some(feed) = &st.feed {
        if let Some((n, img)) = feed.latest() { if n != st.frame_gen { st.frame_gen = n; let _ = app.textures.set(ctx, "overlay-media", &img); st.media = Some(img); } }
        if let Some(e) = feed.error() { status(st, &format!("Camera error: {e}"), Color32::RED); }
        ctx.request_repaint_after(Duration::from_millis(33));
    }
    if let Some((g, img)) = ph.frame_after(ph.last_gen) {
        ph.last_gen = g;
        #[cfg(feature = "camera")]
        { st.feed = None; }
        let _ = app.textures.set(ctx, "overlay-media", &img);
        st.media = Some(img);
        st.phone_source = true;
    }
    if ph.running() { ctx.request_repaint_after(Duration::from_millis(50)); }
    if st.photo_gen == 1 { if let Some(p) = &st.photo { let _ = app.textures.set(ctx, "overlay-media", p); st.media = Some(p.clone()); } st.photo_gen = 2; }

    let key = format!("{:?}|{}|{}", st.layers, sess.renders_hash, sess.project.progress.current_step);
    if st.composite_key != key {
        let c = compose(sess, &st.layers);
        let _ = app.textures.set(ctx, "overlay-guide", &c);
        st.composite = Some(c);
        st.composite_key = key;
    }
    let layout = paper_layout(&sess.project.prep.paper, &shared().paper.presets, sess.project.analysis.as_ref().map(|a| a.aspect).unwrap_or(1.0), shared().paper.default_margin_mm);

    egui::TopBottomPanel::top("ov-top").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button("✖ Exit").clicked() { exit = true; }
            ui.label(RichText::new(format!("● {}", st.status.0)).color(st.status.1).strong());
            ui.separator();
            ui.label(RichText::new(&cap.status).small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.selectable_label(ph.open, "📱 Use my phone").on_hover_text("Use your phone as this computer's camera, or as an AR viewer of the guide").clicked() {
                    ph.open = !ph.open;
                    if ph.open && !ph.running() { ph.start(); }
                }
                if ui.button("Use a photo of my paper").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                        match std::fs::read(&path).map_err(|e| e.to_string()).and_then(|b| drawstep_core::decode_image_bytes(&b)) {
                            Ok(img) => {
                                let (w, h) = fit_dims(img.width, img.height, 1600);
                                st.photo = Some(resize_box(&img, w, h)); st.photo_gen = 1;
                                #[cfg(feature = "camera")]
                                { st.feed = None; }
                                st.follow = false;
                            }
                            Err(e) => app.toasts.push(crate::app::Toast { text: e, error: true, until: Instant::now() + Duration::from_secs(6) }),
                        }
                    }
                }
            });
        });
    });
    egui::TopBottomPanel::bottom("ov-panel").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::Slider::new(&mut st.opacity, 0.05..=1.0).text("Guide opacity"));
            ui.add_enabled_ui(!st.locked, |ui| {
                if ui.add(egui::Slider::new(&mut st.scale, 0.2..=3.0).text("Size")).changed() { st.corners = None; }
                if ui.add(egui::Slider::new(&mut st.rotation, -180.0..=180.0).text("Rotation °")).changed() { st.corners = None; }
            });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Show:");
            for (k, l) in [("reference", "Picture"), ("outline", "Outline"), ("shapes", "Shapes"), ("contours", "Contours"), ("shading", "Shading"), ("step", "Current step")] {
                let on = st.layers.contains(k);
                if ui.selectable_label(on, l).clicked() { if on { st.layers.remove(k); } else { st.layers.insert(k); } }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(!st.locked, |ui| {
                for (t, d) in [("Left", [-8.0, 0.0]), ("Up", [0.0, -8.0]), ("Down", [0.0, 8.0]), ("Right", [8.0, 0.0])] {
                    if ui.button(t).clicked() { st.offset[0] += d[0]; st.offset[1] += d[1]; st.corners = None; }
                }
                if ui.button("Recenter").clicked() { st.offset = [0.0, 0.0]; st.scale = 1.0; st.rotation = 0.0; st.corners = None; }
                if ui.selectable_label(st.calibrating, "Calibrate corners").clicked() {
                    st.calibrating = !st.calibrating;
                    if st.calibrating && st.corners.is_none() { st.corners = Some([[0.12, 0.12], [0.88, 0.12], [0.88, 0.88], [0.12, 0.88]]); }
                }
                if ui.button("Detect paper").clicked() { detect(st, true, app_toast(&mut app.toasts)); }
                #[cfg(feature = "camera")]
                if st.feed.is_some() { ui.checkbox(&mut st.follow, "Follow paper (experimental)"); }
            });
            if ui.checkbox(&mut st.locked, "Lock alignment").changed() && st.locked { st.follow = false; }
        });
        ui.collapsing("About the camera overlay", |ui| { ui.label(RichText::new(LIMITS).small()); for r in &cap.reasons { ui.label(RichText::new(r).small().weak()); } });
    });

    if ph.open {
        let mut open = true;
        egui::Window::new("Use your phone").open(&mut open).resizable(false).default_width(360.0).anchor(egui::Align2::RIGHT_TOP, vec2(-12.0, 60.0)).show(ctx, |ui| {
            if let Some(e) = &ph.error { ui.colored_label(Color32::from_rgb(0xb3, 0x26, 0x1e), e); }
            if let (Some(url), Some(qr)) = (ph.url(), ph.qr()) {
                ui.label("1. Connect the phone to the same Wi-Fi as this computer.");
                ui.label("2. Scan this code with the phone's camera:");
                let tex = app.textures.get(ctx, "phone-qr", &qr);
                ui.image(egui::load::SizedTexture::new(tex.id(), vec2(220.0, 220.0)));
                ui.horizontal(|ui| { ui.label(RichText::new(&url).monospace().small()); if ui.small_button("Copy").clicked() { ctx.copy_text(url.clone()); } });
                ui.label(RichText::new("3. The phone warns about the certificate (it is made by this computer just now). Choose “Advanced → Continue/Visit website”. This keeps the camera link encrypted on your network.").small().weak());
                ui.label("4. On the phone choose “Be the computer's camera”, or “See the guide here” for an AR view on the phone.");
                let (frames, viewer) = ph.stats();
                ui.label(RichText::new(if frames > 0 { format!("Receiving the phone camera ({frames} frames).") } else if viewer { "The phone is showing the guide.".into() } else { "Waiting for the phone…".into() }).strong());
                if ui.button("Send guide to phone").on_hover_text("Shares the current overlay (layers you chose) with the phone's AR view").clicked() {
                    if let Some(c) = &st.composite {
                        if let Ok(png) = drawstep_core::encode_png(c) {
                            ph.set_overlay(png, format!("{{\"name\":{:?}}}", sess.project.name));
                            st.status = ("Guide sent to the phone".into(), Color32::from_rgb(0x2e, 0x7d, 0x32));
                        }
                    }
                }
                if ui.button("Stop sharing").clicked() { ph.stop(); ph.open = false; st.phone_source = false; }
            }
        });
        if !open { ph.open = false; }
    }
    if st.follow && !st.locked && st.last_detect.map_or(true, |t| t.elapsed() > Duration::from_millis(250)) {
        detect(st, false, app_toast(&mut app.toasts));
    }
    if !st.follow {
        if st.locked { status(st, "Alignment locked", Color32::from_rgb(0x2e, 0x7d, 0x32)); }
        else if st.media.is_none() { status(st, if cap.mode == OverlayMode::Camera { "Waiting for camera…" } else { "AR unavailable: choose a photo of your paper" }, Color32::from_rgb(0xef, 0x6c, 0x00)); }
        else { status(st, if st.phone_source { "Phone camera · manual alignment" } else { "Manual alignment (no tracking)" }, Color32::from_rgb(0xef, 0x6c, 0x00)); }
    }

    egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::BLACK)).show(ctx, |ui| {
        let area = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(area, Sense::click_and_drag());
        let painter = ui.painter_at(area);
        let media_rect = match &st.media {
            Some(m) => {
                let r = compute_rect(area, m.width as f32, m.height as f32, &View::default(), 0.0);
                if let Some(t) = app.textures.id("overlay-media") { painter.image(t, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE); }
                r
            }
            None => {
                painter.text(area.center(), egui::Align2::CENTER_CENTER, if cap.mode == OverlayMode::Camera { "Starting the camera…" } else { "No camera available. Choose a photo of your paper (top right), or use the Locked view." }, egui::FontId::proportional(18.0), Color32::WHITE);
                area.shrink(40.0)
            }
        };
        let to_screen = |p: [f64; 2]| pos2(media_rect.min.x + p[0] as f32 * media_rect.width(), media_rect.min.y + p[1] as f32 * media_rect.height());
        let from_screen = |p: Pos2| [((p.x - media_rect.min.x) / media_rect.width()) as f64, ((p.y - media_rect.min.y) / media_rect.height()) as f64];
        // Interaction
        if !st.locked {
            if resp.drag_started() {
                st.drag_handle = None;
                if let (Some(c), Some(pos)) = (st.corners, resp.interact_pointer_pos()) {
                    if st.calibrating { st.drag_handle = c.iter().position(|q| to_screen(*q).distance(pos) < 26.0); }
                }
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                match (st.drag_handle, st.corners.as_mut()) {
                    (Some(i), Some(c)) => { let p = to_screen(c[i]) + d; c[i] = from_screen(p); }
                    (None, Some(c)) => { for q in c.iter_mut() { let p = to_screen(*q) + d; *q = from_screen(p); } }
                    _ => { st.offset[0] += d.x as f64; st.offset[1] += d.y as f64; }
                }
            }
            if resp.drag_stopped() { if let Some(c) = st.corners { st.corners = Some(order_corners(&c)); } st.drag_handle = None; }
            if resp.hovered() && st.corners.is_none() {
                let s = ctx.input(|i| i.smooth_scroll_delta.y) as f64;
                if s.abs() > 0.0 { st.scale = (st.scale * (s * 0.0015).exp()).clamp(0.2, 3.0); }
            }
        }
        // Quad for the drawing area
        let guide_tex = app.textures.id("overlay-guide");
        let quad: [[f64; 2]; 4] = match st.corners {
            Some(c) => {
                let sc = c.map(|q| { let p = to_screen(q); [p.x as f64, p.y as f64] });
                match homography(&[[0.0, 0.0], [layout.paper_w, 0.0], [layout.paper_w, layout.paper_h], [0.0, layout.paper_h]], &sc) {
                    Some(hm) => [[layout.draw_x, layout.draw_y], [layout.draw_x + layout.draw_w, layout.draw_y], [layout.draw_x + layout.draw_w, layout.draw_y + layout.draw_h], [layout.draw_x, layout.draw_y + layout.draw_h]].map(|[x, y]| apply_h(&hm, x, y)),
                    None => sc,
                }
            }
            None => {
                let aspect = layout.draw_w / layout.draw_h;
                let (mw, mh) = (media_rect.width() as f64 * 0.8, media_rect.height() as f64 * 0.8);
                let (w, h) = if mw / mh > aspect { (mh * aspect, mh) } else { (mw, mw / aspect) };
                let c = media_rect.center();
                transformed_rect(c.x as f64 + st.offset[0], c.y as f64 + st.offset[1], w, h, st.scale, st.rotation)
            }
        };
        if let Some(t) = guide_tex { paint_quad(&painter, t, quad, Color32::WHITE.gamma_multiply(st.opacity)); }
        if let (true, Some(c)) = (st.calibrating && !st.locked, st.corners) {
            let pts: Vec<Pos2> = c.iter().map(|q| to_screen(*q)).collect();
            painter.add(egui::Shape::closed_line(pts.clone(), Stroke::new(2.0_f32, Color32::from_rgb(0x7c, 0xc4, 0xb3))));
            for (i, p) in pts.iter().enumerate() {
                painter.circle(*p, 16.0, crate::ui::ACCENT.gamma_multiply(0.7), Stroke::new(3.0_f32, Color32::WHITE));
                painter.text(*p, egui::Align2::CENTER_CENTER, ["TL", "TR", "BR", "BL"][i], egui::FontId::proportional(12.0), Color32::WHITE);
            }
            painter.text(area.center_top() + vec2(0.0, 16.0), egui::Align2::CENTER_TOP, "Drag the round handles onto the corners of your paper", egui::FontId::proportional(16.0), Color32::WHITE);
        }
    });

    sess.project.ar.opacity = st.opacity as f64;
    sess.project.ar.scale = st.scale;
    sess.project.ar.rotation = st.rotation;
    sess.project.ar.alignment_locked = st.locked;
    sess.project.ar.corners = st.corners.map(|c| c.to_vec());
    if exit {
        app.screen_state.overlay = Default::default(); // stops the camera
        app.textures.remove("overlay-media");
        app.save_project();
        app.go(Screen::Workspace);
    }
}

fn app_toast(t: &mut Vec<crate::app::Toast>) -> impl FnMut(&str) + '_ {
    move |s: &str| t.push(crate::app::Toast { text: s.into(), error: false, until: Instant::now() + Duration::from_secs(5) })
}

/// Run paper detection on the current media; `explicit` = user pressed the button.
fn detect(st: &mut OverlayState, explicit: bool, mut toast: impl FnMut(&str)) {
    st.last_detect = Some(Instant::now());
    let Some(m) = &st.media else { if explicit { toast("There is no camera picture or photo yet."); } return };
    let (w, h) = fit_dims(m.width, m.height, 200);
    let g = to_gray(&resize_box(m, w, h));
    let mean = g.data.iter().map(|&v| v as f64).sum::<f64>() / g.data.len() as f64;
    match detect_paper(&g) {
        Some(d) if d.confidence >= if explicit { 0.35 } else { 0.5 } => {
            st.corners = Some(match st.corners {
                Some(prev) if !explicit => { let mut c = prev; for i in 0..4 { for k in 0..2 { c[i][k] = prev[i][k] * 0.6 + d.corners[i][k] * 0.4; } } c }
                _ => d.corners,
            });
            if explicit { st.calibrating = true; toast("Paper found. Check the corners and adjust if needed."); }
            if st.follow { status(st, "Following paper", Color32::from_rgb(0x2e, 0x7d, 0x32)); }
        }
        _ => {
            if explicit { toast(if mean < 50.0 { "It's quite dark. More light will help." } else { "Couldn't find the paper. Make sure all four corners are visible on a darker surface, or calibrate the corners by hand." }); }
            if st.follow { status(st, "Searching for paper… (holding last position)", Color32::from_rgb(0xef, 0x6c, 0x00)); }
        }
    }
}
