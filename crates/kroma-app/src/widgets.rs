//! Reusable editor widgets: color buttons, parameter editors for adjustments / filters /
//! layer styles, the Curves editor, gradient editor and histogram view.

use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use kroma_core::adjust::{
    Adjustment, CurvesChannel, Histogram, LevelsChannel, BW_WEIGHT_NAMES, SELECTIVE_COLOR_RANGES,
};
use kroma_core::blend::BlendMode;
use kroma_core::brush::StrokeLocation;
use kroma_core::effects::Effect;
use kroma_core::filter::{Filter, NoiseKind, RadialKind};
use kroma_core::gradient::{Gradient, GradientShape, GradientStop};
use kroma_core::Color;

pub fn c32(c: Color) -> Color32 {
    let [r, g, b, a] = c.to_rgba8();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// sRGB color button with alpha.
pub fn color_button(ui: &mut Ui, c: &mut Color) -> bool {
    let mut v = c.to_rgba8();
    let r = ui.color_edit_button_srgba_unmultiplied(&mut v);
    if r.changed() {
        *c = Color::from_rgba8(v[0], v[1], v[2], v[3]);
        true
    } else {
        false
    }
}

pub fn blend_combo(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    mode: &mut BlendMode,
) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(mode.name())
        .width(130.0)
        .show_ui(ui, |ui| {
            for m in BlendMode::MENU {
                match m {
                    Some(m) => {
                        if ui.selectable_label(*mode == *m, m.name()).clicked() {
                            *mode = *m;
                            changed = true;
                        }
                    }
                    None => {
                        ui.separator();
                    }
                }
            }
        });
    changed
}

fn slider(ui: &mut Ui, label: &str, v: &mut f32, range: std::ops::RangeInclusive<f32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(v, range).clamping(egui::SliderClamping::Always))
            .changed()
    })
    .inner
}

fn pct(ui: &mut Ui, label: &str, v: &mut f32) -> bool {
    let mut p = *v * 100.0;
    let ch = ui
        .horizontal(|ui| {
            ui.label(label);
            ui.add(
                egui::Slider::new(&mut p, 0.0..=100.0)
                    .suffix("%")
                    .clamping(egui::SliderClamping::Always),
            )
            .changed()
        })
        .inner;
    if ch {
        *v = p / 100.0;
    }
    ch
}

fn signed_pct(ui: &mut Ui, label: &str, v: &mut f32, range: f32) -> bool {
    let mut p = *v * 100.0;
    let ch = ui
        .horizontal(|ui| {
            ui.label(label);
            ui.add(
                egui::Slider::new(&mut p, -range * 100.0..=range * 100.0)
                    .clamping(egui::SliderClamping::Always),
            )
            .changed()
        })
        .inner;
    if ch {
        *v = p / 100.0;
    }
    ch
}

fn u32_slider(ui: &mut Ui, label: &str, v: &mut u32, range: std::ops::RangeInclusive<u32>) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::Slider::new(v, range)).changed()
    })
    .inner
}

// ------------------------------------------------------------------ histogram

pub fn histogram_view(ui: &mut Ui, h: &Histogram, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    if h.count == 0 {
        return;
    }
    let max =
        h.l.iter()
            .chain(h.r.iter())
            .chain(h.g.iter())
            .chain(h.b.iter())
            .copied()
            .max()
            .unwrap_or(1)
            .max(1) as f32;
    let chans: [(&Vec<u32>, Color32); 4] = [
        (&h.r, Color32::from_rgba_unmultiplied(230, 60, 60, 90)),
        (&h.g, Color32::from_rgba_unmultiplied(60, 200, 80, 90)),
        (&h.b, Color32::from_rgba_unmultiplied(70, 110, 240, 90)),
        (&h.l, Color32::from_rgba_unmultiplied(220, 220, 220, 110)),
    ];
    for (bins, col) in chans {
        let pts: Vec<Pos2> = (0..256)
            .map(|i| {
                let x = rect.left() + rect.width() * i as f32 / 255.0;
                let v = (bins[i] as f32 / max).sqrt();
                Pos2::new(x, rect.bottom() - v * rect.height())
            })
            .collect();
        let mut poly = vec![Pos2::new(rect.left(), rect.bottom())];
        poly.extend(pts);
        poly.push(Pos2::new(rect.right(), rect.bottom()));
        for w in poly.windows(2) {
            p.line_segment([w[0], w[1]], Stroke::new(1.0, col));
        }
        // filled columns
        for i in 0..256 {
            let x = rect.left() + rect.width() * i as f32 / 255.0;
            let v = (bins[i] as f32 / max).sqrt();
            p.line_segment(
                [
                    Pos2::new(x, rect.bottom()),
                    Pos2::new(x, rect.bottom() - v * rect.height()),
                ],
                Stroke::new(rect.width() / 256.0 + 0.5, col.gamma_multiply(0.6)),
            );
        }
    }
}

