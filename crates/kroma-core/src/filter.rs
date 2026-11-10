//! Filter gallery: blur, sharpen, noise, stylize, distort, pixelate, render, other.
//! All filters are parameterised + serializable (usable in Actions and the CLI).

use crate::adjust::{apply_fn, Histogram};
use crate::buffer::{Image, Mask};
use crate::color::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoiseKind {
    #[default]
    Gaussian,
    Uniform,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RadialKind {
    #[default]
    Zoom,
    Spin,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Filter {
    // Blur
    GaussianBlur {
        radius: f32,
    },
    BoxBlur {
        radius: u32,
    },
    MotionBlur {
        angle: f32,
        distance: f32,
    },
    RadialBlur {
        amount: f32,
        kind: RadialKind,
        cx: f32,
        cy: f32,
    },
    SurfaceBlur {
        radius: u32,
        threshold: f32,
    },
    TiltShift {
        focus_y: f32,
        band: f32,
        blur: f32,
    },
    // Sharpen
    UnsharpMask {
        amount: f32,
        radius: f32,
        threshold: f32,
    },
    Sharpen,
    HighPass {
        radius: f32,
    },
    Clarity {
        amount: f32,
    },
    // Noise
    AddNoise {
        amount: f32,
        kind: NoiseKind,
        monochrome: bool,
        seed: u32,
    },
    Median {
        radius: u32,
    },
    Despeckle,
    ReduceNoise {
        strength: f32,
    },
    // Stylize
    FindEdges,
    Emboss {
        angle: f32,
        height: f32,
        amount: f32,
    },
    OilPaint {
        radius: u32,
    },
    GlowingEdges {
        width: f32,
        brightness: f32,
    },
    // Distort
    Twirl {
        angle: f32,
    },
    Pinch {
        amount: f32,
    },
    Spherize {
        amount: f32,
    },
    Wave {
        amplitude: f32,
        wavelength: f32,
        horizontal: bool,
        vertical: bool,
    },
    Ripple {
        amount: f32,
        size: f32,
    },
    PolarCoordinates {
        to_polar: bool,
    },
    // Pixelate
    Mosaic {
        cell: u32,
    },
    Crystallize {
        cell: u32,
        seed: u32,
    },
    ColorHalftone {
        max_radius: f32,
    },
    // Render
    Clouds {
        scale: f32,
        seed: u32,
    },
    Vignette {
        amount: f32,
        midpoint: f32,
    },
    LensFlare {
        cx: f32,
        cy: f32,
        brightness: f32,
    },
    // Other
    Offset {
        dx: i32,
        dy: i32,
        wrap: bool,
    },
    Maximum {
        radius: u32,
    },
    Minimum {
        radius: u32,
    },
    ChromaticAberration {
        amount: f32,
    },
    Equalize,
    ShadowsHighlights {
        shadows: f32,
        highlights: f32,
        radius: f32,
    },
    Dehaze {
        amount: f32,
    },
}

/// Context the filters may need (foreground/background colors for Clouds etc).
#[derive(Clone, Copy, Debug)]
pub struct FilterCtx {
    pub fg: Color,
    pub bg: Color,
}

impl Default for FilterCtx {
    fn default() -> Self {
        FilterCtx {
            fg: Color::BLACK,
            bg: Color::WHITE,
        }
    }
}

impl Filter {
    pub fn name(&self) -> &'static str {
        use Filter::*;
        match self {
            GaussianBlur { .. } => "Gaussian Blur",
            BoxBlur { .. } => "Box Blur",
            MotionBlur { .. } => "Motion Blur",
            RadialBlur { .. } => "Radial Blur",
            SurfaceBlur { .. } => "Surface Blur",
            TiltShift { .. } => "Tilt-Shift",
            UnsharpMask { .. } => "Unsharp Mask",
            Sharpen => "Sharpen",
            HighPass { .. } => "High Pass",
            Clarity { .. } => "Clarity",
            AddNoise { .. } => "Add Noise",
            Median { .. } => "Median",
            Despeckle => "Despeckle",
            ReduceNoise { .. } => "Reduce Noise",
            FindEdges => "Find Edges",
            Emboss { .. } => "Emboss",
            OilPaint { .. } => "Oil Paint",
            GlowingEdges { .. } => "Glowing Edges",
            Twirl { .. } => "Twirl",
            Pinch { .. } => "Pinch",
            Spherize { .. } => "Spherize",
            Wave { .. } => "Wave",
            Ripple { .. } => "Ripple",
            PolarCoordinates { .. } => "Polar Coordinates",
            Mosaic { .. } => "Mosaic",
            Crystallize { .. } => "Crystallize",
            ColorHalftone { .. } => "Color Halftone",
            Clouds { .. } => "Clouds",
            Vignette { .. } => "Vignette",
            LensFlare { .. } => "Lens Flare",
            Offset { .. } => "Offset",
            Maximum { .. } => "Maximum",
            Minimum { .. } => "Minimum",
            ChromaticAberration { .. } => "Chromatic Aberration",
            Equalize => "Equalize",
            ShadowsHighlights { .. } => "Shadows/Highlights",
            Dehaze { .. } => "Dehaze",
        }
    }

    /// Photoshop-style Filter menu: (submenu, default filters).
    pub fn menu() -> Vec<(&'static str, Vec<Filter>)> {
        use Filter::*;
        vec![
            (
                "Blur",
                vec![
                    GaussianBlur { radius: 4.0 },
                    BoxBlur { radius: 4 },
                    MotionBlur {
                        angle: 0.0,
                        distance: 20.0,
                    },
                    RadialBlur {
                        amount: 0.15,
                        kind: RadialKind::Zoom,
                        cx: 0.5,
                        cy: 0.5,
                    },
                    SurfaceBlur {
                        radius: 5,
                        threshold: 0.1,
                    },
                    TiltShift {
                        focus_y: 0.5,
                        band: 0.15,
                        blur: 8.0,
                    },
                ],
            ),
            (
                "Sharpen",
                vec![
                    UnsharpMask {
                        amount: 1.0,
                        radius: 1.5,
                        threshold: 0.0,
                    },
                    Sharpen,
                    HighPass { radius: 3.0 },
                    Clarity { amount: 0.5 },
                ],
            ),
            (
                "Noise",
                vec![
                    AddNoise {
                        amount: 0.1,
                        kind: NoiseKind::Gaussian,
                        monochrome: false,
                        seed: 1,
                    },
                    Median { radius: 2 },
                    Despeckle,
                    ReduceNoise { strength: 0.5 },
                ],
            ),
            (
                "Stylize",
                vec![
                    FindEdges,
                    Emboss {
                        angle: 135.0,
                        height: 2.0,
                        amount: 1.0,
                    },
                    OilPaint { radius: 4 },
                    GlowingEdges {
                        width: 1.5,
                        brightness: 2.0,
                    },
                ],
            ),
            (
                "Distort",
                vec![
                    Twirl { angle: 90.0 },
                    Pinch { amount: 0.5 },
                    Spherize { amount: 0.6 },
                    Wave {
                        amplitude: 10.0,
                        wavelength: 60.0,
                        horizontal: true,
                        vertical: false,
                    },
                    Ripple {
                        amount: 6.0,
                        size: 18.0,
                    },
                    PolarCoordinates { to_polar: true },
                ],
            ),
            (
                "Pixelate",
                vec![
                    Mosaic { cell: 12 },
                    Crystallize { cell: 16, seed: 7 },
                    ColorHalftone { max_radius: 6.0 },
                ],
            ),
            (
                "Render",
                vec![
                    Clouds {
                        scale: 120.0,
                        seed: 3,
                    },
                    Vignette {
                        amount: 0.6,
                        midpoint: 0.5,
                    },
                    LensFlare {
                        cx: 0.3,
                        cy: 0.3,
                        brightness: 1.0,
                    },
                ],
            ),
            (
                "Other",
                vec![
                    Offset {
                        dx: 50,
                        dy: 50,
                        wrap: true,
                    },
                    Maximum { radius: 2 },
                    Minimum { radius: 2 },
                    ChromaticAberration { amount: 4.0 },
                    Equalize,
                    ShadowsHighlights {
                        shadows: 0.35,
                        highlights: 0.0,
                        radius: 30.0,
                    },
                    Dehaze { amount: 0.4 },
                ],
            ),
        ]
    }

    pub fn has_params(&self) -> bool {
        !matches!(
            self,
            Filter::Sharpen | Filter::Despeckle | Filter::FindEdges | Filter::Equalize
        )
    }

    /// Apply the filter to `img`, restricted to `sel`.
    pub fn apply(&self, img: &mut Image, sel: Option<&Mask>, ctx: &FilterCtx) {
        let out = self.render(img, ctx);
        blend_by_mask(img, &out, sel);
    }

    /// Produce the fully filtered image (unmasked).
    pub fn render(&self, img: &Image, ctx: &FilterCtx) -> Image {
        use Filter::*;
        let (w, h) = (img.width as f32, img.height as f32);
        match self.clone() {
            GaussianBlur { radius } => gaussian_blur(img, radius),
            BoxBlur { radius } => box_blur_premul(img, radius as usize, 1),
            MotionBlur { angle, distance } => motion_blur(img, angle, distance),
            RadialBlur {
                amount,
                kind,
                cx,
                cy,
            } => radial_blur(img, amount, kind, cx * w, cy * h),
            SurfaceBlur { radius, threshold } => surface_blur(img, radius, threshold),
            TiltShift {
                focus_y,
                band,
                blur,
            } => {
                let blurred = gaussian_blur(img, blur);
                let mut out = img.clone();
                let hh = img.height as f32;
                let ww = img.width as usize;
                let bp = blurred.pixels();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        let d = ((y as f32 / hh) - focus_y).abs();
                        let t = smoothstep(band, band * 2.5 + 0.05, d);
                        for (x, p) in row.iter_mut().enumerate() {
                            let b = bp[y * ww + x];
                            for c in 0..4 {
                                p[c] += (b[c] - p[c]) * t;
                            }
                        }
                    });
                out
            }
            UnsharpMask {
                amount,
                radius,
                threshold,
            } => unsharp(img, amount, radius, threshold),
            Sharpen => convolve3(
                img,
                [[0.0, -0.5, 0.0], [-0.5, 3.0, -0.5], [0.0, -0.5, 0.0]],
                0.0,
            ),
            HighPass { radius } => {
                let b = gaussian_blur(img, radius);
                zip_map(img, &b, |p, q| {
                    [
                        clamp01(p[0] - q[0] + 0.5),
                        clamp01(p[1] - q[1] + 0.5),
                        clamp01(p[2] - q[2] + 0.5),
                        p[3],
                    ]
                })
            }
            Clarity { amount } => {
                // local contrast on midtones using a wide blur
                let b = gaussian_blur(img, (img.width.max(img.height) as f32 / 60.0).max(8.0));
                zip_map(img, &b, |p, q| {
                    let l = luma709([p[0], p[1], p[2]]);
                    let mid = 1.0 - (2.0 * l - 1.0).powi(2);
                    let k = amount * mid;
                    [
                        clamp01(p[0] + (p[0] - q[0]) * k),
                        clamp01(p[1] + (p[1] - q[1]) * k),
                        clamp01(p[2] + (p[2] - q[2]) * k),
                        p[3],
                    ]
                })
            }
            AddNoise {
                amount,
                kind,
                monochrome,
                seed,
            } => {
                let mut out = img.clone();
                let ww = img.width as usize;
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let base = (y * ww + x) as u32 ^ seed.wrapping_mul(0x9E37_79B9);
                            let n = |k: u32| match kind {
                                NoiseKind::Uniform => rand01(base, k) * 2.0 - 1.0,
                                NoiseKind::Gaussian => gauss(base, k) * 0.5,
                            };
                            if monochrome {
                                let v = n(0) * amount;
                                for c in 0..3 {
                                    p[c] = clamp01(p[c] + v);
                                }
                            } else {
                                for c in 0..3 {
                                    p[c] = clamp01(p[c] + n(c as u32) * amount);
                                }
                            }
                        }
                    });
                out
            }
            Median { radius } => median(img, radius),
            Despeckle => {
                // edge-preserving: median only where local variance is low
                let m = median(img, 1);
                let edges = sobel_mag(img);
                let mut out = img.clone();
                let ep = edges.data();
                let mp = m.pixels();
                out.pixels_mut()
                    .par_iter_mut()
                    .enumerate()
                    .for_each(|(i, p)| {
                        let t = 1.0 - smoothstep(0.1, 0.3, ep[i]);
                        for c in 0..3 {
                            p[c] += (mp[i][c] - p[c]) * t;
                        }
                    });
                out
            }
            ReduceNoise { strength } => {
                surface_blur(img, (2.0 + strength * 4.0) as u32, 0.03 + strength * 0.12)
            }
            FindEdges => {
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let (x, y) = (x as i32, y as i32);
                            for c in 0..3 {
                                let s = |dx: i32, dy: i32| img.get_clamped(x + dx, y + dy)[c];
                                let gx = -s(-1, -1) - 2.0 * s(-1, 0) - s(-1, 1)
                                    + s(1, -1)
                                    + 2.0 * s(1, 0)
                                    + s(1, 1);
                                let gy = -s(-1, -1) - 2.0 * s(0, -1) - s(1, -1)
                                    + s(-1, 1)
                                    + 2.0 * s(0, 1)
                                    + s(1, 1);
                                p[c] = clamp01(1.0 - (gx * gx + gy * gy).sqrt() * 0.5);
                            }
                        }
                    });
                out
            }
            Emboss {
                angle,
                height,
                amount,
            } => {
                let (dx, dy) = (
                    angle.to_radians().cos() * height,
                    -angle.to_radians().sin() * height,
                );
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let a = img.sample_bilinear(x as f32 + 0.5 + dx, y as f32 + 0.5 + dy);
                            let b = img.sample_bilinear(x as f32 + 0.5 - dx, y as f32 + 0.5 - dy);
                            let d = luma709([a[0], a[1], a[2]]) - luma709([b[0], b[1], b[2]]);
                            let v = clamp01(0.5 + d * amount * 2.0);
                            p[0] = v;
                            p[1] = v;
                            p[2] = v;
                        }
                    });
                out
            }
            OilPaint { radius } => kuwahara(img, radius.max(1) as i32),
            GlowingEdges { width, brightness } => {
                let b = gaussian_blur(img, width * 0.5);
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let (x, y) = (x as i32, y as i32);
                            for c in 0..3 {
                                let s = |dx: i32, dy: i32| b.get_clamped(x + dx, y + dy)[c];
                                let gx = s(1, 0) - s(-1, 0);
                                let gy = s(0, 1) - s(0, -1);
                                p[c] = clamp01((gx * gx + gy * gy).sqrt() * brightness * 2.0);
                            }
                        }
                    });
                out
            }
            Twirl { angle } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let r = cx.min(cy);
                let a = angle.to_radians();
                remap(img, move |x, y| {
                    let (dx, dy) = (x - cx, y - cy);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d >= r {
                        return (x, y);
                    }
                    let t = a * (1.0 - d / r).powi(2);
                    let (s, c) = t.sin_cos();
                    (cx + dx * c - dy * s, cy + dx * s + dy * c)
                })
            }
            Pinch { amount } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let r = cx.min(cy);
                remap(img, move |x, y| {
                    let (dx, dy) = (x - cx, y - cy);
                    let d = (dx * dx + dy * dy).sqrt() / r;
                    if d >= 1.0 || d == 0.0 {
                        return (x, y);
                    }
                    let f =
                        (d.powf(1.0 - amount.clamp(-0.95, 0.95) * 0.7 * (1.0 - d)) / d).max(0.01);
                    (cx + dx * f, cy + dy * f)
                })
            }
            Spherize { amount } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let r = cx.min(cy);
                remap(img, move |x, y| {
                    let (dx, dy) = (x - cx, y - cy);
                    let d = (dx * dx + dy * dy).sqrt() / r;
                    if d >= 1.0 || d == 0.0 {
                        return (x, y);
                    }
                    let sph = (d.asin() * 2.0 / PI) / d;
                    let f = 1.0 + (sph - 1.0) * amount;
                    (cx + dx * f, cy + dy * f)
                })
            }
            Wave {
                amplitude,
                wavelength,
                horizontal,
                vertical,
            } => remap(img, move |x, y| {
                let k = TAU / wavelength.max(1.0);
                let nx = if horizontal {
                    x + amplitude * (y * k).sin()
                } else {
                    x
                };
                let ny = if vertical {
                    y + amplitude * (x * k).sin()
                } else {
                    y
                };
                (nx, ny)
            }),
            Ripple { amount, size } => remap(img, move |x, y| {
                let k = TAU / size.max(1.0);
                (
                    x + amount * (y * k).sin() * (x * k * 0.37).cos(),
                    y + amount * (x * k).sin() * (y * k * 0.41).cos(),
                )
            }),
            PolarCoordinates { to_polar } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let rmax = cx.min(cy);
                if to_polar {
                    remap(img, move |x, y| {
                        let (dx, dy) = (x - cx, y - cy);
                        let r = (dx * dx + dy * dy).sqrt() / rmax;
                        let a = (dx.atan2(-dy) + PI) / TAU;
                        (a * w, r * h)
                    })
                } else {
                    remap(img, move |x, y| {
                        let a = x / w * TAU - PI;
                        let r = y / h * rmax;
                        (cx + r * a.sin(), cy - r * a.cos())
                    })
                }
            }
            Mosaic { cell } => {
                let c = cell.max(1) as usize;
                let ww = img.width as usize;
                let hh = img.height as usize;
                let mut out = img.clone();
                let cells: Vec<(usize, usize, Px)> = (0..hh.div_ceil(c))
                    .into_par_iter()
                    .flat_map_iter(|cy| (0..ww.div_ceil(c)).map(move |cx| (cx, cy)))
                    .map(|(cx, cy)| {
                        let mut acc = [0.0f32; 4];
                        let mut n = 0.0;
                        for y in cy * c..((cy + 1) * c).min(hh) {
                            for x in cx * c..((cx + 1) * c).min(ww) {
                                let p = img.get(x as u32, y as u32);
                                for k in 0..3 {
                                    acc[k] += p[k] * p[3];
                                }
                                acc[3] += p[3];
                                n += 1.0;
                            }
                        }
                        let a = acc[3].max(1e-6);
                        (cx, cy, [acc[0] / a, acc[1] / a, acc[2] / a, acc[3] / n])
                    })
                    .collect();
                let px = out.pixels_mut();
                for (cx, cy, p) in cells {
                    for y in cy * c..((cy + 1) * c).min(hh) {
                        for x in cx * c..((cx + 1) * c).min(ww) {
                            px[y * ww + x] = p;
                        }
                    }
                }
                out
            }
            Crystallize { cell, seed } => crystallize(img, cell.max(2), seed),
            ColorHalftone { max_radius } => halftone(img, max_radius.max(2.0)),
            Clouds { scale, seed } => {
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let t = fbm(x as f32 / scale, y as f32 / scale, seed, 6);
                            let c = ctx.fg.lerp(ctx.bg, t);
                            *p = [c.r, c.g, c.b, 1.0];
                        }
                    });
                out
            }
            Vignette { amount, midpoint } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let dx = (x as f32 - cx) / cx;
                            let dy = (y as f32 - cy) / cy;
                            let d = (dx * dx + dy * dy).sqrt() / std::f32::consts::SQRT_2;
                            let v = smoothstep(midpoint * 0.8, 1.05, d);
                            let k = if amount >= 0.0 { 1.0 - v * amount } else { 1.0 };
                            for c in 0..3 {
                                p[c] = if amount >= 0.0 {
                                    p[c] * k
                                } else {
                                    p[c] + (1.0 - p[c]) * v * -amount
                                };
                            }
                        }
                    });
                out
            }
            LensFlare { cx, cy, brightness } => {
                let (fx, fy) = (cx * w, cy * h);
                let (ox, oy) = (w - fx, h - fy);
                let diag = (w * w + h * h).sqrt();
                let ww = img.width as usize;
                let mut out = img.clone();
                let ghosts = [
                    (0.3f32, 0.04f32, [0.4, 0.8, 1.0]),
                    (0.55, 0.025, [1.0, 0.6, 0.3]),
                    (0.8, 0.06, [0.5, 1.0, 0.6]),
                    (1.1, 0.03, [0.9, 0.5, 1.0]),
                ];
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let (x, y) = (x as f32, y as f32);
                            let d = ((x - fx).powi(2) + (y - fy).powi(2)).sqrt() / diag;
                            let mut add = [0.0f32; 3];
                            let core = (-d * 40.0).exp() * 1.5 + (-d * 8.0).exp() * 0.35;
                            let ang = (y - fy).atan2(x - fx);
                            let rays =
                                ((ang * 12.0).sin().abs().powf(40.0)) * (-d * 6.0).exp() * 0.4;
                            for c in 0..3 {
                                add[c] += core + rays;
                            }
                            add[2] += (-d * 4.0).exp() * 0.1;
                            for (t, size, col) in ghosts {
                                let gx = fx + (ox - fx) * t;
                                let gy = fy + (oy - fy) * t;
                                let gd = ((x - gx).powi(2) + (y - gy).powi(2)).sqrt() / diag;
                                let ring = smoothstep(size, size * 0.7, gd) * 0.25;
                                for c in 0..3 {
                                    add[c] += ring * col[c];
                                }
                            }
                            for c in 0..3 {
                                // screen blend
                                let a = clamp01(add[c] * brightness);
                                p[c] = p[c] + a - p[c] * a;
                            }
                        }
                    });
                out
            }
            Offset { dx, dy, wrap } => {
                let (iw, ih) = (img.width as i32, img.height as i32);
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let sx = x as i32 - dx;
                            let sy = y as i32 - dy;
                            *p = if wrap {
                                img.get(sx.rem_euclid(iw) as u32, sy.rem_euclid(ih) as u32)
                            } else {
                                img.get_i(sx, sy)
                            };
                        }
                    });
                out
            }
            Maximum { radius } => morph_rgb(img, radius as i32, true),
            Minimum { radius } => morph_rgb(img, radius as i32, false),
            ChromaticAberration { amount } => {
                let (cx, cy) = (w / 2.0, h / 2.0);
                let ww = img.width as usize;
                let mut out = img.clone();
                out.pixels_mut()
                    .par_chunks_mut(ww)
                    .enumerate()
                    .for_each(|(y, row)| {
                        for (x, p) in row.iter_mut().enumerate() {
                            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                            let (dx, dy) = ((fx - cx) / cx, (fy - cy) / cy);
                            let r = img.sample_bilinear(fx + dx * amount, fy + dy * amount);
                            let b = img.sample_bilinear(fx - dx * amount, fy - dy * amount);
                            p[0] = r[0];
                            p[2] = b[2];
                        }
                    });
                out
            }
            Equalize => {
                let hgm = Histogram::of(img);
                let mut cdf = [0.0f32; 256];
                let mut acc = 0u64;
                for i in 0..256 {
                    acc += hgm.l[i] as u64;
                    cdf[i] = acc as f32 / hgm.count.max(1) as f32;
                }
                let mut out = img.clone();
                apply_fn(&mut out, None, &move |p: Px| {
                    let l = luma709([p[0], p[1], p[2]]).max(1e-4);
                    let nl = cdf[(clamp01(l) * 255.0) as usize];
                    let k = nl / l;
                    [
                        clamp01(p[0] * k),
                        clamp01(p[1] * k),
                        clamp01(p[2] * k),
                        p[3],
                    ]
                });
                out
            }
            ShadowsHighlights {
                shadows,
                highlights,
                radius,
            } => {
                let lum_img = Image::from_pixels(
                    img.width,
                    img.height,
                    img.pixels()
                        .iter()
                        .map(|p| {
                            let l = luma709([p[0], p[1], p[2]]);
                            [l, l, l, 1.0]
                        })
                        .collect(),
                );
                let bl = gaussian_blur(&lum_img, radius);
                zip_map(img, &bl, |p, q| {
                    let l = q[0];
                    let sh = (1.0 - l / 0.5).max(0.0).powi(2) * shadows;
                    let hi = ((l - 0.5) / 0.5).max(0.0).powi(2) * highlights;
                    let mut o = [0.0; 4];
                    for c in 0..3 {
                        let v = p[c];
                        let v = v + (1.0 - v) * v.sqrt() * sh * 1.5;
                        o[c] = clamp01(v - v * v * hi * 0.9);
                    }
                    o[3] = p[3];
                    o
                })
            }
            Dehaze { amount } => {
                // dark-channel-prior style: estimate haze from local min, subtract it
                let dark = Image::from_pixels(
                    img.width,
                    img.height,
                    img.pixels()
                        .iter()
                        .map(|p| {
                            let m = p[0].min(p[1]).min(p[2]);
                            [m, m, m, 1.0]
                        })
                        .collect(),
                );
                let dark = gaussian_blur(&morph_rgb(&dark, 7, false), 15.0);
                let atmo = Histogram::percentile(
                    &Histogram::of(img).l,
                    (img.width * img.height) as u64,
                    0.001,
                    true,
                )
                .max(0.6);
                zip_map(img, &dark, |p, d| {
                    let t = (1.0 - amount * 0.95 * d[0] / atmo).max(0.1);
                    let mut o = [0.0; 4];
                    for c in 0..3 {
                        o[c] = clamp01((p[c] - atmo) / t + atmo);
                    }
                    o[3] = p[3];
                    o
                })
            }
        }
    }
}

