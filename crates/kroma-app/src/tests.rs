//! Headless UI integration tests: drive real frames with synthetic input.

use crate::app::{Cmd, KromaApp};
use crate::tab::DocTab;
use crate::tools::Tool;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use kroma_core::layer::{Layer, LayerKind};
use kroma_core::text::TextStyle;
use kroma_core::{Color, Document};

struct H {
    ctx: egui::Context,
    app: KromaApp,
    t: f64,
    mods: Modifiers,
}

fn small_doc() -> Document {
    let mut d = Document::new(320, 200, Some(Color::WHITE));
    let id = d.new_raster_layer();
    {
        let img = d.layer_mut(id).unwrap().image_mut();
        for y in 40..160 {
            for x in 60..260 {
                img.set(x, y, [0.8, 0.3, 0.2, 1.0]);
            }
        }
    }
    d.add_layer(Layer::text(
        TextStyle {
            text: "Hi".into(),
            size: 30.0,
            ..Default::default()
        },
        320,
        200,
    ));
    d.add_adjustment_layer(kroma_core::adjust::Adjustment::Vibrance {
        vibrance: 0.2,
        saturation: 0.0,
    });
    d.active = id;
    d
}

impl H {
    fn new(doc: Document) -> H {
        let ctx = egui::Context::default();
        let mut app = KromaApp::build(&ctx, None, vec![]);
        app.tabs.clear();
        app.tabs.push(DocTab::new(doc, None));
        app.active_tab = 0;
        let mut h = H {
            ctx,
            app,
            t: 0.0,
            mods: Modifiers::NONE,
        };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }
    fn frame(&mut self, events: Vec<Event>) {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1480.0, 920.0))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        self.t += 1.0 / 60.0;
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(raw, |ui| app.frame_ui(ui));
        out.textures_delta.clear();
    }
    fn canvas(&self) -> Rect {
        self.ctx
            .data(|d| d.get_temp::<Rect>(egui::Id::new("canvas_rect")))
            .expect("canvas rect")
    }
    /// Screen position of a document coordinate.
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let t = &self.app.tabs[self.app.active_tab];
        t.doc_to_screen(self.canvas(), x, y)
    }
    fn press(&mut self, p: Pos2, pressed: bool) {
        let mods = self.mods;
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed,
            modifiers: mods,
        }]);
    }
    fn drag(&mut self, a: Pos2, b: Pos2) {
        self.frame(vec![Event::PointerMoved(a)]);
        self.press(a, true);
        for i in 1..=8 {
            let p = a + (b - a) * (i as f32 / 8.0);
            self.frame(vec![Event::PointerMoved(p)]);
        }
        self.press(b, false);
        self.frame(vec![]);
    }
    fn click(&mut self, p: Pos2) {
        self.frame(vec![Event::PointerMoved(p)]);
        self.press(p, true);
        self.press(p, false);
        self.frame(vec![]);
    }
    fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
    }
    fn cmd(&mut self, c: Cmd) {
        self.app.pending.push(c);
        self.frame(vec![]);
        self.frame(vec![]);
    }
    fn tab(&self) -> &DocTab {
        &self.app.tabs[self.app.active_tab]
    }
}

#[test]
fn showcase_renders() {
    let mut h = H::new(crate::demo::showcase());
    for _ in 0..3 {
        h.frame(vec![]);
    }
    assert!(!h.tab().tiles.is_empty());
}

#[test]
fn brush_stroke_paints_and_undoes() {
    let mut h = H::new(small_doc());
    h.app.tool = Tool::Brush;
    h.app.fg = Color::rgb(0.0, 0.0, 1.0);
    let before = h.tab().doc.active_layer().unwrap().image().get(20, 100);
    h.drag(h.at(10.0, 100.0), h.at(50.0, 100.0));
    let after = h.tab().doc.active_layer().unwrap().image().get(20, 100);
    assert_ne!(before, after, "brush did not paint");
    assert!(after[2] > 0.9 && after[3] > 0.9, "{after:?}");
    assert_eq!(h.tab().history.names().last(), Some("Brush Tool"));
    h.key(Key::Z, Modifiers::COMMAND);
    let undone = h.tab().doc.active_layer().unwrap().image().get(20, 100);
    assert_eq!(undone, before, "undo restores pixels");
}

#[test]
fn marquee_wand_and_fill() {
    let mut h = H::new(small_doc());
    h.app.tool = Tool::MarqueeRect;
    h.drag(h.at(10.0, 10.0), h.at(100.0, 80.0));
    let s = h.tab().doc.selection.clone().expect("selection");
    assert!(s.get(50, 50) > 0.9 && s.get(150, 150) < 0.1);
    h.key(Key::D, Modifiers::COMMAND);
    assert!(h.tab().doc.selection.is_none());
    h.app.tool = Tool::MagicWand;
    h.click(h.at(150.0, 100.0));
    let s = h.tab().doc.selection.clone().expect("wand selection");
    assert!(s.get(150, 100) > 0.9 && s.get(10, 10) < 0.1);
    // Edit > Fill with foreground through the dialog + Enter
    h.app.fg = Color::rgb(0.0, 1.0, 0.0);
    h.cmd(Cmd::FillDialog);
    h.key(Key::Enter, Modifiers::NONE);
    let p = h.tab().doc.active_layer().unwrap().image().get(150, 100);
    assert!(p[1] > 0.9, "fill applied {p:?}");
}

