use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use eframe::egui::{self, Align, CentralPanel, Layout, SidePanel, Stroke, TopBottomPanel, Vec2};

use crate::core::bridge;
use crate::core::queue::{self, DownloadJob, JobStatus};
use crate::core::settings::{self, Settings};
use crate::core::sys;
use crate::engine::{self, Control, EngineState, Event};
use crate::ui::add_dialog::{self, AddDialog};
use crate::ui::batch_dialog::{self, BatchDialog};
use crate::ui::clipboard_watch::{ClipboardWatch, PromptAction};
use crate::ui::lifecycle;
use crate::ui::queue_view::{self, Filter};
use crate::ui::settings_view;
use crate::ui::sidebar;
use crate::ui::theme::{self, Palette};

const NAV_WIDTH: f32 = 214.0;

pub struct LotusApp {
    settings: Settings,
    nav: Nav,
    filter: Filter,
    url_input: String,
    queue: Vec<DownloadJob>,
    add_dialog: Option<AddDialog>,
    batch_dialog: Option<BatchDialog>,
    controls: HashMap<String, Arc<Control>>,
    event_tx: Sender<Event>,
    event_rx: Receiver<Event>,
    retry: lifecycle::RetryTracker,
    watch: ClipboardWatch,
    auto_armed: bool,
    bridge_rx: Receiver<bridge::AddRequest>,
    bridge_port: Option<u16>,
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
        engine::set_global_limit_kbps(settings.global_speed_limit_kbps);

        let mut queue = queue::load();
        for job in queue.iter_mut() {
            if matches!(job.status, JobStatus::Running | JobStatus::Paused) {
                job.status = JobStatus::Queued;
                job.speed_bps = 0;
            }
        }

        let (event_tx, event_rx) = mpsc::channel();
        let (bridge_tx, bridge_rx) = mpsc::channel();
        let bridge_port = if settings.bridge_enabled {
            let ctx = cc.egui_ctx.clone();
            bridge::start(
                settings.bridge_port,
                bridge_tx,
                Arc::new(move || ctx.request_repaint()),
            )
        } else {
            None
        };
        Self {
            settings,
            nav: Nav::Home,
            filter: Filter::All,
            url_input: String::new(),
            queue,
            add_dialog: None,
            batch_dialog: None,
            controls: HashMap::new(),
            event_tx,
            event_rx,
            retry: lifecycle::RetryTracker::default(),
            watch: ClipboardWatch::default(),
            auto_armed: true,
            bridge_rx,
            bridge_port,
        }
    }

    /// Queue what is typed in the toolbar and start it right away.
    fn submit_url(&mut self) {
        let url = self.url_input.trim().to_string();
        if url.is_empty() {
            return;
        }
        let filename = queue::derive_filename(&url);
        let folder = self.settings.default_folder.clone();
        self.queue
            .push(DownloadJob::new_queued(url, filename, folder));
        if let Err(err) = queue::save(&self.queue) {
            eprintln!("lotusdm: queue save failed: {err}");
        }
        if let Some(job) = self.queue.last() {
            lifecycle::spawn_job(job, &mut self.controls, &self.event_tx);
        }
        self.url_input.clear();
    }

    /// Links handed over by the browser extension land here.
    fn drain_bridge(&mut self) {
        while let Ok(request) = self.bridge_rx.try_recv() {
            let filename = request
                .filename
                .unwrap_or_else(|| queue::derive_filename(&request.url));
            let folder = self.settings.default_folder.clone();
            self.queue
                .push(DownloadJob::new_queued(request.url, filename, folder));
            if let Err(err) = queue::save(&self.queue) {
                eprintln!("lotusdm: queue save failed: {err}");
            }
            if let Some(job) = self.queue.last() {
                lifecycle::spawn_job(job, &mut self.controls, &self.event_tx);
            }
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
        match &state {
            EngineState::Failed(_) => self.retry.on_failure(id),
            EngineState::Done => self.retry.forget(id),
            _ => {}
        }
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

    fn maybe_auto_start(&mut self) {
        if !self.settings.auto_start_enabled {
            self.auto_armed = true;
            return;
        }
        let (hour, minute) = sys::local_hm();
        if hour == self.settings.auto_start_hour && minute == self.settings.auto_start_minute {
            if self.auto_armed {
                self.auto_armed = false;
                lifecycle::start_all_queued(&mut self.queue, &mut self.controls, &self.event_tx);
            }
        } else {
            self.auto_armed = true;
        }
    }

    fn sidebar(&mut self, ctx: &egui::Context, p: &Palette) {
        SidePanel::left("nav")
            .resizable(false)
            .default_width(NAV_WIDTH)
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                ui.add_space(12.0);
                let (_, logo_rect) = ui.allocate_space(Vec2::new(ui.available_width(), 76.0));
                sidebar::draw_logo(ui, logo_rect, p);
                ui.add_space(4.0);
                sidebar::wordmark(ui, p);
                ui.add_space(20.0);

                if sidebar::nav_pill(ui, "Home", self.nav == Nav::Home, p) {
                    self.nav = Nav::Home;
                }
                if sidebar::nav_pill(ui, "Queue", self.nav == Nav::Queue, p) {
                    self.nav = Nav::Queue;
                }
                if sidebar::nav_pill(ui, "Settings", self.nav == Nav::Settings, p) {
                    self.nav = Nav::Settings;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    ui.add_space(10.0);
                    sidebar::version_row(ui, p);
                    sidebar::status_row(ui, p);
                });
            });
    }

    fn toolbar(&mut self, ctx: &egui::Context, p: &Palette) {
        TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .fill(p.bg)
                    .stroke(Stroke::new(1.0_f32, p.border)),
            )
            .show(ctx, |ui| {
                ui.add_space(9.0);
                ui.horizontal(|ui| {
                    let field_w = (ui.available_width() - 204.0).max(180.0);
                    let edit = ui.add_sized(
                        [field_w, 30.0],
                        egui::TextEdit::singleline(&mut self.url_input)
                            .hint_text("Paste a download link"),
                    );
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let can_add = !self.url_input.trim().is_empty();
                    let add = egui::Button::new(egui::RichText::new("Add").color(p.bg))
                        .fill(p.accent)
                        .min_size(Vec2::new(62.0, 30.0));
                    if ui.add_enabled(can_add, add).clicked() || (enter && can_add) {
                        self.submit_url();
                    }
                    let more = egui::Button::new("Paste list").min_size(Vec2::new(96.0, 30.0));
                    if ui.add(more).clicked() {
                        self.batch_dialog = Some(BatchDialog::default());
                    }
                });
                ui.add_space(9.0);
            });
    }
}