// ---------------------------------------------------------------- helpers

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = clamp01((x - e0) / (e1 - e0));
    t * t * (3.0 - 2.0 * t)
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}
pub fn rand01(seed: u32, k: u32) -> f32 {
    (hash(
        seed.wrapping_mul(3)
            .wrapping_add(k.wrapping_mul(0x632B_E5AB)),
    ) & 0xFF_FFFF) as f32
        / 16_777_215.0
}
fn gauss(seed: u32, k: u32) -> f32 {
    let u1 = rand01(seed, k * 2 + 11).max(1e-7);
    let u2 = rand01(seed, k * 2 + 12);
    (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (tx, ty) = (x - x.floor(), y - y.floor());
    let r = |i: i32, j: i32| {
        rand01(
            hash((i as u32).wrapping_mul(1_619) ^ (j as u32).wrapping_mul(31_337) ^ seed),
            0,
        )
    };
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy) = (s(tx), s(ty));
    let a = r(xi, yi) + (r(xi + 1, yi) - r(xi, yi)) * sx;
    let b = r(xi, yi + 1) + (r(xi + 1, yi + 1) - r(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

pub fn fbm(x: f32, y: f32, seed: u32, oct: u32) -> f32 {
    let (mut f, mut amp, mut sum, mut norm) = (1.0, 0.5, 0.0, 0.0);
    for o in 0..oct {
        sum += value_noise(x * f, y * f, seed.wrapping_add(o * 977)) * amp;
        norm += amp;
        f *= 2.0;
        amp *= 0.5;
    }
    sum / norm
}

pub fn zip_map(a: &Image, b: &Image, f: impl Fn(Px, Px) -> Px + Sync) -> Image {
    let data = a
        .pixels()
        .par_iter()
        .zip(b.pixels().par_iter())
        .map(|(p, q)| f(*p, *q))
        .collect();
    Image::from_pixels(a.width, a.height, data)
}

/// Replace `img` with `out` weighted by selection (or entirely).
pub fn blend_by_mask(img: &mut Image, out: &Image, sel: Option<&Mask>) {
    match sel {
        None => *img = out.clone(),
        Some(s) => {
            let sd = s.data();
            let op = out.pixels();
            img.pixels_mut()
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, p)| {
                    let m = sd[i];
                    if m <= 0.0 {
                        return;
                    }
                    let q = op[i];
                    for c in 0..4 {
                        p[c] += (q[c] - p[c]) * m;
                    }
                });
        }
    }
}

fn premultiply(img: &Image) -> Vec<[f32; 4]> {
    img.pixels()
        .par_iter()
        .map(|p| [p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
        .collect()
}
fn unpremultiply(w: u32, h: u32, v: Vec<[f32; 4]>) -> Image {
    let data = v
        .into_par_iter()
        .map(|p| {
            if p[3] > 1e-6 {
                [
                    clamp01(p[0] / p[3]),
                    clamp01(p[1] / p[3]),
                    clamp01(p[2] / p[3]),
                    clamp01(p[3]),
                ]
            } else {
                [0.0; 4]
            }
        })
        .collect();
    Image::from_pixels(w, h, data)
}

/// Generic sliding-window box blur on any `[f32; N]`-like buffer via closures over channels.
fn box_pass<const N: usize>(
    src: &[[f32; N]],
    w: usize,
    h: usize,
    r: usize,
    horizontal: bool,
) -> Vec<[f32; N]> {
    let mut dst = vec![[0.0f32; N]; w * h];
    if r == 0 {
        dst.copy_from_slice(src);
        return dst;
    }
    let norm = 1.0 / (2 * r + 1) as f32;
    if horizontal {
        dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            let s = &src[y * w..(y + 1) * w];
            let get = |i: isize| s[i.clamp(0, w as isize - 1) as usize];
            let mut acc = [0.0f32; N];
            for i in -(r as isize)..=(r as isize) {
                let p = get(i);
                for c in 0..N {
                    acc[c] += p[c];
                }
            }
            for x in 0..w {
                for c in 0..N {
                    row[x][c] = acc[c] * norm;
                }
                let add = get(x as isize + r as isize + 1);
                let sub = get(x as isize - r as isize);
                for c in 0..N {
                    acc[c] += add[c] - sub[c];
                }
            }
        });
    } else {
        // process in column strips for cache friendliness: transpose via rows of output
        let cols: Vec<Vec<[f32; N]>> = (0..w)
            .into_par_iter()
            .map(|x| {
                let get = |i: isize| src[i.clamp(0, h as isize - 1) as usize * w + x];
                let mut acc = [0.0f32; N];
                for i in -(r as isize)..=(r as isize) {
                    let p = get(i);
                    for c in 0..N {
                        acc[c] += p[c];
                    }
                }
                let mut col = vec![[0.0f32; N]; h];
                for y in 0..h {
                    for c in 0..N {
                        col[y][c] = acc[c] * norm;
                    }
                    let add = get(y as isize + r as isize + 1);
                    let sub = get(y as isize - r as isize);
                    for c in 0..N {
                        acc[c] += add[c] - sub[c];
                    }
                }
                col
            })
            .collect();
        dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for x in 0..w {
                row[x] = cols[x][y];
            }
        });
    }
    dst
}

