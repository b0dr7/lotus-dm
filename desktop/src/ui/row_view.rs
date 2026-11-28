use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::core::queue::{host_of, DownloadJob, JobStatus};
use crate::engine::{Control, Event};
use crate::ui::format;
use crate::ui::lifecycle;
use crate::ui::theme::Palette;

const ROW_HEIGHT: f32 = 78.0;
const PAD: f32 = 14.0;
const BUTTON_H: f32 = 24.0;
const BUTTON_GAP: f32 = 8.0;

/// One download. Returns true when the card itself was clicked, which is how
/// the details panel gets its selection.
pub fn row(
    ui: &mut egui::Ui,
    job: &DownloadJob,
    p: &Palette,
    error_color: Color32,
    selected: bool,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
) -> bool {
    let width = ui.available_width().max(0.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW_HEIGHT), Sense::click());

    let painter = ui.painter();
    let fill = if selected { p.surface2 } else { p.surface };
    painter.rect_filled(rect, CornerRadius::same(12), fill);
    let border = if selected {
        Stroke::new(1.0_f32, p.accent)
    } else if response.hovered() {
        Stroke::new(1.0_f32, p.hover)
    } else {
        Stroke::new(1.0_f32, p.border)
    };
    painter.rect(
        rect,
        CornerRadius::same(12),
        Color32::TRANSPARENT,
        border,
        StrokeKind::Inside,
    );

    let left = rect.left() + PAD;
    let right = rect.right() - PAD;

    let (chip_fg, chip_bg) = chip_colors(job.status, p, error_color);
    let chip_galley = painter.layout_no_wrap(
        status_label(job).to_string(),
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
        Pos2::new(left, rect.top() + PAD + 7.0),
        Align2::LEFT_CENTER,
        &job.filename,
        FontId::proportional(14.0),
        p.text,
    );

    let meta_y = rect.top() + PAD + 26.0;
    let row_clip = Rect::from_min_max(Pos2::new(left, rect.top()), Pos2::new(right, rect.bottom()));
    if job.status == JobStatus::Failed {
        if let Some(error) = &job.error {
            painter.with_clip_rect(row_clip).text(
                Pos2::new(left, meta_y),
                Align2::LEFT_CENTER,
                error,
                FontId::proportional(12.0),
                error_color,
            );
        }
    } else {
        painter.with_clip_rect(row_clip).text(
            Pos2::new(left, meta_y),
            Align2::LEFT_CENTER,
            if (job.quality.is_some() {
                format!("{}  ·  {}", host_of(&job.url), job.quality.as_deref().unwrap_or(""))
            } else {
                host_of(&job.url)
            },
            FontId::proportional(12.0),
            p.muted,
        );
        let right_text = format::progress_line(
            job.received_bytes,
            job.total_bytes,
            job.speed_bps,
            job.status == JobStatus::Running,
        );
        painter.text(
            Pos2::new(right, meta_y),
            Align2::RIGHT_CENTER,
            right_text,
            FontId::proportional(12.0),
            p.muted,
        );
    }

    let band_y = rect.bottom() - PAD - BUTTON_H;
    let buttons_w = draw_buttons(ui, job, band_y, right, p, controls, tx);
    let percent_w = draw_percent(ui, job, band_y, right - buttons_w - 10.0, p);
    draw_progress_bar(
        ui,
        job,
        band_y,
        left,
        right - buttons_w - percent_w,
        p,
    );

    response.clicked()
}

/// Right-aligned percentage in front of the buttons; returns the width used.
fn draw_percent(
    ui: &mut egui::Ui,
    job: &DownloadJob,
    band_y: f32,
    right: f32,
    p: &Palette,
) -> f32 {
    let text = format::percent_text(job.received_bytes, job.total_bytes);
    let galley = ui
        .painter()
        .layout_no_wrap(text, FontId::proportional(11.5), p.muted);
    let width = galley.size().x;
    ui.painter().galley(
        Pos2::new(right - width, band_y + BUTTON_H / 2.0 - galley.size().y / 2.0),
        galley,
        p.muted,
    );
    width
}

