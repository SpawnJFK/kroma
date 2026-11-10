//! One open document: model, history, view transform, GPU tile cache and in-progress tool state.

use egui::{
    Color32, ColorImage, Context, Pos2, Rect as ERect, TextureHandle, TextureOptions, Vec2,
};
use kroma_core::brush::Stroke;
use kroma_core::buffer::{Image, Mask, Rect};
use kroma_core::history::History;
use kroma_core::layer::{LayerId, LayerKind};
use kroma_core::transform::{Affine, Liquify};
use kroma_core::Document;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub const TILE: i32 = 1024;

pub struct Tile {
    pub rect: Rect,
    pub tex: TextureHandle,
}

/// Something the user is in the middle of doing on the canvas.
pub enum Interaction {
    None,
    Painting {
        stroke: Stroke,
        on_mask: bool,
        layer: LayerId,
        name: &'static str,
    },
    Marquee {
        start: (f32, f32),
        end: (f32, f32),
        ellipse: bool,
    },
    Lasso {
        points: Vec<(f32, f32)>,
    },
    Moving {
        layer: LayerId,
        original: Image,
        original_mask: Option<Mask>,
        start: (f32, f32),
        text_origin: Option<(f32, f32)>,
        sel_original: Option<Mask>,
    },
    Gradient {
        start: (f32, f32),
        end: (f32, f32),
    },
    Shape {
        start: (f32, f32),
        end: (f32, f32),
    },
    Crop {
        start: (f32, f32),
        end: (f32, f32),
    },
    SpotHeal {
        mask: Mask,
        last: Option<(f32, f32)>,
        layer: LayerId,
        points: Vec<(f32, f32)>,
    },
    Liquify {
        last: (f32, f32),
    },
    Pan {
        last: Pos2,
    },
}

/// Ctrl+T free transform session.
pub struct FreeTransform {
    pub layer: LayerId,
    pub original: Image,
    pub bounds: Rect,
    pub tx: f32,
    pub ty: f32,
    pub sx: f32,
    pub sy: f32,
    pub rot: f32,
    pub drag: Option<TransformDrag>,
    /// Distort mode: four free corners (TL, TR, BR, BL) in document space.
    pub corners: Option<[(f32, f32); 4]>,
}

#[derive(Clone, Copy)]
pub enum TransformDrag {
    Move {
        start: (f32, f32),
        tx0: f32,
        ty0: f32,
    },
    Scale {
        handle: usize,
        sx0: f32,
        sy0: f32,
        start: (f32, f32),
    },
    Rotate {
        start_angle: f32,
        rot0: f32,
    },
    Corner {
        idx: usize,
    },
}

impl FreeTransform {
    pub fn pivot(&self) -> (f32, f32) {
        (
            self.bounds.x as f32 + self.bounds.w as f32 / 2.0,
            self.bounds.y as f32 + self.bounds.h as f32 / 2.0,
        )
    }
    pub fn affine(&self) -> Affine {
        Affine::free(self.pivot(), self.sx, self.sy, self.rot, self.tx, self.ty)
    }
    /// Current corner positions (TL, TR, BR, BL) in document space.
    pub fn quad(&self) -> [(f32, f32); 4] {
        if let Some(c) = self.corners {
            return c;
        }
        let b = self.bounds;
        let t = self.affine();
        let (x0, y0, x1, y1) = (b.x as f32, b.y as f32, b.right() as f32, b.bottom() as f32);
        [
            t.apply(x0, y0),
            t.apply(x1, y0),
            t.apply(x1, y1),
            t.apply(x0, y1),
        ]
    }
    pub fn render(&self, w: u32, h: u32) -> Image {
        match self.corners {
            Some(c) => {
                // warp the layer's bounding box content onto the quad
                let src = self.original.sub_image(self.bounds);
                kroma_core::transform::warp_perspective(&src, c, w, h)
            }
            None => kroma_core::transform::transform_affine(&self.original, &self.affine(), w, h),
        }
    }
}

pub struct LiquifySession {
    pub layer: LayerId,
    pub original: Image,
    pub field: Liquify,
}

/// Live preview of a dialog operation on one layer.
pub struct Preview {
    pub layer: LayerId,
    pub original: Image,
}

