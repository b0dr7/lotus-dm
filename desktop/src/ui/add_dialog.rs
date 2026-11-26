use eframe::egui::{self, Align2, RichText, Vec2};

use crate::core::queue::{derive_filename, save, DownloadJob};
use crate::ui::theme::Palette;

#[derive(Default)]
pub struct AddDialog {
    pub url: String,
    pub filename: String,
    pub folder: String,
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
        queue.push(DownloadJob::new_queued(url, filename, folder));
        if let Err(err) = save(queue) {
            eprintln!("lotusdm: queue save failed: {err}");
        }
    } else if open {
        *add_dialog = Some(dialog);
    }
}
