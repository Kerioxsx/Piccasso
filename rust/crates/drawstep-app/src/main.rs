//! DrawStep desktop (Rust + egui).
//!
//! The Rust app is a native implementation: decoding, image processing,
//! analysis, step generation, storage, keep-awake and the camera overlay all
//! run in Rust (`drawstep-core`); egui only draws the interface.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod camera;
mod export;
mod screens;
mod settings;
mod ui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DrawStep")
            .with_inner_size([1200.0, 820.0])
            .with_min_inner_size([420.0, 520.0]),
        ..Default::default()
    };
    eframe::run_native("DrawStep", options, Box::new(|cc| Ok(Box::new(app::DrawStepApp::new(cc)))))
}