// ------------------------------------------------------------------ curves

/// Interactive curve editor. Click to add a point, drag to move, drag off the box to delete.
pub fn curve_editor(
    ui: &mut Ui,
    ch: &mut CurvesChannel,
    color: Color32,
    hist: Option<&[u32]>,
) -> bool {
    let size = ui.available_width().min(260.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let p = ui.painter_at(rect.expand(4.0));
    p.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    if let Some(h) = hist {
        let max = h.iter().copied().max().unwrap_or(1).max(1) as f32;
        for (i, v) in h.iter().enumerate() {
            let x = rect.left() + rect.width() * i as f32 / 255.0;
            let hh = (*v as f32 / max).sqrt() * rect.height();
            p.line_segment(
                [
                    Pos2::new(x, rect.bottom()),
                    Pos2::new(x, rect.bottom() - hh),
                ],
                Stroke::new(1.0, Color32::from_gray(55)),
            );
        }
    }
    for i in 1..4 {
        let t = i as f32 / 4.0;
        let g = Stroke::new(1.0, Color32::from_gray(70));
        p.line_segment(
            [
                Pos2::new(rect.left() + rect.width() * t, rect.top()),
                Pos2::new(rect.left() + rect.width() * t, rect.bottom()),
            ],
            g,
        );
        p.line_segment(
            [
                Pos2::new(rect.left(), rect.top() + rect.height() * t),
                Pos2::new(rect.right(), rect.top() + rect.height() * t),
            ],
            g,
        );
    }
    p.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(1.0, Color32::from_gray(90)),
    );
    let to_screen = |(x, y): (f32, f32)| {
        Pos2::new(
            rect.left() + x * rect.width(),
            rect.bottom() - y * rect.height(),
        )
    };
    let from_screen = |pos: Pos2| {
        (
            ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
            ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0),
        )
    };
    let lut = ch.lut(256);
    let pts: Vec<Pos2> = lut
        .iter()
        .enumerate()
        .map(|(i, v)| to_screen((i as f32 / 255.0, *v)))
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(2.0, color)));
    let mut changed = false;
    let id = resp.id.with("drag_idx");
    let mut dragging: Option<usize> = ui.data(|d| d.get_temp(id));
    if resp.drag_started() || resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let near = ch
                .points
                .iter()
                .position(|pt| to_screen(*pt).distance(pos) < 8.0);
            dragging = Some(match near {
                Some(i) => i,
                None => {
                    ch.points.push(from_screen(pos));
                    changed = true;
                    ch.points.len() - 1
                }
            });
        }
    }
    if resp.dragged() {
        if let (Some(i), Some(pos)) = (dragging, resp.interact_pointer_pos()) {
            if i < ch.points.len() {
                let (x, y) = from_screen(pos);
                let outside = !rect.expand(20.0).contains(pos);
                if outside && ch.points.len() > 2 {
                    ch.points.remove(i);
                    dragging = None;
                } else {
                    ch.points[i] = (x, y);
                }
                changed = true;
            }
        }
    }
    if resp.drag_stopped() {
        dragging = None;
        ch.points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    }
    ui.data_mut(|d| d.insert_temp(id, dragging));
    for pt in &ch.points {
        p.circle(
            to_screen(*pt),
            4.0,
            Color32::from_gray(20),
            Stroke::new(1.5, color),
        );
    }
    if ui.small_button("Reset").clicked() {
        *ch = CurvesChannel::default();
        changed = true;
    }
    changed
}

