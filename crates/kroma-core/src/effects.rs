//! Layer Styles (fx): Drop Shadow, Inner Shadow, Outer Glow, Inner Glow, Bevel & Emboss,
//! Satin, Color Overlay, Gradient Overlay, Stroke.

use crate::blend::{composite_px, BlendMode};
use crate::brush::StrokeLocation;
use crate::buffer::{Image, Mask};
use crate::color::*;
use crate::filter::{gaussian_blur_mask, morph_mask};
use crate::gradient::{gradient_t, Gradient, GradientShape};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    DropShadow {
        enabled: bool,
        color: Color,
        opacity: f32,
        angle: f32,
        distance: f32,
        size: f32,
        spread: f32,
        mode: BlendMode,
    },
    InnerShadow {
        enabled: bool,
        color: Color,
        opacity: f32,
        angle: f32,
        distance: f32,
        size: f32,
        choke: f32,
        mode: BlendMode,
    },
    OuterGlow {
        enabled: bool,
        color: Color,
        opacity: f32,
        size: f32,
        spread: f32,
        mode: BlendMode,
    },
    InnerGlow {
        enabled: bool,
        color: Color,
        opacity: f32,
        size: f32,
        choke: f32,
        from_center: bool,
        mode: BlendMode,
    },
    BevelEmboss {
        enabled: bool,
        depth: f32,
        size: f32,
        angle: f32,
        altitude: f32,
        highlight: Color,
        highlight_opacity: f32,
        shadow: Color,
        shadow_opacity: f32,
        soften: f32,
    },
    Satin {
        enabled: bool,
        color: Color,
        opacity: f32,
        angle: f32,
        distance: f32,
        size: f32,
        invert: bool,
    },
    ColorOverlay {
        enabled: bool,
        color: Color,
        opacity: f32,
        mode: BlendMode,
    },
    GradientOverlay {
        enabled: bool,
        gradient: Gradient,
        opacity: f32,
        angle: f32,
        shape: GradientShape,
        reverse: bool,
        mode: BlendMode,
    },
    Stroke {
        enabled: bool,
        color: Color,
        size: f32,
        position: StrokeLocation,
        opacity: f32,
        mode: BlendMode,
    },
}

