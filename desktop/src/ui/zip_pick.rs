use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;

use eframe::egui::{self, Align2, RichText, Vec2};

use crate::engine::zip_partial::{self, ZipEntry};
use crate::engine::Control;
use crate::ui::format;
use crate::ui::theme::Palette;

pub struct ZipPick {
    pub url: String,
    entries: Vec<ZipEntry>,
    picked: HashSet<usize>,
    status: String,
    listing: Option<Receiver<Result<Vec<ZipEntry>, String>>>,
    pulling: Option<Receiver<String>>,
    control: Arc<Control>,
}

impl Default for ZipPick {
    fn default() -> Self {
        Self {
            url: String::new(),
            entries: Vec::new(),
            picked: HashSet::new(),
            status: String::new(),
            listing: None,
            pulling: None,
            control: Arc::new(Control::default()),
        }
    }
}

impl ZipPick {
    fn busy(&self) -> bool {
        self.listing.is_some() || self.pulling.is_some()
    }

    fn list(&mut self) {
        if self.busy() || self.url.trim().is_empty() {
            return;
        }
        let url = self.url.trim().to_string();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(zip_partial::list(&url));
        });
        self.listing = Some(rx);
        self.status = "Reading the archive index...".to_string();
        self.entries.clear();
        self.picked.clear();
    }

    fn pull(&mut self, folder: &str) {
        if self.busy() {
            return;
        }
        let url = self.url.trim().to_string();
        let wanted: Vec<ZipEntry> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(index, _)| self.picked.contains(index))
            .map(|(_, entry)| entry.clone())
            .collect();
        if url.is_empty() || wanted.is_empty() {
            return;
        }
        let folder = std::path::PathBuf::from(folder);
        let control = self.control.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut written = 0usize;
            let mut failures: Vec<String> = Vec::new();
            for entry in &wanted {
                match zip_partial::extract(&url, entry, &folder, &control) {
                    Ok(_) => written += 1,
                    Err(err) => failures.push(format!("{}: {err}", entry.name)),
                }
            }
            let mut line = format!("{written} of {} files pulled out.", wanted.len());
            if !failures.is_empty() {
                line.push_str(" Failed: ");
                line.push_str(&failures.join(" | "));
            }
            let _ = tx.send(line);
        });
        self.pulling = Some(rx);
        self.status = format!("Pulling out {} files...", wanted.len());
    }

    fn poll(&mut self) {
        if let Some(rx) = self.listing.as_ref() {
            match rx.try_recv() {
                Ok(Ok(entries)) => {
                    self.status = match entries.len() {
                        0 => "The archive is empty.".to_string(),
                        1 => "1 file in this archive.".to_string(),
                        n => format!("{n} files in this archive."),
                    };
                    self.picked = (0..entries.len()).collect();
                    self.entries = entries;
                    self.listing = None;
                }
                Ok(Err(err)) => {
                    self.status = err;
                    self.listing = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.status = "Reading the index stopped.".to_string();
                    self.listing = None;
                }
            }
        }
        if let Some(rx) = self.pulling.as_ref() {
            if let Ok(line) = rx.try_recv() {
                self.status = line;
                self.pulling = None;
            }
        }
    }
}

/// Window for pulling single files out of a remote zip.
pub fn zip_window(
    ctx: &egui::Context,
    state: &mut Option<ZipPick>,
    folder: &str,
    p: &Palette,
) {
    let Some(pick) = state.as_mut() else {
        return;
    };
    pick.poll();
    let mut open = true;
    let mut list_clicked = false;
    let mut pull_clicked = false;

    egui::Window::new("Files inside a zip")
        .collapsible(false)
        .resizable(true)
        .default_width(520.0)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("Archive URL");
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 76.0).max(160.0);
                ui.add_sized(
                    [width, 26.0],
                    egui::TextEdit::singleline(&mut pick.url)
                        .hint_text("https://example.com/archive.zip"),
                );
                if ui
                    .add_enabled(!pick.busy(), egui::Button::new("List"))
                    .clicked()
                {
                    list_clicked = true;
                }
            });
            if !pick.status.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(pick.status.as_str()).color(p.muted).size(12.0));
            }
            ui.add_space(8.0);
            if !pick.entries.is_empty() {
                ui.horizontal(|ui| {
                    if ui.button("Select all").clicked() {
                        pick.picked = (0..pick.entries.len()).collect();
                    }
                    if ui.button("Select none").clicked() {
                        pick.picked.clear();
                    }
                    let count = pick.picked.len();
                    let pull = egui::Button::new(
                        RichText::new(format!("Pull out {count}")).color(p.bg),
                    )
                    .fill(p.accent);
                    if ui.add_enabled(count > 0 && !pick.busy(), pull).clicked() {
                        pull_clicked = true;
                    }
                });
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    let mut changes: Vec<(usize, bool)> = Vec::new();
                    for (index, entry) in pick.entries.iter().enumerate() {
                        let mut checked = pick.picked.contains(&index);
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut checked, "").changed() {
                                changes.push((index, checked));
                            }
                            ui.label(RichText::new(entry.name.as_str()).size(12.0));
                            ui.label(
                                RichText::new(format::size(entry.uncompressed_size))
                                    .color(p.muted)
                                    .size(11.5),
                            );
                            if !entry.is_stored() {
                                ui.label(
                                    RichText::new("compressed")
                                        .color(p.muted)
                                        .size(11.0),
                                );
                            }
                        });
                    }
                    for (index, checked) in changes {
                        if checked {
                            pick.picked.insert(index);
                        } else {
                            pick.picked.remove(&index);
                        }
                    }
                });
            }
            ui.add_space(10.0);
            ui.label(
                RichText::new("Only uncompressed entries can be pulled out in this build.")
                    .color(p.muted)
                    .size(11.0),
            );
        });

    if !open {
        *state = None;
        return;
    }
    if list_clicked {
        pick.list();
    }
    if pull_clicked {
        let folder = folder.to_string();
        pick.pull(&folder);
    }
    if pick.busy() {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}
