//! Selections: marquees, lasso, magic wand, color range, and Select menu modifiers.

use crate::buffer::{Image, Mask};
use crate::color::*;
use crate::filter::{gaussian_blur_mask, morph_mask};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectOp {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl SelectOp {
    pub const ALL: [SelectOp; 4] = [
        SelectOp::Replace,
        SelectOp::Add,
        SelectOp::Subtract,
        SelectOp::Intersect,
    ];
    pub fn name(self) -> &'static str {
        match self {
            SelectOp::Replace => "New",
            SelectOp::Add => "Add",
            SelectOp::Subtract => "Subtract",
            SelectOp::Intersect => "Intersect",
        }
    }
}

/// Combine a new shape into the current selection.
pub fn combine(current: Option<&Mask>, shape: Mask, op: SelectOp) -> Option<Mask> {
    let out = match (op, current) {
        (SelectOp::Replace, _) | (_, None) => {
            if op == SelectOp::Subtract || op == SelectOp::Intersect {
                return None;
            }
            shape
        }
        (op, Some(cur)) => {
            let data: Vec<f32> = cur
                .data()
                .par_iter()
                .zip(shape.data().par_iter())
                .map(|(a, b)| match op {
                    SelectOp::Add => a.max(*b),
                    SelectOp::Subtract => a.min(1.0 - b),
                    SelectOp::Intersect => a.min(*b),
                    SelectOp::Replace => *b,
                })
                .collect();
            Mask::from_vec(shape.width, shape.height, data)
        }
    };
    if out.is_empty_selection() {
        None
    } else {
        Some(out)
    }
}

/// Anti-aliased rectangle (4x4 supersampling at edges is unnecessary: coverage is analytic).
pub fn rect_mask(w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Mask {
    let (ax, bx) = (x0.min(x1), x0.max(x1));
    let (ay, by) = (y0.min(y1), y0.max(y1));
    let data = (0..(w * h) as usize)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w as usize) as f32, (i / w as usize) as f32);
            let cx = clamp01((x + 1.0).min(bx) - x.max(ax));
            let cy = clamp01((y + 1.0).min(by) - y.max(ay));
            cx * cy
        })
        .collect();
    Mask::from_vec(w, h, data)
}

pub fn ellipse_mask(w: u32, h: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> Mask {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (rx, ry) = (
        ((x1 - x0) / 2.0).abs().max(0.5),
        ((y1 - y0) / 2.0).abs().max(0.5),
    );
    let data = (0..(w * h) as usize)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w as usize) as f32 + 0.5, (i / w as usize) as f32 + 0.5);
            let dx = (x - cx) / rx;
            let dy = (y - cy) / ry;
            let d = (dx * dx + dy * dy).sqrt();
            // signed distance in px (approx) for antialiasing
            let px = (d - 1.0) * rx.min(ry);
            clamp01(0.5 - px)
        })
        .collect();
    Mask::from_vec(w, h, data)
}

/// Polygon (lasso / polygonal lasso) via even-odd scanline fill with 4x vertical supersampling.
pub fn polygon_mask(w: u32, h: u32, pts: &[(f32, f32)]) -> Mask {
    let mut data = vec![0.0f32; (w * h) as usize];
    if pts.len() < 3 {
        return Mask::from_vec(w, h, data);
    }
    const SS: usize = 4;
    data.par_chunks_mut(w as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let mut cov = vec![0.0f32; w as usize];
            for s in 0..SS {
                let sy = y as f32 + (s as f32 + 0.5) / SS as f32;
                let mut xs: Vec<f32> = Vec::new();
                for i in 0..pts.len() {
                    let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                    if (a.1 <= sy && b.1 > sy) || (b.1 <= sy && a.1 > sy) {
                        xs.push(a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                    }
                }
                xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for pair in xs.chunks(2) {
                    if pair.len() < 2 {
                        continue;
                    }
                    let (l, r) = (pair[0].max(0.0), pair[1].min(w as f32));
                    if r <= l {
                        continue;
                    }
                    let (li, ri) = (l.floor() as usize, (r.ceil() as usize).min(w as usize));
                    for x in li..ri {
                        let c = clamp01((x as f32 + 1.0).min(r) - (x as f32).max(l));
                        cov[x] += c / SS as f32;
                    }
                }
            }
            row.copy_from_slice(&cov);
        });
    Mask::from_vec(w, h, data)
}

