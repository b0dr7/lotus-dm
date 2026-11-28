
use eframe::egui::{self, RichText, Stroke};

use crate::core::queue::{host_of, DownloadJob, JobStatus};
use crate::ui::format;
use crate::ui::theme::Palette;

pub enum Action {
    Start,
    Pause,
    Resume,
    Cancel,
    OpenFolder,
    OpenFile,
}

pub struct Outcome {
    pub action: Option<Action>,
    pub changed: bool,
    pub close: bool,
}

pub fn open_folder(folder: &str) {
    #[cfg(windows)]
    let mut command = std::process::Command::new("explorer");
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(folder);
    let _ = command.spawn();
}

pub fn open_file(path: &str) {
    #[cfg(windows)]
    let mut command = std::process::Command::new("cmd");
    #[cfg(windows)]
    {
        command.args(["/C", "start", "", path]);
    }
    #[cfg(not(windows))]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(not(windows))]
    {
        command.arg(path);
    }
    let _ = command.spawn();
}

pub fn show(ctx: &egui::Context, job: &mut DownloadJob, p: &Palette) -> Outcome {
    let mut outcome = Outcome {
        action: None,
        changed: false,
        close: false,
    };

    egui::SidePanel::right("job-details")
        .resizable(false)
        .exact_width(296.0)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(p.bg)
                .stroke(Stroke::new(1.0_f32, p.border)),
        )
        .show(ctx, |ui| {
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Download").size(15.0).strong().color(p.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        outcome.close = true;
                    }
                });
            });
            ui.add_space(6.0);

            ui.label(RichText::new(job.filename.as_str()).size(14.0).color(p.text));
            ui.add_space(2.0);
            ui.label(
                RichText::new(format!(
                    "{}  ·  {}",
                    job.status.label(),
                    format::percent_text(job.received_bytes, job.total_bytes)
                ))
                .size(12.0)
                .color(p.muted),
            );
            ui.add_space(10.0);

            let fraction = if job.total_bytes == 0 {
                0.0
            } else {
                (job.received_bytes as f64 / job.total_bytes as f64).clamp(0.0, 1.0) as f32
            };
            ui.add(
                egui::ProgressBar::new(fraction)
                    .fill(p.accent)
                    .desired_width(ui.available_width()),
            );
            ui.add_space(12.0);

            egui::Grid::new("job-details-grid")
                .num_columns(2)
                .spacing([12.0, 7.0])
                .show(ui, |ui| {
                    key(ui, "Source", p);
                    ui.label(RichText::new(host_of(&job.url)).color(p.text));
                    ui.end_row();

                    key(ui, "Size", p);
                    let size = if job.total_bytes > 0 {
                        format!(
                            "{} of {}",
                            format::size(job.received_bytes),
                            format::size(job.total_bytes)
                        )
                    } else if job.received_bytes > 0 {
                        format::size(job.received_bytes)
                    } else {
                        "unknown".to_string()
                    };
                    ui.label(RichText::new(size).color(p.text));
                    ui.end_row();

                    key(ui, "Speed", p);
                    let speed = if job.speed_bps > 0 {
                        format!(
                            "{}  ·  {} left",
                            format::speed(job.speed_bps),
                            format::eta(job.received_bytes, job.total_bytes, job.speed_bps)
                        )
                    } else {
                        "--".to_string()
                    };
                    ui.label(RichText::new(speed).color(p.text));
                    ui.end_row();

                    key(ui, "Folder", p);
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(RichText::new(job.folder.as_str()).color(p.text))
                                .wrap(),
                        );
                        if ui.button("Open").clicked() {
                            outcome.action = Some(Action::OpenFolder);
                        }
                    });
                    ui.end_row();
                });

            ui.add_space(10.0);
            if job.status == JobStatus::Failed {
                if let Some(error) = &job.error {
                    ui.label(RichText::new(error.as_str()).size(12.0).color(p.muted));
                    ui.add_space(8.0);
                }
            }

            ui.label(RichText::new("Speed limit for this download").color(p.text));
            ui.add_space(2.0);
            let mut limit = job.speed_limit_kbps;
            let response = ui.add(
                egui::DragValue::new(&mut limit)
                    .range(0..=102_400u64)
                    .speed(16)
                    .suffix(" KB/s"),
            );
            if response.changed() {
                job.speed_limit_kbps = limit;
                outcome.changed = true;
            }
            let note = if job.speed_limit_kbps == 0 {
                "0 means the global limit only."
            } else if matches!(job.status, JobStatus::Running | JobStatus::Paused) {
                "Takes effect the next time this download starts."
            } else {
                "Applied when this download starts."
            };
            ui.label(RichText::new(note).size(11.5).color(p.muted));

            ui.add_space(8.0);
            ui.label(RichText::new("Filename").color(p.text));
            ui.add_space(2.0);
            let mut name = job.filename.clone();
            let name_response = ui.add(
                egui::TextEdit::singleline(&mut name)
                    .desired_width(ui.available_width()),
            );
            if name_response.changed() && !name.trim().is_empty() {
                job.filename = name.trim().to_string();
                outcome.changed = true;
            }

            if let Some(quality) = &job.quality {
                ui.add_space(8.0);
                ui.label(RichText::new(format!("Quality: {quality}")).color(p.text));
            }

            ui.add_space(14.0);
            ui.horizontal_wrapped(|ui| {
                let (label, action, primary) = match job.status {
                    JobStatus::Queued => ("Start", Action::Start, true),
                    JobStatus::Running => ("Pause", Action::Pause, false),
                    JobStatus::Paused => ("Resume", Action::Resume, true),
                    JobStatus::Failed => ("Retry", Action::Start, true),
                    JobStatus::Done => ("Open folder", Action::OpenFolder, false),
                };
                let button = if primary {
                    egui::Button::new(RichText::new(label).color(p.bg)).fill(p.accent)
                } else {
                    egui::Button::new(RichText::new(label).color(p.text))
                };
                if ui.add(button).clicked() {
                    outcome.action = Some(action);
                }
                if job.status == JobStatus::Done
                    && ui.button("Open file").clicked()
                {
                    outcome.action = Some(Action::OpenFile);
                }
                if matches!(
                    job.status,
                    JobStatus::Running | JobStatus::Paused | JobStatus::Queued
                ) && ui.button("Cancel").clicked()
                {
                    outcome.action = Some(Action::Cancel);
                }
                if job.status != JobStatus::Done && ui.button("Copy link").clicked() {
                    ui.ctx().copy_text(job.url.clone());
                }
            });

            ui.add_space(10.0);
            ui.label(RichText::new(job.url.as_str()).size(11.0).color(p.muted));
        });

    outcome
}

fn key(ui: &mut egui::Ui, text: &str, p: &Palette) {
    ui.label(RichText::new(text).size(12.0).color(p.muted));
}
