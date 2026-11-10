//! Pixel buffers. `Image` and `Mask` share storage through `Arc` with copy-on-write,
//! which makes history snapshots cheap: untouched layers are never duplicated.

use crate::color::{Color, Px, TRANSPARENT};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn from_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let (ax, bx) = (x0.min(x1), x0.max(x1));
        let (ay, by) = (y0.min(y1), y0.max(y1));
        Rect::new(ax, ay, bx - ax, by - ay)
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Rect::from_points(
            self.x.min(o.x),
            self.y.min(o.y),
            self.right().max(o.right()),
            self.bottom().max(o.bottom()),
        )
    }
    pub fn inflate(&self, d: i32) -> Rect {
        Rect::new(self.x - d, self.y - d, self.w + 2 * d, self.h + 2 * d)
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.right() && y < self.bottom()
    }
}

/// RGBA float image, straight alpha.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    data: Arc<Vec<Px>>,
}

impl Default for Image {
    fn default() -> Self {
        Image::new(0, 0)
    }
}

impl PartialEq for Image {
    fn eq(&self, o: &Self) -> bool {
        self.width == o.width
            && self.height == o.height
            && (Arc::ptr_eq(&self.data, &o.data) || self.data == o.data)
    }
}

impl Image {
    pub fn new(width: u32, height: u32) -> Self {
        Self::filled(width, height, TRANSPARENT)
    }
    pub fn filled(width: u32, height: u32, px: Px) -> Self {
        Image {
            width,
            height,
            data: Arc::new(vec![px; (width as usize) * (height as usize)]),
        }
    }
    pub fn from_pixels(width: u32, height: u32, data: Vec<Px>) -> Self {
        assert_eq!(data.len(), width as usize * height as usize);
        Image {
            width,
            height,
            data: Arc::new(data),
        }
    }
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn pixels(&self) -> &[Px] {
        &self.data
    }
    /// Mutable access; clones the buffer if it is shared with a history snapshot.
    pub fn pixels_mut(&mut self) -> &mut [Px] {
        Arc::make_mut(&mut self.data).as_mut_slice()
    }
    pub fn shares_storage(&self, o: &Image) -> bool {
        Arc::ptr_eq(&self.data, &o.data)
    }
    #[inline]
    pub fn idx(&self, x: u32, y: u32) -> usize {
        y as usize * self.width as usize + x as usize
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> Px {
        self.data[self.idx(x, y)]
    }
    #[inline]
    pub fn get_i(&self, x: i32, y: i32) -> Px {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            TRANSPARENT
        } else {
            self.data[y as usize * self.width as usize + x as usize]
        }
    }
    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> Px {
        let x = x.clamp(0, self.width as i32 - 1) as u32;
        let y = y.clamp(0, self.height as i32 - 1) as u32;
        self.get(x, y)
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, p: Px) {
        let i = self.idx(x, y);
        self.pixels_mut()[i] = p;
    }
    /// Bilinear sample with edge clamping, coordinates in pixel space (pixel centers at +0.5).
    pub fn sample_bilinear(&self, x: f32, y: f32) -> Px {
        let fx = x - 0.5;
        let fy = y - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (x0, y0) = (x0 as i32, y0 as i32);
        let a = self.get_clamped(x0, y0);
        let b = self.get_clamped(x0 + 1, y0);
        let c = self.get_clamped(x0, y0 + 1);
        let d = self.get_clamped(x0 + 1, y0 + 1);
        premul_lerp4(a, b, c, d, tx, ty)
    }
    /// Bilinear sample returning transparent outside the image.
    pub fn sample_bilinear_transparent(&self, x: f32, y: f32) -> Px {
        let fx = x - 0.5;
        let fy = y - 0.5;
        let x0 = fx.floor();
        let y0 = fy.floor();
        let tx = fx - x0;
        let ty = fy - y0;
        let (x0, y0) = (x0 as i32, y0 as i32);
        premul_lerp4(
            self.get_i(x0, y0),
            self.get_i(x0 + 1, y0),
            self.get_i(x0, y0 + 1),
            self.get_i(x0 + 1, y0 + 1),
            tx,
            ty,
        )
    }
    pub fn fill(&mut self, c: Color) {
        let p = c.px();
        self.pixels_mut().par_iter_mut().for_each(|v| *v = p);
    }
    pub fn clear(&mut self) {
        self.fill(Color::TRANSPARENT)
    }
    pub fn sub_image(&self, r: Rect) -> Image {
        let r = r.intersect(&self.bounds());
        let mut out = Image::new(r.w.max(0) as u32, r.h.max(0) as u32);
        let w = out.width as usize;
        if w == 0 {
            return out;
        }
        let src = self.pixels();
        let sw = self.width as usize;
        out.pixels_mut()
            .par_chunks_mut(w)
            .enumerate()
            .for_each(|(y, row)| {
                let s = (r.y as usize + y) * sw + r.x as usize;
                row.copy_from_slice(&src[s..s + w]);
            });
        out
    }
    /// Paste `src` with its top-left at (dx,dy), replacing pixels.
    pub fn blit(&mut self, src: &Image, dx: i32, dy: i32) {
        let dst_r =
            Rect::new(dx, dy, src.width as i32, src.height as i32).intersect(&self.bounds());
        if dst_r.is_empty() {
            return;
        }
        let w = self.width as usize;
        let sw = src.width as usize;
        let sp = src.pixels();
        let dp = self.pixels_mut();
        for y in dst_r.y..dst_r.bottom() {
            let sy = (y - dy) as usize;
            let sx = (dst_r.x - dx) as usize;
            let di = y as usize * w + dst_r.x as usize;
            let n = dst_r.w as usize;
            dp[di..di + n].copy_from_slice(&sp[sy * sw + sx..sy * sw + sx + n]);
        }
    }
    /// Bounding box of pixels with alpha > threshold.
    pub fn opaque_bounds(&self, threshold: f32) -> Rect {
        let (w, h) = (self.width as i32, self.height as i32);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, -1, -1);
        for y in 0..h {
            for x in 0..w {
                if self.data[(y * w + x) as usize][3] > threshold {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        if x1 < 0 {
            Rect::default()
        } else {
            Rect::from_points(x0, y0, x1 + 1, y1 + 1)
        }
    }
    pub fn to_rgba8(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.data.len() * 4];
        out.par_chunks_mut(4)
            .zip(self.data.par_iter())
            .for_each(|(o, p)| {
                o.copy_from_slice(&Color::from_px(*p).to_rgba8());
            });
        out
    }
    pub fn from_rgba8(width: u32, height: u32, bytes: &[u8]) -> Self {
        let data = bytes
            .par_chunks(4)
            .map(|c| {
                [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ]
            })
            .collect();
        Image::from_pixels(width, height, data)
    }
    pub fn to_rgba16(&self) -> Vec<u16> {
        self.data
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16))
            .collect()
    }
    pub fn from_rgba16(width: u32, height: u32, v: &[u16]) -> Self {
        let data = v
            .chunks(4)
            .map(|c| [0, 1, 2, 3].map(|i| c[i] as f32 / 65535.0))
            .collect();
        Image::from_pixels(width, height, data)
    }
    pub fn from_dynamic(img: &image::DynamicImage) -> Self {
        let r = img.to_rgba32f();
        let (w, h) = r.dimensions();
        let data = r
            .into_raw()
            .chunks(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        Image::from_pixels(w, h, data)
    }
    pub fn to_dynamic8(&self) -> image::DynamicImage {
        image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_raw(self.width, self.height, self.to_rgba8()).expect("size"),
        )
    }
    pub fn to_dynamic16(&self) -> image::DynamicImage {
        image::DynamicImage::ImageRgba16(
            image::ImageBuffer::from_raw(self.width, self.height, self.to_rgba16()).expect("size"),
        )
    }
}

