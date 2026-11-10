//! Menus, toolbox, options bar, docked panels, document tabs, status bar, command palette.

use crate::app::{Cmd, KromaApp};
use crate::tools::text_options;
use crate::tools::Tool;
use crate::widgets::{adjustment_ui, blend_combo, c32, color_button, histogram_view};
use egui::{Color32, Pos2, Rect as ERect, RichText, Sense, Stroke, Ui, Vec2};
use kroma_core::adjust::{Adjustment, Histogram};
use kroma_core::effects::Effect;
use kroma_core::filter::Filter;
use kroma_core::layer::{FillSource, LayerId, LayerKind};

fn item(ui: &mut Ui, app: &mut KromaApp, label: &str, shortcut: &str, cmd: Cmd) {
    let btn = egui::Button::new(label).shortcut_text(shortcut);
    if ui.add(btn).clicked() {
        app.pending.push(cmd);
        ui.close();
    }
}

/// Every runnable command with a display name (for the command palette).
pub fn all_commands() -> Vec<(String, Cmd, &'static str)> {
    let mut v: Vec<(String, Cmd, &'static str)> = vec![
        ("New Document".into(), Cmd::NewDoc, "Ctrl+N"),
        ("Open".into(), Cmd::Open, "Ctrl+O"),
        ("Place Embedded (Import as Layer)".into(), Cmd::Place, ""),
        ("Save".into(), Cmd::Save, "Ctrl+S"),
        ("Save As".into(), Cmd::SaveAs, "Ctrl+Shift+S"),
        ("Export As".into(), Cmd::ExportAs, "Ctrl+Alt+Shift+W"),
        ("Quick Export as PNG".into(), Cmd::QuickExportPng, ""),
        ("Close Document".into(), Cmd::CloseTab, "Ctrl+W"),
        ("Revert".into(), Cmd::Revert, ""),
        ("Undo".into(), Cmd::Undo, "Ctrl+Z"),
        ("Redo".into(), Cmd::Redo, "Ctrl+Shift+Z"),
        ("Cut".into(), Cmd::Cut, "Ctrl+X"),
        ("Copy".into(), Cmd::Copy, "Ctrl+C"),
        ("Copy Merged".into(), Cmd::CopyMerged, "Ctrl+Shift+C"),
        ("Paste".into(), Cmd::Paste, "Ctrl+V"),
        ("New Document from Clipboard".into(), Cmd::PasteAsNewDoc, ""),
        ("Clear".into(), Cmd::Clear, "Del"),
        ("Fill".into(), Cmd::FillDialog, "Shift+F5"),
        ("Stroke Selection".into(), Cmd::StrokeDialog, ""),
        ("Content-Aware Fill".into(), Cmd::ContentAwareFill, ""),
        ("Free Transform".into(), Cmd::FreeTransform, "Ctrl+T"),
        ("Distort / Perspective".into(), Cmd::Distort, ""),
        ("Flip Layer Horizontal".into(), Cmd::FlipLayer(true), ""),
        ("Flip Layer Vertical".into(), Cmd::FlipLayer(false), ""),
        ("Rotate Layer 90° CW".into(), Cmd::RotateLayer(90), ""),
        ("Rotate Layer 90° CCW".into(), Cmd::RotateLayer(-90), ""),
        ("Rotate Layer 180°".into(), Cmd::RotateLayer(180), ""),
        ("Auto Tone".into(), Cmd::AutoTone, "Ctrl+Shift+L"),
        (
            "Auto Contrast".into(),
            Cmd::AutoContrast,
            "Ctrl+Alt+Shift+L",
        ),
        ("Auto Color".into(), Cmd::AutoColor, "Ctrl+Shift+B"),
        ("Invert".into(), Cmd::QuickInvert, "Ctrl+I"),
        ("Desaturate".into(), Cmd::QuickDesaturate, "Ctrl+Shift+U"),
        ("Image Size".into(), Cmd::ImageSize, "Ctrl+Alt+I"),
        ("Canvas Size".into(), Cmd::CanvasSize, "Ctrl+Alt+C"),
        ("Rotate Canvas 90° CW".into(), Cmd::RotateCanvas(90), ""),
        ("Rotate Canvas 90° CCW".into(), Cmd::RotateCanvas(-90), ""),
        ("Rotate Canvas 180°".into(), Cmd::RotateCanvas(180), ""),
        ("Rotate Canvas Arbitrary".into(), Cmd::RotateArbitrary, ""),
        ("Flip Canvas Horizontal".into(), Cmd::FlipCanvas(true), ""),
        ("Flip Canvas Vertical".into(), Cmd::FlipCanvas(false), ""),
        ("Crop to Selection".into(), Cmd::CropToSelection, ""),
        ("Trim Transparent".into(), Cmd::Trim, ""),
        ("Duplicate Document".into(), Cmd::DuplicateDoc, ""),
        ("New Layer".into(), Cmd::NewLayer, "Ctrl+Shift+N"),
        ("Layer via Copy".into(), Cmd::LayerViaCopy, "Ctrl+J"),
        ("Layer via Cut".into(), Cmd::LayerViaCut, "Ctrl+Shift+J"),
        ("Duplicate Layer".into(), Cmd::DuplicateLayer, ""),
        ("Delete Layer".into(), Cmd::DeleteLayer, ""),
        ("Rename Layer".into(), Cmd::RenameLayer, ""),
        ("New Solid Color Fill Layer".into(), Cmd::NewFillSolid, ""),
        ("New Gradient Fill Layer".into(), Cmd::NewFillGradient, ""),
        ("Layer Style (Blending Options)".into(), Cmd::LayerStyle, ""),
        ("Clear Layer Style".into(), Cmd::ClearStyle, ""),
        (
            "Add Layer Mask: Reveal All".into(),
            Cmd::AddMask {
                from_selection: false,
                hide: false,
            },
            "",
        ),
        (
            "Add Layer Mask: Hide All".into(),
            Cmd::AddMask {
                from_selection: false,
                hide: true,
            },
            "",
        ),
        (
            "Add Layer Mask: Reveal Selection".into(),
            Cmd::AddMask {
                from_selection: true,
                hide: false,
            },
            "",
        ),
        (
            "Add Layer Mask: Hide Selection".into(),
            Cmd::AddMask {
                from_selection: true,
                hide: true,
            },
            "",
        ),
        ("Delete Layer Mask".into(), Cmd::DeleteMask, ""),
        ("Apply Layer Mask".into(), Cmd::ApplyMask, ""),
        (
            "Disable/Enable Layer Mask".into(),
            Cmd::ToggleMaskEnabled,
            "",
        ),
        ("Invert Layer Mask".into(), Cmd::InvertMask, ""),
        (
            "Create/Release Clipping Mask".into(),
            Cmd::ToggleClip,
            "Ctrl+Alt+G",
        ),
        ("Group Layers".into(), Cmd::Group, "Ctrl+G"),
        ("Ungroup Layers".into(), Cmd::Ungroup, "Ctrl+Shift+G"),
        ("Bring to Front".into(), Cmd::Arrange(2), "Ctrl+Shift+]"),
        ("Bring Forward".into(), Cmd::Arrange(1), "Ctrl+]"),
        ("Send Backward".into(), Cmd::Arrange(-1), "Ctrl+["),
        ("Send to Back".into(), Cmd::Arrange(-2), "Ctrl+Shift+["),
        ("Rasterize Layer".into(), Cmd::Rasterize, ""),
        ("Merge Down".into(), Cmd::MergeDown, "Ctrl+E"),
        ("Merge Visible".into(), Cmd::MergeVisible, "Ctrl+Shift+E"),
        ("Flatten Image".into(), Cmd::Flatten, ""),
        (
            "Load Selection from Layer".into(),
            Cmd::SelectLayerPixels,
            "",
        ),
        ("Select All".into(), Cmd::SelectAll, "Ctrl+A"),
        ("Deselect".into(), Cmd::Deselect, "Ctrl+D"),
        ("Reselect".into(), Cmd::Reselect, "Ctrl+Shift+D"),
        (
            "Inverse Selection".into(),
            Cmd::InvertSelection,
            "Ctrl+Shift+I",
        ),
        ("Select Subject".into(), Cmd::SelectSubject, ""),
        ("Color Range".into(), Cmd::ColorRange, ""),
        (
            "Modify Selection: Border".into(),
            Cmd::ModifySelection(0),
            "",
        ),
        (
            "Modify Selection: Smooth".into(),
            Cmd::ModifySelection(1),
            "",
        ),
        (
            "Modify Selection: Expand".into(),
            Cmd::ModifySelection(2),
            "",
        ),
        (
            "Modify Selection: Contract".into(),
            Cmd::ModifySelection(3),
            "",
        ),
        (
            "Modify Selection: Feather".into(),
            Cmd::ModifySelection(4),
            "Shift+F6",
        ),
        ("Grow Selection".into(), Cmd::Grow, ""),
        ("Select Similar".into(), Cmd::Similar, ""),
        ("Last Filter".into(), Cmd::LastFilter, "Ctrl+Alt+F"),
        ("Zoom In".into(), Cmd::ZoomIn, "Ctrl++"),
        ("Zoom Out".into(), Cmd::ZoomOut, "Ctrl+-"),
        ("Fit on Screen".into(), Cmd::FitScreen, "Ctrl+0"),
        ("100% (Actual Pixels)".into(), Cmd::ActualPixels, "Ctrl+1"),
        ("Toggle Pixel Grid".into(), Cmd::TogglePixelGrid, ""),
        ("Toggle Guides".into(), Cmd::ToggleGuides, ""),
        ("New Vertical Guide".into(), Cmd::AddGuide(true), ""),
        ("New Horizontal Guide".into(), Cmd::AddGuide(false), ""),
        ("Clear Guides".into(), Cmd::ClearGuides, ""),
        ("Take History Snapshot".into(), Cmd::TakeSnapshot, ""),
        ("Start/Stop Recording Action".into(), Cmd::RecordToggle, ""),
        ("Batch Process…".into(), Cmd::BatchProcess, ""),
        ("Keyboard Shortcuts".into(), Cmd::Shortcuts, "F1"),
        ("Preferences".into(), Cmd::Preferences, ""),
        ("Swap Colors".into(), Cmd::SwapColors, "X"),
        ("Default Colors".into(), Cmd::DefaultColors, "D"),
    ];
    for a in Adjustment::catalog() {
        v.push((
            format!("Adjustment: {}", a.name()),
            Cmd::Adjust(a.clone()),
            "",
        ));
        v.push((
            format!("New Adjustment Layer: {}", a.name()),
            Cmd::NewAdjustmentLayer(a),
            "",
        ));
    }
    for (group, fs) in Filter::menu() {
        for f in fs {
            v.push((
                format!("Filter › {group} › {}", f.name()),
                Cmd::Filter(f),
                "",
            ));
        }
    }
    for g in Tool::GROUPS {
        for t in g.iter() {
            v.push((format!("Tool: {}", t.name()), Cmd::SelectTool(*t), ""));
        }
    }
    v
}

