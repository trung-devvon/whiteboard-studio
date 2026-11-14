//! SPIKE S2 — engine "reveal" bằng tiny-skia, biên dịch sang WASM để phát xem trước trong WebView.
//!
//! Câu hỏi: tiny-skia trong WASM có vẽ kịp ≥ 30fps ở 960×540 với ~2.000 nét không?
//! Đồng thời là đầu nhận của SPIKE S5: DrawPlan nhị phân (postcard) gửi từ Rust qua IPC → nạp vào đây.
//!
//! Kỹ thuật reveal thử nghiệm: KHÔNG ghép mask cả khung mỗi frame. Thay vào đó "đóng dấu" đoạn nét mới
//! lên lớp tích lũy bằng **Pattern shader lấy từ ảnh nguồn** → chỉ tốn công ở vùng nét mới,
//! rồi copy lớp tích lũy + vẽ bàn tay. Cùng code này sẽ chạy native khi xuất video.

use tiny_skia::{
    Color, FillRule, FilterQuality, LineCap, Paint, PathBuilder, Pattern, Pixmap, PixmapPaint, SpreadMode, Stroke,
    Transform,
};
use wasm_bindgen::prelude::*;
use wb_model::{DrawPlan, PlannedStroke, PointW, StrokeKind};

/// Sinh DrawPlan giả lập: `n` nét ngoằn ngoèo trong khung `w×h`, tất định theo `seed`.
pub fn synthetic_plan(n: u32, seed: u32, w: u32, h: u32) -> DrawPlan {
    let mut s = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut rnd = move || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f32 / (1u64 << 53) as f32
    };
    let mut strokes = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let (mut x, mut y) = (rnd() * w as f32, rnd() * h as f32);
        let mut a = rnd() * std::f32::consts::TAU;
        let width = 1.0 + rnd() * 2.5;
        let pts: Vec<PointW> = (0..20)
            .map(|_| {
                a += (rnd() - 0.5) * 0.8;
                x = (x + a.cos() * 6.0).clamp(0.0, w as f32 - 1.0);
                y = (y + a.sin() * 6.0).clamp(0.0, h as f32 - 1.0);
                PointW { x, y, w: width }
            })
            .collect();
        strokes.push(PlannedStroke::new(StrokeKind::Outline, pts));
    }
    DrawPlan::new(w, h, strokes)
}

/// DrawPlan giả lập đã mã hoá postcard (dùng cho IPC nhị phân ở S5).
pub fn synthetic_plan_bytes(n: u32, seed: u32, w: u32, h: u32) -> Vec<u8> {
    postcard::to_allocvec(&synthetic_plan(n, seed, w, h)).expect("postcard encode")
}

#[wasm_bindgen(js_name = syntheticPlanBytes)]
pub fn synthetic_plan_bytes_js(n: u32, seed: u32, w: u32, h: u32) -> Vec<u8> {
    synthetic_plan_bytes(n, seed, w, h)
}

fn path_of(pts: &[PointW]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    pb.move_to(pts[0].x, pts[0].y);
    for p in &pts[1..] {
        pb.line_to(p.x, p.y);
    }
    pb.finish()
}

#[wasm_bindgen]
pub struct Player {
    w: u32,
    h: u32,
    source: Pixmap,   // "ảnh nguồn" (ở app thật: ảnh người dùng); ở spike: vẽ từ chính các nét
    revealed: Pixmap, // lớp tích lũy: nền + phần ảnh đã được mở
    frame: Pixmap,    // khung hình xuất ra canvas
    hand: Pixmap,
    plan: DrawPlan,
    cum: Vec<f32>, // độ dài tích luỹ tới đầu mỗi nét
    total: f32,
    drawn: f32,
}

