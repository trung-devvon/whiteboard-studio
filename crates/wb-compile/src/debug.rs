//! Ảnh debug cho `wb analyze`: mực mờ + nét tô màu theo thứ tự (xanh → đỏ), chấm đen = điểm bắt đầu,
//! chấm tím = chấm vụn, đường chấm xám = quãng nhấc bút.

use tiny_skia::{Color, FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, StrokeDash, Transform};
use wb_engine::pixmap_from_rgba;

pub fn render_debug(ink: &image::RgbaImage, plan: &wb_model::DrawPlan, max_side: u32) -> Pixmap {
    let (w, h) = ink.dimensions();
    let s = (max_side as f32 / w.max(h) as f32).min(1.0);
    let (ow, oh) = (((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1));
    let mut pm = Pixmap::new(ow, oh).unwrap();
    pm.fill(Color::WHITE);
    let ts = Transform::from_scale(s, s);
    // mực mờ 25%
    if let Some(ink_pm) = pixmap_from_rgba(w, h, ink.as_raw()) {
        let paint = tiny_skia::PixmapPaint {
            opacity: 0.25,
            quality: tiny_skia::FilterQuality::Bilinear,
            ..Default::default()
        };
        pm.draw_pixmap(0, 0, ink_pm.as_ref(), &paint, ts, None);
    }
    let n = plan.strokes.len().max(1) as f32;
    let line_w = (2.0 / s).max(1.0);
    let mut prev_end: Option<wb_model::PointW> = None;
    for (i, st) in plan.strokes.iter().enumerate() {
        let t = i as f32 / n;
        if let Some(a) = prev_end {
            let b = st.points[0];
            let mut pb = PathBuilder::new();
            pb.move_to(a.x, a.y);
            pb.line_to(b.x, b.y);
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(150, 150, 150, 160);
                let stroke = Stroke {
                    width: line_w * 0.6,
                    dash: StrokeDash::new(vec![4.0 / s, 4.0 / s], 0.0),
                    ..Stroke::default()
                };
                pm.stroke_path(&path, &paint, &stroke, ts, None);
            }
        }
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(
            (40.0 + 210.0 * t) as u8,
            (110.0 - 70.0 * t) as u8,
            (230.0 - 190.0 * t) as u8,
            255,
        );
        if st.points.len() >= 2 {
            let mut pb = PathBuilder::new();
            pb.move_to(st.points[0].x, st.points[0].y);
            for p in &st.points[1..] {
                pb.line_to(p.x, p.y);
            }
            if let Some(path) = pb.finish() {
                pm.stroke_path(
                    &path,
                    &paint,
                    &Stroke {
                        width: line_w,
                        line_cap: LineCap::Round,
                        ..Stroke::default()
                    },
                    ts,
                    None,
                );
            }
        }
        let dot = |pm: &mut Pixmap, x: f32, y: f32, r: f32, c: Color| {
            if let Some(cp) = PathBuilder::from_circle(x, y, r) {
                let mut p = Paint {
                    anti_alias: true,
                    ..Paint::default()
                };
                p.set_color(c);
                pm.fill_path(&cp, &p, FillRule::Winding, ts, None);
            }
        };
        dot(&mut pm, st.points[0].x, st.points[0].y, 2.5 / s, Color::BLACK);
        for d in &st.dots {
            dot(
                &mut pm,
                d.point.x,
                d.point.y,
                2.0 / s,
                Color::from_rgba8(150, 40, 200, 255),
            );
        }
        prev_end = st.points.last().copied();
    }
    pm
}
