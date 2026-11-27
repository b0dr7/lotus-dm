use eframe::egui::{self, RichText};

use crate::core::settings::{self, Settings};
use crate::engine;
use crate::ui::theme::{self, Palette, ThemeMode};

fn persist(cfg: &Settings) {
    if let Err(err) = settings::save(cfg) {
        eprintln!("lotusdm: settings save failed: {err}");
    }
}

pub fn settings_panel(ui: &mut egui::Ui, cfg: &mut Settings, bridge_port: Option<u16>, p: &Palette) {
    ui.add_space(8.0);
    ui.heading("Settings");
    ui.add_space(16.0);

    ui.label(RichText::new("Theme").color(p.text));
    ui.label(
        RichText::new("System follows the Windows light and dark setting.").color(p.muted),
    );
    ui.add_space(6.0);
    let mut mode = cfg.theme_mode;
    ui.horizontal_wrapped(|ui| {
        for candidate in [
            ThemeMode::System,
            ThemeMode::Dark,
            ThemeMode::Light,
            ThemeMode::Amoled,
            ThemeMode::HighContrast,
        ] {
            if ui
                .selectable_label(mode == candidate, candidate.label())
                .clicked()
            {
                mode = candidate;
            }
        }
    });
    if mode != cfg.theme_mode {
        cfg.theme_mode = mode;
        theme::apply(mode, ui.ctx());
        persist(cfg);
    }
    ui.add_space(14.0);

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
    ui.add_space(12.0);

    ui.label(RichText::new("Browser bridge").color(p.text));
    ui.label(
        RichText::new(
            "Lets the Lotus extension hand links to the queue. Listens on 127.0.0.1 only - \
             nothing is reachable from other machines.",
        )
        .color(p.muted),
    );
    ui.add_space(4.0);
    let mut bridge_changed = ui
        .checkbox(&mut cfg.bridge_enabled, "Accept links from the browser extension")
        .changed();
    ui.horizontal(|ui| {
        bridge_changed |= ui
            .add(
                egui::DragValue::new(&mut cfg.bridge_port)
                    .range(1024..=65535)
                    .speed(1),
            )
            .changed();
        ui.label(RichText::new("port - takes effect after restart").color(p.muted));
    });
    let state = match (cfg.bridge_enabled, bridge_port) {
        (true, Some(port)) => format!("Listening on http://127.0.0.1:{port}/ping"),
        (true, None) => "Not listening - the port is busy. Change it and restart.".to_string(),
        (false, _) => "Off - the browser extension cannot reach this app.".to_string(),
    };
    ui.label(RichText::new(state).color(p.muted));
    if bridge_changed {
        persist(cfg);
    }
    ui.add_space(6.0);
    ui.label(RichText::new("Extension setup").color(p.text));
    ui.label(
        RichText::new(
            "Firefox: install Lotus from addons.mozilla.org. Chromium: download the .zip from \
             the Lotus site, unzip it, open chrome://extensions, turn on Developer mode, click \
             Load unpacked and pick the unzipped folder. Then pin Lotus to the toolbar. The \
             bridge line above should read Listening.",
        )
        .color(p.muted),
    );
    ui.add_space(8.0);
}
