use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};

use crate::core::sys;
use crate::ui::theme::Palette;

const POLL: Duration = Duration::from_secs(2);
const CARD_WIDTH: f32 = 360.0;
const CARD_HEIGHT: f32 = 92.0;

pub enum PromptAction {
    Add,
    Dismiss,
}

#[derive(Default)]
pub struct ClipboardWatch {
    last_poll: Option<Instant>,
    last_text: String,
    pub pending: Option<String>,
}

impl ClipboardWatch {
    /// Poll the clipboard every 2s while enabled and the window has focus.
    /// A newly copied URL becomes the pending prompt.
    pub fn poll(&mut self, ctx: &egui::Context, enabled: bool) {
        if !enabled {
            self.last_poll = None;
            self.last_text.clear();
            self.pending = None;
            return;
        }
        if !ctx.input(|input| input.focused) {
            return;
        }
        if let Some(last) = self.last_poll {
            if last.elapsed() < POLL {
                return;
            }
        }
        self.last_poll = Some(Instant::now());
        let Some(text) = sys::clipboard_text() else {
            return;
        };
        let text = text.trim().to_string();
        if text == self.last_text {
            return;
        }
        self.last_text = text;
        if let Some(url) = first_url(&self.last_text) {
            self.pending = Some(url);
        }
    }

    /// Draw the inline "Add download?" card and report the user's choice.
    pub fn show(&self, ctx: &egui::Context, p: &Palette) -> Option<PromptAction> {
        let url = self.pending.as_ref()?;
        let mut action = None;

        egui::Area::new(egui::Id::new("clip_prompt"))
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -16.0))
            .show(ctx, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(CARD_WIDTH, CARD_HEIGHT), Sense::hover());

                let button_h = 24.0;
                let button_y = rect.max.y - 12.0 - button_h;
                let dismiss_w = 74.0;
                let add_w = 54.0;
                let add_rect = Rect::from_min_size(
                    Pos2::new(rect.max.x - 12.0 - add_w, button_y),
                    Vec2::new(add_w, button_h),
                );
                let dismiss_rect = Rect::from_min_size(
                    Pos2::new(add_rect.left() - 8.0 - dismiss_w, button_y),
                    Vec2::new(dismiss_w, button_h),
                );
                let add = ui.allocate_rect(add_rect, Sense::click());
                let dismiss = ui.allocate_rect(dismiss_rect, Sense::click());

                let painter = ui.painter();
                painter.rect_filled(rect, CornerRadius::same(12), p.surface);
                painter.rect(
                    rect,
                    CornerRadius::same(12),
                    Color32::TRANSPARENT,
                    Stroke::new(1.0_f32, p.border),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.min + Vec2::new(12.0, 12.0),
                    Align2::LEFT_TOP,
                    "Add download?",
                    FontId::proportional(13.0),
                    p.text,
                );

                let url_clip = Rect::from_min_max(
                    rect.min + Vec2::new(12.0, 34.0),
                    Pos2::new(rect.max.x - 12.0, rect.max.y - 36.0),
                );
                painter.with_clip_rect(url_clip).text(
                    url_clip.min,
                    Align2::LEFT_TOP,
                    url,
                    FontId::proportional(12.0),
                    p.muted,
                );

                painter.rect_filled(add_rect, CornerRadius::same(12), p.accent);
                painter.text(
                    add_rect.center(),
                    Align2::CENTER_CENTER,
                    "Add",
                    FontId::proportional(12.0),
                    p.bg,
                );
                painter.rect_filled(dismiss_rect, CornerRadius::same(12), p.surface2);
                painter.rect(
                    dismiss_rect,
                    CornerRadius::same(12),
                    Color32::TRANSPARENT,
                    Stroke::new(1.0_f32, p.border),
                    StrokeKind::Inside,
                );
                painter.text(
                    dismiss_rect.center(),
                    Align2::CENTER_CENTER,
                    "Dismiss",
                    FontId::proportional(12.0),
                    p.text,
                );

                if add.clicked() {
                    action = Some(PromptAction::Add);
                } else if dismiss.clicked() {
                    action = Some(PromptAction::Dismiss);
                }
            });

        action
    }
}

/// First http(s) URL in the text, taken from the first non-empty line.
fn first_url(text: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("http://") || line.starts_with("https://") {
            return Some(line.to_string());
        }
    }
    None
}
