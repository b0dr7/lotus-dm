use eframe::egui::{self, RichText};

use crate::core::settings::{self, Settings};
use crate::engine;
use crate::ui::theme::Palette;

pub fn settings_panel(ui: &mut egui::Ui, cfg: &mut Settings, p: &Palette) {
    ui.add_space(8.0);
    ui.heading("Settings");
    ui.add_space(16.0);

    ui.label(RichText::new("Global speed limit").color(p.text));
    ui.label(
        RichText::new(
            "Shared by every running download. 0 = unlimited. Per-download limits override it.",
        )
        .color(p.muted),
    );
    ui.add_space(6.0);
    let response = ui.add(egui::DragValue::new(&mut cfg.global_speed_limit_kbps).suffix(" KB/s"));
    if response.changed() {
        engine::set_global_limit_kbps(cfg.global_speed_limit_kbps);
        if let Err(err) = settings::save(cfg) {
            eprintln!("lotusdm: settings save failed: {err}");
        }
    }
    ui.add_space(8.0);
}
