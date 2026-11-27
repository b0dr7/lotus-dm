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
use crate::ui::help_view;
use crate::ui::lifecycle;
use crate::ui::queue_view::{self, Filter};
use crate::ui::settings_view;
use crate::ui::sidebar;
use crate::ui::theme::{self, Palette, ThemeMode};
use crate::ui::tray::{self, Tray, TrayAction};

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
    light_theme: bool,
    tray: Option<Tray>,
    quitting: bool,
    hidden: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Home,
    Queue,
    Settings,
    Help,
}

impl LotusApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install_system_fonts(&cc.egui_ctx);
        let settings = settings::load();
        theme::apply(settings.theme_mode, &cc.egui_ctx);
        let light_theme = theme::is_light(settings.theme_mode);
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
        let tray = Tray::new(&cc.egui_ctx);
        let hidden = settings.start_hidden;
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
            light_theme,
            tray,
            quitting: false,
            hidden,
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

    /// Tray menu clicks: they arrive once per frame from `ui::tray`.
    fn handle_tray(&mut self, action: TrayAction, ctx: &egui::Context) {
        match action {
            TrayAction::Open => {
                self.hidden = false;
                sys::set_main_window_visible(true);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            TrayAction::PauseAll => {
                for job in self.queue.iter() {
                    if job.status == JobStatus::Running {
                        if let Some(control) = self.controls.get(&job.id) {
                            control.request_pause();
                        }
                    }
                }
            }
            TrayAction::ResumeAll => {
                for index in 0..self.queue.len() {
                    let id = self.queue[index].id.clone();
                    match self.queue[index].status {
                        JobStatus::Paused => {
                            if let Some(control) = self.controls.get(&id) {
                                control.request_resume();
                            } else {
                                let pending = self.queue[index].clone();
                                lifecycle::spawn_job(&pending, &mut self.controls, &self.event_tx);
                            }
                        }
                        JobStatus::Queued => {
                            let pending = self.queue[index].clone();
                            lifecycle::spawn_job(&pending, &mut self.controls, &self.event_tx);
                        }
                        _ => {}
                    }
                }
            }
            TrayAction::Speed(kbps) => {
                self.settings.global_speed_limit_kbps = kbps;
                self.settings.traffic_mode = settings::TrafficMode::Custom;
                engine::set_global_limit_kbps(kbps);
                if let Err(err) = settings::save(&self.settings) {
                    eprintln!("lotusdm: settings save failed: {err}");
                }
            }
            TrayAction::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// One notification per terminal state, and none for a failure that is
    /// about to be retried automatically.
    fn notify_finished(&self, id: &str, finished_ok: bool, message: &str) {
        if !self.settings.notifications {
            return;
        }
        let name = self
            .queue
            .iter()
            .find(|job| job.id == id)
            .map(|job| job.filename.clone())
            .unwrap_or_default();
        let name = if name.is_empty() {
            "Download".to_string()
        } else {
            name
        };
        if finished_ok {
            tray::notify("Download finished", &format!("{name} is ready."), false);
        } else if !self.retry.will_retry(id) {
            tray::notify("Download failed", &format!("{name} - {message}"), true);
        }
    }

    fn apply_state(&mut self, id: &str, state: EngineState) {
        let finished = match &state {
            EngineState::Done => Some(true),
            EngineState::Failed(_) => Some(false),
            _ => None,
        };
        let failure = match &state {
            EngineState::Failed(message) => message.clone(),
            _ => String::new(),
        };
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
        if let Some(finished_ok) = finished {
            self.notify_finished(id, finished_ok, &failure);
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
                if sidebar::nav_pill(ui, "Help", self.nav == Nav::Help, p) {
                    self.nav = Nav::Help;
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
        tray::set_speed_hint(self.settings.global_speed_limit_kbps);
        for action in tray::poll() {
            self.handle_tray(action, ctx);
        }

        self.drain_bridge();
        self.drain_events();

        // Close to tray: the window disappears, downloads keep going.
        if ctx.input(|input| input.viewport().close_requested())
            && self.settings.close_to_tray
            && !self.quitting
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            sys::set_main_window_visible(false);
            self.hidden = true;
        }
        if self.hidden {
            // A hidden window is still ticked so progress and finish
            // notifications keep flowing.
            ctx.request_repaint_after(std::time::Duration::from_millis(500));
        }

        // Auto-shutdown: once every job is finished (done or failed) and the
        // setting is on, close the window.
        if self.settings.auto_shutdown && !self.queue.is_empty() {
            let all_done = self
                .queue
                .iter()
                .all(|job| matches!(job.status, JobStatus::Done | JobStatus::Failed));
            if all_done {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
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
            self.light_theme = theme::is_light(self.settings.theme_mode);
            if let Err(err) = settings::save(&self.settings) {
                eprintln!("lotusdm: settings save failed: {err}");
            }
        }

        // System mode follows Windows while the window stays open.
        if theme::is_light(self.settings.theme_mode) != self.light_theme {
            self.light_theme = theme::is_light(self.settings.theme_mode);
            theme::apply(self.settings.theme_mode, ctx);
        }
        if self.settings.theme_mode == ThemeMode::System {
            ctx.request_repaint_after(std::time::Duration::from_secs(15));
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
            Nav::Help => {
                help_view::help_panel(ui, &p);
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