#[inline]
fn color_dist(a: Px, b: Px) -> f32 {
    // Photoshop wand compares per-channel max difference (0..255 scale); include alpha
    (a[0] - b[0])
        .abs()
        .max((a[1] - b[1]).abs())
        .max((a[2] - b[2]).abs())
        .max((a[3] - b[3]).abs())
}

/// Magic Wand / Paint Bucket region. `tolerance` in 0..255 like Photoshop.
pub fn magic_wand(
    img: &Image,
    sx: u32,
    sy: u32,
    tolerance: f32,
    contiguous: bool,
    antialias: bool,
) -> Mask {
    let (w, h) = (img.width, img.height);
    let mut m = Mask::new(w, h, 0.0);
    if sx >= w || sy >= h {
        return m;
    }
    let seed = img.get(sx, sy);
    let tol = tolerance / 255.0;
    let inside = |p: Px| color_dist(p, seed) <= tol + 1e-6;
    if !contiguous {
        let data = img
            .pixels()
            .par_iter()
            .map(|p| if inside(*p) { 1.0 } else { 0.0 })
            .collect();
        m = Mask::from_vec(w, h, data);
    } else {
        let mut visited = vec![false; (w * h) as usize];
        let md = m.data_mut();
        let mut q = VecDeque::new();
        q.push_back((sx, sy));
        visited[(sy * w + sx) as usize] = true;
        while let Some((x, y)) = q.pop_front() {
            md[(y * w + x) as usize] = 1.0;
            let nb = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in nb {
                if nx < w && ny < h {
                    let i = (ny * w + nx) as usize;
                    if !visited[i] && inside(img.pixels()[i]) {
                        visited[i] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
        }
    }
    if antialias {
        m = gaussian_blur_mask(&m, 0.8);
    }
    m
}

/// Select > Color Range: soft selection by color similarity (fuzziness 0..200 like PS).
pub fn color_range(img: &Image, target: Color, fuzziness: f32) -> Mask {
    let f = (fuzziness / 255.0).max(1e-3);
    let t = target.px();
    let data = img
        .pixels()
        .par_iter()
        .map(|p| {
            let d = ((p[0] - t[0]).powi(2) + (p[1] - t[1]).powi(2) + (p[2] - t[2]).powi(2)).sqrt();
            clamp01(1.0 - d / f) * p[3].min(1.0)
        })
        .collect();
    Mask::from_vec(img.width, img.height, data)
}

/// Select > Color Range > Highlights / Midtones / Shadows.
pub fn tonal_range(img: &Image, which: u8) -> Mask {
    let data = img
        .pixels()
        .par_iter()
        .map(|p| {
            let l = luma709([p[0], p[1], p[2]]);
            match which {
                0 => crate::filter::smoothstep(0.55, 0.8, l),
                1 => 1.0 - ((l - 0.5).abs() * 2.5).min(1.0),
                _ => 1.0 - crate::filter::smoothstep(0.2, 0.45, l),
            }
        })
        .collect();
    Mask::from_vec(img.width, img.height, data)
}

/// "Select Subject"-style heuristic without ML: picks the region whose colors differ most
/// from the border colors (background estimate), then cleans it up. Swap in an ML
/// segmentation model here for production-grade results (see ROADMAP).
pub fn select_subject_heuristic(img: &Image) -> Mask {
    let (w, h) = (img.width as usize, img.height as usize);
    // background palette = average of border pixels in 8 segments
    let mut samples: Vec<Px> = Vec::new();
    let step = ((w + h) / 64).max(1);
    for x in (0..w).step_by(step) {
        samples.push(img.get(x as u32, 0));
        samples.push(img.get(x as u32, h as u32 - 1));
    }
    for y in (0..h).step_by(step) {
        samples.push(img.get(0, y as u32));
        samples.push(img.get(w as u32 - 1, y as u32));
    }
    let data: Vec<f32> = img
        .pixels()
        .par_iter()
        .map(|p| {
            let mut best = f32::MAX;
            for s in &samples {
                let d =
                    ((p[0] - s[0]).powi(2) + (p[1] - s[1]).powi(2) + (p[2] - s[2]).powi(2)).sqrt();
                best = best.min(d);
            }
            crate::filter::smoothstep(0.08, 0.22, best) * p[3]
        })
        .collect();
    let m = Mask::from_vec(img.width, img.height, data);
    // close small holes, drop specks
    let m = morph_mask(&morph_mask(&m, 3.0), -3.0);
    let m = morph_mask(&morph_mask(&m, -2.0), 2.0);
    gaussian_blur_mask(&m, 1.0)
}

pub fn invert(m: &Mask) -> Mask {
    Mask::from_vec(
        m.width,
        m.height,
        m.data().par_iter().map(|v| 1.0 - v).collect(),
    )
}

pub fn feather(m: &Mask, radius: f32) -> Mask {
    gaussian_blur_mask(m, radius * 2.0)
}

pub fn expand(m: &Mask, px: f32) -> Mask {
    morph_mask(m, px)
}

pub fn contract(m: &Mask, px: f32) -> Mask {
    morph_mask(m, -px)
}

pub fn border(m: &Mask, width: f32) -> Mask {
    let outer = morph_mask(m, width / 2.0);
    let inner = morph_mask(m, -width / 2.0);
    Mask::from_vec(
        m.width,
        m.height,
        outer
            .data()
            .iter()
            .zip(inner.data())
            .map(|(o, i)| (o - i).max(0.0))
            .collect(),
    )
}

pub fn smooth(m: &Mask, radius: f32) -> Mask {
    let b = gaussian_blur_mask(m, radius * 2.0);
    Mask::from_vec(
        m.width,
        m.height,
        b.data()
            .iter()
            .map(|v| crate::filter::smoothstep(0.45, 0.55, *v))
            .collect(),
    )
}

/// Select > Grow: extend to adjacent similar colors.
pub fn grow(img: &Image, m: &Mask, tolerance: f32) -> Mask {
    let (w, h) = (img.width, img.height);
    let mut out = m.clone();
    let tol = tolerance / 255.0;
    let mut q = VecDeque::new();
    let mut visited = vec![false; (w * h) as usize];
    for (i, v) in m.data().iter().enumerate() {
        if *v >= 0.5 {
            visited[i] = true;
            q.push_back(i as u32);
        }
    }
    let px = img.pixels();
    let od = out.data_mut();
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        let base = px[i as usize];
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < w && ny < h {
                let j = (ny * w + nx) as usize;
                if !visited[j] && color_dist(px[j], base) <= tol {
                    visited[j] = true;
                    od[j] = 1.0;
                    q.push_back(j as u32);
                }
            }
        }
    }
    out
}

/// Marching-ants outline: list of boundary pixel edges for drawing, as line segments.
pub fn outline_segments(m: &Mask, step: u32) -> Vec<((f32, f32), (f32, f32))> {
    let (w, h) = (m.width as i32, m.height as i32);
    let s = step.max(1) as i32;
    let at = |x: i32, y: i32| m.get_i(x, y) >= 0.5;
    let mut segs = Vec::new();
    let mut y = 0;
    while y <= h {
        let mut x = 0;
        while x <= w {
            let c = at(x, y);
            if c != at(x - s, y) {
                segs.push(((x as f32, y as f32), (x as f32, (y + s) as f32)));
            }
            if c != at(x, y - s) {
                segs.push(((x as f32, y as f32), ((x + s) as f32, y as f32)));
            }
            x += s;
        }
        y += s;
    }
    segs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wand_contiguous() {
        let mut img = Image::filled(10, 10, [1.0, 1.0, 1.0, 1.0]);
        for y in 0..10 {
            img.set(5, y, [0.0, 0.0, 0.0, 1.0]);
        }
        let m = magic_wand(&img, 0, 0, 10.0, true, false);
        assert_eq!(m.get(4, 4), 1.0);
        assert_eq!(m.get(6, 4), 0.0);
        let g = magic_wand(&img, 0, 0, 10.0, false, false);
        assert_eq!(g.get(6, 4), 1.0);
    }
    #[test]
    fn polygon_area() {
        let m = polygon_mask(
            20,
            20,
            &[(2.0, 2.0), (12.0, 2.0), (12.0, 12.0), (2.0, 12.0)],
        );
        let area: f32 = m.data().iter().sum();
        assert!((area - 100.0).abs() < 1.0, "{area}");
    }
    #[test]
    fn combine_ops() {
        let a = rect_mask(10, 10, 0.0, 0.0, 5.0, 10.0);
        let b = rect_mask(10, 10, 3.0, 0.0, 10.0, 10.0);
        let i = combine(Some(&a), b.clone(), SelectOp::Intersect).unwrap();
        assert_eq!(i.get(4, 0), 1.0);
        assert_eq!(i.get(1, 0), 0.0);
        let s = combine(Some(&a), b, SelectOp::Subtract).unwrap();
        assert_eq!(s.get(1, 0), 1.0);
        assert_eq!(s.get(4, 0), 0.0);
    }
}
