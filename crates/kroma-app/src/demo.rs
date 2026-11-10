//! A showcase document (KROMA_DEMO=1) used for screenshots and smoke tests.

use kroma_core::adjust::Adjustment;
use kroma_core::effects::Effect;
use kroma_core::filter::{Filter, FilterCtx};
use kroma_core::gradient::{Gradient, GradientFill, GradientShape};
use kroma_core::layer::{FillSource, Layer};
use kroma_core::shapes::{ShapeKind, ShapeStyle};
use kroma_core::text::{TextAlign, TextStyle};
use kroma_core::{Color, Document};

pub fn showcase() -> Document {
    let (w, h) = (1600u32, 1000u32);
    let mut d = Document::new(w, h, None);
    d.name = "Kroma showcase".into();
    let sky = Gradient::presets()
        .into_iter()
        .find(|(n, _)| *n == "Sunset")
        .unwrap()
        .1;
    d.layers[0] = Layer::fill(
        "Sky",
        FillSource::Gradient {
            fill: GradientFill {
                gradient: sky,
                shape: GradientShape::Linear,
                ..Default::default()
            },
            from: (0.0, 0.0),
            to: (0.0, h as f32),
        },
        w,
        h,
    );
    d.active = d.layers[0].id;
    // clouds, screen-blended
    let mut clouds = Layer::raster("Clouds", w, h);
    Filter::Clouds {
        scale: 260.0,
        seed: 11,
    }
    .apply(
        clouds.image_mut(),
        None,
        &FilterCtx {
            fg: Color::BLACK,
            bg: Color::WHITE,
        },
    );
    clouds.blend = kroma_core::blend::BlendMode::Screen;
    clouds.opacity = 0.35;
    let clouds_id = d.add_layer(clouds);
    // sun
    let mut sun = Layer::raster("Sun", w, h);
    kroma_core::shapes::draw_shape(
        sun.image_mut(),
        None,
        ShapeKind::Ellipse,
        &ShapeStyle {
            fill: Some(Color::from_hex("#ffd27a").unwrap()),
            ..Default::default()
        },
        (1080.0, 220.0),
        (1340.0, 480.0),
    );
    sun.effects.push(Effect::OuterGlow {
        enabled: true,
        color: Color::from_hex("#ffb347").unwrap(),
        opacity: 0.85,
        size: 60.0,
        spread: 0.1,
        mode: kroma_core::blend::BlendMode::Screen,
    });
    d.add_layer(sun);
    // mountains
    let mut mtn = Layer::raster("Mountains", w, h);
    let pts: Vec<(f32, f32)> = vec![
        (0.0, 1000.0),
        (0.0, 720.0),
        (220.0, 560.0),
        (420.0, 690.0),
        (640.0, 470.0),
        (900.0, 700.0),
        (1120.0, 590.0),
        (1380.0, 740.0),
        (1600.0, 620.0),
        (1600.0, 1000.0),
    ];
    let m = kroma_core::selection::polygon_mask(w, h, &pts);
    kroma_core::brush::fill_selection(
        mtn.image_mut(),
        Some(&m),
        Color::from_hex("#1b1035").unwrap(),
        1.0,
        kroma_core::blend::BlendMode::Normal,
        false,
    );
    mtn.effects.push(Effect::InnerGlow {
        enabled: true,
        color: Color::from_hex("#c33764").unwrap(),
        opacity: 0.5,
        size: 40.0,
        choke: 0.0,
        from_center: false,
        mode: kroma_core::blend::BlendMode::Screen,
    });
    d.add_layer(mtn);
    // title text with styles
    let mut title = Layer::text(
        TextStyle {
            text: "KROMA".into(),
            size: 190.0,
            color: Color::WHITE,
            x: 800.0,
            y: 300.0,
            align: TextAlign::Center,
            tracking: 18.0,
            faux_bold: true,
            ..Default::default()
        },
        w,
        h,
    );
    title.effects.push(Effect::DropShadow {
        enabled: true,
        color: Color::BLACK,
        opacity: 0.55,
        angle: 120.0,
        distance: 10.0,
        size: 18.0,
        spread: 0.0,
        mode: kroma_core::blend::BlendMode::Multiply,
    });
    title.effects.push(Effect::Stroke {
        enabled: true,
        color: Color::from_hex("#f8a14b").unwrap(),
        size: 4.0,
        position: kroma_core::brush::StrokeLocation::Outside,
        opacity: 1.0,
        mode: kroma_core::blend::BlendMode::Normal,
    });
    d.add_layer(title);
    let sub = Layer::text(
        TextStyle {
            text: "image editing, built in Rust".into(),
            size: 44.0,
            color: Color::from_hex("#fde8a8").unwrap(),
            x: 800.0,
            y: 520.0,
            align: TextAlign::Center,
            ..Default::default()
        },
        w,
        h,
    );
    d.add_layer(sub);
    // grade
    d.add_adjustment_layer(Adjustment::Vibrance {
        vibrance: 0.3,
        saturation: 0.05,
    });
    d.active = clouds_id;
    d.selection = Some(kroma_core::selection::ellipse_mask(
        w, h, 1040.0, 180.0, 1380.0, 520.0,
    ));
    d
}
