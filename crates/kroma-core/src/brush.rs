//! Brush engine. Implements Photoshop's opacity/flow model: flow accumulates per dab into a
//! stroke coverage buffer, opacity caps the whole stroke. Covers Brush, Pencil, Eraser,
//! Clone Stamp, Healing Brush, Smudge, Blur, Sharpen, Dodge, Burn, Sponge and mask painting.

use crate::blend::{composite_px, BlendMode};
use crate::buffer::{Image, Mask, Rect};
use crate::color::*;
use crate::filter::gaussian_blur;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToneRange {
    Shadows,
    #[default]
    Midtones,
    Highlights,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BrushTool {
    Paint,
    Pencil,
    Eraser,
    CloneStamp,
    Healing,
    Smudge,
    Blur,
    Sharpen,
    Dodge(ToneRange),
    Burn(ToneRange),
    Sponge { saturate: bool },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrushSettings {
    /// Diameter in pixels.
    pub size: f32,
    /// 0 = very soft, 1 = hard edge.
    pub hardness: f32,
    /// Stroke opacity cap 0..1.
    pub opacity: f32,
    /// Per-dab flow 0..1 (also "strength" for smudge/blur/sharpen, "exposure" for dodge/burn).
    pub flow: f32,
    /// Dab spacing as a fraction of diameter.
    pub spacing: f32,
    pub angle: f32,
    /// 1 = circle, < 1 = squashed ellipse.
    pub roundness: f32,
    pub mode: BlendMode,
    pub color: Color,
    pub pressure_size: bool,
    pub pressure_opacity: bool,
    /// Random scatter as fraction of diameter.
    pub scatter: f32,
    /// Size jitter 0..1.
    pub size_jitter: f32,
    /// Keep the target's existing alpha (Lock Transparent Pixels).
    pub lock_alpha: bool,
}

impl Default for BrushSettings {
    fn default() -> Self {
        BrushSettings {
            size: 30.0,
            hardness: 0.8,
            opacity: 1.0,
            flow: 1.0,
            spacing: 0.15,
            angle: 0.0,
            roundness: 1.0,
            mode: BlendMode::Normal,
            color: Color::BLACK,
            pressure_size: true,
            pressure_opacity: false,
            scatter: 0.0,
            size_jitter: 0.0,
            lock_alpha: false,
        }
    }
}

/// Saved brush presets (an addition to PS defaults: tuned starting points).
pub fn presets() -> Vec<(&'static str, BrushSettings)> {
    let b = BrushSettings::default();
    vec![
        (
            "Hard Round",
            BrushSettings {
                hardness: 1.0,
                ..b.clone()
            },
        ),
        (
            "Soft Round",
            BrushSettings {
                hardness: 0.0,
                ..b.clone()
            },
        ),
        (
            "Airbrush",
            BrushSettings {
                hardness: 0.0,
                flow: 0.08,
                size: 80.0,
                ..b.clone()
            },
        ),
        (
            "Ink Pen",
            BrushSettings {
                hardness: 0.95,
                size: 6.0,
                spacing: 0.05,
                pressure_size: true,
                ..b.clone()
            },
        ),
        (
            "Chisel",
            BrushSettings {
                hardness: 0.9,
                roundness: 0.25,
                angle: 45.0,
                ..b.clone()
            },
        ),
        (
            "Spray",
            BrushSettings {
                hardness: 0.6,
                size: 6.0,
                scatter: 4.0,
                size_jitter: 0.6,
                flow: 0.6,
                spacing: 0.3,
                ..b.clone()
            },
        ),
        (
            "Soft Eraser",
            BrushSettings {
                hardness: 0.2,
                size: 60.0,
                ..b
            },
        ),
    ]
}

/// Dab alpha at offset (dx,dy) from the dab center.
#[inline]
fn dab_alpha(
    dx: f32,
    dy: f32,
    r: f32,
    hardness: f32,
    cos: f32,
    sin: f32,
    roundness: f32,
    aliased: bool,
) -> f32 {
    let rx = dx * cos + dy * sin;
    let ry = (-dx * sin + dy * cos) / roundness.max(0.05);
    let d = (rx * rx + ry * ry).sqrt();
    if aliased {
        return if d <= r { 1.0 } else { 0.0 };
    }
    if d >= r {
        return 0.0;
    }
    let h = hardness.clamp(0.0, 0.999);
    let inner = r * h;
    if d <= inner {
        1.0
    } else {
        let t = (d - inner) / (r - inner).max(1e-3);
        // smooth falloff; hardness 0 gives a gaussian-ish soft tip
        let s = 1.0 - t;
        s * s * (3.0 - 2.0 * s)
    }
}

pub struct Stroke {
    pub tool: BrushTool,
    pub s: BrushSettings,
    original: Image,
    coverage: Vec<f32>,
    width: u32,
    height: u32,
    last: Option<(f32, f32, f32)>,
    residual: f32,
    /// Clone/heal source image and offset (source = dest + offset).
    source: Option<Image>,
    offset: (f32, f32),
    low_src: Option<Image>,
    low_dst: Option<Image>,
    dab_count: u32,
    last_dab: Option<(f32, f32)>,
    pub dirty: Rect,
    // mask painting
    original_mask: Option<Mask>,
}

impl Stroke {
    /// Start a stroke on `target`. For CloneStamp/Healing pass the sample source (usually the
    /// layer itself or the merged image) and the offset from destination to source.
    pub fn begin(
        target: &Image,
        tool: BrushTool,
        s: BrushSettings,
        source: Option<Image>,
        offset: (f32, f32),
    ) -> Stroke {
        let (low_src, low_dst) = if tool == BrushTool::Healing {
            let r = (s.size * 0.35).max(2.0);
            let src = source.clone().unwrap_or_else(|| target.clone());
            (Some(gaussian_blur(&src, r)), Some(gaussian_blur(target, r)))
        } else {
            (None, None)
        };
        Stroke {
            tool,
            original: target.clone(),
            coverage: vec![0.0; (target.width * target.height) as usize],
            width: target.width,
            height: target.height,
            last: None,
            residual: 0.0,
            source,
            offset,
            low_src,
            low_dst,
            dab_count: 0,
            last_dab: None,
            dirty: Rect::default(),
            original_mask: None,
            s,
        }
    }

    pub fn begin_mask(mask: &Mask, tool: BrushTool, s: BrushSettings) -> Stroke {
        Stroke {
            tool,
            original: Image::new(0, 0),
            coverage: vec![0.0; (mask.width * mask.height) as usize],
            width: mask.width,
            height: mask.height,
            last: None,
            residual: 0.0,
            source: None,
            offset: (0.0, 0.0),
            low_src: None,
            low_dst: None,
            dab_count: 0,
            last_dab: None,
            dirty: Rect::default(),
            original_mask: Some(mask.clone()),
            s,
        }
    }

    fn dab_positions(&mut self, x: f32, y: f32, pressure: f32) -> Vec<(f32, f32, f32)> {
        let mut out = Vec::new();
        match self.last {
            None => out.push((x, y, pressure)),
            Some((lx, ly, lp)) => {
                let (dx, dy) = (x - lx, y - ly);
                let dist = (dx * dx + dy * dy).sqrt();
                let size = self.s.size
                    * if self.s.pressure_size {
                        pressure.max(0.05)
                    } else {
                        1.0
                    };
                let step = (size * self.s.spacing).max(0.5);
                let mut t = step - self.residual;
                while t <= dist {
                    let f = t / dist.max(1e-6);
                    out.push((lx + dx * f, ly + dy * f, lp + (pressure - lp) * f));
                    t += step;
                }
                self.residual = dist - (t - step);
            }
        }
        self.last = Some((x, y, pressure));
        out
    }

    /// Add a point to the stroke and paint into `target`. Returns the dirty rect for this call.
    pub fn add_point(
        &mut self,
        target: &mut Image,
        sel: Option<&Mask>,
        x: f32,
        y: f32,
        pressure: f32,
    ) -> Rect {
        let dabs = self.dab_positions(x, y, pressure);
        let mut dirty = Rect::default();
        for (dx, dy, p) in dabs {
            dirty = dirty.union(&self.dab(target, sel, dx, dy, p));
        }
        self.dirty = self.dirty.union(&dirty);
        dirty
    }

    pub fn add_point_mask(&mut self, mask: &mut Mask, x: f32, y: f32, pressure: f32) -> Rect {
        let dabs = self.dab_positions(x, y, pressure);
        let mut dirty = Rect::default();
        for (dx, dy, p) in dabs {
            dirty = dirty.union(&self.dab_mask(mask, dx, dy, p));
        }
        self.dirty = self.dirty.union(&dirty);
        dirty
    }

    fn jitter(&mut self, x: f32, y: f32, size: f32) -> (f32, f32, f32) {
        self.dab_count += 1;
        let k = self.dab_count;
        let mut size = size;
        let (mut x, mut y) = (x, y);
        if self.s.size_jitter > 0.0 {
            size *= 1.0 - crate::filter::rand01(k, 3) * self.s.size_jitter;
        }
        if self.s.scatter > 0.0 {
            let a = crate::filter::rand01(k, 1) * std::f32::consts::TAU;
            let r = crate::filter::rand01(k, 2) * self.s.scatter * self.s.size * 0.5;
            x += a.cos() * r;
            y += a.sin() * r;
        }
        (x, y, size)
    }

    fn dab_rect(&self, x: f32, y: f32, r: f32) -> Rect {
        Rect::from_points(
            (x - r - 1.0).floor() as i32,
            (y - r - 1.0).floor() as i32,
            (x + r + 2.0).ceil() as i32,
            (y + r + 2.0).ceil() as i32,
        )
        .intersect(&Rect::new(0, 0, self.width as i32, self.height as i32))
    }

    fn dab(
        &mut self,
        target: &mut Image,
        sel: Option<&Mask>,
        x: f32,
        y: f32,
        pressure: f32,
    ) -> Rect {
        let s = self.s.clone();
        let size = s.size
            * if s.pressure_size {
                pressure.max(0.05)
            } else {
                1.0
            };
        let (x, y, size) = self.jitter(x, y, size);
        let r = (size / 2.0).max(0.5);
        let rect = self.dab_rect(x, y, r);
        if rect.is_empty() {
            return rect;
        }
        let (sn, cs) = s.angle.to_radians().sin_cos();
        let aliased = self.tool == BrushTool::Pencil;
        let flow = s.flow * if s.pressure_opacity { pressure } else { 1.0 };
        let w = self.width as usize;
        let lock_alpha = s.lock_alpha;

        match self.tool {
            BrushTool::Paint
            | BrushTool::Pencil
            | BrushTool::Eraser
            | BrushTool::CloneStamp
            | BrushTool::Healing => {
                let color = s.color.px();
                let tool = self.tool;
                let (ox, oy) = self.offset;
                for py in rect.y..rect.bottom() {
                    for px in rect.x..rect.right() {
                        let a = dab_alpha(
                            px as f32 + 0.5 - x,
                            py as f32 + 0.5 - y,
                            r,
                            s.hardness,
                            cs,
                            sn,
                            s.roundness,
                            aliased,
                        );
                        if a <= 0.0 {
                            continue;
                        }
                        let i = py as usize * w + px as usize;
                        let c = &mut self.coverage[i];
                        *c += flow * a * (1.0 - *c);
                        let m = sel.map(|m| m.data()[i]).unwrap_or(1.0);
                        let amt = (s.opacity * *c * m).min(1.0);
                        let orig = self.original.pixels()[i];
                        let out = match tool {
                            BrushTool::Paint | BrushTool::Pencil => {
                                composite_px(orig, color, s.mode, amt, px as u32, py as u32)
                            }
                            BrushTool::Eraser => {
                                let mut o = orig;
                                o[3] *= 1.0 - amt;
                                o
                            }
                            BrushTool::CloneStamp | BrushTool::Healing => {
                                let src = self.source.as_ref().unwrap_or(&self.original);
                                let sx = px as f32 + 0.5 + ox;
                                let sy = py as f32 + 0.5 + oy;
                                let mut sp = src.sample_bilinear_transparent(sx, sy);
                                if tool == BrushTool::Healing {
                                    if let (Some(ls), Some(ld)) = (&self.low_src, &self.low_dst) {
                                        let l_src = ls.sample_bilinear(sx, sy);
                                        let l_dst = ld.pixels()[i];
                                        for k in 0..3 {
                                            sp[k] = clamp01(sp[k] - l_src[k] + l_dst[k]);
                                        }
                                        sp[3] = sp[3].max(orig[3]);
                                    }
                                }
                                composite_px(orig, sp, BlendMode::Normal, amt, px as u32, py as u32)
                            }
                            _ => unreachable!(),
                        };
                        let t = &mut target.pixels_mut()[i];
                        *t = out;
                        if lock_alpha {
                            t[3] = orig[3];
                        }
                    }
                }
            }
            BrushTool::Smudge => {
                let (lx, ly) = self.last_dab.unwrap_or((x, y));
                self.last_dab = Some((x, y));
                // drag pixels from the previous dab position toward the current one
                let (ddx, ddy) = (x - lx, y - ly);
                let snapshot = target.sub_image(rect.inflate(r.ceil() as i32 + 4));
                let sr = rect
                    .inflate(r.ceil() as i32 + 4)
                    .intersect(&target.bounds());
                for py in rect.y..rect.bottom() {
                    for px in rect.x..rect.right() {
                        let a = dab_alpha(
                            px as f32 + 0.5 - x,
                            py as f32 + 0.5 - y,
                            r,
                            s.hardness,
                            cs,
                            sn,
                            s.roundness,
                            false,
                        );
                        if a <= 0.0 {
                            continue;
                        }
                        let i = py as usize * w + px as usize;
                        let m = sel.map(|m| m.data()[i]).unwrap_or(1.0);
                        let k = a * s.flow * m * 0.9;
                        let src = snapshot.sample_bilinear(
                            (px - sr.x) as f32 + 0.5 - ddx,
                            (py - sr.y) as f32 + 0.5 - ddy,
                        );
                        let t = &mut target.pixels_mut()[i];
                        for c in 0..4 {
                            t[c] += (src[c] - t[c]) * k;
                        }
                    }
                }
            }
            BrushTool::Blur | BrushTool::Sharpen => {
                let snap_r = rect.inflate(2).intersect(&target.bounds());
                let snap = target.sub_image(snap_r);
                let sharpen = self.tool == BrushTool::Sharpen;
                for py in rect.y..rect.bottom() {
                    for px in rect.x..rect.right() {
                        let a = dab_alpha(
                            px as f32 + 0.5 - x,
                            py as f32 + 0.5 - y,
                            r,
                            s.hardness,
                            cs,
                            sn,
                            s.roundness,
                            false,
                        );
                        if a <= 0.0 {
                            continue;
                        }
                        let i = py as usize * w + px as usize;
                        let m = sel.map(|m| m.data()[i]).unwrap_or(1.0);
                        let (lx, ly) = (px - snap_r.x, py - snap_r.y);
                        let mut avg = [0.0f32; 4];
                        for j in -1..=1 {
                            for k in -1..=1 {
                                let q = snap.get_clamped(lx + k, ly + j);
                                for c in 0..4 {
                                    avg[c] += q[c] / 9.0;
                                }
                            }
                        }
                        let cur = snap.get_clamped(lx, ly);
                        let k = a * s.flow * m * 0.5;
                        let t = &mut target.pixels_mut()[i];
                        for c in 0..3 {
                            let goal = if sharpen {
                                clamp01(cur[c] + (cur[c] - avg[c]) * 1.5)
                            } else {
                                avg[c]
                            };
                            t[c] += (goal - t[c]) * k;
                        }
                        if !sharpen {
                            t[3] += (avg[3] - t[3]) * k;
                        }
                    }
                }
            }
            BrushTool::Dodge(range) | BrushTool::Burn(range) => {
                let dodge = matches!(self.tool, BrushTool::Dodge(_));
                for py in rect.y..rect.bottom() {
                    for px in rect.x..rect.right() {
                        let a = dab_alpha(
                            px as f32 + 0.5 - x,
                            py as f32 + 0.5 - y,
                            r,
                            s.hardness,
                            cs,
                            sn,
                            s.roundness,
                            false,
                        );
                        if a <= 0.0 {
                            continue;
                        }
                        let i = py as usize * w + px as usize;
                        let m = sel.map(|m| m.data()[i]).unwrap_or(1.0);
                        let t = &mut target.pixels_mut()[i];
                        let l = luma709([t[0], t[1], t[2]]);
                        let wr = match range {
                            ToneRange::Shadows => (1.0 - l).powi(2),
                            ToneRange::Midtones => 1.0 - (2.0 * l - 1.0).powi(2),
                            ToneRange::Highlights => l * l,
                        };
                        let k = a * s.flow * m * wr * 0.12;
                        for c in 0..3 {
                            t[c] = if dodge {
                                clamp01(t[c] + (1.0 - t[c]) * k)
                            } else {
                                clamp01(t[c] - t[c] * k)
                            };
                        }
                    }
                }
            }
            BrushTool::Sponge { saturate } => {
                for py in rect.y..rect.bottom() {
                    for px in rect.x..rect.right() {
                        let a = dab_alpha(
                            px as f32 + 0.5 - x,
                            py as f32 + 0.5 - y,
                            r,
                            s.hardness,
                            cs,
                            sn,
                            s.roundness,
                            false,
                        );
                        if a <= 0.0 {
                            continue;
                        }
                        let i = py as usize * w + px as usize;
                        let m = sel.map(|m| m.data()[i]).unwrap_or(1.0);
                        let t = &mut target.pixels_mut()[i];
                        let l = luma709([t[0], t[1], t[2]]);
                        let k = a * s.flow * m * 0.1 * if saturate { 1.0 } else { -1.0 };
                        for c in 0..3 {
                            t[c] = clamp01(t[c] + (t[c] - l) * k);
                        }
                    }
                }
            }
        }
        rect
    }

    fn dab_mask(&mut self, mask: &mut Mask, x: f32, y: f32, pressure: f32) -> Rect {
        let s = self.s.clone();
        let size = s.size
            * if s.pressure_size {
                pressure.max(0.05)
            } else {
                1.0
            };
        let (x, y, size) = self.jitter(x, y, size);
        let r = (size / 2.0).max(0.5);
        let rect = self.dab_rect(x, y, r);
        let (sn, cs) = s.angle.to_radians().sin_cos();
        let aliased = self.tool == BrushTool::Pencil;
        let w = self.width as usize;
        // On masks: paint = set toward luminance of color, eraser = reveal (1.0)
        let target_v = if self.tool == BrushTool::Eraser {
            1.0
        } else {
            luma709([s.color.r, s.color.g, s.color.b])
        };
        let orig = self.original_mask.clone().unwrap();
        for py in rect.y..rect.bottom() {
            for px in rect.x..rect.right() {
                let a = dab_alpha(
                    px as f32 + 0.5 - x,
                    py as f32 + 0.5 - y,
                    r,
                    s.hardness,
                    cs,
                    sn,
                    s.roundness,
                    aliased,
                );
                if a <= 0.0 {
                    continue;
                }
                let i = py as usize * w + px as usize;
                let c = &mut self.coverage[i];
                *c += s.flow * a * (1.0 - *c);
                let amt = (s.opacity * *c).min(1.0);
                let o = orig.data()[i];
                mask.data_mut()[i] = o + (target_v - o) * amt;
            }
        }
        rect
    }
}

/// Paint Bucket: fill the region similar to (x,y) with `color` (contiguous/global, tolerance 0..255).
pub fn bucket_fill(
    img: &mut Image,
    sel: Option<&Mask>,
    x: u32,
    y: u32,
    color: Color,
    tolerance: f32,
    contiguous: bool,
    opacity: f32,
    mode: BlendMode,
) {
    let region = crate::selection::magic_wand(img, x, y, tolerance, contiguous, true);
    let c = color.px();
    let w = img.width as usize;
    let rd = region.data();
    let sd = sel.map(|s| s.data());
    for (i, p) in img.pixels_mut().iter_mut().enumerate() {
        let m = rd[i] * sd.map(|s| s[i]).unwrap_or(1.0);
        if m > 0.0 {
            *p = composite_px(*p, c, mode, opacity * m, (i % w) as u32, (i / w) as u32);
        }
    }
}

/// Edit > Fill with a solid color, honoring the selection.
pub fn fill_selection(
    img: &mut Image,
    sel: Option<&Mask>,
    color: Color,
    opacity: f32,
    mode: BlendMode,
    preserve_alpha: bool,
) {
    let c = color.px();
    let w = img.width as usize;
    let sd = sel.map(|s| s.data());
    for (i, p) in img.pixels_mut().iter_mut().enumerate() {
        let m = sd.map(|s| s[i]).unwrap_or(1.0);
        if m > 0.0 {
            let a = p[3];
            *p = composite_px(*p, c, mode, opacity * m, (i % w) as u32, (i / w) as u32);
            if preserve_alpha {
                p[3] = a;
            }
        }
    }
}

/// Edit > Stroke: outline the selection with `width` px of `color`.
pub fn stroke_selection(
    img: &mut Image,
    sel: &Mask,
    color: Color,
    width: f32,
    location: StrokeLocation,
) {
    let ring = match location {
        StrokeLocation::Inside => {
            let inner = crate::selection::contract(sel, width);
            Mask::from_vec(
                sel.width,
                sel.height,
                sel.data()
                    .iter()
                    .zip(inner.data())
                    .map(|(a, b)| (a - b).max(0.0))
                    .collect(),
            )
        }
        StrokeLocation::Center => crate::selection::border(sel, width),
        StrokeLocation::Outside => {
            let outer = crate::selection::expand(sel, width);
            Mask::from_vec(
                sel.width,
                sel.height,
                outer
                    .data()
                    .iter()
                    .zip(sel.data())
                    .map(|(a, b)| (a - b).max(0.0))
                    .collect(),
            )
        }
    };
    fill_selection(img, Some(&ring), color, 1.0, BlendMode::Normal, false);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum StrokeLocation {
    Inside,
    #[default]
    Center,
    Outside,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opacity_caps_stroke() {
        let mut img = Image::filled(64, 64, [1.0, 1.0, 1.0, 1.0]);
        let s = BrushSettings {
            size: 20.0,
            hardness: 1.0,
            opacity: 0.5,
            flow: 1.0,
            color: Color::BLACK,
            pressure_size: false,
            ..Default::default()
        };
        let mut st = Stroke::begin(&img, BrushTool::Paint, s, None, (0.0, 0.0));
        for i in 0..20 {
            st.add_point(&mut img, None, 20.0 + i as f32, 32.0, 1.0);
        }
        // overlapping dabs never exceed 50%
        let p = img.get(30, 32);
        assert!((p[0] - 0.5).abs() < 0.02, "{p:?}");
    }
    #[test]
    fn eraser_and_selection() {
        let mut img = Image::filled(32, 32, [1.0, 0.0, 0.0, 1.0]);
        let sel = crate::selection::rect_mask(32, 32, 0.0, 0.0, 16.0, 32.0);
        let s = BrushSettings {
            size: 40.0,
            hardness: 1.0,
            pressure_size: false,
            ..Default::default()
        };
        let mut st = Stroke::begin(&img, BrushTool::Eraser, s, None, (0.0, 0.0));
        st.add_point(&mut img, Some(&sel), 16.0, 16.0, 1.0);
        assert_eq!(img.get(4, 16)[3], 0.0);
        assert_eq!(img.get(28, 16)[3], 1.0);
    }
}