pub fn menu_bar(app: &mut KromaApp, ui: &mut Ui) {
    egui::Panel::top("menu").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(
                RichText::new("Kroma")
                    .strong()
                    .color(Color32::from_rgb(120, 170, 255)),
            );
            ui.menu_button("File", |ui| {
                item(ui, app, "New…", "Ctrl+N", Cmd::NewDoc);
                item(ui, app, "Open…", "Ctrl+O", Cmd::Open);
                ui.menu_button("Open Recent", |ui| {
                    if app.recent.is_empty() {
                        ui.label("Nothing yet");
                    }
                    for p in app.recent.clone() {
                        item(ui, app, &p.display().to_string(), "", Cmd::OpenPath(p));
                    }
                });
                item(ui, app, "Place Embedded…", "", Cmd::Place);
                ui.separator();
                item(ui, app, "Save", "Ctrl+S", Cmd::Save);
                item(
                    ui,
                    app,
                    "Save As… (.kroma / .ora)",
                    "Ctrl+Shift+S",
                    Cmd::SaveAs,
                );
                item(ui, app, "Export As…", "Ctrl+Alt+Shift+W", Cmd::ExportAs);
                item(ui, app, "Quick Export as PNG", "", Cmd::QuickExportPng);
                item(ui, app, "Revert", "", Cmd::Revert);
                ui.separator();
                item(ui, app, "Automate › Batch…", "", Cmd::BatchProcess);
                ui.separator();
                item(ui, app, "Close", "Ctrl+W", Cmd::CloseTab);
                item(ui, app, "Exit", "", Cmd::Quit);
            });
            ui.menu_button("Edit", |ui| {
                let (u, r) = app
                    .tabs
                    .get(app.active_tab)
                    .map(|t| (t.history.can_undo(), t.history.can_redo()))
                    .unwrap_or((false, false));
                ui.add_enabled_ui(u, |ui| item(ui, app, "Undo", "Ctrl+Z", Cmd::Undo));
                ui.add_enabled_ui(r, |ui| item(ui, app, "Redo", "Ctrl+Shift+Z", Cmd::Redo));
                ui.separator();
                item(ui, app, "Cut", "Ctrl+X", Cmd::Cut);
                item(ui, app, "Copy", "Ctrl+C", Cmd::Copy);
                item(ui, app, "Copy Merged", "Ctrl+Shift+C", Cmd::CopyMerged);
                item(ui, app, "Paste", "Ctrl+V", Cmd::Paste);
                item(ui, app, "Clear", "Del", Cmd::Clear);
                ui.separator();
                item(ui, app, "Fill…", "Shift+F5", Cmd::FillDialog);
                item(ui, app, "Stroke…", "", Cmd::StrokeDialog);
                item(ui, app, "Content-Aware Fill", "", Cmd::ContentAwareFill);
                ui.separator();
                item(ui, app, "Free Transform", "Ctrl+T", Cmd::FreeTransform);
                ui.menu_button("Transform", |ui| {
                    item(ui, app, "Distort / Perspective", "", Cmd::Distort);
                    item(ui, app, "Rotate 180°", "", Cmd::RotateLayer(180));
                    item(ui, app, "Rotate 90° Clockwise", "", Cmd::RotateLayer(90));
                    item(
                        ui,
                        app,
                        "Rotate 90° Counter Clockwise",
                        "",
                        Cmd::RotateLayer(-90),
                    );
                    item(ui, app, "Flip Horizontal", "", Cmd::FlipLayer(true));
                    item(ui, app, "Flip Vertical", "", Cmd::FlipLayer(false));
                });
                ui.separator();
                item(ui, app, "Preferences…", "", Cmd::Preferences);
            });
            ui.menu_button("Image", |ui| {
                ui.menu_button("Adjustments", |ui| {
                    for (i, a) in Adjustment::catalog().into_iter().enumerate() {
                        let sc = match i {
                            1 => "Ctrl+L",
                            2 => "Ctrl+M",
                            5 => "Ctrl+U",
                            6 => "Ctrl+B",
                            13 => "Ctrl+I",
                            14 => "Ctrl+Shift+U",
                            _ => "",
                        };
                        if i == 3 || i == 7 || i == 13 {
                            ui.separator();
                        }
                        item(ui, app, &format!("{}…", a.name()), sc, Cmd::Adjust(a));
                    }
                    ui.separator();
                    item(
                        ui,
                        app,
                        "Shadows/Highlights…",
                        "",
                        Cmd::Filter(Filter::ShadowsHighlights {
                            shadows: 0.35,
                            highlights: 0.0,
                            radius: 30.0,
                        }),
                    );
                    item(ui, app, "Equalize", "", Cmd::Filter(Filter::Equalize));
                });
                ui.separator();
                item(ui, app, "Auto Tone", "Ctrl+Shift+L", Cmd::AutoTone);
                item(
                    ui,
                    app,
                    "Auto Contrast",
                    "Ctrl+Alt+Shift+L",
                    Cmd::AutoContrast,
                );
                item(ui, app, "Auto Color", "Ctrl+Shift+B", Cmd::AutoColor);
                ui.separator();
                item(ui, app, "Image Size…", "Ctrl+Alt+I", Cmd::ImageSize);
                item(ui, app, "Canvas Size…", "Ctrl+Alt+C", Cmd::CanvasSize);
                ui.menu_button("Image Rotation", |ui| {
                    item(ui, app, "180°", "", Cmd::RotateCanvas(180));
                    item(ui, app, "90° Clockwise", "", Cmd::RotateCanvas(90));
                    item(ui, app, "90° Counter Clockwise", "", Cmd::RotateCanvas(-90));
                    item(ui, app, "Arbitrary…", "", Cmd::RotateArbitrary);
                    ui.separator();
                    item(ui, app, "Flip Canvas Horizontal", "", Cmd::FlipCanvas(true));
                    item(ui, app, "Flip Canvas Vertical", "", Cmd::FlipCanvas(false));
                });
                item(ui, app, "Crop (to Selection)", "", Cmd::CropToSelection);
                item(ui, app, "Trim", "", Cmd::Trim);
                ui.separator();
                item(ui, app, "Duplicate…", "", Cmd::DuplicateDoc);
            });
            ui.menu_button("Layer", |ui| {
                ui.menu_button("New", |ui| {
                    item(ui, app, "Layer", "Ctrl+Shift+N", Cmd::NewLayer);
                    item(ui, app, "Layer via Copy", "Ctrl+J", Cmd::LayerViaCopy);
                    item(ui, app, "Layer via Cut", "Ctrl+Shift+J", Cmd::LayerViaCut);
                    item(ui, app, "Group from Layer", "Ctrl+G", Cmd::Group);
                });
                item(ui, app, "Duplicate Layer", "", Cmd::DuplicateLayer);
                item(ui, app, "Delete Layer", "", Cmd::DeleteLayer);
                item(ui, app, "Rename Layer…", "", Cmd::RenameLayer);
                ui.separator();
                ui.menu_button("Layer Style", |ui| {
                    item(ui, app, "Blending Options / Styles…", "", Cmd::LayerStyle);
                    item(ui, app, "Clear Layer Style", "", Cmd::ClearStyle);
                });
                ui.menu_button("New Fill Layer", |ui| {
                    item(ui, app, "Solid Color", "", Cmd::NewFillSolid);
                    item(ui, app, "Gradient", "", Cmd::NewFillGradient);
                });
                ui.menu_button("New Adjustment Layer", |ui| {
                    for a in Adjustment::catalog() {
                        item(ui, app, a.name(), "", Cmd::NewAdjustmentLayer(a));
                    }
                });
                ui.menu_button("Layer Mask", |ui| {
                    item(
                        ui,
                        app,
                        "Reveal All",
                        "",
                        Cmd::AddMask {
                            from_selection: false,
                            hide: false,
                        },
                    );
                    item(
                        ui,
                        app,
                        "Hide All",
                        "",
                        Cmd::AddMask {
                            from_selection: false,
                            hide: true,
                        },
                    );
                    item(
                        ui,
                        app,
                        "Reveal Selection",
                        "",
                        Cmd::AddMask {
                            from_selection: true,
                            hide: false,
                        },
                    );
                    item(
                        ui,
                        app,
                        "Hide Selection",
                        "",
                        Cmd::AddMask {
                            from_selection: true,
                            hide: true,
                        },
                    );
                    ui.separator();
                    item(ui, app, "Delete", "", Cmd::DeleteMask);
                    item(ui, app, "Apply", "", Cmd::ApplyMask);
                    item(ui, app, "Disable / Enable", "", Cmd::ToggleMaskEnabled);
                    item(ui, app, "Invert", "", Cmd::InvertMask);
                });
                item(
                    ui,
                    app,
                    "Create / Release Clipping Mask",
                    "Ctrl+Alt+G",
                    Cmd::ToggleClip,
                );
                ui.separator();
                item(ui, app, "Group Layers", "Ctrl+G", Cmd::Group);
                item(ui, app, "Ungroup Layers", "Ctrl+Shift+G", Cmd::Ungroup);
                ui.menu_button("Arrange", |ui| {
                    item(ui, app, "Bring to Front", "Ctrl+Shift+]", Cmd::Arrange(2));
                    item(ui, app, "Bring Forward", "Ctrl+]", Cmd::Arrange(1));
                    item(ui, app, "Send Backward", "Ctrl+[", Cmd::Arrange(-1));
                    item(ui, app, "Send to Back", "Ctrl+Shift+[", Cmd::Arrange(-2));
                });
                item(ui, app, "Rasterize", "", Cmd::Rasterize);
                ui.separator();
                item(ui, app, "Merge Down", "Ctrl+E", Cmd::MergeDown);
                item(ui, app, "Merge Visible", "Ctrl+Shift+E", Cmd::MergeVisible);
                item(ui, app, "Flatten Image", "", Cmd::Flatten);
            });
            ui.menu_button("Select", |ui| {
                item(ui, app, "All", "Ctrl+A", Cmd::SelectAll);
                item(ui, app, "Deselect", "Ctrl+D", Cmd::Deselect);
                item(ui, app, "Reselect", "Ctrl+Shift+D", Cmd::Reselect);
                item(ui, app, "Inverse", "Ctrl+Shift+I", Cmd::InvertSelection);
                ui.separator();
                item(ui, app, "Subject", "", Cmd::SelectSubject);
                item(ui, app, "Color Range…", "", Cmd::ColorRange);
                item(
                    ui,
                    app,
                    "Load from Layer Transparency",
                    "",
                    Cmd::SelectLayerPixels,
                );
                ui.separator();
                ui.menu_button("Modify", |ui| {
                    item(ui, app, "Border…", "", Cmd::ModifySelection(0));
                    item(ui, app, "Smooth…", "", Cmd::ModifySelection(1));
                    item(ui, app, "Expand…", "", Cmd::ModifySelection(2));
                    item(ui, app, "Contract…", "", Cmd::ModifySelection(3));
                    item(ui, app, "Feather…", "Shift+F6", Cmd::ModifySelection(4));
                });
                item(ui, app, "Grow", "", Cmd::Grow);
                item(ui, app, "Similar", "", Cmd::Similar);
            });
            ui.menu_button("Filter", |ui| {
                let last = app
                    .last_filter
                    .as_ref()
                    .map(|f| format!("Last Filter ({})", f.name()))
                    .unwrap_or_else(|| "Last Filter".into());
                ui.add_enabled_ui(app.last_filter.is_some(), |ui| {
                    item(ui, app, &last, "Ctrl+Alt+F", Cmd::LastFilter)
                });
                ui.separator();
                item(
                    ui,
                    app,
                    "Liquify (tool)",
                    "Q",
                    Cmd::SelectTool(Tool::Liquify),
                );
                item(ui, app, "Content-Aware Fill", "", Cmd::ContentAwareFill);
                ui.separator();
                for (group, fs) in Filter::menu() {
                    ui.menu_button(group, |ui| {
                        for f in fs {
                            let n = if f.has_params() {
                                format!("{}…", f.name())
                            } else {
                                f.name().to_string()
                            };
                            item(ui, app, &n, "", Cmd::Filter(f));
                        }
                    });
                }
            });
            ui.menu_button("View", |ui| {
                item(ui, app, "Zoom In", "Ctrl++", Cmd::ZoomIn);
                item(ui, app, "Zoom Out", "Ctrl+-", Cmd::ZoomOut);
                item(ui, app, "Fit on Screen", "Ctrl+0", Cmd::FitScreen);
                item(ui, app, "100%", "Ctrl+1", Cmd::ActualPixels);
                ui.separator();
                let pg = if app.show_pixel_grid {
                    "✔ Pixel Grid"
                } else {
                    "   Pixel Grid"
                };
                item(ui, app, pg, "", Cmd::TogglePixelGrid);
                let gd = if app.show_guides {
                    "✔ Guides"
                } else {
                    "   Guides"
                };
                item(ui, app, gd, "", Cmd::ToggleGuides);
                item(
                    ui,
                    app,
                    "New Vertical Guide (center)",
                    "",
                    Cmd::AddGuide(true),
                );
                item(
                    ui,
                    app,
                    "New Horizontal Guide (center)",
                    "",
                    Cmd::AddGuide(false),
                );
                item(ui, app, "Clear Guides", "", Cmd::ClearGuides);
                ui.separator();
                if ui
                    .button(if app.dark {
                        "Light Theme"
                    } else {
                        "Dark Theme"
                    })
                    .clicked()
                {
                    app.dark = !app.dark;
                    crate::theme::apply(ui.ctx(), app.dark);
                    ui.close();
                }
            });
            ui.menu_button("Window", |ui| {
                item(ui, app, "Command Palette", "Ctrl+K", Cmd::CommandPalette);
                item(ui, app, "Take History Snapshot", "", Cmd::TakeSnapshot);
                let rec = if app.recording.is_some() {
                    "■ Stop Recording"
                } else {
                    "● Record Action"
                };
                item(ui, app, rec, "", Cmd::RecordToggle);
            });
            ui.menu_button("Help", |ui| {
                item(ui, app, "Keyboard Shortcuts", "F1", Cmd::Shortcuts);
                item(ui, app, "About Kroma", "", Cmd::About);
            });
        });
    });
}