fn levels_ui(ui: &mut Ui, label: &str, l: &mut LevelsChannel) -> bool {
    let mut ch = false;
    ui.label(egui::RichText::new(label).strong());
    ch |= slider(ui, "Input black", &mut l.in_black, 0.0..=1.0);
    ch |= slider(ui, "Gamma", &mut l.gamma, 0.1..=9.99);
    ch |= slider(ui, "Input white", &mut l.in_white, 0.0..=1.0);
    ch |= slider(ui, "Output black", &mut l.out_black, 0.0..=1.0);
    ch |= slider(ui, "Output white", &mut l.out_white, 0.0..=1.0);
    if l.in_white <= l.in_black + 0.01 {
        l.in_white = (l.in_black + 0.01).min(1.0);
    }
    ch
}

pub fn gradient_editor(ui: &mut Ui, g: &mut Gradient, id: &str) -> bool {
    let mut changed = false;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().min(260.0), 18.0),
        Sense::hover(),
    );
    let p = ui.painter_at(rect);
    let n = 64;
    for i in 0..n {
        let t0 = i as f32 / n as f32;
        let r = Rect::from_min_max(
            Pos2::new(rect.left() + rect.width() * t0, rect.top()),
            Pos2::new(
                rect.left() + rect.width() * (i + 1) as f32 / n as f32 + 1.0,
                rect.bottom(),
            ),
        );
        p.rect_filled(r, 0.0, c32(g.sample(t0 + 0.5 / n as f32)));
    }
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt((id, "preset"))
            .selected_text("Presets")
            .show_ui(ui, |ui| {
                for (name, pg) in Gradient::presets() {
                    if ui.button(name).clicked() {
                        *g = pg;
                        changed = true;
                    }
                }
            });
        if ui.small_button("Reverse").clicked() {
            *g = g.reversed();
            changed = true;
        }
        if ui.small_button("+ Stop").clicked() {
            let c = g.sample(0.5);
            g.stops.push(GradientStop { pos: 0.5, color: c });
            changed = true;
        }
    });
    let mut remove = None;
    let nstops = g.stops.len();
    for (i, s) in g.stops.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            changed |= color_button(ui, &mut s.color);
            changed |= ui
                .add(egui::Slider::new(&mut s.pos, 0.0..=1.0).show_value(false))
                .changed();
            if nstops > 2 && ui.small_button("×").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        g.stops.remove(i);
        changed = true;
    }
    if changed {
        g.stops.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap());
    }
    changed
}