impl eframe::App for LotusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_bridge();
        self.drain_events();
        lifecycle::pump_retries(
            &mut self.queue,
            &mut self.controls,
            &self.event_tx,
            &mut self.retry,
        );
        self.maybe_auto_start();
        self.watch.poll(ctx, self.settings.clipboard_watch);

        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::T)) {
            self.settings.theme_mode = self.settings.theme_mode.next();
            theme::apply(self.settings.theme_mode, ctx);
            if let Err(err) = settings::save(&self.settings) {
                eprintln!("lotusdm: settings save failed: {err}");
            }
        }

        let p = theme::palette(self.settings.theme_mode);
        self.sidebar(ctx, &p);
        self.toolbar(ctx, &p);

        CentralPanel::default().show(ctx, |ui| match self.nav {
            Nav::Home => {
                ui.add_space(16.0);
                ui.heading("Home");
                ui.label(
                    egui::RichText::new("Everything you are downloading, in one list.")
                        .color(p.muted)
                        .size(12.5),
                );
                ui.add_space(16.0);
                queue_view::stats_row(ui, &self.queue, &p);
                ui.add_space(18.0);
                let filter = self.filter;
                queue_view::list_panel(
                    ui,
                    &mut self.queue,
                    filter,
                    &mut self.controls,
                    &self.event_tx,
                    &p,
                );
            }
            Nav::Queue => {
                ui.add_space(16.0);
                ui.heading("Queue");
                ui.add_space(12.0);
                if queue_view::filters_row(ui, &self.queue, &mut self.filter, &p) {
                    self.queue.retain(|job| job.status != JobStatus::Done);
                    if let Err(err) = queue::save(&self.queue) {
                        eprintln!("lotusdm: queue save failed: {err}");
                    }
                }
                ui.add_space(12.0);
                let filter = self.filter;
                queue_view::list_panel(
                    ui,
                    &mut self.queue,
                    filter,
                    &mut self.controls,
                    &self.event_tx,
                    &p,
                );
            }
            Nav::Settings => {
                settings_view::settings_panel(ui, &mut self.settings, self.bridge_port, &p)
            }
        });

        add_dialog::add_dialog_window(
            ctx,
            &mut self.queue,
            &mut self.add_dialog,
            &self.settings.default_folder,
            &p,
        );
        batch_dialog::batch_dialog_window(
            ctx,
            &mut self.queue,
            &mut self.batch_dialog,
            &self.settings.default_folder,
            &p,
        );
        if let Some(action) = self.watch.show(ctx, &p) {
            match action {
                PromptAction::Add => {
                    if let Some(url) = self.watch.pending.take() {
                        let filename = queue::derive_filename(&url);
                        let folder = self.settings.default_folder.clone();
                        self.queue
                            .push(DownloadJob::new_queued(url, filename, folder));
                        if let Err(err) = queue::save(&self.queue) {
                            eprintln!("lotusdm: queue save failed: {err}");
                        }
                    }
                }
                PromptAction::Dismiss => {
                    self.watch.pending = None;
                }
            }
        }
    }
}
