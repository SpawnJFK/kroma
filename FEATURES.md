# Kroma vs Photoshop: parity matrix

Status: **Done** works end to end in the app and engine. **Partial** works with stated limits. **Planned** is in ROADMAP.md with a phase number.

## Tools

| Photoshop tool | Kroma | Notes |
|---|---|---|
| Move, auto-select, align | Done | Moves selected pixels only when a selection exists; arrow-key nudge; align to canvas or selection |
| Artboard | Planned (P4) | |
| Rectangular / Elliptical Marquee | Done | Add/Subtract/Intersect via Shift/Alt, feather, anti-alias |
| Single row/column marquee | Planned (P2) | |
| Lasso / Polygonal Lasso | Done | |
| Magnetic Lasso | Planned (P2) | |
| Object Selection, Quick Selection | Planned (P2, P6) | Graph-cut quick selection, ML object selection |
| Magic Wand | Done | Tolerance, contiguous, sample all layers |
| Crop | Done | Aspect presets, can extend canvas |
| Perspective Crop | Planned (P7) | |
| Eyedropper | Done | Layer or merged sample, Alt = background |
| Color Sampler, Ruler, Note, Count, Slice, Frame | Planned (P7) | |
| Spot Healing Brush / Remove | Done | Content-aware (PatchMatch) on release |
| Healing Brush | Done | Texture from source, tone from destination |
| Patch, Content-Aware Move, Red Eye | Planned (P7) | |
| Brush, Pencil | Done | Opacity vs flow, hardness, spacing, angle, roundness, scatter, jitter, pressure, 27 modes |
| Mixer Brush, Color Replacement | Planned (P7) | |
| Clone Stamp | Done | Aligned or not, sample layer or all |
| Pattern Stamp, History Brush, Art History | Planned (P7) | |
| Eraser | Done | Also on masks |
| Background Eraser, Magic Eraser | Partial | Magic Wand + Delete covers Magic Eraser |
| Gradient | Done | Linear, Radial, Angle, Reflected, Diamond, stops editor, presets, dither |
| Paint Bucket | Done | |
| Blur, Sharpen, Smudge | Done | |
| Dodge, Burn, Sponge | Done | Shadows / Midtones / Highlights |
| Pen tools, Path Selection | Planned (P3) | Needs vector paths |
| Type | Done | System fonts, size, tracking, leading, alignment, faux bold/italic, underline, multi-line, editable |
| Vertical Type, Type on Path | Planned (P3) | |
| Shapes | Partial | Rect, rounded, ellipse, polygon, star, line/arrow drawn as pixels on a new layer. Vector shape layers in P3 |
| Hand, Zoom | Done | Space pan, wheel zoom at cursor, navigator |
| Rotate View | Planned (P7) | |
| Liquify | Done | Forward Warp, Pucker, Bloat, Twirl, Reconstruct |

## Layers

| Feature | Kroma |
|---|---|
| Pixel layers, duplicate, delete, rename, reorder, visibility | Done |
| Groups (pass-through and isolated), nesting | Done |
| Layer masks: reveal/hide all, from selection, paint, invert, disable, apply | Done |
| Clipping masks | Done |
| Blend modes (all 27) | Done |
| Opacity and Fill opacity | Done |
| Adjustment layers (19 types, masked, live Properties panel) | Done |
| Fill layers: solid, gradient | Done. Pattern fill: Planned (P4) |
| Layer styles: Drop Shadow, Inner Shadow, Outer/Inner Glow, Bevel & Emboss, Satin, Color/Gradient Overlay, Stroke | Done. Pattern Overlay: Planned (P4) |
| Lock transparency | Done. Lock pixels/position/all: Partial (stored, not enforced) |
| Merge Down, Merge Visible, Flatten, Rasterize | Done |
| Smart Objects, Smart Filters | Planned (P4) |
| Vector masks | Planned (P3) |
| Layer comps, linking, distribute | Planned (P4) |

## Image > Adjustments

Brightness/Contrast (modern + legacy), Levels (per channel), Curves (per channel, editor with histogram and presets), Exposure, Vibrance, Hue/Saturation (+ Colorize), Color Balance, Black & White (+ tint), Photo Filter, Channel Mixer (+ monochrome), Temperature & Tint (the 2026 Color & Vibrance controls), Selective Color (relative/absolute, 9 ranges), Gradient Map, Invert, Desaturate, Posterize, Threshold, Solarize, Sepia, Shadows/Highlights, Equalize, Auto Tone, Auto Contrast, Auto Color: **Done**.