#[test]
fn adjustment_dialog_preview_ok_and_cancel() {
    let mut h = H::new(small_doc());
    let orig = h.tab().doc.active_layer().unwrap().image().get(150, 100);
    h.cmd(Cmd::Adjust(kroma_core::adjust::Adjustment::Invert));
    // live preview already applied
    let prev = h.tab().doc.active_layer().unwrap().image().get(150, 100);
    assert!((prev[0] - (1.0 - orig[0])).abs() < 1e-4);
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(
        h.tab().doc.active_layer().unwrap().image().get(150, 100),
        orig,
        "cancel restores"
    );
    h.cmd(Cmd::Adjust(kroma_core::adjust::Adjustment::Invert));
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.app.dialog.is_none());
    assert_eq!(h.tab().history.names().last(), Some("Invert"));
}

#[test]
fn free_transform_moves_layer() {
    let mut h = H::new(small_doc());
    h.cmd(Cmd::FreeTransform);
    assert!(h.tab().transform.is_some());
    h.drag(h.at(150.0, 100.0), h.at(180.0, 100.0));
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.tab().transform.is_none());
    let img = h.tab().doc.active_layer().unwrap().image();
    assert!(img.get(70, 100)[3] < 0.1, "left edge moved away");
    assert!(img.get(280, 100)[3] > 0.9, "content moved right");
}

#[test]
fn text_tool_creates_layer() {
    let mut h = H::new(small_doc());
    h.app.tool = Tool::Text;
    let n = h.tab().doc.layer_count();
    h.click(h.at(20.0, 150.0));
    assert_eq!(h.tab().doc.layer_count(), n + 1);
    assert!(matches!(
        h.tab().doc.active_layer().unwrap().kind,
        LayerKind::Text(_)
    ));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(h.app.dialog.is_none());
}

#[test]
fn crop_tool_and_spot_heal() {
    let mut h = H::new(small_doc());
    h.app.tool = Tool::SpotHeal;
    h.app.opts.retouch.size = 12.0;
    h.drag(h.at(100.0, 100.0), h.at(110.0, 100.0));
    assert_eq!(h.tab().history.names().last(), Some("Spot Healing Brush"));
    h.app.tool = Tool::Crop;
    h.drag(h.at(20.0, 20.0), h.at(220.0, 120.0));
    assert!(h.tab().crop_pending.is_some());
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!((h.tab().doc.width, h.tab().doc.height), (200, 100));
}

/// Run (almost) every command from the palette on a fresh document, confirming dialogs with
/// Enter, to catch panics anywhere in the command surface.
#[test]
fn every_command_smoke() {
    let skip = [
        "Open",
        "Place",
        "Save",
        "Export",
        "Batch",
        "Close Document",
        "Load JSON",
    ];
    let all = crate::panels::all_commands();
    let mut ran = 0;
    for (name, cmd, _) in all {
        if skip.iter().any(|s| name.contains(s)) {
            continue;
        }
        let mut h = H::new(small_doc());
        // give selection-dependent commands something to work with
        if name.contains("Selection")
            || name.contains("Content-Aware")
            || name.contains("Stroke")
            || name.contains("Grow")
            || name.contains("Similar")
        {
            h.app.pending.push(Cmd::SelectAll);
            h.frame(vec![]);
            let s = kroma_core::selection::rect_mask(320, 200, 100.0, 60.0, 180.0, 120.0);
            h.app.tabs[0].doc.selection = Some(s);
        }
        h.cmd(cmd);
        h.key(Key::Enter, Modifiers::NONE);
        h.frame(vec![]);
        if let Some(t) = h.app.tabs.first_mut() {
            t.undo();
            t.redo();
        }
        h.frame(vec![]);
        ran += 1;
    }
    assert!(ran > 150, "ran {ran} commands");
}

#[test]
fn recovery_write_scan_and_open() {
    let dir = std::env::temp_dir().join(format!("kroma_recovery_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("KROMA_RECOVERY_DIR", &dir);
    let doc = small_doc();
    crate::recovery::write(7, &doc).unwrap();
    let entries = crate::recovery::scan();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].tab_id, 7);
    let path = entries[0].path.clone();
    let opened = kroma_core::io::open(&path).unwrap();
    assert_eq!(opened.width, 320);
    let ctx = egui::Context::default();
    let app = KromaApp::build(&ctx, None, vec![]);
    assert!(matches!(
        app.dialog,
        Some(crate::dialogs::Dialog::Recovery { .. })
    ));
    crate::recovery::delete(7);
    assert!(crate::recovery::scan().is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

struct MemStorage {
    kv: std::collections::HashMap<String, String>,
}

impl eframe::Storage for MemStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.kv.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.kv.insert(key.to_owned(), value);
    }
    fn remove_string(&mut self, key: &str) {
        self.kv.remove(key);
    }
    fn flush(&mut self) {}
}

#[test]
fn settings_persist_tooling_and_swatches() {
    let ctx = egui::Context::default();
    let mut app = KromaApp::build(&ctx, None, vec![]);
    app.fg = Color::from_hex("#112233").unwrap();
    app.opts.brush.size = 77.0;
    app.swatches.push(Color::from_hex("#abcdef").unwrap());
    app.actions.push(kroma_core::ops::Action {
        name: "Test Action".into(),
        ops: vec![],
    });
    let mut storage = MemStorage {
        kv: std::collections::HashMap::new(),
    };
    app.save_settings(&mut storage);
    let app2 = KromaApp::build(&ctx, Some(&storage), vec![]);
    assert!((app2.opts.brush.size - 77.0).abs() < 0.01);
    assert_eq!(app2.fg, app.fg);
    assert!(app2
        .swatches
        .iter()
        .any(|c| *c == Color::from_hex("#abcdef").unwrap()));
    assert!(app2.actions.iter().any(|a| a.name == "Test Action"));
}
