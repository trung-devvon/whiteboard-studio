//! Bộ lọc ảnh cơ bản dùng chung: độ sáng, làm mờ Gauss, thu nhỏ về độ phân giải làm việc.

use image::{RgbaImage, imageops::FilterType};

/// Thu nhỏ để cạnh dài ≤ `working_size`. Trả (ảnh, tỉ lệ làm việc / nguồn ≤ 1).
pub fn to_working(img: &RgbaImage, working_size: u32) -> (RgbaImage, f32) {
    let (w, h) = img.dimensions();
    let scale = (working_size as f32 / w.max(h) as f32).min(1.0);
    if scale >= 1.0 {
        return (img.clone(), 1.0);
    }
    let (ww, wh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    (image::imageops::resize(img, ww, wh, FilterType::Triangle), scale)
}

/// Độ sáng (0..1) của ảnh khi đặt lên nền trắng — pixel trong suốt thành trắng.
pub fn luminance_over_white(img: &RgbaImage) -> Vec<f32> {
    img.pixels()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            let l = (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0;
            l * a + (1.0 - a)
        })
        .collect()
}

/// Làm mờ Gauss tách hai chiều (biên: kẹp toạ độ).
pub fn gaussian_blur(src: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    if sigma <= 0.0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let r = (sigma * 3.0).ceil() as isize;
    let mut kernel: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);

    let mut tmp = vec![0.0; w * h];
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        for x in 0..w {
            let mut acc = 0.0;
            for (ki, k) in kernel.iter().enumerate() {
                let xx = (x as isize + ki as isize - r).clamp(0, w as isize - 1) as usize;
                acc += row[xx] * k;
            }
            tmp[y * w + x] = acc;
        }
    }
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let mut acc = 0.0;
            for (ki, k) in kernel.iter().enumerate() {
                let yy = (y as isize + ki as isize - r).clamp(0, h as isize - 1) as usize;
                acc += tmp[yy * w + x] * k;
            }
            out[y * w + x] = acc;
        }
    }
    out
}

/// Khoảng cách màu RGB (Euclid).
pub fn color_dist(a: [u8; 3], b: [u8; 3]) -> f32 {
    let d = |i: usize| a[i] as f32 - b[i] as f32;
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_preserves_constant_and_mass() {
        let w = 20;
        let h = 10;
        let flat = vec![0.5; w * h];
        assert!(gaussian_blur(&flat, w, h, 2.0).iter().all(|v| (v - 0.5).abs() < 1e-5));
        let mut spike = vec![0.0; w * h];
        spike[5 * w + 10] = 1.0;
        let b = gaussian_blur(&spike, w, h, 1.0);
        let total: f32 = b.iter().sum();
        assert!((total - 1.0).abs() < 1e-3);
        assert!(b[5 * w + 10] < 0.5 && b[5 * w + 11] > 0.0);
    }
}
