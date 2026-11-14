//! Tách **lớp mực** khỏi nền giấy bằng thuật toán *color-to-alpha*.
//!
//! Với màu giấy `P` (ước lượng từ ảnh) và màu điểm ảnh `c`, chọn độ phủ nhỏ nhất `a` sao cho
//! tồn tại màu mực `k` với `c = a·k + (1−a)·P`. Kết quả: phủ lớp mực lên nền giấy `P` cho lại
//! **đúng** ảnh gốc, còn vùng giấy trở nên trong suốt → reveal lố ra ngoài nét cũng không lộ "tờ giấy".

use image::{Rgba, RgbaImage};

/// Ước lượng màu giấy: phân vị 95 độ sáng của các điểm đục, lấy trung bình màu các điểm sáng nhất.
pub fn estimate_paper(img: &RgbaImage) -> [f32; 3] {
    let mut lums: Vec<(u16, [u8; 3])> = img
        .pixels()
        .filter(|p| p[3] > 200)
        .map(|p| {
            let l = (p[0] as u16 * 3 + p[1] as u16 * 6 + p[2] as u16) / 10;
            (l, [p[0], p[1], p[2]])
        })
        .collect();
    if lums.is_empty() {
        return [255.0; 3];
    }
    lums.sort_unstable_by_key(|v| v.0);
    let from = lums.len() * 90 / 100;
    let top = &lums[from..];
    let mut acc = [0.0f32; 3];
    for (_, c) in top {
        for ch in 0..3 {
            acc[ch] += c[ch] as f32;
        }
    }
    acc.map(|v| (v / top.len() as f32).max(1.0))
}

/// Tạo lớp mực (alpha thẳng, không premultiply). Điểm trong suốt của ảnh gốc giữ trong suốt.
pub fn color_to_alpha(img: &RgbaImage, paper: [f32; 3]) -> RgbaImage {
    let mut out = RgbaImage::new(img.width(), img.height());
    for (src, dst) in img.pixels().zip(out.pixels_mut()) {
        let src_a = src[3] as f32 / 255.0;
        let c = [src[0] as f32, src[1] as f32, src[2] as f32];
        // độ phủ tối thiểu cho từng kênh: kênh tối hơn giấy cần a ≥ (P−c)/P, sáng hơn cần a ≥ (c−P)/(255−P)
        let mut a = 0.0f32;
        for ch in 0..3 {
            let p = paper[ch];
            let need = if c[ch] < p {
                (p - c[ch]) / p
            } else if c[ch] > p && p < 255.0 {
                (c[ch] - p) / (255.0 - p)
            } else {
                0.0
            };
            a = a.max(need);
        }
        let a = a.clamp(0.0, 1.0);
        let px = if a <= 1.0 / 255.0 {
            Rgba([0, 0, 0, 0])
        } else {
            let k = [0, 1, 2].map(|ch| (paper[ch] + (c[ch] - paper[ch]) / a).round().clamp(0.0, 255.0) as u8);
            Rgba([k[0], k[1], k[2], (a * src_a * 255.0).round() as u8])
        };
        *dst = px;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn over(ink: Rgba<u8>, paper: [f32; 3]) -> [f32; 3] {
        let a = ink[3] as f32 / 255.0;
        [0, 1, 2].map(|ch| ink[ch] as f32 * a + paper[ch] * (1.0 - a))
    }

    #[test]
    fn compositing_ink_over_paper_restores_original() {
        let paper = [250.0, 248.0, 240.0];
        let samples = [
            [250u8, 248, 240],
            [20, 20, 25],
            [128, 120, 110],
            [200, 60, 40],
            [245, 240, 230],
        ];
        let img = RgbaImage::from_fn(samples.len() as u32, 1, |x, _| {
            let s = samples[x as usize];
            Rgba([s[0], s[1], s[2], 255])
        });
        let ink = color_to_alpha(&img, paper);
        for (x, s) in samples.iter().enumerate() {
            let back = over(*ink.get_pixel(x as u32, 0), paper);
            for ch in 0..3 {
                assert!((back[ch] - s[ch] as f32).abs() <= 2.0, "mẫu {s:?} → {back:?}");
            }
        }
        // giấy thuần → trong suốt
        assert_eq!(ink.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn paper_estimate_ignores_ink() {
        let img = RgbaImage::from_fn(100, 100, |x, _| {
            if x < 20 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([240, 240, 230, 255])
            }
        });
        let p = estimate_paper(&img);
        assert!((p[0] - 240.0).abs() < 1.0 && (p[2] - 230.0).abs() < 1.0);
    }
}