/// Editor for every adjustment's parameters. Returns true if anything changed.
pub fn adjustment_ui(ui: &mut Ui, adj: &mut Adjustment, hist: Option<&Histogram>) -> bool {
    let mut ch = false;
    match adj {
        Adjustment::BrightnessContrast {
            brightness,
            contrast,
            legacy,
        } => {
            ch |= signed_pct(ui, "Brightness", brightness, 1.0);
            ch |= signed_pct(ui, "Contrast", contrast, 1.0);
            ch |= ui.checkbox(legacy, "Use Legacy").changed();
        }
        Adjustment::Levels {
            master,
            red,
            green,
            blue,
        } => {
            if let Some(h) = hist {
                histogram_view(ui, h, 70.0);
            }
            let id = ui.id().with("levels_tab");
            let mut tab: usize = ui.data(|d| d.get_temp(id).unwrap_or(0));
            ui.horizontal(|ui| {
                for (i, n) in ["RGB", "Red", "Green", "Blue"].iter().enumerate() {
                    ui.selectable_value(&mut tab, i, *n);
                }
            });
            ui.data_mut(|d| d.insert_temp(id, tab));
            ch |= match tab {
                0 => levels_ui(ui, "RGB", master),
                1 => levels_ui(ui, "Red", red),
                2 => levels_ui(ui, "Green", green),
                _ => levels_ui(ui, "Blue", blue),
            };
        }
        Adjustment::Curves {
            master,
            red,
            green,
            blue,
        } => {
            let id = ui.id().with("curves_tab");
            let mut tab: usize = ui.data(|d| d.get_temp(id).unwrap_or(0));
            ui.horizontal(|ui| {
                for (i, n) in ["RGB", "Red", "Green", "Blue"].iter().enumerate() {
                    ui.selectable_value(&mut tab, i, *n);
                }
            });
            ui.data_mut(|d| d.insert_temp(id, tab));
            let (c, col, hb) = match tab {
                0 => (master, Color32::from_gray(230), hist.map(|h| &h.l[..])),
                1 => (red, Color32::from_rgb(240, 80, 80), hist.map(|h| &h.r[..])),
                2 => (
                    green,
                    Color32::from_rgb(80, 220, 100),
                    hist.map(|h| &h.g[..]),
                ),
                _ => (
                    blue,
                    Color32::from_rgb(90, 140, 255),
                    hist.map(|h| &h.b[..]),
                ),
            };
            ch |= curve_editor(ui, c, col, hb);
            ui.horizontal(|ui| {
                if ui.small_button("Lighter").clicked() {
                    c.points = vec![(0.0, 0.0), (0.5, 0.62), (1.0, 1.0)];
                    ch = true;
                }
                if ui.small_button("Darker").clicked() {
                    c.points = vec![(0.0, 0.0), (0.5, 0.38), (1.0, 1.0)];
                    ch = true;
                }
                if ui.small_button("S-Contrast").clicked() {
                    c.points = vec![(0.0, 0.0), (0.25, 0.18), (0.75, 0.82), (1.0, 1.0)];
                    ch = true;
                }
                if ui.small_button("Negative").clicked() {
                    c.points = vec![(0.0, 1.0), (1.0, 0.0)];
                    ch = true;
                }
            });
        }
        Adjustment::Exposure {
            exposure,
            offset,
            gamma,
        } => {
            ch |= slider(ui, "Exposure (stops)", exposure, -5.0..=5.0);
            ch |= slider(ui, "Offset", offset, -0.5..=0.5);
            ch |= slider(ui, "Gamma", gamma, 0.1..=4.0);
        }
        Adjustment::Vibrance {
            vibrance,
            saturation,
        } => {
            ch |= signed_pct(ui, "Vibrance", vibrance, 1.0);
            ch |= signed_pct(ui, "Saturation", saturation, 1.0);
        }
        Adjustment::HueSaturation {
            hue,
            saturation,
            lightness,
            colorize,
        } => {
            ch |= slider(ui, "Hue", hue, -180.0..=180.0);
            ch |= signed_pct(ui, "Saturation", saturation, 1.0);
            ch |= signed_pct(ui, "Lightness", lightness, 1.0);
            ch |= ui.checkbox(colorize, "Colorize").changed();
        }
        Adjustment::ColorBalance {
            shadows,
            midtones,
            highlights,
            preserve_luminosity,
        } => {
            for (name, v) in [
                ("Shadows", shadows),
                ("Midtones", midtones),
                ("Highlights", highlights),
            ] {
                ui.label(egui::RichText::new(name).strong());
                ch |= signed_pct(ui, "Cyan ↔ Red", &mut v[0], 1.0);
                ch |= signed_pct(ui, "Magenta ↔ Green", &mut v[1], 1.0);
                ch |= signed_pct(ui, "Yellow ↔ Blue", &mut v[2], 1.0);
            }
            ch |= ui
                .checkbox(preserve_luminosity, "Preserve Luminosity")
                .changed();
        }
        Adjustment::BlackWhite { weights, tint } => {
            for (i, n) in BW_WEIGHT_NAMES.iter().enumerate() {
                ch |= slider(ui, n, &mut weights[i], -2.0..=3.0);
            }
            let mut has = tint.is_some();
            if ui.checkbox(&mut has, "Tint").changed() {
                *tint = if has {
                    Some(Color::from_hex("#e1d3b3").unwrap())
                } else {
                    None
                };
                ch = true;
            }
            if let Some(t) = tint {
                ch |= color_button(ui, t);
            }
            if ui.small_button("Auto").clicked() {
                *weights = [0.4, 0.6, 0.4, 0.6, 0.2, 0.8];
                ch = true;
            }
        }
        Adjustment::PhotoFilter {
            color,
            density,
            preserve_luminosity,
        } => {
            ui.horizontal(|ui| {
                ui.label("Filter color");
                ch |= color_button(ui, color);
                for (n, h) in [
                    ("Warming 85", "#ec8a00"),
                    ("Cooling 80", "#006dff"),
                    ("Sepia", "#ac7a33"),
                    ("Underwater", "#00c2b1"),
                ] {
                    if ui.small_button(n).clicked() {
                        *color = Color::from_hex(h).unwrap();
                        ch = true;
                    }
                }
            });
            ch |= pct(ui, "Density", density);
            ch |= ui
                .checkbox(preserve_luminosity, "Preserve Luminosity")
                .changed();
        }
        Adjustment::ChannelMixer {
            red,
            green,
            blue,
            monochrome,
        } => {
            ch |= ui.checkbox(monochrome, "Monochrome").changed();
            let rows: Vec<(&str, &mut [f32; 4])> = if *monochrome {
                vec![("Gray", red)]
            } else {
                vec![("Red out", red), ("Green out", green), ("Blue out", blue)]
            };
            for (n, row) in rows {
                ui.label(egui::RichText::new(n).strong());
                ch |= signed_pct(ui, "Red", &mut row[0], 2.0);
                ch |= signed_pct(ui, "Green", &mut row[1], 2.0);
                ch |= signed_pct(ui, "Blue", &mut row[2], 2.0);
                ch |= signed_pct(ui, "Constant", &mut row[3], 1.0);
            }
        }
        Adjustment::TemperatureTint { temperature, tint } => {
            ch |= signed_pct(ui, "Temperature", temperature, 1.0);
            ch |= signed_pct(ui, "Tint", tint, 1.0);
        }
        Adjustment::SelectiveColor { ranges, absolute } => {
            let id = ui.id().with("sc_range");
            let mut r: usize = ui.data(|d| d.get_temp(id).unwrap_or(0));
            egui::ComboBox::from_id_salt("sc_combo")
                .selected_text(SELECTIVE_COLOR_RANGES[r])
                .show_ui(ui, |ui| {
                    for (i, n) in SELECTIVE_COLOR_RANGES.iter().enumerate() {
                        ui.selectable_value(&mut r, i, *n);
                    }
                });
            ui.data_mut(|d| d.insert_temp(id, r));
            let c = &mut ranges[r];
            ch |= signed_pct(ui, "Cyan", &mut c.c, 1.0);
            ch |= signed_pct(ui, "Magenta", &mut c.m, 1.0);
            ch |= signed_pct(ui, "Yellow", &mut c.y, 1.0);
            ch |= signed_pct(ui, "Black", &mut c.k, 1.0);
            ui.horizontal(|ui| {
                ch |= ui.radio_value(absolute, false, "Relative").changed();
                ch |= ui.radio_value(absolute, true, "Absolute").changed();
            });
        }
        Adjustment::GradientMap { gradient, reverse } => {
            ch |= gradient_editor(ui, gradient, "gmap");
            ch |= ui.checkbox(reverse, "Reverse").changed();
        }
        Adjustment::Posterize { levels } => ch |= u32_slider(ui, "Levels", levels, 2..=64),
        Adjustment::Threshold { level } => {
            if let Some(h) = hist {
                histogram_view(ui, h, 60.0);
            }
            ch |= slider(ui, "Threshold level", level, 0.0..=1.0)
        }
        Adjustment::Sepia { amount } => ch |= pct(ui, "Amount", amount),
        Adjustment::Invert | Adjustment::Desaturate | Adjustment::Solarize => {
            ui.label("No settings.");
        }
    }
    ch
}

