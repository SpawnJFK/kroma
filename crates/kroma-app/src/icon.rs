//! App icon (256px, accent blue on dark panel).

use egui::IconData;

const SIZE: usize = 256;

/// RGBA pixels for a 256x256 Kroma mark.
pub fn rgba_256() -> Vec<u8> {
    let mut px = vec![0u8; SIZE * SIZE * 4];
    let bg = [50, 50, 52, 255];
    let accent = [56, 132, 255, 255];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = (y * SIZE + x) * 4;
            px[i..i + 4].copy_from_slice(&bg);
            let fx = x as f32 / SIZE as f32;
            let fy = y as f32 / SIZE as f32;
            // Rounded square frame
            let cx = 0.5f32;
            let cy = 0.5f32;
            let dx = (fx - cx).abs();
            let dy = (fy - cy).abs();
            let corner = 0.38f32;
            let in_frame = dx < 0.42
                && dy < 0.42
                && (dx < corner
                    || dy < corner
                    || (dx - 0.42).max(0.0) + (dy - 0.42).max(0.0) < 0.08);
            // Stylized "K" strokes
            let k_stem = fx > 0.28 && fx < 0.36 && fy > 0.22 && fy < 0.78;
            let k_diag1 = {
                let t = (fy - 0.22) / 0.28;
                (fx - (0.36 + t * 0.18)).abs() < 0.035 && (0.22..=0.50).contains(&fy)
            };
            let k_diag2 = {
                let t = (fy - 0.50) / 0.28;
                (fx - (0.54 - t * 0.18)).abs() < 0.035 && (0.50..=0.78).contains(&fy)
            };
            if in_frame || k_stem || k_diag1 || k_diag2 {
                px[i..i + 4].copy_from_slice(&accent);
            }
        }
    }
    px
}

pub fn icon_data() -> IconData {
    IconData {
        rgba: rgba_256(),
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
