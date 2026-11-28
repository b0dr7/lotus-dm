use eframe::egui::{self, Align2, RichText, Vec2};

use crate::core::queue::{derive_filename, save, DownloadJob};
use crate::ui::theme::Palette;

#[derive(Default)]
pub struct BatchDialog {
    pub text: String,
}

pub fn batch_dialog_window(
    ctx: &egui::Context,
    queue: &mut Vec<DownloadJob>,
    batch_dialog: &mut Option<BatchDialog>,
    default_folder: &str,
    p: &Palette,
) {
    let Some(mut dialog) = batch_dialog.take() else {
        return;
    };
    let mut open = true;
    let mut cancel = false;
    let mut submit = false;

    egui::Window::new("Paste list")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("One URL per line.");
            ui.add_space(6.0);
            ui.add_sized(
                [420.0, 180.0],
                egui::TextEdit::multiline(&mut dialog.text)
                    .hint_text("https://example.com/a.zip\nhttps://example.com/b.zip"),
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let has_lines = dialog.text.lines().any(|line| !line.trim().is_empty());
                let add = egui::Button::new(RichText::new("Add all").color(p.bg)).fill(p.accent);
                if ui.add_enabled(has_lines, add).clicked() {
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
        for line in dialog.text.lines() {
            let url = line.trim();
            if url.is_empty() {
                continue;
            }
            queue.push(DownloadJob::new_queued(
                url.to_string(),
                derive_filename(url),
                default_folder.to_string(),
            ));
        }
        if let Err(err) = save(queue) {
            eprintln!("lotusdm: queue save failed: {err}");
        }
    } else if open {
        *batch_dialog = Some(dialog);
    }
}
