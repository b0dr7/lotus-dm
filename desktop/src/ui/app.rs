use eframe::egui::{
    self, Align, Align2, CentralPanel, Color32, FontId, Layout, Pos2, Sense, SidePanel, Stroke,
    Vec2,
};

use crate::core::settings::{self, Settings};
use crate::ui::theme::{self, Palette};

const NAV_WIDTH: f32 = 210.0;
const LOGO_SIDE: f32 = 64.0;
const PILL_HEIGHT: f32 = 34.0;

pub struct LotusApp {
    settings: Settings,
    nav: Nav,
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
        Self {
            settings,
            nav: Nav::Home,
        }
    }
}

impl eframe::App for LotusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                draw_logo(ui, logo_rect, &p);
                ui.add_space(4.0);

                if nav_pill(ui, "Home", self.nav == Nav::Home, &p) {
                    self.nav = Nav::Home;
                }
                if nav_pill(ui, "Queue", self.nav == Nav::Queue, &p) {
                    self.nav = Nav::Queue;
                }
                if nav_pill(ui, "Settings", self.nav == Nav::Settings, &p) {
                    self.nav = Nav::Settings;
                }

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    status_row(ui, &p);
                });
            });

        CentralPanel::default().show(ctx, |ui| {
            let (title, placeholder) = match self.nav {
                Nav::Home => ("Home", "Nothing here yet. New downloads will show up here."),
                Nav::Queue => ("Queue", "Your download queue is empty."),
                Nav::Settings => ("Settings", "Settings arrive in a later phase."),
            };

            ui.add_space(8.0);
            ui.heading(title);
            ui.add_space(2.0);
            ui.label(egui::RichText::new(placeholder).color(p.muted));
        });
    }
}

fn nav_pill(ui: &mut egui::Ui, label: &str, selected: bool, p: &Palette) -> bool {
    let size = Vec2::new(ui.available_width().max(0.0), PILL_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();

    if selected {
        painter.rect_filled(rect, egui::CornerRadius::same(12), p.accent_tint);
    } else if response.hovered() {
        painter.rect_filled(rect, egui::CornerRadius::same(12), p.hover_tint);
    }

    let color = if selected {
        p.accent
    } else if response.hovered() {
        p.hover
    } else {
        p.text
    };
    painter.text(
        rect.left_center() + Vec2::new(14.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        color,
    );
    response.clicked()
}

fn status_row(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(6.0);
    let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), 18.0));
    let center = Pos2::new(rect.left() + 6.0, rect.center().y);
    ui.painter().circle_filled(center, 3.5, p.accent);
}

/// 5-petal lotus with a chevron-down knocked into its center.
fn draw_logo(ui: &mut egui::Ui, rect: egui::Rect, p: &Palette) {
    let center = Pos2::new(rect.center().x, rect.top() + 8.0 + LOGO_SIDE / 2.0);
    let radius = LOGO_SIDE / 2.0;

    let petal_fill = Color32::from_rgba_unmultiplied(p.text.r(), p.text.g(), p.text.b(), 235);
    for dir in [0.0, 72.0, -72.0, 144.0, -144.0] {
        let points = petal_points(center, radius, dir);
        ui.painter().add(egui::epaint::Shape::convex_polygon(
            points,
            petal_fill,
            egui::epaint::Stroke::NONE,
        ));
    }

    ui.painter().circle_filled(center, 10.0, p.bg);
    let chevron = vec![
        Pos2::new(center.x - 7.0, center.y - 3.0),
        Pos2::new(center.x, center.y + 4.0),
        Pos2::new(center.x + 7.0, center.y - 3.0),
    ];
    ui.painter().add(egui::epaint::Shape::line(
        chevron,
        Stroke::new(2.5_f32, p.accent),
    ));
}

fn petal_points(center: Pos2, radius: f32, dir_deg: f32) -> Vec<Pos2> {
    let length = radius * 0.94;
    let width = radius * 0.44;
    let steps = 10usize;
    let angle = dir_deg.to_radians();
    let (sin, cos) = (angle.sin(), angle.cos());

    let mut points = Vec::with_capacity((steps + 1) * 2);
    for side in [1.0f32, -1.0f32] {
        let range: Box<dyn Iterator<Item = usize>> = if side > 0.0 {
            Box::new(0..=steps)
        } else {
            Box::new((0..=steps).rev())
        };
        for i in range {
            let s = i as f32 / steps as f32;
            let local_x = side * width * (std::f32::consts::PI * s).sin().powf(0.85);
            let local_y = -length * s;
            points.push(Pos2::new(
                center.x + local_x * cos - local_y * sin,
                center.y + local_x * sin + local_y * cos,
            ));
        }
    }
    points
}
