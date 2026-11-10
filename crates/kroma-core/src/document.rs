//! Document: layer stack, selection, guides, compositing, and whole-image operations.

use crate::adjust::Adjustment;
use crate::blend::{composite_px, BlendMode};
use crate::buffer::{Image, Mask, Rect};
use crate::color::{Color, Px, TRANSPARENT};
use crate::layer::{Layer, LayerId, LayerKind};
use crate::transform::{self, Anchor, Resample};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub vertical: bool,
    pub pos: f32,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
    /// Bottom-to-top.
    pub layers: Vec<Layer>,
    pub active: LayerId,
    pub selection: Option<Mask>,
    pub guides: Vec<Guide>,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundContents {
    White,
    Black,
    Transparent,
    Color(u32),
}

impl Document {
    pub fn new(width: u32, height: u32, bg: Option<Color>) -> Document {
        let mut layer = Layer::raster(
            if bg.is_some() {
                "Background"
            } else {
                "Layer 1"
            },
            width,
            height,
        );
        if let Some(c) = bg {
            layer.image_mut().fill(c);
        }
        let id = layer.id;
        Document {
            width,
            height,
            dpi: 72.0,
            layers: vec![layer],
            active: id,
            selection: None,
            guides: vec![],
            name: "Untitled".into(),
        }
    }

    pub fn from_image(name: &str, img: Image) -> Document {
        let (w, h) = (img.width, img.height);
        let layer = Layer::from_image("Background", img);
        let id = layer.id;
        Document {
            width: w,
            height: h,
            dpi: 72.0,
            layers: vec![layer],
            active: id,
            selection: None,
            guides: vec![],
            name: name.into(),
        }
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }

    // ------------------------------------------------------------ layer lookup

    /// Path of indices to a layer (top-level index, then child indices).
    pub fn path_of(&self, id: LayerId) -> Option<Vec<usize>> {
        fn find(list: &[Layer], id: LayerId, path: &mut Vec<usize>) -> bool {
            for (i, l) in list.iter().enumerate() {
                path.push(i);
                if l.id == id {
                    return true;
                }
                if let Some(ch) = l.children() {
                    if find(ch, id, path) {
                        return true;
                    }
                }
                path.pop();
            }
            false
        }
        let mut p = Vec::new();
        if find(&self.layers, id, &mut p) {
            Some(p)
        } else {
            None
        }
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        let path = self.path_of(id)?;
        let mut list = &self.layers;
        let mut cur = None;
        for i in path {
            let l = &list[i];
            cur = Some(l);
            if let Some(ch) = l.children() {
                list = ch;
            }
        }
        cur
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        let path = self.path_of(id)?;
        let (last, parents) = path.split_last()?;
        let mut list = &mut self.layers;
        for i in parents {
            list = list[*i].children_mut()?;
        }
        list.get_mut(*last)
    }

    /// Sibling list containing `id`, and its index.
    pub fn parent_list_mut(&mut self, id: LayerId) -> Option<(&mut Vec<Layer>, usize)> {
        let path = self.path_of(id)?;
        let (last, parents) = path.split_last()?;
        let mut list = &mut self.layers;
        for i in parents {
            list = list[*i].children_mut()?;
        }
        Some((list, *last))
    }

