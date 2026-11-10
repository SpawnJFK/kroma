//! Toolbox definitions and per-tool options (the Options bar).

use crate::widgets::{blend_combo, color_button, gradient_editor};
use egui::Ui;
use kroma_core::brush::{presets as brush_presets, BrushSettings, ToneRange};
use kroma_core::gradient::{GradientFill, GradientShape};
use kroma_core::selection::SelectOp;
use kroma_core::shapes::{ShapeKind, ShapeStyle};
use kroma_core::text::{FontChoice, TextAlign, TextStyle};
use kroma_core::transform::LiquifyMode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tool {
    Move,
    MarqueeRect,
    MarqueeEllipse,
    Lasso,
    PolyLasso,
    MagicWand,
    Crop,
    Eyedropper,
    SpotHeal,
    Heal,
    Clone,
    Brush,
    Pencil,
    Eraser,
    Smudge,
    BlurTool,
    SharpenTool,
    Dodge,
    Burn,
    Sponge,
    Gradient,
    Bucket,
    Text,
    Shape,
    Liquify,
    Hand,
    Zoom,
}

impl Tool {
    /// Toolbox groups, Photoshop order. Each group shares one keyboard shortcut (cycle with Shift).
    pub const GROUPS: &'static [&'static [Tool]] = &[
        &[Tool::Move],
        &[Tool::MarqueeRect, Tool::MarqueeEllipse],
        &[Tool::Lasso, Tool::PolyLasso],
        &[Tool::MagicWand],
        &[Tool::Crop],
        &[Tool::Eyedropper],
        &[Tool::SpotHeal, Tool::Heal],
        &[Tool::Brush, Tool::Pencil],
        &[Tool::Clone],
        &[Tool::Eraser],
        &[Tool::Gradient, Tool::Bucket],
        &[Tool::BlurTool, Tool::SharpenTool, Tool::Smudge],
        &[Tool::Dodge, Tool::Burn, Tool::Sponge],
        &[Tool::Text],
        &[Tool::Shape],
        &[Tool::Liquify],
        &[Tool::Hand],
        &[Tool::Zoom],
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Move => "Move",
            Tool::MarqueeRect => "Rectangular Marquee",
            Tool::MarqueeEllipse => "Elliptical Marquee",
            Tool::Lasso => "Lasso",
            Tool::PolyLasso => "Polygonal Lasso",
            Tool::MagicWand => "Magic Wand",
            Tool::Crop => "Crop",
            Tool::Eyedropper => "Eyedropper",
            Tool::SpotHeal => "Spot Healing Brush",
            Tool::Heal => "Healing Brush",
            Tool::Clone => "Clone Stamp",
            Tool::Brush => "Brush",
            Tool::Pencil => "Pencil",
            Tool::Eraser => "Eraser",
            Tool::Smudge => "Smudge",
            Tool::BlurTool => "Blur",
            Tool::SharpenTool => "Sharpen",
            Tool::Dodge => "Dodge",
            Tool::Burn => "Burn",
            Tool::Sponge => "Sponge",
            Tool::Gradient => "Gradient",
            Tool::Bucket => "Paint Bucket",
            Tool::Text => "Type",
            Tool::Shape => "Shape",
            Tool::Liquify => "Liquify",
            Tool::Hand => "Hand",
            Tool::Zoom => "Zoom",
        }
    }

    pub fn shortcut(self) -> Option<egui::Key> {
        use egui::Key::*;
        Some(match self {
            Tool::Move => V,
            Tool::MarqueeRect | Tool::MarqueeEllipse => M,
            Tool::Lasso | Tool::PolyLasso => L,
            Tool::MagicWand => W,
            Tool::Crop => C,
            Tool::Eyedropper => I,
            Tool::SpotHeal | Tool::Heal => J,
            Tool::Brush | Tool::Pencil => B,
            Tool::Clone => S,
            Tool::Eraser => E,
            Tool::Gradient | Tool::Bucket => G,
            Tool::BlurTool | Tool::SharpenTool | Tool::Smudge => R,
            Tool::Dodge | Tool::Burn | Tool::Sponge => O,
            Tool::Text => T,
            Tool::Shape => U,
            Tool::Liquify => Q,
            Tool::Hand => H,
            Tool::Zoom => Z,
        })
    }

    pub fn is_brush_like(self) -> bool {
        matches!(
            self,
            Tool::Brush
                | Tool::Pencil
                | Tool::Eraser
                | Tool::Clone
                | Tool::Heal
                | Tool::SpotHeal
                | Tool::Smudge
                | Tool::BlurTool
                | Tool::SharpenTool
                | Tool::Dodge
                | Tool::Burn
                | Tool::Sponge
                | Tool::Liquify
        )
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Move => "Drag to move the layer (or the selected pixels). Arrow keys nudge 1px, Shift+arrow 10px.",
            Tool::MarqueeRect | Tool::MarqueeEllipse => "Drag to select. Shift = add, Alt = subtract, Shift+Alt = intersect.",
            Tool::Lasso => "Drag a freehand selection.",
            Tool::PolyLasso => "Click points, double-click or Enter to close, Esc to cancel.",
            Tool::MagicWand => "Click to select similar colors. Shift/Alt add/subtract.",
            Tool::Crop => "Drag the crop box, then press Enter (or Apply). Esc cancels.",
            Tool::Eyedropper => "Click to pick foreground, Alt+click for background.",
            Tool::SpotHeal => "Paint over a blemish; it is filled from the surroundings when you release.",
            Tool::Heal | Tool::Clone => "Alt+click to set the source, then paint.",
            Tool::Text => "Click to add text, click existing text to edit it.",
            Tool::Gradient => "Drag to draw a gradient.",
            Tool::Bucket => "Click to fill similar colors.",
            Tool::Shape => "Drag to draw a shape on a new layer. Shift = constrain.",
            Tool::Liquify => "Drag to push pixels. Enter applies, Esc cancels.",
            Tool::Hand => "Drag to pan (or hold Space with any tool).",
            Tool::Zoom => "Click to zoom in, Alt+click to zoom out.",
            _ => "Paint on the canvas. [ ] change size, Shift+[ ] hardness.",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ToolOptions {
    pub brush: BrushSettings,
    pub pencil: BrushSettings,
    pub eraser: BrushSettings,
    pub retouch: BrushSettings,
    pub clone_aligned: bool,
    pub clone_sample_all: bool,
    pub tone_range: ToneRange,
    pub sponge_saturate: bool,
    pub select_op: SelectOp,
    pub feather: f32,
    pub antialias: bool,
    pub wand_tolerance: f32,
    pub wand_contiguous: bool,
    pub wand_sample_all: bool,
    pub eyedropper_sample_all: bool,
    pub gradient: GradientFill,
    pub gradient_use_fg_bg: bool,
    pub bucket_tolerance: f32,
    pub bucket_contiguous: bool,
    pub bucket_opacity: f32,
    pub text: TextStyle,
    pub shape: ShapeKind,
    pub shape_style: ShapeStyle,
    pub shape_new_layer: bool,
    pub crop_ratio: Option<(f32, f32)>,
    pub crop_delete_pixels: bool,
    pub liquify_mode: LiquifyMode,
    pub liquify_size: f32,
    pub liquify_pressure: f32,
    pub move_auto_select: bool,
}

impl Default for ToolOptions {
    fn default() -> Self {
        ToolOptions {
            brush: BrushSettings::default(),
            pencil: BrushSettings {
                hardness: 1.0,
                size: 4.0,
                spacing: 0.05,
                ..Default::default()
            },
            eraser: BrushSettings {
                hardness: 0.6,
                size: 40.0,
                ..Default::default()
            },
            retouch: BrushSettings {
                hardness: 0.5,
                size: 40.0,
                flow: 0.5,
                ..Default::default()
            },
            clone_aligned: true,
            clone_sample_all: false,
            tone_range: ToneRange::Midtones,
            sponge_saturate: false,
            select_op: SelectOp::Replace,
            feather: 0.0,
            antialias: true,
            wand_tolerance: 32.0,
            wand_contiguous: true,
            wand_sample_all: false,
            eyedropper_sample_all: true,
            gradient: GradientFill::default(),
            gradient_use_fg_bg: true,
            bucket_tolerance: 32.0,
            bucket_contiguous: true,
            bucket_opacity: 1.0,
            text: TextStyle::default(),
            shape: ShapeKind::Rectangle,
            shape_style: ShapeStyle::default(),
            shape_new_layer: true,
            crop_ratio: None,
            crop_delete_pixels: true,
            liquify_mode: LiquifyMode::ForwardWarp,
            liquify_size: 80.0,
            liquify_pressure: 0.6,
            move_auto_select: false,
        }
    }
}

impl ToolOptions {
    pub fn brush_for(&mut self, tool: Tool) -> Option<&mut BrushSettings> {
        match tool {
            Tool::Brush => Some(&mut self.brush),
            Tool::Pencil => Some(&mut self.pencil),
            Tool::Eraser => Some(&mut self.eraser),
            Tool::Clone
            | Tool::Heal
            | Tool::SpotHeal
            | Tool::Smudge
            | Tool::BlurTool
            | Tool::SharpenTool
            | Tool::Dodge
            | Tool::Burn
            | Tool::Sponge => Some(&mut self.retouch),
            _ => None,
        }
    }
    pub fn tool_size(&mut self, tool: Tool) -> Option<&mut f32> {
        if tool == Tool::Liquify {
            return Some(&mut self.liquify_size);
        }
        self.brush_for(tool).map(|b| &mut b.size)
    }
}

fn brush_common(ui: &mut Ui, b: &mut BrushSettings, tool: Tool) {
    egui::ComboBox::from_id_salt("brush_preset")
        .selected_text("Presets")
        .width(80.0)
        .show_ui(ui, |ui| {
            for (n, p) in brush_presets() {
                if ui.button(n).clicked() {
                    let color = b.color;
                    *b = p;
                    b.color = color;
                }
            }
        });
    ui.label("Size");
    ui.add(
        egui::DragValue::new(&mut b.size)
            .range(1.0..=2500.0)
            .suffix(" px")
            .speed(0.5),
    );
    if tool != Tool::Pencil {
        ui.label("Hardness");
        let mut h = b.hardness * 100.0;
        if ui
            .add(egui::DragValue::new(&mut h).range(0.0..=100.0).suffix("%"))
            .changed()
        {
            b.hardness = h / 100.0;
        }
    }
    let is_retouch = matches!(
        tool,
        Tool::Smudge | Tool::BlurTool | Tool::SharpenTool | Tool::Dodge | Tool::Burn | Tool::Sponge
    );
    if !is_retouch {
        ui.label("Opacity");
        let mut o = b.opacity * 100.0;
        if ui
            .add(egui::DragValue::new(&mut o).range(1.0..=100.0).suffix("%"))
            .changed()
        {
            b.opacity = o / 100.0;
        }
    }
    ui.label(match tool {
        Tool::Smudge | Tool::BlurTool | Tool::SharpenTool => "Strength",
        Tool::Dodge | Tool::Burn => "Exposure",
        _ => "Flow",
    });
    let mut f = b.flow * 100.0;
    if ui
        .add(egui::DragValue::new(&mut f).range(1.0..=100.0).suffix("%"))
        .changed()
    {
        b.flow = f / 100.0;
    }
    ui.menu_button("Brush tip", |ui| {
        ui.add(egui::Slider::new(&mut b.spacing, 0.01..=2.0).text("Spacing"));
        ui.add(egui::Slider::new(&mut b.angle, -180.0..=180.0).text("Angle"));
        ui.add(egui::Slider::new(&mut b.roundness, 0.05..=1.0).text("Roundness"));
        ui.add(egui::Slider::new(&mut b.scatter, 0.0..=10.0).text("Scatter"));
        ui.add(egui::Slider::new(&mut b.size_jitter, 0.0..=1.0).text("Size jitter"));
        ui.checkbox(&mut b.pressure_size, "Pen pressure → size");
        ui.checkbox(&mut b.pressure_opacity, "Pen pressure → flow");
    });
    if matches!(tool, Tool::Brush | Tool::Pencil) {
        blend_combo(ui, "brush_mode", &mut b.mode);
    }
    if matches!(tool, Tool::Brush | Tool::Pencil | Tool::Eraser) {
        ui.checkbox(&mut b.lock_alpha, "Lock transparency");
    }
}

fn select_op_ui(ui: &mut Ui, o: &mut ToolOptions) {
    for op in SelectOp::ALL {
        ui.selectable_value(&mut o.select_op, op, op.name());
    }
    ui.separator();
}

/// The Options bar under the menu: shows the active tool's settings.
pub fn options_bar(
    ui: &mut Ui,
    tool: Tool,
    o: &mut ToolOptions,
    actions: &mut Vec<crate::app::Cmd>,
) {
    use crate::app::Cmd;
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(tool.name()).strong());
        ui.separator();
        match tool {
            Tool::Brush | Tool::Pencil | Tool::Eraser => {
                if let Some(b) = o.brush_for(tool) {
                    brush_common(ui, b, tool);
                }
            }
            Tool::Clone | Tool::Heal => {
                brush_common(ui, &mut o.retouch, tool);
                ui.checkbox(&mut o.clone_aligned, "Aligned");
                ui.checkbox(&mut o.clone_sample_all, "Sample all layers");
            }
            Tool::SpotHeal => {
                brush_common(ui, &mut o.retouch, tool);
                ui.label("Type: Content-Aware");
            }
            Tool::Smudge | Tool::BlurTool | Tool::SharpenTool => {
                brush_common(ui, &mut o.retouch, tool)
            }
            Tool::Dodge | Tool::Burn => {
                brush_common(ui, &mut o.retouch, tool);
                egui::ComboBox::from_id_salt("tone")
                    .selected_text(format!("{:?}", o.tone_range))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut o.tone_range, ToneRange::Shadows, "Shadows");
                        ui.selectable_value(&mut o.tone_range, ToneRange::Midtones, "Midtones");
                        ui.selectable_value(&mut o.tone_range, ToneRange::Highlights, "Highlights");
                    });
            }
            Tool::Sponge => {
                brush_common(ui, &mut o.retouch, tool);
                ui.selectable_value(&mut o.sponge_saturate, false, "Desaturate");
                ui.selectable_value(&mut o.sponge_saturate, true, "Saturate");
            }
            Tool::MarqueeRect | Tool::MarqueeEllipse | Tool::Lasso | Tool::PolyLasso => {
                select_op_ui(ui, o);
                ui.label("Feather");
                ui.add(
                    egui::DragValue::new(&mut o.feather)
                        .range(0.0..=250.0)
                        .suffix(" px"),
                );
                ui.checkbox(&mut o.antialias, "Anti-alias");
                if ui.button("Select Subject").clicked() {
                    actions.push(Cmd::SelectSubject);
                }
            }
            Tool::MagicWand => {
                select_op_ui(ui, o);
                ui.label("Tolerance");
                ui.add(egui::DragValue::new(&mut o.wand_tolerance).range(0.0..=255.0));
                ui.checkbox(&mut o.antialias, "Anti-alias");
                ui.checkbox(&mut o.wand_contiguous, "Contiguous");
                ui.checkbox(&mut o.wand_sample_all, "Sample all layers");
                if ui.button("Select Subject").clicked() {
                    actions.push(Cmd::SelectSubject);
                }
            }
            Tool::Crop => {
                egui::ComboBox::from_id_salt("crop_ratio")
                    .selected_text(match o.crop_ratio {
                        None => "Ratio: free".to_string(),
                        Some((a, b)) => format!("{a}:{b}"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut o.crop_ratio, None, "Free");
                        for r in [
                            (1.0, 1.0),
                            (4.0, 5.0),
                            (3.0, 2.0),
                            (4.0, 3.0),
                            (16.0, 9.0),
                            (9.0, 16.0),
                            (2.0, 3.0),
                        ] {
                            ui.selectable_value(
                                &mut o.crop_ratio,
                                Some(r),
                                format!("{}:{}", r.0, r.1),
                            );
                        }
                    });
                if ui.button("✔ Apply (Enter)").clicked() {
                    actions.push(Cmd::ApplyCrop);
                }
                if ui.button("✖ Cancel").clicked() {
                    actions.push(Cmd::Cancel);
                }
            }
            Tool::Eyedropper => {
                ui.checkbox(&mut o.eyedropper_sample_all, "Sample all layers");
            }
            Tool::Gradient => {
                ui.checkbox(&mut o.gradient_use_fg_bg, "Foreground → Background");
                if !o.gradient_use_fg_bg {
                    ui.menu_button("Edit gradient", |ui| {
                        gradient_editor(ui, &mut o.gradient.gradient, "tool_grad");
                    });
                }
                for s in GradientShape::ALL {
                    ui.selectable_value(&mut o.gradient.shape, s, s.name());
                }
                blend_combo(ui, "grad_mode", &mut o.gradient.mode);
                ui.label("Opacity");
                let mut op = o.gradient.opacity * 100.0;
                if ui
                    .add(egui::DragValue::new(&mut op).range(1.0..=100.0).suffix("%"))
                    .changed()
                {
                    o.gradient.opacity = op / 100.0;
                }
                ui.checkbox(&mut o.gradient.reverse, "Reverse");
                ui.checkbox(&mut o.gradient.dither, "Dither");
            }
            Tool::Bucket => {
                ui.label("Tolerance");
                ui.add(egui::DragValue::new(&mut o.bucket_tolerance).range(0.0..=255.0));
                ui.checkbox(&mut o.bucket_contiguous, "Contiguous");
                ui.label("Opacity");
                let mut op = o.bucket_opacity * 100.0;
                if ui
                    .add(egui::DragValue::new(&mut op).range(1.0..=100.0).suffix("%"))
                    .changed()
                {
                    o.bucket_opacity = op / 100.0;
                }
            }
            Tool::Text => {
                text_options(ui, &mut o.text);
            }
            Tool::Shape => {
                for k in ShapeKind::defaults() {
                    let selected = std::mem::discriminant(&o.shape) == std::mem::discriminant(&k);
                    if ui.selectable_label(selected, k.name()).clicked() && !selected {
                        o.shape = k;
                    }
                }
                match &mut o.shape {
                    ShapeKind::RoundedRect { radius } => {
                        ui.label("Radius");
                        ui.add(egui::DragValue::new(radius).range(0.0..=2000.0));
                    }
                    ShapeKind::Polygon { sides } => {
                        ui.label("Sides");
                        ui.add(egui::DragValue::new(sides).range(3..=64));
                    }
                    ShapeKind::Star { points, inner } => {
                        ui.label("Points");
                        ui.add(egui::DragValue::new(points).range(3..=64));
                        ui.label("Indent");
                        ui.add(egui::DragValue::new(inner).range(0.05..=0.95).speed(0.01));
                    }
                    ShapeKind::Line {
                        width,
                        arrow_end,
                        arrow_start,
                    } => {
                        ui.label("Weight");
                        ui.add(egui::DragValue::new(width).range(1.0..=500.0));
                        ui.checkbox(arrow_start, "Arrow start");
                        ui.checkbox(arrow_end, "Arrow end");
                    }
                    _ => {}
                }
                ui.separator();
                let mut has_fill = o.shape_style.fill.is_some();
                if ui.checkbox(&mut has_fill, "Fill").changed() {
                    o.shape_style.fill = if has_fill {
                        Some(kroma_core::Color::BLACK)
                    } else {
                        None
                    };
                }
                if let Some(c) = &mut o.shape_style.fill {
                    color_button(ui, c);
                }
                let mut has_stroke = o.shape_style.stroke.is_some();
                if ui.checkbox(&mut has_stroke, "Stroke").changed() {
                    o.shape_style.stroke = if has_stroke {
                        Some(kroma_core::Color::BLACK)
                    } else {
                        None
                    };
                }
                if let Some(c) = &mut o.shape_style.stroke {
                    color_button(ui, c);
                    ui.add(
                        egui::DragValue::new(&mut o.shape_style.stroke_width)
                            .range(1.0..=200.0)
                            .suffix(" px"),
                    );
                }
                ui.checkbox(&mut o.shape_new_layer, "New layer per shape");
            }
            Tool::Liquify => {
                for (m, n) in [
                    (LiquifyMode::ForwardWarp, "Forward Warp"),
                    (LiquifyMode::Pucker, "Pucker"),
                    (LiquifyMode::Bloat, "Bloat"),
                    (LiquifyMode::TwirlClockwise, "Twirl"),
                    (LiquifyMode::Reconstruct, "Reconstruct"),
                ] {
                    ui.selectable_value(&mut o.liquify_mode, m, n);
                }
                ui.label("Size");
                ui.add(egui::DragValue::new(&mut o.liquify_size).range(5.0..=1500.0));
                ui.label("Pressure");
                ui.add(
                    egui::DragValue::new(&mut o.liquify_pressure)
                        .range(0.01..=1.0)
                        .speed(0.01),
                );
                if ui.button("✔ Apply").clicked() {
                    actions.push(Cmd::Commit);
                }
                if ui.button("✖ Cancel").clicked() {
                    actions.push(Cmd::Cancel);
                }
            }
            Tool::Move => {
                ui.checkbox(&mut o.move_auto_select, "Auto-select layer");
                ui.separator();
                ui.label("Align to canvas:");
                for (lbl, c) in [
                    ("⇤", Cmd::Align(0)),
                    ("↔", Cmd::Align(1)),
                    ("⇥", Cmd::Align(2)),
                    ("⬆", Cmd::Align(3)),
                    ("↕", Cmd::Align(4)),
                    ("⬇", Cmd::Align(5)),
                ] {
                    if ui.button(lbl).clicked() {
                        actions.push(c);
                    }
                }
            }
            Tool::Hand | Tool::Zoom => {
                if ui.button("100%").clicked() {
                    actions.push(Cmd::ActualPixels);
                }
                if ui.button("Fit Screen").clicked() {
                    actions.push(Cmd::FitScreen);
                }
            }
        }
    });
}

