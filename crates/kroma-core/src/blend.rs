//! All 27 Photoshop blend modes plus W3C-style alpha compositing.

use crate::color::{clamp01, lum, sat, set_lum, set_sat, Px};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Dissolve,
    // Darken group
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    // Lighten group
    Lighten,
    Screen,
    ColorDodge,
    LinearDodge,
    LighterColor,
    // Contrast group
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    // Inversion group
    Difference,
    Exclusion,
    Subtract,
    Divide,
    // Component group
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// Grouped exactly like Photoshop's blend-mode dropdown (None = separator).
    pub const MENU: &'static [Option<BlendMode>] = &[
        Some(BlendMode::Normal),
        Some(BlendMode::Dissolve),
        None,
        Some(BlendMode::Darken),
        Some(BlendMode::Multiply),
        Some(BlendMode::ColorBurn),
        Some(BlendMode::LinearBurn),
        Some(BlendMode::DarkerColor),
        None,
        Some(BlendMode::Lighten),
        Some(BlendMode::Screen),
        Some(BlendMode::ColorDodge),
        Some(BlendMode::LinearDodge),
        Some(BlendMode::LighterColor),
        None,
        Some(BlendMode::Overlay),
        Some(BlendMode::SoftLight),
        Some(BlendMode::HardLight),
        Some(BlendMode::VividLight),
        Some(BlendMode::LinearLight),
        Some(BlendMode::PinLight),
        Some(BlendMode::HardMix),
        None,
        Some(BlendMode::Difference),
        Some(BlendMode::Exclusion),
        Some(BlendMode::Subtract),
        Some(BlendMode::Divide),
        None,
        Some(BlendMode::Hue),
        Some(BlendMode::Saturation),
        Some(BlendMode::Color),
        Some(BlendMode::Luminosity),
    ];

    pub fn all() -> impl Iterator<Item = BlendMode> {
        Self::MENU.iter().flatten().copied()
    }

    pub fn name(self) -> &'static str {
        use BlendMode::*;
        match self {
            Normal => "Normal",
            Dissolve => "Dissolve",
            Darken => "Darken",
            Multiply => "Multiply",
            ColorBurn => "Color Burn",
            LinearBurn => "Linear Burn",
            DarkerColor => "Darker Color",
            Lighten => "Lighten",
            Screen => "Screen",
            ColorDodge => "Color Dodge",
            LinearDodge => "Linear Dodge (Add)",
            LighterColor => "Lighter Color",
            Overlay => "Overlay",
            SoftLight => "Soft Light",
            HardLight => "Hard Light",
            VividLight => "Vivid Light",
            LinearLight => "Linear Light",
            PinLight => "Pin Light",
            HardMix => "Hard Mix",
            Difference => "Difference",
            Exclusion => "Exclusion",
            Subtract => "Subtract",
            Divide => "Divide",
            Hue => "Hue",
            Saturation => "Saturation",
            Color => "Color",
            Luminosity => "Luminosity",
        }
    }

    /// OpenRaster `composite-op` names (svg: namespace) for interchange with GIMP/Krita.
    pub fn ora_name(self) -> &'static str {
        use BlendMode::*;
        match self {
            Normal | Dissolve => "svg:src-over",
            Darken => "svg:darken",
            Multiply => "svg:multiply",
            ColorBurn => "svg:color-burn",
            Lighten => "svg:lighten",
            Screen => "svg:screen",
            ColorDodge => "svg:color-dodge",
            LinearDodge => "svg:plus",
            Overlay => "svg:overlay",
            SoftLight => "svg:soft-light",
            HardLight => "svg:hard-light",
            Difference => "svg:difference",
            Exclusion => "svg:exclusion",
            Hue => "svg:hue",
            Saturation => "svg:saturation",
            Color => "svg:color",
            Luminosity => "svg:luminosity",
            // Kroma extensions, still round-trip through Kroma itself
            LinearBurn => "kroma:linear-burn",
            DarkerColor => "kroma:darker-color",
            LighterColor => "kroma:lighter-color",
            VividLight => "kroma:vivid-light",
            LinearLight => "kroma:linear-light",
            PinLight => "kroma:pin-light",
            HardMix => "kroma:hard-mix",
            Subtract => "kroma:subtract",
            Divide => "kroma:divide",
        }
    }

    pub fn from_ora_name(s: &str) -> BlendMode {
        BlendMode::all()
            .find(|m| m.ora_name() == s && *m != BlendMode::Dissolve)
            .unwrap_or(BlendMode::Normal)
    }

    /// Map from the `psd` crate's blend mode discriminant (PassThrough = 0 .. Luminosity = 27).
    pub fn from_psd_code(code: u8) -> BlendMode {
        use BlendMode::*;
        const TABLE: [BlendMode; 28] = [
            Normal,
            Normal,
            Dissolve,
            Darken,
            Multiply,
            ColorBurn,
            LinearBurn,
            DarkerColor,
            Lighten,
            Screen,
            ColorDodge,
            LinearDodge,
            LighterColor,
            Overlay,
            SoftLight,
            HardLight,
            VividLight,
            LinearLight,
            PinLight,
            HardMix,
            Difference,
            Exclusion,
            Subtract,
            Divide,
            Hue,
            Saturation,
            Color,
            Luminosity,
        ];
        TABLE.get(code as usize).copied().unwrap_or(Normal)
    }
}

#[inline]
fn sep(f: impl Fn(f32, f32) -> f32, b: [f32; 3], s: [f32; 3]) -> [f32; 3] {
    [f(b[0], s[0]), f(b[1], s[1]), f(b[2], s[2])]
}

