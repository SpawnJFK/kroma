//! Shape tools: rectangle, rounded rectangle, ellipse, polygon, star, line/arrow.

use crate::blend::BlendMode;
use crate::brush::fill_selection;
use crate::buffer::{Image, Mask};
use crate::color::{clamp01, Color};
use crate::selection::{border, ellipse_mask, polygon_mask, rect_mask};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    RoundedRect {
        radius: f32,
    },
    Ellipse,
    Polygon {
        sides: u32,
    },
    Star {
        points: u32,
        inner: f32,
    },
    Line {
        width: f32,
        arrow_end: bool,
        arrow_start: bool,
    },
}

impl ShapeKind {
    pub fn name(&self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::RoundedRect { .. } => "Rounded Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Polygon { .. } => "Polygon",
            ShapeKind::Star { .. } => "Star",
            ShapeKind::Line { .. } => "Line / Arrow",
        }
    }
    pub fn defaults() -> Vec<ShapeKind> {
        vec![
            ShapeKind::Rectangle,
            ShapeKind::RoundedRect { radius: 16.0 },
            ShapeKind::Ellipse,
            ShapeKind::Polygon { sides: 6 },
            ShapeKind::Star {
                points: 5,
                inner: 0.45,
            },
            ShapeKind::Line {
                width: 4.0,
                arrow_end: false,
                arrow_start: false,
            },
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShapeStyle {
    pub fill: Option<Color>,
    pub stroke: Option<Color>,
    pub stroke_width: f32,
    pub opacity: f32,
}

impl Default for ShapeStyle {
    fn default() -> Self {
        ShapeStyle {
            fill: Some(Color::BLACK),
            stroke: None,
            stroke_width: 3.0,
            opacity: 1.0,
        }
    }
}

fn rounded_rect_mask(w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Mask {
    let (ax, bx) = (x0.min(x1), x0.max(x1));
    let (ay, by) = (y0.min(y1), y0.max(y1));
    let (cx, cy) = ((ax + bx) / 2.0, (ay + by) / 2.0);
    let (hx, hy) = ((bx - ax) / 2.0, (by - ay) / 2.0);
    let r = r.min(hx).min(hy).max(0.0);
    let data = (0..(w * h) as usize)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w as usize) as f32 + 0.5, (i / w as usize) as f32 + 0.5);
            let qx = (x - cx).abs() - (hx - r);
            let qy = (y - cy).abs() - (hy - r);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let sd = outside + qx.max(qy).min(0.0) - r;
            clamp01(0.5 - sd)
        })
        .collect();
    Mask::from_vec(w, h, data)
}

fn regular_points(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    n: u32,
    inner: Option<f32>,
) -> Vec<(f32, f32)> {
    let n = n.max(3);
    let total = if inner.is_some() { n * 2 } else { n };
    (0..total)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / total as f32;
            let k = match inner {
                Some(f) if i % 2 == 1 => f,
                _ => 1.0,
            };
            (cx + a.cos() * rx * k, cy + a.sin() * ry * k)
        })
        .collect()
}

fn line_polygon(a: (f32, f32), b: (f32, f32), width: f32) -> Vec<(f32, f32)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (nx, ny) = (-dy / len * width / 2.0, dx / len * width / 2.0);
    vec![
        (a.0 + nx, a.1 + ny),
        (b.0 + nx, b.1 + ny),
        (b.0 - nx, b.1 - ny),
        (a.0 - nx, a.1 - ny),
    ]
}

fn arrow_head(tip: (f32, f32), from: (f32, f32), width: f32) -> Vec<(f32, f32)> {
    let (dx, dy) = (tip.0 - from.0, tip.1 - from.1);
    let len = (dx * dx + dy * dy).sqrt().max(1e-3);
    let (ux, uy) = (dx / len, dy / len);
    let size = (width * 4.0).max(10.0);
    let base = (tip.0 - ux * size, tip.1 - uy * size);
    let (nx, ny) = (-uy * size * 0.5, ux * size * 0.5);
    vec![tip, (base.0 + nx, base.1 + ny), (base.0 - nx, base.1 - ny)]
}

/// Coverage mask of a shape dragged from `a` to `b`.
pub fn shape_mask(kind: ShapeKind, w: u32, h: u32, a: (f32, f32), b: (f32, f32)) -> Mask {
    let (cx, cy) = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let (rx, ry) = (((b.0 - a.0) / 2.0).abs(), ((b.1 - a.1) / 2.0).abs());
    match kind {
        ShapeKind::Rectangle => rect_mask(w, h, a.0, a.1, b.0, b.1),
        ShapeKind::RoundedRect { radius } => rounded_rect_mask(w, h, a.0, a.1, b.0, b.1, radius),
        ShapeKind::Ellipse => ellipse_mask(w, h, a.0, a.1, b.0, b.1),
        ShapeKind::Polygon { sides } => {
            polygon_mask(w, h, &regular_points(cx, cy, rx, ry, sides, None))
        }
        ShapeKind::Star { points, inner } => {
            polygon_mask(w, h, &regular_points(cx, cy, rx, ry, points, Some(inner)))
        }
        ShapeKind::Line {
            width,
            arrow_end,
            arrow_start,
        } => {
            let mut m = polygon_mask(w, h, &line_polygon(a, b, width));
            let mut add = |poly: Vec<(f32, f32)>| {
                let am = polygon_mask(w, h, &poly);
                let md = m.data_mut();
                for (v, q) in md.iter_mut().zip(am.data()) {
                    *v = v.max(*q);
                }
            };
            if arrow_end {
                add(arrow_head(b, a, width));
            }
            if arrow_start {
                add(arrow_head(a, b, width));
            }
            m
        }
    }
}

pub fn draw_shape(
    img: &mut Image,
    sel: Option<&Mask>,
    kind: ShapeKind,
    style: &ShapeStyle,
    a: (f32, f32),
    b: (f32, f32),
) {
    let m = shape_mask(kind, img.width, img.height, a, b);
    let combine = |mut m: Mask| {
        if let Some(s) = sel {
            for (v, q) in m.data_mut().iter_mut().zip(s.data()) {
                *v *= q;
            }
        }
        m
    };
    if let Some(c) = style.fill {
        if !matches!(kind, ShapeKind::Line { .. }) || style.stroke.is_none() {
            fill_selection(
                img,
                Some(&combine(m.clone())),
                c,
                style.opacity,
                BlendMode::Normal,
                false,
            );
        }
    }
    if let Some(c) = style.stroke {
        let ring = border(&m, style.stroke_width);
        fill_selection(
            img,
            Some(&combine(ring)),
            c,
            style.opacity,
            BlendMode::Normal,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapes_cover_area() {
        for k in ShapeKind::defaults() {
            let m = shape_mask(k, 64, 64, (8.0, 8.0), (56.0, 56.0));
            let s: f32 = m.data().iter().sum();
            assert!(s > 50.0, "{} too small: {s}", k.name());
        }
    }
}