impl Effect {
    pub fn name(&self) -> &'static str {
        match self {
            Effect::DropShadow { .. } => "Drop Shadow",
            Effect::InnerShadow { .. } => "Inner Shadow",
            Effect::OuterGlow { .. } => "Outer Glow",
            Effect::InnerGlow { .. } => "Inner Glow",
            Effect::BevelEmboss { .. } => "Bevel & Emboss",
            Effect::Satin { .. } => "Satin",
            Effect::ColorOverlay { .. } => "Color Overlay",
            Effect::GradientOverlay { .. } => "Gradient Overlay",
            Effect::Stroke { .. } => "Stroke",
        }
    }
    pub fn enabled(&self) -> bool {
        match self {
            Effect::DropShadow { enabled, .. }
            | Effect::InnerShadow { enabled, .. }
            | Effect::OuterGlow { enabled, .. }
            | Effect::InnerGlow { enabled, .. }
            | Effect::BevelEmboss { enabled, .. }
            | Effect::Satin { enabled, .. }
            | Effect::ColorOverlay { enabled, .. }
            | Effect::GradientOverlay { enabled, .. }
            | Effect::Stroke { enabled, .. } => *enabled,
        }
    }
    pub fn set_enabled(&mut self, v: bool) {
        match self {
            Effect::DropShadow { enabled, .. }
            | Effect::InnerShadow { enabled, .. }
            | Effect::OuterGlow { enabled, .. }
            | Effect::InnerGlow { enabled, .. }
            | Effect::BevelEmboss { enabled, .. }
            | Effect::Satin { enabled, .. }
            | Effect::ColorOverlay { enabled, .. }
            | Effect::GradientOverlay { enabled, .. }
            | Effect::Stroke { enabled, .. } => *enabled = v,
        }
    }
    pub fn catalog() -> Vec<Effect> {
        vec![
            Effect::BevelEmboss {
                enabled: true,
                depth: 1.0,
                size: 6.0,
                angle: 120.0,
                altitude: 30.0,
                highlight: Color::WHITE,
                highlight_opacity: 0.75,
                shadow: Color::BLACK,
                shadow_opacity: 0.75,
                soften: 0.0,
            },
            Effect::Stroke {
                enabled: true,
                color: Color::from_hex("#d6232f").unwrap(),
                size: 3.0,
                position: StrokeLocation::Outside,
                opacity: 1.0,
                mode: BlendMode::Normal,
            },
            Effect::InnerShadow {
                enabled: true,
                color: Color::BLACK,
                opacity: 0.6,
                angle: 120.0,
                distance: 5.0,
                size: 5.0,
                choke: 0.0,
                mode: BlendMode::Multiply,
            },
            Effect::InnerGlow {
                enabled: true,
                color: Color::from_hex("#ffffbe").unwrap(),
                opacity: 0.75,
                size: 5.0,
                choke: 0.0,
                from_center: false,
                mode: BlendMode::Screen,
            },
            Effect::Satin {
                enabled: true,
                color: Color::BLACK,
                opacity: 0.4,
                angle: 19.0,
                distance: 11.0,
                size: 14.0,
                invert: true,
            },
            Effect::ColorOverlay {
                enabled: true,
                color: Color::from_hex("#d6232f").unwrap(),
                opacity: 1.0,
                mode: BlendMode::Normal,
            },
            Effect::GradientOverlay {
                enabled: true,
                gradient: Gradient::two(Color::BLACK, Color::WHITE),
                opacity: 1.0,
                angle: 90.0,
                shape: GradientShape::Linear,
                reverse: false,
                mode: BlendMode::Normal,
            },
            Effect::OuterGlow {
                enabled: true,
                color: Color::from_hex("#ffffbe").unwrap(),
                opacity: 0.75,
                size: 10.0,
                spread: 0.0,
                mode: BlendMode::Screen,
            },
            Effect::DropShadow {
                enabled: true,
                color: Color::BLACK,
                opacity: 0.6,
                angle: 120.0,
                distance: 8.0,
                size: 10.0,
                spread: 0.0,
                mode: BlendMode::Multiply,
            },
        ]
    }
}

fn shift(m: &Mask, dx: f32, dy: f32) -> Mask {
    let (w, h) = (m.width as i32, m.height as i32);
    let (dx, dy) = (dx.round() as i32, dy.round() as i32);
    let data = (0..(w * h))
        .into_par_iter()
        .map(|i| m.get_i(i % w - dx, i / w - dy))
        .collect();
    Mask::from_vec(m.width, m.height, data)
}

fn soft(m: &Mask, size: f32, spread: f32) -> Mask {
    // spread: fraction of size used as hard expansion, remainder as blur
    let grow = size * spread.clamp(0.0, 1.0);
    let blur = size * (1.0 - spread.clamp(0.0, 1.0));
    let g = if grow > 0.5 {
        morph_mask(m, grow)
    } else {
        m.clone()
    };
    gaussian_blur_mask(&g, blur)
}

fn paint(
    dst: &mut Image,
    alpha: &[f32],
    color: Color,
    opacity: f32,
    mode: BlendMode,
    within: Option<&[f32]>,
) {
    let c = color.px();
    let w = dst.width as usize;
    dst.pixels_mut()
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, p)| {
            let mut a = alpha[i] * opacity;
            if let Some(wm) = within {
                a *= wm[i];
            }
            if a > 0.0 {
                let keep = p[3];
                *p = composite_px(*p, c, mode, a, (i % w) as u32, (i / w) as u32);
                if within.is_some() {
                    p[3] = keep;
                }
            }
        });
}

