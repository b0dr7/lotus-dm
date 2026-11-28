use eframe::egui::{self, Align2, Color32, FontId, Pos2, RichText, Sense, Stroke, Vec2};

use crate::ui::theme::Palette;

const LOGO_SIDE: f32 = 64.0;
const PILL_HEIGHT: f32 = 34.0;

pub fn nav_pill(ui: &mut egui::Ui, label: &str, selected: bool, p: &Palette) -> bool {
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

pub fn wordmark(ui: &mut egui::Ui, p: &Palette) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("LOTUS").size(13.0).strong().color(p.text));
        ui.label(RichText::new("DOWNLOAD MANAGER").size(8.5).color(p.muted));
    });
}

pub fn version_row(ui: &mut egui::Ui, p: &Palette) {
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                .size(10.5)
                .color(p.muted),
        );
    });
}

pub fn status_row(ui: &mut egui::Ui, p: &Palette) {
    ui.add_space(6.0);
    let (_, rect) = ui.allocate_space(Vec2::new(ui.available_width(), 18.0));
    let center = Pos2::new(rect.left() + 6.0, rect.center().y);
    ui.painter().circle_filled(center, 3.5, p.accent);
}

pub fn draw_logo(ui: &mut egui::Ui, rect: egui::Rect, p: &Palette) {
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
