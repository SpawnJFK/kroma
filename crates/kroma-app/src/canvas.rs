//! The canvas: rendering (tiles, checkerboard, overlays) and all on-canvas tool interaction.

use crate::app::{Cmd, KromaApp};
use crate::dialogs::Dialog;
use crate::tab::{DocTab, FreeTransform, Interaction, LiquifySession, TransformDrag};
use crate::tools::Tool;
use egui::{
    Color32, CursorIcon, Id, PointerButton, Pos2, Rect as ERect, Sense, Shape, Stroke as EStroke,
    Ui, Vec2,
};
use kroma_core::brush::{BrushTool, Stroke};
use kroma_core::buffer::{Image, Mask, Rect};
use kroma_core::layer::{Layer, LayerKind};
use kroma_core::selection::{self, SelectOp};
use kroma_core::transform::Liquify;
use kroma_core::Color;

fn sel_op(app: &KromaApp, m: egui::Modifiers) -> SelectOp {
    match (m.shift, m.alt) {
        (true, true) => SelectOp::Intersect,
        (true, false) => SelectOp::Add,
        (false, true) => SelectOp::Subtract,
        _ => app.opts.select_op,
    }
}

pub fn canvas(app: &mut KromaApp, ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    ui.ctx()
        .data_mut(|d| d.insert_temp(Id::new("canvas_rect"), rect));
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    let painter = ui.painter_at(rect);

    if app.tabs.is_empty() {
        welcome(app, ui, rect);
        return;
    }
    let mods = ctx.input(|i| i.modifiers);
    let hover = resp.hover_pos();
    let pressure = app.pressure;
    let checker = app.checker.as_ref().map(|t| t.id());
    let tool = if app.space_down { Tool::Hand } else { app.tool };
    let ai = app.active_tab;

    {
        let tab = &mut app.tabs[ai];
        if tab.needs_fit {
            tab.fit(rect);
        }
        // ---- zoom & pan
        if resp.hovered() {
            let (scroll, zd) = ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
            if let Some(p) = hover {
                if (zd - 1.0).abs() > 1e-4 {
                    tab.zoom_about(rect, p, tab.zoom * zd);
                } else if scroll != Vec2::ZERO {
                    if mods.shift || mods.command {
                        tab.offset += scroll;
                    } else {
                        let f = (scroll.y * 0.0025).exp();
                        tab.zoom_about(rect, p, tab.zoom * f);
                    }
                }
            }
        }
        if resp.dragged_by(PointerButton::Middle) {
            tab.offset += resp.drag_delta();
        }
        tab.sync_textures(&ctx);

        // ---- draw document
        let doc_rect = tab.doc_rect_on_screen(rect);
        painter.rect_filled(doc_rect.expand(1.0), 0.0, Color32::from_black_alpha(120));
        if let Some(ch) = checker {
            let vis = doc_rect.intersect(rect);
            if vis.is_positive() {
                let uv = ERect::from_min_max(
                    Pos2::new(
                        (vis.min.x - doc_rect.min.x) / 16.0,
                        (vis.min.y - doc_rect.min.y) / 16.0,
                    ),
                    Pos2::new(
                        (vis.max.x - doc_rect.min.x) / 16.0,
                        (vis.max.y - doc_rect.min.y) / 16.0,
                    ),
                );
                painter.image(ch, vis, uv, Color32::WHITE);
            }
        }
        for t in &tab.tiles {
            let r = ERect::from_min_max(
                tab.doc_to_screen(rect, t.rect.x as f32, t.rect.y as f32),
                tab.doc_to_screen(rect, t.rect.right() as f32, t.rect.bottom() as f32),
            );
            if r.intersects(rect) {
                painter.image(
                    t.tex.id(),
                    r,
                    ERect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
        // pixel grid
        if app.show_pixel_grid && tab.zoom >= 12.0 {
            let vis = doc_rect.intersect(rect);
            let (x0, y0) = tab.screen_to_doc(rect, vis.min);
            let (x1, y1) = tab.screen_to_doc(rect, vis.max);
            let st = EStroke::new(1.0, Color32::from_white_alpha(25));
            for x in x0.floor() as i32..=x1.ceil() as i32 {
                let sx = tab.doc_to_screen(rect, x as f32, 0.0).x;
                painter.line_segment([Pos2::new(sx, vis.min.y), Pos2::new(sx, vis.max.y)], st);
            }
            for y in y0.floor() as i32..=y1.ceil() as i32 {
                let sy = tab.doc_to_screen(rect, 0.0, y as f32).y;
                painter.line_segment([Pos2::new(vis.min.x, sy), Pos2::new(vis.max.x, sy)], st);
            }
        }
        if app.show_guides {
            for g in &tab.doc.guides {
                let (a, b) = if g.vertical {
                    let x = tab.doc_to_screen(rect, g.pos, 0.0).x;
                    (Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom()))
                } else {
                    let y = tab.doc_to_screen(rect, 0.0, g.pos).y;
                    (Pos2::new(rect.left(), y), Pos2::new(rect.right(), y))
                };
                painter.line_segment([a, b], EStroke::new(1.0, Color32::from_rgb(0, 220, 255)));
            }
        }
        // marching ants
        let t_anim = ctx.input(|i| i.time) as f32;
        let zoom = tab.zoom;
        let origin = rect.min + tab.offset;
        if let Some(segs) = tab.selection_segments() {
            let mut shapes = Vec::with_capacity(segs.len() * 2);
            let visible = rect.expand(2.0);
            let mut lines_white = Vec::new();
            for (a, b) in segs.iter() {
                let pa = origin + a.to_vec2() * zoom;
                let pb = origin + b.to_vec2() * zoom;
                if !visible.intersects(ERect::from_two_pos(pa, pb).expand(1.0)) {
                    continue;
                }
                lines_white.push(Shape::line_segment(
                    [pa, pb],
                    EStroke::new(1.0, Color32::WHITE),
                ));
                Shape::dashed_line_many_with_offset(
                    &[pa, pb],
                    EStroke::new(1.0, Color32::BLACK),
                    &[4.0],
                    &[4.0],
                    (t_anim * 16.0) % 8.0 + (pa.x + pa.y) % 8.0,
                    &mut shapes,
                );
            }
            painter.extend(lines_white);
            painter.extend(shapes);
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        if let Some(p) = hover {
            app.cursor_doc = Some(tab.screen_to_doc(rect, p));
        } else {
            app.cursor_doc = None;
        }
    }

    // ---- free transform takes over input
    if app.tabs[ai].transform.is_some() {
        transform_interaction(app, &resp, rect, &painter, mods);
        return;
    }

    let tab_snapshot_zoom = app.tabs[ai].zoom;

    // ---- tool input
    let p_doc = |tab: &DocTab, p: Pos2| tab.screen_to_doc(rect, p);
    if resp.drag_started_by(PointerButton::Primary) {
        // start where the button went down, not where the drag threshold was crossed
        let origin = ctx
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos());
        if let Some(p) = origin {
            begin_drag(app, tool, p_doc(&app.tabs[ai], p), p, mods);
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            continue_drag(app, p_doc(&app.tabs[ai], p), p, mods, pressure);
        }
    }
    if resp.drag_stopped_by(PointerButton::Primary) {
        end_drag(app, mods);
    }
    if resp.clicked_by(PointerButton::Primary) {
        if let Some(p) = resp.interact_pointer_pos() {
            click(
                app,
                tool,
                p_doc(&app.tabs[ai], p),
                p,
                mods,
                resp.double_clicked(),
                rect,
            );
        }
    }
    if resp.double_clicked() && tool == Tool::PolyLasso {
        finish_poly_lasso(app);
    }

    // ---- overlays for in-progress interactions
    let tab = &mut app.tabs[ai];
    let to_s = |x: f32, y: f32| tab.doc_to_screen(rect, x, y);
    match &tab.interaction {
        Interaction::Marquee {
            start,
            end,
            ellipse,
        } => {
            let r = ERect::from_two_pos(to_s(start.0, start.1), to_s(end.0, end.1));
            if *ellipse {
                painter.add(Shape::ellipse_stroke(
                    r.center(),
                    r.size() / 2.0,
                    EStroke::new(1.0, Color32::WHITE),
                ));
            } else {
                painter.rect_stroke(
                    r,
                    0.0,
                    EStroke::new(1.0, Color32::WHITE),
                    egui::StrokeKind::Middle,
                );
                painter.extend(Shape::dashed_line(
                    &[
                        r.left_top(),
                        r.right_top(),
                        r.right_bottom(),
                        r.left_bottom(),
                        r.left_top(),
                    ],
                    EStroke::new(1.0, Color32::BLACK),
                    4.0,
                    4.0,
                ));
            }
        }
        Interaction::Lasso { points } => {
            let pts: Vec<Pos2> = points.iter().map(|p| to_s(p.0, p.1)).collect();
            painter.add(Shape::line(pts, EStroke::new(1.0, Color32::WHITE)));
        }
        Interaction::Gradient { start, end } => {
            let (a, b) = (to_s(start.0, start.1), to_s(end.0, end.1));
            painter.line_segment([a, b], EStroke::new(3.0, Color32::BLACK));
            painter.line_segment([a, b], EStroke::new(1.0, Color32::WHITE));
            painter.circle_filled(a, 4.0, Color32::WHITE);
            painter.circle_filled(b, 4.0, Color32::WHITE);
        }
        Interaction::Shape { start, end } => {
            let r = ERect::from_two_pos(to_s(start.0, start.1), to_s(end.0, end.1));
            match app.opts.shape {
                kroma_core::shapes::ShapeKind::Ellipse => {
                    painter.add(Shape::ellipse_stroke(
                        r.center(),
                        r.size() / 2.0,
                        EStroke::new(1.0, Color32::from_rgb(0, 160, 255)),
                    ));
                }
                kroma_core::shapes::ShapeKind::Line { .. } => {
                    painter.line_segment(
                        [to_s(start.0, start.1), to_s(end.0, end.1)],
                        EStroke::new(1.5, Color32::from_rgb(0, 160, 255)),
                    );
                }
                _ => {
                    painter.rect_stroke(
                        r,
                        0.0,
                        EStroke::new(1.0, Color32::from_rgb(0, 160, 255)),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        Interaction::Crop { start, end } => {
            let r = ERect::from_two_pos(to_s(start.0, start.1), to_s(end.0, end.1));
            draw_crop_overlay(&painter, rect, r);
        }
        Interaction::SpotHeal { points, .. } => {
            let rad = app.opts.retouch.size * tab.zoom / 2.0;
            for p in points {
                painter.circle_filled(
                    to_s(p.0, p.1),
                    rad,
                    Color32::from_rgba_unmultiplied(255, 40, 120, 40),
                );
            }
        }
        _ => {}
    }
    if let Some(r) = tab.crop_pending {
        let sr = ERect::from_two_pos(
            to_s(r.x as f32, r.y as f32),
            to_s(r.right() as f32, r.bottom() as f32),
        );
        draw_crop_overlay(&painter, rect, sr);
    }
    if !tab.poly_points.is_empty() {
        let mut pts: Vec<Pos2> = tab.poly_points.iter().map(|p| to_s(p.0, p.1)).collect();
        if let Some(h) = hover {
            pts.push(h);
        }
        painter.add(Shape::line(pts.clone(), EStroke::new(1.0, Color32::WHITE)));
        painter.extend(Shape::dashed_line(
            &pts,
            EStroke::new(1.0, Color32::BLACK),
            4.0,
            4.0,
        ));
        ctx.request_repaint();
    }
    if let (Some(src), true) = (tab.clone_source, matches!(tool, Tool::Clone | Tool::Heal)) {
        let s = to_s(src.0, src.1);
        painter.line_segment(
            [s - Vec2::new(6.0, 0.0), s + Vec2::new(6.0, 0.0)],
            EStroke::new(1.5, Color32::WHITE),
        );
        painter.line_segment(
            [s - Vec2::new(0.0, 6.0), s + Vec2::new(0.0, 6.0)],
            EStroke::new(1.5, Color32::WHITE),
        );
    }

    // ---- cursor
    if let Some(h) = hover {
        if tool.is_brush_like() {
            let size = if tool == Tool::Liquify {
                app.opts.liquify_size
            } else {
                app.opts.brush_for(tool).map(|b| b.size).unwrap_or(10.0)
            };
            let r = (size * tab_snapshot_zoom / 2.0).max(1.0);
            painter.circle_stroke(h, r, EStroke::new(2.0, Color32::from_black_alpha(160)));
            painter.circle_stroke(h, r, EStroke::new(1.0, Color32::from_white_alpha(220)));
            ctx.set_cursor_icon(if r < 4.0 {
                CursorIcon::Crosshair
            } else {
                CursorIcon::None
            });
        } else {
            ctx.set_cursor_icon(match tool {
                Tool::Hand => {
                    if resp.dragged() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    }
                }
                Tool::Zoom => {
                    if mods.alt {
                        CursorIcon::ZoomOut
                    } else {
                        CursorIcon::ZoomIn
                    }
                }
                Tool::Move => CursorIcon::Move,
                Tool::Text => CursorIcon::Text,
                _ => CursorIcon::Crosshair,
            });
        }
    }
    if !matches!(app.tabs[ai].interaction, Interaction::None) {
        ctx.request_repaint();
    }
}

fn draw_crop_overlay(painter: &egui::Painter, canvas: ERect, r: ERect) {
    let shade = Color32::from_black_alpha(150);
    painter.rect_filled(
        ERect::from_min_max(canvas.min, Pos2::new(canvas.max.x, r.min.y)),
        0.0,
        shade,
    );
    painter.rect_filled(
        ERect::from_min_max(Pos2::new(canvas.min.x, r.max.y), canvas.max),
        0.0,
        shade,
    );
    painter.rect_filled(
        ERect::from_min_max(
            Pos2::new(canvas.min.x, r.min.y),
            Pos2::new(r.min.x, r.max.y),
        ),
        0.0,
        shade,
    );
    painter.rect_filled(
        ERect::from_min_max(
            Pos2::new(r.max.x, r.min.y),
            Pos2::new(canvas.max.x, r.max.y),
        ),
        0.0,
        shade,
    );
    painter.rect_stroke(
        r,
        0.0,
        EStroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Middle,
    );
    for i in 1..3 {
        let t = i as f32 / 3.0;
        let x = r.left() + r.width() * t;
        let y = r.top() + r.height() * t;
        painter.line_segment(
            [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
            EStroke::new(1.0, Color32::from_white_alpha(90)),
        );
        painter.line_segment(
            [Pos2::new(r.left(), y), Pos2::new(r.right(), y)],
            EStroke::new(1.0, Color32::from_white_alpha(90)),
        );
    }
}

fn welcome(app: &mut KromaApp, ui: &mut Ui, rect: ERect) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(40.0))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    child.add_space(rect.height() * 0.25);
    child.heading(egui::RichText::new("Kroma").size(42.0).strong());
    child.label("Layered image editing, built in Rust.");
    child.add_space(16.0);
    if child.button("➕  New document   (Ctrl+N)").clicked() {
        app.pending.push(Cmd::NewDoc);
    }
    if child.button("📂  Open…   (Ctrl+O)").clicked() {
        app.pending.push(Cmd::Open);
    }
    if child.button("📋  New from clipboard").clicked() {
        app.pending.push(Cmd::PasteAsNewDoc);
    }
    child.add_space(10.0);
    child.label(egui::RichText::new("…or drop image files anywhere on this window").weak());
    if !app.recent.is_empty() {
        child.add_space(20.0);
        child.label(egui::RichText::new("Recent").strong());
        for p in app.recent.clone().iter().take(8) {
            if child.link(p.display().to_string()).clicked() {
                app.pending.push(Cmd::OpenPath(p.clone()));
            }
        }
    }
}

fn brush_tool_for(app: &KromaApp, tool: Tool) -> Option<BrushTool> {
    Some(match tool {
        Tool::Brush => BrushTool::Paint,
        Tool::Pencil => BrushTool::Pencil,
        Tool::Eraser => BrushTool::Eraser,
        Tool::Clone => BrushTool::CloneStamp,
        Tool::Heal => BrushTool::Healing,
        Tool::Smudge => BrushTool::Smudge,
        Tool::BlurTool => BrushTool::Blur,
        Tool::SharpenTool => BrushTool::Sharpen,
        Tool::Dodge => BrushTool::Dodge(app.opts.tone_range),
        Tool::Burn => BrushTool::Burn(app.opts.tone_range),
        Tool::Sponge => BrushTool::Sponge {
            saturate: app.opts.sponge_saturate,
        },
        _ => return None,
    })
}

fn begin_drag(app: &mut KromaApp, tool: Tool, p: (f32, f32), screen: Pos2, mods: egui::Modifiers) {
    let ai = app.active_tab;
    match tool {
        Tool::Hand => app.tabs[ai].interaction = Interaction::Pan { last: screen },
        Tool::MarqueeRect | Tool::MarqueeEllipse => {
            app.tabs[ai].interaction = Interaction::Marquee {
                start: p,
                end: p,
                ellipse: tool == Tool::MarqueeEllipse,
            };
        }
        Tool::Lasso => app.tabs[ai].interaction = Interaction::Lasso { points: vec![p] },
        Tool::Crop => {
            app.tabs[ai].crop_pending = None;
            app.tabs[ai].interaction = Interaction::Crop { start: p, end: p };
        }
        Tool::Gradient => app.tabs[ai].interaction = Interaction::Gradient { start: p, end: p },
        Tool::Shape => app.tabs[ai].interaction = Interaction::Shape { start: p, end: p },
        Tool::Eyedropper => sample_color(app, p, mods.alt),
        Tool::Move => {
            if app.opts.move_auto_select {
                auto_select_layer(app, p);
            }
            let tab = &mut app.tabs[ai];
            let Some(l) = tab.doc.active_layer() else {
                return;
            };
            let text_origin = if let LayerKind::Text(s) = &l.kind {
                Some((s.x, s.y))
            } else {
                None
            };
            if !matches!(l.kind, LayerKind::Raster | LayerKind::Text(_)) {
                app.toast("Move works on pixel and text layers");
                return;
            }
            let layer = l.id;
            let original = l.image().clone();
            let original_mask = l.mask().cloned();
            let sel_original = tab.doc.selection.clone().filter(|_| text_origin.is_none());
            tab.interaction = Interaction::Moving {
                layer,
                original,
                original_mask,
                start: p,
                text_origin,
                sel_original,
            };
        }
        Tool::SpotHeal => {
            let tab = &mut app.tabs[ai];
            if !tab.active_is_pixel() {
                app.toast("Spot Healing needs a pixel layer");
                return;
            }
            let (w, h) = (tab.doc.width, tab.doc.height);
            let layer = tab.doc.active;
            tab.interaction = Interaction::SpotHeal {
                mask: Mask::new(w, h, 0.0),
                last: None,
                layer,
                points: vec![],
            };
            let size = app.opts.retouch.size;
            spot_heal_dab(&mut app.tabs[ai], p, size);
        }
        Tool::Liquify => {
            let tab = &mut app.tabs[ai];
            if tab.liquify.is_none() {
                let Some(l) = tab.doc.active_layer() else {
                    return;
                };
                if !matches!(l.kind, LayerKind::Raster) {
                    app.toast("Liquify needs a pixel layer");
                    return;
                }
                tab.liquify = Some(LiquifySession {
                    layer: l.id,
                    original: l.image().clone(),
                    field: Liquify::new(tab.doc.width, tab.doc.height),
                });
            }
            tab.interaction = Interaction::Liquify { last: p };
        }
        t if t.is_brush_like() => {
            if mods.alt && matches!(t, Tool::Clone | Tool::Heal) {
                let tab = &mut app.tabs[ai];
                tab.clone_source = Some(p);
                tab.clone_offset = None;
                app.toast("Clone source set");
                return;
            }
            let Some(bt) = brush_tool_for(app, t) else {
                return;
            };
            let mut settings = app.opts.brush_for(t).cloned().unwrap_or_default();
            settings.color = app.fg;
            let aligned = app.opts.clone_aligned;
            let sample_all = app.opts.clone_sample_all;
            let tab = &mut app.tabs[ai];
            let editing_mask =
                tab.editing_mask && matches!(t, Tool::Brush | Tool::Pencil | Tool::Eraser);
            let Some(layer) = tab.doc.active_layer() else {
                return;
            };
            let lid = layer.id;
            let name: &'static str = match t {
                Tool::Brush => "Brush Tool",
                Tool::Pencil => "Pencil",
                Tool::Eraser => "Eraser",
                Tool::Clone => "Clone Stamp",
                Tool::Heal => "Healing Brush",
                Tool::Smudge => "Smudge",
                Tool::BlurTool => "Blur",
                Tool::SharpenTool => "Sharpen",
                Tool::Dodge => "Dodge",
                Tool::Burn => "Burn",
                _ => "Sponge",
            };
            if editing_mask {
                let Some(m) = layer.mask() else { return };
                let stroke = Stroke::begin_mask(m, bt, settings);
                tab.interaction = Interaction::Painting {
                    stroke,
                    on_mask: true,
                    layer: lid,
                    name,
                };
            } else {
                if !matches!(layer.kind, LayerKind::Raster) {
                    let msg = format!(
                        "'{}' is a {} layer. Layer › Rasterize to paint on it.",
                        layer.name,
                        layer.kind_label()
                    );
                    app.toast(msg);
                    return;
                }
                let (source, offset) = if matches!(t, Tool::Clone | Tool::Heal) {
                    let Some(src) = tab.clone_source else {
                        app.toast("Alt+click to define a source point first");
                        return;
                    };
                    let off = match (aligned, tab.clone_offset) {
                        (true, Some(o)) => o,
                        _ => (src.0 - p.0, src.1 - p.1),
                    };
                    tab.clone_offset = Some(off);
                    let img = if sample_all {
                        tab.doc.composite()
                    } else {
                        layer.image().clone()
                    };
                    (Some(img), off)
                } else {
                    (None, (0.0, 0.0))
                };
                let stroke = Stroke::begin(layer.image(), bt, settings, source, offset);
                tab.paint_under = Some(std::sync::Arc::new(tab.doc.composite_omit_layer(lid)));
                tab.interaction = Interaction::Painting {
                    stroke,
                    on_mask: false,
                    layer: lid,
                    name,
                };
            }
            continue_drag(app, p, screen, mods, app.pressure);
        }
        _ => {}
    }
}

fn spot_heal_dab(tab: &mut DocTab, p: (f32, f32), size: f32) {
    if let Interaction::SpotHeal {
        mask, last, points, ..
    } = &mut tab.interaction
    {
        let r = size / 2.0;
        let from = last.unwrap_or(p);
        let dist = ((p.0 - from.0).powi(2) + (p.1 - from.1).powi(2)).sqrt();
        let steps = (dist / (r * 0.3).max(1.0)).ceil().max(1.0) as i32;
        let (w, h) = (mask.width as i32, mask.height as i32);
        for s in 0..=steps {
            let t = s as f32 / steps as f32;
            let (cx, cy) = (from.0 + (p.0 - from.0) * t, from.1 + (p.1 - from.1) * t);
            for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
                for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                    if x < 0 || y < 0 || x >= w || y >= h {
                        continue;
                    }
                    let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                    if d <= r {
                        mask.set(x as u32, y as u32, 1.0);
                    }
                }
            }
        }
        *last = Some(p);
        points.push(p);
    }
}

fn continue_drag(
    app: &mut KromaApp,
    p: (f32, f32),
    screen: Pos2,
    mods: egui::Modifiers,
    pressure: f32,
) {
    let ai = app.active_tab;
    let crop_ratio = app.opts.crop_ratio;
    let retouch_size = app.opts.retouch.size;
    let (lq_mode, lq_size, lq_pressure) = (
        app.opts.liquify_mode,
        app.opts.liquify_size,
        app.opts.liquify_pressure,
    );
    let tab = &mut app.tabs[ai];
    let sel = tab.doc.selection.clone();
    let mut dirty: Option<Rect> = None;
    let mut full = false;
    let mut spot = false;
    match &mut tab.interaction {
        Interaction::Pan { last } => {
            tab.offset += screen - *last;
            *last = screen;
        }
        Interaction::Marquee { start, end, .. } | Interaction::Shape { start, end } => {
            let mut e = p;
            if mods.shift {
                let d = (p.0 - start.0).abs().max((p.1 - start.1).abs());
                e = (
                    start.0 + d * (p.0 - start.0).signum(),
                    start.1 + d * (p.1 - start.1).signum(),
                );
            }
            *end = e;
        }
        Interaction::Gradient { end, start } => {
            let mut e = p;
            if mods.shift {
                // snap to 45°
                let (dx, dy) = (p.0 - start.0, p.1 - start.1);
                let a = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round()
                    * std::f32::consts::FRAC_PI_4;
                let l = (dx * dx + dy * dy).sqrt();
                e = (start.0 + a.cos() * l, start.1 + a.sin() * l);
            }
            *end = e;
        }
        Interaction::Crop { start, end } => {
            let mut e = p;
            if let Some((rw, rh)) = crop_ratio {
                let w = (p.0 - start.0).abs();
                let sy = if p.1 >= start.1 { 1.0 } else { -1.0 };
                e = (p.0, start.1 + sy * w * rh / rw);
            }
            *end = e;
        }
        Interaction::Lasso { points } => {
            if let Some(l) = points.last() {
                if (l.0 - p.0).abs() + (l.1 - p.1).abs() > 1.0 {
                    points.push(p);
                }
            }
        }
        Interaction::Painting {
            stroke,
            on_mask,
            layer,
            ..
        } => {
            let (on_mask, lid) = (*on_mask, *layer);
            if let Some(l) = tab.doc.layer_mut(lid) {
                let r = if on_mask {
                    match l.mask_mut() {
                        Some(m) => stroke.add_point_mask(m, p.0, p.1, pressure),
                        None => Rect::default(),
                    }
                } else {
                    stroke.add_point(l.image_mut(), sel.as_ref(), p.0, p.1, pressure)
                };
                dirty = Some(r);
            }
        }
        Interaction::Moving {
            layer,
            original,
            original_mask,
            start,
            text_origin,
            sel_original,
        } => {
            let (dx, dy) = (
                (p.0 - start.0).round() as i32,
                (p.1 - start.1).round() as i32,
            );
            let lid = *layer;
            let (w, h) = (tab.doc.width, tab.doc.height);
            if let Some((tx, ty)) = *text_origin {
                if let Some(l) = tab.doc.layer_mut(lid) {
                    if let LayerKind::Text(s) = &mut l.kind {
                        s.x = tx + dx as f32;
                        s.y = ty + dy as f32;
                    }
                    l.rerender(w, h);
                }
            } else if let Some(sel0) = sel_original {
                // move only the selected pixels; the hole stays transparent
                let mut moved = Image::new(w, h);
                let mut lifted = original.clone();
                let mut rest = original.clone();
                for (i, (a, b)) in lifted
                    .pixels_mut()
                    .iter_mut()
                    .zip(rest.pixels_mut().iter_mut())
                    .enumerate()
                {
                    let m = sel0.data()[i];
                    a[3] *= m;
                    b[3] *= 1.0 - m;
                }
                moved.blit(&rest, 0, 0);
                let mut shifted = Image::new(w, h);
                shifted.blit(&lifted, dx, dy);
                for (d, s) in moved.pixels_mut().iter_mut().zip(shifted.pixels()) {
                    *d = kroma_core::blend::composite_px(
                        *d,
                        *s,
                        kroma_core::blend::BlendMode::Normal,
                        1.0,
                        0,
                        0,
                    );
                }
                if let Some(l) = tab.doc.layer_mut(lid) {
                    l.set_image(moved);
                }
                let mut ns = Mask::new(w, h, 0.0);
                for y in 0..h as i32 {
                    for x in 0..w as i32 {
                        let v = sel0.get_i(x - dx, y - dy);
                        if v > 0.0 {
                            ns.set(x as u32, y as u32, v);
                        }
                    }
                }
                tab.doc.selection = Some(ns);
            } else {
                let mut img = Image::new(original.width, original.height);
                img.blit(original, dx, dy);
                let mask = original_mask.as_ref().map(|m| {
                    let mut out = Mask::new(m.width, m.height, 0.0);
                    for y in 0..m.height as i32 {
                        for x in 0..m.width as i32 {
                            out.set(x as u32, y as u32, m.get_i(x - dx, y - dy));
                        }
                    }
                    out
                });
                if let Some(l) = tab.doc.layer_mut(lid) {
                    l.set_image(img);
                    if mask.is_some() {
                        l.set_mask(mask);
                    }
                }
            }
            full = true;
        }
        Interaction::SpotHeal { .. } => spot = true,
        Interaction::Liquify { last } => {
            if let Some(s) = &mut tab.liquify {
                s.field.stroke(
                    lq_mode,
                    last.0,
                    last.1,
                    p.0,
                    p.1,
                    lq_size / 2.0,
                    lq_pressure,
                );
                let img = s.field.render(&s.original);
                let lid = s.layer;
                if let Some(l) = tab.doc.layer_mut(lid) {
                    l.set_image(img);
                }
                full = true;
            }
            *last = p;
        }
        Interaction::None => {}
    }
    if spot {
        spot_heal_dab(tab, p, retouch_size);
    }
    if let Some(r) = dirty {
        tab.mark_dirty(r);
    }
    if full {
        tab.full_redraw = true;
    }
}

fn end_drag(app: &mut KromaApp, mods: egui::Modifiers) {
    let ai = app.active_tab;
    let op = sel_op(app, mods);
    let feather = app.opts.feather;
    let fg = app.fg;
    let bg = app.bg;
    let interaction = std::mem::replace(&mut app.tabs[ai].interaction, Interaction::None);
    let (w, h) = (app.tabs[ai].doc.width, app.tabs[ai].doc.height);
    match interaction {
        Interaction::Painting { stroke, name, .. } => {
            let r = stroke.dirty;
            app.tabs[ai].paint_under = None;
            app.tabs[ai].commit_region(name, r);
        }
        Interaction::Marquee {
            start,
            end,
            ellipse,
        } => {
            let tiny = (end.0 - start.0).abs() < 2.0 && (end.1 - start.1).abs() < 2.0;
            let tab = &mut app.tabs[ai];
            if tiny {
                if op == SelectOp::Replace && tab.doc.selection.is_some() {
                    app.last_selection = tab.doc.selection.take();
                    app.tabs[ai].commit("Deselect");
                }
                return;
            }
            let mut m = if ellipse {
                selection::ellipse_mask(w, h, start.0, start.1, end.0, end.1)
            } else {
                selection::rect_mask(w, h, start.0, start.1, end.0, end.1)
            };
            if feather > 0.0 {
                m = selection::feather(&m, feather);
            }
            tab.doc.selection = selection::combine(tab.doc.selection.as_ref(), m, op);
            tab.commit(if ellipse {
                "Elliptical Marquee"
            } else {
                "Rectangular Marquee"
            });
        }
        Interaction::Lasso { points } => {
            let tab = &mut app.tabs[ai];
            if points.len() < 3 {
                return;
            }
            let mut m = selection::polygon_mask(w, h, &points);
            if feather > 0.0 {
                m = selection::feather(&m, feather);
            }
            tab.doc.selection = selection::combine(tab.doc.selection.as_ref(), m, op);
            tab.commit("Lasso");
        }
        Interaction::Gradient { start, end } => {
            let mut g = app.opts.gradient.clone();
            if app.opts.gradient_use_fg_bg {
                g.gradient = kroma_core::gradient::Gradient::two(fg, bg);
            }
            let tab = &mut app.tabs[ai];
            if !tab.active_is_pixel() {
                app.toast("Gradient needs a pixel layer");
                return;
            }
            let sel = tab.doc.selection.clone();
            let l = tab.doc.active_layer_mut().unwrap();
            kroma_core::gradient::draw_gradient(l.image_mut(), sel.as_ref(), &g, start, end);
            tab.commit("Gradient");
        }
        Interaction::Shape { start, end } => {
            if (end.0 - start.0).abs() < 1.0 && (end.1 - start.1).abs() < 1.0 {
                return;
            }
            let kind = app.opts.shape;
            let style = app.opts.shape_style.clone();
            let new_layer = app.opts.shape_new_layer;
            let tab = &mut app.tabs[ai];
            if new_layer || !tab.active_is_pixel() {
                let n = tab.doc.layer_count() + 1;
                tab.doc
                    .add_layer(Layer::raster(&format!("{} {n}", kind.name()), w, h));
            }
            let sel = tab.doc.selection.clone();
            let l = tab.doc.active_layer_mut().unwrap();
            kroma_core::shapes::draw_shape(l.image_mut(), sel.as_ref(), kind, &style, start, end);
            tab.commit(kind.name());
        }
        Interaction::Crop { start, end } => {
            let r = Rect::from_points(
                start.0.round() as i32,
                start.1.round() as i32,
                end.0.round() as i32,
                end.1.round() as i32,
            );
            if r.w > 2 && r.h > 2 {
                app.tabs[ai].crop_pending = Some(r);
                app.toast("Press Enter to crop, Esc to cancel. Crop can extend past the canvas to add space.");
            }
        }
        Interaction::Moving { .. } => app.tabs[ai].commit("Move"),
        Interaction::SpotHeal { mask, layer, .. } => {
            let tab = &mut app.tabs[ai];
            let m = kroma_core::filter::morph_mask(&mask, 2.0);
            if let Some(l) = tab.doc.layer_mut(layer) {
                let out = kroma_core::inpaint::content_aware_fill(l.image(), &m);
                l.set_image(out);
            }
            tab.commit("Spot Healing Brush");
        }
        Interaction::Liquify { .. } | Interaction::Pan { .. } | Interaction::None => {}
    }
}

fn click(
    app: &mut KromaApp,
    tool: Tool,
    p: (f32, f32),
    screen: Pos2,
    mods: egui::Modifiers,
    double: bool,
    rect: ERect,
) {
    let ai = app.active_tab;
    let (w, h) = (app.tabs[ai].doc.width, app.tabs[ai].doc.height);
    let inside = p.0 >= 0.0 && p.1 >= 0.0 && p.0 < w as f32 && p.1 < h as f32;
    match tool {
        Tool::Zoom => {
            let t = &mut app.tabs[ai];
            let z = crate::app::next_zoom(t.zoom, !mods.alt);
            t.zoom_about(rect, screen, z);
        }
        Tool::Eyedropper => sample_color(app, p, mods.alt),
        Tool::MagicWand if inside => {
            let op = sel_op(app, mods);
            let (tol, cont, aa, all) = (
                app.opts.wand_tolerance,
                app.opts.wand_contiguous,
                app.opts.antialias,
                app.opts.wand_sample_all,
            );
            let tab = &mut app.tabs[ai];
            let img = if all {
                tab.doc.composite()
            } else {
                tab.doc
                    .active_layer()
                    .filter(|l| l.is_raster_like())
                    .map(|l| l.image().clone())
                    .unwrap_or_else(|| tab.doc.composite())
            };
            let m = selection::magic_wand(&img, p.0 as u32, p.1 as u32, tol, cont, aa);
            tab.doc.selection = selection::combine(tab.doc.selection.as_ref(), m, op);
            tab.commit("Magic Wand");
        }
        Tool::Bucket if inside => {
            let (tol, cont, op) = (
                app.opts.bucket_tolerance,
                app.opts.bucket_contiguous,
                app.opts.bucket_opacity,
            );
            let fg = app.fg;
            let tab = &mut app.tabs[ai];
            if !tab.active_is_pixel() {
                app.toast("Paint Bucket needs a pixel layer");
                return;
            }
            let sel = tab.doc.selection.clone();
            let l = tab.doc.active_layer_mut().unwrap();
            kroma_core::brush::bucket_fill(
                l.image_mut(),
                sel.as_ref(),
                p.0 as u32,
                p.1 as u32,
                fg,
                tol,
                cont,
                op,
                kroma_core::blend::BlendMode::Normal,
            );
            tab.commit("Paint Bucket");
        }
        Tool::PolyLasso => {
            let tab = &mut app.tabs[ai];
            if !double {
                tab.poly_points.push(p);
            }
        }
        Tool::Text => {
            let tab = &mut app.tabs[ai];
            // click on an existing text layer → edit it
            let hit = tab
                .doc
                .flat_list()
                .into_iter()
                .find_map(|(_, l)| match &l.kind {
                    LayerKind::Text(s) if l.visible => {
                        let (x, y, bw, bh) = kroma_core::text::bounds(s);
                        if p.0 >= x && p.0 <= x + bw && p.1 >= y && p.1 <= y + bh {
                            Some(l.id)
                        } else {
                            None
                        }
                    }
                    _ => None,
                });
            let id = match hit {
                Some(id) => {
                    tab.doc.active = id;
                    id
                }
                None => {
                    let mut style = app.opts.text.clone();
                    style.text = "Type here".into();
                    style.x = p.0;
                    style.y = p.1;
                    style.color = app.fg;
                    let l = Layer::text(style, w, h);
                    let tab = &mut app.tabs[ai];
                    let id = tab.doc.add_layer(l);
                    tab.commit("New Type Layer");
                    id
                }
            };
            app.dialog = Some(Dialog::TextEdit {
                layer: id,
                select_all: hit.is_none(),
            });
        }
        Tool::Move if app.opts.move_auto_select => auto_select_layer(app, p),
        Tool::MarqueeRect | Tool::MarqueeEllipse | Tool::Lasso => {
            let tab = &mut app.tabs[ai];
            if tab.doc.selection.is_some() && sel_op(app, mods) == SelectOp::Replace {
                let tab = &mut app.tabs[ai];
                app.last_selection = tab.doc.selection.take();
                app.tabs[ai].commit("Deselect");
            }
        }
        _ => {}
    }
}

pub fn finish_poly_lasso(app: &mut KromaApp) {
    let ai = app.active_tab;
    let Some(tab) = app.tabs.get_mut(ai) else {
        return;
    };
    let pts = std::mem::take(&mut tab.poly_points);
    if pts.len() < 3 {
        return;
    }
    let (w, h) = (tab.doc.width, tab.doc.height);
    let mut m = selection::polygon_mask(w, h, &pts);
    if app.opts.feather > 0.0 {
        m = selection::feather(&m, app.opts.feather);
    }
    let op = app.opts.select_op;
    let tab = &mut app.tabs[ai];
    tab.doc.selection = selection::combine(tab.doc.selection.as_ref(), m, op);
    tab.commit("Polygonal Lasso");
}

fn sample_color(app: &mut KromaApp, p: (f32, f32), to_bg: bool) {
    let all = app.opts.eyedropper_sample_all;
    let tab = &app.tabs[app.active_tab];
    let (w, h) = (tab.doc.width, tab.doc.height);
    if p.0 < 0.0 || p.1 < 0.0 || p.0 >= w as f32 || p.1 >= h as f32 {
        return;
    }
    let px = if all {
        tab.doc.sample_merged(p.0 as u32, p.1 as u32)
    } else {
        match tab.doc.active_layer() {
            Some(l) if l.is_raster_like() => l.image().get(p.0 as u32, p.1 as u32),
            _ => tab.doc.sample_merged(p.0 as u32, p.1 as u32),
        }
    };
    let c = Color::rgba(px[0], px[1], px[2], 1.0);
    if to_bg {
        app.bg = c;
    } else {
        app.fg = c;
    }
}

fn auto_select_layer(app: &mut KromaApp, p: (f32, f32)) {
    let tab = &mut app.tabs[app.active_tab];
    let (x, y) = (p.0 as i32, p.1 as i32);
    if x < 0 || y < 0 || x >= tab.doc.width as i32 || y >= tab.doc.height as i32 {
        return;
    }
    let hit = tab
        .doc
        .flat_list()
        .into_iter()
        .find(|(_, l)| {
            l.visible && l.is_raster_like() && l.image().get(x as u32, y as u32)[3] > 0.1
        })
        .map(|(_, l)| l.id);
    if let Some(id) = hit {
        tab.doc.active = id;
    }
}

pub fn nudge(app: &mut KromaApp, dx: i32, dy: i32) {
    let Some(tab) = app.tabs.get_mut(app.active_tab) else {
        return;
    };
    let (w, h) = (tab.doc.width, tab.doc.height);
    let Some(l) = tab.doc.active_layer_mut() else {
        return;
    };
    match &mut l.kind {
        LayerKind::Text(s) => {
            s.x += dx as f32;
            s.y += dy as f32;
            l.rerender(w, h);
        }
        LayerKind::Raster => {
            let mut img = Image::new(w, h);
            img.blit(l.image(), dx, dy);
            l.set_image(img);
        }
        _ => return,
    }
    tab.commit("Nudge");
}

// ------------------------------------------------------------------ free transform

const HANDLE: f32 = 7.0;

fn transform_interaction(
    app: &mut KromaApp,
    resp: &egui::Response,
    rect: ERect,
    painter: &egui::Painter,
    mods: egui::Modifiers,
) {
    let ai = app.active_tab;
    let tab = &mut app.tabs[ai];
    let (w, h) = (tab.doc.width, tab.doc.height);
    let mut changed = false;
    {
        let zoom = tab.zoom;
        let off = tab.offset;
        let to_s = |x: f32, y: f32| rect.min + off + Vec2::new(x * zoom, y * zoom);
        let to_d = |p: Pos2| {
            let v = p - rect.min - off;
            (v.x / zoom, v.y / zoom)
        };
        let ft: &mut FreeTransform = tab.transform.as_mut().unwrap();
        let q = ft.quad();
        let qs: Vec<Pos2> = q.iter().map(|p| to_s(p.0, p.1)).collect();
        let handles: Vec<Pos2> = if ft.corners.is_some() {
            qs.clone()
        } else {
            vec![
                qs[0],
                qs[0].lerp(qs[1], 0.5),
                qs[1],
                qs[1].lerp(qs[2], 0.5),
                qs[2],
                qs[2].lerp(qs[3], 0.5),
                qs[3],
                qs[3].lerp(qs[0], 0.5),
            ]
        };
        // draw
        let mut outline = qs.clone();
        outline.push(qs[0]);
        painter.add(Shape::line(
            outline,
            EStroke::new(1.0, Color32::from_rgb(0, 150, 255)),
        ));
        for hp in &handles {
            painter.rect_filled(
                ERect::from_center_size(*hp, Vec2::splat(HANDLE)),
                0.0,
                Color32::WHITE,
            );
            painter.rect_stroke(
                ERect::from_center_size(*hp, Vec2::splat(HANDLE)),
                0.0,
                EStroke::new(1.0, Color32::from_rgb(0, 120, 220)),
                egui::StrokeKind::Middle,
            );
        }
        let piv = ft.pivot();
        let pivot_s = to_s(piv.0 + ft.tx, piv.1 + ft.ty);
        if ft.corners.is_none() {
            painter.circle_stroke(pivot_s, 4.0, EStroke::new(1.0, Color32::WHITE));
        }
        let inside_quad = |p: Pos2| {
            let mut inside = false;
            let n = qs.len();
            for i in 0..n {
                let (a, b) = (qs[i], qs[(i + 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                {
                    inside = !inside;
                }
            }
            inside
        };
        if let Some(hp) = resp.hover_pos() {
            let on_handle = handles.iter().any(|h| h.distance(hp) < HANDLE + 3.0);
            resp.ctx.set_cursor_icon(if on_handle {
                CursorIcon::ResizeNwSe
            } else if inside_quad(hp) {
                CursorIcon::Move
            } else {
                CursorIcon::Alias
            });
        }
        if resp.drag_started_by(PointerButton::Primary) {
            if let Some(p) = resp
                .ctx
                .input(|i| i.pointer.press_origin())
                .or(resp.interact_pointer_pos())
            {
                let hit = handles.iter().position(|h| h.distance(p) < HANDLE + 3.0);
                ft.drag = Some(match (hit, ft.corners.is_some()) {
                    (Some(i), true) => TransformDrag::Corner { idx: i },
                    (Some(i), false) => TransformDrag::Scale {
                        handle: i,
                        sx0: ft.sx,
                        sy0: ft.sy,
                        start: to_d(p),
                    },
                    (None, _) if inside_quad(p) => TransformDrag::Move {
                        start: to_d(p),
                        tx0: ft.tx,
                        ty0: ft.ty,
                    },
                    (None, _) => {
                        let v = p - pivot_s;
                        TransformDrag::Rotate {
                            start_angle: v.y.atan2(v.x),
                            rot0: ft.rot,
                        }
                    }
                });
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            if let (Some(d), Some(p)) = (ft.drag, resp.interact_pointer_pos()) {
                let dp = to_d(p);
                match d {
                    TransformDrag::Move { start, tx0, ty0 } => {
                        let (dx, dy) = (dp.0 - start.0, dp.1 - start.1);
                        if let Some(c) = &mut ft.corners {
                            let prev = (ft.tx, ft.ty);
                            for k in c.iter_mut() {
                                k.0 += dx - (prev.0 - tx0);
                                k.1 += dy - (prev.1 - ty0);
                            }
                        }
                        ft.tx = tx0 + dx;
                        ft.ty = ty0 + dy;
                    }
                    TransformDrag::Corner { idx } => {
                        if let Some(c) = &mut ft.corners {
                            c[idx] = dp;
                        }
                    }
                    TransformDrag::Scale {
                        handle,
                        sx0,
                        sy0,
                        start,
                    } => {
                        let piv = (ft.pivot().0 + ft.tx, ft.pivot().1 + ft.ty);
                        let (s, c) = (-ft.rot).to_radians().sin_cos();
                        let local = |p: (f32, f32)| {
                            let (vx, vy) = (p.0 - piv.0, p.1 - piv.1);
                            (vx * c - vy * s, vx * s + vy * c)
                        };
                        let (a, b) = (local(start), local(dp));
                        let fx = if a.0.abs() > 1e-3 { b.0 / a.0 } else { 1.0 };
                        let fy = if a.1.abs() > 1e-3 { b.1 / a.1 } else { 1.0 };
                        let corner = handle % 2 == 0;
                        let horizontal_edge = handle == 1 || handle == 5;
                        let vertical_edge = handle == 3 || handle == 7;
                        if corner {
                            if mods.shift {
                                ft.sx = sx0 * fx;
                                ft.sy = sy0 * fy;
                            } else {
                                let f = (fx.abs() + fy.abs()) / 2.0;
                                ft.sx = sx0 * f * fx.signum();
                                ft.sy = sy0 * f * fy.signum();
                            }
                        } else if horizontal_edge {
                            ft.sy = sy0 * fy;
                        } else if vertical_edge {
                            ft.sx = sx0 * fx;
                        }
                    }
                    TransformDrag::Rotate { start_angle, rot0 } => {
                        let v = p - pivot_s;
                        let mut r = rot0 + (v.y.atan2(v.x) - start_angle).to_degrees();
                        if mods.shift {
                            r = (r / 15.0).round() * 15.0;
                        }
                        ft.rot = r;
                    }
                }
                changed = true;
            }
        }
        if resp.drag_stopped() {
            ft.drag = None;
        }
    }
    if changed {
        let ft = tab.transform.as_ref().unwrap();
        let img = ft.render(w, h);
        let lid = ft.layer;
        if let Some(l) = tab.doc.layer_mut(lid) {
            l.set_image(img);
        }
        tab.full_redraw = true;
    }
    // first frame: show live render
    if tab
        .transform
        .as_ref()
        .map(|t| t.drag.is_none())
        .unwrap_or(false)
        && resp.double_clicked()
    {
        app.pending.push(Cmd::Commit);
    }
}
