//! Color math: pixel type, HSL/HSV conversions, luminance, sRGB transfer, color temperature.

use serde::{Deserialize, Serialize};

/// Straight-alpha RGBA pixel, all channels 0..=1, RGB in sRGB-encoded space
/// (matches the classic Photoshop 8/16-bit working behaviour).
pub type Px = [f32; 4];

pub const TRANSPARENT: Px = [0.0, 0.0, 0.0, 0.0];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Color::BLACK
    }
}

impl Color {
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Color { r, g, b, a: 1.0 }
    }
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }
    }
    pub fn from_rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color::rgba(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            a as f32 / 255.0,
        )
    }
    pub fn to_rgba8(self) -> [u8; 4] {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        [q(self.r), q(self.g), q(self.b), q(self.a)]
    }
    pub fn px(self) -> Px {
        [self.r, self.g, self.b, self.a]
    }
    pub fn from_px(p: Px) -> Self {
        Color::rgba(p[0], p[1], p[2], p[3])
    }
    /// Parse "#rrggbb", "#rrggbbaa", "rrggbb".
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
        match s.len() {
            6 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, 255)),
            8 => Some(Color::from_rgba8(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            3 => {
                let n = |i: usize| {
                    u8::from_str_radix(s.get(i..i + 1)?, 16)
                        .ok()
                        .map(|v| v * 17)
                };
                Some(Color::from_rgba8(n(0)?, n(1)?, n(2)?, 255))
            }
            _ => None,
        }
    }
    pub fn to_hex(self) -> String {
        let [r, g, b, a] = self.to_rgba8();
        if a == 255 {
            format!("#{r:02x}{g:02x}{b:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        }
    }
    pub fn lerp(self, o: Color, t: f32) -> Color {
        Color::rgba(
            self.r + (o.r - self.r) * t,
            self.g + (o.g - self.g) * t,
            self.b + (o.b - self.b) * t,
            self.a + (o.a - self.a) * t,
        )
    }
}

#[inline]
pub fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Rec.601-ish luminosity weights used by Photoshop's non-separable blend modes.
#[inline]
pub fn lum(c: [f32; 3]) -> f32 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// Perceptual luma (Rec.709) for histograms, B&W, thresholds.
#[inline]
pub fn luma709(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

#[inline]
pub fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 {
        let d = l - n;
        if d > 1e-6 {
            for v in out.iter_mut() {
                *v = l + (*v - l) * l / d;
            }
        }
    }
    if x > 1.0 {
        let d = x - l;
        if d > 1e-6 {
            for v in out.iter_mut() {
                *v = l + (*v - l) * (1.0 - l) / d;
            }
        }
    }
    out
}

pub fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

pub fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let range = mx - mn;
    let mut out = [0.0; 3];
    if range > 1e-6 {
        for i in 0..3 {
            out[i] = (c[i] - mn) * s / range;
        }
    }
    out
}

/// RGB (0..1) -> HSL (h in 0..360, s,l 0..1)
pub fn rgb_to_hsl(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let l = (mx + mn) * 0.5;
    if (mx - mn).abs() < 1e-7 {
        return [0.0, 0.0, l];
    }
    let d = mx - mn;
    let s = if l > 0.5 {
        d / (2.0 - mx - mn)
    } else {
        d / (mx + mn)
    };
    let h = if mx == c[0] {
        (c[1] - c[2]) / d + if c[1] < c[2] { 6.0 } else { 0.0 }
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h * 60.0, s, l]
}

pub fn hsl_to_rgb(hsl: [f32; 3]) -> [f32; 3] {
    let [h, s, l] = hsl;
    if s <= 0.0 {
        return [l, l, l];
    }
    let h = h.rem_euclid(360.0) / 360.0;
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 {
            t += 1.0
        }
        if t > 1.0 {
            t -= 1.0
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

/// RGB -> HSV (h 0..360, s,v 0..1)
pub fn rgb_to_hsv(c: [f32; 3]) -> [f32; 3] {
    let mx = c[0].max(c[1]).max(c[2]);
    let mn = c[0].min(c[1]).min(c[2]);
    let d = mx - mn;
    let s = if mx > 0.0 { d / mx } else { 0.0 };
    if d < 1e-7 {
        return [0.0, s, mx];
    }
    let h = if mx == c[0] {
        ((c[1] - c[2]) / d).rem_euclid(6.0)
    } else if mx == c[1] {
        (c[2] - c[0]) / d + 2.0
    } else {
        (c[0] - c[1]) / d + 4.0
    };
    [h * 60.0, s, mx]
}

pub fn hsv_to_rgb(hsv: [f32; 3]) -> [f32; 3] {
    let [h, s, v] = hsv;
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

#[inline]
pub fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[inline]
pub fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055
    }
}

/// Approximate white point of a blackbody at `kelvin` (1000..40000), normalised RGB 0..1.
pub fn kelvin_to_rgb(kelvin: f32) -> [f32; 3] {
    let t = kelvin.clamp(1000.0, 40000.0) / 100.0;
    let r = if t <= 66.0 {
        255.0
    } else {
        329.698_73 * (t - 60.0).powf(-0.133_204_76)
    };
    let g = if t <= 66.0 {
        99.470_8 * t.ln() - 161.119_57
    } else {
        288.122_16 * (t - 60.0).powf(-0.075_514_85)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.517_73 * (t - 10.0).ln() - 305.044_8
    };
    [clamp01(r / 255.0), clamp01(g / 255.0), clamp01(b / 255.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hsl_roundtrip() {
        for c in [
            [0.2, 0.5, 0.9],
            [1.0, 0.0, 0.0],
            [0.3, 0.3, 0.3],
            [0.9, 0.8, 0.1],
        ] {
            let back = hsl_to_rgb(rgb_to_hsl(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-4, "{c:?} -> {back:?}");
            }
            let back = hsv_to_rgb(rgb_to_hsv(c));
            for i in 0..3 {
                assert!((back[i] - c[i]).abs() < 1e-4, "{c:?} -> {back:?}");
            }
        }
    }
    #[test]
    fn hex() {
        assert_eq!(Color::from_hex("#ff8000").unwrap().to_hex(), "#ff8000");
        assert_eq!(Color::from_hex("fff").unwrap().to_hex(), "#ffffff");
    }
}