pub fn options_bar(app: &mut KromaApp, ui: &mut Ui) {
    egui::Panel::top("options").show(ui, |ui| {
        ui.add_space(2.0);
        let mut acts = Vec::new();
        let tool = app.tool;
        crate::tools::options_bar(ui, tool, &mut app.opts, &mut acts);
        app.pending.extend(acts);
        ui.add_space(2.0);
    });
}

pub fn status_bar(app: &mut KromaApp, ui: &mut Ui) {
    egui::Panel::bottom("status").show(ui, |ui| {
        ui.horizontal(|ui| {
            if let Some(t) = app.tabs.get_mut(app.active_tab) {
                let mut z = t.zoom * 100.0;
                if ui
                    .add(
                        egui::DragValue::new(&mut z)
                            .range(1.0..=6400.0)
                            .suffix("%")
                            .speed(1.0),
                    )
                    .changed()
                {
                    let rect = ui
                        .ctx()
                        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("canvas_rect")))
                        .unwrap_or(ERect::ZERO);
                    t.zoom_about(rect, rect.center(), z / 100.0);
                }
                ui.separator();
                ui.label(format!("{} × {} px", t.doc.width, t.doc.height));
                ui.separator();
                if let Some((x, y)) = app.cursor_doc {
                    if x >= 0.0 && y >= 0.0 && x < t.doc.width as f32 && y < t.doc.height as f32 {
                        let p = t.doc.sample_merged(x as u32, y as u32);
                        let c = kroma_core::Color::rgba(p[0], p[1], p[2], p[3]);
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, c32(c));
                        ui.label(format!(
                            "X {:>5}  Y {:>5}   {}  α {:.0}%",
                            x as i32,
                            y as i32,
                            c.to_hex(),
                            p[3] * 100.0
                        ));
                    } else {
                        ui.label(format!("X {:>5}  Y {:>5}", x as i32, y as i32));
                    }
                }
                ui.separator();
            }
            if let Some(r) = &app.recording {
                ui.label(
                    RichText::new(format!("● REC {} ({} steps)", r.name, r.ops.len()))
                        .color(Color32::from_rgb(240, 70, 70)),
                );
                ui.separator();
            }
            if let Some(b) = &app.batch {
                let b = b.lock().unwrap();
                if b.finished {
                    ui.label(format!(
                        "Batch done: {}/{} ({} errors)",
                        b.done,
                        b.total,
                        b.errors.len()
                    ));
                } else {
                    ui.add(
                        egui::ProgressBar::new(b.done as f32 / b.total.max(1) as f32)
                            .desired_width(140.0)
                            .text(format!("Batch {}/{}", b.done, b.total)),
                    );
                }
                ui.separator();
            }
            let msg = if app.status_time < 6.0 {
                app.status.clone()
            } else {
                app.tool.hint().to_string()
            };
            ui.label(RichText::new(msg).weak());
        });
    });
}

