use eframe::egui::{self, Align2, RichText, Vec2};

use crate::core::queue::{derive_filename, save, DownloadJob};
use crate::ui::theme::Palette;

#[derive(Default)]
pub struct AddDialog {
    pub url: String,
    pub filename: String,
    pub folder: String,
    /// Media pick for video/audio jobs: a height like `1080p` or a format like `mp3`.
    /// None keeps the engine default (720p video / mp3 audio).
    pub quality: Option<String>,
    /// Delay the start by this many minutes (None/empty = start immediately).
    pub start_in: String,
    pub delay: bool,
}

pub fn add_dialog_window(
    ctx: &egui::Context,
    queue: &mut Vec<DownloadJob>,
    add_dialog: &mut Option<AddDialog>,
    default_folder: &str,
    p: &Palette,
) {
    let Some(mut dialog) = add_dialog.take() else {
        return;
    };
    let mut open = true;
    let mut cancel = false;
    let mut submit = false;

    egui::Window::new("Add URL")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("URL *");
            ui.add_sized(
                [ui.available_width(), 26.0],
                egui::TextEdit::singleline(&mut dialog.url)
                    .hint_text("https://example.com/file.zip"),
            );
            ui.add_space(8.0);
            ui.label("Filename (optional)");
            ui.add_sized(
                [ui.available_width(), 26.0],
                egui::TextEdit::singleline(&mut dialog.filename)
                    .hint_text("derived from URL if empty"),
            );
            ui.add_space(8.0);
            ui.label("Folder (optional)");
            ui.add_sized(
                [ui.available_width(), 26.0],
                egui::TextEdit::singleline(&mut dialog.folder).hint_text(default_folder),
            );
            if crate::engine::media::is_media(&dialog.url) {
                ui.add_space(8.0);
                ui.label("Quality");
                let video_pick = |pick: &str| {
                    pick.ends_with('p')
                        && pick[..pick.len() - 1].chars().all(|c| c.is_ascii_digit())
                        && !pick.is_empty()
                };
                let candidates: Vec<String> = if dialog
                    .quality
                    .as_deref()
                    .is_some_and(|pick| !video_pick(pick))
                {
                    crate::engine::media::audio_qualities()
                        .iter()
                        .map(|q| q.label.clone())
                        .collect()
                } else {
                    crate::engine::media::video_qualities()
                        .iter()
                        .map(|q| q.label.clone())
                        .collect()
                };
                for candidate in candidates {
                    let selected = dialog.quality.as_deref() == Some(candidate.as_str());
                    if ui.selectable_button(candidate, selected).clicked() {
                        dialog.quality = Some(candidate);
                    }
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut dialog.delay, "Delay start");
                if dialog.delay {
                    ui.label("by (minutes)");
                    ui.add_sized([72.0, 26.0], egui::TextEdit::singleline(&mut dialog.start_in));
                }
            });
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let can_submit = !dialog.url.trim().is_empty();
                let add = egui::Button::new(RichText::new("Add").color(p.bg)).fill(p.accent);
                if ui.add_enabled(can_submit, add).clicked() {
                    submit = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
            ui.add_space(4.0);
        });

    if cancel {
        open = false;
    }

    if submit {
        let url = dialog.url.trim().to_string();
        let filename = if dialog.filename.trim().is_empty() {
            derive_filename(&url)
        } else {
            dialog.filename.trim().to_string()
        };
        let folder = if dialog.folder.trim().is_empty() {
            default_folder.to_string()
        } else {
            dialog.folder.trim().to_string()
        };
        let quality = if crate::engine::media::is_media(&url) {
            dialog.quality.clone().or_else(|| {
                crate::engine::media::resolve(
                    crate::engine::media::MediaKind::Video,
                    "720p",
                )
                .map(|q| q.label)
            })
        } else {
            None
        };
        let start_at = if dialog.delay {
            dialog
                .start_in
                .trim()
                .parse::<u32>()
                .ok()
                .map(|minutes| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    now + (minutes as u64) * 60_000
                })
        } else {
            None
        };
        let mut job = DownloadJob::new_queued(url, filename, folder);
        job.quality = quality;
        job.start_at = start_at;
        queue.push(job);
        if let Err(err) = save(queue) {
            eprintln!("lotusdm: queue save failed: {err}");
        }
    } else if open {
        *add_dialog = Some(dialog);
    }
}