/// Render `content` (alpha = layer shape) with its effects. `fill` is Fill Opacity.
pub fn render(content: &Image, effects: &[Effect], fill: f32) -> Image {
    let (w, h) = (content.width, content.height);
    let a = Mask::from_alpha(content);
    let ad = a.data().to_vec();
    let mut out = Image::new(w, h);

    // 1. behind-content effects
    for e in effects.iter().filter(|e| e.enabled()) {
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
                let r = angle.to_radians();
                let m = soft(
                    &shift(&a, -r.cos() * distance, r.sin() * distance),
                    *size,
                    *spread,
                );
                // knock out under content
                let alpha: Vec<f32> = m.data().to_vec();
                let mut layer = Image::new(w, h);
                paint(
                    &mut layer,
                    &alpha,
                    *color,
                    *opacity,
                    BlendMode::Normal,
                    None,
                );
                composite_into(&mut out, &layer, *mode, 1.0);
            }
            Effect::OuterGlow {
                color,
                opacity,
                size,
                spread,
                mode,
                ..
            } => {
                let m = soft(&a, *size, *spread);
                let alpha: Vec<f32> = m
                    .data()
                    .iter()
                    .zip(&ad)
                    .map(|(g, s)| (g - s).max(0.0) * 1.0 + g * 0.0)
                    .collect();
                let mut layer = Image::new(w, h);
                paint(
                    &mut layer,
                    &alpha,
                    *color,
                    *opacity,
                    BlendMode::Normal,
                    None,
                );
                composite_into(&mut out, &layer, *mode, 1.0);
            }
            _ => {}
        }
    }

    // 2. interior: content at fill opacity, then interior effects clipped to shape
    let mut interior = content.clone();
    if fill < 1.0 {
        interior
            .pixels_mut()
            .par_iter_mut()
            .for_each(|p| p[3] *= fill);
    }
    let interior_effects: Vec<&Effect> = effects.iter().filter(|e| e.enabled()).collect();
    // overlays first (Satin, Color, Gradient), then glows/shadows, then bevel
    let order = |e: &Effect| match e {
        Effect::Satin { .. } => 0,
        Effect::GradientOverlay { .. } => 1,
        Effect::ColorOverlay { .. } => 2,
        Effect::InnerGlow { .. } => 3,
        Effect::InnerShadow { .. } => 4,
        Effect::BevelEmboss { .. } => 5,
        _ => 9,
    };
    let mut ie = interior_effects.clone();
    ie.sort_by_key(|e| order(e));
    let mut shape_layer = Image::new(w, h); // effects painted in the shape, composited onto interior
    for e in ie {
        match e {
            Effect::ColorOverlay {
                color,
                opacity,
                mode,
                ..
            } => {
                let mut l = Image::new(w, h);
                paint(&mut l, &ad, *color, *opacity, BlendMode::Normal, None);
                composite_into_keep_alpha(&mut interior, &l, *mode, &ad);
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
                let b = a.bounds_above(0.01);
                if b.is_empty() {
                    continue;
                }
                let (cx, cy) = (b.x as f32 + b.w as f32 / 2.0, b.y as f32 + b.h as f32 / 2.0);
                let r = angle.to_radians();
                let half = (b.w as f32 * r.cos().abs() + b.h as f32 * r.sin().abs()) / 2.0;
                let (p0, p1) =
                    if *shape == GradientShape::Linear || *shape == GradientShape::Reflected {
                        (
                            (cx - r.cos() * half, cy + r.sin() * half),
                            (cx + r.cos() * half, cy - r.sin() * half),
                        )
                    } else {
                        ((cx, cy), (cx + half, cy))
                    };
                let g = if *reverse {
                    gradient.reversed()
                } else {
                    gradient.clone()
                };
                let ww = w as usize;
                let mut l = Image::new(w, h);
                l.pixels_mut()
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(i, p)| {
                        if ad[i] > 0.0 {
                            let t = gradient_t(
                                *shape,
                                p0,
                                p1,
                                ((i % ww) as f32 + 0.5, (i / ww) as f32 + 0.5),
                            );
                            let c = g.sample(t);
                            *p = [c.r, c.g, c.b, c.a * ad[i] * opacity];
                        }
                    });
                composite_into_keep_alpha(&mut interior, &l, *mode, &ad);
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
                let r = angle.to_radians();
                let (dx, dy) = (r.cos() * distance, -r.sin() * distance);
                let m1 = gaussian_blur_mask(&shift(&a, dx, dy), *size);
                let m2 = gaussian_blur_mask(&shift(&a, -dx, -dy), *size);
                let alpha: Vec<f32> = m1
                    .data()
                    .iter()
                    .zip(m2.data())
                    .map(|(x, y)| {
                        let d = (x - y).abs();
                        if *invert {
                            1.0 - d
                        } else {
                            d
                        }
                    })
                    .collect();
                let mut l = Image::new(w, h);
                paint(
                    &mut l,
                    &alpha,
                    *color,
                    *opacity,
                    BlendMode::Normal,
                    Some(&ad),
                );
                composite_into_keep_alpha(&mut interior, &l, BlendMode::Multiply, &ad);
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
                let inv = Mask::from_vec(w, h, ad.iter().map(|v| 1.0 - v).collect());
                let g = soft(&inv, *size, *choke);
                let alpha: Vec<f32> = g
                    .data()
                    .iter()
                    .zip(&ad)
                    .map(|(gv, s)| if *from_center { (1.0 - gv) * s } else { gv * s })
                    .collect();
                let mut l = Image::new(w, h);
                paint(&mut l, &alpha, *color, *opacity, BlendMode::Normal, None);
                composite_into_keep_alpha(&mut interior, &l, *mode, &ad);
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
                let r = angle.to_radians();
                let inv = Mask::from_vec(w, h, ad.iter().map(|v| 1.0 - v).collect());
                let m = soft(
                    &shift(&inv, -r.cos() * distance, r.sin() * distance),
                    *size,
                    *choke,
                );
                let alpha: Vec<f32> = m.data().iter().zip(&ad).map(|(g, s)| g * s).collect();
                let mut l = Image::new(w, h);
                paint(&mut l, &alpha, *color, *opacity, BlendMode::Normal, None);
                composite_into_keep_alpha(&mut interior, &l, *mode, &ad);
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
                // height field = distance-to-edge, smoothed; shade with a directional light
                let inside: Vec<bool> = ad.iter().map(|v| *v < 0.5).collect();
                let dist = crate::filter::distance_transform(w as usize, h as usize, &inside);
                let hf = Mask::from_vec(
                    w,
                    h,
                    dist.iter().map(|d| (d / size.max(1.0)).min(1.0)).collect(),
                );
                let hf = gaussian_blur_mask(&hf, 1.0 + soften);
                let (az, al) = (angle.to_radians(), altitude.to_radians());
                let l = [az.cos() * al.cos(), -az.sin() * al.cos(), al.sin()];
                let flat = l[2];
                let wi = w as i32;
                let mut hi_a = vec![0.0f32; ad.len()];
                let mut sh_a = vec![0.0f32; ad.len()];
                hi_a.par_iter_mut()
                    .zip(sh_a.par_iter_mut())
                    .enumerate()
                    .for_each(|(i, (ha, sa))| {
                        if ad[i] <= 0.0 {
                            return;
                        }
                        let (x, y) = (i as i32 % wi, i as i32 / wi);
                        let gx = (hf.get_i(x + 1, y) - hf.get_i(x - 1, y)) * size * depth;
                        let gy = (hf.get_i(x, y + 1) - hf.get_i(x, y - 1)) * size * depth;
                        let n = [-gx, -gy, 2.0];
                        let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                        let shade = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]) / nl;
                        let d = shade - flat;
                        if d > 0.0 {
                            *ha = (d * 2.0).min(1.0) * ad[i];
                        } else {
                            *sa = (-d * 2.0).min(1.0) * ad[i];
                        }
                    });
                let mut lh = Image::new(w, h);
                paint(
                    &mut lh,
                    &hi_a,
                    *highlight,
                    *highlight_opacity,
                    BlendMode::Normal,
                    None,
                );
                composite_into_keep_alpha(&mut interior, &lh, BlendMode::Screen, &ad);
                let mut ls = Image::new(w, h);
                paint(
                    &mut ls,
                    &sh_a,
                    *shadow,
                    *shadow_opacity,
                    BlendMode::Normal,
                    None,
                );
                composite_into_keep_alpha(&mut interior, &ls, BlendMode::Multiply, &ad);
            }
            _ => {}
        }
    }
    let _ = &mut shape_layer;
    composite_into(&mut out, &interior, BlendMode::Normal, 1.0);

    // 3. stroke on top
    for e in effects.iter().filter(|e| e.enabled()) {
        if let Effect::Stroke {
            color,
            size,
            position,
            opacity,
            mode,
            ..
        } = e
        {
            let ring: Vec<f32> = match position {
                StrokeLocation::Outside => morph_mask(&a, *size)
                    .data()
                    .iter()
                    .zip(&ad)
                    .map(|(o, s)| (o - s).max(0.0))
                    .collect(),
                StrokeLocation::Inside => {
                    let inner = morph_mask(&a, -size);
                    ad.iter()
                        .zip(inner.data())
                        .map(|(s, i)| (s - i).max(0.0))
                        .collect()
                }
                StrokeLocation::Center => {
                    let o = morph_mask(&a, size / 2.0);
                    let i = morph_mask(&a, -size / 2.0);
                    o.data()
                        .iter()
                        .zip(i.data())
                        .map(|(o, i)| (o - i).max(0.0))
                        .collect()
                }
            };
            let mut l = Image::new(w, h);
            paint(&mut l, &ring, *color, *opacity, BlendMode::Normal, None);
            composite_into(&mut out, &l, *mode, 1.0);
        }
    }
    out
}