pub fn box_blur_premul(img: &Image, r: usize, passes: usize) -> Image {
    let (w, h) = (img.width as usize, img.height as usize);
    let mut v = premultiply(img);
    for _ in 0..passes {
        v = box_pass(&v, w, h, r, true);
        v = box_pass(&v, w, h, r, false);
    }
    unpremultiply(img.width, img.height, v)
}

/// Box radii approximating a gaussian with three passes.
fn gauss_boxes(sigma: f32) -> [usize; 3] {
    let n = 3.0;
    let wi = ((12.0 * sigma * sigma / n) + 1.0).sqrt().floor();
    let wl = if (wi as i32) % 2 == 0 { wi - 1.0 } else { wi };
    let wu = wl + 2.0;
    let m =
        ((12.0 * sigma * sigma - n * wl * wl - 4.0 * n * wl - 3.0 * n) / (-4.0 * wl - 4.0)).round();
    let mut out = [0usize; 3];
    for (i, o) in out.iter_mut().enumerate() {
        let size = if (i as f32) < m { wl } else { wu };
        *o = ((size - 1.0) / 2.0).max(0.0) as usize;
    }
    out
}

/// Gaussian blur (radius ~ 2 sigma, like Photoshop's slider).
pub fn gaussian_blur(img: &Image, radius: f32) -> Image {
    if radius < 0.3 {
        return img.clone();
    }
    let sigma = radius / 2.0 + 0.3;
    let (w, h) = (img.width as usize, img.height as usize);
    let mut v = premultiply(img);
    for r in gauss_boxes(sigma) {
        v = box_pass(&v, w, h, r, true);
        v = box_pass(&v, w, h, r, false);
    }
    unpremultiply(img.width, img.height, v)
}

