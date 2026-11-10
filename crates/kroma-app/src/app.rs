//! Application shell: state, commands (menus, shortcuts, command palette), layout.

use crate::dialogs::Dialog;
use crate::tab::{checker_texture, DocTab};
use crate::tools::{Tool, ToolOptions};
use egui::{Key, KeyboardShortcut, Modifiers, TextureHandle};
use kroma_core::adjust::{auto_color, auto_contrast, auto_tone, Adjustment, Histogram};
use kroma_core::buffer::{Image, Mask, Rect};
use kroma_core::filter::{Filter, FilterCtx};
use kroma_core::layer::{FillSource, Layer, LayerKind};
use kroma_core::ops::{Action, Op};
use kroma_core::{Color, Document};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub enum Cmd {
    // File
    NewDoc,
    Open,
    OpenPath(PathBuf),
    Place,
    Save,
    SaveAs,
    ExportAs,
    QuickExportPng,
    CloseTab,
    Revert,
    Quit,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    CopyMerged,
    Paste,
    PasteAsNewDoc,
    Clear,
    FillDialog,
    StrokeDialog,
    ContentAwareFill,
    FreeTransform,
    Distort,
    FlipLayer(bool),
    RotateLayer(i32),
    TakeSnapshot,
    Preferences,
    // Image
    Adjust(Adjustment),
    AutoTone,
    AutoContrast,
    AutoColor,
    QuickInvert,
    QuickDesaturate,
    ImageSize,
    CanvasSize,
    RotateCanvas(i32),
    RotateArbitrary,
    FlipCanvas(bool),
    CropToSelection,
    Trim,
    DuplicateDoc,
    // Layer
    NewLayer,
    LayerViaCopy,
    LayerViaCut,
    DuplicateLayer,
    DeleteLayer,
    RenameLayer,
    NewFillSolid,
    NewFillGradient,
    NewAdjustmentLayer(Adjustment),
    LayerStyle,
    ClearStyle,
    AddMask { from_selection: bool, hide: bool },
    DeleteMask,
    ApplyMask,
    ToggleMaskEnabled,
    InvertMask,
    EditMask(bool),
    ToggleClip,
    Group,
    Ungroup,
    Arrange(i32),
    Rasterize,
    MergeDown,
    MergeVisible,
    Flatten,
    SelectLayerPixels,
    // Select
    SelectAll,
    Deselect,
    Reselect,
    InvertSelection,
    SelectSubject,
    ColorRange,
    ModifySelection(u8),
    Grow,
    Similar,
    // Filter
    Filter(Filter),
    LastFilter,
    // View
    ZoomIn,
    ZoomOut,
    FitScreen,
    ActualPixels,
    TogglePixelGrid,
    ToggleGuides,
    AddGuide(bool),
    ClearGuides,
    // Misc
    CommandPalette,
    Shortcuts,
    About,
    SwapColors,
    DefaultColors,
    ApplyCrop,
    Cancel,
    Commit,
    Align(u8),
    RunAction(usize),
    RecordToggle,
    BatchProcess,
    SelectTool(Tool),
}

pub struct BatchJob {
    pub total: usize,
    pub done: usize,
    pub errors: Vec<String>,
    pub finished: bool,
}

pub struct KromaApp {
    pub tabs: Vec<DocTab>,
    pub active_tab: usize,
    pub tool: Tool,
    pub opts: ToolOptions,
    pub fg: Color,
    pub bg: Color,
    pub swatches: Vec<Color>,
    pub dialog: Option<Dialog>,
    pub palette_open: bool,
    pub palette_query: String,
    pub pending: Vec<Cmd>,
    pub checker: Option<TextureHandle>,
    pub clipboard: Option<(Image, Rect)>,
    pub last_filter: Option<Filter>,
    pub last_selection: Option<Mask>,
    pub status: String,
    pub status_time: f64,
    pub cursor_doc: Option<(f32, f32)>,
    pub pressure: f32,
    pub show_pixel_grid: bool,
    pub show_guides: bool,
    pub actions: Vec<Action>,
    pub recording: Option<Action>,
    pub batch: Option<Arc<Mutex<BatchJob>>>,
    pub hist_cache: Option<(u64, usize, Histogram)>,
    pub dark: bool,
    pub recent: Vec<PathBuf>,
    pub quit_requested: bool,
    pub allow_close: bool,
    pub space_down: bool,
    last_autosave: std::time::Instant,
}

const SETTINGS_KEY: &str = "kroma_settings_v1";

#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    opts: ToolOptions,
    fg: Color,
    bg: Color,
    swatches: Vec<Color>,
    actions: Vec<Action>,
    dark: bool,
    recent: Vec<PathBuf>,
}

