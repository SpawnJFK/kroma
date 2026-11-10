//! Hand-drawn vector tool icons (no font dependencies, crisp at any DPI).

use crate::tools::Tool;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

pub fn paint(p: &Painter, r: Rect, tool: Tool, col: Color32) {
    let c = r.center();
    let s = r.width().min(r.height()) / 24.0; // design grid 24
    let pt = |x: f32, y: f32| Pos2::new(c.x + (x - 12.0) * s, c.y + (y - 12.0) * s);
    let st = Stroke::new(1.6 * s.max(0.8), col);
    let thin = Stroke::new(1.0 * s.max(0.8), col);
    let line = |a: (f32, f32), b: (f32, f32)| p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], st);
    let poly = |pts: &[(f32, f32)], closed: bool| {
        let mut v: Vec<Pos2> = pts.iter().map(|q| pt(q.0, q.1)).collect();
        if closed {
            v.push(v[0]);
        }
        p.add(Shape::line(v, st));
    };
    let dashed = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|q| pt(q.0, q.1)).collect();
        p.extend(Shape::dashed_line(&v, thin, 2.5 * s, 2.0 * s));
    };
    match tool {
        Tool::Move => {
            line((12.0, 3.0), (12.0, 21.0));
            line((3.0, 12.0), (21.0, 12.0));
            for (a, b, d) in [
                ((12.0, 3.0), (9.0, 6.0), (15.0, 6.0)),
                ((12.0, 21.0), (9.0, 18.0), (15.0, 18.0)),
                ((3.0, 12.0), (6.0, 9.0), (6.0, 15.0)),
                ((21.0, 12.0), (18.0, 9.0), (18.0, 15.0)),
            ] {
                poly(&[b, a, d], false);
            }
        }
        Tool::MarqueeRect => dashed(&[
            (4.0, 5.0),
            (20.0, 5.0),
            (20.0, 19.0),
            (4.0, 19.0),
            (4.0, 5.0),
        ]),
        Tool::MarqueeEllipse => {
            let pts: Vec<(f32, f32)> = (0..=32)
                .map(|i| {
                    let a = i as f32 / 32.0 * std::f32::consts::TAU;
                    (12.0 + 8.5 * a.cos(), 12.0 + 7.0 * a.sin())
                })
                .collect();
            dashed(&pts);
        }
        Tool::Lasso => {
            let pts: Vec<(f32, f32)> = (0..=28)
                .map(|i| {
                    let a = i as f32 / 28.0 * std::f32::consts::TAU;
                    (12.5 + 7.5 * a.cos(), 9.0 + 5.0 * a.sin())
                })
                .collect();
            poly(&pts, false);
            poly(&[(7.0, 12.5), (8.0, 16.0), (6.0, 19.5), (8.5, 21.0)], false);
        }
        Tool::PolyLasso => {
            poly(
                &[
                    (5.0, 8.0),
                    (13.0, 4.0),
                    (20.0, 9.0),
                    (17.0, 17.0),
                    (8.0, 15.0),
                ],
                true,
            );
            poly(&[(8.0, 15.0), (7.0, 19.0), (9.0, 21.0)], false);
        }
        Tool::MagicWand => {
            line((5.0, 19.0), (15.0, 9.0));
            for (a, b) in [
                ((17.0, 3.0), (17.0, 7.0)),
                ((15.0, 5.0), (19.0, 5.0)),
                ((20.0, 9.0), (20.0, 12.0)),
                ((18.5, 10.5), (21.5, 10.5)),
            ] {
                p.line_segment([pt(a.0, a.1), pt(b.0, b.1)], thin);
            }
        }
        Tool::Crop => {
            poly(&[(7.0, 3.0), (7.0, 17.0), (21.0, 17.0)], false);
            poly(&[(3.0, 7.0), (17.0, 7.0), (17.0, 21.0)], false);
        }
        Tool::Eyedropper => {
            poly(
                &[
                    (4.0, 20.0),
                    (5.0, 16.0),
                    (13.0, 8.0),
                    (16.0, 11.0),
                    (8.0, 19.0),
                    (4.0, 20.0),
                ],
                false,
            );
            p.circle_filled(pt(17.0, 7.0), 3.2 * s, col);
            line((12.0, 7.0), (17.0, 12.0));
        }
        Tool::SpotHeal | Tool::Heal => {
            let base = [(4.0, 14.0), (14.0, 4.0), (20.0, 10.0), (10.0, 20.0)];
            poly(&base, true);
            p.rect_filled(
                Rect::from_center_size(pt(12.0, 12.0), Vec2::splat(4.0 * s)),
                0.0,
                col,
            );
            if tool == Tool::SpotHeal {
                for (x, y) in [(5.0, 5.0), (19.0, 19.0)] {
                    p.circle_filled(pt(x, y), 1.2 * s, col);
                }
            }
        }
        Tool::Clone => {
            poly(
                &[(4.0, 20.0), (20.0, 20.0), (20.0, 16.0), (4.0, 16.0)],
                true,
            );
            poly(
                &[(8.0, 16.0), (10.0, 11.0), (14.0, 11.0), (16.0, 16.0)],
                false,
            );
            p.circle_stroke(pt(12.0, 7.0), 3.2 * s, st);
        }
        Tool::Brush => {
            poly(&[(20.0, 3.0), (11.0, 13.0)], false);
            poly(&[(11.0, 13.0), (13.0, 15.0)], false);
            let tip = vec![
                pt(10.5, 12.5),
                pt(13.5, 15.5),
                pt(9.0, 20.5),
                pt(3.5, 21.0),
                pt(5.0, 17.0),
            ];
            p.add(Shape::convex_polygon(tip, col, Stroke::NONE));
        }
        Tool::Pencil => {
            poly(
                &[
                    (5.0, 19.0),
                    (17.0, 7.0),
                    (20.0, 10.0),
                    (8.0, 22.0),
                    (4.0, 23.0),
                    (5.0, 19.0),
                ],
                false,
            );
            line((15.0, 9.0), (18.0, 12.0));
        }
        Tool::Eraser => {
            poly(
                &[
                    (3.0, 15.0),
                    (12.0, 6.0),
                    (20.0, 14.0),
                    (13.0, 21.0),
                    (9.0, 21.0),
                ],
                true,
            );
            let half = vec![
                pt(3.0, 15.0),
                pt(7.5, 10.5),
                pt(15.5, 18.5),
                pt(13.0, 21.0),
                pt(9.0, 21.0),
            ];
            p.add(Shape::convex_polygon(
                half,
                col.gamma_multiply(0.6),
                Stroke::NONE,
            ));
            line((9.0, 21.0), (21.0, 21.0));
        }
        Tool::Smudge => {
            poly(
                &[
                    (9.0, 21.0),
                    (8.0, 13.0),
                    (10.0, 4.0),
                    (13.0, 4.0),
                    (13.0, 11.0),
                ],
                false,
            );
            poly(&[(13.0, 9.0), (16.0, 9.0), (16.0, 12.0)], false);
            poly(
                &[(16.0, 11.0), (19.0, 11.0), (19.0, 16.0), (16.0, 21.0)],
                false,
            );
        }
        Tool::BlurTool => {
            let pts: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let a = std::f32::consts::PI * (i as f32 / 24.0);
                    (12.0 - 6.0 * a.cos(), 14.0 + 6.0 * a.sin())
                })
                .collect();
            let mut v = vec![(12.0, 3.0)];
            v.extend(pts);
            poly(&v, true);
        }
        Tool::SharpenTool => poly(&[(12.0, 3.0), (20.0, 20.0), (4.0, 20.0)], true),
        Tool::Dodge => {
            p.circle_stroke(pt(10.0, 9.0), 6.0 * s, st);
            line((14.5, 13.5), (21.0, 20.0));
        }
        Tool::Burn => {
            poly(
                &[
                    (5.0, 14.0),
                    (5.0, 8.0),
                    (8.0, 6.0),
                    (8.0, 4.0),
                    (11.0, 4.0),
                    (11.0, 3.0),
                    (14.0, 3.0),
                    (14.0, 5.0),
                    (17.0, 5.0),
                    (17.0, 9.0),
                    (19.0, 9.0),
                    (19.0, 15.0),
                    (15.0, 21.0),
                    (9.0, 21.0),
                    (5.0, 14.0),
                ],
                false,
            );
        }
        Tool::Sponge => {
            let rr = Rect::from_min_max(pt(4.0, 6.0), pt(20.0, 18.0));
            p.rect_stroke(rr, 4.0 * s, st, egui::StrokeKind::Middle);
            for (x, y) in [
                (8.0, 10.0),
                (12.0, 13.0),
                (16.0, 10.0),
                (9.0, 15.0),
                (15.0, 15.0),
            ] {
                p.circle_filled(pt(x, y), 1.1 * s, col);
            }
        }
        Tool::Gradient => {
            let rr = Rect::from_min_max(pt(3.0, 6.0), pt(21.0, 18.0));
            let n = 9;
            for i in 0..n {
                let t = i as f32 / (n - 1) as f32;
                let x0 = rr.left() + rr.width() * i as f32 / n as f32;
                let x1 = rr.left() + rr.width() * (i + 1) as f32 / n as f32;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(x0, rr.top()), Pos2::new(x1 + 0.5, rr.bottom())),
                    0.0,
                    col.gamma_multiply(t),
                );
            }
            p.rect_stroke(rr, 0.0, thin, egui::StrokeKind::Middle);
        }
        Tool::Bucket => {
            poly(
                &[
                    (4.0, 11.0),
                    (11.0, 4.0),
                    (19.0, 12.0),
                    (12.0, 19.0),
                    (4.0, 11.0),
                ],
                false,
            );
            line((4.0, 11.0), (19.0, 12.0));
            let drop = vec![
                pt(20.0, 14.5),
                pt(22.0, 18.5),
                pt(20.0, 20.5),
                pt(18.0, 18.5),
            ];
            p.add(Shape::convex_polygon(drop, col, Stroke::NONE));
        }
        Tool::Text => {
            line((5.0, 5.0), (19.0, 5.0));
            line((12.0, 5.0), (12.0, 20.0));
            line((9.0, 20.0), (15.0, 20.0));
            p.line_segment([pt(5.0, 5.0), pt(5.0, 8.0)], st);
            p.line_segment([pt(19.0, 5.0), pt(19.0, 8.0)], st);
        }
        Tool::Shape => {
            p.rect_filled(
                Rect::from_min_max(pt(3.0, 3.0), pt(14.0, 14.0)),
                1.0,
                col.gamma_multiply(0.85),
            );
            p.circle_stroke(pt(15.0, 15.0), 6.0 * s, st);
        }
        Tool::Liquify => {
            for y in [7.0f32, 12.0, 17.0] {
                let pts: Vec<(f32, f32)> = (0..=16)
                    .map(|i| {
                        let x = 3.0 + i as f32 * 18.0 / 16.0;
                        (x, y + 2.0 * ((x / 18.0) * std::f32::consts::TAU).sin())
                    })
                    .collect();
                poly(&pts, false);
            }
        }
        Tool::Hand => {
            poly(
                &[
                    (7.0, 21.0),
                    (4.0, 14.0),
                    (5.5, 12.5),
                    (8.0, 15.0),
                    (8.0, 5.0),
                    (10.0, 4.0),
                    (11.0, 5.0),
                    (11.0, 11.0),
                    (11.0, 3.0),
                    (13.5, 3.0),
                    (14.0, 11.0),
                    (14.5, 4.5),
                    (17.0, 5.0),
                    (17.0, 12.0),
                    (17.5, 7.0),
                    (20.0, 7.5),
                    (20.0, 15.0),
                    (17.0, 21.0),
                ],
                false,
            );
        }
        Tool::Zoom => {
            p.circle_stroke(pt(10.0, 10.0), 6.0 * s, st);
            p.line_segment([pt(14.5, 14.5), pt(21.0, 21.0)], Stroke::new(2.6 * s, col));
            p.line_segment([pt(7.0, 10.0), pt(13.0, 10.0)], thin);
            p.line_segment([pt(10.0, 7.0), pt(10.0, 13.0)], thin);
        }
    }
}

/// An icon button for the toolbox. Returns the response.
pub fn tool_button(ui: &mut egui::Ui, tool: Tool, selected: bool, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
    let v = ui.visuals();
    let bg = if selected {
        v.selection.bg_fill.gamma_multiply(0.85)
    } else if resp.hovered() {
        v.widgets.hovered.weak_bg_fill
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 4.0, bg);
    let col = if selected {
        Color32::WHITE
    } else {
        v.text_color()
    };
    paint(ui.painter(), rect.shrink(size * 0.16), tool, col);
    resp
}
