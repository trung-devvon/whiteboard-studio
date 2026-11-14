//! Element đang vẽ: lớp mực cục bộ + mặt nạ phủ đóng dấu dần theo nét.

use crate::{ElementAsset, local_transform, place_ink, screen_rect};
use tiny_skia::{FillRule, IntRect, LineCap, LineJoin, Mask, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform};
use wb_model::{PointW, REVEAL_MARGIN};
use wb_timing::Progress;

pub(crate) struct Active {
    pub index: usize,
    rect: Option<IntRect>,
    ink: Option<Pixmap>,
    mask: Option<Mask>,
    /// bản sao để ghép mỗi khung (tránh cấp phát lại)
    scratch: Option<Pixmap>,
    settle: Option<Mask>,
    /// đã đóng dấu tới: (nét, độ dài cung)
    done_stroke: usize,
    done_s: f32,
    settle_u: f32,
}

impl Active {
    pub fn new(index: usize, el: &ElementAsset, k: f32, out_w: u32, out_h: u32) -> Self {
        let rect = screen_rect(el, k, out_w, out_h);
        let ink = rect.map(|r| place_ink(el, k, r));
        let mask = rect.and_then(|r| Mask::new(r.width(), r.height()));
        Self {
            index,
            rect,
            scratch: ink.clone(),
            ink,
            mask,
            settle: None,
            done_stroke: 0,
            done_s: 0.0,
            settle_u: 0.0,
        }
    }

    /// Đóng dấu tới trạng thái `prog`; trả vị trí mũi bút (px đầu ra) nếu tay đang hiện.
    pub fn advance(
        &mut self,
        el: &ElementAsset,
        prog: Progress,
        k: f32,
        out_w: u32,
        out_h: u32,
        want_pen: bool,
    ) -> Option<(f32, f32)> {
        let strokes = &el.plan.strokes;
        let to_screen = |p: PointW| {
            (
                el.place.ox * k + p.x * el.place.scale * k,
                el.place.oy * k + p.y * el.place.scale * k,
            )
        };
        // điểm "ngoài khung" để tay đi vào / rút ra (góc dưới-phải)
        let offscreen = (out_w as f32 * 1.08, out_h as f32 * 1.15);
        let smooth = |u: f32| u * u * (3.0 - 2.0 * u);
        let lerp = |a: (f32, f32), b: (f32, f32), u: f32| (a.0 + (b.0 - a.0) * u, a.1 + (b.1 - a.1) * u);

        match prog {
            Progress::Entering { u } => {
                let first = strokes.first().and_then(|s| s.points.first()).copied()?;
                want_pen.then(|| lerp(offscreen, to_screen(first), smooth(u)))
            }
            Progress::Drawing { stroke, s } => {
                self.stamp_until(el, stroke, s, k);
                want_pen.then(|| to_screen(strokes[stroke].point_at(s)))
            }
            Progress::Lifting { stroke, u } => {
                self.stamp_until(el, stroke, 0.0, k);
                let a = to_screen(*strokes[stroke - 1].points.last().unwrap());
                let b = to_screen(strokes[stroke].points[0]);
                want_pen.then(|| {
                    let (x, y) = lerp(a, b, smooth(u));
                    // nhấc bút: cung nhẹ lên trên
                    let lift = (std::f32::consts::PI * u).sin() * 10.0 * k;
                    (x, y - lift)
                })
            }
            Progress::Settling { u } => {
                self.stamp_until(el, strokes.len(), 0.0, k);
                self.settle_u = u;
                let last = strokes.last().and_then(|s| s.points.last()).copied()?;
                want_pen.then(|| lerp(to_screen(last), offscreen, smooth(u)))
            }
            Progress::Done => None,
        }
    }

    /// Đóng dấu mọi phần nét trước vị trí (nét `upto`, độ dài `s`).
    fn stamp_until(&mut self, el: &ElementAsset, upto: usize, s: f32, k: f32) {
        let (Some(rect), Some(mask)) = (self.rect, self.mask.as_mut()) else {
            return;
        };
        let ts = local_transform(el, k, rect);
        let strokes = &el.plan.strokes;
        while self.done_stroke < upto.min(strokes.len()) {
            let st = &strokes[self.done_stroke];
            stamp(mask, &st.sub_path(self.done_s, st.length), ts);
            for d in st.dots.iter().filter(|d| d.at >= self.done_s) {
                stamp(mask, &[d.point], ts);
            }
            self.done_stroke += 1;
            self.done_s = 0.0;
        }
        if upto < strokes.len() && s > self.done_s {
            let st = &strokes[upto];
            stamp(mask, &st.sub_path(self.done_s, s), ts);
            for d in st.dots.iter().filter(|d| d.at >= self.done_s && d.at < s) {
                stamp(mask, &[d.point], ts);
            }
            self.done_s = s;
        }
    }

    /// Ghép lớp mực × mặt nạ (nâng theo pha hoàn thiện) lên khung.
    pub fn composite_onto(&mut self, frame: &mut Pixmap) {
        let (Some(rect), Some(ink), Some(mask), Some(scratch)) =
            (self.rect, &self.ink, &self.mask, self.scratch.as_mut())
        else {
            return;
        };
        scratch.data_mut().copy_from_slice(ink.data());
        if self.settle_u > 0.0 {
            let floor = (self.settle_u.clamp(0.0, 1.0) * 255.0).round() as u8;
            let settle = self.settle.get_or_insert_with(|| mask.clone());
            for (dst, src) in settle.data_mut().iter_mut().zip(mask.data()) {
                *dst = (*src).max(floor);
            }
            scratch.apply_mask(settle);
        } else {
            scratch.apply_mask(mask);
        }
        frame.draw_pixmap(
            rect.x(),
            rect.y(),
            scratch.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }

    /// Element xong: vẽ toàn bộ lớp mực vào nền (không mặt nạ) → khớp đúng ảnh gốc.
    pub fn bake_into(&self, base: &mut Pixmap) {
        if let (Some(rect), Some(ink)) = (self.rect, &self.ink) {
            base.draw_pixmap(
                rect.x(),
                rect.y(),
                ink.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
    }
}

/// Đóng dấu đường gấp khúc vào mặt nạ: nét bút tròn rộng `2·(max w + biên)` cho mỗi đoạn.
pub(crate) fn stamp(mask: &mut Mask, pts: &[PointW], ts: Transform) {
    match pts {
        [] => {}
        [p] => {
            if let Some(c) = PathBuilder::from_circle(p.x, p.y, p.w + REVEAL_MARGIN) {
                mask.fill_path(&c, FillRule::Winding, true, ts);
            }
        }
        _ => {
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
    }
}