pub fn text_options(ui: &mut Ui, t: &mut TextStyle) -> bool {
    let mut ch = false;
    let fonts = system_fonts_cached();
    egui::ComboBox::from_id_salt("font")
        .selected_text(t.font.label())
        .width(170.0)
        .show_ui(ui, |ui| {
            ch |= ui
                .selectable_value(&mut t.font, FontChoice::Sans, FontChoice::Sans.label())
                .changed();
            ch |= ui
                .selectable_value(&mut t.font, FontChoice::Mono, FontChoice::Mono.label())
                .changed();
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    for f in fonts.iter() {
                        let fc = FontChoice::File(f.clone());
                        let label = fc.label();
                        ch |= ui.selectable_value(&mut t.font, fc, label).changed();
                    }
                });
        });
    ui.label("Size");
    ch |= ui
        .add(
            egui::DragValue::new(&mut t.size)
                .range(2.0..=2000.0)
                .suffix(" px"),
        )
        .changed();
    ch |= color_button(ui, &mut t.color);
    for (a, n) in [
        (TextAlign::Left, "⬅"),
        (TextAlign::Center, "⬌"),
        (TextAlign::Right, "➡"),
    ] {
        ch |= ui.selectable_value(&mut t.align, a, n).changed();
    }
    ch |= ui
        .toggle_value(&mut t.faux_bold, egui::RichText::new("B").strong())
        .changed();
    ch |= ui
        .toggle_value(&mut t.faux_italic, egui::RichText::new("I").italics())
        .changed();
    ch |= ui
        .toggle_value(&mut t.underline, egui::RichText::new("U").underline())
        .changed();
    ui.label("Tracking");
    ch |= ui
        .add(egui::DragValue::new(&mut t.tracking).range(-50.0..=200.0))
        .changed();
    ui.label("Leading");
    ch |= ui
        .add(
            egui::DragValue::new(&mut t.leading)
                .range(0.5..=4.0)
                .speed(0.01),
        )
        .changed();
    ch
}

fn system_fonts_cached() -> &'static Vec<String> {
    static FONTS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    FONTS.get_or_init(kroma_core::text::system_fonts)
}
