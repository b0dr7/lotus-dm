#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod engine;
mod ui;

use eframe::egui;

fn main() -> eframe::Result {
    let icon = egui::IconData {
        rgba: include_bytes!("../assets/lotus-64.rgba").to_vec(),
        width: 64,
        height: 64,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Lotus Download Manager")
            .with_icon(std::sync::Arc::new(icon))
            .with_inner_size([1040.0, 680.0])
            .with_min_inner_size([760.0, 520.0]),
        ..Default::default()
    };

    eframe::run_native(
        "lotus-dm",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LotusApp::new(cc)) as Box<dyn eframe::App>)),
    )
}