#[wasm_bindgen]
impl Player {
    /// Nạp DrawPlan nhị phân (postcard). Toạ độ nét được scale về khung `w×h`.
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, plan_bytes: &[u8]) -> Result<Player, JsError> {
        let mut plan: DrawPlan = postcard::from_bytes(plan_bytes).map_err(|e| JsError::new(&e.to_string()))?;
        let (sx, sy) = (
            w as f32 / plan.source_width as f32,
            h as f32 / plan.source_height as f32,
        );
        for s in &mut plan.strokes {
            for p in &mut s.points {
                p.x *= sx;
                p.y *= sy;
            }
        }
        // Ảnh nguồn: vẽ các nét màu mực
        let mut source = Pixmap::new(w, h).ok_or_else(|| JsError::new("kích thước sai"))?;
        source.fill(Color::WHITE);
        let mut paint = Paint::default();
        paint.set_color_rgba8(25, 25, 35, 255);
        paint.anti_alias = true;
        for s in &plan.strokes {
            if let Some(path) = path_of(&s.points) {
                let st = Stroke {
                    width: s.points[0].w * 2.0,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                };
                source.stroke_path(&path, &paint, &st, Transform::identity(), None);
            }
        }
        let mut revealed = Pixmap::new(w, h).unwrap();
        revealed.fill(Color::WHITE);
        // Bàn tay giả: hình tròn màu da 70px (spike — ảnh tay thật ở Giai đoạn 1)
        let mut hand = Pixmap::new(140, 140).unwrap();
        if let Some(c) = PathBuilder::from_circle(70.0, 70.0, 60.0) {
            let mut p = Paint::default();
            p.set_color_rgba8(240, 195, 160, 235);
            p.anti_alias = true;
            hand.fill_path(&c, &p, FillRule::Winding, Transform::identity(), None);
        }
        let mut cum = Vec::with_capacity(plan.strokes.len());
        let mut total = 0.0;
        for s in &plan.strokes {
            cum.push(total);
            total += s.length;
        }
        Ok(Player {
            w,
            h,
            frame: revealed.clone(),
            source,
            revealed,
            hand,
            plan,
            cum,
            total,
            drawn: 0.0,
        })
    }

    #[wasm_bindgen(js_name = strokeCount)]
    pub fn stroke_count(&self) -> u32 {
        self.plan.strokes.len() as u32
    }

    /// Vẽ khung tại tiến độ `progress` ∈ [0,1]. Trả con trỏ tới dữ liệu RGBA (w*h*4 byte) trong bộ nhớ WASM.
    pub fn render(&mut self, progress: f32) -> *const u8 {
        let target = progress.clamp(0.0, 1.0) * self.total;
        if target < self.drawn {
            // tua ngược: làm lại từ đầu (đo chi phí seek)
            self.revealed.fill(Color::WHITE);
            self.drawn = 0.0;
        }
        let pen = self.stamp_range(self.drawn, target);
        self.drawn = target;
        self.frame.data_mut().copy_from_slice(self.revealed.data());
        if let Some((x, y)) = pen {
            self.frame.draw_pixmap(
                (x - 20.0) as i32,
                (y - 20.0) as i32,
                self.hand.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        self.frame.data().as_ptr()
    }

    #[wasm_bindgen(js_name = frameLen)]
    pub fn frame_len(&self) -> usize {
        (self.w * self.h * 4) as usize
    }
}

impl Player {
    /// Đóng dấu phần nét trong khoảng độ dài [from, to) lên lớp tích lũy bằng pattern của ảnh nguồn.
    fn stamp_range(&mut self, from: f32, to: f32) -> Option<(f32, f32)> {
        if to <= from {
            return None;
        }
        let pattern = Pattern::new(
            self.source.as_ref(),
            SpreadMode::Pad,
            FilterQuality::Nearest,
            1.0,
            Transform::identity(),
        );
        let paint = Paint {
            shader: pattern,
            anti_alias: true,
            ..Paint::default()
        };
        let first = self.cum.partition_point(|&c| c <= from).saturating_sub(1);
        let mut pen = None;
        for i in first..self.plan.strokes.len() {
            let start = self.cum[i];
            if start >= to {
                break;
            }
            let s = &self.plan.strokes[i];
            let (a, b) = ((from - start).max(0.0), (to - start).min(s.length));
            if b <= a {
                continue;
            }
            let mut pts = Vec::new();
            let mut acc = 0.0;
            for seg in s.points.windows(2) {
                let l = ((seg[1].x - seg[0].x).powi(2) + (seg[1].y - seg[0].y).powi(2)).sqrt();
                if acc + l >= a && acc <= b && l > 0.0 {
                    let t0 = ((a - acc) / l).clamp(0.0, 1.0);
                    let t1 = ((b - acc) / l).clamp(0.0, 1.0);
                    let lerp = |t: f32| PointW {
                        x: seg[0].x + (seg[1].x - seg[0].x) * t,
                        y: seg[0].y + (seg[1].y - seg[0].y) * t,
                        w: seg[0].w,
                    };
                    if pts.is_empty() {
                        pts.push(lerp(t0));
                    }
                    pts.push(lerp(t1));
                }
                acc += l;
            }
            if pts.len() >= 2 {
                if let Some(path) = path_of(&pts) {
                    let st = Stroke {
                        width: pts[0].w * 2.0 + 3.0,
                        line_cap: LineCap::Round,
                        ..Stroke::default()
                    };
                    self.revealed
                        .stroke_path(&path, &paint, &st, Transform::identity(), None);
                }
                let last = pts.last().unwrap();
                pen = Some((last.x, last.y));
            }
        }
        pen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_progress_reveals_whole_source() {
        let bytes = synthetic_plan_bytes(200, 7, 320, 180);
        let mut p = Player::new(320, 180, &bytes).unwrap();
        p.render(1.0);
        // Mọi điểm mực của ảnh nguồn phải đã được mở ở lớp tích lũy
        let diff = p
            .source
            .pixels()
            .iter()
            .zip(p.revealed.pixels())
            .filter(|(s, r)| s.red() < 128 && r.red() > 200)
            .count();
        assert_eq!(diff, 0, "{diff} điểm mực chưa được reveal");
    }
}