pub fn toolbox(app: &mut KromaApp, ui: &mut Ui) {
    egui::Panel::left("toolbox")
        .resizable(false)
        .exact_size(46.0)
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for g in Tool::GROUPS.iter() {
                    let shown = if g.contains(&app.tool) {
                        app.tool
                    } else {
                        g[0]
                    };
                    let selected = g.contains(&app.tool);
                    let sc = shown
                        .shortcut()
                        .map(|k| format!("  ({})", k.name()))
                        .unwrap_or_default();
                    let more = if g.len() > 1 {
                        "\nRight-click for more · Shift+key cycles"
                    } else {
                        ""
                    };
                    let r = crate::icons::tool_button(ui, shown, selected, 32.0)
                        .on_hover_text(format!("{}{}{}", shown.name(), sc, more));
                    if r.clicked() {
                        app.pending.push(Cmd::SelectTool(shown));
                    }
                    if g.len() > 1 {
                        let p = r.rect.right_bottom() - Vec2::new(2.0, 2.0);
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![p, p - Vec2::new(4.0, 0.0), p - Vec2::new(0.0, 4.0)],
                            Color32::GRAY,
                            Stroke::NONE,
                        ));
                        r.context_menu(|ui| {
                            for t in g.iter() {
                                ui.horizontal(|ui| {
                                    let (ir, _) =
                                        ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                                    crate::icons::paint(
                                        ui.painter(),
                                        ir,
                                        *t,
                                        ui.visuals().text_color(),
                                    );
                                    if ui.button(t.name()).clicked() {
                                        app.pending.push(Cmd::SelectTool(*t));
                                        ui.close();
                                    }
                                });
                            }
                        });
                    }
                }
            });
            ui.add_space(10.0);
            // foreground / background swatches
            let (rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 38.0), Sense::hover());
            let fg_r = ERect::from_min_size(rect.min + Vec2::new(3.0, 2.0), Vec2::splat(22.0));
            let bg_r = ERect::from_min_size(rect.min + Vec2::new(15.0, 14.0), Vec2::splat(22.0));
            let rb = ui.interact(bg_r, ui.id().with("bgsw"), Sense::click());
            ui.painter().rect_filled(bg_r, 2.0, c32(app.bg));
            ui.painter().rect_stroke(
                bg_r,
                2.0,
                Stroke::new(1.0, Color32::GRAY),
                egui::StrokeKind::Outside,
            );
            let rf = ui.interact(fg_r, ui.id().with("fgsw"), Sense::click());
            ui.painter().rect_filled(fg_r, 2.0, c32(app.fg));
            ui.painter().rect_stroke(
                fg_r,
                2.0,
                Stroke::new(1.0, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
            if rf.clicked() {
                app.dialog = Some(crate::dialogs::Dialog::ColorPicker {
                    background: false,
                    hsva: egui::ecolor::Hsva::from_srgb([
                        app.fg.to_rgba8()[0],
                        app.fg.to_rgba8()[1],
                        app.fg.to_rgba8()[2],
                    ]),
                });
            }
            if rb.clicked() {
                app.dialog = Some(crate::dialogs::Dialog::ColorPicker {
                    background: true,
                    hsva: egui::ecolor::Hsva::from_srgb([
                        app.bg.to_rgba8()[0],
                        app.bg.to_rgba8()[1],
                        app.bg.to_rgba8()[2],
                    ]),
                });
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                let col = ui.visuals().text_color();
                // swap: two arrows
                let p = ui.painter();
                p.line_segment(
                    [
                        r.left_top() + Vec2::new(4.0, 6.0),
                        r.right_top() + Vec2::new(-4.0, 6.0),
                    ],
                    Stroke::new(1.2, col),
                );
                p.line_segment(
                    [
                        r.right_top() + Vec2::new(-7.0, 3.0),
                        r.right_top() + Vec2::new(-4.0, 6.0),
                    ],
                    Stroke::new(1.2, col),
                );
                p.line_segment(
                    [
                        r.left_bottom() + Vec2::new(4.0, -6.0),
                        r.right_bottom() + Vec2::new(-4.0, -6.0),
                    ],
                    Stroke::new(1.2, col),
                );
                p.line_segment(
                    [
                        r.left_bottom() + Vec2::new(7.0, -3.0),
                        r.left_bottom() + Vec2::new(4.0, -6.0),
                    ],
                    Stroke::new(1.2, col),
                );
                if resp.on_hover_text("Swap colors (X)").clicked() {
                    app.pending.push(Cmd::SwapColors);
                }
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(
                    ERect::from_min_size(r.min + Vec2::new(7.0, 7.0), Vec2::splat(8.0)),
                    0.0,
                    Color32::WHITE,
                );
                p.rect_filled(
                    ERect::from_min_size(r.min + Vec2::new(3.0, 3.0), Vec2::splat(8.0)),
                    0.0,
                    Color32::BLACK,
                );
                p.rect_stroke(
                    ERect::from_min_size(r.min + Vec2::new(3.0, 3.0), Vec2::splat(8.0)),
                    0.0,
                    Stroke::new(1.0, Color32::GRAY),
                    egui::StrokeKind::Outside,
                );
                if resp.on_hover_text("Default colors (D)").clicked() {
                    app.pending.push(Cmd::DefaultColors);
                }
            });
            ui.add_space(6.0);
            if let Some(t) = app.tabs.get(app.active_tab) {
                let has_mask = t
                    .doc
                    .active_layer()
                    .map(|l| l.mask().is_some())
                    .unwrap_or(false);
                if has_mask {
                    let lbl = if t.editing_mask { "Mask" } else { "Pixels" };
                    if ui
                        .small_button(lbl)
                        .on_hover_text("Paint target: layer pixels or its mask")
                        .clicked()
                    {
                        let on = !t.editing_mask;
                        app.pending.push(Cmd::EditMask(on));
                    }
                }
            }
        });
}