pub fn filter_ui(ui: &mut Ui, f: &mut Filter) -> bool {
    let mut ch = false;
    match f {
        Filter::GaussianBlur { radius } => ch |= slider(ui, "Radius (px)", radius, 0.1..=250.0),
        Filter::BoxBlur { radius } => ch |= u32_slider(ui, "Radius (px)", radius, 1..=200),
        Filter::MotionBlur { angle, distance } => {
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Distance", distance, 1.0..=500.0);
        }
        Filter::RadialBlur {
            amount,
            kind,
            cx,
            cy,
        } => {
            ch |= slider(ui, "Amount", amount, 0.0..=1.0);
            ui.horizontal(|ui| {
                ch |= ui.radio_value(kind, RadialKind::Zoom, "Zoom").changed();
                ch |= ui.radio_value(kind, RadialKind::Spin, "Spin").changed();
            });
            ch |= slider(ui, "Center X", cx, 0.0..=1.0);
            ch |= slider(ui, "Center Y", cy, 0.0..=1.0);
        }
        Filter::SurfaceBlur { radius, threshold } => {
            ch |= u32_slider(ui, "Radius", radius, 1..=40);
            ch |= slider(ui, "Threshold", threshold, 0.01..=1.0);
        }
        Filter::TiltShift {
            focus_y,
            band,
            blur,
        } => {
            ch |= slider(ui, "Focus line", focus_y, 0.0..=1.0);
            ch |= slider(ui, "Sharp band", band, 0.0..=0.5);
            ch |= slider(ui, "Blur", blur, 0.5..=60.0);
        }
        Filter::UnsharpMask {
            amount,
            radius,
            threshold,
        } => {
            ch |= slider(ui, "Amount", amount, 0.0..=5.0);
            ch |= slider(ui, "Radius", radius, 0.1..=100.0);
            ch |= slider(ui, "Threshold", threshold, 0.0..=0.5);
        }
        Filter::HighPass { radius } => ch |= slider(ui, "Radius", radius, 0.1..=200.0),
        Filter::Clarity { amount } => ch |= slider(ui, "Amount", amount, -1.0..=2.0),
        Filter::AddNoise {
            amount,
            kind,
            monochrome,
            seed,
        } => {
            ch |= slider(ui, "Amount", amount, 0.0..=1.0);
            ui.horizontal(|ui| {
                ch |= ui
                    .radio_value(kind, NoiseKind::Uniform, "Uniform")
                    .changed();
                ch |= ui
                    .radio_value(kind, NoiseKind::Gaussian, "Gaussian")
                    .changed();
            });
            ch |= ui.checkbox(monochrome, "Monochromatic").changed();
            ch |= u32_slider(ui, "Seed", seed, 0..=9999);
        }
        Filter::Median { radius } => ch |= u32_slider(ui, "Radius", radius, 1..=12),
        Filter::ReduceNoise { strength } => ch |= slider(ui, "Strength", strength, 0.0..=1.0),
        Filter::Emboss {
            angle,
            height,
            amount,
        } => {
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Height", height, 1.0..=10.0);
            ch |= slider(ui, "Amount", amount, 0.1..=5.0);
        }
        Filter::OilPaint { radius } => ch |= u32_slider(ui, "Radius", radius, 1..=12),
        Filter::GlowingEdges { width, brightness } => {
            ch |= slider(ui, "Edge width", width, 0.5..=10.0);
            ch |= slider(ui, "Brightness", brightness, 0.5..=10.0);
        }
        Filter::Twirl { angle } => ch |= slider(ui, "Angle", angle, -999.0..=999.0),
        Filter::Pinch { amount } => ch |= slider(ui, "Amount", amount, -1.0..=1.0),
        Filter::Spherize { amount } => ch |= slider(ui, "Amount", amount, -1.0..=1.0),
        Filter::Wave {
            amplitude,
            wavelength,
            horizontal,
            vertical,
        } => {
            ch |= slider(ui, "Amplitude", amplitude, 0.0..=100.0);
            ch |= slider(ui, "Wavelength", wavelength, 2.0..=500.0);
            ch |= ui.checkbox(horizontal, "Horizontal").changed();
            ch |= ui.checkbox(vertical, "Vertical").changed();
        }
        Filter::Ripple { amount, size } => {
            ch |= slider(ui, "Amount", amount, 0.0..=50.0);
            ch |= slider(ui, "Size", size, 2.0..=200.0);
        }
        Filter::PolarCoordinates { to_polar } => {
            ch |= ui
                .radio_value(to_polar, true, "Rectangular to Polar")
                .changed();
            ch |= ui
                .radio_value(to_polar, false, "Polar to Rectangular")
                .changed();
        }
        Filter::Mosaic { cell } => ch |= u32_slider(ui, "Cell size", cell, 2..=200),
        Filter::Crystallize { cell, seed } => {
            ch |= u32_slider(ui, "Cell size", cell, 3..=300);
            ch |= u32_slider(ui, "Seed", seed, 0..=9999);
        }
        Filter::ColorHalftone { max_radius } => {
            ch |= slider(ui, "Max radius", max_radius, 2.0..=40.0)
        }
        Filter::Clouds { scale, seed } => {
            ch |= slider(ui, "Scale", scale, 8.0..=1000.0);
            ch |= u32_slider(ui, "Seed", seed, 0..=9999);
            ui.label("Uses foreground → background colors.");
        }
        Filter::Vignette { amount, midpoint } => {
            ch |= slider(ui, "Amount (− = white)", amount, -1.0..=1.0);
            ch |= slider(ui, "Midpoint", midpoint, 0.0..=1.0);
        }
        Filter::LensFlare { cx, cy, brightness } => {
            ch |= slider(ui, "Center X", cx, 0.0..=1.0);
            ch |= slider(ui, "Center Y", cy, 0.0..=1.0);
            ch |= slider(ui, "Brightness", brightness, 0.1..=3.0);
        }
        Filter::Offset { dx, dy, wrap } => {
            ui.horizontal(|ui| {
                ui.label("Horizontal");
                ch |= ui.add(egui::DragValue::new(dx)).changed();
                ui.label("Vertical");
                ch |= ui.add(egui::DragValue::new(dy)).changed();
            });
            ch |= ui.checkbox(wrap, "Wrap around").changed();
        }
        Filter::Maximum { radius } | Filter::Minimum { radius } => {
            ch |= u32_slider(ui, "Radius", radius, 1..=50)
        }
        Filter::ChromaticAberration { amount } => {
            ch |= slider(ui, "Amount (px)", amount, 0.0..=40.0)
        }
        Filter::ShadowsHighlights {
            shadows,
            highlights,
            radius,
        } => {
            ch |= slider(ui, "Shadows", shadows, 0.0..=1.0);
            ch |= slider(ui, "Highlights", highlights, 0.0..=1.0);
            ch |= slider(ui, "Radius", radius, 1.0..=200.0);
        }
        Filter::Dehaze { amount } => ch |= slider(ui, "Amount", amount, 0.0..=1.0),
        Filter::Sharpen | Filter::Despeckle | Filter::FindEdges | Filter::Equalize => {
            ui.label("No settings.");
        }
    }
    ch
}