pub fn gaussian_blur_mask(m: &Mask, radius: f32) -> Mask {
    if radius < 0.3 {
        return m.clone();
    }
    let sigma = radius / 2.0 + 0.3;
    let (w, h) = (m.width as usize, m.height as usize);
    let mut v: Vec<[f32; 1]> = m.data().iter().map(|x| [*x]).collect();
    for r in gauss_boxes(sigma) {
        v = box_pass(&v, w, h, r, true);
        v = box_pass(&v, w, h, r, false);
    }
    Mask::from_vec(m.width, m.height, v.into_iter().map(|x| x[0]).collect())
}

fn unsharp(img: &Image, amount: f32, radius: f32, threshold: f32) -> Image {
    let b = gaussian_blur(img, radius);
    zip_map(img, &b, |p, q| {
        let mut o = p;
        for c in 0..3 {
            let d = p[c] - q[c];
            if d.abs() >= threshold {
                o[c] = clamp01(p[c] + d * amount);
            }
        }
        o
    })
}

fn convolve3(img: &Image, k: [[f32; 3]; 3], bias: f32) -> Image {
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                for c in 0..3 {
                    let mut s = bias;
                    for (j, kr) in k.iter().enumerate() {
                        for (i, kv) in kr.iter().enumerate() {
                            s += img.get_clamped(x as i32 + i as i32 - 1, y as i32 + j as i32 - 1)
                                [c]
                                * kv;
                        }
                    }
                    p[c] = clamp01(s);
                }
            }
        });
    out
}