fn premul_lerp4(a: Px, b: Px, c: Px, d: Px, tx: f32, ty: f32) -> Px {
    let wa = (1.0 - tx) * (1.0 - ty);
    let wb = tx * (1.0 - ty);
    let wc = (1.0 - tx) * ty;
    let wd = tx * ty;
    let alpha = a[3] * wa + b[3] * wb + c[3] * wc + d[3] * wd;
    if alpha <= 1e-7 {
        return TRANSPARENT;
    }
    let mut out = [0.0; 4];
    for i in 0..3 {
        out[i] =
            (a[i] * a[3] * wa + b[i] * b[3] * wb + c[i] * c[3] * wc + d[i] * d[3] * wd) / alpha;
    }
    out[3] = alpha;
    out
}

/// Single-channel float mask (selections, layer masks). 1 = selected / visible.
#[derive(Clone, Debug)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    data: Arc<Vec<f32>>,
}

impl PartialEq for Mask {
    fn eq(&self, o: &Self) -> bool {
        self.width == o.width
            && self.height == o.height
            && (Arc::ptr_eq(&self.data, &o.data) || self.data == o.data)
    }
}

impl Mask {
    pub fn new(width: u32, height: u32, v: f32) -> Self {
        Mask {
            width,
            height,
            data: Arc::new(vec![v; width as usize * height as usize]),
        }
    }
    pub fn from_vec(width: u32, height: u32, data: Vec<f32>) -> Self {
        assert_eq!(data.len(), width as usize * height as usize);
        Mask {
            width,
            height,
            data: Arc::new(data),
        }
    }
    pub fn same_storage(&self, o: &Mask) -> bool {
        Arc::ptr_eq(&self.data, &o.data)
    }
    pub fn data(&self) -> &[f32] {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut [f32] {
        Arc::make_mut(&mut self.data).as_mut_slice()
    }
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }
    #[inline]
    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.data[y as usize * self.width as usize + x as usize]
    }
    #[inline]
    pub fn get_i(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            0.0
        } else {
            self.data[y as usize * self.width as usize + x as usize]
        }
    }
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, v: f32) {
        let w = self.width as usize;
        self.data_mut()[y as usize * w + x as usize] = v;
    }
    pub fn is_empty_selection(&self) -> bool {
        !self.data.par_iter().any(|v| *v > 0.002)
    }
    /// Bounding box of values > threshold.
    pub fn bounds_above(&self, threshold: f32) -> Rect {
        let (w, h) = (self.width as i32, self.height as i32);
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, -1, -1);
        for y in 0..h {
            let row = &self.data[(y * w) as usize..((y + 1) * w) as usize];
            if let Some(first) = row.iter().position(|v| *v > threshold) {
                let last = row.iter().rposition(|v| *v > threshold).unwrap();
                x0 = x0.min(first as i32);
                x1 = x1.max(last as i32);
                y0 = y0.min(y);
                y1 = y1.max(y);
            }
        }
        if x1 < 0 {
            Rect::default()
        } else {
            Rect::from_points(x0, y0, x1 + 1, y1 + 1)
        }
    }
    pub fn to_image(&self) -> Image {
        Image::from_pixels(
            self.width,
            self.height,
            self.data.iter().map(|v| [*v, *v, *v, 1.0]).collect(),
        )
    }
    pub fn from_image_luma(img: &Image) -> Mask {
        Mask::from_vec(
            img.width,
            img.height,
            img.pixels()
                .iter()
                .map(|p| crate::color::luma709([p[0], p[1], p[2]]) * p[3])
                .collect(),
        )
    }
    pub fn from_alpha(img: &Image) -> Mask {
        Mask::from_vec(
            img.width,
            img.height,
            img.pixels().iter().map(|p| p[3]).collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cow_snapshots() {
        let a = Image::new(4, 4);
        let mut b = a.clone();
        assert!(a.shares_storage(&b));
        b.set(1, 1, [1.0; 4]);
        assert!(!a.shares_storage(&b));
        assert_eq!(a.get(1, 1)[3], 0.0);
    }
    #[test]
    fn rect_ops() {
        let r = Rect::new(0, 0, 10, 10).intersect(&Rect::new(5, 5, 10, 10));
        assert_eq!(r, Rect::new(5, 5, 5, 5));
    }
}
