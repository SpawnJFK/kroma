//! File IO: native .kroma (layered, lossless 16-bit, keeps text/adjustments/styles),
//! OpenRaster .ora (GIMP/Krita interchange), PSD import, and flat import/export of
//! PNG, JPEG, WebP, TIFF, BMP, GIF, TGA, ICO, PNM, QOI, HDR, EXR.

use crate::buffer::{Image, Mask};
use crate::color::Color;
use crate::document::{Document, Guide};
use crate::layer::{Layer, LayerId, LayerKind};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;

pub const OPEN_EXTENSIONS: &[&str] = &[
    "kroma", "ora", "psd", "png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif", "tga", "ico",
    "pnm", "ppm", "pgm", "qoi", "hdr", "exr",
];
pub const EXPORT_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "tif", "tiff", "bmp", "gif", "tga", "ico", "qoi", "exr",
];

fn ext_of(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn open(path: &Path) -> Result<Document> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".into());
    let mut doc = match ext_of(path).as_str() {
        "kroma" => load_kroma(path)?,
        "ora" => load_ora(path)?,
        "psd" => load_psd(&std::fs::read(path)?)?,
        _ => Document::from_image(&name, load_image(path)?),
    };
    doc.name = name;
    Ok(doc)
}

pub fn load_image(path: &Path) -> Result<Image> {
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .with_context(|| format!("decoding {}", path.display()))?;
    Ok(Image::from_dynamic(&img))
}

pub fn load_image_bytes(bytes: &[u8]) -> Result<Image> {
    let img = image::load_from_memory(bytes)?;
    Ok(Image::from_dynamic(&img))
}

#[derive(Clone, Debug)]
pub struct ExportOptions {
    /// JPEG / WebP quality 1..100.
    pub quality: u8,
    /// Write 16-bit PNG/TIFF.
    pub sixteen_bit: bool,
    /// Background for formats without alpha (JPEG).
    pub matte: Color,
    /// Optional resize on export (Export As / Save for Web).
    pub scale: f32,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            quality: 90,
            sixteen_bit: false,
            matte: Color::WHITE,
            scale: 1.0,
        }
    }
}

/// Encode a flat image to bytes in the format implied by `ext`.
pub fn encode_image(img: &Image, ext: &str, opts: &ExportOptions) -> Result<Vec<u8>> {
    let img = if (opts.scale - 1.0).abs() > 1e-3 {
        crate::transform::resize(
            img,
            ((img.width as f32 * opts.scale).round() as u32).max(1),
            ((img.height as f32 * opts.scale).round() as u32).max(1),
            crate::transform::Resample::Lanczos3,
        )
    } else {
        img.clone()
    };
    let mut out = Cursor::new(Vec::new());
    match ext {
        "jpg" | "jpeg" => {
            let mut flat = Image::filled(img.width, img.height, opts.matte.px());
            flat.blit(&Image::new(0, 0), 0, 0);
            let fp = img.pixels().to_vec();
            for (d, s) in flat.pixels_mut().iter_mut().zip(fp) {
                *d = crate::blend::composite_px(*d, s, crate::blend::BlendMode::Normal, 1.0, 0, 0);
            }
            let rgb = flat.to_dynamic8().to_rgb8();
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(
                &mut out,
                opts.quality.clamp(1, 100),
            );
            rgb.write_with_encoder(enc)?;
        }
        "png" if opts.sixteen_bit => img
            .to_dynamic16()
            .write_to(&mut out, image::ImageFormat::Png)?,
        "tif" | "tiff" if opts.sixteen_bit => img
            .to_dynamic16()
            .write_to(&mut out, image::ImageFormat::Tiff)?,
        "exr" => image::DynamicImage::ImageRgba32F(
            image::Rgba32FImage::from_raw(
                img.width,
                img.height,
                img.pixels().iter().flat_map(|p| *p).collect(),
            )
            .unwrap(),
        )
        .write_to(&mut out, image::ImageFormat::OpenExr)?,
        "ico" => {
            let small = if img.width > 256 || img.height > 256 {
                let s = 256.0 / img.width.max(img.height) as f32;
                crate::transform::resize(
                    &img,
                    ((img.width as f32 * s) as u32).max(1),
                    ((img.height as f32 * s) as u32).max(1),
                    crate::transform::Resample::Lanczos3,
                )
            } else {
                img.clone()
            };
            small
                .to_dynamic8()
                .write_to(&mut out, image::ImageFormat::Ico)?
        }
        "bmp" | "gif" | "tga" | "qoi" | "webp" | "png" | "tif" | "tiff" => {
            let fmt = image::ImageFormat::from_extension(ext)
                .ok_or_else(|| anyhow!("unknown format {ext}"))?;
            img.to_dynamic8().write_to(&mut out, fmt)?
        }
        other => bail!("cannot export .{other}"),
    }
    Ok(out.into_inner())
}

