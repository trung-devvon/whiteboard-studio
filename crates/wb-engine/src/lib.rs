//! Engine render tất định: cho dự án đã "biên dịch" (lớp mực + DrawPlan + Timeline) → khung hình tại `t`.
//!
//! **Không I/O** — ảnh được nạp sẵn thành [`Pixmap`] bởi bên gọi → biên dịch được sang wasm32
//! (Player trong app) lẫn native (xuất video). Cùng code → Preview = Export (ADR-0002).
//!
//! Cách dựng khung (ADR-0001, reveal):
//! - `base`: nền scene + các element **đã vẽ xong** (được "nướng" vào, không tính lại).
//! - Element **đang vẽ**: lớp mực (đã đặt vào vị trí màn hình, chỉ trong hộp bao) × mặt nạ phủ.
//!   Mặt nạ được đóng dấu dần theo đường tâm nét; đóng dấu chồng lên nhau không làm đậm thêm.
//! - Pha hoàn thiện: mặt nạ được nâng đều lên 100% → khung cuối **khớp đúng** ảnh gốc.
//! - Bàn tay vẽ trên cùng, kích thước theo màn hình (không phóng theo ảnh).
//!
//! Render tuần tự nhanh nhất (chỉ đóng dấu phần nét mới). Tua ngược → dựng lại scene từ đầu.

mod bundle;
mod hand;
mod reveal;

pub use bundle::{Bundle, BundleElement, BundleError, BundleHand, BundleScene, RawImage};
pub use hand::HandSprite;
pub use tiny_skia::Pixmap;

use reveal::Active;
use tiny_skia::{Color, IntRect, PixmapPaint, Transform};
use wb_model::DrawPlan;
use wb_timing::{Progress, Timeline, progress_at};

/// Vị trí ảnh trên khung tham chiếu: điểm ảnh nguồn (x, y) → (ox + x·scale, oy + y·scale).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
}

/// Một element đã chuẩn bị sẵn.
pub struct ElementAsset {
    /// Lớp mực ở độ phân giải ảnh nguồn (premultiplied).
    pub ink: Pixmap,
    pub place: Place,
    pub plan: DrawPlan,
}

pub struct SceneAsset {
    pub background: [u8; 4],
    pub elements: Vec<ElementAsset>,
}

/// Tạo Pixmap từ RGBA alpha **thẳng** (như ảnh PNG) — tự premultiply.
pub fn pixmap_from_rgba(width: u32, height: u32, rgba: &[u8]) -> Option<Pixmap> {
    let mut data = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        let a = px[3] as u16;
        let pm = |c: u8| ((c as u16 * a + 127) / 255) as u8;
        data.extend_from_slice(&[pm(px[0]), pm(px[1]), pm(px[2]), px[3]]);
    }
    Pixmap::from_vec(data, tiny_skia::IntSize::from_wh(width, height)?)
}

pub struct Engine {
    out_w: u32,
    out_h: u32,
    /// px khung tham chiếu → px đầu ra
    k: f32,
    scenes: Vec<SceneAsset>,
    timeline: Timeline,
    hand: Option<HandSprite>,
    // trạng thái render tuần tự
    scene: Option<usize>,
    base: Pixmap,
    frame: Pixmap,
    baked: usize,
    active: Option<Active>,
    last_t: f64,
}

impl Engine {
    /// `reference_width`: chiều rộng khung tham chiếu (1920) — để quy đổi sang độ phân giải đầu ra.
    pub fn new(
        out_w: u32,
        out_h: u32,
        reference_width: f32,
        scenes: Vec<SceneAsset>,
        timeline: Timeline,
        hand: Option<HandSprite>,
    ) -> Self {
        assert_eq!(scenes.len(), timeline.scenes.len(), "số scene không khớp Timeline");
        let k = out_w as f32 / reference_width;
        // chuẩn bị tay một lần: thu nhỏ đúng cỡ đầu ra
        let hand = hand.map(|h| h.prepared(k));
        Self {
            out_w,
            out_h,
            k,
            scenes,
            timeline,
            hand,
            scene: None,
            base: Pixmap::new(out_w, out_h).expect("kích thước khung"),
            frame: Pixmap::new(out_w, out_h).expect("kích thước khung"),
            baked: 0,
            active: None,
            last_t: f64::NEG_INFINITY,
        }
    }