fn sobel_mag(img: &Image) -> Mask {
    let (w, h) = (img.width as usize, img.height as usize);
    let l: Vec<f32> = img
        .pixels()
        .iter()
        .map(|p| luma709([p[0], p[1], p[2]]))
        .collect();
    let g = |x: i32, y: i32| {
        l[y.clamp(0, h as i32 - 1) as usize * w + x.clamp(0, w as i32 - 1) as usize]
    };
    let data = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            let gx = -g(x - 1, y - 1) - 2.0 * g(x - 1, y) - g(x - 1, y + 1)
                + g(x + 1, y - 1)
                + 2.0 * g(x + 1, y)
                + g(x + 1, y + 1);
            let gy = -g(x - 1, y - 1) - 2.0 * g(x, y - 1) - g(x + 1, y - 1)
                + g(x - 1, y + 1)
                + 2.0 * g(x, y + 1)
                + g(x + 1, y + 1);
            (gx * gx + gy * gy).sqrt() * 0.25
        })
        .collect();
    Mask::from_vec(img.width, img.height, data)
}

fn remap(img: &Image, f: impl Fn(f32, f32) -> (f32, f32) + Sync) -> Image {
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let (sx, sy) = f(x as f32 + 0.5, y as f32 + 0.5);
                *p = img.sample_bilinear(sx, sy);
            }
        });
    out
}