/// File > Export As: flatten and write.
pub fn export(doc: &Document, path: &Path, opts: &ExportOptions) -> Result<()> {
    let bytes = encode_image(&doc.composite(), &ext_of(path), opts)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Save in the format implied by the extension (layered for .kroma / .ora, flat otherwise).
pub fn save(doc: &Document, path: &Path) -> Result<()> {
    match ext_of(path).as_str() {
        "kroma" => save_kroma(doc, path),
        "ora" => save_ora(doc, path),
        _ => export(doc, path, &ExportOptions::default()),
    }
}

// ---------------------------------------------------------------- .kroma native

#[derive(Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    width: u32,
    height: u32,
    dpi: f32,
    active: LayerId,
    guides: Vec<Guide>,
    layers: Vec<Layer>,
}

fn png16(img: &Image) -> Result<Vec<u8>> {
    let mut c = Cursor::new(Vec::new());
    img.to_dynamic16()
        .write_to(&mut c, image::ImageFormat::Png)?;
    Ok(c.into_inner())
}

fn png8(img: &Image) -> Result<Vec<u8>> {
    let mut c = Cursor::new(Vec::new());
    img.to_dynamic8()
        .write_to(&mut c, image::ImageFormat::Png)?;
    Ok(c.into_inner())
}

fn mask_png(m: &Mask) -> Result<Vec<u8>> {
    let data: Vec<u16> = m
        .data()
        .iter()
        .map(|v| (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
        .collect();
    let img = image::ImageBuffer::<image::Luma<u16>, _>::from_raw(m.width, m.height, data).unwrap();
    let mut c = Cursor::new(Vec::new());
    image::DynamicImage::ImageLuma16(img).write_to(&mut c, image::ImageFormat::Png)?;
    Ok(c.into_inner())
}

fn mask_from_png(bytes: &[u8]) -> Result<Mask> {
    let img = image::load_from_memory(bytes)?.to_luma16();
    let (w, h) = img.dimensions();
    Ok(Mask::from_vec(
        w,
        h,
        img.into_raw()
            .into_iter()
            .map(|v| v as f32 / 65535.0)
            .collect(),
    ))
}

fn visit_layers<'a>(list: &'a [Layer], f: &mut dyn FnMut(&'a Layer)) {
    for l in list {
        f(l);
        if let Some(ch) = l.children() {
            visit_layers(ch, f);
        }
    }
}

fn visit_layers_mut(list: &mut [Layer], f: &mut dyn FnMut(&mut Layer)) {
    for l in list.iter_mut() {
        f(l);
        if let LayerKind::Group { children, .. } = &mut l.kind {
            visit_layers_mut(children, f);
        }
    }
}

pub fn save_kroma_bytes(doc: &Document) -> Result<Vec<u8>> {
    let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let manifest = Manifest {
        format: "kroma".into(),
        version: 1,
        width: doc.width,
        height: doc.height,
        dpi: doc.dpi,
        active: doc.active,
        guides: doc.guides.clone(),
        layers: doc.layers.clone(),
    };
    zw.start_file("manifest.json", SimpleFileOptions::default())?;
    zw.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    let mut err = None;
    visit_layers(&doc.layers, &mut |l| {
        let r: Result<()> = (|| {
            if matches!(l.kind, LayerKind::Raster) && !l.image().is_empty() {
                zw.start_file(format!("layers/{}.png", l.id), stored)?;
                zw.write_all(&png16(l.image())?)?;
            }
            if let Some(m) = l.mask() {
                zw.start_file(format!("masks/{}.png", l.id), stored)?;
                zw.write_all(&mask_png(m)?)?;
            }
            Ok(())
        })();
        if let Err(e) = r {
            err = Some(e);
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    if let Some(sel) = &doc.selection {
        zw.start_file("selection.png", stored)?;
        zw.write_all(&mask_png(sel)?)?;
    }
    let comp = doc.composite();
    zw.start_file("preview.png", stored)?;
    zw.write_all(&png8(&comp)?)?;
    Ok(zw.finish()?.into_inner())
}

pub fn save_kroma(doc: &Document, path: &Path) -> Result<()> {
    let bytes = save_kroma_bytes(doc)?;
    // atomic-ish save: write temp then rename, so a crash never corrupts the original
    let tmp = path.with_extension("kroma.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load_kroma(path: &Path) -> Result<Document> {
    load_kroma_bytes(&std::fs::read(path)?)
}

pub fn load_kroma_bytes(bytes: &[u8]) -> Result<Document> {
    let mut za = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut s = String::new();
    za.by_name("manifest.json")?.read_to_string(&mut s)?;
    let m: Manifest = serde_json::from_str(&s)?;
    if m.format != "kroma" {
        bail!("not a Kroma document");
    }
    let mut layers = m.layers;
    let (w, h) = (m.width, m.height);
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut f = za.by_name(name).ok()?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).ok()?;
        Some(b)
    };
    let mut errors = Vec::new();
    visit_layers_mut(&mut layers, &mut |l| {
        l.reset_runtime();
        if matches!(l.kind, LayerKind::Raster) {
            match read(&format!("layers/{}.png", l.id)).map(|b| load_image_bytes(&b)) {
                Some(Ok(img)) => l.set_image_raw(img),
                Some(Err(e)) => errors.push(e),
                None => l.set_image_raw(Image::new(w, h)),
            }
        }
        if let Some(b) = read(&format!("masks/{}.png", l.id)) {
            match mask_from_png(&b) {
                Ok(mk) => l.set_mask_raw(Some(mk)),
                Err(e) => errors.push(e),
            }
        }
        l.rerender(w, h);
    });
    if let Some(e) = errors.pop() {
        return Err(e);
    }
    let selection = read("selection.png").and_then(|b| mask_from_png(&b).ok());
    let active = m.active;
    let mut doc = Document {
        width: w,
        height: h,
        dpi: m.dpi,
        layers,
        active,
        selection,
        guides: m.guides,
        name: "Untitled".into(),
    };
    if doc.layer(active).is_none() {
        doc.active = doc.layers.last().map(|l| l.id).unwrap_or(0);
    }
    Ok(doc)
}

// ---------------------------------------------------------------- OpenRaster

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn save_ora(doc: &Document, path: &Path) -> Result<()> {
    let mut zw = zip::ZipWriter::new(std::fs::File::create(path)?);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zw.start_file("mimetype", stored)?;
    zw.write_all(b"image/openraster")?;
    let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<image version=\"0.0.5\" w=\"{}\" h=\"{}\" xres=\"{}\" yres=\"{}\">\n", doc.width, doc.height, doc.dpi, doc.dpi);
    fn write_stack(
        list: &[Layer],
        zw: &mut zip::ZipWriter<std::fs::File>,
        xml: &mut String,
        depth: usize,
        stored: SimpleFileOptions,
    ) -> Result<()> {
        let pad = "  ".repeat(depth);
        for l in list.iter().rev() {
            let vis = if l.visible { "visible" } else { "hidden" };
            match &l.kind {
                LayerKind::Group {
                    children,
                    pass_through,
                    ..
                } => {
                    xml.push_str(&format!(
                        "{pad}<stack name=\"{}\" opacity=\"{:.3}\" visibility=\"{vis}\" isolation=\"{}\" composite-op=\"{}\">\n",
                        xml_escape(&l.name),
                        l.opacity,
                        if *pass_through { "auto" } else { "isolate" },
                        l.blend.ora_name()
                    ));
                    write_stack(children, zw, xml, depth + 1, stored)?;
                    xml.push_str(&format!("{pad}</stack>\n"));
                }
                LayerKind::Adjustment(_) => {
                    // OpenRaster has no adjustment layers; bake the effect into a pixel layer is
                    // not possible in isolation, so it is skipped (use .kroma to keep it).
                }
                _ => {
                    let src = format!("data/{}.png", l.id);
                    zw.start_file(&src, stored)?;
                    zw.write_all(&png8(&l.styled_content())?)?;
                    xml.push_str(&format!(
                        "{pad}<layer name=\"{}\" src=\"{src}\" x=\"0\" y=\"0\" opacity=\"{:.3}\" visibility=\"{vis}\" composite-op=\"{}\"/>\n",
                        xml_escape(&l.name),
                        l.opacity,
                        l.blend.ora_name()
                    ));
                }
            }
        }
        Ok(())
    }
    xml.push_str("<stack>\n");
    write_stack(&doc.layers, &mut zw, &mut xml, 1, stored)?;
    xml.push_str("</stack>\n</image>\n");
    zw.start_file("stack.xml", SimpleFileOptions::default())?;
    zw.write_all(xml.as_bytes())?;
    let comp = doc.composite();
    zw.start_file("mergedimage.png", stored)?;
    zw.write_all(&png8(&comp)?)?;
    let s = 256.0 / comp.width.max(comp.height) as f32;
    let thumb = crate::transform::resize(
        &comp,
        ((comp.width as f32 * s.min(1.0)) as u32).max(1),
        ((comp.height as f32 * s.min(1.0)) as u32).max(1),
        crate::transform::Resample::Bilinear,
    );
    zw.start_file("Thumbnails/thumbnail.png", stored)?;
    zw.write_all(&png8(&thumb)?)?;
    zw.finish()?;
    Ok(())
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(
        tag[start..end]
            .replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

pub fn load_ora(path: &Path) -> Result<Document> {
    let mut za = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    let mut xml = String::new();
    za.by_name("stack.xml")?.read_to_string(&mut xml)?;
    let img_tag_start = xml
        .find("<image")
        .ok_or_else(|| anyhow!("stack.xml: no <image>"))?;
    let img_tag = &xml[img_tag_start..img_tag_start + xml[img_tag_start..].find('>').unwrap_or(0)];
    let w: u32 = attr(img_tag, "w")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| anyhow!("no width"))?;
    let h: u32 = attr(img_tag, "h")
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| anyhow!("no height"))?;
    // tokenize tags
    let mut tags = Vec::new();
    let mut rest = &xml[img_tag_start + img_tag.len()..];
    while let Some(s) = rest.find('<') {
        let e = rest[s..].find('>').map(|e| s + e + 1).unwrap_or(rest.len());
        tags.push(rest[s..e].to_string());
        rest = &rest[e..];
    }
    // build tree: stack of child lists (top-to-bottom order as in file)
    let mut stack: Vec<(Option<Layer>, Vec<Layer>)> = vec![];
    let mut root: Vec<Layer> = vec![];
    let mut first_stack = true;
    for t in tags {
        if t.starts_with("<stack") {
            if first_stack {
                first_stack = false;
                stack.push((None, vec![]));
                continue;
            }
            let mut g = Layer::group(&attr(&t, "name").unwrap_or_else(|| "Group".into()), vec![]);
            g.opacity = attr(&t, "opacity")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            g.visible = attr(&t, "visibility")
                .map(|v| v != "hidden")
                .unwrap_or(true);
            g.blend = crate::blend::BlendMode::from_ora_name(
                &attr(&t, "composite-op").unwrap_or_default(),
            );
            if let LayerKind::Group { pass_through, .. } = &mut g.kind {
                *pass_through = attr(&t, "isolation")
                    .map(|v| v != "isolate")
                    .unwrap_or(true);
            }
            stack.push((Some(g), vec![]));
        } else if t.starts_with("</stack") {
            if let Some((g, mut children)) = stack.pop() {
                children.reverse(); // file is top-to-bottom, Kroma stores bottom-to-top
                match g {
                    Some(mut g) => {
                        if let LayerKind::Group { children: c, .. } = &mut g.kind {
                            *c = children;
                        }
                        if let Some(top) = stack.last_mut() {
                            top.1.push(g);
                        }
                    }
                    None => root = children,
                }
            }
        } else if t.starts_with("<layer") {
            let src = attr(&t, "src").unwrap_or_default();
            let mut bytes = Vec::new();
            if let Ok(mut f) = za.by_name(&src) {
                f.read_to_end(&mut bytes)?;
            }
            let x: i32 = attr(&t, "x")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0) as i32;
            let y: i32 = attr(&t, "y")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(0.0) as i32;
            let mut canvas = Image::new(w, h);
            if let Ok(img) = load_image_bytes(&bytes) {
                canvas.blit(&img, x, y);
            }
            let mut l =
                Layer::from_image(&attr(&t, "name").unwrap_or_else(|| "Layer".into()), canvas);
            l.opacity = attr(&t, "opacity")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            l.visible = attr(&t, "visibility")
                .map(|v| v != "hidden")
                .unwrap_or(true);
            l.blend = crate::blend::BlendMode::from_ora_name(
                &attr(&t, "composite-op").unwrap_or_default(),
            );
            if let Some(top) = stack.last_mut() {
                top.1.push(l);
            }
        }
    }
    if root.is_empty() {
        bail!("OpenRaster file has no layers");
    }
    let active = root.last().unwrap().id;
    Ok(Document {
        width: w,
        height: h,
        dpi: 72.0,
        layers: root,
        active,
        selection: None,
        guides: vec![],
        name: "Untitled".into(),
    })
}

// ---------------------------------------------------------------- PSD import

pub fn load_psd(bytes: &[u8]) -> Result<Document> {
    let psd = psd::Psd::from_bytes(bytes).map_err(|e| anyhow!("PSD: {e}"))?;
    let (w, h) = (psd.width(), psd.height());
    if psd.layers().is_empty() {
        let img = Image::from_rgba8(w, h, &psd.rgba());
        return Ok(Document::from_image("PSD", img));
    }
    // layers: psd index 0 is the bottom layer
    #[derive(Clone)]
    enum Item {
        L(usize),
        G(u32),
    }
    let groups = psd.groups();
    let mut children: std::collections::HashMap<Option<u32>, Vec<Item>> = Default::default();
    for (i, l) in psd.layers().iter().enumerate() {
        children.entry(l.parent_id()).or_default().push(Item::L(i));
    }
    for (gid, g) in groups.iter() {
        children
            .entry(g.parent_id())
            .or_default()
            .push(Item::G(*gid));
    }
    // order key: the highest layer index inside an item
    fn max_idx(item: &Item, ch: &std::collections::HashMap<Option<u32>, Vec<Item>>) -> i64 {
        match item {
            Item::L(i) => *i as i64,
            Item::G(g) => ch
                .get(&Some(*g))
                .map(|v| v.iter().map(|it| max_idx(it, ch)).max().unwrap_or(-1))
                .unwrap_or(-1),
        }
    }
    fn build(
        parent: Option<u32>,
        psd: &psd::Psd,
        ch: &std::collections::HashMap<Option<u32>, Vec<Item>>,
        w: u32,
        h: u32,
    ) -> Vec<Layer> {
        let mut items = ch.get(&parent).cloned().unwrap_or_default();
        items.sort_by_key(|it| max_idx(it, ch));
        items
            .into_iter()
            .filter_map(|it| match it {
                Item::L(i) => {
                    let pl = &psd.layers()[i];
                    let rgba = pl.rgba();
                    let img = if rgba.len() == (w * h * 4) as usize {
                        Image::from_rgba8(w, h, &rgba)
                    } else {
                        Image::new(w, h)
                    };
                    let mut l = Layer::from_image(pl.name(), img);
                    l.visible = pl.visible();
                    l.opacity = pl.opacity() as f32 / 255.0;
                    l.blend = crate::blend::BlendMode::from_psd_code(pl.blend_mode() as u8);
                    l.clipped = pl.is_clipping_mask();
                    Some(l)
                }
                Item::G(gid) => {
                    let g = psd.groups().get(&gid)?;
                    let kids = build(Some(gid), psd, ch, w, h);
                    if kids.is_empty() {
                        return None;
                    }
                    let mut l = Layer::group(g.name(), kids);
                    l.visible = g.visible();
                    l.opacity = g.opacity() as f32 / 255.0;
                    let mode = g.blend_mode() as u8;
                    if let LayerKind::Group { pass_through, .. } = &mut l.kind {
                        *pass_through = mode == 0;
                    }
                    l.blend = crate::blend::BlendMode::from_psd_code(mode);
                    Some(l)
                }
            })
            .collect()
    }
    let layers = build(None, &psd, &children, w, h);
    if layers.is_empty() {
        bail!("PSD has no readable layers");
    }
    let active = layers.last().unwrap().id;
    Ok(Document {
        width: w,
        height: h,
        dpi: 72.0,
        layers,
        active,
        selection: None,
        guides: vec![],
        name: "PSD".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adjust::Adjustment;
    use crate::effects::Effect;
    use crate::text::TextStyle;

    fn sample_doc() -> Document {
        let mut d = Document::new(40, 30, Some(Color::rgb(0.9, 0.8, 0.7)));
        let id = d.new_raster_layer();
        d.layer_mut(id)
            .unwrap()
            .image_mut()
            .set(3, 3, [1.0, 0.0, 0.0, 1.0]);
        d.layer_mut(id)
            .unwrap()
            .effects
            .push(Effect::catalog().remove(1));
        d.add_mask(false, false);
        d.add_layer(Layer::text(
            TextStyle {
                text: "Hi".into(),
                size: 14.0,
                ..Default::default()
            },
            40,
            30,
        ));
        d.add_adjustment_layer(Adjustment::Posterize { levels: 3 });
        let gid = d.group_layer(id).unwrap();
        d.layer_mut(gid).unwrap().name = "My Group".into();
        d
    }

    #[test]
    fn kroma_roundtrip() {
        let d = sample_doc();
        let bytes = save_kroma_bytes(&d).unwrap();
        let back = load_kroma_bytes(&bytes).unwrap();
        assert_eq!(back.layer_count(), d.layer_count());
        let a = d.composite();
        let b = back.composite();
        let diff: f32 = a
            .pixels()
            .iter()
            .zip(b.pixels())
            .map(|(p, q)| (p[0] - q[0]).abs() + (p[3] - q[3]).abs())
            .sum();
        assert!(
            diff < 0.05 * (40 * 30) as f32 / 100.0,
            "composite drift {diff}"
        );
        assert!(matches!(
            back.flat_list()
                .iter()
                .find(|(_, l)| l.name == "My Group")
                .unwrap()
                .1
                .kind,
            LayerKind::Group { .. }
        ));
    }

    #[test]
    fn ora_roundtrip() {
        let dir = std::env::temp_dir().join(format!("kroma_ora_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.ora");
        let mut d = Document::new(20, 10, Some(Color::WHITE));
        let id = d.new_raster_layer();
        d.layer_mut(id)
            .unwrap()
            .image_mut()
            .set(5, 5, [0.0, 0.0, 1.0, 1.0]);
        d.layer_mut(id).unwrap().blend = crate::blend::BlendMode::Multiply;
        save_ora(&d, &p).unwrap();
        let back = load_ora(&p).unwrap();
        assert_eq!(back.layers.len(), 2);
        assert_eq!(back.layers[1].blend, crate::blend::BlendMode::Multiply);
        assert!(back.layers[1].image().get(5, 5)[2] > 0.99);
    }

    #[test]
    fn export_formats() {
        let d = sample_doc();
        let img = d.composite();
        for ext in EXPORT_EXTENSIONS {
            let b = encode_image(&img, ext, &ExportOptions::default())
                .unwrap_or_else(|e| panic!("{ext}: {e}"));
            assert!(!b.is_empty(), "{ext}");
            if !matches!(*ext, "ico" | "tga") {
                let back = load_image_bytes(&b).unwrap_or_else(|e| panic!("{ext} decode: {e}"));
                assert_eq!((back.width, back.height), (40, 30), "{ext}");
            }
        }
    }
}
