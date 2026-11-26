use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind,
    Vec2,
};

use crate::core::queue::{derive_filename, host_of, save, DownloadJob, JobStatus};
use crate::ui::theme::Palette;

const ROW_HEIGHT: f32 = 76.0;
const PAD: f32 = 12.0;

#[derive(Default)]
pub struct AddDialog {
    pub url: String,
    pub filename: String,
    pub folder: String,
}

pub fn queue_panel(
    ui: &mut egui::Ui,
    queue: &mut Vec<DownloadJob>,
    add_dialog: &mut Option<AddDialog>,
    p: &Palette,
) {
    ui.add_space(8.0);
    ui.heading("Queue");
    ui.add_space(8.0);

    let add = egui::Button::new(RichText::new("Add URL").color(p.bg)).fill(p.accent);
    if ui.add(add).clicked() {
        *add_dialog = Some(AddDialog::default());
    }
    ui.add_space(8.0);

    if queue.is_empty() {
        ui.label(RichText::new("Your download queue is empty.").color(p.muted));
        return;
    }

    let error_color = ui.style().visuals.error_fg_color;
    for job in queue.iter() {
        row(ui, job, p, error_color);
    }
}

fn row(ui: &mut egui::Ui, job: &DownloadJob, p: &Palette, error_color: Color32) {
    let width = ui.available_width().max(0.0);
    let (rect, _response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::hover());

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), p.surface);
    painter.rect(
        rect,
        CornerRadius::same(12),
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, p.border),
        StrokeKind::Inside,
    );

    let left = rect.left() + PAD;
    let right = rect.right() - PAD;

    let (chip_fg, chip_bg) = chip_colors(job.status, p, error_color);
    let chip_galley = painter.layout_no_wrap(
        job.status.label().to_string(),
        FontId::proportional(12.0),
        chip_fg,
    );
    let chip_size = chip_galley.size() + Vec2::new(14.0, 6.0);
    let chip_rect =
        Rect::from_min_size(Pos2::new(right - chip_size.x, rect.top() + PAD), chip_size);
    painter.rect_filled(chip_rect, CornerRadius::same(12), chip_bg);
    painter.galley(chip_rect.min + Vec2::new(7.0, 3.0), chip_galley, chip_fg);

    let name_clip = Rect::from_min_max(
        Pos2::new(left, rect.top()),
        Pos2::new(chip_rect.left() - PAD, rect.bottom()),
    );
    painter.with_clip_rect(name_clip).text(
        Pos2::new(left, rect.top() + PAD + 8.0),
        Align2::LEFT_CENTER,
        &job.filename,
        FontId::proportional(14.0),
        p.text,
    );

    let meta_y = rect.top() + PAD + 28.0;
    painter.text(
        Pos2::new(left, meta_y),
        Align2::LEFT_CENTER,
        host_of(&job.url),
        FontId::proportional(12.0),
        p.muted,
    );
    painter.text(
        Pos2::new(right, meta_y),
        Align2::RIGHT_CENTER,
        "--",
        FontId::proportional(12.0),
        p.muted,
    );

    let bar = Rect::from_min_max(
        Pos2::new(left, rect.bottom() - PAD - 8.0),
        Pos2::new(right, rect.bottom() - PAD),
    );
    painter.rect_filled(bar, CornerRadius::same(4), p.surface2);
    painter.rect(
        bar,
        CornerRadius::same(4),
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, p.border),
        StrokeKind::Inside,
    );
}

fn chip_colors(status: JobStatus, p: &Palette, error_color: Color32) -> (Color32, Color32) {
    match status {
        JobStatus::Queued => (p.muted, p.surface2),
        JobStatus::Running => (p.accent, p.accent_tint),
        JobStatus::Paused => (p.hover, p.hover_tint),
        JobStatus::Done => (p.text, p.surface2),
        JobStatus::Failed => (
            error_color,
            Color32::from_rgba_unmultiplied(error_color.r(), error_color.g(), error_color.b(), 45),
        ),
    }
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
