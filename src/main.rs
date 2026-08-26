#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ui;
mod utils;
mod video;

use app::VideoCropTrimApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Ensure PATH environment variable includes Homebrew / MacPorts / user bins on macOS/Linux
    utils::process::ensure_path_env();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1150.0, 780.0])
            .with_min_inner_size([760.0, 520.0])
            .with_title("VideoCropTrim - Video Spatial Crop & Temporal Trim")
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "VideoCropTrim",
        native_options,
        Box::new(|cc| Ok(Box::new(VideoCropTrimApp::new(cc)))),
    )
}

