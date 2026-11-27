use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke,
    TextStyle, Vec2,
};
use serde::{Deserialize, Serialize};

const RADIUS: u8 = 12;
const SPACING: f32 = 14.0;
const SEMIBOLD: &str = "system-semibold";

static SYSTEM_FONTS: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    /// Follow the Windows light/dark setting. The default for new installs.
    #[default]
    System,
    Dark,
    Light,
    Amoled,
    HighContrast,
}

impl ThemeMode {
    pub fn next(self) -> ThemeMode {
        match self {
            ThemeMode::System => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Amoled,
            ThemeMode::Amoled => ThemeMode::HighContrast,
            ThemeMode::HighContrast => ThemeMode::System,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "System",
            ThemeMode::Dark => "Dark",
            ThemeMode::Light => "Light",
            ThemeMode::Amoled => "Amoled",
            ThemeMode::HighContrast => "High contrast",
        }
    }
}

/// True when the palette in use is the light one (directly or via System).
pub fn is_light(mode: ThemeMode) -> bool {
    concrete(mode) == ThemeMode::Light
}

/// Turns System into the light or dark mode Windows is currently using.
fn concrete(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => {
            if system_prefers_light() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            }
        }
        other => other,
    }
}

/// Cached for a few seconds so a per-frame palette() call never spawns a
/// process more than once in a while.
fn system_prefers_light() -> bool {
    static CACHE: LazyLock<Mutex<(Option<Instant>, bool)>> =
        LazyLock::new(|| Mutex::new((None, false)));
    let mut guard = CACHE.lock().unwrap_or_else(|err| err.into_inner());
    let fresh = guard
        .0
        .map(|at| at.elapsed() < Duration::from_secs(30))
        .unwrap_or(false);
    if fresh {
        return guard.1;
    }
    let value = detect_system_light();
    *guard = (Some(Instant::now()), value);
    value
}

#[cfg(windows)]
fn detect_system_light() -> bool {
    use std::process::Command;
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output();
    match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).contains("0x1"),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn detect_system_light() -> bool {
    false
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub surface2: Color32,
    pub accent: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub hover: Color32,
    pub border: Color32,
    pub accent_tint: Color32,
    pub hover_tint: Color32,
    pub selection_fill: Color32,
}

