//! Geometry: image/canvas size, rotate, flip, crop, free transform (affine), perspective
//! (4-corner homography) and a Liquify-style displacement warp.

use crate::buffer::{Image, Rect};
use crate::color::{Px, TRANSPARENT};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resample {
    Nearest,
    Bilinear,
    #[default]
    Bicubic,
    Lanczos3,
}

impl Resample {
    pub const ALL: [Resample; 4] = [
        Resample::Nearest,
        Resample::Bilinear,
        Resample::Bicubic,
        Resample::Lanczos3,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Resample::Nearest => "Nearest Neighbor (hard edges)",
            Resample::Bilinear => "Bilinear",
            Resample::Bicubic => "Bicubic (smooth gradients)",
            Resample::Lanczos3 => "Lanczos (sharpest)",
        }
    }
}

fn kernel(r: Resample, x: f32) -> f32 {
    let x = x.abs();
    match r {
        Resample::Nearest => {
            if x < 0.5 {
                1.0
            } else {
                0.0
            }
        }
        Resample::Bilinear => (1.0 - x).max(0.0),
        Resample::Bicubic => {
            let a = -0.5;
            if x < 1.0 {
                (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
            } else if x < 2.0 {
                a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
            } else {
                0.0
            }
        }
        Resample::Lanczos3 => {
            if x < 1e-6 {
                1.0
            } else if x < 3.0 {
                let px = std::f32::consts::PI * x;
                3.0 * px.sin() * (px / 3.0).sin() / (px * px)
            } else {
                0.0
            }
        }
    }
}

fn support(r: Resample) -> f32 {
    match r {
        Resample::Nearest => 0.5,
        Resample::Bilinear => 1.0,
        Resample::Bicubic => 2.0,
        Resample::Lanczos3 => 3.0,
    }
}

/// Separable resample on premultiplied data (no dark fringes), with proper downscale filtering.
pub fn resize(img: &Image, nw: u32, nh: u32, method: Resample) -> Image {
    if nw == 0 || nh == 0 {
        return Image::new(nw.max(1), nh.max(1));
    }
    if nw == img.width && nh == img.height {
        return img.clone();
    }
    let pre: Vec<[f32; 4]> = img
        .pixels()
        .iter()
        .map(|p| [p[0] * p[3], p[1] * p[3], p[2] * p[3], p[3]])
        .collect();
    let tmp = resample_axis(
        &pre,
        img.width as usize,
        img.height as usize,
        nw as usize,
        true,
        method,
    );
    let out = resample_axis(
        &tmp,
        nw as usize,
        img.height as usize,
        nh as usize,
        false,
        method,
    );
    let data = out
        .into_par_iter()
        .map(|p| {
            let a = p[3].clamp(0.0, 1.0);
            if a > 1e-6 {
                [
                    (p[0] / a).clamp(0.0, 1.0),
                    (p[1] / a).clamp(0.0, 1.0),
                    (p[2] / a).clamp(0.0, 1.0),
                    a,
                ]
            } else {
                TRANSPARENT
            }
        })
        .collect();
    Image::from_pixels(nw, nh, data)
}

fn resample_axis(
    src: &[[f32; 4]],
    w: usize,
    h: usize,
    n: usize,
    horizontal: bool,
    method: Resample,
) -> Vec<[f32; 4]> {
    let (len, other) = if horizontal { (w, h) } else { (h, w) };
    let scale = n as f32 / len as f32;
    let filter_scale = if scale < 1.0 { 1.0 / scale } else { 1.0 };
    let sup = support(method) * filter_scale;
    // precompute weights for each output index
    let weights: Vec<(usize, Vec<f32>)> = (0..n)
        .map(|i| {
            let center = (i as f32 + 0.5) / scale;
            let start = (center - sup).floor().max(0.0) as usize;
            let end = ((center + sup).ceil() as usize).min(len);
            let mut ws: Vec<f32> = (start..end)
                .map(|j| kernel(method, (j as f32 + 0.5 - center) / filter_scale))
                .collect();
            let sum: f32 = ws.iter().sum();
            if sum.abs() > 1e-8 {
                for v in ws.iter_mut() {
                    *v /= sum;
                }
            } else if !ws.is_empty() {
                let k = ((center as usize).min(end - 1)).saturating_sub(start);
                ws.iter_mut().for_each(|v| *v = 0.0);
                ws[k] = 1.0;
            }
            (start, ws)
        })
        .collect();
    let (ow, oh) = if horizontal { (n, h) } else { (w, n) };
    let mut out = vec![[0.0f32; 4]; ow * oh];
    if horizontal {
        out.par_chunks_mut(ow).enumerate().for_each(|(y, row)| {
            for (x, o) in row.iter_mut().enumerate() {
                let (start, ws) = &weights[x];
                let mut acc = [0.0f32; 4];
                for (k, wgt) in ws.iter().enumerate() {
                    let p = src[y * w + start + k];
                    for c in 0..4 {
                        acc[c] += p[c] * wgt;
                    }
                }
                *o = acc;
            }
        });
    } else {
        out.par_chunks_mut(ow).enumerate().for_each(|(y, row)| {
            let (start, ws) = &weights[y];
            for (k, wgt) in ws.iter().enumerate() {
                let srow = &src[(start + k) * w..(start + k + 1) * w];
                for x in 0..w {
                    for c in 0..4 {
                        row[x][c] += srow[x][c] * wgt;
                    }
                }
            }
        });
    }
    let _ = other;
    out
}

pub fn flip_h(img: &Image) -> Image {
    let w = img.width as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(w)
        .for_each(|row| row.reverse());
    out
}

pub fn flip_v(img: &Image) -> Image {
    let w = img.width as usize;
    let src = img.pixels();
    let h = img.height as usize;
    let mut out = img.clone();
    out.pixels_mut()
        .par_chunks_mut(w)
        .enumerate()
        .for_each(|(y, row)| row.copy_from_slice(&src[(h - 1 - y) * w..(h - y) * w]));
    out
}

/// Rotate 90° clockwise.
pub fn rotate_cw(img: &Image) -> Image {
    let (w, h) = (img.width as usize, img.height as usize);
    let src = img.pixels();
    let data = (0..w * h).into_par_iter().map(|i| {
        let (x, y) = (i % h, i / h); // new image is h wide, w tall
        src[(h - 1 - x) * w + y]
    });
    Image::from_pixels(h as u32, w as u32, data.collect())
}

pub fn rotate_ccw(img: &Image) -> Image {
    rotate_cw(&rotate_cw(&rotate_cw(img)))
}

pub fn rotate_180(img: &Image) -> Image {
    let mut out = img.clone();
    out.pixels_mut().reverse();
    out
}

/// Arbitrary rotation (degrees, clockwise). If `expand`, the canvas grows to fit.
pub fn rotate(img: &Image, degrees: f32, expand: bool, background: Px) -> Image {
    let a = degrees.to_radians();
    let (s, c) = a.sin_cos();
    let (w, h) = (img.width as f32, img.height as f32);
    let (nw, nh) = if expand {
        (
            (w * c.abs() + h * s.abs()).ceil(),
            (w * s.abs() + h * c.abs()).ceil(),
        )
    } else {
        (w, h)
    };
    let (cx, cy, ncx, ncy) = (w / 2.0, h / 2.0, nw / 2.0, nh / 2.0);
    let nwu = nw as usize;
    let data: Vec<Px> = (0..(nw * nh) as usize)
        .into_par_iter()
        .map(|i| {
            let x = (i % nwu) as f32 + 0.5 - ncx;
            let y = (i / nwu) as f32 + 0.5 - ncy;
            let sx = x * c + y * s + cx;
            let sy = -x * s + y * c + cy;
            let p = img.sample_bilinear_transparent(sx, sy);
            if background[3] > 0.0 && p[3] < 1.0 {
                crate::blend::composite_px(
                    background,
                    p,
                    crate::blend::BlendMode::Normal,
                    1.0,
                    0,
                    0,
                )
            } else {
                p
            }
        })
        .collect();
    Image::from_pixels(nw as u32, nh as u32, data)
}

pub fn crop(img: &Image, r: Rect) -> Image {
    let mut out = Image::new(r.w.max(1) as u32, r.h.max(1) as u32);
    out.blit(img, -r.x, -r.y);
    out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl Anchor {
    pub const GRID: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];
    pub fn offset(self, ow: i32, oh: i32, nw: i32, nh: i32) -> (i32, i32) {
        let i = Anchor::GRID.iter().position(|a| *a == self).unwrap() as i32;
        let (ax, ay) = (i % 3, i / 3);
        ((nw - ow) * ax / 2, (nh - oh) * ay / 2)
    }
}

/// Image > Canvas Size: returns new image with old content placed per anchor.
pub fn canvas_size(img: &Image, nw: u32, nh: u32, anchor: Anchor, fill: Px) -> Image {
    let (dx, dy) = anchor.offset(img.width as i32, img.height as i32, nw as i32, nh as i32);
    let mut out = Image::filled(nw, nh, fill);
    out.blit(img, dx, dy);
    out
}

/// 2D affine matrix [a b c; d e f] mapping (x,y) -> (a x + b y + c, d x + e y + f).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Affine {
    pub m: [f32; 6],
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        m: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };
    pub fn translate(x: f32, y: f32) -> Self {
        Affine {
            m: [1.0, 0.0, x, 0.0, 1.0, y],
        }
    }
    pub fn scale(sx: f32, sy: f32) -> Self {
        Affine {
            m: [sx, 0.0, 0.0, 0.0, sy, 0.0],
        }
    }
    pub fn rotate_deg(d: f32) -> Self {
        let (s, c) = d.to_radians().sin_cos();
        Affine {
            m: [c, -s, 0.0, s, c, 0.0],
        }
    }
    pub fn skew_deg(kx: f32, ky: f32) -> Self {
        Affine {
            m: [
                1.0,
                kx.to_radians().tan(),
                0.0,
                ky.to_radians().tan(),
                1.0,
                0.0,
            ],
        }
    }
    /// self ∘ o  (apply o first, then self)
    pub fn then(self, after: Affine) -> Affine {
        let a = after.m;
        let b = self.m;
        Affine {
            m: [
                a[0] * b[0] + a[1] * b[3],
                a[0] * b[1] + a[1] * b[4],
                a[0] * b[2] + a[1] * b[5] + a[2],
                a[3] * b[0] + a[4] * b[3],
                a[3] * b[1] + a[4] * b[4],
                a[3] * b[2] + a[4] * b[5] + a[5],
            ],
        }
    }
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let m = self.m;
        (m[0] * x + m[1] * y + m[2], m[3] * x + m[4] * y + m[5])
    }
    pub fn inverse(&self) -> Option<Affine> {
        let m = self.m;
        let det = m[0] * m[4] - m[1] * m[3];
        if det.abs() < 1e-9 {
            return None;
        }
        let id = 1.0 / det;
        Some(Affine {
            m: [
                m[4] * id,
                -m[1] * id,
                (m[1] * m[5] - m[4] * m[2]) * id,
                -m[3] * id,
                m[0] * id,
                (m[3] * m[2] - m[0] * m[5]) * id,
            ],
        })
    }
    /// Scale/rotate about a pivot then translate - the Free Transform parameterisation.
    pub fn free(pivot: (f32, f32), sx: f32, sy: f32, rot_deg: f32, tx: f32, ty: f32) -> Affine {
        Affine::translate(-pivot.0, -pivot.1)
            .then(Affine::scale(sx, sy))
            .then(Affine::rotate_deg(rot_deg))
            .then(Affine::translate(pivot.0 + tx, pivot.1 + ty))
    }
}

