use egui::{
    style::{Selection, WidgetVisuals, Widgets},
    Color32, Context, CornerRadius, Shadow, Stroke, Style, Visuals,
};

pub fn setup_custom_theme(ctx: &Context) {
    let mut style = Style::default();

    let bg_dark = Color32::from_rgb(20, 22, 26);
    let panel_bg = Color32::from_rgb(28, 30, 36);
    let border_color = Color32::from_rgb(48, 52, 62);
    let accent_color = Color32::from_rgb(255, 180, 0); // QuickTime Yellow
    let accent_hover = Color32::from_rgb(255, 200, 40);

    let visuals = Visuals {
        dark_mode: true,
        override_text_color: Some(Color32::from_rgb(235, 238, 245)),
        window_fill: panel_bg,
        panel_fill: bg_dark,
        window_stroke: Stroke::new(1.0, border_color),
        window_corner_radius: CornerRadius::same(8),
        window_shadow: Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(160),
        },
        selection: Selection {
            bg_fill: accent_color,
            stroke: Stroke::new(1.0, Color32::WHITE),
        },
        widgets: Widgets {
            noninteractive: WidgetVisuals {
                bg_fill: panel_bg,
                weak_bg_fill: panel_bg,
                bg_stroke: Stroke::new(1.0, border_color),
                corner_radius: CornerRadius::same(4),
                fg_stroke: Stroke::new(1.0, Color32::from_rgb(180, 185, 195)),
                expansion: 0.0,
            },
            inactive: WidgetVisuals {
                bg_fill: Color32::from_rgb(38, 41, 48),
                weak_bg_fill: Color32::from_rgb(38, 41, 48),
                bg_stroke: Stroke::new(1.0, border_color),
                corner_radius: CornerRadius::same(5),
                fg_stroke: Stroke::new(1.0, Color32::from_rgb(220, 225, 235)),
                expansion: 0.0,
            },
            hovered: WidgetVisuals {
                bg_fill: Color32::from_rgb(50, 54, 64),
                weak_bg_fill: Color32::from_rgb(50, 54, 64),
                bg_stroke: Stroke::new(1.0, accent_hover),
                corner_radius: CornerRadius::same(5),
                fg_stroke: Stroke::new(1.5, Color32::WHITE),
                expansion: 1.0,
            },
            active: WidgetVisuals {
                bg_fill: accent_color,
                weak_bg_fill: accent_color,
                bg_stroke: Stroke::new(1.0, accent_color),
                corner_radius: CornerRadius::same(5),
                fg_stroke: Stroke::new(2.0, Color32::BLACK),
                expansion: 1.0,
            },
            open: WidgetVisuals {
                bg_fill: Color32::from_rgb(45, 48, 56),
                weak_bg_fill: Color32::from_rgb(45, 48, 56),
                bg_stroke: Stroke::new(1.0, border_color),
                corner_radius: CornerRadius::same(4),
                fg_stroke: Stroke::new(1.0, Color32::WHITE),
                expansion: 0.0,
            },
        },
        ..Visuals::dark()
    };

    style.visuals = visuals;
    ctx.set_style(style);
}