fn tint(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn palette(mode: ThemeMode) -> Palette {
    let (bg, surface, surface2, accent, text, muted, hover, border) = match concrete(mode) {
        ThemeMode::Light => (
            rgb(0xF7, 0xF4, 0xEC),
            rgb(0xFF, 0xFF, 0xFF),
            rgb(0xF7, 0xF4, 0xEC),
            rgb(0xA1, 0x62, 0x07),
            rgb(0x14, 0x14, 0x14),
            rgb(0x6B, 0x6B, 0x70),
            rgb(0xCA, 0x8A, 0x04),
            Color32::from_black_alpha(26),
        ),
        ThemeMode::Dark => (
            rgb(0x0D, 0x0D, 0x0F),
            rgb(0x16, 0x16, 0x1A),
            rgb(0x1E, 0x1E, 0x24),
            rgb(0xF2, 0xC1, 0x4E),
            rgb(0xEF, 0xEF, 0xEA),
            rgb(0x8A, 0x8A, 0x93),
            rgb(0xFF, 0xD3, 0x4D),
            Color32::from_white_alpha(26),
        ),
        ThemeMode::Amoled => (
            rgb(0x00, 0x00, 0x00),
            rgb(0x0A, 0x0A, 0x0A),
            rgb(0x0A, 0x0A, 0x0A),
            rgb(0xFF, 0xD3, 0x4D),
            rgb(0xEF, 0xEF, 0xEA),
            rgb(0x7A, 0x7A, 0x82),
            rgb(0xFF, 0xD3, 0x4D),
            Color32::from_white_alpha(26),
        ),
        ThemeMode::HighContrast => (
            rgb(0x00, 0x00, 0x00),
            rgb(0x00, 0x00, 0x00),
            rgb(0x00, 0x00, 0x00),
            rgb(0xFF, 0xE1, 0x4D),
            rgb(0xFF, 0xFF, 0xFF),
            rgb(0xC9, 0xC9, 0xD1),
            rgb(0xFF, 0xE1, 0x4D),
            Color32::WHITE,
        ),
        // concrete() already replaced System with Light or Dark.
        ThemeMode::System => unreachable!(),
    };

    Palette {
        bg,
        surface,
        surface2,
        accent,
        text,
        muted,
        hover,
        border,
        accent_tint: tint(accent, 45),
        hover_tint: tint(hover, 30),
        selection_fill: tint(accent, 90),
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Loads the Windows system font (Segoe UI + Segoe UI Semibold) so text renders
/// as system-ui per the plan. Silently keeps egui's defaults when the files
/// are not present (e.g. running cargo check on a non-windows host).
pub fn install_system_fonts(ctx: &egui::Context) {
    if SYSTEM_FONTS.load(Ordering::Relaxed) {
        return;
    }
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_string());
    let (body, semibold) = match (
        std::fs::read(format!("{windir}\\Fonts\\segoeui.ttf")),
        std::fs::read(format!("{windir}\\Fonts\\seguisb.ttf")),
    ) {
        (Ok(body), Ok(semibold)) => (body, semibold),
        _ => return,
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert("system".into(), Arc::new(FontData::from_owned(body)));
    fonts
        .font_data
        .insert(SEMIBOLD.into(), Arc::new(FontData::from_owned(semibold)));
    if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
        family.insert(0, "system".into());
    }
    // apply() asks for this family by name for headings; without the binding
    // egui panics on the first text layout (windows-only: the fonts load).
    fonts.families.insert(
        FontFamily::Name(SEMIBOLD.into()),
        vec![SEMIBOLD.into(), "system".into()],
    );
    ctx.set_fonts(fonts);
    SYSTEM_FONTS.store(true, Ordering::Relaxed);
}

pub fn apply(mode: ThemeMode, ctx: &egui::Context) {
    let mode = concrete(mode);
    let p = palette(mode);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::splat(SPACING);

    if SYSTEM_FONTS.load(Ordering::Relaxed) {
        style.text_styles.insert(
            TextStyle::Heading,
            FontId::new(18.0, FontFamily::Name(SEMIBOLD.into())),
        );
    }

    let radius = CornerRadius::same(RADIUS);
    let border = Stroke::new(1.0_f32, p.border);

    let mut v = if mode == ThemeMode::Light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.dark_mode = mode != ThemeMode::Light;

    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.accent;
    v.faint_bg_color = p.surface;
    v.extreme_bg_color = p.surface2;
    v.code_bg_color = p.surface;

    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = border;
    v.widgets.noninteractive.corner_radius = radius;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);

    v.widgets.inactive.bg_fill = p.surface;
    v.widgets.inactive.weak_bg_fill = p.surface;
    v.widgets.inactive.bg_stroke = border;
    v.widgets.inactive.corner_radius = radius;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);

    v.widgets.hovered.bg_fill = p.hover_tint;
    v.widgets.hovered.weak_bg_fill = p.hover_tint;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.hover);
    v.widgets.hovered.corner_radius = radius;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.hover);

    v.widgets.active.bg_fill = tint(p.hover, 70);
    v.widgets.active.weak_bg_fill = tint(p.hover, 70);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.hover);
    v.widgets.active.corner_radius = radius;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.hover);

    v.widgets.open.bg_fill = p.hover_tint;
    v.widgets.open.weak_bg_fill = p.hover_tint;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.hover);
    v.widgets.open.corner_radius = radius;
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, p.hover);

    v.selection.bg_fill = p.selection_fill;
    v.selection.stroke = Stroke::new(1.0_f32, p.accent);

    v.window_corner_radius = radius;
    v.window_fill = p.bg;
    v.window_stroke = border;
    v.menu_corner_radius = radius;
    v.panel_fill = p.bg;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;

    style.visuals = v;
    ctx.set_style(style);
}