/// Apply an affine transform to `img` into a canvas of the same size (layer stays doc-sized).
pub fn transform_affine(img: &Image, t: &Affine, out_w: u32, out_h: u32) -> Image {
    let inv = match t.inverse() {
        Some(i) => i,
        None => return Image::new(out_w, out_h),
    };
    // supersample when shrinking a lot to avoid aliasing
    let m = t.m;
    let scale = ((m[0] * m[4] - m[1] * m[3]).abs()).sqrt();
    let src = if scale < 0.5 {
        let f = scale.max(0.05);
        let small = resize(
            img,
            ((img.width as f32 * f).ceil() as u32).max(1),
            ((img.height as f32 * f).ceil() as u32).max(1),
            Resample::Bilinear,
        );
        let adj = Affine::scale(
            img.width as f32 / small.width as f32,
            img.height as f32 / small.height as f32,
        );
        return transform_affine(&small, &adj.then(*t), out_w, out_h);
    } else {
        img
    };
    let w = out_w as usize;
    let data = (0..(out_w * out_h) as usize)
        .into_par_iter()
        .map(|i| {
            let (sx, sy) = inv.apply((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            src.sample_bilinear_transparent(sx, sy)
        })
        .collect();
    Image::from_pixels(out_w, out_h, data)
}

/// Homography from 4 source points to 4 destination points (projective/perspective transform).
pub fn homography(src: [(f32, f32); 4], dst: [(f32, f32); 4]) -> Option<[f32; 9]> {
    let mut a = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let (x, y) = (src[i].0 as f64, src[i].1 as f64);
        let (u, v) = (dst[i].0 as f64, dst[i].1 as f64);
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    // Gaussian elimination
    for col in 0..8 {
        let piv =
            (col..8).max_by(|&i, &j| a[i][col].abs().partial_cmp(&a[j][col].abs()).unwrap())?;
        if a[piv][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, piv);
        for r in 0..8 {
            if r != col {
                let f = a[r][col] / a[col][col];
                for c in col..9 {
                    a[r][c] -= f * a[col][c];
                }
            }
        }
    }
    let mut h = [0.0f32; 9];
    for i in 0..8 {
        h[i] = (a[i][8] / a[i][i]) as f32;
    }
    h[8] = 1.0;
    Some(h)
}

/// Edit > Transform > Distort / Perspective: map the image corners to `corners` (TL, TR, BR, BL).
pub fn warp_perspective(img: &Image, corners: [(f32, f32); 4], out_w: u32, out_h: u32) -> Image {
    let (w, h) = (img.width as f32, img.height as f32);
    let src = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)];
    // inverse map: destination -> source
    let hm = match homography(corners, src) {
        Some(m) => m,
        None => return Image::new(out_w, out_h),
    };
    let ow = out_w as usize;
    let data = (0..(out_w * out_h) as usize)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % ow) as f32 + 0.5, (i / ow) as f32 + 0.5);
            let d = hm[6] * x + hm[7] * y + hm[8];
            if d.abs() < 1e-9 {
                return TRANSPARENT;
            }
            let sx = (hm[0] * x + hm[1] * y + hm[2]) / d;
            let sy = (hm[3] * x + hm[4] * y + hm[5]) / d;
            img.sample_bilinear_transparent(sx, sy)
        })
        .collect();
    Image::from_pixels(out_w, out_h, data)
}

