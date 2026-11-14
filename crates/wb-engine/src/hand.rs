//! Ảnh bàn tay cầm bút, vẽ sao cho **mũi bút** trùng vị trí nét đang vẽ.

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

pub struct HandSprite {
    /// Ảnh tay (premultiplied), nền trong suốt.
    pub pixmap: Pixmap,
    /// Toạ độ mũi bút trong ảnh tay (px của `pixmap`).
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// Chiều cao hiển thị (px khung tham chiếu 1080p) — tay không phóng theo ảnh/camera.
    pub display_height: f32,
}

impl HandSprite {
    /// Vẽ tay với mũi bút tại (x, y) px đầu ra. `k` = px đầu ra / px khung tham chiếu.
    pub fn draw(&self, frame: &mut Pixmap, x: f32, y: f32, k: f32) {
        let s = self.display_height * k / self.pixmap.height() as f32;
        let ts = Transform::from_row(s, 0.0, 0.0, s, x - self.anchor_x * s, y - self.anchor_y * s);
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        frame.draw_pixmap(0, 0, self.pixmap.as_ref(), &paint, ts, None);
    }
}
