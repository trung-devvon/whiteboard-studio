//! **Tự nhận loại ảnh** bằng vài đặc trưng đơn giản (không cần AI) — kế hoạch File 02 §1.
//!
//! - `LineArt`: phần lớn là giấy, phần còn lại là mực tối ít màu (nét bút, sơ đồ, icon đen trắng).
//! - `Color`: minh hoạ / clipart / sticker — có nền đồng màu hoặc trong suốt để tách, hoặc ít màu.
//! - `Photo`: ảnh chụp, tranh tô kín khung (nhiều màu, mép ảnh không đồng màu).
//!
//! Chỉ là gợi ý mặc định: người dùng luôn đổi được trong dự án (`pipeline`).

use crate::background::{Border, analyze_border};
use crate::filters::{color_dist, to_working};
use crate::ink::estimate_paper;
use image::RgbaImage;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageClass {
    LineArt,
    Color,
    Photo,
    /// SVG có path nét — đi thẳng theo path (nhóm E, không qua phân tích điểm ảnh).
    Vector,
}

/// Đặc trưng đo được (in ra cho người dùng / debug).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Features {
    /// Tỉ lệ điểm trong suốt.
    pub transparent: f32,
    /// Tỉ lệ điểm gần màu giấy (sáng nhất).
    pub paper: f32,
    /// Độ đậm màu trung bình (max−min kênh, 0..255) của các điểm **không** phải giấy.
    pub ink_chroma: f32,
    /// Số ô màu (4 bit/kênh) chiếm ≥ 0,2% ảnh.
    pub colors: u32,
    /// Mép ảnh đồng màu / trong suốt.
    pub border_uniform: bool,
    pub border_transparent: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Classification {
    pub class: ImageClass,
    pub features: Features,
}

/// Phân loại ảnh (tự thu nhỏ về ≤ 512px để nhanh).
pub fn classify(img: &RgbaImage) -> Classification {
    let (small, _) = to_working(img, 512);
    let total = (small.width() * small.height()).max(1) as f32;
    let paper = estimate_paper(&small);
    let paper_u8 = paper.map(|v| v.round().clamp(0.0, 255.0) as u8);

    let mut transparent = 0usize;
    let mut near_paper = 0usize;
    let mut chroma_sum = 0.0f32;
    let mut ink_n = 0usize;
    let mut bins = vec![0u32; 4096];
    for p in small.pixels() {
        if p[3] < 32 {
            transparent += 1;
            continue;
        }
        let c = [p[0], p[1], p[2]];
        bins[((c[0] as usize >> 4) << 8) | ((c[1] as usize >> 4) << 4) | (c[2] as usize >> 4)] += 1;
        if color_dist(c, paper_u8) < 40.0 {
            near_paper += 1;
        } else {
            let mx = c.iter().max().copied().unwrap_or(0);
            let mn = c.iter().min().copied().unwrap_or(0);
            chroma_sum += (mx - mn) as f32;
            ink_n += 1;
        }
    }
    let min_bin = (total * 0.002).max(1.0) as u32;
    let colors = bins.iter().filter(|&&n| n >= min_bin).count() as u32;
    let border = analyze_border(&small);
    let f = Features {
        transparent: transparent as f32 / total,
        paper: near_paper as f32 / total,
        ink_chroma: if ink_n > 0 { chroma_sum / ink_n as f32 } else { 0.0 },
        colors,
        border_uniform: matches!(border, Border::Uniform(_)),
        border_transparent: border == Border::Transparent,
    };

    let mostly_paper = f.paper + f.transparent > 0.55;
    let class = if mostly_paper && f.ink_chroma < 45.0 {
        ImageClass::LineArt
    } else if f.border_uniform || f.border_transparent || mostly_paper || f.colors < 40 {
        ImageClass::Color
    } else {
        ImageClass::Photo
    };
    Classification { class, features: f }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn simple_cases() {
        // nét đen trên giấy trắng
        let line = RgbaImage::from_fn(200, 200, |x, y| {
            if x % 50 < 3 || y % 70 < 3 {
                Rgba([20, 20, 20, 255])
            } else {
                Rgba([250, 250, 248, 255])
            }
        });
        assert_eq!(classify(&line).class, ImageClass::LineArt);

        // hình màu trên phông xanh
        let flat = RgbaImage::from_fn(200, 200, |x, y| {
            if (50..150).contains(&x) && (40..160).contains(&y) {
                Rgba([220, 80, 40, 255])
            } else {
                Rgba([40, 120, 60, 255])
            }
        });
        assert_eq!(classify(&flat).class, ImageClass::Color);

        // "ảnh chụp": chuyển màu liên tục khắp khung, mép nhiều màu
        let photo = RgbaImage::from_fn(200, 200, |x, y| {
            Rgba([(x * 255 / 200) as u8, (y * 255 / 200) as u8, 128, 255])
        });
        assert_eq!(classify(&photo).class, ImageClass::Photo);
    }
}
