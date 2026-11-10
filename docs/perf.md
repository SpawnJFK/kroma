# Kroma performance notes

Recorded on Windows 10, release build, Rust stable (2026-10-07).

## Brush stroke partial composite (6000 x 4000, 20 layers)

Scenario: active pixel layer, dirty rect 400 x 400 while painting.

| Path | Avg frame time |
|------|----------------|
| Full `composite_rect` on dirty region | 18.3 ms |
| Cached underlay + active layer only | 1.7 ms |

Target for interactive painting is under 16 ms per frame. The paint-under cache keeps typical brush frames well under that budget on the reference image.

Probe: `cargo test -p kroma-core perf_brush_dirty_rect_6000x4000 --release -- --ignored --nocapture`

## UI throttles

- Layer panel thumbnails: at most 4 texture regens per frame per document tab.
- Histogram panel: computed on a 512 px proxy of the merged composite.

## Canvas

- GPU tiles refresh on dirty rects only (1024 px tiles).
- Full composite runs on full redraw (undo, resize, etc.).
