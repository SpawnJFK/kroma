//! Gradients: color stops, shapes, and the Gradient tool renderer.

use crate::blend::{composite_px, BlendMode};
use crate::buffer::{Image, Mask};
use crate::color::{Color, Px};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientStop {
    pub pos: f32,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub stops: Vec<GradientStop>,
}

impl Gradient {
    pub fn two(a: Color, b: Color) -> Self {
        Gradient {
            stops: vec![
                GradientStop { pos: 0.0, color: a },
                GradientStop { pos: 1.0, color: b },
            ],
        }
    }
    pub fn fg_to_transparent(a: Color) -> Self {
        let mut t = a;
        t.a = 0.0;
        Gradient::two(a, t)
    }
    pub fn presets() -> Vec<(&'static str, Gradient)> {
        let s = |v: &[(f32, &str)]| Gradient {
            stops: v
                .iter()
                .map(|(p, h)| GradientStop {
                    pos: *p,
                    color: Color::from_hex(h).unwrap(),
                })
                .collect(),
        };
        vec![
            ("Black, White", s(&[(0.0, "#000000"), (1.0, "#ffffff")])),
            (
                "Spectrum",
                s(&[
                    (0.0, "#ff0000"),
                    (0.17, "#ffff00"),
                    (0.33, "#00ff00"),
                    (0.5, "#00ffff"),
                    (0.67, "#0000ff"),
                    (0.83, "#ff00ff"),
                    (1.0, "#ff0000"),
                ]),
            ),
            (
                "Sunset",
                s(&[
                    (0.0, "#1b1035"),
                    (0.45, "#c33764"),
                    (0.75, "#f8a14b"),
                    (1.0, "#fde8a8"),
                ]),
            ),
            (
                "Copper",
                s(&[(0.0, "#3b1f0e"), (0.5, "#c47a3d"), (1.0, "#f6d0a3")]),
            ),
            (
                "Ocean",
                s(&[(0.0, "#021b2c"), (0.5, "#0f6e8c"), (1.0, "#9ee6e3")]),
            ),
            (
                "Duotone Teal/Orange",
                s(&[(0.0, "#0e3b43"), (1.0, "#f28c38")]),
            ),
            (
                "Infrared",
                s(&[
                    (0.0, "#000000"),
                    (0.3, "#5a0a7a"),
                    (0.6, "#ff3b00"),
                    (0.85, "#ffd000"),
                    (1.0, "#ffffff"),
                ]),
            ),
        ]
    }
    pub fn sample(&self, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let st = &self.stops;
        if st.is_empty() {
            return Color::BLACK;
        }
        if t <= st[0].pos {
            return st[0].color;
        }
        for w in st.windows(2) {
            if t <= w[1].pos {
                let span = (w[1].pos - w[0].pos).max(1e-6);
                return w[0].color.lerp(w[1].color, (t - w[0].pos) / span);
            }
        }
        st[st.len() - 1].color
    }
    pub fn reversed(&self) -> Gradient {
        let mut stops: Vec<_> = self
            .stops
            .iter()
            .map(|s| GradientStop {
                pos: 1.0 - s.pos,
                color: s.color,
            })
            .collect();
        stops.reverse();
        Gradient { stops }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientShape {
    #[default]
    Linear,
    Radial,
    Angle,
    Reflected,
    Diamond,
}

impl GradientShape {
    pub const ALL: [GradientShape; 5] = [
        GradientShape::Linear,
        GradientShape::Radial,
        GradientShape::Angle,
        GradientShape::Reflected,
        GradientShape::Diamond,
    ];
    pub fn name(self) -> &'static str {
        match self {
            GradientShape::Linear => "Linear",
            GradientShape::Radial => "Radial",
            GradientShape::Angle => "Angle",
            GradientShape::Reflected => "Reflected",
            GradientShape::Diamond => "Diamond",
        }
    }
}

/// Parameter t (0..1) of the gradient at point p for a drag from a to b.
pub fn gradient_t(shape: GradientShape, a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (px, py) = (p.0 - a.0, p.1 - a.1);
    match shape {
        GradientShape::Linear => (px * dx + py * dy) / (len * len),
        GradientShape::Radial => (px * px + py * py).sqrt() / len,
        GradientShape::Reflected => ((px * dx + py * dy) / (len * len)).abs(),
        GradientShape::Diamond => {
            let (ux, uy) = (dx / len, dy / len);
            let along = (px * ux + py * uy).abs();
            let across = (-px * uy + py * ux).abs();
            (along + across) / len
        }
        GradientShape::Angle => {
            let base = dy.atan2(dx);
            let ang = py.atan2(px) - base;
            ang.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GradientFill {
    pub gradient: Gradient,
    pub shape: GradientShape,
    pub reverse: bool,
    pub dither: bool,
    pub opacity: f32,
    pub mode: BlendMode,
}

impl Default for GradientFill {
    fn default() -> Self {
        GradientFill {
            gradient: Gradient::two(Color::BLACK, Color::WHITE),
            shape: GradientShape::Linear,
            reverse: false,
            dither: true,
            opacity: 1.0,
            mode: BlendMode::Normal,
        }
    }
}

/// Render a gradient drag onto `img`, limited to `sel` (if any).
pub fn draw_gradient(
    img: &mut Image,
    sel: Option<&Mask>,
    g: &GradientFill,
    a: (f32, f32),
    b: (f32, f32),
) {
    let w = img.width as usize;
    let grad = if g.reverse {
        g.gradient.reversed()
    } else {
        g.gradient.clone()
    };
    img.pixels_mut()
        .par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, px) in row.iter_mut().enumerate() {
                let m = sel.map(|s| s.get(x as u32, y as u32)).unwrap_or(1.0);
                if m <= 0.0 {
                    continue;
                }
                let mut t = gradient_t(g.shape, a, b, (x as f32 + 0.5, y as f32 + 0.5));
                if g.dither {
                    let n = ((x as u32).wrapping_mul(73_856_093)
                        ^ (y as u32).wrapping_mul(19_349_663))
                        % 1024;
                    t += (n as f32 / 1024.0 - 0.5) / 512.0;
                }
                let c: Px = grad.sample(t).px();
                *px = composite_px(*px, c, g.mode, g.opacity * m, x as u32, y as u32);
            }
        });
}
