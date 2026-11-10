//! Per-pixel color adjustments. Every adjustment is serializable so it can live as a
//! non-destructive adjustment layer, be recorded in an Action, or run from the CLI.

use crate::buffer::{Image, Mask};
use crate::color::*;
use crate::gradient::Gradient;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LevelsChannel {
    pub in_black: f32,
    pub in_white: f32,
    pub gamma: f32,
    pub out_black: f32,
    pub out_white: f32,
}

impl Default for LevelsChannel {
    fn default() -> Self {
        LevelsChannel {
            in_black: 0.0,
            in_white: 1.0,
            gamma: 1.0,
            out_black: 0.0,
            out_white: 1.0,
        }
    }
}

impl LevelsChannel {
    pub fn map(&self, v: f32) -> f32 {
        let range = (self.in_white - self.in_black).max(1e-4);
        let t = clamp01((v - self.in_black) / range).powf(1.0 / self.gamma.max(0.01));
        self.out_black + t * (self.out_white - self.out_black)
    }
    pub fn is_identity(&self) -> bool {
        *self == LevelsChannel::default()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurvesChannel {
    /// Control points (input, output), 0..1, sorted by input.
    pub points: Vec<(f32, f32)>,
}

impl Default for CurvesChannel {
    fn default() -> Self {
        CurvesChannel {
            points: vec![(0.0, 0.0), (1.0, 1.0)],
        }
    }
}

impl CurvesChannel {
    /// Monotone cubic (Fritsch–Carlson) interpolation through the points.
    pub fn lut(&self, n: usize) -> Vec<f32> {
        let mut p = self.points.clone();
        p.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        p.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-5);
        if p.len() < 2 {
            return (0..n).map(|i| i as f32 / (n - 1) as f32).collect();
        }
        let k = p.len();
        let mut d = vec![0.0f32; k - 1];
        for i in 0..k - 1 {
            d[i] = (p[i + 1].1 - p[i].1) / (p[i + 1].0 - p[i].0).max(1e-6);
        }
        let mut m = vec![0.0f32; k];
        m[0] = d[0];
        m[k - 1] = d[k - 2];
        for i in 1..k - 1 {
            m[i] = if d[i - 1] * d[i] <= 0.0 {
                0.0
            } else {
                (d[i - 1] + d[i]) * 0.5
            };
        }
        for i in 0..k - 1 {
            if d[i].abs() < 1e-7 {
                m[i] = 0.0;
                m[i + 1] = 0.0;
            } else {
                let a = m[i] / d[i];
                let b = m[i + 1] / d[i];
                let s = a * a + b * b;
                if s > 9.0 {
                    let t = 3.0 / s.sqrt();
                    m[i] = t * a * d[i];
                    m[i + 1] = t * b * d[i];
                }
            }
        }
        (0..n)
            .map(|i| {
                let x = i as f32 / (n - 1) as f32;
                if x <= p[0].0 {
                    return clamp01(p[0].1);
                }
                if x >= p[k - 1].0 {
                    return clamp01(p[k - 1].1);
                }
                let mut j = 0;
                while j < k - 2 && x > p[j + 1].0 {
                    j += 1;
                }
                let h = p[j + 1].0 - p[j].0;
                let t = (x - p[j].0) / h;
                let (t2, t3) = (t * t, t * t * t);
                let y = (2.0 * t3 - 3.0 * t2 + 1.0) * p[j].1
                    + (t3 - 2.0 * t2 + t) * h * m[j]
                    + (-2.0 * t3 + 3.0 * t2) * p[j + 1].1
                    + (t3 - t2) * h * m[j + 1];
                clamp01(y)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cmyk {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Adjustment {
    BrightnessContrast {
        brightness: f32,
        contrast: f32,
        legacy: bool,
    },
    Levels {
        master: LevelsChannel,
        red: LevelsChannel,
        green: LevelsChannel,
        blue: LevelsChannel,
    },
    Curves {
        master: CurvesChannel,
        red: CurvesChannel,
        green: CurvesChannel,
        blue: CurvesChannel,
    },
    Exposure {
        exposure: f32,
        offset: f32,
        gamma: f32,
    },
    Vibrance {
        vibrance: f32,
        saturation: f32,
    },
    HueSaturation {
        hue: f32,
        saturation: f32,
        lightness: f32,
        colorize: bool,
    },
    ColorBalance {
        shadows: [f32; 3],
        midtones: [f32; 3],
        highlights: [f32; 3],
        preserve_luminosity: bool,
    },
    BlackWhite {
        weights: [f32; 6],
        tint: Option<Color>,
    },
    PhotoFilter {
        color: Color,
        density: f32,
        preserve_luminosity: bool,
    },
    ChannelMixer {
        red: [f32; 4],
        green: [f32; 4],
        blue: [f32; 4],
        monochrome: bool,
    },
    /// Temperature (-1 cool .. +1 warm) and tint (-1 green .. +1 magenta) - the 2026 "Color & Vibrance" controls.
    TemperatureTint {
        temperature: f32,
        tint: f32,
    },
    SelectiveColor {
        ranges: [Cmyk; 9],
        absolute: bool,
    },
    GradientMap {
        gradient: Gradient,
        reverse: bool,
    },
    Invert,
    Desaturate,
    Posterize {
        levels: u32,
    },
    Threshold {
        level: f32,
    },
    Solarize,
    Sepia {
        amount: f32,
    },
}

pub const SELECTIVE_COLOR_RANGES: [&str; 9] = [
    "Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas", "Whites", "Neutrals", "Blacks",
];
pub const BW_WEIGHT_NAMES: [&str; 6] = ["Reds", "Yellows", "Greens", "Cyans", "Blues", "Magentas"];

impl Adjustment {
    pub fn name(&self) -> &'static str {
        use Adjustment::*;
        match self {
            BrightnessContrast { .. } => "Brightness/Contrast",
            Levels { .. } => "Levels",
            Curves { .. } => "Curves",
            Exposure { .. } => "Exposure",
            Vibrance { .. } => "Vibrance",
            HueSaturation { .. } => "Hue/Saturation",
            ColorBalance { .. } => "Color Balance",
            BlackWhite { .. } => "Black & White",
            PhotoFilter { .. } => "Photo Filter",
            ChannelMixer { .. } => "Channel Mixer",
            TemperatureTint { .. } => "Temperature & Tint",
            SelectiveColor { .. } => "Selective Color",
            GradientMap { .. } => "Gradient Map",
            Invert => "Invert",
            Desaturate => "Desaturate",
            Posterize { .. } => "Posterize",
            Threshold { .. } => "Threshold",
            Solarize => "Solarize",
            Sepia { .. } => "Sepia",
        }
    }

    /// Default instances of every adjustment, in menu order.
    pub fn catalog() -> Vec<Adjustment> {
        use Adjustment::*;
        vec![
            BrightnessContrast {
                brightness: 0.0,
                contrast: 0.0,
                legacy: false,
            },
            Levels {
                master: Default::default(),
                red: Default::default(),
                green: Default::default(),
                blue: Default::default(),
            },
            Curves {
                master: Default::default(),
                red: Default::default(),
                green: Default::default(),
                blue: Default::default(),
            },
            Exposure {
                exposure: 0.0,
                offset: 0.0,
                gamma: 1.0,
            },
            Vibrance {
                vibrance: 0.0,
                saturation: 0.0,
            },
            HueSaturation {
                hue: 0.0,
                saturation: 0.0,
                lightness: 0.0,
                colorize: false,
            },
            ColorBalance {
                shadows: [0.0; 3],
                midtones: [0.0; 3],
                highlights: [0.0; 3],
                preserve_luminosity: true,
            },
            BlackWhite {
                weights: [0.4, 0.6, 0.4, 0.6, 0.2, 0.8],
                tint: None,
            },
            PhotoFilter {
                color: Color::from_hex("#ec8a00").unwrap(),
                density: 0.25,
                preserve_luminosity: true,
            },
            ChannelMixer {
                red: [1.0, 0.0, 0.0, 0.0],
                green: [0.0, 1.0, 0.0, 0.0],
                blue: [0.0, 0.0, 1.0, 0.0],
                monochrome: false,
            },
            TemperatureTint {
                temperature: 0.0,
                tint: 0.0,
            },
            SelectiveColor {
                ranges: [Cmyk::default(); 9],
                absolute: false,
            },
            GradientMap {
                gradient: Gradient::two(Color::BLACK, Color::WHITE),
                reverse: false,
            },
            Invert,
            Desaturate,
            Posterize { levels: 4 },
            Threshold { level: 0.5 },
            Solarize,
            Sepia { amount: 1.0 },
        ]
    }

    /// Precompute LUTs etc. and return a fast per-pixel function.
    pub fn compile(&self) -> Box<dyn Fn(Px) -> Px + Send + Sync> {
        use Adjustment::*;
        const N: usize = 1024;
        let lut_of = |f: &dyn Fn(f32) -> f32| -> Vec<f32> {
            (0..N).map(|i| f(i as f32 / (N - 1) as f32)).collect()
        };
        fn look(l: &[f32], v: f32) -> f32 {
            let f = clamp01(v) * (l.len() - 1) as f32;
            let i = f as usize;
            if i + 1 >= l.len() {
                return l[l.len() - 1];
            }
            let t = f - i as f32;
            l[i] + (l[i + 1] - l[i]) * t
        }
        match self.clone() {
            BrightnessContrast {
                brightness,
                contrast,
                legacy,
            } => {
                // brightness -1..1, contrast -1..1
                let lut = lut_of(&|v| {
                    if legacy {
                        let v = v + brightness * 0.6;
                        clamp01((v - 0.5) * (1.0 + contrast).max(0.0) + 0.5)
                    } else {
                        // Modern: brightness as a midtone curve, contrast as an S-curve around 0.5
                        let g = if brightness >= 0.0 {
                            1.0 / (1.0 + brightness * 1.5)
                        } else {
                            1.0 - brightness * 1.5
                        };
                        let v = v.powf(g);
                        let c = contrast.clamp(-0.99, 0.99);
                        let x = v - 0.5;
                        let s = if c >= 0.0 {
                            // smooth S curve that keeps 0 and 1 fixed; g -> 0 is the identity
                            let g = 1e-3 + c * 6.0;
                            (x * g).tanh() / (0.5 * g).tanh() * 0.5
                        } else {
                            x * (1.0 + c)
                        };
                        clamp01(s + 0.5)
                    }
                });
                Box::new(move |p| [look(&lut, p[0]), look(&lut, p[1]), look(&lut, p[2]), p[3]])
            }
            Levels {
                master,
                red,
                green,
                blue,
            } => {
                let ch: Vec<Vec<f32>> = [red, green, blue]
                    .iter()
                    .map(|c| lut_of(&|v| master.map(c.map(v))))
                    .collect();
                Box::new(move |p| {
                    [
                        look(&ch[0], p[0]),
                        look(&ch[1], p[1]),
                        look(&ch[2], p[2]),
                        p[3],
                    ]
                })
            }
            Curves {
                master,
                red,
                green,
                blue,
            } => {
                let m = master.lut(N);
                let ch: Vec<Vec<f32>> = [red, green, blue]
                    .iter()
                    .map(|c| c.lut(N).into_iter().map(|v| look(&m, v)).collect())
                    .collect();
                Box::new(move |p| {
                    [
                        look(&ch[0], p[0]),
                        look(&ch[1], p[1]),
                        look(&ch[2], p[2]),
                        p[3],
                    ]
                })
            }
            Exposure {
                exposure,
                offset,
                gamma,
            } => {
                let mul = 2f32.powf(exposure);
                let lut = lut_of(&|v| {
                    let lin = srgb_to_linear(v) * mul + offset;
                    clamp01(linear_to_srgb(lin.max(0.0)).powf(1.0 / gamma.max(0.01)))
                });
                Box::new(move |p| [look(&lut, p[0]), look(&lut, p[1]), look(&lut, p[2]), p[3]])
            }
            Vibrance {
                vibrance,
                saturation,
            } => Box::new(move |p| {
                let c = [p[0], p[1], p[2]];
                let l = luma709(c);
                let s = sat(c);
                // vibrance boosts low-saturation colors more and protects skin-ish hues a bit
                let vib = vibrance * (1.0 - s).powi(2);
                let f = (1.0 + saturation + vib).max(0.0);
                [
                    clamp01(l + (c[0] - l) * f),
                    clamp01(l + (c[1] - l) * f),
                    clamp01(l + (c[2] - l) * f),
                    p[3],
                ]
            }),
            HueSaturation {
                hue,
                saturation,
                lightness,
                colorize,
            } => Box::new(move |p| {
                let c = [p[0], p[1], p[2]];
                let mut hsl = rgb_to_hsl(c);
                if colorize {
                    hsl[0] = hue.rem_euclid(360.0);
                    hsl[1] = clamp01(0.25 + saturation * 0.75).max(0.0);
                } else {
                    hsl[0] += hue;
                    hsl[1] = if saturation >= 0.0 {
                        clamp01(hsl[1] + (1.0 - hsl[1]) * saturation * hsl[1].sqrt())
                    } else {
                        clamp01(hsl[1] * (1.0 + saturation))
                    };
                }
                let mut rgb = hsl_to_rgb(hsl);
                if lightness > 0.0 {
                    for v in rgb.iter_mut() {
                        *v += (1.0 - *v) * lightness;
                    }
                } else if lightness < 0.0 {
                    for v in rgb.iter_mut() {
                        *v *= 1.0 + lightness;
                    }
                }
                [clamp01(rgb[0]), clamp01(rgb[1]), clamp01(rgb[2]), p[3]]
            }),
            ColorBalance {
                shadows,
                midtones,
                highlights,
                preserve_luminosity,
            } => Box::new(move |p| {
                let c = [p[0], p[1], p[2]];
                let l = luma709(c);
                let ws = clamp01(1.0 - l * 3.0).powf(0.8);
                let wh = clamp01(l * 3.0 - 2.0).powf(0.8);
                let wm = clamp01(1.0 - ws - wh) * 0.9 + 0.1 * (1.0 - (2.0 * l - 1.0).abs());
                let mut out = [0.0; 3];
                for i in 0..3 {
                    out[i] = clamp01(
                        c[i] + (shadows[i] * ws + midtones[i] * wm + highlights[i] * wh) * 0.5,
                    );
                }
                if preserve_luminosity {
                    out = set_lum(out, lum(c));
                }
                [clamp01(out[0]), clamp01(out[1]), clamp01(out[2]), p[3]]
            }),
            BlackWhite { weights, tint } => Box::new(move |p| {
                // Photoshop-style: gray = min + weighted contributions of the hue sextants.
                let c = [p[0], p[1], p[2]];
                let hsv = rgb_to_hsv(c);
                let mn = c[0].min(c[1]).min(c[2]);
                let mx = c[0].max(c[1]).max(c[2]);
                let h = hsv[0] / 60.0;
                let i = h.floor() as usize % 6;
                let f = h - h.floor();
                // sextant order: R, Y, G, C, B, M
                let w0 = weights[i];
                let w1 = weights[(i + 1) % 6];
                let wmix = w0 + (w1 - w0) * f;
                let gray = clamp01(mn + (mx - mn) * wmix * 1.0 + luma709(c) * 0.0);
                let rgb = match tint {
                    Some(t) => {
                        let hsl = rgb_to_hsl([t.r, t.g, t.b]);
                        hsl_to_rgb([hsl[0], hsl[1] * 0.8, gray])
                    }
                    None => [gray, gray, gray],
                };
                [rgb[0], rgb[1], rgb[2], p[3]]
            }),
            PhotoFilter {
                color,
                density,
                preserve_luminosity,
            } => Box::new(move |p| {
                let c = [p[0], p[1], p[2]];
                let f = [color.r, color.g, color.b];
                let mut out = [0.0; 3];
                for i in 0..3 {
                    let mul = c[i] * f[i];
                    out[i] = c[i] + (mul - c[i]) * density;
                }
                if preserve_luminosity {
                    out = set_lum(out, lum(c));
                }
                [clamp01(out[0]), clamp01(out[1]), clamp01(out[2]), p[3]]
            }),
            ChannelMixer {
                red,
                green,
                blue,
                monochrome,
            } => Box::new(move |p| {
                let row = |r: [f32; 4]| clamp01(p[0] * r[0] + p[1] * r[1] + p[2] * r[2] + r[3]);
                if monochrome {
                    let v = row(red);
                    [v, v, v, p[3]]
                } else {
                    [row(red), row(green), row(blue), p[3]]
                }
            }),
            TemperatureTint { temperature, tint } => {
                let k = 6500.0 * 2f32.powf(-temperature * 1.2);
                let wp = kelvin_to_rgb(k);
                let ref_wp = kelvin_to_rgb(6500.0);
                let gain = [
                    ref_wp[0] / wp[0].max(1e-3),
                    ref_wp[1] / wp[1].max(1e-3),
                    ref_wp[2] / wp[2].max(1e-3),
                ];
                let gmax = gain[0].max(gain[1]).max(gain[2]);
                let gain = [gain[0] / gmax, gain[1] / gmax, gain[2] / gmax];
                let tg = [1.0 + tint * 0.15, 1.0 - tint * 0.2, 1.0 + tint * 0.15];
                Box::new(move |p| {
                    let c = [p[0], p[1], p[2]];
                    let mut lin = [0.0; 3];
                    for i in 0..3 {
                        lin[i] = srgb_to_linear(c[i]) * gain[i] * tg[i];
                    }
                    let mut out = [
                        linear_to_srgb(lin[0]),
                        linear_to_srgb(lin[1]),
                        linear_to_srgb(lin[2]),
                    ];
                    out = set_lum(out, lum(c));
                    [clamp01(out[0]), clamp01(out[1]), clamp01(out[2]), p[3]]
                })
            }
            SelectiveColor { ranges, absolute } => Box::new(move |p| {
                let c = [p[0], p[1], p[2]];
                let mx = c[0].max(c[1]).max(c[2]);
                let mn = c[0].min(c[1]).min(c[2]);
                let mid = c[0] + c[1] + c[2] - mx - mn;
                let mut out = c;
                let apply = |out: &mut [f32; 3], adj: &Cmyk, amount: f32| {
                    if amount <= 0.0 {
                        return;
                    }
                    let cmy = [adj.c, adj.m, adj.y];
                    for i in 0..3 {
                        let ink = 1.0 - c[i];
                        let delta = if absolute {
                            cmy[i] + adj.k
                        } else {
                            (cmy[i] + adj.k) * ink.max(0.0) * 2.0
                        };
                        out[i] -= delta * amount * 0.5;
                    }
                };
                // hue ranges: amount = max - mid for the dominant primary / secondary
                let hues: [(usize, f32); 6] = {
                    let (r, g, b) = (c[0], c[1], c[2]);
                    let pr = if r == mx { mx - mid } else { 0.0 };
                    let pg = if g == mx { mx - mid } else { 0.0 };
                    let pb = if b == mx { mx - mid } else { 0.0 };
                    let sy = if b == mn { mid - mn } else { 0.0 }
                        * if r.min(g) == mid { 1.0 } else { 0.0 };
                    let sc = if r == mn { mid - mn } else { 0.0 }
                        * if g.min(b) == mid { 1.0 } else { 0.0 };
                    let sm = if g == mn { mid - mn } else { 0.0 }
                        * if r.min(b) == mid { 1.0 } else { 0.0 };
                    [(0, pr), (1, sy), (2, pg), (3, sc), (4, pb), (5, sm)]
                };
                for (i, a) in hues {
                    apply(&mut out, &ranges[i], a);
                }
                let l = luma709(c);
                apply(&mut out, &ranges[6], clamp01((l - 0.5) * 2.0));
                apply(&mut out, &ranges[7], 1.0 - (l - 0.5).abs() * 2.0);
                apply(&mut out, &ranges[8], clamp01((0.5 - l) * 2.0));
                [clamp01(out[0]), clamp01(out[1]), clamp01(out[2]), p[3]]
            }),
            GradientMap { gradient, reverse } => {
                let lut: Vec<Color> = (0..256)
                    .map(|i| {
                        gradient.sample(if reverse {
                            1.0 - i as f32 / 255.0
                        } else {
                            i as f32 / 255.0
                        })
                    })
                    .collect();
                Box::new(move |p| {
                    let l = luma709([p[0], p[1], p[2]]);
                    let c = lut[(clamp01(l) * 255.0) as usize];
                    [c.r, c.g, c.b, p[3] * c.a]
                })
            }
            Invert => Box::new(|p| [1.0 - p[0], 1.0 - p[1], 1.0 - p[2], p[3]]),
            Desaturate => Box::new(|p| {
                let c = [p[0], p[1], p[2]];
                let v = (c[0].max(c[1]).max(c[2]) + c[0].min(c[1]).min(c[2])) * 0.5;
                [v, v, v, p[3]]
            }),
            Posterize { levels } => {
                let n = levels.clamp(2, 255) as f32 - 1.0;
                Box::new(move |p| {
                    let q = |v: f32| (clamp01(v) * n).round() / n;
                    [q(p[0]), q(p[1]), q(p[2]), p[3]]
                })
            }
            Threshold { level } => Box::new(move |p| {
                let v = if luma709([p[0], p[1], p[2]]) >= level {
                    1.0
                } else {
                    0.0
                };
                [v, v, v, p[3]]
            }),
            Solarize => Box::new(|p| {
                let s = |v: f32| if v > 0.5 { 1.0 - v } else { v } * 2.0;
                [s(p[0]).min(1.0), s(p[1]).min(1.0), s(p[2]).min(1.0), p[3]]
            }),
            Sepia { amount } => Box::new(move |p| {
                let (r, g, b) = (p[0], p[1], p[2]);
                let s = [
                    clamp01(0.393 * r + 0.769 * g + 0.189 * b),
                    clamp01(0.349 * r + 0.686 * g + 0.168 * b),
                    clamp01(0.272 * r + 0.534 * g + 0.131 * b),
                ];
                [
                    r + (s[0] - r) * amount,
                    g + (s[1] - g) * amount,
                    b + (s[2] - b) * amount,
                    p[3],
                ]
            }),
        }
    }

    /// Apply to an image in place, optionally masked by a selection.
    pub fn apply(&self, img: &mut Image, sel: Option<&Mask>) {
        let f = self.compile();
        apply_fn(img, sel, &*f);
    }
}

/// Apply a per-pixel function to an image, blended by the selection mask.
pub fn apply_fn(img: &mut Image, sel: Option<&Mask>, f: &(dyn Fn(Px) -> Px + Send + Sync)) {
    let sel_data = sel.map(|s| s.data());
    img.pixels_mut()
        .par_iter_mut()
        .enumerate()
        .for_each(|(i, p)| {
            let m = sel_data.map(|s| s[i]).unwrap_or(1.0);
            if m <= 0.0 || p[3] <= 0.0 {
                return;
            }
            let n = f(*p);
            if m >= 1.0 {
                *p = n;
            } else {
                for c in 0..4 {
                    p[c] += (n[c] - p[c]) * m;
                }
            }
        });
}

/// 256-bin histograms for R, G, B and luminosity (alpha-weighted).
#[derive(Clone, Debug, Default)]
pub struct Histogram {
    pub r: Vec<u32>,
    pub g: Vec<u32>,
    pub b: Vec<u32>,
    pub l: Vec<u32>,
    pub count: u64,
}

impl Histogram {
    pub fn of(img: &Image) -> Histogram {
        let (r, g, b, l, count) = img
            .pixels()
            .par_chunks(4096)
            .map(|chunk| {
                let mut h = (
                    vec![0u32; 256],
                    vec![0u32; 256],
                    vec![0u32; 256],
                    vec![0u32; 256],
                    0u64,
                );
                for p in chunk {
                    if p[3] < 0.5 {
                        continue;
                    }
                    let q = |v: f32| (clamp01(v) * 255.0 + 0.5) as usize;
                    h.0[q(p[0])] += 1;
                    h.1[q(p[1])] += 1;
                    h.2[q(p[2])] += 1;
                    h.3[q(luma709([p[0], p[1], p[2]]))] += 1;
                    h.4 += 1;
                }
                h
            })
            .reduce(
                || (vec![0; 256], vec![0; 256], vec![0; 256], vec![0; 256], 0),
                |mut a, b| {
                    for i in 0..256 {
                        a.0[i] += b.0[i];
                        a.1[i] += b.1[i];
                        a.2[i] += b.2[i];
                        a.3[i] += b.3[i];
                    }
                    a.4 += b.4;
                    a
                },
            );
        Histogram { r, g, b, l, count }
    }

    /// Input value below which `clip` fraction of pixels fall.
    pub fn percentile(bins: &[u32], total: u64, clip: f32, from_top: bool) -> f32 {
        let target = (total as f64 * clip as f64) as u64;
        let mut acc = 0u64;
        if from_top {
            for i in (0..256).rev() {
                acc += bins[i] as u64;
                if acc > target {
                    return i as f32 / 255.0;
                }
            }
            0.0
        } else {
            for i in 0..256 {
                acc += bins[i] as u64;
                if acc > target {
                    return i as f32 / 255.0;
                }
            }
            1.0
        }
    }
    pub fn mean_luma(&self) -> f32 {
        let s: u64 = self
            .l
            .iter()
            .enumerate()
            .map(|(i, c)| i as u64 * *c as u64)
            .sum();
        if self.count == 0 {
            0.5
        } else {
            s as f32 / self.count as f32 / 255.0
        }
    }
}

/// Image > Auto Tone: per-channel levels stretch with 0.1% clipping.
pub fn auto_tone(img: &Image) -> Adjustment {
    let h = Histogram::of(img);
    let ch = |bins: &Vec<u32>| LevelsChannel {
        in_black: Histogram::percentile(bins, h.count, 0.001, false),
        in_white: Histogram::percentile(bins, h.count, 0.001, true).max(0.02),
        ..Default::default()
    };
    Adjustment::Levels {
        master: Default::default(),
        red: ch(&h.r),
        green: ch(&h.g),
        blue: ch(&h.b),
    }
}

/// Image > Auto Contrast: same stretch on all channels (keeps color balance).
pub fn auto_contrast(img: &Image) -> Adjustment {
    let h = Histogram::of(img);
    let lo = Histogram::percentile(&h.r, h.count, 0.001, false)
        .min(Histogram::percentile(&h.g, h.count, 0.001, false))
        .min(Histogram::percentile(&h.b, h.count, 0.001, false));
    let hi = Histogram::percentile(&h.r, h.count, 0.001, true)
        .max(Histogram::percentile(&h.g, h.count, 0.001, true))
        .max(Histogram::percentile(&h.b, h.count, 0.001, true));
    Adjustment::Levels {
        master: LevelsChannel {
            in_black: lo,
            in_white: hi.max(lo + 0.02),
            ..Default::default()
        },
        red: Default::default(),
        green: Default::default(),
        blue: Default::default(),
    }
}

/// Image > Auto Color: contrast stretch plus neutralising midtone gray-world cast.
pub fn auto_color(img: &Image) -> Adjustment {
    let (sum, n) = img
        .pixels()
        .par_iter()
        .filter(|p| p[3] > 0.5)
        .map(|p| ([p[0] as f64, p[1] as f64, p[2] as f64], 1u64))
        .reduce(
            || ([0.0; 3], 0),
            |a, b| {
                (
                    [a.0[0] + b.0[0], a.0[1] + b.0[1], a.0[2] + b.0[2]],
                    a.1 + b.1,
                )
            },
        );
    if n == 0 {
        return auto_tone(img);
    }
    let mean = [sum[0] / n as f64, sum[1] / n as f64, sum[2] / n as f64];
    let gray = (mean[0] + mean[1] + mean[2]) / 3.0;
    let base = auto_contrast(img);
    let master = match base {
        Adjustment::Levels { master, .. } => master,
        _ => unreachable!(),
    };
    let g = |m: f64| LevelsChannel {
        gamma: ((gray.max(1e-3)).ln() / (m.max(1e-3)).ln()).clamp(0.5, 2.0) as f32,
        ..Default::default()
    };
    // gamma such that m^(1/gamma) = gray  => gamma = ln(m)/ln(gray)
    let fix = |m: f64| {
        let mut c = g(m);
        c.gamma = 1.0 / c.gamma;
        c
    };
    Adjustment::Levels {
        master,
        red: fix(mean[0]),
        green: fix(mean[1]),
        blue: fix(mean[2]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_adjustments() {
        let p = [0.25, 0.5, 0.75, 1.0];
        for a in Adjustment::catalog() {
            let identity = matches!(
                a,
                Adjustment::Levels { .. }
                    | Adjustment::Curves { .. }
                    | Adjustment::Exposure { .. }
                    | Adjustment::Vibrance { .. }
                    | Adjustment::HueSaturation { .. }
                    | Adjustment::ChannelMixer { .. }
                    | Adjustment::ColorBalance { .. }
                    | Adjustment::TemperatureTint { .. }
                    | Adjustment::SelectiveColor { .. }
                    | Adjustment::BrightnessContrast { .. }
            );
            let out = a.compile()(p);
            if identity {
                for i in 0..4 {
                    assert!(
                        (out[i] - p[i]).abs() < 0.01,
                        "{} changed pixel: {:?}",
                        a.name(),
                        out
                    );
                }
            }
            for v in out {
                assert!((0.0..=1.0).contains(&v), "{} out of range", a.name());
            }
        }
    }
    #[test]
    fn curves_monotone() {
        let c = CurvesChannel {
            points: vec![(0.0, 0.0), (0.3, 0.2), (0.7, 0.85), (1.0, 1.0)],
        };
        let l = c.lut(256);
        assert!(l.windows(2).all(|w| w[1] >= w[0] - 1e-6));
    }
}
