//! Embed Windows .ico from the same pixels as the viewport icon.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ico_path = std::path::Path::new(&out_dir).join("kroma.ico");
    write_ico(&ico_path);
    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().unwrap());
    if let Err(e) = res.compile() {
        panic!("winresource: {e}");
    }
}

fn write_ico(path: &std::path::Path) {
    let rgba = icon_rgba_256();
    let img = image::RgbaImage::from_raw(256, 256, rgba).expect("icon pixels");
    img.save(path).expect("write ico");
}

fn icon_rgba_256() -> Vec<u8> {
    const SIZE: usize = 256;
    let mut px = vec![0u8; SIZE * SIZE * 4];
    let bg = [50, 50, 52, 255];
    let accent = [56, 132, 255, 255];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let i = (y * SIZE + x) * 4;
            px[i..i + 4].copy_from_slice(&bg);
            let fx = x as f32 / SIZE as f32;
            let fy = y as f32 / SIZE as f32;
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
