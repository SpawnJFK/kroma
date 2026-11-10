//! Actions: serializable editing operations. The same `Op` list powers the Actions panel
//! (record/replay in the app), JSON recipes, and the `kroma` batch CLI.

use crate::adjust::{auto_color, auto_contrast, auto_tone, Adjustment};
use crate::buffer::Rect;
use crate::color::Color;
use crate::document::Document;
use crate::filter::{Filter, FilterCtx};
use crate::layer::{Layer, LayerKind};
use crate::text::TextStyle;
use crate::transform::{Anchor, Resample};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Adjust {
        adjustment: Adjustment,
    },
    Filter {
        filter: Filter,
    },
    AdjustmentLayer {
        adjustment: Adjustment,
    },
    AutoTone,
    AutoContrast,
    AutoColor,
    /// Resize: give width and/or height (aspect kept if one is missing) or a percent.
    Resize {
        width: Option<u32>,
        height: Option<u32>,
        percent: Option<f32>,
        #[serde(default)]
        method: Resample,
    },
    /// Shrink to fit inside a box (never enlarges).
    FitWithin {
        max_width: u32,
        max_height: u32,
    },
    CanvasSize {
        width: u32,
        height: u32,
        #[serde(default)]
        anchor: Anchor,
    },
    Crop {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    /// Center crop to an aspect ratio like 16:9 or 1:1.
    CropAspect {
        ratio_w: f32,
        ratio_h: f32,
    },
    Trim,
    RotateCw,
    RotateCcw,
    Rotate180,
    Rotate {
        degrees: f32,
    },
    FlipHorizontal,
    FlipVertical,
    Flatten,
    SelectAll,
    Deselect,
    InvertSelection,
    SelectColorRange {
        color: Color,
        fuzziness: f32,
    },
    SelectSubject,
    ContentAwareFill,
    FillSelection {
        color: Color,
    },
    NewLayer,
    Text {
        style: TextStyle,
    },
    /// Text watermark anchored to a corner with margin and opacity.
    Watermark {
        text: String,
        size: f32,
        color: Color,
        opacity: f32,
        #[serde(default)]
        anchor: Anchor,
        margin: f32,
    },
}

