use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{self, Align, CentralPanel, Layout, SidePanel, Stroke, Vec2};

use crate::core::queue::{self, DownloadJob, JobStatus};
use crate::core::settings::{self, Settings};
use crate::engine::{Control, EngineState, Event};
use crate::ui::add_dialog::{self, AddDialog};
use crate::ui::queue_view;
use crate::ui::sidebar;
use crate::ui::theme::{self, Palette};

const NAV_WIDTH: f32 = 210.0;

pub struct LotusApp {
    settings: Settings,
    nav: Nav,
    queue: Vec<DownloadJob>,
    add_dialog: Option<AddDialog>,
    controls: HashMap<String, Arc<Control>>,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Home,
    Queue,
    Settings,
}

impl LotusApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_system_fonts(&cc.egui_ctx);
        let settings = settings::load();
        theme::apply(settings.theme_mode, &cc.egui_ctx);

        let mut queue = queue::load();
        for job in queue.iter_mut() {
            if matches!(job.status, JobStatus::Running | JobStatus::Paused) {
                job.status = JobStatus::Queued;
                job.speed_bps = 0;
            }
        }

        let (event_tx, event_rx) = mpsc::channel();
        Self {
            settings,
            nav: Nav::Home,
            queue,
            add_dialog: None,
            controls: HashMap::new(),
            event_tx,
            event_rx,
        }
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                Event::Progress {
                    id,
                    received,
                    total,
                    speed_bps,
                } => {
                    if let Some(job) = self.queue.iter_mut().find(|job| job.id == id) {
                        job.received_bytes = received;
                        job.total_bytes = total;
                        job.speed_bps = speed_bps;
                    }
                }
                Event::NoResume { id } => {
                    let mut changed = false;
                    if let Some(job) = self.queue.iter_mut().find(|job| job.id == id) {
                        job.no_resume = true;
                        changed = true;
                    }
                    if changed {
                        if let Err(err) = queue::save(&self.queue) {
                            eprintln!("lotusdm: queue save failed: {err}");
                        }
                    }
                }
                Event::State { id, state } => self.apply_state(&id, state),
            }
        }
    }

    fn apply_state(&mut self, id: &str, state: EngineState) {
        let terminal = matches!(
            state,
            EngineState::Queued | EngineState::Done | EngineState::Failed(_)
        );
        let mut changed = false;
        if let Some(job) = self.queue.iter_mut().find(|job| job.id == id) {
            match state {
                EngineState::Running => {
                    job.status = JobStatus::Running;
                    job.error = None;
                    changed = true;
                }
                EngineState::Paused => {
                    job.status = JobStatus::Paused;
                    job.speed_bps = 0;
                    changed = true;
                }
                EngineState::Queued => {
                    job.status = JobStatus::Queued;
                    job.speed_bps = 0;
                    changed = true;
                }
                EngineState::Done => {
                    job.status = JobStatus::Done;
                    job.speed_bps = 0;
                    if job.total_bytes == 0 {
                        job.total_bytes = job.received_bytes;
                    }
                    changed = true;
                }
                EngineState::Failed(message) => {
                    job.status = JobStatus::Failed;
                    job.error = Some(message);
                    job.speed_bps = 0;
                    changed = true;
                }
            }
        }
        if terminal {
            self.controls.remove(id);
        }
        if changed {
            if let Err(err) = queue::save(&self.queue) {
                eprintln!("lotusdm: queue save failed: {err}");
            }
        }
    }
}

impl eframe::App for LotusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.settings.theme_mode = self.settings.theme_mode.next();
            theme::apply(self.settings.theme_mode, ctx);
            if let Err(err) = settings::save(&self.settings) {
                eprintln!("lotusdm: settings save failed: {err}");
            }
        }

        let p = theme::palette(self.settings.theme_mode);

        SidePanel::left("nav")
            .resizable(true)
            .default_width(NAV_WIDTH)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                ui.add_space(4.0);
                let (_, logo_rect) = ui.allocate_space(Vec2::new(ui.available_width(), 96.0));
                sidebar::draw_logo(ui, logo_rect, &p);
                ui.add_space(4.0);

                if sidebar::nav_pill(ui, "Home", self.nav == Nav::Home, &p) {
                    self.nav = Nav::Home;
                }
                if sidebar::nav_pill(ui, "Queue", self.nav == Nav::Queue, &p) {
                    self.nav = Nav::Queue;
                }
                if sidebar::nav_pill(ui, "Settings", self.nav == Nav::Settings, &p) {
                    self.nav = Nav::Settings;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    sidebar::status_row(ui, &p);
                });
            });

        let nav = self.nav;
        CentralPanel::default().show(ctx, |ui| match nav {
            Nav::Queue => queue_view::queue_panel(
                ui,
                &mut self.queue,
                &mut self.add_dialog,
                &mut self.controls,
                &self.event_tx,
                &p,
            ),
            Nav::Home => placeholder(
                ui,
                "Home",
                "Nothing here yet. New downloads will show up here.",
                &p,
            ),
            Nav::Settings => placeholder(ui, "Settings", "Settings arrive in a later phase.", &p),
        });

        add_dialog::add_dialog_window(
            ctx,
            &mut self.queue,
            &mut self.add_dialog,
            &self.settings.default_folder,
            &p,
        );
    }
}

fn placeholder(ui: &mut egui::Ui, title: &str, text: &str, p: &Palette) {
    ui.add_space(8.0);
    ui.heading(title);
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).color(p.muted));
}