impl KromaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>) -> Self {
        Self::build(&cc.egui_ctx, cc.storage, files)
    }

    pub fn build(
        egui_ctx: &egui::Context,
        storage: Option<&dyn eframe::Storage>,
        files: Vec<PathBuf>,
    ) -> Self {
        crate::theme::apply(egui_ctx, true);
        let mut app = KromaApp {
            tabs: Vec::new(),
            active_tab: 0,
            tool: Tool::Brush,
            opts: ToolOptions::default(),
            fg: Color::BLACK,
            bg: Color::WHITE,
            swatches: default_swatches(),
            dialog: None,
            palette_open: false,
            palette_query: String::new(),
            pending: Vec::new(),
            checker: None,
            clipboard: None,
            last_filter: None,
            last_selection: None,
            status: "Welcome to Kroma. Ctrl+K opens the command palette.".into(),
            status_time: 0.0,
            cursor_doc: None,
            pressure: 1.0,
            show_pixel_grid: true,
            show_guides: true,
            actions: Action::builtins(),
            recording: None,
            batch: None,
            hist_cache: None,
            dark: true,
            recent: Vec::new(),
            quit_requested: false,
            allow_close: false,
            space_down: false,
            last_autosave: std::time::Instant::now(),
        };
        if let Some(storage) = storage {
            if let Some(s) = eframe::get_value::<Settings>(storage, SETTINGS_KEY) {
                app.opts = s.opts;
                app.fg = s.fg;
                app.bg = s.bg;
                app.swatches = s.swatches;
                let mut acts = Action::builtins();
                for a in s.actions {
                    if !acts.iter().any(|b| b.name == a.name) {
                        acts.push(a);
                    }
                }
                app.actions = acts;
                app.dark = s.dark;
                app.recent = s.recent;
            }
        }
        crate::theme::apply(egui_ctx, app.dark);
        let open_files = !files.is_empty();
        for f in files {
            app.pending.push(Cmd::OpenPath(f));
        }
        if !open_files && std::env::var("KROMA_DEMO").is_err() {
            let entries = crate::recovery::scan();
            if !entries.is_empty() {
                app.dialog = Some(Dialog::Recovery { entries });
            }
        }
        if std::env::var("KROMA_DEMO").is_ok() {
            app.tabs.push(DocTab::new(crate::demo::showcase(), None));
            // KROMA_DEMO_CMD="Adjustment: Curves" opens a command at startup (screenshots / QA)
            if let Ok(name) = std::env::var("KROMA_DEMO_CMD") {
                if let Some((_, c, _)) = crate::panels::all_commands()
                    .into_iter()
                    .find(|(n, _, _)| n == &name)
                {
                    app.pending.push(c);
                }
            }
            if let Ok(t) = std::env::var("KROMA_DEMO_TOOL") {
                if let Some(tool) = crate::tools::Tool::GROUPS
                    .iter()
                    .flat_map(|g| g.iter())
                    .find(|x| x.name() == t)
                {
                    app.tool = *tool;
                }
            }
        } else if app.pending.is_empty() {
            // a friendly starting canvas
            let doc = Document::new(1600, 1000, Some(Color::WHITE));
            app.tabs.push(DocTab::new(doc, None));
        }
        app
    }

    pub fn tab(&mut self) -> Option<&mut DocTab> {
        self.tabs.get_mut(self.active_tab)
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_time = 0.0;
    }

    pub fn filter_ctx(&self) -> FilterCtx {
        FilterCtx {
            fg: self.fg,
            bg: self.bg,
        }
    }

    pub fn record(&mut self, op: Op) {
        if let Some(r) = &mut self.recording {
            r.ops.push(op);
        }
    }

    pub fn open_doc(&mut self, doc: Document, path: Option<PathBuf>) {
        self.tabs.push(DocTab::new(doc, path.clone()));
        self.active_tab = self.tabs.len() - 1;
        if let Some(p) = path {
            self.recent.retain(|r| r != &p);
            self.recent.insert(0, p);
            self.recent.truncate(12);
        }
    }

    // ------------------------------------------------------------------ commands

    pub fn run(&mut self, ctx: &egui::Context, cmd: Cmd) {
        let has_tab = !self.tabs.is_empty();
        let needs_tab = !matches!(
            cmd,
            Cmd::NewDoc
                | Cmd::Open
                | Cmd::OpenPath(_)
                | Cmd::Quit
                | Cmd::CommandPalette
                | Cmd::Shortcuts
                | Cmd::About
                | Cmd::SwapColors
                | Cmd::DefaultColors
                | Cmd::PasteAsNewDoc
                | Cmd::BatchProcess
                | Cmd::SelectTool(_)
                | Cmd::Preferences
                | Cmd::RecordToggle
        );
        if needs_tab && !has_tab {
            return;
        }
        // finish sessions that would conflict with other commands
        if !matches!(
            cmd,
            Cmd::Commit
                | Cmd::Cancel
                | Cmd::ApplyCrop
                | Cmd::ZoomIn
                | Cmd::ZoomOut
                | Cmd::FitScreen
                | Cmd::ActualPixels
                | Cmd::SelectTool(_)
                | Cmd::CommandPalette
                | Cmd::SwapColors
                | Cmd::DefaultColors
                | Cmd::TogglePixelGrid
                | Cmd::ToggleGuides
        ) {
            if let Some(t) = self.tab() {
                if t.transform.is_some() || t.liquify.is_some() {
                    self.commit_sessions();
                }
            }
        }
        let fctx = self.filter_ctx();
        match cmd {
            Cmd::NewDoc => {
                self.dialog = Some(Dialog::new_doc(
                    self.clipboard.as_ref().map(|c| (c.0.width, c.0.height)),
                ))
            }
            Cmd::Open => {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("Images", kroma_core::io::OPEN_EXTENSIONS)
                    .pick_files()
                {
                    for f in files {
                        self.run(ctx, Cmd::OpenPath(f));
                    }
                }
            }
            Cmd::OpenPath(p) => match kroma_core::io::open(&p) {
                Ok(doc) => {
                    let ext = p
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let keep_path = matches!(ext.as_str(), "kroma");
                    self.open_doc(doc, Some(p.clone()));
                    if !keep_path {
                        // editing a flat file: Save will ask where to put the layered .kroma
                        if let Some(t) = self.tab() {
                            t.path = None;
                        }
                        self.recent.retain(|r| r != &p);
                        self.recent.insert(0, p);
                    }
                    self.toast("Opened");
                }
                Err(e) => self.toast(format!("Could not open {}: {e:#}", p.display())),
            },
            Cmd::Place => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter("Images", kroma_core::io::OPEN_EXTENSIONS)
                    .pick_file()
                {
                    match kroma_core::io::open(&p) {
                        Ok(other) => {
                            let img = other.composite();
                            let t = self.tab().unwrap();
                            let (dw, dh) = (t.doc.width, t.doc.height);
                            // scale down to fit, center
                            let s = (dw as f32 / img.width as f32)
                                .min(dh as f32 / img.height as f32)
                                .min(1.0);
                            let img = if s < 1.0 {
                                kroma_core::transform::resize(
                                    &img,
                                    ((img.width as f32 * s) as u32).max(1),
                                    ((img.height as f32 * s) as u32).max(1),
                                    kroma_core::transform::Resample::Lanczos3,
                                )
                            } else {
                                img
                            };
                            let mut canvas = Image::new(dw, dh);
                            canvas.blit(
                                &img,
                                (dw as i32 - img.width as i32) / 2,
                                (dh as i32 - img.height as i32) / 2,
                            );
                            let name = p
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Placed".into());
                            t.doc.add_layer(Layer::from_image(&name, canvas));
                            t.commit("Place");
                        }
                        Err(e) => self.toast(format!("Could not place: {e:#}")),
                    }
                }
            }
            Cmd::Save => {
                let path = self.tab().unwrap().path.clone();
                match path {
                    Some(p) => self.save_to(p),
                    None => self.run(ctx, Cmd::SaveAs),
                }
            }
            Cmd::SaveAs => {
                let name = self.tab().unwrap().doc.name.clone();
                let stem = std::path::Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(name);
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name(format!("{stem}.kroma"))
                    .add_filter("Kroma document", &["kroma"])
                    .add_filter("OpenRaster (GIMP/Krita)", &["ora"])
                    .save_file()
                {
                    self.save_to(p);
                }
            }
            Cmd::ExportAs => {
                self.dialog = Some(Dialog::Export {
                    ext: 0,
                    quality: 90,
                    scale: 1.0,
                    sixteen: false,
                })
            }
            Cmd::QuickExportPng => {
                let name = self.tab().unwrap().doc.name.clone();
                let stem = std::path::Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(name);
                if let Some(p) = rfd::FileDialog::new()
                    .set_file_name(format!("{stem}.png"))
                    .add_filter("PNG", &["png"])
                    .save_file()
                {
                    let doc = &self.tab().unwrap().doc;
                    match kroma_core::io::export(doc, &p, &Default::default()) {
                        Ok(_) => self.toast(format!("Exported {}", p.display())),
                        Err(e) => self.toast(format!("Export failed: {e:#}")),
                    }
                }
            }
            Cmd::CloseTab => {
                if self.tab().map(|t| t.dirty).unwrap_or(false) {
                    self.dialog = Some(Dialog::ConfirmClose {
                        tab: self.active_tab,
                        quitting: false,
                    });
                } else {
                    self.close_tab(self.active_tab);
                }
            }
            Cmd::Revert => {
                let p = self.tab().unwrap().path.clone();
                if let Some(p) = p {
                    if let Ok(doc) = kroma_core::io::open(&p) {
                        let t = self.tab().unwrap();
                        t.doc = doc;
                        t.commit("Revert");
                        t.dirty = false;
                    }
                }
            }
            Cmd::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),

            Cmd::Undo => self.tab().unwrap().undo(),
            Cmd::Redo => self.tab().unwrap().redo(),
            Cmd::Copy | Cmd::Cut | Cmd::CopyMerged => {
                let merged = matches!(cmd, Cmd::CopyMerged);
                let cut = matches!(cmd, Cmd::Cut);
                let t = self.tab().unwrap();
                let src = if merged {
                    t.doc.composite()
                } else {
                    match t.doc.active_layer() {
                        Some(l) if l.is_raster_like() => l.masked_content(),
                        _ => return,
                    }
                };
                let (img, r) = match &t.doc.selection {
                    Some(s) => {
                        let b = s.bounds_above(0.002);
                        let mut part = src.sub_image(b);
                        for y in 0..part.height {
                            for x in 0..part.width {
                                let m = s.get((b.x + x as i32) as u32, (b.y + y as i32) as u32);
                                let mut p = part.get(x, y);
                                p[3] *= m;
                                part.set(x, y, p);
                            }
                        }
                        (part, b)
                    }
                    None => (src.clone(), t.doc.bounds()),
                };
                if cut && t.active_is_pixel() {
                    let sel = t.doc.selection.clone();
                    let l = t.doc.active_layer_mut().unwrap();
                    match sel {
                        None => l.image_mut().clear(),
                        Some(sd) => {
                            // erase (alpha) under selection
                            let px = l.image_mut().pixels_mut();
                            for (i, p) in px.iter_mut().enumerate() {
                                p[3] *= 1.0 - sd.data()[i];
                            }
                        }
                    }
                    t.commit("Cut");
                }
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_image(arboard::ImageData {
                        width: img.width as usize,
                        height: img.height as usize,
                        bytes: img.to_rgba8().into(),
                    });
                }
                self.clipboard = Some((img, r));
                self.toast(if cut { "Cut" } else { "Copied" });
            }
            Cmd::Paste | Cmd::PasteAsNewDoc => {
                // prefer the system clipboard if it holds a different image
                let mut src = self.clipboard.clone();
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if let Ok(img) = cb.get_image() {
                        let im = Image::from_rgba8(img.width as u32, img.height as u32, &img.bytes);
                        let same = src
                            .as_ref()
                            .map(|(s, _)| s.width == im.width && s.height == im.height)
                            .unwrap_or(false);
                        if !same {
                            src = Some((im, Rect::default()));
                        }
                    }
                }
                let Some((img, r)) = src else {
                    self.toast("Clipboard is empty");
                    return;
                };
                if matches!(cmd, Cmd::PasteAsNewDoc) || self.tabs.is_empty() {
                    let mut doc = Document::new(img.width, img.height, None);
                    doc.layers[0].set_image(img);
                    doc.name = "Pasted".into();
                    self.open_doc(doc, None);
                    return;
                }
                let t = self.tab().unwrap();
                let (dw, dh) = (t.doc.width as i32, t.doc.height as i32);
                let (x, y) = if !r.is_empty() && r.right() <= dw && r.bottom() <= dh {
                    (r.x, r.y)
                } else {
                    ((dw - img.width as i32) / 2, (dh - img.height as i32) / 2)
                };
                let mut canvas = Image::new(dw as u32, dh as u32);
                canvas.blit(&img, x, y);
                let n = t.doc.layer_count() + 1;
                t.doc
                    .add_layer(Layer::from_image(&format!("Layer {n}"), canvas));
                t.doc.selection = None;
                t.commit("Paste");
            }
            Cmd::Clear => {
                let t = self.tab().unwrap();
                if !t.active_is_pixel() {
                    return;
                }
                let sel = t.doc.selection.clone();
                let l = t.doc.active_layer_mut().unwrap();
                match sel {
                    None => l.image_mut().clear(),
                    Some(s) => {
                        let px = l.image_mut().pixels_mut();
                        for (i, p) in px.iter_mut().enumerate() {
                            p[3] *= 1.0 - s.data()[i];
                        }
                    }
                }
                t.commit("Clear");
            }
            Cmd::FillDialog => {
                self.dialog = Some(Dialog::Fill {
                    source: 0,
                    color: self.fg,
                    opacity: 1.0,
                    mode: kroma_core::blend::BlendMode::Normal,
                    preserve: false,
                })
            }
            Cmd::StrokeDialog => {
                if self.tab().unwrap().doc.selection.is_none() {
                    self.toast("Stroke needs a selection");
                    return;
                }
                self.dialog = Some(Dialog::Stroke {
                    width: 3.0,
                    color: self.fg,
                    location: kroma_core::brush::StrokeLocation::Center,
                });
            }
            Cmd::ContentAwareFill => {
                let t = self.tab().unwrap();
                if !t.active_is_pixel() {
                    self.toast("Select a pixel layer");
                    return;
                }
                if t.doc.selection.is_none() {
                    self.toast(
                        "Content-Aware Fill needs a selection (select the object to remove)",
                    );
                    return;
                }
                let r = kroma_core::ops::apply(&mut t.doc, &Op::ContentAwareFill, &fctx);
                match r {
                    Ok(_) => {
                        t.commit("Content-Aware Fill");
                        self.record(Op::ContentAwareFill);
                    }
                    Err(e) => self.toast(e.to_string()),
                }
            }
            Cmd::FreeTransform | Cmd::Distort => self.begin_transform(matches!(cmd, Cmd::Distort)),
            Cmd::FlipLayer(h) => self.map_active_layer(
                if h {
                    "Flip Layer Horizontal"
                } else {
                    "Flip Layer Vertical"
                },
                &move |img| {
                    if h {
                        kroma_core::transform::flip_h(img)
                    } else {
                        kroma_core::transform::flip_v(img)
                    }
                },
            ),
            Cmd::RotateLayer(deg) => {
                let d = deg as f32;
                self.map_active_layer("Rotate Layer", &move |img| {
                    kroma_core::transform::rotate(img, d, false, [0.0; 4])
                })
            }
            Cmd::TakeSnapshot => {
                let t = self.tab().unwrap();
                let n = t.history.snapshots.len() + 1;
                t.history.take_snapshot(&format!("Snapshot {n}"));
                self.toast("Snapshot taken");
            }
            Cmd::Preferences => self.dialog = Some(Dialog::Preferences),

            Cmd::Adjust(adj) => self.open_adjust_dialog(adj),
            Cmd::AutoTone | Cmd::AutoContrast | Cmd::AutoColor => {
                let t = self.tab().unwrap();
                if !t.active_is_pixel() {
                    self.toast("Select a pixel layer");
                    return;
                }
                let sel = t.doc.selection.clone();
                let l = t.doc.active_layer_mut().unwrap();
                let (adj, name, op) = match cmd {
                    Cmd::AutoTone => (auto_tone(l.image()), "Auto Tone", Op::AutoTone),
                    Cmd::AutoContrast => {
                        (auto_contrast(l.image()), "Auto Contrast", Op::AutoContrast)
                    }
                    _ => (auto_color(l.image()), "Auto Color", Op::AutoColor),
                };
                adj.apply(l.image_mut(), sel.as_ref());
                t.commit(name);
                self.record(op);
            }
            Cmd::QuickInvert | Cmd::QuickDesaturate => {
                let adj = if matches!(cmd, Cmd::QuickInvert) {
                    Adjustment::Invert
                } else {
                    Adjustment::Desaturate
                };
                let t = self.tab().unwrap();
                if !t.active_is_pixel() {
                    self.toast("Select a pixel layer");
                    return;
                }
                let sel = t.doc.selection.clone();
                let l = t.doc.active_layer_mut().unwrap();
                adj.apply(l.image_mut(), sel.as_ref());
                t.commit(adj.name());
                self.record(Op::Adjust { adjustment: adj });
            }
            Cmd::ImageSize => {
                let t = self.tab().unwrap();
                self.dialog = Some(Dialog::ImageSize {
                    w: t.doc.width,
                    h: t.doc.height,
                    keep: true,
                    method: kroma_core::transform::Resample::Bicubic,
                    ow: t.doc.width,
                    oh: t.doc.height,
                });
            }
            Cmd::CanvasSize => {
                let t = self.tab().unwrap();
                self.dialog = Some(Dialog::CanvasSize {
                    w: t.doc.width,
                    h: t.doc.height,
                    anchor: kroma_core::transform::Anchor::Center,
                });
            }
            Cmd::RotateCanvas(k) => {
                let t = self.tab().unwrap();
                let (name, op) = match k {
                    90 => {
                        t.doc.rotate_canvas_cw();
                        ("Rotate 90° Clockwise", Op::RotateCw)
                    }
                    -90 => {
                        t.doc.rotate_canvas_ccw();
                        ("Rotate 90° Counter Clockwise", Op::RotateCcw)
                    }
                    _ => {
                        t.doc.rotate_canvas_180();
                        ("Rotate 180°", Op::Rotate180)
                    }
                };
                t.needs_fit = true;
                t.commit(name);
                self.record(op);
            }
            Cmd::RotateArbitrary => self.dialog = Some(Dialog::Rotate { degrees: 15.0 }),
            Cmd::FlipCanvas(h) => {
                let t = self.tab().unwrap();
                t.doc.flip_canvas(h);
                t.commit(if h {
                    "Flip Canvas Horizontal"
                } else {
                    "Flip Canvas Vertical"
                });
                self.record(if h {
                    Op::FlipHorizontal
                } else {
                    Op::FlipVertical
                });
            }
            Cmd::CropToSelection => {
                let t = self.tab().unwrap();
                if let Some(s) = &t.doc.selection {
                    let b = s.bounds_above(0.002);
                    if !b.is_empty() {
                        t.doc.crop(b);
                        t.needs_fit = true;
                        t.commit("Crop");
                        self.record(Op::Crop {
                            x: b.x,
                            y: b.y,
                            width: b.w,
                            height: b.h,
                        });
                    }
                } else {
                    self.toast("Make a selection first (or use the Crop tool)");
                }
            }
            Cmd::Trim => {
                let t = self.tab().unwrap();
                t.doc.trim();
                t.needs_fit = true;
                t.commit("Trim");
                self.record(Op::Trim);
            }
            Cmd::DuplicateDoc => {
                let mut d = self.tab().unwrap().doc.clone();
                d.name = format!("{} copy", d.name);
                self.open_doc(d, None);
            }

            Cmd::NewLayer => {
                let t = self.tab().unwrap();
                t.doc.new_raster_layer();
                t.commit("New Layer");
            }
            Cmd::LayerViaCopy | Cmd::LayerViaCut => {
                let cut = matches!(cmd, Cmd::LayerViaCut);
                let t = self.tab().unwrap();
                let Some(l) = t.doc.active_layer() else {
                    return;
                };
                if !l.is_raster_like() {
                    if let Some(id) = t.doc.duplicate_layer(t.doc.active) {
                        let _ = id;
                        t.commit("Duplicate Layer");
                    }
                    return;
                }
                let sel = t.doc.selection.clone();
                match sel {
                    None => {
                        t.doc.duplicate_layer(t.doc.active);
                        t.commit("Layer via Copy");
                    }
                    Some(s) => {
                        let mut img = l.image().clone();
                        for (i, p) in img.pixels_mut().iter_mut().enumerate() {
                            p[3] *= s.data()[i];
                        }
                        if cut {
                            let al = t.doc.active_layer_mut().unwrap();
                            let px = al.image_mut().pixels_mut();
                            for (i, p) in px.iter_mut().enumerate() {
                                p[3] *= 1.0 - s.data()[i];
                            }
                        }
                        let n = t.doc.layer_count() + 1;
                        t.doc
                            .add_layer(Layer::from_image(&format!("Layer {n}"), img));
                        t.doc.selection = None;
                        t.commit(if cut {
                            "Layer via Cut"
                        } else {
                            "Layer via Copy"
                        });
                    }
                }
            }
            Cmd::DuplicateLayer => {
                let t = self.tab().unwrap();
                t.doc.duplicate_layer(t.doc.active);
                t.commit("Duplicate Layer");
            }
            Cmd::DeleteLayer => {
                let t = self.tab().unwrap();
                if t.doc.delete_layer(t.doc.active) {
                    t.editing_mask = false;
                    t.commit("Delete Layer");
                }
            }
            Cmd::RenameLayer => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer() {
                    self.dialog = Some(Dialog::Rename {
                        id: l.id,
                        name: l.name.clone(),
                    });
                }
            }
            Cmd::NewFillSolid | Cmd::NewFillGradient => {
                let (fg, bg) = (self.fg, self.bg);
                let t = self.tab().unwrap();
                let (w, h) = (t.doc.width, t.doc.height);
                let src = if matches!(cmd, Cmd::NewFillSolid) {
                    FillSource::Solid(fg)
                } else {
                    FillSource::Gradient {
                        fill: kroma_core::gradient::GradientFill {
                            gradient: kroma_core::gradient::Gradient::two(fg, bg),
                            ..Default::default()
                        },
                        from: (0.0, 0.0),
                        to: (w as f32, h as f32),
                    }
                };
                let mut l = Layer::fill(
                    if matches!(cmd, Cmd::NewFillSolid) {
                        "Color Fill"
                    } else {
                        "Gradient Fill"
                    },
                    src,
                    w,
                    h,
                );
                if let Some(s) = &t.doc.selection {
                    l.set_mask(Some(s.clone()));
                }
                t.doc.add_layer(l);
                t.commit("New Fill Layer");
            }
            Cmd::NewAdjustmentLayer(adj) => {
                let t = self.tab().unwrap();
                t.doc.add_adjustment_layer(adj.clone());
                t.commit(&format!("New {} Layer", adj.name()));
                self.record(Op::AdjustmentLayer { adjustment: adj });
            }
            Cmd::LayerStyle => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer() {
                    if l.is_raster_like() {
                        let effects = l.effects.clone();
                        self.dialog = Some(Dialog::LayerStyle {
                            layer: l.id,
                            original: effects.clone(),
                            selected: 0,
                        });
                    } else {
                        self.toast("Layer styles apply to pixel, text and fill layers");
                    }
                }
            }
            Cmd::ClearStyle => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    l.effects.clear();
                    l.touch();
                }
                t.commit("Clear Layer Style");
            }
            Cmd::AddMask {
                from_selection,
                hide,
            } => {
                let t = self.tab().unwrap();
                t.doc.add_mask(from_selection, hide);
                t.editing_mask = true;
                t.commit("Add Layer Mask");
            }
            Cmd::DeleteMask => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    l.set_mask(None);
                }
                t.editing_mask = false;
                t.commit("Delete Layer Mask");
            }
            Cmd::ApplyMask => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    if matches!(l.kind, LayerKind::Raster) && l.mask().is_some() {
                        let img = l.masked_content();
                        l.set_image(img);
                        l.set_mask(None);
                    }
                }
                t.editing_mask = false;
                t.commit("Apply Layer Mask");
            }
            Cmd::ToggleMaskEnabled => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    l.mask_enabled = !l.mask_enabled;
                    l.touch();
                }
                t.commit("Toggle Layer Mask");
            }
            Cmd::InvertMask => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    if let Some(m) = l.mask() {
                        let inv = kroma_core::selection::invert(m);
                        l.set_mask(Some(inv));
                    }
                }
                t.commit("Invert Mask");
            }
            Cmd::EditMask(on) => {
                let t = self.tab().unwrap();
                t.editing_mask = on
                    && t.doc
                        .active_layer()
                        .map(|l| l.mask().is_some())
                        .unwrap_or(false);
            }
            Cmd::ToggleClip => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    l.clipped = !l.clipped;
                    l.touch();
                }
                t.commit("Clipping Mask");
            }
            Cmd::Group => {
                let t = self.tab().unwrap();
                if let Some(id) = t.doc.group_layer(t.doc.active) {
                    t.doc.active = id;
                    t.commit("Group Layers");
                }
            }
            Cmd::Ungroup => {
                let t = self.tab().unwrap();
                if t.doc.ungroup(t.doc.active) {
                    t.commit("Ungroup Layers");
                }
            }
            Cmd::Arrange(k) => {
                let t = self.tab().unwrap();
                let id = t.doc.active;
                match k {
                    2 => t.doc.move_layer_to_top(id),
                    -2 => t.doc.move_layer_to_bottom(id),
                    d => t.doc.move_layer(id, d),
                }
                t.commit("Arrange");
            }
            Cmd::Rasterize => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer_mut() {
                    l.rasterize();
                }
                t.commit("Rasterize Layer");
            }
            Cmd::MergeDown => {
                let t = self.tab().unwrap();
                if t.doc.merge_down(t.doc.active) {
                    t.editing_mask = false;
                    t.commit("Merge Down");
                }
            }
            Cmd::MergeVisible => {
                let t = self.tab().unwrap();
                t.doc.merge_visible();
                t.commit("Merge Visible");
            }
            Cmd::Flatten => {
                let t = self.tab().unwrap();
                t.doc.flatten();
                t.editing_mask = false;
                t.commit("Flatten Image");
                self.record(Op::Flatten);
            }
            Cmd::SelectLayerPixels => {
                let t = self.tab().unwrap();
                if let Some(l) = t.doc.active_layer() {
                    if l.is_raster_like() {
                        t.doc.selection = Some(Mask::from_alpha(&l.masked_content()));
                        t.commit("Load Selection");
                    }
                }
            }

            Cmd::SelectAll => {
                let t = self.tab().unwrap();
                t.doc.selection = Some(Mask::new(t.doc.width, t.doc.height, 1.0));
                t.commit("Select All");
                self.record(Op::SelectAll);
            }
            Cmd::Deselect => {
                let t = self.tab().unwrap();
                if let Some(s) = t.doc.selection.take() {
                    self.last_selection = Some(s);
                    let t = self.tab().unwrap();
                    t.commit("Deselect");
                }
                self.record(Op::Deselect);
            }
            Cmd::Reselect => {
                if let Some(s) = self.last_selection.clone() {
                    let t = self.tab().unwrap();
                    if s.width == t.doc.width && s.height == t.doc.height {
                        t.doc.selection = Some(s);
                        t.commit("Reselect");
                    }
                }
            }
            Cmd::InvertSelection => {
                let t = self.tab().unwrap();
                t.doc.selection = Some(match &t.doc.selection {
                    Some(s) => kroma_core::selection::invert(s),
                    None => Mask::new(t.doc.width, t.doc.height, 1.0),
                });
                t.commit("Inverse");
                self.record(Op::InvertSelection);
            }
            Cmd::SelectSubject => {
                let t = self.tab().unwrap();
                let img = t.doc.composite();
                t.doc.selection = Some(kroma_core::selection::select_subject_heuristic(&img));
                t.commit("Select Subject");
                self.record(Op::SelectSubject);
                self.toast(
                    "Select Subject (contrast-based). Refine with Shift/Alt + selection tools.",
                );
            }
            Cmd::ColorRange => {
                self.dialog = Some(Dialog::ColorRange {
                    color: self.fg,
                    fuzziness: 40.0,
                    mode: 0,
                })
            }
            Cmd::ModifySelection(k) => {
                if self.tab().unwrap().doc.selection.is_none() {
                    self.toast("No selection");
                    return;
                }
                self.dialog = Some(Dialog::ModifySelection {
                    kind: k,
                    amount: if k == 4 { 4.0 } else { 5.0 },
                });
            }
            Cmd::Grow | Cmd::Similar => {
                let tol = self.opts.wand_tolerance;
                let t = self.tab().unwrap();
                let Some(s) = t.doc.selection.clone() else {
                    return;
                };
                let img = t.doc.composite();
                let out = if matches!(cmd, Cmd::Grow) {
                    kroma_core::selection::grow(&img, &s, tol)
                } else {
                    // Similar: every pixel close to any color found in the selection's center
                    let b = s.bounds_above(0.5);
                    let p = img.get((b.x + b.w / 2) as u32, (b.y + b.h / 2) as u32);
                    let other = kroma_core::selection::magic_wand(
                        &img,
                        (b.x + b.w / 2) as u32,
                        (b.y + b.h / 2) as u32,
                        tol,
                        false,
                        true,
                    );
                    let _ = p;
                    kroma_core::selection::combine(
                        Some(&s),
                        other,
                        kroma_core::selection::SelectOp::Add,
                    )
                    .unwrap_or(s)
                };
                t.doc.selection = Some(out);
                t.commit(if matches!(cmd, Cmd::Grow) {
                    "Grow"
                } else {
                    "Similar"
                });
            }

            Cmd::Filter(f) => self.open_filter_dialog(f),
            Cmd::LastFilter => {
                if let Some(f) = self.last_filter.clone() {
                    let t = self.tab().unwrap();
                    if !t.active_is_pixel() {
                        return;
                    }
                    let sel = t.doc.selection.clone();
                    let l = t.doc.active_layer_mut().unwrap();
                    f.apply(l.image_mut(), sel.as_ref(), &fctx);
                    t.commit(f.name());
                    self.record(Op::Filter { filter: f });
                }
            }

            Cmd::ZoomIn | Cmd::ZoomOut | Cmd::FitScreen | Cmd::ActualPixels => {
                let rect = ctx
                    .memory(|m| m.data.get_temp::<egui::Rect>(egui::Id::new("canvas_rect")))
                    .unwrap_or(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    ));
                let t = self.tab().unwrap();
                match cmd {
                    Cmd::FitScreen => t.fit(rect),
                    Cmd::ActualPixels => t.zoom_about(rect, rect.center(), 1.0),
                    Cmd::ZoomIn => t.zoom_about(rect, rect.center(), next_zoom(t.zoom, true)),
                    _ => t.zoom_about(rect, rect.center(), next_zoom(t.zoom, false)),
                }
            }
            Cmd::TogglePixelGrid => self.show_pixel_grid = !self.show_pixel_grid,
            Cmd::ToggleGuides => self.show_guides = !self.show_guides,
            Cmd::AddGuide(vertical) => {
                let t = self.tab().unwrap();
                let pos = if vertical {
                    t.doc.width as f32 / 2.0
                } else {
                    t.doc.height as f32 / 2.0
                };
                t.doc
                    .guides
                    .push(kroma_core::document::Guide { vertical, pos });
                t.commit("New Guide");
                self.toast("Guide added at center. Drag guides with the Move tool.");
            }
            Cmd::ClearGuides => {
                let t = self.tab().unwrap();
                t.doc.guides.clear();
                t.commit("Clear Guides");
            }

            Cmd::CommandPalette => {
                self.palette_open = !self.palette_open;
                self.palette_query.clear();
            }
            Cmd::Shortcuts => self.dialog = Some(Dialog::Shortcuts),
            Cmd::About => self.dialog = Some(Dialog::About),
            Cmd::SwapColors => std::mem::swap(&mut self.fg, &mut self.bg),
            Cmd::DefaultColors => {
                self.fg = Color::BLACK;
                self.bg = Color::WHITE;
            }
            Cmd::ApplyCrop => self.apply_crop(),
            Cmd::Cancel => {
                if self.palette_open {
                    self.palette_open = false;
                } else if let Some(t) = self.tab() {
                    t.cancel_sessions();
                }
            }
            Cmd::Commit => {
                self.commit_sessions();
                if self.tool == Tool::Crop {
                    self.apply_crop();
                }
                if self.tool == Tool::PolyLasso {
                    crate::canvas::finish_poly_lasso(self);
                }
            }
            Cmd::Align(k) => {
                let t = self.tab().unwrap();
                let Some(l) = t.doc.active_layer() else {
                    return;
                };
                if !matches!(l.kind, LayerKind::Raster) {
                    return;
                }
                let b = l.image().opaque_bounds(0.004);
                if b.is_empty() {
                    return;
                }
                let target = t
                    .doc
                    .selection
                    .as_ref()
                    .map(|s| s.bounds_above(0.5))
                    .filter(|r| !r.is_empty())
                    .unwrap_or(t.doc.bounds());
                let (dx, dy) = match k {
                    0 => (target.x - b.x, 0),
                    1 => (target.x + target.w / 2 - (b.x + b.w / 2), 0),
                    2 => (target.right() - b.right(), 0),
                    3 => (0, target.y - b.y),
                    4 => (0, target.y + target.h / 2 - (b.y + b.h / 2)),
                    _ => (0, target.bottom() - b.bottom()),
                };
                let l = t.doc.active_layer_mut().unwrap();
                let mut out = Image::new(l.image().width, l.image().height);
                out.blit(l.image(), dx, dy);
                l.set_image(out);
                t.commit("Align");
            }
            Cmd::RunAction(i) => {
                if let Some(a) = self.actions.get(i).cloned() {
                    let t = self.tab().unwrap();
                    match a.run(&mut t.doc, &fctx) {
                        Ok(_) => {
                            t.needs_fit = true;
                            t.commit(&format!("Action: {}", a.name));
                            self.toast(format!("Ran action '{}'", a.name));
                        }
                        Err(e) => {
                            t.full_redraw = true;
                            self.toast(format!("Action stopped: {e}"));
                        }
                    }
                }
            }
            Cmd::RecordToggle => {
                match self.recording.take() {
                    Some(a) => {
                        if a.ops.is_empty() {
                            self.toast("Recording discarded (no steps)");
                        } else {
                            self.toast(format!(
                                "Saved action '{}' with {} steps",
                                a.name,
                                a.ops.len()
                            ));
                            self.actions.push(a);
                        }
                    }
                    None => {
                        let n = self.actions.len() + 1;
                        self.recording = Some(Action {
                            name: format!("Action {n}"),
                            ops: vec![],
                        });
                        self.toast("Recording… run adjustments, filters and image commands, then press Stop");
                    }
                }
            }
            Cmd::BatchProcess => {
                self.dialog = Some(Dialog::Batch {
                    action: 0,
                    files: vec![],
                    out_dir: None,
                    ext: 0,
                    quality: 90,
                })
            }
            Cmd::SelectTool(t) => self.set_tool(t),
        }
    }

    pub fn set_tool(&mut self, t: Tool) {
        if t != self.tool {
            self.commit_sessions();
            if let Some(tab) = self.tab() {
                tab.crop_pending = None;
                tab.poly_points.clear();
            }
        }
        self.tool = t;
    }

    fn save_to(&mut self, p: PathBuf) {
        let tab_id = self.tabs[self.active_tab].id;
        let t = self.tab().unwrap();
        match kroma_core::io::save(&t.doc, &p) {
            Ok(_) => {
                t.doc.name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let layered = matches!(p.extension().and_then(|e| e.to_str()), Some("kroma"));
                if layered {
                    t.path = Some(p.clone());
                }
                t.dirty = false;
                crate::recovery::delete(tab_id);
                self.recent.retain(|r| r != &p);
                self.recent.insert(0, p.clone());
                self.toast(format!("Saved {}", p.display()));
            }
            Err(e) => self.toast(format!("Save failed: {e:#}")),
        }
    }

    pub fn close_tab(&mut self, i: usize) {
        if i < self.tabs.len() {
            let id = self.tabs[i].id;
            crate::recovery::delete(id);
            self.tabs.remove(i);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len().saturating_sub(1);
            }
        }
    }

    fn map_active_layer(&mut self, name: &str, f: &dyn Fn(&Image) -> Image) {
        let t = self.tab().unwrap();
        let Some(l) = t.doc.active_layer_mut() else {
            return;
        };
        if !matches!(l.kind, LayerKind::Raster) {
            self.toast("Rasterize the layer first");
            return;
        }
        let out = f(l.image());
        l.set_image(out);
        t.commit(name);
    }

    pub fn begin_transform(&mut self, distort: bool) {
        let t = self.tab().unwrap();
        let Some(l) = t.doc.active_layer() else {
            return;
        };
        if !matches!(l.kind, LayerKind::Raster) {
            self.toast("Free Transform works on pixel layers (rasterize text first)");
            return;
        }
        let mut original = l.image().clone();
        let id = l.id;
        // with a selection: transform only the selected pixels (lifted onto the same layer)
        if let Some(s) = t.doc.selection.clone() {
            let mut lifted = original.clone();
            for (i, p) in lifted.pixels_mut().iter_mut().enumerate() {
                p[3] *= s.data()[i];
            }
            let mut rest = original.clone();
            for (i, p) in rest.pixels_mut().iter_mut().enumerate() {
                p[3] *= 1.0 - s.data()[i];
            }
            // put the remainder in a new layer below so the transform only moves the selection
            let n = t.doc.layer_count() + 1;
            let below = Layer::from_image(&format!("Layer {n}"), rest);
            let (list, idx) = t.doc.parent_list_mut(id).unwrap();
            list.insert(idx, below);
            t.doc.layer_mut(id).unwrap().set_image(lifted.clone());
            t.doc.selection = None;
            original = lifted;
        }
        let bounds = original.opaque_bounds(0.004);
        if bounds.is_empty() {
            self.toast("Layer is empty");
            return;
        }
        let mut ft = crate::tab::FreeTransform {
            layer: id,
            original,
            bounds,
            tx: 0.0,
            ty: 0.0,
            sx: 1.0,
            sy: 1.0,
            rot: 0.0,
            drag: None,
            corners: None,
        };
        if distort {
            ft.corners = Some(ft.quad());
        }
        let t = self.tab().unwrap();
        t.transform = Some(ft);
        self.toast("Transform: drag inside to move, handles to scale (Shift = keep ratio), outside to rotate. Enter applies.");
    }

    pub fn commit_sessions(&mut self) {
        let Some(t) = self.tabs.get_mut(self.active_tab) else {
            return;
        };
        if let Some(ft) = t.transform.take() {
            let img = ft.render(t.doc.width, t.doc.height);
            if let Some(l) = t.doc.layer_mut(ft.layer) {
                l.set_image(img);
            }
            t.commit("Free Transform");
        }
        if let Some(s) = t.liquify.take() {
            if let Some(l) = t.doc.layer_mut(s.layer) {
                let img = s.field.render(&s.original);
                l.set_image(img);
            }
            t.commit("Liquify");
        }
    }

    fn apply_crop(&mut self) {
        let delete = self.opts.crop_delete_pixels;
        let Some(t) = self.tab() else { return };
        if let Some(r) = t.crop_pending.take() {
            if r.w > 1 && r.h > 1 {
                let _ = delete;
                t.doc.crop(r);
                t.needs_fit = true;
                t.commit("Crop");
                self.record(Op::Crop {
                    x: r.x,
                    y: r.y,
                    width: r.w,
                    height: r.h,
                });
            }
        }
    }

    fn open_adjust_dialog(&mut self, adj: Adjustment) {
        let t = self.tab().unwrap();
        if !t.active_is_pixel() {
            self.toast("Destructive adjustments need a pixel layer. Use Layer › New Adjustment Layer for non-destructive edits.");
            return;
        }
        let l = t.doc.active_layer().unwrap();
        let hist = Histogram::of(l.image());
        t.preview = Some(crate::tab::Preview {
            layer: l.id,
            original: l.image().clone(),
        });
        self.dialog = Some(Dialog::Adjust {
            adj,
            hist,
            live: true,
            changed: true,
        });
    }

    fn open_filter_dialog(&mut self, f: Filter) {
        let fctx = self.filter_ctx();
        let t = self.tab().unwrap();
        if !t.active_is_pixel() {
            self.toast("Filters need a pixel layer (Layer › Rasterize)");
            return;
        }
        if !f.has_params() {
            let sel = t.doc.selection.clone();
            let l = t.doc.active_layer_mut().unwrap();
            f.apply(l.image_mut(), sel.as_ref(), &fctx);
            t.commit(f.name());
            self.last_filter = Some(f.clone());
            self.record(Op::Filter { filter: f });
            return;
        }
        let l = t.doc.active_layer().unwrap();
        t.preview = Some(crate::tab::Preview {
            layer: l.id,
            original: l.image().clone(),
        });
        self.dialog = Some(Dialog::Filter {
            filter: f,
            live: true,
            changed: true,
        });
    }

    // ------------------------------------------------------------------ shortcuts

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let cmd_mod = Modifiers::COMMAND;
        let shift = Modifiers::SHIFT;
        let alt = Modifiers::ALT;
        let table: Vec<(Modifiers, Key, Cmd)> = vec![
            (cmd_mod, Key::N, Cmd::NewDoc),
            (cmd_mod, Key::O, Cmd::Open),
            (cmd_mod | shift, Key::S, Cmd::SaveAs),
            (cmd_mod, Key::S, Cmd::Save),
            (cmd_mod | alt | shift, Key::W, Cmd::ExportAs),
            (cmd_mod | shift | alt, Key::S, Cmd::QuickExportPng),
            (cmd_mod, Key::W, Cmd::CloseTab),
            (cmd_mod | shift, Key::Z, Cmd::Redo),
            (cmd_mod, Key::Z, Cmd::Undo),
            (cmd_mod, Key::Y, Cmd::Redo),
            (cmd_mod | shift, Key::C, Cmd::CopyMerged),
            (cmd_mod, Key::X, Cmd::Cut),
            (cmd_mod, Key::C, Cmd::Copy),
            (cmd_mod, Key::V, Cmd::Paste),
            (shift, Key::F5, Cmd::FillDialog),
            (cmd_mod, Key::T, Cmd::FreeTransform),
            (cmd_mod | shift, Key::L, Cmd::AutoTone),
            (cmd_mod | alt | shift, Key::L, Cmd::AutoContrast),
            (cmd_mod | shift, Key::B, Cmd::AutoColor),
            (
                cmd_mod,
                Key::L,
                Cmd::Adjust(Adjustment::catalog()[1].clone()),
            ),
            (
                cmd_mod,
                Key::M,
                Cmd::Adjust(Adjustment::catalog()[2].clone()),
            ),
            (
                cmd_mod,
                Key::U,
                Cmd::Adjust(Adjustment::catalog()[5].clone()),
            ),
            (
                cmd_mod,
                Key::B,
                Cmd::Adjust(Adjustment::catalog()[6].clone()),
            ),
            (cmd_mod | shift, Key::U, Cmd::QuickDesaturate),
            (cmd_mod, Key::I, Cmd::QuickInvert),
            (cmd_mod | alt, Key::I, Cmd::ImageSize),
            (cmd_mod | alt, Key::C, Cmd::CanvasSize),
            (cmd_mod | shift, Key::N, Cmd::NewLayer),
            (cmd_mod | shift, Key::J, Cmd::LayerViaCut),
            (cmd_mod, Key::J, Cmd::LayerViaCopy),
            (cmd_mod | shift, Key::G, Cmd::Ungroup),
            (cmd_mod | alt, Key::G, Cmd::ToggleClip),
            (cmd_mod, Key::G, Cmd::Group),
            (cmd_mod | shift, Key::E, Cmd::MergeVisible),
            (cmd_mod, Key::E, Cmd::MergeDown),
            (cmd_mod | shift, Key::CloseBracket, Cmd::Arrange(2)),
            (cmd_mod | shift, Key::OpenBracket, Cmd::Arrange(-2)),
            (cmd_mod, Key::CloseBracket, Cmd::Arrange(1)),
            (cmd_mod, Key::OpenBracket, Cmd::Arrange(-1)),
            (cmd_mod, Key::A, Cmd::SelectAll),
            (cmd_mod | shift, Key::D, Cmd::Reselect),
            (cmd_mod, Key::D, Cmd::Deselect),
            (cmd_mod | shift, Key::I, Cmd::InvertSelection),
            (shift, Key::F6, Cmd::ModifySelection(4)),
            (cmd_mod | alt, Key::F, Cmd::LastFilter),
            (cmd_mod, Key::Plus, Cmd::ZoomIn),
            (cmd_mod, Key::Equals, Cmd::ZoomIn),
            (cmd_mod, Key::Minus, Cmd::ZoomOut),
            (cmd_mod, Key::Num0, Cmd::FitScreen),
            (cmd_mod, Key::Num1, Cmd::ActualPixels),
            (cmd_mod, Key::K, Cmd::CommandPalette),
            (cmd_mod | shift, Key::P, Cmd::CommandPalette),
            (cmd_mod, Key::Slash, Cmd::Shortcuts),
            (Modifiers::NONE, Key::F1, Cmd::Shortcuts),
            (Modifiers::NONE, Key::Enter, Cmd::Commit),
            (Modifiers::NONE, Key::Escape, Cmd::Cancel),
            (Modifiers::NONE, Key::X, Cmd::SwapColors),
            (Modifiers::NONE, Key::D, Cmd::DefaultColors),
            (Modifiers::NONE, Key::Delete, Cmd::Clear),
            (Modifiers::NONE, Key::Backspace, Cmd::Clear),
        ];
        for (m, k, c) in table {
            if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k))) {
                self.pending.push(c);
            }
        }
        // tool shortcuts (Shift cycles within a group)
        let (pressed, mods) = ctx.input(|i| {
            (
                i.events
                    .iter()
                    .filter_map(|e| {
                        if let egui::Event::Key {
                            key,
                            pressed: true,
                            repeat: false,
                            modifiers,
                            ..
                        } = e
                        {
                            Some((*key, *modifiers))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
                i.modifiers,
            )
        });
        let _ = mods;
        for (key, m) in pressed {
            if m.command || m.alt {
                continue;
            }
            for g in Tool::GROUPS {
                if g[0].shortcut() == Some(key) {
                    let next = if m.shift && g.contains(&self.tool) {
                        let i = g.iter().position(|t| *t == self.tool).unwrap();
                        g[(i + 1) % g.len()]
                    } else if g.contains(&self.tool) {
                        self.tool
                    } else {
                        g[0]
                    };
                    self.pending.push(Cmd::SelectTool(next));
                }
            }
            // brush size / hardness
            if key == Key::OpenBracket || key == Key::CloseBracket {
                let up = key == Key::CloseBracket;
                if m.shift {
                    if let Some(b) = self.opts.brush_for(self.tool) {
                        b.hardness = (b.hardness + if up { 0.25 } else { -0.25 }).clamp(0.0, 1.0);
                    }
                } else if let Some(s) = self.opts.tool_size(self.tool) {
                    let step = if *s < 10.0 {
                        1.0
                    } else if *s < 100.0 {
                        5.0
                    } else {
                        25.0
                    };
                    *s = (*s + if up { step } else { -step }).clamp(1.0, 2500.0);
                }
            }
            // number keys set opacity like Photoshop (1 = 10% … 0 = 100%)
            let digit = match key {
                Key::Num1 => Some(0.1),
                Key::Num2 => Some(0.2),
                Key::Num3 => Some(0.3),
                Key::Num4 => Some(0.4),
                Key::Num5 => Some(0.5),
                Key::Num6 => Some(0.6),
                Key::Num7 => Some(0.7),
                Key::Num8 => Some(0.8),
                Key::Num9 => Some(0.9),
                Key::Num0 => Some(1.0),
                _ => None,
            };
            if let Some(v) = digit {
                let tool = self.tool;
                if let Some(b) = self.opts.brush_for(tool) {
                    b.opacity = v;
                } else if let Some(t) = self.tab() {
                    if let Some(l) = t.doc.active_layer_mut() {
                        l.opacity = v;
                        l.touch();
                        t.commit("Layer Opacity");
                    }
                }
            }
            // arrow keys nudge with move tool
            if self.tool == Tool::Move {
                let d = if m.shift { 10 } else { 1 };
                let (dx, dy) = match key {
                    Key::ArrowLeft => (-d, 0),
                    Key::ArrowRight => (d, 0),
                    Key::ArrowUp => (0, -d),
                    Key::ArrowDown => (0, d),
                    _ => (0, 0),
                };
                if dx != 0 || dy != 0 {
                    crate::canvas::nudge(self, dx, dy);
                }
            }
        }
        self.space_down = ctx.input(|i| i.key_down(Key::Space));
    }

    pub fn save_settings(&self, storage: &mut dyn eframe::Storage) {
        let custom: Vec<Action> = self
            .actions
            .iter()
            .filter(|a| !Action::builtins().iter().any(|b| b.name == a.name))
            .cloned()
            .collect();
        let s = Settings {
            opts: self.opts.clone(),
            fg: self.fg,
            bg: self.bg,
            swatches: self.swatches.clone(),
            actions: custom,
            dark: self.dark,
            recent: self.recent.clone(),
        };
        eframe::set_value(storage, SETTINGS_KEY, &s);
    }
}

pub fn next_zoom(z: f32, up: bool) -> f32 {
    const STEPS: [f32; 22] = [
        0.02, 0.03, 0.05, 0.0667, 0.1, 0.125, 0.167, 0.25, 0.333, 0.5, 0.667, 1.0, 1.5, 2.0, 3.0,
        4.0, 5.0, 6.0, 8.0, 12.0, 16.0, 32.0,
    ];
    if up {
        STEPS
            .iter()
            .copied()
            .find(|s| *s > z * 1.01)
            .unwrap_or(64.0)
    } else {
        STEPS
            .iter()
            .rev()
            .copied()
            .find(|s| *s < z * 0.99)
            .unwrap_or(0.01)
    }
}

fn default_swatches() -> Vec<Color> {
    [
        "#000000", "#ffffff", "#808080", "#ff0000", "#ff8000", "#ffff00", "#80ff00", "#00ff00",
        "#00ff80", "#00ffff", "#0080ff", "#0000ff", "#8000ff", "#ff00ff", "#ff0080", "#7f3f1f",
        "#f2d0a4", "#1f3b5c", "#2e7d32", "#c62828",
    ]
    .iter()
    .map(|h| Color::from_hex(h).unwrap())
    .collect()
}

impl eframe::App for KromaApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_ui(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.save_settings(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

impl KromaApp {
    /// One full UI frame (separate from `eframe::App::ui` so tests can drive it headlessly).
    pub fn frame_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if self.checker.is_none() {
            self.checker = Some(checker_texture(&ctx));
        }
        for t in &mut self.tabs {
            t.thumbs_regen_this_frame = 0;
        }
        // window close with unsaved work
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            if let Some(i) = self.tabs.iter().position(|t| t.dirty) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.active_tab = i;
                self.dialog = Some(Dialog::ConfirmClose {
                    tab: i,
                    quitting: true,
                });
            }
        }
        // drag & drop files
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        for p in dropped {
            self.pending.push(Cmd::OpenPath(p));
        }
        // pen pressure from touch/pen events
        ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::Touch { force: Some(f), .. } = e {
                    self.pressure = f.clamp(0.05, 1.0);
                }
            }
        });
        if self.dialog.is_none() && !self.palette_open {
            self.shortcuts(&ctx);
        } else if ctx.input(|i| i.key_pressed(Key::Escape)) && self.palette_open {
            self.palette_open = false;
        }

        crate::panels::menu_bar(self, ui);
        crate::panels::options_bar(self, ui);
        crate::panels::status_bar(self, ui);
        crate::panels::toolbox(self, ui);
        crate::panels::right_panels(self, ui);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(crate::tab::CANVAS_BG))
            .show(ui, |ui| {
                crate::panels::doc_tabs(self, ui);
                crate::canvas::canvas(self, ui);
            });
        crate::dialogs::show(self, &ctx);
        crate::panels::command_palette(self, &ctx);

        let cmds: Vec<Cmd> = std::mem::take(&mut self.pending);
        for c in cmds {
            self.run(&ctx, c);
        }
        if self
            .batch
            .as_ref()
            .map(|b| !b.lock().unwrap().finished)
            .unwrap_or(false)
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        self.status_time += ctx.input(|i| i.stable_dt) as f64;
        self.tick_autosave();
    }

    fn tick_autosave(&mut self) {
        const INTERVAL: u64 = 120;
        if self.last_autosave.elapsed().as_secs() < INTERVAL {
            return;
        }
        self.last_autosave = std::time::Instant::now();
        let jobs: Vec<(u64, Document)> = self
            .tabs
            .iter()
            .filter(|t| t.dirty)
            .map(|t| (t.id, t.doc.clone()))
            .collect();
        if jobs.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            for (id, doc) in jobs {
                let _ = crate::recovery::write(id, &doc);
            }
        });
    }
}
