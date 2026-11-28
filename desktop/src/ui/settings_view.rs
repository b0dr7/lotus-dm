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

    ui.label(RichText::new("Traffic mode").color(p.text));
    ui.label(
        RichText::new(
            "Low / Medium / High set a shared speed cap. Custom keeps the value above. Unlimited removes it.",
        )
        .color(p.muted),
    );
    ui.add_space(6.0);
    let mut mode = cfg.traffic_mode;
    ui.horizontal_wrapped(|ui| {
        for candidate in [
            settings::TrafficMode::Unlimited,
            settings::TrafficMode::High,
            settings::TrafficMode::Medium,
            settings::TrafficMode::Low,
            settings::TrafficMode::Custom,
        ] {
            if ui
                .selectable_label(mode == candidate, candidate.label())
                .clicked()
            {
                mode = candidate;
            }
        }
    });
    if mode != cfg.traffic_mode {
        cfg.traffic_mode = mode;
        if mode != settings::TrafficMode::Custom {
            cfg.global_speed_limit_kbps = mode.suggested_kbps();
            engine::set_global_limit_kbps(cfg.global_speed_limit_kbps);
        }
        persist(cfg);
    }
    ui.add_space(8.0);
    ui.label(RichText::new("Max connections per server").color(p.text));
    ui.label(RichText::new("Segments used for one file (1-32). Default 8.").color(p.muted));
    let r = ui.add(
        egui::DragValue::new(&mut cfg.max_connections_per_server)
            .range(1..=32)
            .speed(1),
    );
    if r.changed() {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Torrents").color(p.text));
    ui.label(RichText::new("Protocol encryption").color(p.text));
    ui.label(
        RichText::new(
            "MSE. Enabled prefers encrypted peers and still allows plaintext; Forced only \
             connects to peers that complete the handshake. rqbit sets this per session, so \
             it applies to the next torrent once no download is running.",
        )
        .color(p.muted),
    );
    ui.add_space(6.0);
    let mut encryption = cfg.torrent_encryption;
    ui.horizontal_wrapped(|ui| {
        for candidate in [
            engine::torrent::EncryptionMode::Disabled,
            engine::torrent::EncryptionMode::Enabled,
            engine::torrent::EncryptionMode::Forced,
        ] {
            if ui
                .selectable_label(encryption == candidate, candidate.label())
                .clicked()
            {
                encryption = candidate;
            }
        }
    });
    if encryption != cfg.torrent_encryption {
        cfg.torrent_encryption = encryption;
        // Rebuilds an idle torrent session in the background; a running
        // download keeps its mode until it finishes.
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(10.0);

    ui.label(
        RichText::new(
            "DHT and local-network discovery are set the same way: an idle engine is \
             rebuilt at once, and a running download picks them up when nothing is \
             downloading any more.",
        )
        .color(p.muted),
    );
    ui.add_space(4.0);
    let mut discovery_changed = ui
        .checkbox(
            &mut cfg.torrent_dht,
            "Use the DHT (needed for magnet links without trackers)",
        )
        .changed();
    discovery_changed |= ui
        .checkbox(&mut cfg.torrent_lsd, "Find peers on the local network")
        .changed();
    if discovery_changed {
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "Stop seeding at this share ratio and/or after this many minutes. 0 and 0 stop \
             as soon as the download is complete.",
        )
        .color(p.muted),
    );
    let mut seed_changed = ui
        .add(
            egui::DragValue::new(&mut cfg.torrent_seed_ratio)
                .range(0.0..=100.0)
                .speed(0.1)
                .suffix(" ratio"),
        )
        .changed();
    seed_changed |= ui
        .add(
            egui::DragValue::new(&mut cfg.torrent_seed_minutes)
                .range(0..=10_080u64)
                .speed(5)
                .suffix(" min"),
        )
        .changed();
    if seed_changed {
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Auto-shutdown").color(p.text));
    ui.label(RichText::new("Close the app automatically once the queue finishes.").color(p.muted));
    if ui.checkbox(&mut cfg.auto_shutdown, "Shut down when the queue is done").changed() {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Launch at startup").color(p.text));
    ui.label(RichText::new("Add Lotus to the per-user Windows Run key.").color(p.muted));
    if ui
        .checkbox(&mut cfg.launch_at_startup, "Start with Windows")
        .changed()
    {
        crate::core::sys::set_autostart(cfg.launch_at_startup);
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new("Portable mode").color(p.text));
    ui.label(
        RichText::new(
            "Portable mode is enabled by placing an empty file named 'portable' next to the \
             executable. Data is then kept in a 'data' folder beside it instead of %APPDATA%.",
        )
        .color(p.muted),
    );
    ui.add_space(12.0);

    ui.label(RichText::new("Tray and notifications").color(p.text));
    ui.label(
        RichText::new(
            "Downloads keep running with the window hidden. The tray menu has Open, Pause all, \
             Resume all, speed presets and Quit.",
        )
        .color(p.muted),
    );
    ui.add_space(4.0);
    let mut tray_changed = ui
        .checkbox(&mut cfg.close_to_tray, "Close button hides to tray")
        .changed();
    tray_changed |= ui
        .checkbox(&mut cfg.start_hidden, "Start hidden in the tray")
        .changed();
    tray_changed |= ui
        .checkbox(&mut cfg.notifications, "Notify when a download finishes or fails")
        .changed();
    if tray_changed {
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
