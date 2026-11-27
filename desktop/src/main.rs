#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Lotus Download Manager")
            .with_inner_size([1000.0, 640.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "lotus-dm",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LotusApp::new(cc)) as Box<dyn eframe::App>)),
    )
}
