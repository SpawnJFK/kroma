//! Visual theme: a neutral dark UI close to pro image editors, plus a light variant.

use egui::{Color32, CornerRadius, Stroke, Visuals};

pub fn apply(ctx: &egui::Context, dark: bool) {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let accent = Color32::from_rgb(56, 132, 255);
    if dark {
        v.panel_fill = Color32::from_rgb(50, 50, 52);
        v.window_fill = Color32::from_rgb(44, 44, 46);
        v.extreme_bg_color = Color32::from_rgb(30, 30, 32);
        v.faint_bg_color = Color32::from_rgb(56, 56, 58);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(50, 50, 52);
        v.widgets.inactive.bg_fill = Color32::from_rgb(64, 64, 67);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(62, 62, 65);
        v.widgets.hovered.bg_fill = Color32::from_rgb(80, 80, 84);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(78, 78, 82);
        v.widgets.active.bg_fill = Color32::from_rgb(90, 90, 96);
    }
    v.selection.bg_fill = accent;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.hyperlink_color = accent;
    v.window_corner_radius = CornerRadius::same(6);
    v.menu_corner_radius = CornerRadius::same(4);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(6.0, 2.0);
        s.spacing.interact_size.y = 20.0;
    });
}