Color Lookup (3D LUT), HDR Toning, Match Color, Replace Color: **Planned (P5)**.

## Filters (37, all with live preview, selection-aware, recordable)

- Blur: Gaussian, Box, Motion, Radial (zoom/spin), Surface, Tilt-Shift
- Sharpen: Unsharp Mask, Sharpen, High Pass, Clarity
- Noise: Add Noise, Median, Despeckle, Reduce Noise
- Stylize: Find Edges, Emboss, Oil Paint, Glowing Edges
- Distort: Twirl, Pinch, Spherize, Wave, Ripple, Polar Coordinates
- Pixelate: Mosaic, Crystallize, Color Halftone
- Render: Clouds, Vignette, Lens Flare
- Other: Offset, Maximum, Minimum, Chromatic Aberration, Equalize, Shadows/Highlights, Dehaze

Filter Gallery artistic set, Lens Blur with depth map, Camera Raw Filter, Neural Filters: **Planned (P5, P6)**.

## Selections

All, Deselect, Reselect, Inverse, Color Range (sampled + highlights/midtones/shadows), Select Subject (heuristic), Grow, Similar, Border, Smooth, Expand, Contract, Feather, load from layer transparency (Ctrl+click thumbnail): **Done**.
Select and Mask workspace, Quick Mask, saved alpha channels, Transform Selection, Focus Area, Sky: **Planned (P2, P6)**.

## Transform

Free Transform (move, scale with handles, proportional by default, rotate with 15° snap), Distort/Perspective (4 corners), Flip/Rotate layer, Image Size (Nearest, Bilinear, Bicubic, Lanczos), Canvas Size (9 anchors), Rotate Canvas 90/180/arbitrary, Flip Canvas, Crop to selection, Trim: **Done**.
Warp, Puppet Warp, Content-Aware Scale, Skew handles: **Planned (P7)**.

## Edit

Undo/Redo, History panel with jump, snapshots, Cut/Copy/Copy Merged/Paste (system clipboard), Clear, Fill (colors, 50% gray, content-aware), Stroke (inside/center/outside), Content-Aware Fill, Free Transform: **Done**.
Define Pattern/Brush, Puppet Warp, Preset Manager: **Planned (P7)**.

## Files

| Format | Read | Write |
|---|---|---|
| .kroma (native, layered, 16-bit, keeps everything) | Done | Done |
| PSD | Done (layers, groups, opacity, visibility, blend modes, clipping) | Planned (P7) |
| OpenRaster .ora (GIMP, Krita, MyPaint) | Done | Done |
| PNG, JPEG, WebP, TIFF, BMP, GIF, TGA, ICO, QOI, PNM, HDR, EXR | Done | Done (16-bit PNG/TIFF, float EXR) |
| Camera RAW | Planned (P5) | |
| PDF, SVG | Planned (P3, P7) | |

## Color

Internal 32-bit float pipeline: **Done**. ICC color management, CMYK and Lab modes, soft proofing: **Planned (P5)**.

## Automation

Actions: record, play, save/load JSON, built-in actions, batch processing in the app (background thread with progress) and `kroma` CLI (parallel, per-file progress): **Done**. Scripting language and plugin API: **Planned (P7)**.

## AI features

Content-Aware Fill / Spot Healing / Remove via PatchMatch: **Done (no AI needed, runs offline)**.
Generative Fill / Expand, ML Select Subject, Remove Background, Super Resolution, Neural Filters, Harmonize: **Planned (P6)** through pluggable providers (local ONNX models or a local ComfyUI, or cloud APIs).

## Shipping

GitHub Actions release zips (Windows, macOS, Linux) on `v*` tags: **Done**. Windows GUI binary (no console), app icon: **Done**. MSI installer (Windows): **Done**. Autosave and crash recovery: **Done**. Settings persistence (tools, colors, swatches, custom actions, recent files): **Done**.

## Beyond Photoshop (already in)

Command palette with every command, tool, adjustment and filter (Ctrl+K). Headless engine and CLI with JSON recipes and parallel batch. OpenRaster interop. Tilt-Shift, Clarity, Dehaze and Chromatic Aberration filters. Export size estimate. Atomic saves that never corrupt the original. Headless UI test suite that drives real frames.
