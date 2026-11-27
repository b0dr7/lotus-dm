use eframe::egui::{self, RichText};

use crate::core::settings::{self, Settings};
use crate::engine;
use crate::ui::theme::Palette;

fn persist(cfg: &Settings) {
    if let Err(err) = settings::save(cfg) {
        eprintln!("lotusdm: settings save failed: {err}");
    }
}

pub fn settings_panel(ui: &mut egui::Ui, cfg: &mut Settings, p: &Palette) {
    ui.add_space(8.0);
    ui.heading("Settings");
    ui.add_space(16.0);

    ui.label(RichText::new("Default download folder").color(p.text));
    ui.horizontal(|ui| {
        ui.add_sized(
            [ui.available_width() - 92.0, 26.0],
            egui::TextEdit::singleline(&mut cfg.default_folder),
        );
        if ui.button("Browse").clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                cfg.default_folder = path.to_string_lossy().into_owned();
                persist(cfg);
            }
        }
    });
    ui.add_space(12.0);

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
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Clipboard watcher").color(p.text));
    ui.label(
        RichText::new(
            "While enabled and the window is focused, watch for copied URLs every 2 seconds.",
        )
        .color(p.muted),
    );
    ui.add_space(4.0);
    if ui
        .checkbox(&mut cfg.clipboard_watch, "Watch clipboard for URLs")
        .changed()
    {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Auto-start queue").color(p.text));
    ui.label(
        RichText::new("When on, queued downloads start automatically at this time every day.")
            .color(p.muted),
    );
    ui.add_space(4.0);
    let mut schedule_changed = false;
    ui.horizontal(|ui| {
        schedule_changed |= ui
            .checkbox(&mut cfg.auto_start_enabled, "Auto-start queue at")
            .changed();
        schedule_changed |= ui
            .add(
                egui::DragValue::new(&mut cfg.auto_start_hour)
                    .range(0..=23)
                    .speed(1),
            )
            .changed();
        schedule_changed |= ui.label(":").changed();
        schedule_changed |= ui
            .add(
                egui::DragValue::new(&mut cfg.auto_start_minute)
                    .range(0..=59)
                    .speed(1),
            )
            .changed();
    });
    if schedule_changed {
        persist(cfg);
    }
    ui.add_space(8.0);
}
