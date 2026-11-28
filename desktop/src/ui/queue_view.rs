use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, RichText, Sense, Stroke, StrokeKind, Vec2,
};

use crate::core::queue::{DownloadJob, JobStatus};
use crate::engine::{Control, Event};
use crate::ui::format;
use crate::ui::row_view::row;
use crate::ui::theme::Palette;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Done,
    Failed,
}

impl Filter {
    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Done => "Done",
            Filter::Failed => "Failed",
        }
    }

    fn matches(self, status: JobStatus) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => matches!(
                status,
                JobStatus::Queued | JobStatus::Running | JobStatus::Paused
            ),
            Filter::Done => status == JobStatus::Done,
            Filter::Failed => status == JobStatus::Failed,
        }
    }
}

const PILL_H: f32 = 28.0;

fn count(queue: &[DownloadJob], filter: Filter) -> usize {
    queue.iter().filter(|j| filter.matches(j.status)).count()
}

fn total_speed(queue: &[DownloadJob]) -> u64 {
    queue
        .iter()
        .filter(|job| job.status == JobStatus::Running)
        .map(|job| job.speed_bps)
        .sum()
}

fn pill(ui: &mut egui::Ui, text: &str, count: usize, selected: bool, p: &Palette) -> bool {
    let label = format!("{text}  {count}");
    let galley = ui
        .painter()
        .layout_no_wrap(label.clone(), FontId::proportional(12.5), p.text);
    let width = galley.size().x + 24.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, PILL_H), Sense::click());

    let fill = if selected {
        p.accent_tint
    } else if response.hovered() {
        p.hover_tint
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(14), fill);
    if !selected {
        ui.painter().rect(
            rect,
            CornerRadius::same(14),
            Color32::TRANSPARENT,
            Stroke::new(1.0_f32, p.border),
            StrokeKind::Inside,
        );
    }
    let color = if selected { p.accent } else { p.muted };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.5),
        color,
    );
    response.clicked()
}

pub fn filters_row(
    ui: &mut egui::Ui,
    queue: &[DownloadJob],
    filter: &mut Filter,
    p: &Palette,
) -> bool {
    let mut clear = false;
    ui.horizontal(|ui| {
        for f in [Filter::All, Filter::Active, Filter::Done, Filter::Failed] {
            if pill(ui, f.label(), count(queue, f), *filter == f, p) {
                *filter = f;
            }
            ui.add_space(4.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let any_done = queue.iter().any(|j| j.status == JobStatus::Done);
            if ui
                .add_enabled(
                    any_done,
                    egui::Button::new(RichText::new("Clear completed").color(p.muted)),
                )
                .clicked()
            {
                clear = true;
            }
        });
    });
    clear
}

pub fn summary_line(ui: &mut egui::Ui, queue: &[DownloadJob], p: &Palette) {
    ui.horizontal(|ui| {
        let shown = queue.len();
        let label = if shown == 1 {
            "1 download".to_string()
        } else {
            format!("{shown} downloads")
        };
        ui.label(RichText::new(label).size(12.5).color(p.muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let active = queue
                .iter()
                .filter(|job| job.status == JobStatus::Running)
                .count();
            let speed = total_speed(queue);
            let text = if active == 0 {
                "idle".to_string()
            } else {
                format!("{} active  ·  {}", active, format::speed(speed))
            };
            ui.label(RichText::new(text).size(12.5).color(p.muted));
        });
    });
}

pub fn status_bar(ctx: &egui::Context, queue: &[DownloadJob], limit_kbps: u64, p: &Palette) {
    egui::TopBottomPanel::bottom("status")
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(p.bg)
                .stroke(Stroke::new(1.0_f32, p.border)),
        )
        .show(ctx, |ui| {
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                let active = queue
                    .iter()
                    .filter(|job| job.status == JobStatus::Running)
                    .count();
                ui.label(
                    RichText::new(format!("{}  ·  {} active", format::speed(total_speed(queue)), active))
                        .size(12.0)
                        .color(p.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let limit = if limit_kbps == 0 {
                        "No speed limit".to_string()
                    } else {
                        format!("Limit {} KB/s", limit_kbps)
                    };
                    ui.label(RichText::new(limit).size(12.0).color(p.muted));
                });
            });
            ui.add_space(5.0);
        });
}

pub fn stat_card(ui: &mut egui::Ui, value: String, label: &str, p: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 64.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(12), p.surface);
    ui.painter().text(
        rect.left_top() + Vec2::new(16.0, 14.0),
        Align2::LEFT_TOP,
        value,
        FontId::proportional(22.0),
        p.text,
    );
    ui.painter().text(
        rect.left_bottom() + Vec2::new(16.0, -14.0),
        Align2::LEFT_BOTTOM,
        label,
        FontId::proportional(11.5),
        p.muted,
    );
}

pub fn stats_row(ui: &mut egui::Ui, queue: &[DownloadJob], p: &Palette) {
    let active = queue
        .iter()
        .filter(|j| matches!(j.status, JobStatus::Running | JobStatus::Paused))
        .count();
    let done = count(queue, Filter::Done);
    let failed = count(queue, Filter::Failed);
    let gap = 10.0;
    let col = (ui.available_width() - gap * 2.0) / 3.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui(Vec2::new(col, 64.0), |ui| {
            stat_card(ui, active.to_string(), "Active", p)
        });
        ui.allocate_ui(Vec2::new(col, 64.0), |ui| {
            stat_card(ui, done.to_string(), "Completed", p)
        });
        ui.allocate_ui(Vec2::new(col, 64.0), |ui| {
            stat_card(ui, failed.to_string(), "Failed", p)
        });
    });
}

pub fn empty_state(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(56.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Nothing in the queue")
                .color(p.text)
                .size(16.0),
        );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Paste a link in the bar above and press Add - the download starts right away.",
            )
            .color(p.muted)
            .size(12.5),
        );
    });
}

pub fn list_panel(
    ui: &mut egui::Ui,
    queue: &mut Vec<DownloadJob>,
    filter: Filter,
    selected: &Option<String>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
    p: &Palette,
) -> Option<String> {
    let error_color = ui.style().visuals.error_fg_color;
    let mut clicked = None;
    let mut shown = 0usize;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for job in queue.iter() {
                if !filter.matches(job.status) {
                    continue;
                }
                shown += 1;
                let is_selected = selected.as_deref() == Some(job.id.as_str());
                if row(ui, job, p, error_color, is_selected, controls, tx) {
                    clicked = Some(job.id.clone());
                }
                ui.add_space(8.0);
            }
        });
    if shown == 0 {
        empty_state(ui, p);
    }
    clicked
}