pub fn doc_tabs(app: &mut KromaApp, ui: &mut Ui) {
    if app.tabs.is_empty() {
        return;
    }
    egui::Frame::NONE
        .fill(ui.visuals().panel_fill)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut close = None;
                for (i, t) in app.tabs.iter().enumerate() {
                    let label = format!("{}  {:.0}%", t.title(), t.zoom * 100.0);
                    if ui.selectable_label(i == app.active_tab, label).clicked() {
                        app.active_tab = i;
                    }
                    if ui.small_button("×").on_hover_text("Close").clicked() {
                        close = Some(i);
                    }
                    ui.separator();
                }
                if let Some(i) = close {
                    if app.tabs[i].dirty {
                        app.dialog = Some(crate::dialogs::Dialog::ConfirmClose {
                            tab: i,
                            quitting: false,
                        });
                    } else {
                        app.close_tab(i);
                    }
                }
            });
        });
}

// ------------------------------------------------------------------ right dock

pub fn right_panels(app: &mut KromaApp, ui: &mut Ui) {
    egui::Panel::right("dock")
        .resizable(true)
        .default_size(300.0)
        .min_size(240.0)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::CollapsingHeader::new(RichText::new("Color").strong())
                        .default_open(true)
                        .show(ui, |ui| color_panel(app, ui));
                    egui::CollapsingHeader::new(RichText::new("Navigator").strong())
                        .default_open(false)
                        .show(ui, |ui| navigator(app, ui));
                    egui::CollapsingHeader::new(RichText::new("Histogram").strong())
                        .default_open(false)
                        .show(ui, |ui| histogram_panel(app, ui));
                    egui::CollapsingHeader::new(RichText::new("Properties").strong())
                        .default_open(true)
                        .show(ui, |ui| properties(app, ui));
                    egui::CollapsingHeader::new(RichText::new("Layers").strong())
                        .default_open(true)
                        .show(ui, |ui| layers_panel(app, ui));
                    egui::CollapsingHeader::new(RichText::new("History").strong())
                        .default_open(true)
                        .show(ui, |ui| history_panel(app, ui));
                    egui::CollapsingHeader::new(RichText::new("Actions").strong())
                        .default_open(false)
                        .show(ui, |ui| actions_panel(app, ui));
                });
        });
}

fn color_panel(app: &mut KromaApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label("FG");
        color_button(ui, &mut app.fg);
        let mut hex = app.fg.to_hex();
        if ui
            .add(egui::TextEdit::singleline(&mut hex).desired_width(70.0))
            .changed()
        {
            if let Some(c) = kroma_core::Color::from_hex(&hex) {
                app.fg = c;
            }
        }
        ui.label("BG");
        color_button(ui, &mut app.bg);
    });
    // quick HSV square for the foreground
    let mut c = Color32::from_rgb(
        app.fg.to_rgba8()[0],
        app.fg.to_rgba8()[1],
        app.fg.to_rgba8()[2],
    );
    if egui::widgets::color_picker::color_picker_color32(
        ui,
        &mut c,
        egui::widgets::color_picker::Alpha::Opaque,
    ) {
        app.fg = kroma_core::Color::from_rgba8(c.r(), c.g(), c.b(), 255);
    }
    ui.label(
        RichText::new("Swatches (click = FG, right-click = BG)")
            .small()
            .weak(),
    );
    ui.horizontal_wrapped(|ui| {
        let mut remove = None;
        for (i, s) in app.swatches.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
            ui.painter().rect_filled(r, 2.0, c32(*s));
            if resp.clicked() {
                app.fg = *s;
            }
            if resp.secondary_clicked() {
                app.bg = *s;
            }
            if resp.middle_clicked() {
                remove = Some(i);
            }
        }
        if ui
            .small_button("+")
            .on_hover_text("Add foreground to swatches (middle-click a swatch to remove)")
            .clicked()
        {
            app.swatches.push(app.fg);
        }
        if let Some(i) = remove {
            app.swatches.remove(i);
        }
    });
}

