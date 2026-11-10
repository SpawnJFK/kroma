//! Layers: raster, text, fill, adjustment, group. Masks, clipping, styles, locks.

use crate::adjust::Adjustment;
use crate::blend::BlendMode;
use crate::buffer::{Image, Mask};
use crate::color::Color;
use crate::effects::Effect;
use crate::gradient::GradientFill;
use crate::text::TextStyle;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub type LayerId = u64;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REV: AtomicU64 = AtomicU64::new(1);

pub fn new_id() -> LayerId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}
pub fn bump_id_floor(v: LayerId) {
    NEXT_ID.fetch_max(v + 1, Ordering::Relaxed);
}
fn new_rev() -> u64 {
    NEXT_REV.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FillSource {
    Solid(Color),
    Gradient {
        fill: GradientFill,
        from: (f32, f32),
        to: (f32, f32),
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayerKind {
    Raster,
    Text(TextStyle),
    Fill(FillSource),
    Adjustment(Adjustment),
    /// Children are stored bottom-to-top like the document stack.
    Group {
        children: Vec<Layer>,
        pass_through: bool,
        expanded: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Locks {
    pub transparency: bool,
    pub pixels: bool,
    pub position: bool,
    pub all: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    /// Pixel content (doc-sized). For Text and Fill layers this is the rendered cache.
    #[serde(skip)]
    image: Image,
    pub visible: bool,
    pub opacity: f32,
    /// Fill opacity: affects content but not layer styles.
    pub fill: f32,
    pub blend: BlendMode,
    #[serde(skip)]
    mask: Option<Mask>,
    pub mask_enabled: bool,
    pub has_mask: bool,
    /// Clipping mask: clipped to the nearest non-clipped layer below.
    pub clipped: bool,
    pub effects: Vec<Effect>,
    pub effects_visible: bool,
    pub locks: Locks,
    /// Color label like Photoshop's layer colors.
    pub label: Option<Color>,
    #[serde(skip, default = "new_rev")]
    rev: u64,
    #[serde(skip)]
    fx_cache: Arc<Mutex<Option<(u64, Image)>>>,
}

impl PartialEq for Layer {
    fn eq(&self, o: &Self) -> bool {
        self.id == o.id && self.rev == o.rev
    }
}

impl Layer {
    fn base(name: &str, kind: LayerKind, image: Image) -> Layer {
        Layer {
            id: new_id(),
            name: name.to_string(),
            kind,
            image,
            visible: true,
            opacity: 1.0,
            fill: 1.0,
            blend: BlendMode::Normal,
            mask: None,
            mask_enabled: true,
            has_mask: false,
            clipped: false,
            effects: Vec::new(),
            effects_visible: true,
            locks: Locks::default(),
            label: None,
            rev: new_rev(),
            fx_cache: Arc::new(Mutex::new(None)),
        }
    }
    pub fn raster(name: &str, w: u32, h: u32) -> Layer {
        Layer::base(name, LayerKind::Raster, Image::new(w, h))
    }
    pub fn from_image(name: &str, image: Image) -> Layer {
        Layer::base(name, LayerKind::Raster, image)
    }
    pub fn text(style: TextStyle, w: u32, h: u32) -> Layer {
        let name: String = style
            .text
            .lines()
            .next()
            .unwrap_or("Text")
            .chars()
            .take(30)
            .collect();
        let mut l = Layer::base(&name, LayerKind::Text(style), Image::new(w, h));
        l.rerender(w, h);
        l
    }
    pub fn fill(name: &str, src: FillSource, w: u32, h: u32) -> Layer {
        let mut l = Layer::base(name, LayerKind::Fill(src), Image::new(w, h));
        l.rerender(w, h);
        l
    }
    pub fn adjustment(adj: Adjustment, w: u32, h: u32) -> Layer {
        let name = adj.name().to_string();
        let mut l = Layer::base(&name, LayerKind::Adjustment(adj), Image::new(0, 0));
        // adjustment layers come with a white (reveal-all) mask, like Photoshop
        l.set_mask(Some(Mask::new(w, h, 1.0)));
        l
    }
    pub fn group(name: &str, children: Vec<Layer>) -> Layer {
        Layer::base(
            name,
            LayerKind::Group {
                children,
                pass_through: true,
                expanded: true,
            },
            Image::new(0, 0),
        )
    }

    pub fn rev(&self) -> u64 {
        self.rev
    }
    pub fn touch(&mut self) {
        self.rev = new_rev();
    }
    pub fn image(&self) -> &Image {
        &self.image
    }
    /// Mutable pixels; marks the layer dirty.
    pub fn image_mut(&mut self) -> &mut Image {
        self.touch();
        &mut self.image
    }
    pub fn set_image(&mut self, img: Image) {
        self.touch();
        self.image = img;
    }
    pub fn mask(&self) -> Option<&Mask> {
        self.mask.as_ref()
    }
    pub fn mask_mut(&mut self) -> Option<&mut Mask> {
        self.touch();
        self.mask.as_mut()
    }
    pub fn set_mask(&mut self, m: Option<Mask>) {
        self.touch();
        self.has_mask = m.is_some();
        self.mask = m;
    }
    pub fn active_mask(&self) -> Option<&Mask> {
        if self.mask_enabled {
            self.mask.as_ref()
        } else {
            None
        }
    }
    pub fn is_raster_like(&self) -> bool {
        matches!(
            self.kind,
            LayerKind::Raster | LayerKind::Text(_) | LayerKind::Fill(_)
        )
    }
    pub fn is_group(&self) -> bool {
        matches!(self.kind, LayerKind::Group { .. })
    }
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            LayerKind::Raster => "Pixel",
            LayerKind::Text(_) => "Text",
            LayerKind::Fill(_) => "Fill",
            LayerKind::Adjustment(_) => "Adjustment",
            LayerKind::Group { .. } => "Group",
        }
    }
    pub fn children(&self) -> Option<&Vec<Layer>> {
        match &self.kind {
            LayerKind::Group { children, .. } => Some(children),
            _ => None,
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Layer>> {
        self.touch();
        match &mut self.kind {
            LayerKind::Group { children, .. } => Some(children),
            _ => None,
        }
    }

    /// Re-render text/fill caches (after edits or canvas size changes).
    pub fn rerender(&mut self, w: u32, h: u32) {
        match &self.kind {
            LayerKind::Text(style) => {
                let img = crate::text::render(style, w, h);
                self.set_image(img);
            }
            LayerKind::Fill(src) => {
                let mut img = Image::new(w, h);
                match src {
                    FillSource::Solid(c) => img.fill(*c),
                    FillSource::Gradient { fill, from, to } => {
                        crate::gradient::draw_gradient(&mut img, None, fill, *from, *to)
                    }
                }
                self.set_image(img);
            }
            _ => {}
        }
    }

    /// Text/Fill -> Raster (Layer > Rasterize).
    pub fn rasterize(&mut self) {
        if matches!(self.kind, LayerKind::Text(_) | LayerKind::Fill(_)) {
            self.kind = LayerKind::Raster;
            self.touch();
        }
    }

    /// Content with layer mask applied (alpha multiplied), without styles.
    pub fn masked_content(&self) -> Image {
        match self.active_mask() {
            None => self.image.clone(),
            Some(m) => {
                let mut img = self.image.clone();
                let md = m.data();
                for (i, p) in img.pixels_mut().iter_mut().enumerate() {
                    p[3] *= md[i];
                }
                img
            }
        }
    }

    pub fn has_active_effects(&self) -> bool {
        self.effects_visible && self.effects.iter().any(|e| e.enabled())
    }

    /// Content with mask + layer styles + fill opacity, cached by revision.
    pub fn styled_content(&self) -> Image {
        if !self.has_active_effects() {
            let mut img = self.masked_content();
            if self.fill < 1.0 {
                for p in img.pixels_mut() {
                    p[3] *= self.fill;
                }
            }
            return img;
        }
        let mut cache = self.fx_cache.lock().unwrap();
        if let Some((rev, img)) = cache.as_ref() {
            if *rev == self.rev {
                return img.clone();
            }
        }
        let img = crate::effects::render(&self.masked_content(), &self.effects, self.fill);
        *cache = Some((self.rev, img.clone()));
        img
    }

    /// Fresh copy with a new id (Layer > Duplicate).
    pub fn duplicate(&self) -> Layer {
        let mut l = self.clone();
        l.id = new_id();
        l.name = format!("{} copy", self.name);
        l.fx_cache = Arc::new(Mutex::new(None));
        if let LayerKind::Group { children, .. } = &mut l.kind {
            for c in children.iter_mut() {
                *c = c.duplicate_keep_name();
            }
        }
        l.touch();
        l
    }
    fn duplicate_keep_name(&self) -> Layer {
        let mut l = self.duplicate();
        l.name = self.name.clone();
        l
    }

    /// Resize the layer's pixel storage (used by Image Size / Canvas Size / Crop / Rotate).
    pub fn map_pixels(
        &mut self,
        f: &dyn Fn(&Image) -> Image,
        fm: &dyn Fn(&Mask) -> Mask,
        w: u32,
        h: u32,
    ) {
        match &mut self.kind {
            LayerKind::Raster => {
                let img = f(&self.image);
                self.image = img;
            }
            LayerKind::Text(_) | LayerKind::Fill(_) => {}
            LayerKind::Adjustment(_) => {}
            LayerKind::Group { children, .. } => {
                for c in children.iter_mut() {
                    c.map_pixels(f, fm, w, h);
                }
            }
        }
        if let Some(m) = &self.mask {
            self.mask = Some(fm(m));
        }
        self.rerender(w, h);
        self.touch();
    }

    // serialization helpers (used by io)
    pub(crate) fn set_image_raw(&mut self, img: Image) {
        self.image = img;
    }
    pub(crate) fn set_mask_raw(&mut self, m: Option<Mask>) {
        self.mask = m;
    }
    pub(crate) fn reset_runtime(&mut self) {
        self.rev = new_rev();
        self.fx_cache = Arc::new(Mutex::new(None));
        bump_id_floor(self.id);
    }
}