pub fn effect_ui(ui: &mut Ui, e: &mut Effect) -> bool {
    let mut ch = false;
    let id = e.name();
    match e {
        Effect::DropShadow {
            color,
            opacity,
            angle,
            distance,
            size,
            spread,
            mode,
            ..
        } => {
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Distance", distance, 0.0..=200.0);
            ch |= pct(ui, "Spread", spread);
            ch |= slider(ui, "Size", size, 0.0..=150.0);
        }
        Effect::InnerShadow {
            color,
            opacity,
            angle,
            distance,
            size,
            choke,
            mode,
            ..
        } => {
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Distance", distance, 0.0..=200.0);
            ch |= pct(ui, "Choke", choke);
            ch |= slider(ui, "Size", size, 0.0..=150.0);
        }
        Effect::OuterGlow {
            color,
            opacity,
            size,
            spread,
            mode,
            ..
        } => {
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
            ch |= pct(ui, "Spread", spread);
            ch |= slider(ui, "Size", size, 0.0..=150.0);
        }
        Effect::InnerGlow {
            color,
            opacity,
            size,
            choke,
            from_center,
            mode,
            ..
        } => {
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
            ch |= pct(ui, "Choke", choke);
            ch |= slider(ui, "Size", size, 0.0..=150.0);
            ui.horizontal(|ui| {
                ch |= ui.radio_value(from_center, false, "Edge").changed();
                ch |= ui.radio_value(from_center, true, "Center").changed();
            });
        }
        Effect::BevelEmboss {
            depth,
            size,
            angle,
            altitude,
            highlight,
            highlight_opacity,
            shadow,
            shadow_opacity,
            soften,
            ..
        } => {
            ch |= slider(ui, "Depth", depth, 0.1..=10.0);
            ch |= slider(ui, "Size", size, 1.0..=150.0);
            ch |= slider(ui, "Soften", soften, 0.0..=16.0);
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Altitude", altitude, 0.0..=90.0);
            ui.horizontal(|ui| {
                ui.label("Highlight");
                ch |= color_button(ui, highlight);
            });
            ch |= pct(ui, "Highlight opacity", highlight_opacity);
            ui.horizontal(|ui| {
                ui.label("Shadow");
                ch |= color_button(ui, shadow);
            });
            ch |= pct(ui, "Shadow opacity", shadow_opacity);
        }
        Effect::Satin {
            color,
            opacity,
            angle,
            distance,
            size,
            invert,
            ..
        } => {
            ch |= color_button(ui, color);
            ch |= pct(ui, "Opacity", opacity);
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
            ch |= slider(ui, "Distance", distance, 0.0..=250.0);
            ch |= slider(ui, "Size", size, 0.0..=250.0);
            ch |= ui.checkbox(invert, "Invert").changed();
        }
        Effect::ColorOverlay {
            color,
            opacity,
            mode,
            ..
        } => {
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
        }
        Effect::GradientOverlay {
            gradient,
            opacity,
            angle,
            shape,
            reverse,
            mode,
            ..
        } => {
            ch |= blend_combo(ui, (id, "m"), mode);
            ch |= pct(ui, "Opacity", opacity);
            ch |= gradient_editor(ui, gradient, "fx_grad");
            ch |= ui.checkbox(reverse, "Reverse").changed();
            egui::ComboBox::from_id_salt((id, "shape"))
                .selected_text(shape.name())
                .show_ui(ui, |ui| {
                    for s in GradientShape::ALL {
                        ch |= ui.selectable_value(shape, s, s.name()).changed();
                    }
                });
            ch |= slider(ui, "Angle", angle, -180.0..=180.0);
        }
        Effect::Stroke {
            color,
            size,
            position,
            opacity,
            mode,
            ..
        } => {
            ch |= slider(ui, "Size", size, 1.0..=250.0);
            ui.horizontal(|ui| {
                ui.label("Position");
                ch |= ui
                    .radio_value(position, StrokeLocation::Outside, "Outside")
                    .changed();
                ch |= ui
                    .radio_value(position, StrokeLocation::Inside, "Inside")
                    .changed();
                ch |= ui
                    .radio_value(position, StrokeLocation::Center, "Center")
                    .changed();
            });
            ui.horizontal(|ui| {
                ch |= blend_combo(ui, (id, "m"), mode);
                ch |= color_button(ui, color);
            });
            ch |= pct(ui, "Opacity", opacity);
        }
    }
    ch
}