fn navigator(app: &mut KromaApp, ui: &mut Ui) {
    let canvas = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new("canvas_rect")))
        .unwrap_or(ERect::ZERO);
    let Some(t) = app.tabs.get_mut(app.active_tab) else {
        return;
    };
    let w = ui.available_width();
    let s = w / t.doc.width.max(t.doc.height) as f32;
    let size = Vec2::new(t.doc.width as f32 * s, t.doc.height as f32 * s);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    for tile in &t.tiles {
        let r = ERect::from_min_size(
            rect.min + Vec2::new(tile.rect.x as f32 * s, tile.rect.y as f32 * s),
            Vec2::new(tile.rect.w as f32 * s, tile.rect.h as f32 * s),
        );
        ui.painter().image(
            tile.tex.id(),
            r,
            ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    // visible area
    let (x0, y0) = t.screen_to_doc(canvas, canvas.min);
    let (x1, y1) = t.screen_to_doc(canvas, canvas.max);
    let vr = ERect::from_min_max(
        rect.min + Vec2::new(x0 * s, y0 * s),
        rect.min + Vec2::new(x1 * s, y1 * s),
    )
    .intersect(rect);
    ui.painter().rect_stroke(
        vr,
        0.0,
        Stroke::new(1.5, Color32::RED),
        egui::StrokeKind::Middle,
    );
    if resp.dragged() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            let (dx, dy) = ((p.x - rect.min.x) / s, (p.y - rect.min.y) / s);
            let center = canvas.center() - canvas.min;
            t.offset = center - Vec2::new(dx * t.zoom, dy * t.zoom);
        }
    }
    let mut z = t.zoom;
    if ui
        .add(
            egui::Slider::new(&mut z, 0.01..=32.0)
                .logarithmic(true)
                .text("zoom"),
        )
        .changed()
    {
        t.zoom_about(canvas, canvas.center(), z);
    }
}

fn histogram_panel(app: &mut KromaApp, ui: &mut Ui) {
    let Some(t) = app.tabs.get(app.active_tab) else {
        return;
    };
    let key = (t.id, t.history.cursor());
    let stale = app
        .hist_cache
        .as_ref()
        .map(|(id, c, _)| (*id, *c) != key)
        .unwrap_or(true);
    if stale {
        let comp = t.doc.composite();
        let max_edge = 512u32;
        let scale = (max_edge as f32 / comp.width.max(comp.height) as f32).min(1.0);
        let (nw, nh) = (
            ((comp.width as f32 * scale) as u32).max(1),
            ((comp.height as f32 * scale) as u32).max(1),
        );
        let proxy =
            kroma_core::transform::resize(&comp, nw, nh, kroma_core::transform::Resample::Bilinear);
        let h = Histogram::of(&proxy);
        app.hist_cache = Some((key.0, key.1, h));
    }
    if let Some((_, _, h)) = &app.hist_cache {
        histogram_view(ui, h, 90.0);
        ui.label(
            RichText::new(format!(
                "Mean luminosity {:.0}   Pixels {}",
                h.mean_luma() * 255.0,
                h.count
            ))
            .small(),
        );
    }
}

fn properties(app: &mut KromaApp, ui: &mut Ui) {
    let pointer_down = ui.ctx().input(|i| i.pointer.any_down());
    let Some(t) = app.tabs.get_mut(app.active_tab) else {
        return;
    };
    let (w, h) = (t.doc.width, t.doc.height);
    let pending_id = egui::Id::new(("props_dirty", t.id));
    let mut changed = false;
    let hist_needed = matches!(
        t.doc.active_layer().map(|l| &l.kind),
        Some(LayerKind::Adjustment(
            Adjustment::Levels { .. } | Adjustment::Curves { .. } | Adjustment::Threshold { .. }
        ))
    );
    let hist = if hist_needed {
        Some(Histogram::of(&t.doc.composite()))
    } else {
        None
    };
    let Some(l) = t.doc.active_layer_mut() else {
        return;
    };
    ui.label(RichText::new(format!("{} layer: {}", l.kind_label(), l.name)).weak());
    match &mut l.kind {
        LayerKind::Adjustment(adj) => {
            ui.label(RichText::new(adj.name()).strong());
            changed |= adjustment_ui(ui, adj, hist.as_ref());
        }
        LayerKind::Text(style) => {
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut style.text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            ui.horizontal_wrapped(|ui| {
                changed |= text_options(ui, style);
            });
            ui.horizontal(|ui| {
                ui.label("X");
                changed |= ui.add(egui::DragValue::new(&mut style.x)).changed();
                ui.label("Y");
                changed |= ui.add(egui::DragValue::new(&mut style.y)).changed();
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
            }
        }
        LayerKind::Fill(src) => match src {
            FillSource::Solid(c) => {
                ui.horizontal(|ui| {
                    ui.label("Color");
                    changed |= color_button(ui, c);
                });
            }
            FillSource::Gradient { fill, from, to } => {
                changed |= crate::widgets::gradient_editor(ui, &mut fill.gradient, "fill_layer");
                egui::ComboBox::from_id_salt("fill_shape")
                    .selected_text(fill.shape.name())
                    .show_ui(ui, |ui| {
                        for s in kroma_core::gradient::GradientShape::ALL {
                            changed |= ui.selectable_value(&mut fill.shape, s, s.name()).changed();
                        }
                    });
                ui.horizontal(|ui| {
                    ui.label("From");
                    changed |= ui.add(egui::DragValue::new(&mut from.0)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut from.1)).changed();
                });
                ui.horizontal(|ui| {
                    ui.label("To");
                    changed |= ui.add(egui::DragValue::new(&mut to.0)).changed();
                    changed |= ui.add(egui::DragValue::new(&mut to.1)).changed();
                });
            }
        },
        LayerKind::Group { pass_through, .. } => {
            changed |= ui
                .checkbox(
                    pass_through,
                    "Pass Through (group does not isolate blending)",
                )
                .changed();
        }
        LayerKind::Raster => {
            let b = l.image().opaque_bounds(0.004);
            ui.label(format!(
                "Content bounds: {}×{} at ({}, {})",
                b.w, b.h, b.x, b.y
            ));
        }
    }
    if l.mask().is_some() {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Layer mask");
            if ui.small_button("Invert").clicked() {
                app.pending.push(Cmd::InvertMask);
            }
            if ui
                .small_button(if l.mask_enabled { "Disable" } else { "Enable" })
                .clicked()
            {
                app.pending.push(Cmd::ToggleMaskEnabled);
            }
            if ui.small_button("Apply").clicked() {
                app.pending.push(Cmd::ApplyMask);
            }
            if ui.small_button("Delete").clicked() {
                app.pending.push(Cmd::DeleteMask);
            }
        });
    }
    if changed {
        l.rerender(w, h);
        l.touch();
        t.full_redraw = true;
        ui.ctx().data_mut(|d| d.insert_temp(pending_id, true));
    }
    let pending: bool = ui.ctx().data(|d| d.get_temp(pending_id).unwrap_or(false));
    if pending && !pointer_down {
        ui.ctx().data_mut(|d| d.insert_temp(pending_id, false));
        t.commit("Properties");
    }
}

