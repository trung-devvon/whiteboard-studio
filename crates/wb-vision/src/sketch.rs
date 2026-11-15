//! **Nét phác thảo** cho ảnh màu / ảnh chụp, không cần AI: lọc *Difference of Gaussians* (họ XDoG).
//!
//! Ý tưởng: làm mờ độ sáng ở hai mức (σ và 1,6σ) rồi lấy hiệu. Nơi nào **tối hơn xung quanh**
//! (đường viền, nếp gấp, mép vật thể trên nền sáng) cho hiệu âm → thành nét chì.
//! Kết quả gồm mặt nạ nét (để rút xương → đường tay đi) và **lớp phác thảo** màu chì có độ đậm mềm
//! (để reveal, trông như vẽ bút chì chứ không phải ngưỡng cứng).

use crate::binary::Binary;
use crate::filters::{gaussian_blur, luminance_over_white};
use image::{Rgba, RgbaImage};

/// Kết quả phác thảo ở độ phân giải làm việc.
pub struct Sketch {
    /// Mặt nạ nét (để rút xương).
    pub mask: Binary,
    /// Lớp phác thảo: màu chì, alpha theo độ đậm nét.
    pub layer: RgbaImage,
}

/// Màu "chì" của lớp phác thảo.
pub const PENCIL: [u8; 3] = [45, 45, 52];

/// Phác thảo ảnh `img` (đã ở độ phân giải làm việc). `sensitivity` 0..1: cao → nhiều nét hơn.
pub fn sketch(img: &RgbaImage, sensitivity: f32) -> Sketch {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let lum = luminance_over_white(img);
    // σ theo kích thước ảnh: ~1px ở 1000px, để nét không quá vụn cũng không quá nhoè
    let sigma = (w.max(h) as f32 / 1000.0).clamp(0.8, 2.0);
    let g1 = gaussian_blur(&lum, w, h, sigma);
    let g2 = gaussian_blur(&lum, w, h, sigma * 1.6);
    let s = sensitivity.clamp(0.0, 1.0);
    // ngưỡng hiệu độ sáng: độ nhạy 0.5 → 0.035
    let thr = 0.06 - 0.05 * s;

    let mut strength = vec![0.0f32; w * h];
    for i in 0..w * h {
        let d = g1[i] - g2[i];
        // độ đậm mềm: 0 ở thr/2, 1 ở 2·thr
        strength[i] = ((-d - thr * 0.5) / (thr * 1.5)).clamp(0.0, 1.0);
    }
    let mut mask = Binary::from_fn(w, h, |x, y| strength[y * w + x] > 0.5);
    let min_area = ((w.max(h) as f32 / 250.0).powi(2) as usize).max(6);
    mask.remove_small_components(min_area);

    let layer = RgbaImage::from_fn(w as u32, h as u32, |x, y| {
        let i = y as usize * w + x as usize;
        // điểm bị loại vì là nhiễu nhỏ: bỏ luôn khỏi lớp phác thảo
        let keep = mask.get(x as usize, y as usize) || strength[i] < 0.5;
        let a = if keep { strength[i] } else { 0.0 };
        Rgba([PENCIL[0], PENCIL[1], PENCIL[2], (a * 235.0).round() as u8])
    });
    Sketch { mask, layer }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_outline_on_light_fill_becomes_sketch_lines() {
        // ô màu vàng nhạt, viền đen 3px, trên nền trắng
        let img = RgbaImage::from_fn(200, 160, |x, y| {
            let inside = (40..160).contains(&x) && (40..120).contains(&y);
            let core = (43..157).contains(&x) && (43..117).contains(&y);
            if core {
                Rgba([250, 230, 150, 255])
            } else if inside {
                Rgba([20, 20, 20, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let s = sketch(&img, 0.5);
        assert!(s.mask.get(41, 80), "viền trái phải là nét");
        assert!(!s.mask.get(100, 80), "giữa ô màu phẳng không có nét");
        assert!(!s.mask.get(10, 10), "nền trắng không có nét");
        assert!(s.layer.get_pixel(41, 80)[3] > 150);
        assert_eq!(s.layer.get_pixel(100, 80)[3], 0);
    }
}