pub struct DocTab {
    pub id: u64,
    pub doc: Document,
    pub history: History,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub zoom: f32,
    /// Screen offset of document pixel (0,0) relative to the canvas rect's min corner.
    pub offset: Vec2,
    pub needs_fit: bool,
    pub tiles: Vec<Tile>,
    pub full_redraw: bool,
    pub dirty_rect: Rect,
    pub interaction: Interaction,
    pub transform: Option<FreeTransform>,
    pub liquify: Option<LiquifySession>,
    pub preview: Option<Preview>,
    pub editing_mask: bool,
    pub thumbs: HashMap<LayerId, (u64, TextureHandle)>,
    pub sel_cache: Option<(Mask, Vec<(Pos2, Pos2)>)>,
    pub clone_source: Option<(f32, f32)>,
    pub clone_offset: Option<(f32, f32)>,
    pub crop_pending: Option<Rect>,
    pub poly_points: Vec<(f32, f32)>,
    /// Layers below the active layer, cached for fast brush redraws.
    pub paint_under: Option<Arc<Image>>,
    /// Layer thumbnail regens this frame (max 4).
    pub thumbs_regen_this_frame: u8,
}

static NEXT_TAB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DocTab {
    pub fn new(doc: Document, path: Option<PathBuf>) -> DocTab {
        let history = History::new(&doc, if path.is_some() { "Open" } else { "New" });
        DocTab {
            id: NEXT_TAB.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            doc,
            history,
            path,
            dirty: false,
            zoom: 1.0,
            offset: Vec2::ZERO,
            needs_fit: true,
            tiles: Vec::new(),
            full_redraw: true,
            dirty_rect: Rect::default(),
            interaction: Interaction::None,
            transform: None,
            liquify: None,
            preview: None,
            editing_mask: false,
            thumbs: HashMap::new(),
            sel_cache: None,
            clone_source: None,
            clone_offset: None,
            crop_pending: None,
            poly_points: Vec::new(),
            paint_under: None,
            thumbs_regen_this_frame: 0,
        }
    }

    pub fn title(&self) -> String {
        format!("{}{}", self.doc.name, if self.dirty { " •" } else { "" })
    }

    /// Record a finished operation in history.
    pub fn commit(&mut self, name: &str) {
        self.history.push(name, &self.doc);
        self.dirty = true;
        self.full_redraw = true;
    }

    /// Like commit but only the given region changed (cheap redraw).
    pub fn commit_region(&mut self, name: &str, r: Rect) {
        self.history.push(name, &self.doc);
        self.dirty = true;
        self.mark_dirty(r);
    }

    pub fn mark_dirty(&mut self, r: Rect) {
        self.dirty_rect = self.dirty_rect.union(&r);
    }

    pub fn undo(&mut self) {
        self.cancel_sessions();
        if let Some(d) = self.history.undo() {
            self.doc = d;
            self.full_redraw = true;
            self.dirty = true;
        }
    }
    pub fn redo(&mut self) {
        self.cancel_sessions();
        if let Some(d) = self.history.redo() {
            self.doc = d;
            self.full_redraw = true;
            self.dirty = true;
        }
    }
    pub fn goto_history(&mut self, i: usize) {
        self.cancel_sessions();
        if let Some(d) = self.history.goto(i) {
            self.doc = d;
            self.full_redraw = true;
            self.dirty = true;
        }
    }

    /// Abort free transform / liquify / preview without applying.
    pub fn cancel_sessions(&mut self) {
        if let Some(t) = self.transform.take() {
            if let Some(l) = self.doc.layer_mut(t.layer) {
                l.set_image(t.original);
            }
            self.full_redraw = true;
        }
        if let Some(s) = self.liquify.take() {
            if let Some(l) = self.doc.layer_mut(s.layer) {
                l.set_image(s.original);
            }
            self.full_redraw = true;
        }
        if let Some(p) = self.preview.take() {
            if let Some(l) = self.doc.layer_mut(p.layer) {
                l.set_image(p.original);
            }
            self.full_redraw = true;
        }
        self.interaction = Interaction::None;
        self.paint_under = None;
        self.crop_pending = None;
        self.poly_points.clear();
    }

    pub fn active_is_pixel(&self) -> bool {
        self.doc
            .active_layer()
            .map(|l| matches!(l.kind, LayerKind::Raster))
            .unwrap_or(false)
    }

    // ------------------------------------------------------------- view

    pub fn fit(&mut self, canvas: ERect) {
        let (w, h) = (self.doc.width as f32, self.doc.height as f32);
        let z = ((canvas.width() - 40.0) / w)
            .min((canvas.height() - 40.0) / h)
            .clamp(0.01, 32.0);
        self.zoom = z;
        self.offset = Vec2::new(
            (canvas.width() - w * z) / 2.0,
            (canvas.height() - h * z) / 2.0,
        );
        self.needs_fit = false;
    }
    pub fn zoom_about(&mut self, canvas: ERect, screen: Pos2, new_zoom: f32) {
        let new_zoom = new_zoom.clamp(0.01, 64.0);
        let doc_pt = self.screen_to_doc(canvas, screen);
        self.zoom = new_zoom;
        self.offset = (screen - canvas.min) - Vec2::new(doc_pt.0 * new_zoom, doc_pt.1 * new_zoom);
    }
    pub fn screen_to_doc(&self, canvas: ERect, p: Pos2) -> (f32, f32) {
        let v = p - canvas.min - self.offset;
        (v.x / self.zoom, v.y / self.zoom)
    }
    pub fn doc_to_screen(&self, canvas: ERect, x: f32, y: f32) -> Pos2 {
        canvas.min + self.offset + Vec2::new(x * self.zoom, y * self.zoom)
    }
    pub fn doc_rect_on_screen(&self, canvas: ERect) -> ERect {
        ERect::from_min_max(
            self.doc_to_screen(canvas, 0.0, 0.0),
            self.doc_to_screen(canvas, self.doc.width as f32, self.doc.height as f32),
        )
    }

    // ------------------------------------------------------------- GPU upload

    pub fn sync_textures(&mut self, ctx: &Context) {
        let (w, h) = (self.doc.width as i32, self.doc.height as i32);
        let layout_ok = !self.tiles.is_empty()
            && self.tiles.iter().map(|t| t.rect.right()).max() == Some(w)
            && self.tiles.iter().map(|t| t.rect.bottom()).max() == Some(h);
        if !layout_ok {
            self.tiles.clear();
            self.full_redraw = true;
        }
        let opts = TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Linear,
            ..Default::default()
        };
        if self.full_redraw {
            let comp = self.doc.composite();
            if self.tiles.is_empty() {
                let mut y = 0;
                while y < h {
                    let mut x = 0;
                    while x < w {
                        let r = Rect::new(x, y, TILE.min(w - x), TILE.min(h - y));
                        let img = to_color_image(&comp.sub_image(r));
                        let tex =
                            ctx.load_texture(format!("tile_{}_{}_{}", self.id, x, y), img, opts);
                        self.tiles.push(Tile { rect: r, tex });
                        x += TILE;
                    }
                    y += TILE;
                }
            } else {
                for t in self.tiles.iter_mut() {
                    t.tex.set(to_color_image(&comp.sub_image(t.rect)), opts);
                }
            }
            self.full_redraw = false;
            self.dirty_rect = Rect::default();
        } else if !self.dirty_rect.is_empty() {
            let r = self.dirty_rect.intersect(&self.doc.bounds());
            self.dirty_rect = Rect::default();
            if r.is_empty() {
                return;
            }
            let comp = if let (
                Some(under),
                Interaction::Painting {
                    on_mask: false,
                    layer,
                    ..
                },
            ) = (&self.paint_under, &self.interaction)
            {
                self.doc.composite_layer_over(under, *layer, r)
            } else {
                self.doc.composite_rect(r)
            };
            for t in self.tiles.iter_mut() {
                let ir = t.rect.intersect(&r);
                if ir.is_empty() {
                    continue;
                }
                let part = comp.sub_image(Rect::new(ir.x - r.x, ir.y - r.y, ir.w, ir.h));
                t.tex.set_partial(
                    [(ir.x - t.rect.x) as usize, (ir.y - t.rect.y) as usize],
                    to_color_image(&part),
                    opts,
                );
            }
        }
    }

    /// Layer thumbnail texture (refreshed when the layer revision changes).
    pub fn thumb(&mut self, ctx: &Context, id: LayerId, size: u32) -> Option<egui::TextureId> {
        let l = self.doc.layer(id)?;
        let rev = l.rev();
        if let Some((r, t)) = self.thumbs.get(&id) {
            if *r == rev {
                return Some(t.id());
            }
        }
        if self.thumbs_regen_this_frame >= 4 {
            if let Some((_, t)) = self.thumbs.get(&id) {
                return Some(t.id());
            }
        }
        self.thumbs_regen_this_frame = self.thumbs_regen_this_frame.saturating_add(1);
        let src: Image = match &l.kind {
            LayerKind::Adjustment(_) | LayerKind::Group { .. } => {
                let m = l.mask()?;
                m.to_image()
            }
            _ => l.image().clone(),
        };
        if src.is_empty() {
            return None;
        }
        let s = size as f32 / src.width.max(src.height) as f32;
        let (tw, th) = (
            ((src.width as f32 * s) as u32).max(1),
            ((src.height as f32 * s) as u32).max(1),
        );
        let mut px = Vec::with_capacity((tw * th * 4) as usize);
        for y in 0..th {
            for x in 0..tw {
                let p = src.get(
                    ((x as f32 + 0.5) / s) as u32 % src.width,
                    ((y as f32 + 0.5) / s) as u32 % src.height,
                );
                // checker under transparency
                let chk = if ((x / 4) + (y / 4)) % 2 == 0 {
                    0.9
                } else {
                    0.7
                };
                for c in 0..3 {
                    px.push(((p[c] * p[3] + chk * (1.0 - p[3])) * 255.0) as u8);
                }
                px.push(255);
            }
        }
        let img = ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], &px);
        match self.thumbs.get_mut(&id) {
            Some((r, t)) => {
                t.set(img, TextureOptions::LINEAR);
                *r = rev;
                Some(t.id())
            }
            None => {
                let t = ctx.load_texture(
                    format!("thumb_{}_{}", self.id, id),
                    img,
                    TextureOptions::LINEAR,
                );
                let tid = t.id();
                self.thumbs.insert(id, (rev, t));
                Some(tid)
            }
        }
    }

    /// Marching-ants segments in document space, cached per selection buffer.
    pub fn selection_segments(&mut self) -> Option<&Vec<(Pos2, Pos2)>> {
        let sel = self.doc.selection.as_ref()?;
        let stale = match &self.sel_cache {
            Some((m, _)) => !m.same_storage(sel),
            None => true,
        };
        if stale {
            let step = if (sel.width as u64 * sel.height as u64) > 6_000_000 {
                2
            } else {
                1
            };
            let segs = kroma_core::selection::outline_segments(sel, step);
            self.sel_cache = Some((sel.clone(), merge_segments(segs)));
        }
        self.sel_cache.as_ref().map(|(_, s)| s)
    }
}

