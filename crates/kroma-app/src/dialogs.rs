//! Modal-ish dialogs: New, Image/Canvas Size, adjustments & filters with live preview,
//! Layer Style, Fill, Stroke, Color Range, Modify Selection, Export, Batch, and friends.

use crate::app::{BatchJob, Cmd, KromaApp};
use crate::widgets::{adjustment_ui, blend_combo, color_button, effect_ui, filter_ui};
use egui::{Context, RichText, Ui};
use kroma_core::adjust::{Adjustment, Histogram};
use kroma_core::blend::BlendMode;
use kroma_core::brush::StrokeLocation;
use kroma_core::effects::Effect;
use kroma_core::filter::Filter;
use kroma_core::layer::{LayerId, LayerKind};
use kroma_core::ops::Op;
use kroma_core::transform::{Anchor, Resample};
use kroma_core::{Color, Document};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub enum Dialog {
    New {
        name: String,
        w: u32,
        h: u32,
        dpi: f32,
        bg: u8,
    },
    ImageSize {
        w: u32,
        h: u32,
        keep: bool,
        method: Resample,
        ow: u32,
        oh: u32,
    },
    CanvasSize {
        w: u32,
        h: u32,
        anchor: Anchor,
    },
    Rotate {
        degrees: f32,
    },
    Adjust {
        adj: Adjustment,
        hist: Histogram,
        live: bool,
        changed: bool,
    },
    Filter {
        filter: Filter,
        live: bool,
        changed: bool,
    },
    LayerStyle {
        layer: LayerId,
        original: Vec<Effect>,
        selected: usize,
    },
    Fill {
        source: u8,
        color: Color,
        opacity: f32,
        mode: BlendMode,
        preserve: bool,
    },
    Stroke {
        width: f32,
        color: Color,
        location: StrokeLocation,
    },
    ColorRange {
        color: Color,
        fuzziness: f32,
        mode: u8,
    },
    ModifySelection {
        kind: u8,
        amount: f32,
    },
    Export {
        ext: usize,
        quality: u8,
        scale: f32,
        sixteen: bool,
    },
    Batch {
        action: usize,
        files: Vec<PathBuf>,
        out_dir: Option<PathBuf>,
        ext: usize,
        quality: u8,
    },
    Rename {
        id: LayerId,
        name: String,
    },
    TextEdit {
        layer: LayerId,
        select_all: bool,
    },
    ColorPicker {
        background: bool,
        hsva: egui::ecolor::Hsva,
    },
    ConfirmClose {
        tab: usize,
        quitting: bool,
    },
    Recovery {
        entries: Vec<crate::recovery::RecoveryEntry>,
    },
    Shortcuts,
    About,
    Preferences,
}

impl Dialog {
    pub fn new_doc(clip: Option<(u32, u32)>) -> Dialog {
        let (w, h) = clip.unwrap_or((1920, 1080));
        Dialog::New {
            name: "Untitled".into(),
            w,
            h,
            dpi: 72.0,
            bg: 0,
        }
    }
}

enum Outcome {
    Keep,
    Close,
}

fn ok_cancel(ui: &mut Ui) -> (bool, bool) {
    ui.separator();
    ui.horizontal(|ui| {
        let ok = ui
            .add(egui::Button::new(RichText::new("   OK   ").strong()))
            .clicked();
        let cancel = ui.button("Cancel").clicked();
        (ok, cancel)
    })
    .inner
}

fn window(title: &str) -> egui::Window<'_> {
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .default_pos(egui::pos2(380.0, 120.0))
}