fn composite_into(dst: &mut Image, src: &Image, mode: BlendMode, opacity: f32) {
    let w = dst.width as usize;
    let sp = src.pixels();
    dst.pixels_mut()
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, p)| {
            let s = sp[i];
            if s[3] > 0.0 {
                *p = composite_px(*p, s, mode, opacity, (i % w) as u32, (i / w) as u32);
            }
        });
}

/// Composite but keep the destination alpha (effects clipped to the layer shape). Where the
/// destination is transparent because Fill = 0, the effect still shows using the shape alpha.
fn composite_into_keep_alpha(dst: &mut Image, src: &Image, mode: BlendMode, shape: &[f32]) {
    let w = dst.width as usize;
    let sp = src.pixels();
    dst.pixels_mut()
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, p)| {
            let s = sp[i];
            if s[3] <= 0.0 {
                return;
            }
            let target_alpha = p[3].max(s[3]).min(shape[i].max(p[3]));
            *p = composite_px(*p, s, mode, 1.0, (i % w) as u32, (i / w) as u32);
            p[3] = target_alpha;
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_effects_render() {
        let mut img = Image::new(64, 64);
        for y in 16..48 {
            for x in 16..48 {
                img.set(x, y, [0.2, 0.5, 0.8, 1.0]);
            }
        }
        let out = render(&img, &Effect::catalog(), 1.0);
        // drop shadow / glow / stroke extend outside the shape
        assert!(out.get(50, 50)[3] > 0.0);
        for p in out.pixels() {
            for v in p {
                assert!(v.is_finite() && *v >= -1e-4 && *v <= 1.0001);
            }
        }
    }
}
