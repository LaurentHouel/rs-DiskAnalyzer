use egui::{Color32, Context, CornerRadius, FontId, Shadow, Stroke, TextStyle, Theme, Visuals, vec2};

/// Colors that egui's `Visuals` cannot express, resolved for the active theme.
pub struct Palette {
    pub accent: Color32,
    pub hero_fill: Color32,
    pub hero_stroke: Color32,
    pub hero_title: Color32,
    pub hero_text: Color32,
    pub card: Color32,
    pub track: Color32,
    pub drive_icon: Color32,
    pub btn_fill: Color32,
    pub btn_hover: Color32,
    pub btn_border: Color32,
    pub btn_text: Color32,
    pub btn_disabled: Color32,
    pub btn_disabled_border: Color32,
    pub theme_icon: Color32,
    pub ok: Color32,
    pub warn: Color32,
}

impl Palette {
    pub fn new(dark: bool) -> Self {
        if dark {
            Self {
                accent: Color32::from_rgb(77, 148, 255),
                hero_fill: Color32::from_rgb(20, 33, 56),
                hero_stroke: Color32::from_rgb(36, 58, 94),
                hero_title: Color32::from_rgb(235, 242, 255),
                hero_text: Color32::from_rgb(145, 163, 190),
                card: Color32::from_rgb(24, 30, 40),
                track: Color32::from_rgb(40, 49, 62),
                drive_icon: Color32::from_rgb(110, 165, 245),
                btn_fill: Color32::from_rgb(21, 26, 35),
                btn_hover: Color32::from_rgb(30, 37, 50),
                btn_border: Color32::from_rgb(42, 49, 63),
                btn_text: Color32::from_rgb(226, 231, 239),
                btn_disabled: Color32::from_rgb(74, 82, 98),
                btn_disabled_border: Color32::from_rgb(29, 35, 46),
                theme_icon: Color32::from_rgb(246, 198, 62),
                ok: Color32::from_rgb(74, 211, 124),
                warn: Color32::from_rgb(244, 184, 60),
            }
        } else {
            Self {
                accent: Color32::from_rgb(47, 111, 218),
                hero_fill: Color32::from_rgb(237, 244, 255),
                hero_stroke: Color32::from_rgb(218, 230, 249),
                hero_title: Color32::from_rgb(31, 54, 88),
                hero_text: Color32::from_rgb(82, 99, 122),
                card: Color32::WHITE,
                track: Color32::from_rgb(232, 237, 245),
                drive_icon: Color32::from_rgb(82, 139, 218),
                btn_fill: Color32::WHITE,
                btn_hover: Color32::from_rgb(237, 243, 252),
                btn_border: Color32::from_rgb(217, 224, 235),
                btn_text: Color32::from_rgb(36, 48, 70),
                btn_disabled: Color32::from_rgb(172, 180, 193),
                btn_disabled_border: Color32::from_rgb(232, 236, 242),
                theme_icon: Color32::from_rgb(72, 90, 130),
                ok: Color32::from_rgb(40, 168, 92),
                warn: Color32::from_rgb(222, 150, 20),
            }
        }
    }
}

fn dark_visuals() -> Visuals {
    let mut v = Visuals::dark();
    let radius = CornerRadius::same(8);
    v.panel_fill = Color32::from_rgb(15, 19, 25);
    v.window_fill = Color32::from_rgb(22, 27, 35);
    v.extreme_bg_color = Color32::from_rgb(11, 14, 19);
    v.faint_bg_color = Color32::from_rgb(20, 25, 33);
    v.selection.bg_fill = Color32::from_rgb(27, 52, 94);
    v.selection.stroke.color = Color32::from_rgb(110, 168, 255);
    v.hyperlink_color = Color32::from_rgb(110, 168, 255);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, Color32::from_rgb(40, 49, 62));
    v.window_shadow = Shadow {
        offset: [0, 10],
        blur: 28,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    v.popup_shadow = v.window_shadow;
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 27, 35);
    v.widgets.noninteractive.bg_stroke.color = Color32::from_rgb(36, 44, 56);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(28, 34, 44);
    v.widgets.inactive.bg_fill = Color32::from_rgb(28, 34, 44);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(37, 46, 60);
    v.widgets.hovered.bg_stroke.color = Color32::from_rgb(70, 110, 170);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(45, 58, 80);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v
}

fn light_visuals() -> Visuals {
    let mut v = Visuals::light();
    let radius = CornerRadius::same(8);
    v.panel_fill = Color32::from_rgb(246, 248, 251);
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.faint_bg_color = Color32::from_rgb(242, 245, 249);
    v.selection.bg_fill = Color32::from_rgb(229, 239, 255);
    v.selection.stroke.color = Color32::from_rgb(47, 111, 218);
    v.hyperlink_color = Color32::from_rgb(43, 105, 205);
    v.window_corner_radius = CornerRadius::same(14);
    v.menu_corner_radius = CornerRadius::same(10);
    v.widgets.noninteractive.bg_fill = Color32::WHITE;
    v.widgets.noninteractive.bg_stroke.color = Color32::from_rgb(224, 229, 237);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(248, 249, 252);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(235, 242, 252);
    v.widgets.hovered.bg_stroke.color = Color32::from_rgb(174, 197, 232);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(225, 236, 252);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    v
}

/// Installs both themes and selects the one matching `dark`.
pub fn apply(ctx: &Context, dark: bool) {
    ctx.set_visuals_of(Theme::Dark, dark_visuals());
    ctx.set_visuals_of(Theme::Light, light_visuals());
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(10.0, 6.0);
        style.spacing.interact_size.y = 30.0;
        style.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        style.text_styles.insert(TextStyle::Button, FontId::proportional(13.5));
        style.text_styles.insert(TextStyle::Heading, FontId::proportional(25.0));
        style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    });
    ctx.set_theme(Theme::from_dark_mode(dark));
}
