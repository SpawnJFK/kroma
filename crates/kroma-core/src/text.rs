//! Text layers: font loading, multi-line layout with alignment/tracking/leading, rasterization.

use crate::blend::{composite_px, BlendMode};
use crate::buffer::Image;
use crate::color::Color;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum FontChoice {
    #[default]
    Sans,
    Mono,
    /// Path to a .ttf / .otf on disk.
    File(String),
}

impl FontChoice {
    pub fn label(&self) -> String {
        match self {
            FontChoice::Sans => "Ubuntu Light (built-in)".into(),
            FontChoice::Mono => "Hack (built-in)".into(),
            FontChoice::File(p) => std::path::Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| p.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub text: String,
    pub font: FontChoice,
    pub size: f32,
    pub color: Color,
    /// Anchor point (top-left of the first line for Left, top-center for Center, ...).
    pub x: f32,
    pub y: f32,
    pub leading: f32,
    pub tracking: f32,
    pub align: TextAlign,
    pub faux_bold: bool,
    pub faux_italic: bool,
    pub underline: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            text: "Text".into(),
            font: FontChoice::Sans,
            size: 48.0,
            color: Color::BLACK,
            x: 20.0,
            y: 20.0,
            leading: 1.2,
            tracking: 0.0,
            align: TextAlign::Left,
            faux_bold: false,
            faux_italic: false,
            underline: false,
        }
    }
}

pub fn load_font(choice: &FontChoice) -> FontArc {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<FontChoice, FontArc>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(f) = cache.lock().unwrap().get(choice) {
        return f.clone();
    }
    let f = load_font_uncached(choice);
    cache.lock().unwrap().insert(choice.clone(), f.clone());
    f
}

fn load_font_uncached(choice: &FontChoice) -> FontArc {
    match choice {
        FontChoice::Sans => {
            FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("built-in font")
        }
        FontChoice::Mono => {
            FontArc::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("built-in font")
        }
        FontChoice::File(p) => std::fs::read(p)
            .ok()
            .and_then(|b| FontArc::try_from_vec(b).ok())
            .unwrap_or_else(|| load_font(&FontChoice::Sans)),
    }
}

/// System font files (Windows / macOS / Linux standard locations), for the font picker.
pub fn system_fonts() -> Vec<String> {
    let mut dirs: Vec<std::path::PathBuf> = vec![
        "C:\\Windows\\Fonts".into(),
        "/System/Library/Fonts".into(),
        "/Library/Fonts".into(),
        "/usr/share/fonts".into(),
        "/usr/local/share/fonts".into(),
    ];
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let h = std::path::PathBuf::from(home);
        dirs.push(h.join(".fonts"));
        dirs.push(h.join(".local/share/fonts"));
        dirs.push(h.join("Library/Fonts"));
        dirs.push(h.join("AppData\\Local\\Microsoft\\Windows\\Fonts"));
    }
    let mut out = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<String>, depth: u32) {
        if depth > 4 {
            return;
        }
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out, depth + 1);
                } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf") {
                        out.push(p.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    for d in dirs {
        walk(&d, &mut out, 0);
    }
    out.sort_by_key(|p| p.to_ascii_lowercase());
    out.dedup();
    out
}

/// Measure each line: (width, ascent, line height).
pub fn measure(style: &TextStyle) -> (Vec<f32>, f32, f32) {
    let font = load_font(&style.font);
    let sf = font.as_scaled(PxScale::from(style.size));
    let widths = style
        .text
        .lines()
        .map(|line| {
            let mut w = 0.0;
            let mut prev = None;
            for ch in line.chars() {
                let g = sf.glyph_id(ch);
                if let Some(p) = prev {
                    w += sf.kern(p, g);
                }
                w += sf.h_advance(g) + style.tracking;
                prev = Some(g);
            }
            w
        })
        .collect();
    (
        widths,
        sf.ascent(),
        (sf.ascent() - sf.descent() + sf.line_gap()) * style.leading,
    )
}

/// Bounding box of the rendered text (x, y, w, h) in document coordinates.
pub fn bounds(style: &TextStyle) -> (f32, f32, f32, f32) {
    let (widths, _, lh) = measure(style);
    let maxw = widths.iter().cloned().fold(0.0, f32::max);
    let n = widths.len().max(1) as f32;
    let x0 = match style.align {
        TextAlign::Left => style.x,
        TextAlign::Center => style.x - maxw / 2.0,
        TextAlign::Right => style.x - maxw,
    };
    (x0, style.y, maxw, lh * n)
}

/// Rasterize text into a transparent `w`×`h` image.
pub fn render(style: &TextStyle, w: u32, h: u32) -> Image {
    let mut img = Image::new(w, h);
    draw(&mut img, style);
    img
}

pub fn draw(img: &mut Image, style: &TextStyle) {
    let font = load_font(&style.font);
    let scale = PxScale::from(style.size);
    let sf = font.as_scaled(scale);
    let (widths, ascent, lh) = measure(style);
    let color = style.color.px();
    let (w, h) = (img.width as i32, img.height as i32);
    let skew = if style.faux_italic { 0.2 } else { 0.0 };
    let bold_offsets: &[f32] = if style.faux_bold {
        &[0.0, style.size * 0.03]
    } else {
        &[0.0]
    };
    // render coverage into a buffer so overlapping bold passes don't double-blend
    let mut cov = vec![0.0f32; (w * h) as usize];
    for (li, line) in style.text.lines().enumerate() {
        let lw = widths[li];
        let mut pen_x = match style.align {
            TextAlign::Left => style.x,
            TextAlign::Center => style.x - lw / 2.0,
            TextAlign::Right => style.x - lw,
        };
        let base_y = style.y + ascent + li as f32 * lh;
        let mut prev = None;
        let line_start = pen_x;
        for ch in line.chars() {
            let gid = sf.glyph_id(ch);
            if let Some(p) = prev {
                pen_x += sf.kern(p, gid);
            }
            for bo in bold_offsets {
                let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x + bo, base_y));
                if let Some(og) = font.outline_glyph(glyph) {
                    let bb = og.px_bounds();
                    og.draw(|gx, gy, c| {
                        let yy = bb.min.y as i32 + gy as i32;
                        let shear = (base_y - yy as f32) * skew;
                        let xx = (bb.min.x + gx as f32 + shear).round() as i32;
                        if xx >= 0 && yy >= 0 && xx < w && yy < h {
                            let i = (yy * w + xx) as usize;
                            cov[i] = cov[i].max(c);
                        }
                    });
                }
            }
            pen_x += sf.h_advance(gid) + style.tracking;
            prev = Some(gid);
        }
        if style.underline {
            let uy = (base_y + style.size * 0.12).round() as i32;
            let th = (style.size * 0.06).max(1.0) as i32;
            for yy in uy..uy + th {
                for xx in line_start.round() as i32..pen_x.round() as i32 {
                    if xx >= 0 && yy >= 0 && xx < w && yy < h {
                        cov[(yy * w + xx) as usize] = 1.0;
                    }
                }
            }
        }
    }
    let px = img.pixels_mut();
    for (i, c) in cov.iter().enumerate() {
        if *c > 0.0 {
            px[i] = composite_px(px[i], color, BlendMode::Normal, c.min(1.0), 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_pixels() {
        let s = TextStyle {
            text: "Kroma\nline two".into(),
            ..Default::default()
        };
        let img = render(&s, 400, 200);
        let ink = img.pixels().iter().filter(|p| p[3] > 0.5).count();
        assert!(ink > 300, "ink {ink}");
        let (_, _, w, h) = bounds(&s);
        assert!(w > 50.0 && h > 50.0);
    }
}