fn motion_blur(img: &Image, angle: f32, distance: f32) -> Image {
    let n = (distance.abs().ceil() as i32).max(1);
    let (s, c) = angle.to_radians().sin_cos();
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let mut acc = [0.0f32; 4];
                for i in -n..=n {
                    let t = i as f32 * distance / (2.0 * n as f32);
                    let q = img.sample_bilinear(x as f32 + 0.5 + c * t, y as f32 + 0.5 - s * t);
                    for k in 0..3 {
                        acc[k] += q[k] * q[3];
                    }
                    acc[3] += q[3];
                }
                let cnt = (2 * n + 1) as f32;
                *p = if acc[3] > 1e-6 {
                    [
                        acc[0] / acc[3],
                        acc[1] / acc[3],
                        acc[2] / acc[3],
                        acc[3] / cnt,
                    ]
                } else {
                    [0.0; 4]
                };
            }
        });
    out
}

fn radial_blur(img: &Image, amount: f32, kind: RadialKind, cx: f32, cy: f32) -> Image {
    let steps = 24;
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut acc = [0.0f32; 4];
                for i in 0..steps {
                    let t = i as f32 / (steps - 1) as f32 - 0.5;
                    let (sx, sy) = match kind {
                        RadialKind::Zoom => {
                            let k = 1.0 + t * amount;
                            (cx + (fx - cx) * k, cy + (fy - cy) * k)
                        }
                        RadialKind::Spin => {
                            let a = t * amount;
                            let (s, c) = a.sin_cos();
                            let (dx, dy) = (fx - cx, fy - cy);
                            (cx + dx * c - dy * s, cy + dx * s + dy * c)
                        }
                    };
                    let q = img.sample_bilinear(sx, sy);
                    for k in 0..3 {
                        acc[k] += q[k] * q[3];
                    }
                    acc[3] += q[3];
                }
                *p = if acc[3] > 1e-6 {
                    [
                        acc[0] / acc[3],
                        acc[1] / acc[3],
                        acc[2] / acc[3],
                        acc[3] / steps as f32,
                    ]
                } else {
                    [0.0; 4]
                };
            }
        });
    out
}