pub fn show(app: &mut KromaApp, ctx: &Context) {
    let Some(mut dlg) = app.dialog.take() else {
        return;
    };
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !ctx.egui_wants_keyboard_input();
    let pointer_down = ctx.input(|i| i.pointer.any_down());
    let outcome = match &mut dlg {
        Dialog::New {
            name,
            w,
            h,
            dpi,
            bg,
        } => {
            let mut out = Outcome::Keep;
            window("New Document").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(name);
                });
                ui.label(RichText::new("Presets").strong());
                ui.horizontal_wrapped(|ui| {
                    for (n, pw, ph, pd) in [
                        ("Default", 1600, 1000, 72.0),
                        ("HD 1080p", 1920, 1080, 72.0),
                        ("4K UHD", 3840, 2160, 72.0),
                        ("A4 300 dpi", 2480, 3508, 300.0),
                        ("Letter 300 dpi", 2550, 3300, 300.0),
                        ("Instagram Post", 1080, 1080, 72.0),
                        ("Story / Reel", 1080, 1920, 72.0),
                        ("YouTube Thumbnail", 1280, 720, 72.0),
                        ("Web Banner", 1200, 628, 72.0),
                        ("Icon 1024", 1024, 1024, 72.0),
                    ] {
                        if ui.button(n).clicked() {
                            *w = pw;
                            *h = ph;
                            *dpi = pd;
                        }
                    }
                });
                egui::Grid::new("newdoc").show(ui, |ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(w).range(1..=30000).suffix(" px"));
                    ui.end_row();
                    ui.label("Height");
                    ui.add(egui::DragValue::new(h).range(1..=30000).suffix(" px"));
                    ui.end_row();
                    ui.label("Resolution");
                    ui.add(egui::DragValue::new(dpi).range(1.0..=2400.0).suffix(" ppi"));
                    ui.end_row();
                    ui.label("Background");
                    ui.horizontal(|ui| {
                        ui.radio_value(bg, 0, "White");
                        ui.radio_value(bg, 1, "Black");
                        ui.radio_value(bg, 2, "Transparent");
                        ui.radio_value(bg, 3, "Background color");
                    });
                    ui.end_row();
                });
                let mb = (*w as f64 * *h as f64 * 16.0) / 1_048_576.0;
                ui.label(
                    RichText::new(format!("Memory per layer ≈ {mb:.0} MB (32-bit float RGBA)"))
                        .weak()
                        .small(),
                );
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let bgc = match bg {
                        0 => Some(Color::WHITE),
                        1 => Some(Color::BLACK),
                        3 => Some(app.bg),
                        _ => None,
                    };
                    let mut d = Document::new(*w, *h, bgc);
                    d.dpi = *dpi;
                    d.name = name.clone();
                    app.open_doc(d, None);
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::ImageSize {
            w,
            h,
            keep,
            method,
            ow,
            oh,
        } => {
            let mut out = Outcome::Keep;
            window("Image Size").show(ctx, |ui| {
                ui.label(format!("Current: {} × {} px", ow, oh));
                let mut pct = *w as f32 / *ow as f32 * 100.0;
                egui::Grid::new("isz").show(ui, |ui| {
                    ui.label("Width");
                    if ui
                        .add(egui::DragValue::new(w).range(1..=30000).suffix(" px"))
                        .changed()
                        && *keep
                    {
                        *h = ((*w as f32 * *oh as f32 / *ow as f32).round() as u32).max(1);
                    }
                    ui.end_row();
                    ui.label("Height");
                    if ui
                        .add(egui::DragValue::new(h).range(1..=30000).suffix(" px"))
                        .changed()
                        && *keep
                    {
                        *w = ((*h as f32 * *ow as f32 / *oh as f32).round() as u32).max(1);
                    }
                    ui.end_row();
                    ui.label("Percent");
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .range(1.0..=1000.0)
                                .suffix(" %"),
                        )
                        .changed()
                    {
                        *w = ((*ow as f32 * pct / 100.0).round() as u32).max(1);
                        *h = ((*oh as f32 * pct / 100.0).round() as u32).max(1);
                    }
                    ui.end_row();
                });
                ui.checkbox(keep, "Constrain proportions");
                egui::ComboBox::from_label("Resample")
                    .selected_text(method.name())
                    .show_ui(ui, |ui| {
                        for m in Resample::ALL {
                            ui.selectable_value(method, m, m.name());
                        }
                    });
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let (nw, nh, m) = (*w, *h, *method);
                    let t = app.tab().unwrap();
                    t.doc.resize_image(nw, nh, m);
                    t.needs_fit = true;
                    t.commit("Image Size");
                    app.record(Op::Resize {
                        width: Some(nw),
                        height: Some(nh),
                        percent: None,
                        method: m,
                    });
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::CanvasSize { w, h, anchor } => {
            let mut out = Outcome::Keep;
            window("Canvas Size").show(ctx, |ui| {
                egui::Grid::new("csz").show(ui, |ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(w).range(1..=30000).suffix(" px"));
                    ui.end_row();
                    ui.label("Height");
                    ui.add(egui::DragValue::new(h).range(1..=30000).suffix(" px"));
                    ui.end_row();
                });
                ui.label("Anchor");
                egui::Grid::new("anchor").show(ui, |ui| {
                    for (i, a) in Anchor::GRID.iter().enumerate() {
                        if ui
                            .selectable_label(*anchor == *a, if *anchor == *a { "●" } else { "○" })
                            .clicked()
                        {
                            *anchor = *a;
                        }
                        if i % 3 == 2 {
                            ui.end_row();
                        }
                    }
                });
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let (nw, nh, a) = (*w, *h, *anchor);
                    let t = app.tab().unwrap();
                    t.doc.canvas_size(nw, nh, a);
                    t.needs_fit = true;
                    t.commit("Canvas Size");
                    app.record(Op::CanvasSize {
                        width: nw,
                        height: nh,
                        anchor: a,
                    });
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Rotate { degrees } => {
            let mut out = Outcome::Keep;
            window("Rotate Canvas").show(ctx, |ui| {
                ui.add(
                    egui::Slider::new(degrees, -180.0..=180.0)
                        .suffix("°")
                        .text("Angle (clockwise)"),
                );
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let d = *degrees;
                    let t = app.tab().unwrap();
                    t.doc.rotate_canvas(d);
                    t.needs_fit = true;
                    t.commit("Rotate Canvas");
                    app.record(Op::Rotate { degrees: d });
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Adjust {
            adj,
            hist,
            live,
            changed,
        } => {
            let mut out = Outcome::Keep;
            let title = adj.name().to_string();
            let mut ok_pressed = false;
            let mut cancel_pressed = false;
            window(&title).resizable(true).show(ctx, |ui| {
                ui.set_min_width(300.0);
                *changed |= adjustment_ui(ui, adj, Some(hist));
                ui.checkbox(live, "Preview");
                let (ok, cancel) = ok_cancel(ui);
                ok_pressed = ok || enter;
                cancel_pressed = cancel || esc;
            });
            let t = app.tab().unwrap();
            let sel = t.doc.selection.clone();
            if (*changed && *live) || ok_pressed {
                if let Some(p) = &t.preview {
                    let mut img = p.original.clone();
                    adj.apply(&mut img, sel.as_ref());
                    let lid = p.layer;
                    if let Some(l) = t.doc.layer_mut(lid) {
                        l.set_image(img);
                    }
                    t.full_redraw = true;
                }
                *changed = false;
            } else if !*live && *changed {
                if let Some(p) = &t.preview {
                    let orig = p.original.clone();
                    let lid = p.layer;
                    if let Some(l) = t.doc.layer_mut(lid) {
                        l.set_image(orig);
                    }
                    t.full_redraw = true;
                }
            }
            if ok_pressed {
                t.preview = None;
                t.commit(&title);
                let a = adj.clone();
                app.record(Op::Adjust { adjustment: a });
                out = Outcome::Close;
            } else if cancel_pressed {
                t.cancel_sessions();
                out = Outcome::Close;
            }
            out
        }
        Dialog::Filter {
            filter,
            live,
            changed,
        } => {
            let mut out = Outcome::Keep;
            let title = filter.name().to_string();
            let mut ok_pressed = false;
            let mut cancel_pressed = false;
            window(&title).show(ctx, |ui| {
                ui.set_min_width(300.0);
                *changed |= filter_ui(ui, filter);
                ui.checkbox(live, "Preview (updates when you release the slider)");
                let (ok, cancel) = ok_cancel(ui);
                ok_pressed = ok || enter;
                cancel_pressed = cancel || esc;
            });
            let fctx = app.filter_ctx();
            let t = app.tab().unwrap();
            let sel = t.doc.selection.clone();
            if (*changed && *live && !pointer_down)
                || (ok_pressed && *changed)
                || (ok_pressed && !*live)
            {
                if let Some(p) = &t.preview {
                    let mut img = p.original.clone();
                    filter.apply(&mut img, sel.as_ref(), &fctx);
                    let lid = p.layer;
                    if let Some(l) = t.doc.layer_mut(lid) {
                        l.set_image(img);
                    }
                    t.full_redraw = true;
                }
                *changed = false;
            }
            if ok_pressed {
                t.preview = None;
                t.commit(&title);
                let f = filter.clone();
                app.last_filter = Some(f.clone());
                app.record(Op::Filter { filter: f });
                out = Outcome::Close;
            } else if cancel_pressed {
                t.cancel_sessions();
                out = Outcome::Close;
            }
            out
        }
        Dialog::LayerStyle {
            layer,
            original,
            selected,
        } => {
            let mut out = Outcome::Keep;
            let lid = *layer;
            let mut changed = false;
            let mut ok_pressed = false;
            let mut cancel_pressed = false;
            let t = app.tabs.get_mut(app.active_tab).unwrap();
            if let Some(l) = t.doc.layer_mut(lid) {
                window("Layer Style")
                    .resizable(true)
                    .default_width(560.0)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Blend");
                            changed |= blend_combo(ui, "ls_blend", &mut l.blend);
                            ui.label("Opacity");
                            let mut o = l.opacity * 100.0;
                            if ui
                                .add(egui::DragValue::new(&mut o).range(0.0..=100.0).suffix("%"))
                                .changed()
                            {
                                l.opacity = o / 100.0;
                                changed = true;
                            }
                            ui.label("Fill");
                            let mut f = l.fill * 100.0;
                            if ui
                                .add(egui::DragValue::new(&mut f).range(0.0..=100.0).suffix("%"))
                                .changed()
                            {
                                l.fill = f / 100.0;
                                changed = true;
                            }
                        });
                        ui.separator();
                        ui.columns(2, |cols| {
                            let ui = &mut cols[0];
                            for proto in Effect::catalog() {
                                let idx = l.effects.iter().position(|e| e.name() == proto.name());
                                ui.horizontal(|ui| {
                                    let mut on =
                                        idx.map(|i| l.effects[i].enabled()).unwrap_or(false);
                                    if ui.checkbox(&mut on, "").changed() {
                                        match idx {
                                            Some(i) => l.effects[i].set_enabled(on),
                                            None => {
                                                l.effects.push(proto.clone());
                                                *selected = l.effects.len() - 1;
                                            }
                                        }
                                        changed = true;
                                    }
                                    let sel_now = idx == Some(*selected);
                                    if ui.selectable_label(sel_now, proto.name()).clicked() {
                                        match idx {
                                            Some(i) => *selected = i,
                                            None => {
                                                l.effects.push(proto.clone());
                                                *selected = l.effects.len() - 1;
                                                changed = true;
                                            }
                                        }
                                    }
                                });
                            }
                            let ui = &mut cols[1];
                            if let Some(e) = l.effects.get_mut(*selected) {
                                ui.label(RichText::new(e.name()).strong());
                                changed |= effect_ui(ui, e);
                                if ui.small_button("Remove effect").clicked() {
                                    l.effects.remove(*selected);
                                    *selected = 0;
                                    changed = true;
                                }
                            } else {
                                ui.label("Tick an effect on the left to add it.");
                            }
                        });
                        let (ok, cancel) = ok_cancel(ui);
                        ok_pressed = ok || enter;
                        cancel_pressed = cancel || esc;
                    });
                if changed {
                    l.touch();
                    t.full_redraw = true;
                }
                if cancel_pressed {
                    l.effects = original.clone();
                    l.touch();
                    t.full_redraw = true;
                }
            }
            if ok_pressed {
                t.commit("Layer Style");
                out = Outcome::Close;
            } else if cancel_pressed {
                out = Outcome::Close;
            }
            out
        }
        Dialog::Fill {
            source,
            color,
            opacity,
            mode,
            preserve,
        } => {
            let mut out = Outcome::Keep;
            window("Fill").show(ctx, |ui| {
                egui::ComboBox::from_label("Contents")
                    .selected_text(
                        [
                            "Foreground Color",
                            "Background Color",
                            "Color…",
                            "Content-Aware",
                            "Black",
                            "50% Gray",
                            "White",
                        ][*source as usize],
                    )
                    .show_ui(ui, |ui| {
                        for (i, n) in [
                            "Foreground Color",
                            "Background Color",
                            "Color…",
                            "Content-Aware",
                            "Black",
                            "50% Gray",
                            "White",
                        ]
                        .iter()
                        .enumerate()
                        {
                            ui.selectable_value(source, i as u8, *n);
                        }
                    });
                if *source == 2 {
                    color_button(ui, color);
                }
                if *source != 3 {
                    ui.horizontal(|ui| {
                        ui.label("Mode");
                        blend_combo(ui, "fill_mode", mode);
                    });
                    let mut o = *opacity * 100.0;
                    if ui
                        .add(
                            egui::Slider::new(&mut o, 1.0..=100.0)
                                .suffix("%")
                                .text("Opacity"),
                        )
                        .changed()
                    {
                        *opacity = o / 100.0;
                    }
                    ui.checkbox(preserve, "Preserve Transparency");
                }
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let (fg, bg) = (app.fg, app.bg);
                    let t = app.tab().unwrap();
                    if !t.active_is_pixel() {
                        app.toast("Fill needs a pixel layer");
                    } else if *source == 3 {
                        app.pending.push(Cmd::ContentAwareFill);
                    } else {
                        let c = match source {
                            0 => fg,
                            1 => bg,
                            2 => *color,
                            4 => Color::BLACK,
                            5 => Color::rgb(0.5, 0.5, 0.5),
                            _ => Color::WHITE,
                        };
                        let sel = t.doc.selection.clone();
                        let l = t.doc.active_layer_mut().unwrap();
                        kroma_core::brush::fill_selection(
                            l.image_mut(),
                            sel.as_ref(),
                            c,
                            *opacity,
                            *mode,
                            *preserve,
                        );
                        t.commit("Fill");
                        app.record(Op::FillSelection { color: c });
                    }
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Stroke {
            width,
            color,
            location,
        } => {
            let mut out = Outcome::Keep;
            window("Stroke").show(ctx, |ui| {
                ui.add(
                    egui::Slider::new(width, 1.0..=250.0)
                        .suffix(" px")
                        .text("Width"),
                );
                ui.horizontal(|ui| {
                    ui.label("Color");
                    color_button(ui, color);
                });
                ui.horizontal(|ui| {
                    ui.radio_value(location, StrokeLocation::Inside, "Inside");
                    ui.radio_value(location, StrokeLocation::Center, "Center");
                    ui.radio_value(location, StrokeLocation::Outside, "Outside");
                });
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let t = app.tab().unwrap();
                    if let (true, Some(sel)) = (t.active_is_pixel(), t.doc.selection.clone()) {
                        let l = t.doc.active_layer_mut().unwrap();
                        kroma_core::brush::stroke_selection(
                            l.image_mut(),
                            &sel,
                            *color,
                            *width,
                            *location,
                        );
                        t.commit("Stroke");
                    }
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::ColorRange {
            color,
            fuzziness,
            mode,
        } => {
            let mut out = Outcome::Keep;
            window("Color Range").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(mode, 0, "Sampled color");
                    ui.radio_value(mode, 1, "Highlights");
                    ui.radio_value(mode, 2, "Midtones");
                    ui.radio_value(mode, 3, "Shadows");
                });
                if *mode == 0 {
                    ui.horizontal(|ui| {
                        color_button(ui, color);
                        if ui.small_button("Use foreground").clicked() {
                            *color = app.fg;
                        }
                    });
                    ui.add(egui::Slider::new(fuzziness, 0.0..=200.0).text("Fuzziness"));
                    ui.label(
                        RichText::new("Tip: pick the color with the Eyedropper first.")
                            .weak()
                            .small(),
                    );
                }
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let (c, f, m) = (*color, *fuzziness, *mode);
                    let t = app.tab().unwrap();
                    let img = t.doc.composite();
                    t.doc.selection = Some(if m == 0 {
                        kroma_core::selection::color_range(&img, c, f)
                    } else {
                        kroma_core::selection::tonal_range(&img, m - 1)
                    });
                    t.commit("Color Range");
                    if m == 0 {
                        app.record(Op::SelectColorRange {
                            color: c,
                            fuzziness: f,
                        });
                    }
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::ModifySelection { kind, amount } => {
            let mut out = Outcome::Keep;
            let names = ["Border", "Smooth", "Expand", "Contract", "Feather"];
            window(&format!("{} Selection", names[*kind as usize])).show(ctx, |ui| {
                ui.add(
                    egui::Slider::new(amount, 0.5..=250.0)
                        .suffix(" px")
                        .text(if *kind == 4 {
                            "Feather radius"
                        } else {
                            "Amount"
                        }),
                );
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let (k, a) = (*kind, *amount);
                    let t = app.tab().unwrap();
                    if let Some(s) = &t.doc.selection {
                        let m = match k {
                            0 => kroma_core::selection::border(s, a),
                            1 => kroma_core::selection::smooth(s, a),
                            2 => kroma_core::selection::expand(s, a),
                            3 => kroma_core::selection::contract(s, a),
                            _ => kroma_core::selection::feather(s, a),
                        };
                        t.doc.selection = Some(m);
                        t.commit(names[k as usize]);
                    }
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Export {
            ext,
            quality,
            scale,
            sixteen,
        } => {
            let mut out = Outcome::Keep;
            let exts = kroma_core::io::EXPORT_EXTENSIONS;
            window("Export As").show(ctx, |ui| {
                egui::ComboBox::from_label("Format")
                    .selected_text(exts[*ext].to_uppercase())
                    .show_ui(ui, |ui| {
                        for (i, e) in exts.iter().enumerate() {
                            ui.selectable_value(ext, i, e.to_uppercase());
                        }
                    });
                let e = exts[*ext];
                if matches!(e, "jpg" | "jpeg") {
                    ui.add(egui::Slider::new(quality, 1..=100).text("Quality"));
                }
                if matches!(e, "png" | "tif" | "tiff") {
                    ui.checkbox(sixteen, "16 bits per channel");
                }
                let mut pct = *scale * 100.0;
                if ui
                    .add(
                        egui::Slider::new(&mut pct, 5.0..=400.0)
                            .suffix("%")
                            .text("Scale"),
                    )
                    .changed()
                {
                    *scale = pct / 100.0;
                }
                if let Some(t) = app.tabs.get(app.active_tab) {
                    ui.label(format!(
                        "Output: {} × {} px",
                        (t.doc.width as f32 * *scale).round(),
                        (t.doc.height as f32 * *scale).round()
                    ));
                    let key = egui::Id::new((
                        "export_est",
                        t.id,
                        t.history.cursor(),
                        e,
                        *quality,
                        *sixteen,
                    ));
                    let cached: Option<usize> = ctx.data(|d| d.get_temp(key));
                    let est = cached.or_else(|| {
                        let v = estimate(t, e, *quality, *sixteen).ok();
                        if let Some(v) = v {
                            ctx.data_mut(|d| d.insert_temp(key, v));
                        }
                        v
                    });
                    if let Some(bytes) = est {
                        ui.label(
                            RichText::new(format!(
                                "Estimated size ≈ {}",
                                human(bytes as f64 * (*scale as f64).powi(2))
                            ))
                            .weak(),
                        );
                    }
                }
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    let t = app.tabs.get(app.active_tab).unwrap();
                    let stem = std::path::Path::new(&t.doc.name)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "export".into());
                    if let Some(p) = rfd::FileDialog::new()
                        .set_file_name(format!("{stem}.{e}"))
                        .add_filter(e.to_uppercase(), &[e])
                        .save_file()
                    {
                        let opts = kroma_core::io::ExportOptions {
                            quality: *quality,
                            sixteen_bit: *sixteen,
                            matte: Color::WHITE,
                            scale: *scale,
                        };
                        match kroma_core::io::export(&t.doc, &p, &opts) {
                            Ok(_) => app.toast(format!("Exported {}", p.display())),
                            Err(err) => app.toast(format!("Export failed: {err:#}")),
                        }
                        out = Outcome::Close;
                    }
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Batch {
            action,
            files,
            out_dir,
            ext,
            quality,
        } => {
            let mut out = Outcome::Keep;
            let exts = kroma_core::io::EXPORT_EXTENSIONS;
            window("Batch").show(ctx, |ui| {
                egui::ComboBox::from_label("Action").selected_text(app.actions.get(*action).map(|a| a.name.clone()).unwrap_or_default()).show_ui(ui, |ui| {
                    for (i, a) in app.actions.iter().enumerate() {
                        ui.selectable_value(action, i, &a.name);
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Choose source images…").clicked() {
                        if let Some(f) = rfd::FileDialog::new().add_filter("Images", kroma_core::io::OPEN_EXTENSIONS).pick_files() {
                            *files = f;
                        }
                    }
                    ui.label(format!("{} files", files.len()));
                });
                ui.horizontal(|ui| {
                    if ui.button("Choose destination folder…").clicked() {
                        *out_dir = rfd::FileDialog::new().pick_folder();
                    }
                    ui.label(out_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "(none)".into()));
                });
                egui::ComboBox::from_label("Save as").selected_text(exts[*ext].to_uppercase()).show_ui(ui, |ui| {
                    for (i, e) in exts.iter().enumerate() {
                        ui.selectable_value(ext, i, e.to_uppercase());
                    }
                });
                if matches!(exts[*ext], "jpg" | "jpeg") {
                    ui.add(egui::Slider::new(quality, 1..=100).text("Quality"));
                }
                ui.label(RichText::new("Runs in the background; progress shows in the status bar. Originals are never overwritten.").weak().small());
                let (ok, cancel) = ok_cancel(ui);
                if ok && !files.is_empty() && out_dir.is_some() {
                    if let Some(a) = app.actions.get(*action).cloned() {
                        let job = Arc::new(Mutex::new(BatchJob { total: files.len(), done: 0, errors: vec![], finished: false }));
                        app.batch = Some(job.clone());
                        let files = files.clone();
                        let dir = out_dir.clone().unwrap();
                        let e = exts[*ext].to_string();
                        let q = *quality;
                        let fctx = app.filter_ctx();
                        let ctx2 = ctx.clone();
                        std::thread::spawn(move || {
                            for f in files {
                                let r = (|| -> anyhow::Result<()> {
                                    let mut d = kroma_core::io::open(&f)?;
                                    a.run(&mut d, &fctx)?;
                                    let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "out".into());
                                    let mut target = dir.join(format!("{stem}.{e}"));
                                    if target == f {
                                        target = dir.join(format!("{stem}_kroma.{e}"));
                                    }
                                    kroma_core::io::export(&d, &target, &kroma_core::io::ExportOptions { quality: q, ..Default::default() })?;
                                    Ok(())
                                })();
                                let mut j = job.lock().unwrap();
                                j.done += 1;
                                if let Err(err) = r {
                                    j.errors.push(format!("{}: {err:#}", f.display()));
                                }
                                ctx2.request_repaint();
                            }
                            job.lock().unwrap().finished = true;
                            ctx2.request_repaint();
                        });
                        out = Outcome::Close;
                    }
                } else if ok {
                    app.toast("Pick source images and a destination folder");
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Rename { id, name } => {
            let mut out = Outcome::Keep;
            window("Rename Layer").show(ctx, |ui| {
                let r = ui.text_edit_singleline(name);
                r.request_focus();
                let ent = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let (ok, cancel) = ok_cancel(ui);
                if ok || ent {
                    let (lid, n) = (*id, name.clone());
                    let t = app.tab().unwrap();
                    if let Some(l) = t.doc.layer_mut(lid) {
                        l.name = n;
                    }
                    t.commit("Rename Layer");
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::TextEdit { layer, select_all } => {
            let mut out = Outcome::Keep;
            let lid = *layer;
            let t = app.tabs.get_mut(app.active_tab).unwrap();
            let (w, h) = (t.doc.width, t.doc.height);
            let mut changed = false;
            let mut done = false;
            let mut remove = false;
            if let Some(l) = t.doc.layer_mut(lid) {
                if let LayerKind::Text(style) = &mut l.kind {
                    window("Type").resizable(true).show(ctx, |ui| {
                        let r = ui.add(
                            egui::TextEdit::multiline(&mut style.text)
                                .desired_rows(4)
                                .desired_width(380.0)
                                .font(egui::TextStyle::Body),
                        );
                        if *select_all {
                            r.request_focus();
                            if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), r.id) {
                                let range = egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(style.text.chars().count()),
                                );
                                state.cursor.set_char_range(Some(range));
                                state.store(ui.ctx(), r.id);
                            }
                            *select_all = false;
                        }
                        changed |= r.changed();
                        ui.horizontal_wrapped(|ui| {
                            changed |= crate::tools::text_options(ui, style);
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new(" ✔ Done ").strong()).clicked() {
                                done = true;
                            }
                            if ui.button("Delete text layer").clicked() {
                                remove = true;
                            }
                        });
                    });
                    if changed {
                        l.name = style
                            .text
                            .lines()
                            .next()
                            .unwrap_or("Text")
                            .chars()
                            .take(30)
                            .collect();
                        l.rerender(w, h);
                        t.full_redraw = true;
                    }
                } else {
                    done = true;
                }
            } else {
                done = true;
            }
            if esc {
                done = true;
            }
            if remove {
                t.doc.delete_layer(lid);
                t.commit("Delete Type Layer");
                out = Outcome::Close;
            } else if done {
                t.commit("Edit Type");
                out = Outcome::Close;
            }
            out
        }
        Dialog::ColorPicker { background, hsva } => {
            let mut out = Outcome::Keep;
            window(if *background {
                "Color Picker (Background)"
            } else {
                "Color Picker (Foreground)"
            })
            .show(ctx, |ui| {
                egui::widgets::color_picker::color_picker_hsva_2d(
                    ui,
                    hsva,
                    egui::widgets::color_picker::Alpha::Opaque,
                );
                let [r, g, b] = hsva.to_srgb();
                let c = Color::from_rgba8(r, g, b, 255);
                let mut hex = c.to_hex();
                ui.horizontal(|ui| {
                    ui.label("#");
                    if ui.text_edit_singleline(&mut hex).changed() {
                        if let Some(nc) = Color::from_hex(&hex) {
                            let v = nc.to_rgba8();
                            *hsva = egui::ecolor::Hsva::from_srgb([v[0], v[1], v[2]]);
                        }
                    }
                });
                let hsl = kroma_core::color::rgb_to_hsl([c.r, c.g, c.b]);
                ui.label(
                    RichText::new(format!(
                        "RGB {r} {g} {b}   HSL {:.0}° {:.0}% {:.0}%",
                        hsl[0],
                        hsl[1] * 100.0,
                        hsl[2] * 100.0
                    ))
                    .small(),
                );
                let (ok, cancel) = ok_cancel(ui);
                if ok || enter {
                    if *background {
                        app.bg = c;
                    } else {
                        app.fg = c;
                    }
                    out = Outcome::Close;
                }
                if cancel || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Recovery { entries } => {
            let mut out = Outcome::Keep;
            window("Recover unsaved work").show(ctx, |ui| {
                ui.label(
                    "Kroma found recovery snapshots from a previous session. \
                     Open to restore, or Discard to remove them.",
                );
                let mut open_path: Option<std::path::PathBuf> = None;
                let mut discard_path: Option<std::path::PathBuf> = None;
                let mut discard_all = false;
                for e in entries.iter() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} ({})", e.doc_name, e.tab_id)).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Discard").clicked() {
                                discard_path = Some(e.path.clone());
                            }
                            if ui.button(RichText::new("Open").strong()).clicked() {
                                open_path = Some(e.path.clone());
                            }
                        });
                    });
                }
                ui.separator();
                if ui.button("Discard all").clicked() {
                    discard_all = true;
                }
                if let Some(p) = open_path {
                    match kroma_core::io::open(&p) {
                        Ok(doc) => {
                            app.tabs.push(crate::tab::DocTab::new(doc, None));
                            app.active_tab = app.tabs.len().saturating_sub(1);
                            if let Some(t) = app.tabs.last_mut() {
                                t.dirty = true;
                            }
                            crate::recovery::delete_path(&p);
                        }
                        Err(err) => app.toast(format!("Recovery open failed: {err:#}")),
                    }
                }
                if let Some(p) = discard_path {
                    crate::recovery::delete_path(&p);
                }
                if discard_all {
                    crate::recovery::discard_all();
                }
                entries.retain(|e| e.path.exists());
                if entries.is_empty() {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::ConfirmClose { tab, quitting } => {
            let mut out = Outcome::Keep;
            let name = app
                .tabs
                .get(*tab)
                .map(|t| t.doc.name.clone())
                .unwrap_or_default();
            let (ti, q) = (*tab, *quitting);
            window("Unsaved changes").show(ctx, |ui| {
                ui.label(format!("Save changes to \"{name}\" before closing?"));
                ui.horizontal(|ui| {
                    if ui.button(RichText::new("Save").strong()).clicked() {
                        app.active_tab = ti;
                        app.run(ctx, Cmd::Save);
                        if !app.tabs.get(ti).map(|t| t.dirty).unwrap_or(false) {
                            app.close_tab(ti);
                            out = Outcome::Close;
                        }
                    }
                    if ui.button("Don't Save").clicked() {
                        app.close_tab(ti);
                        out = Outcome::Close;
                    }
                    if ui.button("Cancel").clicked() {
                        app.quit_requested = false;
                        out = Outcome::Close;
                    }
                });
            });
            if matches!(out, Outcome::Close) && q && !app.tabs.is_empty()
                || (matches!(out, Outcome::Close) && q && app.tabs.is_empty())
            {
                if let Some(i) = app.tabs.iter().position(|t| t.dirty) {
                    app.dialog = Some(Dialog::ConfirmClose {
                        tab: i,
                        quitting: true,
                    });
                    return;
                } else if q {
                    app.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            out
        }
        Dialog::Shortcuts => {
            let mut out = Outcome::Keep;
            window("Keyboard Shortcuts")
                .resizable(true)
                .default_height(520.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(480.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Tools").strong());
                            egui::Grid::new("sc_tools").striped(true).show(ui, |ui| {
                                for g in crate::tools::Tool::GROUPS {
                                    let names: Vec<&str> = g.iter().map(|t| t.name()).collect();
                                    ui.label(names.join(" / "));
                                    ui.label(
                                        g[0].shortcut()
                                            .map(|k| k.name().to_string())
                                            .unwrap_or_default()
                                            + if g.len() > 1 { "  (Shift cycles)" } else { "" },
                                    );
                                    ui.end_row();
                                }
                                for (a, b) in [
                                    ("Hand (temporary)", "Space"),
                                    ("Brush size", "[  ]"),
                                    ("Brush hardness", "Shift+[  ]"),
                                    ("Opacity 10–100%", "1 … 0"),
                                    ("Swap / default colors", "X / D"),
                                    ("Sample clone source", "Alt+click"),
                                    ("Add / subtract selection", "Shift / Alt + drag"),
                                    ("Load layer as selection", "Ctrl+click thumbnail"),
                                    ("Zoom", "Mouse wheel / Ctrl++ / Ctrl+-"),
                                    ("Pan", "Middle drag, Shift+wheel"),
                                ] {
                                    ui.label(a);
                                    ui.label(b);
                                    ui.end_row();
                                }
                            });
                            ui.separator();
                            ui.label(RichText::new("Commands").strong());
                            egui::Grid::new("sc_cmds").striped(true).show(ui, |ui| {
                                for (n, _, sc) in crate::panels::all_commands() {
                                    if !sc.is_empty() {
                                        ui.label(n);
                                        ui.label(sc);
                                        ui.end_row();
                                    }
                                }
                            });
                        });
                    if ui.button("Close").clicked() || esc {
                        out = Outcome::Close;
                    }
                });
            out
        }
        Dialog::About => {
            let mut out = Outcome::Keep;
            window("About Kroma").show(ctx, |ui| {
                ui.heading("Kroma");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A layered raster image editor written in Rust.");
                ui.label("27 blend modes · 19 adjustments · 37 filters · 9 layer styles · PatchMatch content-aware fill · .kroma / .ora / PSD import");
                if ui.button("Close").clicked() || esc || enter {
                    out = Outcome::Close;
                }
            });
            out
        }
        Dialog::Preferences => {
            let mut out = Outcome::Keep;
            window("Preferences").show(ctx, |ui| {
                if ui.checkbox(&mut app.dark, "Dark interface").changed() {
                    crate::theme::apply(ctx, app.dark);
                }
                ui.checkbox(
                    &mut app.show_pixel_grid,
                    "Show pixel grid when zoomed past 1200%",
                );
                ui.checkbox(&mut app.show_guides, "Show guides");
                let mut lim = app.tabs.first().map(|t| t.history.limit).unwrap_or(100);
                if ui
                    .add(egui::Slider::new(&mut lim, 10..=1000).text("History states"))
                    .changed()
                {
                    for t in app.tabs.iter_mut() {
                        t.history.limit = lim;
                    }
                }
                if ui.button("Close").clicked() || esc {
                    out = Outcome::Close;
                }
            });
            out
        }
    };
    if matches!(outcome, Outcome::Keep) && app.dialog.is_none() {
        app.dialog = Some(dlg);
    }
}

fn estimate(
    t: &crate::tab::DocTab,
    ext: &str,
    quality: u8,
    sixteen: bool,
) -> anyhow::Result<usize> {
    // encode a downscaled proxy and scale the byte count by the area ratio
    let comp = t.doc.composite();
    let s = (256.0 / comp.width.max(comp.height) as f32).min(1.0);
    let small = kroma_core::transform::resize(
        &comp,
        ((comp.width as f32 * s) as u32).max(1),
        ((comp.height as f32 * s) as u32).max(1),
        Resample::Bilinear,
    );
    let bytes = kroma_core::io::encode_image(
        &small,
        ext,
        &kroma_core::io::ExportOptions {
            quality,
            sixteen_bit: sixteen,
            ..Default::default()
        },
    )?;
    Ok((bytes.len() as f32 / (s * s)) as usize)
}

fn human(b: f64) -> String {
    if b > 1_048_576.0 {
        format!("{:.1} MB", b / 1_048_576.0)
    } else {
        format!("{:.0} KB", b / 1024.0)
    }
}