impl Op {
    pub fn label(&self) -> String {
        match self {
            Op::Adjust { adjustment } => adjustment.name().to_string(),
            Op::Filter { filter } => filter.name().to_string(),
            Op::AdjustmentLayer { adjustment } => format!("New {} Layer", adjustment.name()),
            Op::AutoTone => "Auto Tone".into(),
            Op::AutoContrast => "Auto Contrast".into(),
            Op::AutoColor => "Auto Color".into(),
            Op::Resize { .. } => "Image Size".into(),
            Op::FitWithin { .. } => "Fit Image".into(),
            Op::CanvasSize { .. } => "Canvas Size".into(),
            Op::Crop { .. } | Op::CropAspect { .. } => "Crop".into(),
            Op::Trim => "Trim".into(),
            Op::RotateCw => "Rotate 90° CW".into(),
            Op::RotateCcw => "Rotate 90° CCW".into(),
            Op::Rotate180 => "Rotate 180°".into(),
            Op::Rotate { .. } => "Rotate Canvas".into(),
            Op::FlipHorizontal => "Flip Canvas Horizontal".into(),
            Op::FlipVertical => "Flip Canvas Vertical".into(),
            Op::Flatten => "Flatten Image".into(),
            Op::SelectAll => "Select All".into(),
            Op::Deselect => "Deselect".into(),
            Op::InvertSelection => "Inverse".into(),
            Op::SelectColorRange { .. } => "Color Range".into(),
            Op::SelectSubject => "Select Subject".into(),
            Op::ContentAwareFill => "Content-Aware Fill".into(),
            Op::FillSelection { .. } => "Fill".into(),
            Op::NewLayer => "New Layer".into(),
            Op::Text { .. } => "Type Layer".into(),
            Op::Watermark { .. } => "Watermark".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub name: String,
    pub ops: Vec<Op>,
}

impl Action {
    pub fn load(path: &std::path::Path) -> Result<Action> {
        let s = std::fs::read_to_string(path)?;
        // accept either {"name":..,"ops":[..]} or a bare op list
        if let Ok(a) = serde_json::from_str::<Action>(&s) {
            return Ok(a);
        }
        let ops: Vec<Op> = serde_json::from_str(&s)?;
        Ok(Action {
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            ops,
        })
    }
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
    pub fn run(&self, doc: &mut Document, ctx: &FilterCtx) -> Result<()> {
        for op in &self.ops {
            apply(doc, op, ctx)?;
        }
        Ok(())
    }
    /// A few built-in actions (Photoshop ships "Default Actions"; these are practical ones).
    pub fn builtins() -> Vec<Action> {
        vec![
            Action {
                name: "Web Ready (2048px, auto tone, sharpen)".into(),
                ops: vec![
                    Op::FitWithin {
                        max_width: 2048,
                        max_height: 2048,
                    },
                    Op::AutoTone,
                    Op::Filter {
                        filter: Filter::UnsharpMask {
                            amount: 0.6,
                            radius: 0.8,
                            threshold: 0.0,
                        },
                    },
                ],
            },
            Action {
                name: "Instagram Square".into(),
                ops: vec![
                    Op::CropAspect {
                        ratio_w: 1.0,
                        ratio_h: 1.0,
                    },
                    Op::Resize {
                        width: Some(1080),
                        height: Some(1080),
                        percent: None,
                        method: Resample::Lanczos3,
                    },
                ],
            },
            Action {
                name: "Moody B&W".into(),
                ops: vec![
                    Op::Adjust {
                        adjustment: Adjustment::BlackWhite {
                            weights: [0.4, 0.6, 0.4, 0.6, 0.2, 0.8],
                            tint: None,
                        },
                    },
                    Op::Adjust {
                        adjustment: Adjustment::BrightnessContrast {
                            brightness: -0.1,
                            contrast: 0.35,
                            legacy: false,
                        },
                    },
                    Op::Filter {
                        filter: Filter::Vignette {
                            amount: 0.5,
                            midpoint: 0.5,
                        },
                    },
                ],
            },
            Action {
                name: "Vintage Film".into(),
                ops: vec![
                    Op::Adjust {
                        adjustment: Adjustment::Sepia { amount: 0.35 },
                    },
                    Op::Adjust {
                        adjustment: Adjustment::Exposure {
                            exposure: 0.1,
                            offset: 0.02,
                            gamma: 1.0,
                        },
                    },
                    Op::Filter {
                        filter: Filter::AddNoise {
                            amount: 0.04,
                            kind: crate::filter::NoiseKind::Gaussian,
                            monochrome: true,
                            seed: 3,
                        },
                    },
                    Op::Filter {
                        filter: Filter::Vignette {
                            amount: 0.35,
                            midpoint: 0.55,
                        },
                    },
                ],
            },
            Action {
                name: "Teal & Orange Grade".into(),
                ops: vec![
                    Op::Adjust {
                        adjustment: Adjustment::ColorBalance {
                            shadows: [-0.15, 0.0, 0.15],
                            midtones: [0.0, 0.0, 0.0],
                            highlights: [0.15, 0.03, -0.12],
                            preserve_luminosity: true,
                        },
                    },
                    Op::Adjust {
                        adjustment: Adjustment::Vibrance {
                            vibrance: 0.25,
                            saturation: 0.0,
                        },
                    },
                ],
            },
        ]
    }
}

/// Run one op against the document's active layer / whole image.
pub fn apply(doc: &mut Document, op: &Op, ctx: &FilterCtx) -> Result<()> {
    let sel = doc.selection.clone();
    let active_pixels = |doc: &mut Document| -> Result<()> {
        let l = doc
            .active_layer()
            .ok_or_else(|| anyhow!("no active layer"))?;
        if !matches!(l.kind, LayerKind::Raster) {
            return Err(anyhow!(
                "'{}' is not a pixel layer (rasterize it first)",
                l.name
            ));
        }
        Ok(())
    };
    match op {
        Op::Adjust { adjustment } => {
            active_pixels(doc)?;
            let l = doc.active_layer_mut().unwrap();
            adjustment.apply(l.image_mut(), sel.as_ref());
        }
        Op::Filter { filter } => {
            active_pixels(doc)?;
            let l = doc.active_layer_mut().unwrap();
            filter.apply(l.image_mut(), sel.as_ref(), ctx);
        }
        Op::AdjustmentLayer { adjustment } => {
            doc.add_adjustment_layer(adjustment.clone());
        }
        Op::AutoTone | Op::AutoContrast | Op::AutoColor => {
            active_pixels(doc)?;
            let l = doc.active_layer_mut().unwrap();
            let adj = match op {
                Op::AutoTone => auto_tone(l.image()),
                Op::AutoContrast => auto_contrast(l.image()),
                _ => auto_color(l.image()),
            };
            adj.apply(l.image_mut(), sel.as_ref());
        }
        Op::Resize {
            width,
            height,
            percent,
            method,
        } => {
            let (w, h) = (doc.width as f32, doc.height as f32);
            let (nw, nh) = match (width, height, percent) {
                (Some(nw), Some(nh), _) => (*nw as f32, *nh as f32),
                (Some(nw), None, _) => (*nw as f32, h * *nw as f32 / w),
                (None, Some(nh), _) => (w * *nh as f32 / h, *nh as f32),
                (None, None, Some(p)) => (w * p / 100.0, h * p / 100.0),
                _ => return Err(anyhow!("resize needs width, height or percent")),
            };
            doc.resize_image(
                (nw.round() as u32).max(1),
                (nh.round() as u32).max(1),
                *method,
            );
        }
        Op::FitWithin {
            max_width,
            max_height,
        } => {
            let s =
                (*max_width as f32 / doc.width as f32).min(*max_height as f32 / doc.height as f32);
            if s < 1.0 {
                let (nw, nh) = (
                    ((doc.width as f32 * s).round() as u32).max(1),
                    ((doc.height as f32 * s).round() as u32).max(1),
                );
                doc.resize_image(nw, nh, Resample::Lanczos3);
            }
        }
        Op::CanvasSize {
            width,
            height,
            anchor,
        } => doc.canvas_size(*width, *height, *anchor),
        Op::Crop {
            x,
            y,
            width,
            height,
        } => doc.crop(Rect::new(*x, *y, *width, *height)),
        Op::CropAspect { ratio_w, ratio_h } => {
            let target = ratio_w / ratio_h;
            let (w, h) = (doc.width as f32, doc.height as f32);
            let (cw, ch) = if w / h > target {
                (h * target, h)
            } else {
                (w, w / target)
            };
            doc.crop(Rect::new(
                ((w - cw) / 2.0) as i32,
                ((h - ch) / 2.0) as i32,
                cw.round() as i32,
                ch.round() as i32,
            ));
        }
        Op::Trim => doc.trim(),
        Op::RotateCw => doc.rotate_canvas_cw(),
        Op::RotateCcw => doc.rotate_canvas_ccw(),
        Op::Rotate180 => doc.rotate_canvas_180(),
        Op::Rotate { degrees } => doc.rotate_canvas(*degrees),
        Op::FlipHorizontal => doc.flip_canvas(true),
        Op::FlipVertical => doc.flip_canvas(false),
        Op::Flatten => doc.flatten(),
        Op::SelectAll => doc.selection = Some(crate::buffer::Mask::new(doc.width, doc.height, 1.0)),
        Op::Deselect => doc.selection = None,
        Op::InvertSelection => {
            doc.selection = Some(match &doc.selection {
                Some(s) => crate::selection::invert(s),
                None => crate::buffer::Mask::new(doc.width, doc.height, 1.0),
            })
        }
        Op::SelectColorRange { color, fuzziness } => {
            let img = doc.composite();
            doc.selection = Some(crate::selection::color_range(&img, *color, *fuzziness));
        }
        Op::SelectSubject => {
            let img = doc.composite();
            doc.selection = Some(crate::selection::select_subject_heuristic(&img));
        }
        Op::ContentAwareFill => {
            active_pixels(doc)?;
            let s = sel.ok_or_else(|| anyhow!("Content-Aware Fill needs a selection"))?;
            let l = doc.active_layer_mut().unwrap();
            let out = crate::inpaint::content_aware_fill(l.image(), &s);
            l.set_image(out);
        }
        Op::FillSelection { color } => {
            active_pixels(doc)?;
            let l = doc.active_layer_mut().unwrap();
            crate::brush::fill_selection(
                l.image_mut(),
                sel.as_ref(),
                *color,
                1.0,
                crate::blend::BlendMode::Normal,
                false,
            );
        }
        Op::NewLayer => {
            doc.new_raster_layer();
        }
        Op::Text { style } => {
            let l = Layer::text(style.clone(), doc.width, doc.height);
            doc.add_layer(l);
        }
        Op::Watermark {
            text,
            size,
            color,
            opacity,
            anchor,
            margin,
        } => {
            let mut style = TextStyle {
                text: text.clone(),
                size: *size,
                color: *color,
                ..Default::default()
            };
            let (_, _, tw, th) = crate::text::bounds(&style);
            let i = Anchor::GRID.iter().position(|a| a == anchor).unwrap_or(8) as f32;
            let (ax, ay) = (i % 3.0, (i / 3.0).floor());
            let x = *margin + (doc.width as f32 - tw - 2.0 * margin) * ax / 2.0;
            let y = *margin + (doc.height as f32 - th - 2.0 * margin) * ay / 2.0;
            style.x = x;
            style.y = y;
            let mut l = Layer::text(style, doc.width, doc.height);
            l.opacity = *opacity;
            l.name = "Watermark".into();
            doc.add_layer(l);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recipe_json_roundtrip_and_run() {
        let json = r#"[
            {"op":"fit_within","max_width":50,"max_height":50},
            {"op":"auto_tone"},
            {"op":"adjust","adjustment":{"HueSaturation":{"hue":30,"saturation":0.2,"lightness":0,"colorize":false}}},
            {"op":"filter","filter":{"GaussianBlur":{"radius":1.5}}},
            {"op":"crop_aspect","ratio_w":1,"ratio_h":1},
            {"op":"watermark","text":"(c) Kroma","size":8,"color":{"r":1,"g":1,"b":1,"a":1},"opacity":0.6,"anchor":"BottomRight","margin":2}
        ]"#;
        let ops: Vec<Op> = serde_json::from_str(json).unwrap();
        let action = Action {
            name: "t".into(),
            ops,
        };
        let mut d = Document::new(120, 80, Some(Color::rgb(0.3, 0.5, 0.7)));
        action.run(&mut d, &FilterCtx::default()).unwrap();
        assert_eq!((d.width, d.height), (33, 33));
        let s = serde_json::to_string(&action).unwrap();
        let back: Action = serde_json::from_str(&s).unwrap();
        assert_eq!(back, action);
        for a in Action::builtins() {
            let mut d = Document::new(64, 48, Some(Color::rgb(0.6, 0.4, 0.3)));
            a.run(&mut d, &FilterCtx::default()).unwrap();
        }
    }
}