fn surface_blur(img: &Image, radius: u32, threshold: f32) -> Image {
    let r = radius as i32;
    let ww = img.width as usize;
    let sig_s = (r as f32 / 2.0).max(0.5);
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let c0 = img.get(x as u32, y as u32);
                let mut acc = [0.0f32; 3];
                let mut wsum = 0.0;
                let mut j = -r;
                while j <= r {
                    let mut i = -r;
                    while i <= r {
                        let q = img.get_clamped(x as i32 + i, y as i32 + j);
                        let ds = ((i * i + j * j) as f32) / (2.0 * sig_s * sig_s);
                        let dc =
                            ((q[0] - c0[0]).abs() + (q[1] - c0[1]).abs() + (q[2] - c0[2]).abs())
                                / 3.0;
                        let wr = (1.0 - dc / (threshold * 2.5).max(1e-3)).max(0.0);
                        let wgt = (-ds).exp() * wr;
                        for k in 0..3 {
                            acc[k] += q[k] * wgt;
                        }
                        wsum += wgt;
                        i += if r > 6 { 2 } else { 1 };
                    }
                    j += if r > 6 { 2 } else { 1 };
                }
                if wsum > 0.0 {
                    for k in 0..3 {
                        p[k] = acc[k] / wsum;
                    }
                }
            }
        });
    out
}

fn median(img: &Image, radius: u32) -> Image {
    let r = radius as i32;
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            let mut buf: Vec<f32> = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
            for (x, p) in row.iter_mut().enumerate() {
                for c in 0..4 {
                    buf.clear();
                    for j in -r..=r {
                        for i in -r..=r {
                            buf.push(img.get_clamped(x as i32 + i, y as i32 + j)[c]);
                        }
                    }
                    let mid = buf.len() / 2;
                    buf.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap());
                    p[c] = buf[mid];
                }
            }
        });
    out
}

fn morph_rgb(img: &Image, r: i32, max: bool) -> Image {
    let (w, h) = (img.width as usize, img.height as usize);
    // separable square min/max
    let pass = |src: &[Px], horizontal: bool| -> Vec<Px> {
        let mut dst = vec![[0.0; 4]; w * h];
        dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let mut acc = if max { [f32::MIN; 4] } else { [f32::MAX; 4] };
                for d in -r..=r {
                    let (sx, sy) = if horizontal {
                        ((x as i32 + d).clamp(0, w as i32 - 1) as usize, y)
                    } else {
                        (x, (y as i32 + d).clamp(0, h as i32 - 1) as usize)
                    };
                    let q = src[sy * w + sx];
                    for c in 0..4 {
                        acc[c] = if max {
                            acc[c].max(q[c])
                        } else {
                            acc[c].min(q[c])
                        };
                    }
                }
                *p = acc;
            }
        });
        dst
    };
    let a = pass(img.pixels(), true);
    Image::from_pixels(img.width, img.height, pass(&a, false))
}

fn kuwahara(img: &Image, r: i32) -> Image {
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let (x, y) = (x as i32, y as i32);
                let mut best = (f32::MAX, [0.0f32; 3]);
                for (qx, qy) in [(-1, -1), (0, -1), (-1, 0), (0, 0)] {
                    let mut mean = [0.0f32; 3];
                    let mut sq = 0.0f32;
                    let mut n = 0.0;
                    for j in 0..=r {
                        for i in 0..=r {
                            let sx = x + if qx < 0 { -i } else { i };
                            let sy = y + if qy < 0 { -j } else { j };
                            let q = img.get_clamped(sx, sy);
                            for c in 0..3 {
                                mean[c] += q[c];
                            }
                            let l = luma709([q[0], q[1], q[2]]);
                            sq += l * l;
                            n += 1.0;
                        }
                    }
                    for m in mean.iter_mut() {
                        *m /= n;
                    }
                    let ml = luma709(mean);
                    let var = sq / n - ml * ml;
                    if var < best.0 {
                        best = (var, mean);
                    }
                }
                p[0] = best.1[0];
                p[1] = best.1[1];
                p[2] = best.1[2];
            }
        });
    out
}

fn crystallize(img: &Image, cell: u32, seed: u32) -> Image {
    let c = cell as i32;
    let (gw, gh) = (
        (img.width as i32 + c - 1) / c,
        (img.height as i32 + c - 1) / c,
    );
    let pt = |gx: i32, gy: i32| -> (f32, f32) {
        let k = (gx as u32).wrapping_mul(7919) ^ (gy as u32).wrapping_mul(104_729) ^ seed;
        (
            (gx * c) as f32 + rand01(k, 1) * c as f32,
            (gy * c) as f32 + rand01(k, 2) * c as f32,
        )
    };
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let (gx, gy) = (x as i32 / c, y as i32 / c);
                let mut best = (f32::MAX, (0.0, 0.0));
                for j in -1..=1 {
                    for i in -1..=1 {
                        let (ax, ay) = (gx + i, gy + j);
                        if ax < 0 || ay < 0 || ax >= gw || ay >= gh {
                            continue;
                        }
                        let q = pt(ax, ay);
                        let d = (q.0 - x as f32).powi(2) + (q.1 - y as f32).powi(2);
                        if d < best.0 {
                            best = (d, q);
                        }
                    }
                }
                *p = img.get_clamped(best.1 .0 as i32, best.1 .1 as i32);
            }
        });
    out
}