/// Liquify: a backward displacement field (source offset per pixel).
#[derive(Clone, Debug)]
pub struct Liquify {
    pub w: u32,
    pub h: u32,
    pub field: Vec<(f32, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LiquifyMode {
    ForwardWarp,
    Pucker,
    Bloat,
    TwirlClockwise,
    Reconstruct,
}

impl Liquify {
    pub fn new(w: u32, h: u32) -> Self {
        Liquify {
            w,
            h,
            field: vec![(0.0, 0.0); (w * h) as usize],
        }
    }
    /// Apply a brush stroke segment from (x0,y0) to (x1,y1).
    pub fn stroke(
        &mut self,
        mode: LiquifyMode,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        radius: f32,
        pressure: f32,
    ) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let r = radius.max(1.0);
        let xs = ((x1 - r).floor().max(0.0) as u32)..((x1 + r).ceil().min(self.w as f32) as u32);
        let ys = ((y1 - r).floor().max(0.0) as u32)..((y1 + r).ceil().min(self.h as f32) as u32);
        let old = self.field.clone();
        for y in ys {
            for x in xs.clone() {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = ((px - x1).powi(2) + (py - y1).powi(2)).sqrt() / r;
                if d >= 1.0 {
                    continue;
                }
                let f = (1.0 - d * d).powi(2) * pressure;
                let i = (y * self.w + x) as usize;
                let cur = self.field[i];
                let n = match mode {
                    LiquifyMode::ForwardWarp => {
                        // sample the field at the upstream point, then offset
                        let (ux, uy) = (px - dx * f, py - dy * f);
                        let up = self.sample_old(&old, ux, uy);
                        (up.0 + (ux - px), up.1 + (uy - py))
                    }
                    LiquifyMode::Pucker => {
                        (cur.0 + (px - x1) * f * 0.08, cur.1 + (py - y1) * f * 0.08)
                    }
                    LiquifyMode::Bloat => {
                        (cur.0 - (px - x1) * f * 0.08, cur.1 - (py - y1) * f * 0.08)
                    }
                    LiquifyMode::TwirlClockwise => {
                        let a = 0.08 * f;
                        let (vx, vy) = (px - x1, py - y1);
                        (
                            cur.0 + (vx * a.cos() - vy * a.sin() - vx),
                            cur.1 + (vx * a.sin() + vy * a.cos() - vy),
                        )
                    }
                    LiquifyMode::Reconstruct => (cur.0 * (1.0 - f * 0.3), cur.1 * (1.0 - f * 0.3)),
                };
                self.field[i] = n;
            }
        }
        let _ = (x0, y0);
    }
    fn sample_old(&self, old: &[(f32, f32)], x: f32, y: f32) -> (f32, f32) {
        let xi = (x.floor() as i32).clamp(0, self.w as i32 - 1) as usize;
        let yi = (y.floor() as i32).clamp(0, self.h as i32 - 1) as usize;
        old[yi * self.w as usize + xi]
    }
    pub fn render(&self, img: &Image) -> Image {
        let w = self.w as usize;
        let data = (0..(self.w * self.h) as usize)
            .into_par_iter()
            .map(|i| {
                let (x, y) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
                let d = self.field[i];
                img.sample_bilinear(x + d.0, y + d.1)
            })
            .collect();
        Image::from_pixels(self.w, self.h, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotate_roundtrip() {
        let mut img = Image::new(5, 3);
        img.set(1, 0, [1.0; 4]);
        let r = rotate_cw(&img);
        assert_eq!((r.width, r.height), (3, 5));
        assert_eq!(r.get(2, 1)[0], 1.0);
        let back = rotate_ccw(&r);
        assert_eq!(back.get(1, 0)[0], 1.0);
    }
    #[test]
    fn resize_flat() {
        let img = Image::filled(37, 21, [0.3, 0.6, 0.9, 1.0]);
        for m in Resample::ALL {
            let r = resize(&img, 80, 9, m);
            let p = r.get(40, 4);
            assert!((p[1] - 0.6).abs() < 1e-3, "{m:?} {p:?}");
        }
    }
    #[test]
    fn affine_inverse() {
        let t = Affine::free((10.0, 10.0), 2.0, 0.5, 30.0, 5.0, -3.0);
        let (x, y) = t.apply(3.0, 7.0);
        let (bx, by) = t.inverse().unwrap().apply(x, y);
        assert!((bx - 3.0).abs() < 1e-3 && (by - 7.0).abs() < 1e-3);
    }
    #[test]
    fn homography_identity() {
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let h = homography(pts, pts).unwrap();
        assert!((h[0] - 1.0).abs() < 1e-4 && h[1].abs() < 1e-4);
    }
}
