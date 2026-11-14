//! Đo độ phủ: phần trăm điểm mực được "đóng dấu" bởi các nét (dùng cho test & lệnh `wb analyze`).

use crate::binary::Binary;
use tiny_skia::{FillRule, LineCap, LineJoin, Mask, PathBuilder, Stroke, Transform};
use wb_model::PointW;

pub use wb_model::REVEAL_MARGIN;

/// Vẽ các nét (toạ độ theo lưới `ink`) thành mặt nạ và đo tỉ lệ điểm mực được phủ.
pub fn coverage(ink: &Binary, strokes: &[Vec<PointW>]) -> f32 {
    let Some(mut mask) = Mask::new(ink.w as u32, ink.h as u32) else {
        return 0.0;
    };
    for s in strokes {
        stamp_mask(&mut mask, s, Transform::identity());
    }
    let total = ink.count();
    if total == 0 {
        return 1.0;
    }
    let mut hit = 0;
    for y in 0..ink.h {
        for x in 0..ink.w {
            if ink.get(x, y) && mask.data()[y * ink.w + x] >= 128 {
                hit += 1;
            }
        }
    }
    hit as f32 / total as f32
}

/// Đóng dấu một nét vào mặt nạ: mỗi đoạn là một nét bút tròn rộng `2·max(w) + 2·biên`.
pub fn stamp_mask(mask: &mut Mask, pts: &[PointW], ts: Transform) {
    if pts.is_empty() {
        return;
    }
    if pts.len() == 1 {
        let p = pts[0];
        if let Some(c) = PathBuilder::from_circle(p.x, p.y, p.w + REVEAL_MARGIN) {
            mask.fill_path(&c, FillRule::Winding, true, ts);
        }
        return;
    }
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let mut pb = PathBuilder::new();
        pb.move_to(a.x, a.y);
        pb.line_to(b.x, b.y);
        let Some(path) = pb.finish() else { continue };
        let stroke = Stroke {
            width: 2.0 * (a.w.max(b.w) + REVEAL_MARGIN),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        if let Some(outline) = path.stroke(&stroke, 1.0) {
            mask.fill_path(&outline, FillRule::Winding, true, ts);
        }
    }
}
