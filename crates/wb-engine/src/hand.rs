//! Ảnh bàn tay cầm bút, vẽ sao cho **mũi bút** trùng vị trí nét đang vẽ.
//!
//! Hai kiểu hiển thị (chọn trong dự án, xem `wb_model::HandMode`):
//! - **cả cánh tay** — dùng nguyên ảnh; nên để tay to (`hand_scale` > 1) để chỗ ảnh bị cắt ở cẳng tay
//!   thường nằm ngoài khung.
//! - **tới cổ tay** — [`HandSprite::crop_at_wrist`] cắt bỏ phần cẳng tay sau đường cổ tay (khai báo
//!   trong `hand.json`), mép cắt mờ dần → bàn tay "lơ lửng" gọn gàng như nhiều video whiteboard.
//!
//! Trước khi render, [`HandSprite::prepared`] thu nhỏ ảnh tay **một lần** về đúng cỡ hiển thị bằng lọc
//! trung bình vùng → viền mịn, không phải co ảnh 3500px ở mỗi khung.

use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, PremultipliedColorU8, Transform};

#[derive(Clone)]
pub struct HandSprite {
    /// Ảnh tay (premultiplied), nền trong suốt.
    pub pixmap: Pixmap,
    /// Toạ độ mũi bút trong ảnh tay (px của `pixmap`).
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// Chiều cao hiển thị của cả ảnh `pixmap` (px khung tham chiếu 1080p) — tay không phóng theo ảnh/camera.
    pub display_height: f32,
}

impl HandSprite {
    /// Vẽ tay với mũi bút tại (x, y) px đầu ra. `k` = px đầu ra / px khung tham chiếu.
    pub fn draw(&self, frame: &mut Pixmap, x: f32, y: f32, k: f32) {
        let s = self.display_height * k / self.pixmap.height() as f32;
        let exact = (s - 1.0).abs() < 1e-3;
        let ts = if exact {
            // đã chuẩn bị đúng cỡ: chỉ dịch nguyên pixel → không nhoè (lệch ≤ 0,5px không thấy được)
            Transform::from_translate((x - self.anchor_x).round(), (y - self.anchor_y).round())
        } else {
            Transform::from_row(s, 0.0, 0.0, s, x - self.anchor_x * s, y - self.anchor_y * s)
        };
        let paint = PixmapPaint {
            quality: if exact {
                FilterQuality::Nearest
            } else {
                FilterQuality::Bilinear
            },
            ..PixmapPaint::default()
        };
        frame.draw_pixmap(0, 0, self.pixmap.as_ref(), &paint, ts, None);
    }

    /// Bản đã thu nhỏ đúng cỡ cho độ phân giải đầu ra (`k` = px đầu ra / px tham chiếu).
    pub fn prepared(&self, k: f32) -> HandSprite {
        let s = self.display_height * k / self.pixmap.height() as f32;
        let scaled = resample(&self.pixmap, s);
        let sx = scaled.width() as f32 / self.pixmap.width() as f32;
        let sy = scaled.height() as f32 / self.pixmap.height() as f32;
        HandSprite {
            anchor_x: self.anchor_x * sx,
            anchor_y: self.anchor_y * sy,
            display_height: scaled.height() as f32 / k,
            pixmap: scaled,
        }
    }

    /// Cắt bỏ phần cẳng tay sau **đường cổ tay** `wrist = [điểm mép trên, điểm mép dưới]` (px của `pixmap`).
    /// Phía có mũi bút được giữ; mép cắt mờ dần trong `feather` px. Ảnh được thu gọn về phần còn lại.
    pub fn crop_at_wrist(self, wrist: [[f32; 2]; 2], feather: f32) -> HandSprite {
        let [a, b] = wrist;
        // pháp tuyến của đường cổ tay, hướng về phía mũi bút
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-3 {
            return self;
        }
        let mut n = [-dy / len, dx / len];
        let side = |x: f32, y: f32, n: [f32; 2]| (x - a[0]) * n[0] + (y - a[1]) * n[1];
        if side(self.anchor_x, self.anchor_y, n) < 0.0 {
            n = [-n[0], -n[1]];
        }
        let feather = feather.max(0.5);
        let (w, h) = (self.pixmap.width(), self.pixmap.height());
        let mut pm = self.pixmap.clone();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
        let px = pm.pixels_mut();
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) as usize;
                // d > 0: phía bàn tay; mờ dần trong [0, feather]
                let d = side(x as f32 + 0.5, y as f32 + 0.5, n);
                let t = (d / feather).clamp(0.0, 1.0);
                let f = t * t * (3.0 - 2.0 * t);
                let c = px[i];
                if f < 1.0 {
                    let m = |v: u8| (v as f32 * f).round() as u8;
                    px[i] = PremultipliedColorU8::from_rgba(m(c.red()), m(c.green()), m(c.blue()), m(c.alpha()))
                        .unwrap_or(PremultipliedColorU8::TRANSPARENT);
                }
                if px[i].alpha() > 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        if x1 < x0 || y1 < y0 {
            return self;
        }
        // thu gọn về hộp bao phần còn lại
        let (cw, ch) = (x1 - x0 + 1, y1 - y0 + 1);
        let mut out = Pixmap::new(cw, ch).expect("cỡ tay");
        out.draw_pixmap(
            -(x0 as i32),
            -(y0 as i32),
            pm.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        HandSprite {
            pixmap: out,
            anchor_x: self.anchor_x - x0 as f32,
            anchor_y: self.anchor_y - y0 as f32,
            // giữ nguyên tỉ lệ px ảnh → px màn hình
            display_height: self.display_height * ch as f32 / h as f32,
        }
    }
}

