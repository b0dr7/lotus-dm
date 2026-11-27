use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Arc;

use eframe::egui::{self, RichText};

use crate::core::queue::{save, DownloadJob, JobStatus};
use crate::engine::{Control, Event};
use crate::ui::add_dialog::AddDialog;
use crate::ui::batch_dialog::BatchDialog;
use crate::ui::row_view::row;
use crate::ui::theme::Palette;

pub fn queue_panel(
    ui: &mut egui::Ui,
    queue: &mut Vec<DownloadJob>,
    add_dialog: &mut Option<AddDialog>,
    batch_dialog: &mut Option<BatchDialog>,
    controls: &mut HashMap<String, Arc<Control>>,
    tx: &Sender<Event>,
    p: &Palette,
) {
    ui.add_space(8.0);
    ui.heading("Queue");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let add = egui::Button::new(RichText::new("Add URL").color(p.bg)).fill(p.accent);
        if ui.add(add).clicked() {
            *add_dialog = Some(AddDialog::default());
        }
        if ui.button("Paste list").clicked() {
            *batch_dialog = Some(BatchDialog::default());
        }
        let has_done = queue.iter().any(|job| job.status == JobStatus::Done);
        let clear = egui::Button::new("Clear completed");
        if ui.add_enabled(has_done, clear).clicked() {
            queue.retain(|job| job.status != JobStatus::Done);
            if let Err(err) = save(queue) {
                eprintln!("lotusdm: queue save failed: {err}");
            }
        }
    });
    ui.add_space(8.0);

    if queue.is_empty() {
        ui.label(RichText::new("Your download queue is empty.").color(p.muted));
        return;
    }

    let error_color = ui.style().visuals.error_fg_color;
    for job in queue.iter() {
        row(ui, job, p, error_color, controls, tx);
    }
}