fn color_burn(b: f32, s: f32) -> f32 {
    if b >= 1.0 {
        1.0
    } else if s <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - b) / s).min(1.0)
    }
}
fn color_dodge(b: f32, s: f32) -> f32 {
    if b <= 0.0 {
        0.0
    } else if s >= 1.0 {
        1.0
    } else {
        (b / (1.0 - s)).min(1.0)
    }
}
fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b * 2.0 * s
    } else {
        let s2 = 2.0 * s - 1.0;
        b + s2 - b * s2
    }
}
fn soft_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b - (1.0 - 2.0 * s) * b * (1.0 - b)
    } else {
        let d = if b <= 0.25 {
            ((16.0 * b - 12.0) * b + 4.0) * b
        } else {
            b.sqrt()
        };
        b + (2.0 * s - 1.0) * (d - b)
    }
}
fn vivid_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        color_burn(b, 2.0 * s)
    } else {
        color_dodge(b, 2.0 * (s - 0.5))
    }
}
fn pin_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 {
        b.min(2.0 * s)
    } else {
        b.max(2.0 * s - 1.0)
    }
}

/// Blend function B(cb, cs): backdrop and source colors -> mixed color.
pub fn blend_rgb(mode: BlendMode, b: [f32; 3], s: [f32; 3]) -> [f32; 3] {
    use BlendMode::*;
    match mode {
        Normal | Dissolve => s,
        Darken => sep(f32::min, b, s),
        Multiply => sep(|b, s| b * s, b, s),
        ColorBurn => sep(color_burn, b, s),
        LinearBurn => sep(|b, s| clamp01(b + s - 1.0), b, s),
        DarkerColor => {
            if s[0] + s[1] + s[2] < b[0] + b[1] + b[2] {
                s
            } else {
                b
            }
        }
        Lighten => sep(f32::max, b, s),
        Screen => sep(|b, s| b + s - b * s, b, s),
        ColorDodge => sep(color_dodge, b, s),
        LinearDodge => sep(|b, s| (b + s).min(1.0), b, s),
        LighterColor => {
            if s[0] + s[1] + s[2] > b[0] + b[1] + b[2] {
                s
            } else {
                b
            }
        }
        Overlay => sep(|b, s| hard_light(s, b), b, s),
        SoftLight => sep(soft_light, b, s),
        HardLight => sep(hard_light, b, s),
        VividLight => sep(vivid_light, b, s),
        LinearLight => sep(|b, s| clamp01(b + 2.0 * s - 1.0), b, s),
        PinLight => sep(pin_light, b, s),
        HardMix => sep(|b, s| if b + s >= 1.0 { 1.0 } else { 0.0 }, b, s),
        Difference => sep(|b, s| (b - s).abs(), b, s),
        Exclusion => sep(|b, s| b + s - 2.0 * b * s, b, s),
        Subtract => sep(|b, s| (b - s).max(0.0), b, s),
        Divide => sep(
            |b, s| {
                if s <= 0.0 {
                    if b > 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                } else {
                    (b / s).min(1.0)
                }
            },
            b,
            s,
        ),
        Hue => set_lum(set_sat(s, sat(b)), lum(b)),
        Saturation => set_lum(set_sat(b, sat(s)), lum(b)),
        Color => set_lum(s, lum(b)),
        Luminosity => set_lum(b, lum(s)),
    }
}

#[inline]
fn hash01(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ y.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0xC2B2_AE3D);
    h ^= h >> 13;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Composite `src` over `dst` with `mode` and an extra `opacity` multiplier (0..1).
/// (x,y) are only used by Dissolve.
#[inline]
pub fn composite_px(dst: Px, src: Px, mode: BlendMode, opacity: f32, x: u32, y: u32) -> Px {
    let mut sa = src[3] * opacity;
    if sa <= 0.0 {
        return dst;
    }
    if mode == BlendMode::Dissolve {
        sa = if hash01(x, y) < sa { 1.0 } else { return dst };
    }
    let ba = dst[3];
    let cs = [src[0], src[1], src[2]];
    let cb = [dst[0], dst[1], dst[2]];
    let mixed = if ba > 0.0 {
        blend_rgb(mode, cb, cs)
    } else {
        cs
    };
    let ao = sa + ba * (1.0 - sa);
    if ao <= 1e-7 {
        return [0.0; 4];
    }
    let mut out = [0.0; 4];
    for i in 0..3 {
        let c = sa * (1.0 - ba) * cs[i] + sa * ba * mixed[i] + (1.0 - sa) * ba * cb[i];
        out[i] = clamp01(c / ao);
    }
    out[3] = clamp01(ao);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normal_over_opaque() {
        let r = composite_px(
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            BlendMode::Normal,
            0.5,
            0,
            0,
        );
        assert!(
            (r[0] - 0.5).abs() < 1e-5 && (r[2] - 0.5).abs() < 1e-5 && (r[3] - 1.0).abs() < 1e-5
        );
    }
    #[test]
    fn multiply_white_is_identity() {
        let b = [0.3, 0.6, 0.9];
        assert_eq!(blend_rgb(BlendMode::Multiply, b, [1.0, 1.0, 1.0]), b);
        assert_eq!(blend_rgb(BlendMode::Screen, b, [0.0, 0.0, 0.0]), b);
    }
    #[test]
    fn all_modes_stay_in_range() {
        for m in BlendMode::all() {
            for &b in &[0.0, 0.2, 0.5, 0.8, 1.0] {
                for &s in &[0.0, 0.3, 0.5, 0.7, 1.0] {
                    let r = blend_rgb(m, [b, 1.0 - b, b * 0.5], [s, s * 0.3, 1.0 - s]);
                    for v in r {
                        assert!((-1e-4..=1.0001).contains(&v), "{m:?} {b} {s} -> {r:?}");
                    }
                }
            }
        }
        assert_eq!(BlendMode::all().count(), 27);
    }
}