fn draw_buttons(
    ui: &mut egui::Ui,
    job: &DownloadJob,
    band_y: f32,
    right: f32,
    p: &Palette,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
) -> f32 {
    let actions = actions_for(job.status);
    if actions.is_empty() {
        return 0.0;
    }
    let font = FontId::proportional(12.0);
    let mut widths = Vec::with_capacity(actions.len());
    let mut total = 0.0f32;
    for (label, _) in &actions {
        let w = ui
            .painter()
            .layout_no_wrap(label.to_string(), font.clone(), p.text)
            .size()
            .x
            + 18.0;
        widths.push(w);
        total += w;
    }
    total += BUTTON_GAP * (actions.len() - 1) as f32;

    let mut x = right - total;
    for ((label, primary), width) in actions.iter().zip(widths.iter()) {
        let rect = Rect::from_min_size(Pos2::new(x, band_y), Vec2::new(*width, BUTTON_H));
        let response = ui.allocate_rect(rect, Sense::click());

        let (fill, text_color) = if *primary {
            if response.hovered() {
                (p.hover, p.bg)
            } else {
                (p.accent, p.bg)
            }
        } else if response.hovered() {
            (p.hover_tint, p.hover)
        } else {
            (p.surface, p.text)
        };
        ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
        if !*primary {
            ui.painter().rect(
                rect,
                CornerRadius::same(12),
                Color32::TRANSPARENT,
                Stroke::new(1.0_f32, p.border),
                StrokeKind::Inside,
            );
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            *label,
            font.clone(),
            text_color,
        );
        if response.clicked() {
            act(job, label, controls, tx);
        }
        x += width + BUTTON_GAP;
    }
    total
}

fn draw_progress_bar(
    ui: &mut egui::Ui,
    job: &DownloadJob,
    band_y: f32,
    left: f32,
    right: f32,
    p: &Palette,
) {
    let bar_right = right.max(left + 40.0);
    let bar = Rect::from_min_max(
        Pos2::new(left, band_y + 10.0),
        Pos2::new(bar_right, band_y + 16.0),
    );

    ui.painter()
        .rect_filled(bar, CornerRadius::same(4), p.surface2);
    ui.painter().rect(
        bar,
        CornerRadius::same(4),
        Color32::TRANSPARENT,
        Stroke::new(1.0_f32, p.border),
        StrokeKind::Inside,
    );

    if job.total_bytes == 0 || job.received_bytes == 0 {
        return;
    }
    let fraction = (job.received_bytes as f64 / job.total_bytes as f64).clamp(0.0, 1.0) as f32;
    let fill_width = bar.width() * fraction;
    if fill_width < 1.0 {
        return;
    }
    let fill = Rect::from_min_max(bar.min, Pos2::new(bar.min.x + fill_width, bar.max.y));
    ui.painter()
        .rect_filled(fill, CornerRadius::same(4), p.accent);
}

fn actions_for(status: JobStatus) -> Vec<(&'static str, bool)> {
    match status {
        JobStatus::Queued => vec![("Start", true)],
        JobStatus::Running => vec![("Pause", false), ("Cancel", false)],
        JobStatus::Paused => vec![("Resume", true), ("Cancel", false)],
        JobStatus::Done => vec![("Open folder", false)],
        JobStatus::Failed => vec![("Retry", true)],
    }
}

fn act(
    job: &DownloadJob,
    label: &str,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
) {
    match label {
        "Start" | "Retry" => {
            lifecycle::spawn_job(job, controls, tx);
        }
        "Pause" => {
            if let Some(control) = controls.get(&job.id) {
                control.request_pause();
            }
        }
        "Resume" => {
            if let Some(control) = controls.get(&job.id) {
                control.request_resume();
            }
        }
        "Cancel" => {
            if let Some(control) = controls.get(&job.id) {
                control.request_cancel();
            }
        }
        "Open folder" => crate::ui::details::open_folder(&job.folder),
        _ => {}
    }
}

fn status_label(job: &DownloadJob) -> &'static str {
    if job.status == JobStatus::Queued {
        if let Some(start) = job.start_at {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if start > now_ms {
                return "Scheduled";
            }
        }
    }
    job.status.label()
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
