use eframe::egui::{self, RichText};

use crate::core::settings::{self, Lang, Settings};
use crate::engine;
use crate::ui::i18n::{self, Key};
use crate::ui::theme::{self, Palette, ThemeMode};

fn persist(cfg: &Settings) {
    if let Err(err) = settings::save(cfg) {
        eprintln!("lotusdm: settings save failed: {err}");
    }
}

pub fn settings_panel(ui: &mut egui::Ui, cfg: &mut Settings, bridge_port: Option<u16>, p: &Palette) {
    ui.add_space(8.0);
    let lang = cfg.language;
    ui.heading(i18n::tr(lang, Key::SettingsTitle));
    ui.add_space(16.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Language)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::LanguageHint)).color(p.muted));
    ui.add_space(6.0);
    let mut language = cfg.language;
    ui.horizontal_wrapped(|ui| {
        for candidate in Lang::all() {
            if ui
                .selectable_label(language == candidate, candidate.label())
                .clicked()
            {
                language = candidate;
            }
        }
    });
    if language != cfg.language {
        cfg.language = language;
        persist(cfg);
    }
    ui.add_space(14.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Theme)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::ThemeHint)).color(p.muted));
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
                .selectable_label(mode == candidate, i18n::theme_label(lang, candidate))
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

    ui.label(RichText::new(i18n::tr(lang, Key::DefaultFolder)).color(p.text));
    ui.horizontal(|ui| {
        ui.add_sized(
            [ui.available_width() - 92.0, 26.0],
            egui::TextEdit::singleline(&mut cfg.default_folder),
        );
        if ui.button(i18n::tr(lang, Key::Browse)).clicked() {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                cfg.default_folder = path.to_string_lossy().into_owned();
                persist(cfg);
            }
        }
    });
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::GlobalSpeedLimit)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::GlobalSpeedHint)).color(p.muted));
    ui.add_space(6.0);
    let response = ui.add(
        egui::DragValue::new(&mut cfg.global_speed_limit_kbps)
            .suffix(i18n::tr(lang, Key::SpeedSuffix)),
    );
    if response.changed() {
        engine::set_global_limit_kbps(cfg.global_speed_limit_kbps);
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::TrafficMode)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::TrafficHint)).color(p.muted));
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
                .selectable_label(mode == candidate, i18n::traffic_label(lang, candidate))
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
    ui.label(RichText::new(i18n::tr(lang, Key::MaxConns)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::MaxConnsHint)).color(p.muted));
    let r = ui.add(
        egui::DragValue::new(&mut cfg.max_connections_per_server)
            .range(1..=32)
            .speed(1),
    );
    if r.changed() {
        persist(cfg);
    }
    ui.add_space(8.0);
    ui.label(RichText::new(i18n::tr(lang, Key::MaxConcurrent)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::MaxConcurrentHint)).color(p.muted));
    let r = ui.add(egui::DragValue::new(&mut cfg.max_concurrent_downloads).range(0..=32));
    if r.changed() {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Torrents)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::Encryption)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::EncryptionHint)).color(p.muted));
    ui.add_space(6.0);
    let mut encryption = cfg.torrent_encryption;
    ui.horizontal_wrapped(|ui| {
        for candidate in [
            engine::torrent::EncryptionMode::Disabled,
            engine::torrent::EncryptionMode::Enabled,
            engine::torrent::EncryptionMode::Forced,
        ] {
            if ui
                .selectable_label(encryption == candidate, i18n::encryption_label(lang, candidate))
                .clicked()
            {
                encryption = candidate;
            }
        }
    });
    if encryption != cfg.torrent_encryption {
        cfg.torrent_encryption = encryption;
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(10.0);

    ui.label(RichText::new(i18n::tr(lang, Key::DhtLsdHint)).color(p.muted));
    ui.add_space(4.0);
    let mut discovery_changed = ui
        .checkbox(&mut cfg.torrent_dht, i18n::tr(lang, Key::DhtCheck))
        .changed();
    discovery_changed |= ui
        .checkbox(&mut cfg.torrent_lsd, i18n::tr(lang, Key::LsdCheck))
        .changed();
    if discovery_changed {
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(8.0);
    ui.label(RichText::new(i18n::tr(lang, Key::SeedHint)).color(p.muted));
    let mut seed_changed = ui
        .add(
            egui::DragValue::new(&mut cfg.torrent_seed_ratio)
                .range(0.0..=100.0)
                .speed(0.1)
                .suffix(i18n::tr(lang, Key::RatioSuffix)),
        )
        .changed();
    seed_changed |= ui
        .add(
            egui::DragValue::new(&mut cfg.torrent_seed_minutes)
                .range(0..=10_080u64)
                .speed(5)
                .suffix(i18n::tr(lang, Key::MinSuffix)),
        )
        .changed();
    if seed_changed {
        engine::torrent::set_policy(cfg.torrent_policy());
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::AutoShutdown)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::AutoShutdownHint)).color(p.muted));
    if ui
        .checkbox(&mut cfg.auto_shutdown, i18n::tr(lang, Key::AutoShutdownCheck))
        .changed()
    {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::LaunchAtStartup)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::LaunchHint)).color(p.muted));
    if ui
        .checkbox(&mut cfg.launch_at_startup, i18n::tr(lang, Key::LaunchCheck))
        .changed()
    {
        crate::core::sys::set_autostart(cfg.launch_at_startup);
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Portable)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::PortableHint)).color(p.muted));
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Tray)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::TrayHint)).color(p.muted));
    ui.add_space(4.0);
    let mut tray_changed = ui
        .checkbox(&mut cfg.close_to_tray, i18n::tr(lang, Key::TrayClose))
        .changed();
    tray_changed |= ui
        .checkbox(&mut cfg.start_hidden, i18n::tr(lang, Key::TrayHidden))
        .changed();
    tray_changed |= ui
        .checkbox(&mut cfg.notifications, i18n::tr(lang, Key::NotifyCheck))
        .changed();
    if tray_changed {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::Clipboard)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::ClipboardHint)).color(p.muted));
    ui.add_space(4.0);
    if ui
        .checkbox(&mut cfg.clipboard_watch, i18n::tr(lang, Key::ClipboardCheck))
        .changed()
    {
        persist(cfg);
    }
    ui.add_space(12.0);

    ui.label(RichText::new(i18n::tr(lang, Key::AutoStart)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::AutoStartHint)).color(p.muted));
    ui.add_space(4.0);
    let mut schedule_changed = false;
    ui.horizontal(|ui| {
        schedule_changed |= ui
            .checkbox(&mut cfg.auto_start_enabled, i18n::tr(lang, Key::AutoStartCheck))
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

    ui.label(RichText::new(i18n::tr(lang, Key::Bridge)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::BridgeHint)).color(p.muted));
    ui.add_space(4.0);
    let mut bridge_changed = ui
        .checkbox(&mut cfg.bridge_enabled, i18n::tr(lang, Key::BridgeCheck))
        .changed();
    ui.horizontal(|ui| {
        bridge_changed |= ui
            .add(
                egui::DragValue::new(&mut cfg.bridge_port)
                    .range(1024..=65535)
                    .speed(1),
            )
            .changed();
        ui.label(RichText::new(i18n::tr(lang, Key::BridgePortHint)).color(p.muted));
    });
    let state = match (cfg.bridge_enabled, bridge_port) {
        (true, Some(port)) => format!(
            "{}http://127.0.0.1:{port}",
            i18n::tr(lang, Key::ListeningOn)
        ),
        (true, None) => i18n::tr(lang, Key::PortBusy).to_string(),
        (false, _) => i18n::tr(lang, Key::BridgeOff).to_string(),
    };
    ui.label(RichText::new(state).color(p.muted));
    if bridge_changed {
        persist(cfg);
    }
    ui.add_space(6.0);
    ui.label(RichText::new(i18n::tr(lang, Key::ExtensionSetup)).color(p.text));
    ui.label(RichText::new(i18n::tr(lang, Key::ExtensionHint)).color(p.muted));
    ui.add_space(8.0);
}