fn halftone(img: &Image, max_r: f32) -> Image {
    // CMY screens at classic angles, composited subtractively on white
    let cell = max_r * 2.0;
    let angles = [108f32.to_radians(), 162f32.to_radians(), 90f32.to_radians()];
    let ww = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(ww)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, p) in row.iter_mut().enumerate() {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                for ch in 0..3 {
                    let (s, c) = angles[ch].sin_cos();
                    let u = fx * c + fy * s;
                    let v = -fx * s + fy * c;
                    let cu = (u / cell).floor() * cell + cell / 2.0;
                    let cv = (v / cell).floor() * cell + cell / 2.0;
                    let (cx, cy) = (cu * c - cv * s, cu * s + cv * c);
                    let src = img.sample_bilinear(cx, cy);
                    let ink = 1.0 - src[ch];
                    let r = ink.sqrt() * max_r * std::f32::consts::FRAC_1_SQRT_2 * 1.42;
                    let d = ((u - cu).powi(2) + (v - cv).powi(2)).sqrt();
                    let cov = clamp01(r - d + 0.5);
                    p[ch] = 1.0 - cov;
                }
            }
        });
    out
}

/// Exact squared Euclidean distance transform (Felzenszwalb–Huttenlocher) of a binary mask.
/// Returns distance (in px) from each pixel to the nearest pixel where `inside` is true.
pub fn distance_transform(w: usize, h: usize, inside: &[bool]) -> Vec<f32> {
    const INF: f32 = 1e20;
    let mut f: Vec<f32> = inside.iter().map(|b| if *b { 0.0 } else { INF }).collect();
    fn dt1(f: &[f32], n: usize) -> Vec<f32> {
        let mut d = vec![0.0; n];
        let mut v = vec![0usize; n];
        let mut z = vec![0.0f32; n + 1];
        let mut k = 0usize;
        z[0] = -INF;
        z[1] = INF;
        for q in 1..n {
            loop {
                let vk = v[k];
                let s = ((f[q] + (q * q) as f32) - (f[vk] + (vk * vk) as f32))
                    / (2.0 * q as f32 - 2.0 * vk as f32);
                if s <= z[k] {
                    if k == 0 {
                        v[0] = q;
                        z[0] = -INF;
                        z[1] = INF;
                        break;
                    }
                    k -= 1;
                } else {
                    k += 1;
                    v[k] = q;
                    z[k] = s;
                    z[k + 1] = INF;
                    break;
                }
            }
        }
        k = 0;
        for q in 0..n {
            while z[k + 1] < q as f32 {
                k += 1;
            }
            let dq = q as f32 - v[k] as f32;
            d[q] = dq * dq + f[v[k]];
        }
        d
    }
    // columns
    let cols: Vec<Vec<f32>> = (0..w)
        .into_par_iter()
        .map(|x| {
            let col: Vec<f32> = (0..h).map(|y| f[y * w + x]).collect();
            dt1(&col, h)
        })
        .collect();
    for x in 0..w {
        for y in 0..h {
            f[y * w + x] = cols[x][y];
        }
    }
    f.par_chunks_mut(w).for_each(|row| {
        let r = dt1(row, w);
        row.copy_from_slice(&r);
    });
    f.into_par_iter().map(|v| v.sqrt()).collect()
}

/// Grow (positive) or shrink (negative) a mask by `px` pixels with soft edges.
pub fn morph_mask(m: &Mask, px: f32) -> Mask {
    let (w, h) = (m.width as usize, m.height as usize);
    if px >= 0.0 {
        let inside: Vec<bool> = m.data().iter().map(|v| *v >= 0.5).collect();
        let d = distance_transform(w, h, &inside);
        Mask::from_vec(
            m.width,
            m.height,
            d.iter()
                .zip(m.data())
                .map(|(d, o)| clamp01(px + 1.0 - d).max(*o))
                .collect(),
        )
    } else {
        let outside: Vec<bool> = m.data().iter().map(|v| *v < 0.5).collect();
        let d = distance_transform(w, h, &outside);
        Mask::from_vec(
            m.width,
            m.height,
            d.iter()
                .zip(m.data())
                .map(|(d, o)| clamp01(d + px).min(*o))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn test_img() -> Image {
        let mut img = Image::new(48, 32);
        for y in 0..32 {
            for x in 0..48 {
                img.set(x, y, [x as f32 / 47.0, y as f32 / 31.0, 0.5, 1.0]);
            }
        }
        img
    }
    #[test]
    fn every_filter_runs_and_stays_in_range() {
        let img = test_img();
        let ctx = FilterCtx::default();
        for (_, group) in Filter::menu() {
            for f in group {
                let out = f.render(&img, &ctx);
                assert_eq!((out.width, out.height), (48, 32), "{}", f.name());
                for p in out.pixels() {
                    for v in p {
                        assert!(
                            v.is_finite() && (-1e-4..=1.0001).contains(v),
                            "{} produced {v}",
                            f.name()
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn blur_preserves_flat_color() {
        let img = Image::filled(20, 20, [0.2, 0.4, 0.6, 1.0]);
        let b = gaussian_blur(&img, 5.0);
        let p = b.get(10, 10);
        assert!((p[0] - 0.2).abs() < 1e-4 && (p[3] - 1.0).abs() < 1e-4);
    }
    #[test]
    fn distance_transform_basic() {
        let mut inside = vec![false; 9];
        inside[4] = true;
        let d = distance_transform(3, 3, &inside);
        assert_eq!(d[4], 0.0);
        assert!((d[0] - 2f32.sqrt()).abs() < 1e-5);
        assert_eq!(d[1], 1.0);
    }
}