    pub fn active_layer(&self) -> Option<&Layer> {
        self.layer(self.active)
    }
    pub fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        let id = self.active;
        self.layer_mut(id)
    }

    /// All layers flattened top-to-bottom with depth (for the Layers panel).
    pub fn flat_list(&self) -> Vec<(usize, &Layer)> {
        fn walk<'a>(list: &'a [Layer], depth: usize, out: &mut Vec<(usize, &'a Layer)>) {
            for l in list.iter().rev() {
                out.push((depth, l));
                if let LayerKind::Group {
                    children,
                    expanded: true,
                    ..
                } = &l.kind
                {
                    walk(children, depth + 1, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.layers, 0, &mut out);
        out
    }

    pub fn layer_count(&self) -> usize {
        fn count(list: &[Layer]) -> usize {
            list.iter()
                .map(|l| 1 + l.children().map(|c| count(c)).unwrap_or(0))
                .sum()
        }
        count(&self.layers)
    }

    // ------------------------------------------------------------ layer ops

    /// Insert above the active layer (inside the same group) and make it active.
    pub fn add_layer(&mut self, layer: Layer) -> LayerId {
        let id = layer.id;
        let active = self.active;
        match self.parent_list_mut(active) {
            Some((list, i)) => {
                if list[i].is_group() {
                    // inserting with a group selected puts the layer inside the group, on top
                    list.insert(i + 1, layer);
                } else {
                    list.insert(i + 1, layer);
                }
            }
            None => self.layers.push(layer),
        }
        self.active = id;
        id
    }

    pub fn new_raster_layer(&mut self) -> LayerId {
        let n = self.layer_count() + 1;
        let l = Layer::raster(&format!("Layer {n}"), self.width, self.height);
        self.add_layer(l)
    }

    pub fn delete_layer(&mut self, id: LayerId) -> bool {
        if self.layer_count() <= 1 {
            return false;
        }
        let Some((list, i)) = self.parent_list_mut(id) else {
            return false;
        };
        list.remove(i);
        let next = if list.is_empty() {
            None
        } else {
            Some(list[i.saturating_sub(1).min(list.len() - 1)].id)
        };
        self.active = next.unwrap_or_else(|| self.layers.last().map(|l| l.id).unwrap_or(0));
        if self.layer(self.active).is_none() {
            self.active = self.layers.last().map(|l| l.id).unwrap_or(0);
        }
        true
    }

    pub fn duplicate_layer(&mut self, id: LayerId) -> Option<LayerId> {
        let dup = self.layer(id)?.duplicate();
        let nid = dup.id;
        let (list, i) = self.parent_list_mut(id)?;
        list.insert(i + 1, dup);
        self.active = nid;
        Some(nid)
    }

    /// Move a layer up (+1) or down (-1) within its siblings.
    pub fn move_layer(&mut self, id: LayerId, delta: i32) {
        if let Some((list, i)) = self.parent_list_mut(id) {
            let j = (i as i32 + delta).clamp(0, list.len() as i32 - 1) as usize;
            if i != j {
                let l = list.remove(i);
                list.insert(j, l);
            }
        }
    }

    pub fn move_layer_to_top(&mut self, id: LayerId) {
        if let Some((list, i)) = self.parent_list_mut(id) {
            let l = list.remove(i);
            list.push(l);
        }
    }
    pub fn move_layer_to_bottom(&mut self, id: LayerId) {
        if let Some((list, i)) = self.parent_list_mut(id) {
            let l = list.remove(i);
            list.insert(0, l);
        }
    }

    /// Layer > Merge Down (Ctrl+E).
    pub fn merge_down(&mut self, id: LayerId) -> bool {
        let (w, h) = (self.width, self.height);
        let Some((list, i)) = self.parent_list_mut(id) else {
            return false;
        };
        if i == 0 {
            return false;
        }
        let pair = vec![list[i - 1].clone(), list[i].clone()];
        let mut below = Layer::from_image(&list[i - 1].name.clone(), Image::new(w, h));
        let mut acc = Image::new(w, h);
        composite_stack(
            &pair,
            &mut acc,
            Rect::new(0, 0, w as i32, h as i32),
            w,
            CompositeFilter::default(),
        );
        below.set_image(acc);
        let bid = below.id;
        list.remove(i);
        list[i - 1] = below;
        self.active = bid;
        true
    }

    /// Layer > Merge Visible.
    pub fn merge_visible(&mut self) {
        let img = self.composite();
        let hidden: Vec<Layer> = self.layers.iter().filter(|l| !l.visible).cloned().collect();
        let merged = Layer::from_image("Merged", img);
        let id = merged.id;
        self.layers = hidden;
        self.layers.push(merged);
        self.active = id;
    }

    /// Layer > Flatten Image (onto white, like Photoshop).
    pub fn flatten(&mut self) {
        let img = self.composite();
        let mut bg = Image::filled(self.width, self.height, [1.0, 1.0, 1.0, 1.0]);
        let w = self.width as usize;
        let ip = img.pixels();
        bg.pixels_mut()
            .par_iter_mut()
            .enumerate()
            .for_each(|(i, p)| {
                *p = composite_px(
                    *p,
                    ip[i],
                    BlendMode::Normal,
                    1.0,
                    (i % w) as u32,
                    (i / w) as u32,
                )
            });
        let l = Layer::from_image("Background", bg);
        self.active = l.id;
        self.layers = vec![l];
    }

    /// Layer > Group Layers (Ctrl+G) for the active layer.
    pub fn group_layer(&mut self, id: LayerId) -> Option<LayerId> {
        let (list, i) = self.parent_list_mut(id)?;
        let l = list.remove(i);
        let g = Layer::group("Group", vec![l]);
        let gid = g.id;
        list.insert(i, g);
        Some(gid)
    }

    /// Layer > Ungroup (Ctrl+Shift+G).
    pub fn ungroup(&mut self, id: LayerId) -> bool {
        let Some((list, i)) = self.parent_list_mut(id) else {
            return false;
        };
        if !list[i].is_group() {
            return false;
        }
        let g = list.remove(i);
        let children = match g.kind {
            LayerKind::Group { children, .. } => children,
            _ => unreachable!(),
        };
        let top = children.last().map(|l| l.id);
        for (k, c) in children.into_iter().enumerate() {
            list.insert(i + k, c);
        }
        if let Some(t) = top {
            self.active = t;
        }
        true
    }

    // ------------------------------------------------------------ compositing

    pub fn composite(&self) -> Image {
        self.composite_rect(self.bounds())
    }

    /// Composite only `r` (used for fast partial canvas refresh while painting).
    pub fn composite_rect(&self, r: Rect) -> Image {
        self.composite_rect_filter(r, CompositeFilter::default())
    }

    /// Full composite with one layer omitted (paint stroke underlay cache).
    pub fn composite_omit_layer(&self, omit: LayerId) -> Image {
        self.composite_rect_filter(
            self.bounds(),
            CompositeFilter {
                skip: Some(omit),
                only: None,
            },
        )
    }

    /// Composite `layer` over the matching region of `under` (same doc size).
    pub fn composite_layer_over(&self, under: &Image, layer: LayerId, r: Rect) -> Image {
        let r = r.intersect(&self.bounds());
        let mut acc = under.sub_image(r);
        if !r.is_empty() {
            composite_stack(
                &self.layers,
                &mut acc,
                r,
                self.width,
                CompositeFilter {
                    skip: None,
                    only: Some(layer),
                },
            );
        }
        acc
    }

    pub fn composite_rect_filter(&self, r: Rect, filter: CompositeFilter) -> Image {
        let r = r.intersect(&self.bounds());
        let mut acc = Image::new(r.w.max(0) as u32, r.h.max(0) as u32);
        if !r.is_empty() {
            composite_stack(&self.layers, &mut acc, r, self.width, filter);
        }
        acc
    }

    /// Merged pixel at (x,y) (eyedropper "All Layers").
    pub fn sample_merged(&self, x: u32, y: u32) -> Px {
        self.composite_rect(Rect::new(x as i32, y as i32, 1, 1))
            .get(0, 0)
    }

    // ------------------------------------------------------------ whole-image ops

    fn map_all(
        &mut self,
        nw: u32,
        nh: u32,
        f: &dyn Fn(&Image) -> Image,
        fm: &dyn Fn(&Mask) -> Mask,
    ) {
        for l in self.layers.iter_mut() {
            l.map_pixels(f, fm, nw, nh);
        }
        self.selection = self.selection.as_ref().map(fm);
        self.width = nw;
        self.height = nh;
    }

    fn map_text(&mut self, f: &dyn Fn(&mut crate::text::TextStyle)) {
        fn walk(list: &mut [Layer], f: &dyn Fn(&mut crate::text::TextStyle), w: u32, h: u32) {
            for l in list.iter_mut() {
                if let LayerKind::Text(s) = &mut l.kind {
                    f(s);
                    l.rerender(w, h);
                }
                if let Some(ch) = l.children_mut() {
                    walk(ch, f, w, h);
                }
            }
        }
        let (w, h) = (self.width, self.height);
        walk(&mut self.layers, f, w, h);
    }

    fn rasterize_all_text(&mut self) {
        fn walk(list: &mut [Layer]) {
            for l in list.iter_mut() {
                l.rasterize();
                if let Some(ch) = l.children_mut() {
                    walk(ch);
                }
            }
        }
        walk(&mut self.layers);
    }

    /// Image > Image Size.
    pub fn resize_image(&mut self, nw: u32, nh: u32, method: Resample) {
        let (sx, sy) = (
            nw as f32 / self.width as f32,
            nh as f32 / self.height as f32,
        );
        self.map_all(
            nw,
            nh,
            &|img| transform::resize(img, nw, nh, method),
            &|m| mask_resize(m, nw, nh),
        );
        self.map_text(&|s| {
            s.x *= sx;
            s.y *= sy;
            s.size *= (sx * sy).sqrt();
        });
        for g in self.guides.iter_mut() {
            g.pos *= if g.vertical { sx } else { sy };
        }
    }

    /// Image > Canvas Size.
    pub fn canvas_size(&mut self, nw: u32, nh: u32, anchor: Anchor) {
        let (dx, dy) = anchor.offset(self.width as i32, self.height as i32, nw as i32, nh as i32);
        self.map_all(
            nw,
            nh,
            &|img| transform::canvas_size(img, nw, nh, anchor, TRANSPARENT),
            &|m| {
                let mut out = Mask::new(nw, nh, 0.0);
                let (ow, oh) = (m.width as i32, m.height as i32);
                for y in 0..oh {
                    for x in 0..ow {
                        let (tx, ty) = (x + dx, y + dy);
                        if tx >= 0 && ty >= 0 && tx < nw as i32 && ty < nh as i32 {
                            out.set(tx as u32, ty as u32, m.get(x as u32, y as u32));
                        }
                    }
                }
                out
            },
        );
        self.map_text(&|s| {
            s.x += dx as f32;
            s.y += dy as f32;
        });
    }

    /// Image > Crop (to rect).
    pub fn crop(&mut self, r: Rect) {
        let r = Rect::new(r.x, r.y, r.w.max(1), r.h.max(1));
        let (nw, nh) = (r.w as u32, r.h as u32);
        self.map_all(nw, nh, &|img| transform::crop(img, r), &|m| {
            let mut out = Mask::new(nw, nh, 0.0);
            for y in 0..nh as i32 {
                for x in 0..nw as i32 {
                    out.set(x as u32, y as u32, m.get_i(x + r.x, y + r.y));
                }
            }
            out
        });
        self.map_text(&|s| {
            s.x -= r.x as f32;
            s.y -= r.y as f32;
        });
        self.selection = None;
    }

    /// Image > Trim (transparent pixels).
    pub fn trim(&mut self) {
        let b = self.composite().opaque_bounds(0.004);
        if !b.is_empty() {
            self.crop(b);
        }
    }

    pub fn rotate_canvas_cw(&mut self) {
        self.rasterize_all_text();
        let (nw, nh) = (self.height, self.width);
        self.map_all(nw, nh, &transform::rotate_cw, &|m| {
            mask_via_image(m, &transform::rotate_cw)
        });
    }
    pub fn rotate_canvas_ccw(&mut self) {
        self.rasterize_all_text();
        let (nw, nh) = (self.height, self.width);
        self.map_all(nw, nh, &transform::rotate_ccw, &|m| {
            mask_via_image(m, &transform::rotate_ccw)
        });
    }
    pub fn rotate_canvas_180(&mut self) {
        self.rasterize_all_text();
        let (nw, nh) = (self.width, self.height);
        self.map_all(nw, nh, &transform::rotate_180, &|m| {
            mask_via_image(m, &transform::rotate_180)
        });
    }
    pub fn rotate_canvas(&mut self, degrees: f32) {
        self.rasterize_all_text();
        let probe = transform::rotate(
            &Image::new(self.width, self.height),
            degrees,
            true,
            TRANSPARENT,
        );
        let (nw, nh) = (probe.width, probe.height);
        self.map_all(
            nw,
            nh,
            &|img| transform::rotate(img, degrees, true, TRANSPARENT),
            &|m| mask_via_image(m, &|i| transform::rotate(i, degrees, true, TRANSPARENT)),
        );
    }
    pub fn flip_canvas(&mut self, horizontal: bool) {
        self.rasterize_all_text();
        let (nw, nh) = (self.width, self.height);
        if horizontal {
            self.map_all(nw, nh, &transform::flip_h, &|m| {
                mask_via_image(m, &transform::flip_h)
            });
        } else {
            self.map_all(nw, nh, &transform::flip_v, &|m| {
                mask_via_image(m, &transform::flip_v)
            });
        }
    }

    /// Reveal-All / Hide-All / From-Selection mask on the active layer.
    pub fn add_mask(&mut self, from_selection: bool, hide: bool) {
        let (w, h) = (self.width, self.height);
        let sel = self.selection.clone();
        if let Some(l) = self.active_layer_mut() {
            let m = match (from_selection, sel) {
                (true, Some(s)) => {
                    if hide {
                        crate::selection::invert(&s)
                    } else {
                        s
                    }
                }
                _ => Mask::new(w, h, if hide { 0.0 } else { 1.0 }),
            };
            l.set_mask(Some(m));
        }
    }

    /// New adjustment layer above the active one, masked by the selection if any.
    pub fn add_adjustment_layer(&mut self, adj: Adjustment) -> LayerId {
        let mut l = Layer::adjustment(adj, self.width, self.height);
        if let Some(s) = &self.selection {
            l.set_mask(Some(s.clone()));
        }
        self.add_layer(l)
    }
}

fn mask_resize(m: &Mask, nw: u32, nh: u32) -> Mask {
    let img = transform::resize(&m.to_image(), nw, nh, Resample::Bilinear);
    Mask::from_vec(nw, nh, img.pixels().iter().map(|p| p[0]).collect())
}

fn mask_via_image(m: &Mask, f: &dyn Fn(&Image) -> Image) -> Mask {
    // encode mask in alpha so rotations with transparent fill work
    let img = Image::from_pixels(
        m.width,
        m.height,
        m.data().iter().map(|v| [1.0, 1.0, 1.0, *v]).collect(),
    );
    let out = f(&img);
    Mask::from_vec(
        out.width,
        out.height,
        out.pixels().iter().map(|p| p[3]).collect(),
    )
}

#[inline]
fn layer_px(l: &Layer, x: usize, y: usize, doc_w: usize) -> Px {
    let i = y * doc_w + x;
    let mut p = l.image().pixels()[i];
    if let Some(m) = l.active_mask() {
        p[3] *= m.data()[i];
    }
    p[3] *= l.fill;
    p
}

/// Optional layer filter for partial compositing (paint perf).
#[derive(Clone, Copy, Default)]
pub struct CompositeFilter {
    pub skip: Option<LayerId>,
    pub only: Option<LayerId>,
}

fn layer_in_tree(l: &Layer, id: LayerId) -> bool {
    if l.id == id {
        return true;
    }
    l.children()
        .map(|c| c.iter().any(|ch| layer_in_tree(ch, id)))
        .unwrap_or(false)
}

fn composite_visit(l: &Layer, filter: CompositeFilter) -> bool {
    if filter.skip == Some(l.id) {
        return false;
    }
    if let Some(only) = filter.only {
        return layer_in_tree(l, only);
    }
    true
}

/// Composite a bottom-to-top layer list into `acc` (which covers doc rect `r`).
pub fn composite_stack(
    layers: &[Layer],
    acc: &mut Image,
    r: Rect,
    doc_w: u32,
    filter: CompositeFilter,
) {
    let rw = r.w as usize;
    let dw = doc_w as usize;
    // alpha of the current clipping base, per pixel of r
    let mut base_alpha: Option<Vec<f32>> = None;
    let mut base_visible = true;
    for l in layers {
        if !composite_visit(l, filter) {
            continue;
        }
        if !l.clipped {
            base_visible = l.visible;
            base_alpha = None;
        }
        if !l.visible || (l.clipped && !base_visible) {
            if !l.clipped {
                base_alpha = Some(vec![0.0; rw * r.h as usize]);
            }
            continue;
        }
        let clip = if l.clipped {
            base_alpha.as_deref()
        } else {
            None
        };
        match &l.kind {
            LayerKind::Raster | LayerKind::Text(_) | LayerKind::Fill(_) => {
                if l.image().width as i32 != doc_w as i32 || l.image().is_empty() {
                    continue;
                }
                let styled = if l.has_active_effects() {
                    Some(l.styled_content())
                } else {
                    None
                };
                let opacity = l.opacity;
                let mode = l.blend;
                let mut alphas: Option<Vec<f32>> = if !l.clipped {
                    Some(vec![0.0; rw * r.h as usize])
                } else {
                    None
                };
                {
                    let alphas_ptr = alphas.as_mut().map(|v| v.as_mut_ptr() as usize);
                    acc.pixels_mut()
                        .par_chunks_mut(rw)
                        .enumerate()
                        .for_each(|(ry, row)| {
                            let y = r.y as usize + ry;
                            for (rx, p) in row.iter_mut().enumerate() {
                                let x = r.x as usize + rx;
                                let mut s = match &styled {
                                    Some(img) => img.pixels()[y * dw + x],
                                    None => layer_px(l, x, y, dw),
                                };
                                if let Some(ap) = alphas_ptr {
                                    // SAFETY: each (ry, rx) is written by exactly one thread
                                    unsafe { *(ap as *mut f32).add(ry * rw + rx) = s[3] };
                                }
                                if let Some(c) = clip {
                                    s[3] *= c[ry * rw + rx];
                                }
                                if s[3] > 0.0 {
                                    *p = composite_px(*p, s, mode, opacity, x as u32, y as u32);
                                }
                            }
                        });
                }
                if !l.clipped {
                    base_alpha = alphas;
                }
            }
            LayerKind::Adjustment(adj) => {
                let f = adj.compile();
                let opacity = l.opacity;
                let mode = l.blend;
                let mask = l.active_mask();
                acc.pixels_mut()
                    .par_chunks_mut(rw)
                    .enumerate()
                    .for_each(|(ry, row)| {
                        let y = r.y as usize + ry;
                        for (rx, p) in row.iter_mut().enumerate() {
                            if p[3] <= 0.0 {
                                continue;
                            }
                            let x = r.x as usize + rx;
                            let mut amt = opacity;
                            if let Some(m) = mask {
                                amt *= m.data()[y * dw + x];
                            }
                            if let Some(c) = clip {
                                amt *= c[ry * rw + rx];
                            }
                            if amt <= 0.0 {
                                continue;
                            }
                            let n = f(*p);
                            let src = [n[0], n[1], n[2], p[3]];
                            let a = p[3];
                            *p = composite_px(*p, src, mode, amt, x as u32, y as u32);
                            p[3] = a.max(p[3].min(a));
                        }
                    });
                if !l.clipped {
                    // adjustment layers act as a fully opaque clip base where their mask is white
                    base_alpha = Some(match mask {
                        Some(m) => (0..rw * r.h as usize)
                            .map(|i| m.data()[(r.y as usize + i / rw) * dw + r.x as usize + i % rw])
                            .collect(),
                        None => vec![1.0; rw * r.h as usize],
                    });
                }
            }
            LayerKind::Group {
                children,
                pass_through,
                ..
            } => {
                let direct = *pass_through
                    && l.opacity >= 1.0
                    && l.active_mask().is_none()
                    && clip.is_none()
                    && !l.has_active_effects();
                if direct {
                    composite_stack(children, acc, r, doc_w, filter);
                    if !l.clipped {
                        base_alpha = Some(vec![1.0; rw * r.h as usize]);
                    }
                } else {
                    let mut tmp = Image::new(r.w as u32, r.h as u32);
                    composite_stack(children, &mut tmp, r, doc_w, filter);
                    let mask = l.active_mask();
                    let tp = tmp.pixels();
                    let mode = if *pass_through {
                        BlendMode::Normal
                    } else {
                        l.blend
                    };
                    let opacity = l.opacity;
                    let mut alphas = vec![0.0f32; rw * r.h as usize];
                    for (i, a) in alphas.iter_mut().enumerate() {
                        let mut s = tp[i][3];
                        if let Some(m) = mask {
                            s *= m.data()[(r.y as usize + i / rw) * dw + r.x as usize + i % rw];
                        }
                        *a = s;
                    }
                    acc.pixels_mut()
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(i, p)| {
                            let mut s = tp[i];
                            s[3] = alphas[i];
                            if let Some(c) = clip {
                                s[3] *= c[i];
                            }
                            if s[3] > 0.0 {
                                let (x, y) = (r.x as usize + i % rw, r.y as usize + i / rw);
                                *p = composite_px(*p, s, mode, opacity, x as u32, y as u32);
                            }
                        });
                    if !l.clipped {
                        base_alpha = Some(alphas);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adjust::Adjustment;

    #[test]
    fn composite_basic_and_clipping() {
        let mut d = Document::new(16, 16, Some(Color::WHITE));
        let id = d.new_raster_layer();
        d.layer_mut(id)
            .unwrap()
            .image_mut()
            .fill(Color::rgb(1.0, 0.0, 0.0));
        d.layer_mut(id).unwrap().opacity = 0.5;
        let c = d.composite();
        let p = c.get(3, 3);
        assert!((p[0] - 1.0).abs() < 1e-4 && (p[1] - 0.5).abs() < 1e-4);

        // clipped layer only shows where base has alpha
        let base = d.new_raster_layer();
        {
            let l = d.layer_mut(base).unwrap();
            let img = l.image_mut();
            for y in 0..8 {
                for x in 0..16 {
                    img.set(x, y, [0.0, 0.0, 1.0, 1.0]);
                }
            }
        }
        let clipped = d.new_raster_layer();
        {
            let l = d.layer_mut(clipped).unwrap();
            l.image_mut().fill(Color::rgb(0.0, 1.0, 0.0));
            l.clipped = true;
        }
        let c = d.composite();
        assert!(c.get(3, 3)[1] > 0.9, "green inside base");
        assert!(c.get(3, 12)[1] < 0.6, "no green outside base");
    }

    #[test]
    fn adjustment_layer_and_partial_composite() {
        let mut d = Document::new(20, 20, Some(Color::rgb(0.2, 0.4, 0.6)));
        d.add_adjustment_layer(Adjustment::Invert);
        let full = d.composite();
        let p = full.get(5, 5);
        assert!((p[0] - 0.8).abs() < 1e-4);
        let part = d.composite_rect(Rect::new(4, 4, 3, 3));
        assert_eq!(part.get(1, 1), full.get(5, 5));
    }

    #[test]
    fn groups_merge_flatten() {
        let mut d = Document::new(8, 8, Some(Color::WHITE));
        let a = d.new_raster_layer();
        d.layer_mut(a)
            .unwrap()
            .image_mut()
            .fill(Color::rgb(0.0, 0.0, 0.0));
        let g = d.group_layer(a).unwrap();
        d.layer_mut(g).unwrap().opacity = 0.5;
        let p = d.composite().get(0, 0);
        assert!((p[0] - 0.5).abs() < 1e-3, "{p:?}");
        assert!(d.ungroup(g));
        d.merge_down(a);
        assert_eq!(d.layers.len(), 1);
        d.flatten();
        assert_eq!(d.layers.len(), 1);
    }

    #[test]
    fn composite_omit_layer_matches_full() {
        let mut d = Document::new(32, 32, Some(Color::WHITE));
        let id = d.new_raster_layer();
        d.layer_mut(id)
            .unwrap()
            .image_mut()
            .fill(Color::rgb(1.0, 0.0, 0.0));
        let full = d.composite();
        let under = d.composite_omit_layer(id);
        let over = d.composite_layer_over(&under, id, d.bounds());
        assert_eq!(over.get(10, 10), full.get(10, 10));
    }

    #[test]
    #[ignore = "manual perf probe (run with --release -- --ignored --nocapture)"]
    fn perf_brush_dirty_rect_6000x4000() {
        let mut d = Document::new(6000, 4000, Some(Color::rgb(0.9, 0.9, 0.9)));
        for i in 0..19 {
            let id = d.new_raster_layer();
            d.layer_mut(id).unwrap().image_mut().fill(Color::rgb(
                (i as f32 * 0.05) % 1.0,
                0.3,
                0.6,
            ));
        }
        let active = d.active;
        let under = d.composite_omit_layer(active);
        let r = Rect::new(2800, 1800, 400, 400);
        let t0 = std::time::Instant::now();
        for _ in 0..30 {
            let _ = d.composite_rect(r);
        }
        let full_ms = t0.elapsed().as_secs_f64() * 1000.0 / 30.0;
        let t1 = std::time::Instant::now();
        for _ in 0..30 {
            let _ = d.composite_layer_over(&under, active, r);
        }
        let cached_ms = t1.elapsed().as_secs_f64() * 1000.0 / 30.0;
        println!("composite_rect avg {full_ms:.2} ms");
        println!("cached under+active avg {cached_ms:.2} ms");
        assert!(
            cached_ms < 16.0,
            "cached path should stay under 16 ms, got {cached_ms}"
        );
    }

    #[test]
    fn whole_image_ops() {
        let mut d = Document::new(30, 20, Some(Color::WHITE));
        d.resize_image(60, 40, Resample::Bicubic);
        assert_eq!((d.width, d.height), (60, 40));
        d.canvas_size(100, 40, Anchor::Center);
        assert_eq!(d.layers[0].image().width, 100);
        d.rotate_canvas_cw();
        assert_eq!((d.width, d.height), (40, 100));
        d.crop(Rect::new(5, 5, 10, 10));
        assert_eq!((d.width, d.height), (10, 10));
        d.rotate_canvas(30.0);
        assert!(d.width > 10);
    }
}
