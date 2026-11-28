use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, RichText};

use crate::engine::spider::{self, SpiderMatch};
use crate::ui::theme::Palette;

pub struct SpiderState {
    pub url: String,
    pub pattern: String,
    pub depth: u32,
    pub max_pages: u32,
    pub matches: Vec<SpiderMatch>,
    pub picked: HashSet<usize>,
    pub status: String,
    pending: Option<Receiver<Result<Vec<SpiderMatch>, String>>>,
}

impl Default for SpiderState {
    fn default() -> Self {
        Self {
            url: String::new(),
            pattern: String::new(),
            depth: 1,
            max_pages: 25,
            matches: Vec::new(),
            picked: HashSet::new(),
            status: String::new(),
            pending: None,
        }
    }
}

impl SpiderState {
    fn running(&self) -> bool {
        self.pending.is_some()
    }

    fn start(&mut self) {
        if self.running() || self.url.trim().is_empty() {
            return;
        }
        let url = self.url.trim().to_string();
        let pattern = self.pattern.trim().to_string();
        let depth = self.depth as usize;
        let pages = self.max_pages as usize;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(spider::crawl(&url, &pattern, depth, pages));
        });
        self.pending = Some(rx);
        self.status = "Crawling...".to_string();
    }

    fn poll(&mut self) {
        let Some(rx) = self.pending.as_ref() else {
            return;
        };
        let outcome = rx.try_recv();
        match outcome {
            Ok(Ok(found)) => {
                self.status = match found.len() {
                    0 => "No matching files found.".to_string(),
                    1 => "1 matching file.".to_string(),
                    n => format!("{n} matching files."),
                };
                self.picked = (0..found.len()).collect();
                self.matches = found;
                self.pending = None;
            }
            Ok(Err(err)) => {
                self.status = err;
                self.pending = None;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.status = "Crawl stopped.".to_string();
                self.pending = None;
            }
        }
    }

    fn picked_urls(&self) -> Vec<String> {
        self.matches
            .iter()
            .enumerate()
            .filter(|(index, _)| self.picked.contains(index))
            .map(|(_, entry)| entry.url.clone())
            .collect()
    }
}

pub fn spider_panel(state: &mut SpiderState, ui: &mut egui::Ui, p: &Palette) -> Vec<String> {
    state.poll();
    ui.add_space(16.0);
    ui.heading("HTML Spider");
    ui.label(
        RichText::new("Crawl a site and keep the files that match.")
            .color(p.muted)
            .size(12.5),
    );
    ui.add_space(16.0);

    ui.horizontal(|ui| {
        let width = (ui.available_width() - 210.0).max(160.0);
        ui.add_sized(
            [width, 28.0],
            egui::TextEdit::singleline(&mut state.url).hint_text("https://example.com"),
        );
        ui.add_sized(
            [120.0, 28.0],
            egui::TextEdit::singleline(&mut state.pattern).hint_text(".zip"),
        );
        let label = if state.running() { "Crawling" } else { "Crawl" };
        let button = egui::Button::new(RichText::new(label).color(p.bg)).fill(p.accent);
        if ui
            .add_enabled(!state.running() && !state.url.trim().is_empty(), button)
            .clicked()
        {
            state.start();
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Pages deep").color(p.muted).size(12.0));
        ui.add(egui::DragValue::new(&mut state.depth).range(0..=3));
        ui.add_space(10.0);
        ui.label(RichText::new("Max pages").color(p.muted).size(12.0));
        ui.add(egui::DragValue::new(&mut state.max_pages).range(1..=200));
        ui.add_space(10.0);
        ui.label(
            RichText::new("Leave the pattern empty to keep every file link.")
                .color(p.muted)
                .size(11.5),
        );
    });

    if state.running() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
    }

    ui.add_space(10.0);
    if !state.status.is_empty() {
        ui.label(RichText::new(state.status.as_str()).color(p.muted).size(12.0));
        ui.add_space(8.0);
    }

    if state.matches.is_empty() {
        return Vec::new();
    }

    let mut to_add = Vec::new();
    ui.horizontal(|ui| {
        if ui.button("Select all").clicked() {
            state.picked = (0..state.matches.len()).collect();
        }
        if ui.button("Select none").clicked() {
            state.picked.clear();
        }
        let count = state.picked.len();
        let add = egui::Button::new(RichText::new(format!("Add {count} to queue")).color(p.bg))
            .fill(p.accent);
        if ui.add_enabled(count > 0, add).clicked() {
            to_add = state.picked_urls();
        }
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        let mut changes: Vec<(usize, bool)> = Vec::new();
        for (index, entry) in state.matches.iter().enumerate() {
            let mut checked = state.picked.contains(&index);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut checked, "").changed() {
                    changes.push((index, checked));
                }
                ui.label(RichText::new(entry.url.as_str()).size(12.0).color(p.text))
                    .on_hover_text(entry.page.as_str());
            });
        }
        for (index, checked) in changes {
            if checked {
                state.picked.insert(index);
            } else {
                state.picked.remove(&index);
            }
        }
    });

    to_add
}
