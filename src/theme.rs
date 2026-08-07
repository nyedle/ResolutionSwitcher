use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub good: Color32,
    pub bad: Color32,
}

const NIGHT: Palette = Palette {
    bg: Color32::from_rgb(0x16, 0x17, 0x19),
    surface: Color32::from_rgb(0x1d, 0x1f, 0x21),
    raised: Color32::from_rgb(0x27, 0x29, 0x2c),
    hover: Color32::from_rgb(0x32, 0x35, 0x38),
    line: Color32::from_rgb(0x2f, 0x31, 0x34),
    text: Color32::from_rgb(0xe4, 0xe2, 0xdd),
    muted: Color32::from_rgb(0x8c, 0x8a, 0x84),
    accent: Color32::from_rgb(0x6d, 0x8f, 0xa8),
    accent_hover: Color32::from_rgb(0x87, 0xa8, 0xbf),
    good: Color32::from_rgb(0x86, 0xa9, 0x66),
    bad: Color32::from_rgb(0xc7, 0x6b, 0x62),
};

const DAY: Palette = Palette {
    bg: Color32::from_rgb(0xf4, 0xf2, 0xee),
    surface: Color32::from_rgb(0xfd, 0xfc, 0xfa),
    raised: Color32::from_rgb(0xe8, 0xe5, 0xdf),
    hover: Color32::from_rgb(0xdb, 0xd7, 0xcf),
    line: Color32::from_rgb(0xd8, 0xd4, 0xcc),
    text: Color32::from_rgb(0x23, 0x23, 0x20),
    muted: Color32::from_rgb(0x6c, 0x6b, 0x64),
    accent: Color32::from_rgb(0x4a, 0x6f, 0x8a),
    accent_hover: Color32::from_rgb(0x3a, 0x5b, 0x74),
    good: Color32::from_rgb(0x4f, 0x7a, 0x3f),
    bad: Color32::from_rgb(0xa9, 0x4f, 0x45),
};

pub fn colors(dark: bool) -> Palette {
    if dark { NIGHT } else { DAY }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    let visuals = build(colors(dark), dark);

    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(9.0, 4.0);
        style.spacing.menu_margin = egui::Margin::same(4);
        style.spacing.interact_size.y = 23.0;
        style.spacing.indent = 14.0;
        style.spacing.scroll.bar_width = 8.0;

        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(16.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.0, FontFamily::Monospace),
            ),
        ]
        .into();
    });
}

fn build(p: Palette, dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.extreme_bg_color = if dark { p.bg } else { p.surface };
    v.faint_bg_color = p.raised;
    v.error_fg_color = p.bad;
    v.warn_fg_color = p.good;
    v.hyperlink_color = p.accent_hover;
    v.window_stroke = Stroke::new(1.0, p.line);

    v.widgets.noninteractive.bg_fill = p.surface;
    v.widgets.noninteractive.weak_bg_fill = p.surface;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);

    v.widgets.inactive.bg_fill = p.raised;
    v.widgets.inactive.weak_bg_fill = p.raised;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);

    v.widgets.hovered.bg_fill = p.hover;
    v.widgets.hovered.weak_bg_fill = p.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);

    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.weak_bg_fill = p.accent;
    v.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);
    v.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);

    v.widgets.open = v.widgets.hovered;

    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.45 } else { 0.3 });
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.weak_text_color = Some(p.muted);

    for widget in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        widget.corner_radius = CornerRadius::ZERO;
        widget.expansion = 0.0;
    }
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    v.window_shadow.offset = [0, 8];
    v.window_shadow.blur = 24;
    v.popup_shadow.offset = [0, 4];
    v.popup_shadow.blur = 14;
    v
}