/// Đổi cỡ theo hệ số `s`. Thu nhỏ mạnh: gộp khối f×f (trung bình, đúng với premultiplied) rồi bicubic.
fn resample(src: &Pixmap, s: f32) -> Pixmap {
    let tw = ((src.width() as f32 * s).round() as u32).max(1);
    let th = ((src.height() as f32 * s).round() as u32).max(1);
    if tw == src.width() && th == src.height() {
        return src.clone();
    }
    let f = if s < 1.0 { (1.0 / s).floor().max(1.0) as u32 } else { 1 };
    let boxed;
    let mid = if f > 1 {
        boxed = box_down(src, f);
        &boxed
    } else {
        src
    };
    let mut out = Pixmap::new(tw, th).expect("cỡ tay");
    let paint = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..PixmapPaint::default()
    };
    let ts = Transform::from_scale(tw as f32 / mid.width() as f32, th as f32 / mid.height() as f32);
    out.draw_pixmap(0, 0, mid.as_ref(), &paint, ts, None);
    out
}

/// Thu nhỏ nguyên lần `f`: mỗi pixel đích = trung bình khối f×f.
fn box_down(src: &Pixmap, f: u32) -> Pixmap {
    let (w, h) = (src.width() / f, src.height() / f);
    let mut out = Pixmap::new(w.max(1), h.max(1)).expect("cỡ tay");
    let sw = src.width() as usize;
    let sp = src.pixels();
    let n = f * f;
    let op = out.pixels_mut();
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in 0..f {
                let row = (y * f + dy) as usize * sw;
                for dx in 0..f {
                    let c = sp[row + (x * f + dx) as usize];
                    acc[0] += c.red() as u32;
                    acc[1] += c.green() as u32;
                    acc[2] += c.blue() as u32;
                    acc[3] += c.alpha() as u32;
                }
            }
            let avg = |v: u32| ((v + n / 2) / n) as u8;
            let a = avg(acc[3]);
            op[(y * w + x) as usize] =
                PremultipliedColorU8::from_rgba(avg(acc[0]).min(a), avg(acc[1]).min(a), avg(acc[2]).min(a), a)
                    .unwrap_or(PremultipliedColorU8::TRANSPARENT);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Color;

    /// Thanh ngang đỏ 100×20 trên ảnh 120×40; mũi bút ở đầu trái.
    fn bar() -> HandSprite {
        let mut p = Pixmap::new(120, 40).unwrap();
        let red = PremultipliedColorU8::from_rgba(255, 0, 0, 255).unwrap();
        let w = p.width() as usize;
        for y in 10..30 {
            for x in 0..100 {
                p.pixels_mut()[y * w + x] = red;
            }
        }
        HandSprite {
            pixmap: p,
            anchor_x: 0.0,
            anchor_y: 20.0,
            display_height: 40.0,
        }
    }

    #[test]
    fn crop_at_wrist_keeps_pen_side_and_fades_edge() {
        // "cổ tay" là đường dọc x = 60
        let c = bar().crop_at_wrist([[60.0, 0.0], [60.0, 40.0]], 8.0);
        assert!(
            c.pixmap.width() <= 61,
            "phần sau cổ tay phải bị bỏ, rộng {}",
            c.pixmap.width()
        );
        assert_eq!(c.pixmap.height(), 20, "thu gọn về hộp bao");
        assert_eq!((c.anchor_x, c.anchor_y), (0.0, 10.0));
        // tỉ lệ px ảnh → màn hình không đổi: 40px ảnh = 40 → 20px ảnh = 20
        assert!((c.display_height - 20.0).abs() < 1e-3);
        let w = c.pixmap.width() as usize;
        let at = |x: usize| c.pixmap.pixels()[10 * w + x].alpha();
        assert_eq!(at(10), 255, "bàn tay giữ nguyên");
        assert!(at(55) > 0 && at(55) < 255, "mép cắt mờ dần");
    }

    #[test]
    fn prepared_downscales_to_display_size() {
        let mut big = bar();
        big.pixmap = resample(&big.pixmap, 10.0); // 1200×400
        big.anchor_y *= 10.0;
        let p = big.prepared(1.0);
        assert_eq!(p.pixmap.height(), 40);
        assert!((p.anchor_y - 20.0).abs() < 0.6);
        // vẽ đúng cỡ: không phóng thêm
        let mut frame = Pixmap::new(200, 100).unwrap();
        frame.fill(Color::WHITE);
        p.draw(&mut frame, 50.0, 50.0, 1.0);
        let c = frame.pixels()[50 * 200 + 60];
        assert!(c.red() > 250 && c.green() < 10);
    }
}
