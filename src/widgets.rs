use crate::icons::{self, Icon};
use crate::theme::Palette;
use egui::{Color32, CornerRadius, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

const HEIGHT: f32 = 32.0;
const ICON: f32 = 16.0;
const PAD: f32 = 11.0;
const GAP: f32 = 8.0;

/// What a toolbar button shows; `label` is the accessible name (also used for tooltips).
pub struct Btn<'a> {
    pub icon: Icon,
    pub icon_color: Option<Color32>,
    pub text: Option<&'a str>,
    pub trailing: Option<Icon>,
    pub label: &'a str,
}

impl<'a> Btn<'a> {
    pub fn icon(icon: Icon, label: &'a str) -> Self {
        Self { icon, icon_color: None, text: None, trailing: None, label }
    }

    pub fn with_text(mut self, text: &'a str) -> Self {
        self.text = Some(text);
        self
    }

    pub fn icon_color(mut self, color: Color32) -> Self {
        self.icon_color = Some(color);
        self
    }

    pub fn trailing(mut self, icon: Icon) -> Self {
        self.trailing = Some(icon);
        self
    }
}

pub fn tool_button(ui: &mut Ui, pal: &Palette, enabled: bool, btn: Btn) -> Response {
    let text_color = if enabled { pal.btn_text } else { pal.btn_disabled };
    let galley = btn
        .text
        .map(|t| ui.painter().layout_no_wrap(t.to_owned(), FontId::proportional(14.0), text_color));

    let mut content = ICON;
    if let Some(g) = &galley {
        content += GAP + g.size().x;
    }
    if btn.trailing.is_some() {
        content += GAP + 12.0;
    }
    let width = if galley.is_none() && btn.trailing.is_none() { HEIGHT } else { content + 2.0 * PAD };

    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(width, HEIGHT), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, btn.label));
    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };

    if ui.is_rect_visible(rect) {
        let hovered = enabled && resp.hovered();
        let fill = if hovered { pal.btn_hover } else { pal.btn_fill };
        let border = if !enabled {
            pal.btn_disabled_border
        } else if hovered {
            pal.accent.gamma_multiply(0.55)
        } else {
            pal.btn_border
        };
        let p = ui.painter();
        p.rect(rect, CornerRadius::same(9), fill, Stroke::new(1.0, border), StrokeKind::Inside);

        let icon_color = match (enabled, btn.icon_color) {
            (true, Some(c)) => c,
            _ => text_color,
        };
        let cy = rect.center().y;
        let mut x = rect.center().x - content / 2.0;
        let icon_rect = Rect::from_center_size(pos2(x + ICON / 2.0, cy), vec2(ICON, ICON));
        icons::paint(p, btn.icon, icon_rect, icon_color);
        x += ICON;
        if let Some(g) = galley {
            x += GAP;
            let w = g.size().x;
            p.galley(pos2(x, cy - g.size().y / 2.0), g, text_color);
            x += w;
        }
        if let Some(t) = btn.trailing {
            x += GAP;
            let r = Rect::from_center_size(pos2(x + 6.0, cy), vec2(12.0, 12.0));
            icons::paint(p, t, r, text_color);
        }
    }
    resp
}

/// Thin vertical divider between toolbar groups.
pub fn divider(ui: &mut Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, pal.btn_border));
}

/// Rounded status badge anchored to `right_center`, returns its rect.
pub fn status_pill(ui: &mut Ui, pal: &Palette, right_center: egui::Pos2, text: &str, dot: Color32) -> Rect {
    let weak = ui.visuals().weak_text_color();
    let galley = ui.painter().layout_no_wrap(text.to_owned(), FontId::proportional(13.0), weak);
    let size = vec2(14.0 + 7.0 + 8.0 + galley.size().x + 14.0, 28.0);
    let rect = Rect::from_min_size(pos2(right_center.x - size.x, right_center.y - size.y / 2.0), size);
    let p = ui.painter();
    p.rect(rect, CornerRadius::same(14), pal.btn_fill, Stroke::new(1.0, pal.btn_border), StrokeKind::Inside);
    p.circle_filled(pos2(rect.left() + 17.0, rect.center().y), 3.5, dot);
    p.galley(pos2(rect.left() + 29.0, rect.center().y - galley.size().y / 2.0), galley, weak);
    rect
}