fn layers_panel(app: &mut KromaApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let pointer_down = ctx.input(|i| i.pointer.any_down());
    let Some(t) = app.tabs.get_mut(app.active_tab) else {
        return;
    };
    let tid = t.id;
    let pending_id = egui::Id::new(("layer_props_dirty", tid));
    // blend / opacity / fill of active layer
    let mut changed = false;
    if let Some(l) = t.doc.active_layer_mut() {
        ui.horizontal(|ui| {
            changed |= blend_combo(ui, ("layer_blend", tid), &mut l.blend);
            ui.label("Opacity");
            let mut o = l.opacity * 100.0;
            if ui
                .add(egui::DragValue::new(&mut o).range(0.0..=100.0).suffix("%"))
                .changed()
            {
                l.opacity = o / 100.0;
                changed = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Lock:");
            changed |= ui
                .checkbox(&mut l.locks.transparency, "Transparency")
                .on_hover_text("Lock transparent pixels")
                .changed();
            ui.label("  Fill");
            let mut f = l.fill * 100.0;
            if ui
                .add(egui::DragValue::new(&mut f).range(0.0..=100.0).suffix("%"))
                .on_hover_text("Fill opacity (does not fade layer styles)")
                .changed()
            {
                l.fill = f / 100.0;
                changed = true;
            }
        });
        if changed {
            l.touch();
        }
    }
    // keep brush lock-alpha in sync with layer lock
    if let Some(l) = t.doc.active_layer() {
        let lock = l.locks.transparency;
        app.opts.brush.lock_alpha = lock;
        app.opts.pencil.lock_alpha = lock;
    }
    let t = app.tabs.get_mut(app.active_tab).unwrap();
    if changed {
        t.full_redraw = true;
        ctx.data_mut(|d| d.insert_temp(pending_id, true));
    }
    ui.separator();
    // layer list (top to bottom)
    let rows: Vec<(
        usize,
        LayerId,
        String,
        bool,
        bool,
        bool,
        bool,
        bool,
        &'static str,
        bool,
    )> = t
        .doc
        .flat_list()
        .into_iter()
        .map(|(d, l)| {
            (
                d,
                l.id,
                l.name.clone(),
                l.visible,
                l.mask().is_some(),
                l.clipped,
                l.has_active_effects(),
                l.is_group(),
                l.kind_label(),
                l.mask_enabled,
            )
        })
        .collect();
    let mut select: Option<LayerId> = None;
    let mut toggle_vis: Option<LayerId> = None;
    let mut toggle_expand: Option<LayerId> = None;
    let mut rename: Option<(LayerId, String)> = None;
    let mut mask_click: Option<(LayerId, bool)> = None;
    let mut load_sel: Option<LayerId> = None;
    egui::ScrollArea::vertical()
        .id_salt("layers_scroll")
        .max_height(320.0)
        .show(ui, |ui| {
            for (depth, id, name, visible, has_mask, clipped, fx, group, kind, mask_on) in rows {
                let active = id == t.doc.active;
                let fill = if active {
                    ui.visuals().selection.bg_fill.gamma_multiply(0.6)
                } else {
                    Color32::TRANSPARENT
                };
                egui::Frame::NONE
                    .fill(fill)
                    .inner_margin(egui::Margin::symmetric(2, 1))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(depth as f32 * 14.0);
                            if ui
                                .add(
                                    egui::Button::new(if visible { "👁" } else { "  " })
                                        .frame(false)
                                        .min_size(Vec2::new(18.0, 18.0)),
                                )
                                .on_hover_text("Show/Hide")
                                .clicked()
                            {
                                toggle_vis = Some(id);
                            }
                            if clipped {
                                ui.label(RichText::new("clip").small().weak());
                            }
                            if group {
                                let exp = t
                                    .doc
                                    .layer(id)
                                    .and_then(|l| {
                                        if let LayerKind::Group { expanded, .. } = &l.kind {
                                            Some(*expanded)
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or(true);
                                if ui
                                    .add(
                                        egui::Button::new(if exp {
                                            "▾ Group"
                                        } else {
                                            "▸ Group"
                                        })
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    toggle_expand = Some(id);
                                }
                            } else if let Some(tex) = t.thumb(&ctx, id, 36) {
                                let r = ui.add(
                                    egui::Image::new((tex, Vec2::new(30.0, 30.0)))
                                        .sense(Sense::click()),
                                );
                                if r.clicked() {
                                    if ctx.input(|i| i.modifiers.command) {
                                        load_sel = Some(id);
                                    } else {
                                        mask_click = Some((id, false));
                                    }
                                }
                                let editing_pixels = active && !t.editing_mask;
                                if editing_pixels {
                                    ui.painter().rect_stroke(
                                        r.rect.expand(1.0),
                                        0.0,
                                        Stroke::new(1.0, Color32::WHITE),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                            } else {
                                ui.label(
                                    RichText::new(match kind {
                                        "Adjustment" => "◐",
                                        _ => "▣",
                                    })
                                    .size(18.0),
                                );
                            }
                            if has_mask && !group {
                                // mask thumbnail: show as small gray square
                                let (r, resp) =
                                    ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::click());
                                ui.painter().rect_filled(
                                    r,
                                    2.0,
                                    if mask_on {
                                        Color32::from_gray(200)
                                    } else {
                                        Color32::from_rgb(160, 40, 40)
                                    },
                                );
                                ui.painter().rect_filled(
                                    r.shrink(6.0),
                                    1.0,
                                    Color32::from_gray(40),
                                );
                                if active && t.editing_mask {
                                    ui.painter().rect_stroke(
                                        r.expand(1.0),
                                        0.0,
                                        Stroke::new(1.0, Color32::WHITE),
                                        egui::StrokeKind::Outside,
                                    );
                                }
                                if resp.on_hover_text("Click: paint on mask").clicked() {
                                    mask_click = Some((id, true));
                                }
                            }
                            let label = ui.add(
                                egui::Label::new(if active {
                                    RichText::new(&name).strong()
                                } else {
                                    RichText::new(&name)
                                })
                                .sense(Sense::click())
                                .truncate(),
                            );
                            if label.clicked() {
                                select = Some(id);
                            }
                            if label.double_clicked() {
                                rename = Some((id, name.clone()));
                            }
                            if fx {
                                ui.label(RichText::new("fx").italics().small());
                            }
                        });
                    });
            }
        });
    if let Some(id) = select {
        t.doc.active = id;
        t.editing_mask = false;
    }
    if let Some((id, mask)) = mask_click {
        t.doc.active = id;
        t.editing_mask = mask;
    }
    if let Some(id) = load_sel {
        if let Some(l) = t.doc.layer(id) {
            t.doc.selection = Some(kroma_core::Mask::from_alpha(&l.masked_content()));
            t.commit("Load Selection");
        }
    }
    if let Some(id) = toggle_vis {
        if let Some(l) = t.doc.layer_mut(id) {
            l.visible = !l.visible;
        }
        t.commit("Toggle Visibility");
    }
    if let Some(id) = toggle_expand {
        if let Some(l) = t.doc.layer_mut(id) {
            if let LayerKind::Group { expanded, .. } = &mut l.kind {
                *expanded = !*expanded;
            }
        }
    }
    if let Some((id, name)) = rename {
        app.dialog = Some(crate::dialogs::Dialog::Rename { id, name });
    }
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.menu_button("fx", |ui| {
            if ui.button("Blending Options / Styles…").clicked() {
                app.pending.push(Cmd::LayerStyle);
                ui.close();
            }
            for e in Effect::catalog() {
                if ui.button(e.name()).clicked() {
                    if let Some(t) = app.tabs.get_mut(app.active_tab) {
                        if let Some(l) = t.doc.active_layer_mut() {
                            if l.is_raster_like() && !l.effects.iter().any(|x| x.name() == e.name())
                            {
                                l.effects.push(e.clone());
                                l.touch();
                            }
                        }
                    }
                    app.pending.push(Cmd::LayerStyle);
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("Layer styles");
        if ui
            .button("Mask")
            .on_hover_text("Add layer mask (from selection if any)")
            .clicked()
        {
            let has_sel = app
                .tabs
                .get(app.active_tab)
                .map(|t| t.doc.selection.is_some())
                .unwrap_or(false);
            app.pending.push(Cmd::AddMask {
                from_selection: has_sel,
                hide: false,
            });
        }
        ui.menu_button("Adj", |ui| {
            if ui.button("Solid Color…").clicked() {
                app.pending.push(Cmd::NewFillSolid);
                ui.close();
            }
            if ui.button("Gradient…").clicked() {
                app.pending.push(Cmd::NewFillGradient);
                ui.close();
            }
            ui.separator();
            for a in Adjustment::catalog() {
                if ui.button(a.name()).clicked() {
                    app.pending.push(Cmd::NewAdjustmentLayer(a));
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text("New fill or adjustment layer");
        if ui.button("Group").on_hover_text("Group (Ctrl+G)").clicked() {
            app.pending.push(Cmd::Group);
        }
        if ui
            .button("New")
            .on_hover_text("New layer (Ctrl+Shift+N)")
            .clicked()
        {
            app.pending.push(Cmd::NewLayer);
        }
        if ui.button("Dup").on_hover_text("Duplicate layer").clicked() {
            app.pending.push(Cmd::DuplicateLayer);
        }
        if ui.button("Up").on_hover_text("Move up (Ctrl+])").clicked() {
            app.pending.push(Cmd::Arrange(1));
        }
        if ui
            .button("Down")
            .on_hover_text("Move down (Ctrl+[)")
            .clicked()
        {
            app.pending.push(Cmd::Arrange(-1));
        }
        if ui
            .button("Clip")
            .on_hover_text("Clipping mask (Ctrl+Alt+G)")
            .clicked()
        {
            app.pending.push(Cmd::ToggleClip);
        }
        if ui.button("Delete").on_hover_text("Delete layer").clicked() {
            app.pending.push(Cmd::DeleteLayer);
        }
    });
    let pending: bool = ctx.data(|d| d.get_temp(pending_id).unwrap_or(false));
    if pending && !pointer_down {
        ctx.data_mut(|d| d.insert_temp(pending_id, false));
        if let Some(t) = app.tabs.get_mut(app.active_tab) {
            t.commit("Layer Properties");
        }
    }
}

fn history_panel(app: &mut KromaApp, ui: &mut Ui) {
    let Some(t) = app.tabs.get_mut(app.active_tab) else {
        return;
    };
    let names: Vec<String> = t.history.names().map(|s| s.to_string()).collect();
    let cur = t.history.cursor();
    let mut go = None;
    egui::ScrollArea::vertical()
        .id_salt("hist_scroll")
        .max_height(180.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for (i, n) in names.iter().enumerate() {
                let txt = if i > cur {
                    RichText::new(n).weak().italics()
                } else {
                    RichText::new(n)
                };
                if ui.selectable_label(i == cur, txt).clicked() {
                    go = Some(i);
                }
            }
        });
    if let Some(i) = go {
        t.goto_history(i);
    }
    ui.horizontal(|ui| {
        if ui.small_button("📷 Snapshot").clicked() {
            app.pending.push(Cmd::TakeSnapshot);
        }
    });
    let snaps: Vec<String> = t.history.snapshots.iter().map(|s| s.name.clone()).collect();
    let mut restore = None;
    for (i, s) in snaps.iter().enumerate() {
        if ui.link(format!("↺ {s}")).clicked() {
            restore = Some(i);
        }
    }
    if let Some(i) = restore {
        if let Some(d) = t.history.restore_snapshot(i) {
            t.doc = d;
            t.full_redraw = true;
            t.dirty = true;
        }
    }
}

fn actions_panel(app: &mut KromaApp, ui: &mut Ui) {
    let mut run = None;
    let mut remove = None;
    let builtin = kroma_core::ops::Action::builtins().len();
    for (i, a) in app.actions.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui
                .small_button("▶")
                .on_hover_text("Play on current document")
                .clicked()
            {
                run = Some(i);
            }
            ui.label(format!("{} ({} steps)", a.name, a.ops.len()))
                .on_hover_text(
                    a.ops
                        .iter()
                        .map(|o| o.label())
                        .collect::<Vec<_>>()
                        .join("\n"),
                );
            if i >= builtin && ui.small_button("🗑").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = run {
        app.pending.push(Cmd::RunAction(i));
    }
    if let Some(i) = remove {
        app.actions.remove(i);
    }
    ui.horizontal_wrapped(|ui| {
        let rec = app.recording.is_some();
        if ui.button(if rec { "■ Stop" } else { "● Record" }).clicked() {
            app.pending.push(Cmd::RecordToggle);
        }
        if let Some(r) = &mut app.recording {
            ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(110.0));
        }
        if ui.button("Batch…").clicked() {
            app.pending.push(Cmd::BatchProcess);
        }
        if ui.button("Load JSON…").clicked() {
            if let Some(p) = rfd::FileDialog::new()
                .add_filter("Kroma action", &["json"])
                .pick_file()
            {
                match kroma_core::ops::Action::load(&p) {
                    Ok(a) => app.actions.push(a),
                    Err(e) => app.toast(format!("Could not load action: {e}")),
                }
            }
        }
        if ui.button("Save last…").clicked() {
            if let Some(a) = app.actions.last() {
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name(format!("{}.json", a.name))
                    .save_file()
                {
                    let _ = a.save(&p);
                }
            }
        }
    });
}

// ------------------------------------------------------------------ command palette

pub fn command_palette(app: &mut KromaApp, ctx: &egui::Context) {
    if !app.palette_open {
        return;
    }
    let screen = ctx.content_rect();
    let mut run: Option<Cmd> = None;
    egui::Window::new("Command Palette")
        .title_bar(false)
        .fixed_pos(Pos2::new(screen.center().x - 280.0, screen.top() + 80.0))
        .fixed_size(Vec2::new(560.0, 420.0))
        .show(ctx, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut app.palette_query)
                    .hint_text("Type a command, tool, adjustment or filter…")
                    .desired_width(f32::INFINITY),
            );
            r.request_focus();
            let q = app.palette_query.to_lowercase();
            let words: Vec<&str> = q.split_whitespace().collect();
            let mut matches: Vec<(String, Cmd, &str)> = all_commands()
                .into_iter()
                .filter(|(n, _, _)| {
                    let n = n.to_lowercase();
                    words.iter().all(|w| n.contains(w))
                })
                .collect();
            matches.truncate(60);
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if enter {
                if let Some((_, c, _)) = matches.first() {
                    run = Some(c.clone());
                }
            }
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, (n, c, sc)) in matches.iter().enumerate() {
                    let b = egui::Button::new(if i == 0 {
                        RichText::new(n).strong()
                    } else {
                        RichText::new(n)
                    })
                    .shortcut_text(*sc)
                    .frame(i == 0);
                    if ui.add_sized([ui.available_width(), 22.0], b).clicked() {
                        run = Some(c.clone());
                    }
                }
            });
        });
    if let Some(c) = run {
        app.palette_open = false;
        app.pending.push(c);
    }
}