    pub fn duration_ms(&self) -> f64 {
        self.timeline.total_ms
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    /// Khung hình tại `t_ms`. Gọi với `t` tăng dần là nhanh nhất.
    pub fn render(&mut self, t_ms: f64) -> &Pixmap {
        let si = self
            .timeline
            .scenes
            .iter()
            .position(|s| t_ms < s.end_ms)
            .unwrap_or(self.timeline.scenes.len().saturating_sub(1));
        if self.timeline.scenes.is_empty() {
            self.frame.fill(Color::WHITE);
            return &self.frame;
        }
        if self.scene != Some(si) || t_ms < self.last_t {
            self.reset_scene(si);
        }
        self.last_t = t_ms;

        let tl = &self.timeline.scenes[si];
        let scene = &self.scenes[si];
        let mut pen: Option<(f32, f32)> = None;
        // nướng các element đã xong, kích hoạt element hiện tại
        while self.baked < scene.elements.len() {
            let i = self.baked;
            let el = &scene.elements[i];
            let et = &tl.elements[i];
            let prog = progress_at(et, &el.plan, t_ms);
            if prog == Progress::Done {
                let act = match self.active.take() {
                    Some(a) if a.index == i => a,
                    _ => Active::new(i, el, self.k, self.out_w, self.out_h),
                };
                act.bake_into(&mut self.base);
                self.baked += 1;
                continue;
            }
            if self.active.as_ref().map(|a| a.index) != Some(i) {
                self.active = Some(Active::new(i, el, self.k, self.out_w, self.out_h));
            }
            let act = self.active.as_mut().unwrap();
            pen = act.advance(el, prog, self.k, self.out_w, self.out_h, self.hand.is_some());
            break;
        }

        // ghép khung
        self.frame.data_mut().copy_from_slice(self.base.data());
        if let Some(act) = self.active.as_mut() {
            act.composite_onto(&mut self.frame);
        }
        if let (Some(hand), Some((x, y))) = (&self.hand, pen) {
            hand.draw(&mut self.frame, x, y, self.k);
        }
        &self.frame
    }

    fn reset_scene(&mut self, si: usize) {
        let bg = self.scenes[si].background;
        self.base.fill(Color::from_rgba8(bg[0], bg[1], bg[2], 255));
        self.scene = Some(si);
        self.baked = 0;
        self.active = None;
    }
}

/// Hộp bao (px đầu ra) của một element, đã kẹp vào khung.
fn screen_rect(el: &ElementAsset, k: f32, out_w: u32, out_h: u32) -> Option<IntRect> {
    let s = el.place.scale * k;
    let x0 = (el.place.ox * k).floor().max(0.0);
    let y0 = (el.place.oy * k).floor().max(0.0);
    let x1 = (el.place.ox * k + el.ink.width() as f32 * s).ceil().min(out_w as f32);
    let y1 = (el.place.oy * k + el.ink.height() as f32 * s).ceil().min(out_h as f32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    IntRect::from_xywh(x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// Biến đổi toạ độ ảnh nguồn → toạ độ cục bộ trong hộp bao.
fn local_transform(el: &ElementAsset, k: f32, rect: IntRect) -> Transform {
    let s = el.place.scale * k;
    Transform::from_row(
        s,
        0.0,
        0.0,
        s,
        el.place.ox * k - rect.x() as f32,
        el.place.oy * k - rect.y() as f32,
    )
}

/// Vẽ lớp mực của element vào một pixmap cục bộ (cỡ hộp bao) — lọc song tuyến khi co giãn.
fn place_ink(el: &ElementAsset, k: f32, rect: IntRect) -> Pixmap {
    let mut local = Pixmap::new(rect.width(), rect.height()).expect("hộp bao");
    let paint = PixmapPaint {
        quality: tiny_skia::FilterQuality::Bilinear,
        ..PixmapPaint::default()
    };
    local.draw_pixmap(0, 0, el.ink.as_ref(), &paint, local_transform(el, k, rect), None);
    local
}

#[cfg(test)]
mod tests;