/// Merge unit edges into long horizontal/vertical runs (far fewer shapes to draw).
fn merge_segments(segs: Vec<((f32, f32), (f32, f32))>) -> Vec<(Pos2, Pos2)> {
    let mut horiz: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    let mut vert: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
    for (a, b) in segs {
        if (a.1 - b.1).abs() < 1e-3 {
            horiz
                .entry(a.1 as i32)
                .or_default()
                .push((a.0 as i32, b.0 as i32));
        } else {
            vert.entry(a.0 as i32)
                .or_default()
                .push((a.1 as i32, b.1 as i32));
        }
    }
    let mut out = Vec::new();
    let mut runs = |map: HashMap<i32, Vec<(i32, i32)>>, horizontal: bool| {
        for (k, mut v) in map {
            v.sort();
            let mut cur = v[0];
            for s in v.into_iter().skip(1) {
                if s.0 <= cur.1 {
                    cur.1 = cur.1.max(s.1);
                } else {
                    out.push(if horizontal {
                        (
                            Pos2::new(cur.0 as f32, k as f32),
                            Pos2::new(cur.1 as f32, k as f32),
                        )
                    } else {
                        (
                            Pos2::new(k as f32, cur.0 as f32),
                            Pos2::new(k as f32, cur.1 as f32),
                        )
                    });
                    cur = s;
                }
            }
            out.push(if horizontal {
                (
                    Pos2::new(cur.0 as f32, k as f32),
                    Pos2::new(cur.1 as f32, k as f32),
                )
            } else {
                (
                    Pos2::new(k as f32, cur.0 as f32),
                    Pos2::new(k as f32, cur.1 as f32),
                )
            });
        }
    };
    runs(horiz, true);
    runs(vert, false);
    out
}

pub fn to_color_image(img: &Image) -> ColorImage {
    ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.to_rgba8())
}

pub fn checker_texture(ctx: &Context) -> TextureHandle {
    let n = 16;
    let mut px = Vec::with_capacity(n * n * 4);
    for y in 0..n {
        for x in 0..n {
            let v = if ((x / 8) + (y / 8)) % 2 == 0 {
                204
            } else {
                153
            };
            px.extend_from_slice(&[v, v, v, 255]);
        }
    }
    ctx.load_texture(
        "checker",
        ColorImage::from_rgba_unmultiplied([n, n], &px),
        TextureOptions {
            magnification: egui::TextureFilter::Nearest,
            minification: egui::TextureFilter::Nearest,
            wrap_mode: egui::TextureWrapMode::Repeat,
            ..Default::default()
        },
    )
}

pub const CANVAS_BG: Color32 = Color32::from_rgb(38, 38, 40);
