//! Pipeline **Nhóm A — line-art** (nét vẽ sẵn, icon đen trắng, sơ đồ, truyện tranh).
//!
//! ảnh → lớp mực (color-to-alpha) → thu nhỏ về độ phân giải làm việc → ngưỡng → bỏ nhiễu
//! → distance transform (độ dày) → Guo–Hall (xương) → đồ thị xương → nét (px ảnh nguồn).

use crate::binary::Binary;
use crate::coverage::coverage;
use crate::ink::{color_to_alpha, estimate_paper};
use crate::skeleton::{distance_transform, guo_hall, trace_strokes};
use image::{GrayImage, Luma, RgbaImage, imageops::FilterType};
use wb_model::{PointW, VisionParams};

/// Kết quả phân tích một ảnh line-art.
pub struct LineArtResult {
    /// Lớp mực (alpha thẳng), cùng kích thước ảnh nguồn — đây là thứ được reveal.
    pub ink: RgbaImage,
    /// Màu giấy ước lượng (RGB).
    pub paper: [f32; 3],
    /// Đường tâm nét (px ảnh nguồn), **chưa** sắp thứ tự — việc của `wb-planner`.
    pub strokes: Vec<Vec<PointW>>,
    /// Tỉ lệ độ phân giải làm việc / ảnh nguồn (≤ 1).
    pub work_scale: f32,
    /// Mặt nạ mực ở độ phân giải làm việc (để đo độ phủ).
    pub ink_mask: Binary,
    /// Độ phủ của nét thô trên mặt nạ mực (0..1), trước khi planner kéo dài đầu nét.
    pub raw_coverage: f32,
}

/// Phân tích ảnh line-art.
pub fn analyze_line_art(img: &RgbaImage, params: &VisionParams) -> LineArtResult {
    let (sw, sh) = img.dimensions();
    let paper = estimate_paper(img);
    let ink = color_to_alpha(img, paper);

    // Độ phân giải làm việc
    let long = sw.max(sh) as f32;
    let scale = (params.working_size as f32 / long).min(1.0);
    let (ww, wh) = (
        ((sw as f32 * scale).round() as u32).max(1),
        ((sh as f32 * scale).round() as u32).max(1),
    );
    let alpha = GrayImage::from_fn(sw, sh, |x, y| Luma([ink.get_pixel(x, y)[3]]));
    let alpha = if scale < 1.0 {
        image::imageops::resize(&alpha, ww, wh, FilterType::Triangle)
    } else {
        alpha
    };

    // Ngưỡng: độ nhạy 0 → chỉ nét đậm (alpha > 0.6); 1 → bắt cả nét mờ (alpha > 0.15)
    let s = params.sensitivity.clamp(0.0, 1.0);
    let t = ((0.6 - 0.45 * s) * 255.0) as u8;
    let mut mask = Binary::from_fn(ww as usize, wh as usize, |x, y| {
        alpha.get_pixel(x as u32, y as u32)[0] > t
    });
    mask.remove_small_components(3);

    let dist = distance_transform(&mask);
    let skel = guo_hall(&mask);
    let raw = trace_strokes(&skel, &dist, 1.5, 60.0);

    let work_strokes: Vec<Vec<PointW>> = raw
        .iter()
        .map(|s| s.iter().map(|p| PointW::new(p[0], p[1], p[2])).collect())
        .collect();
    let raw_coverage = coverage(&mask, &work_strokes);

    let inv = 1.0 / scale;
    let strokes = work_strokes
        .into_iter()
        .map(|s| {
            s.into_iter()
                .map(|p| PointW::new(p.x * inv, p.y * inv, p.w * inv))
                .collect()
        })
        .collect();

    LineArtResult {
        ink,
        paper,
        strokes,
        work_scale: scale,
        ink_mask: mask,
        raw_coverage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn simple_drawing_is_traced_and_covered() {
        // giấy ngà + hình chữ nhật viền đen dày 6px + một đường chéo
        let img = RgbaImage::from_fn(400, 300, |x, y| {
            let (x, y) = (x as f32, y as f32);
            let border = ((50.0..=350.0).contains(&x) && (50.0..=250.0).contains(&y))
                && !(x > 56.0 && x < 344.0 && y > 56.0 && y < 244.0);
            let diag = ((x - 50.0) - (y - 50.0) * 1.5).abs() < 3.0 && (50.0..250.0).contains(&y);
            if border || diag {
                Rgba([25, 25, 30, 255])
            } else {
                Rgba([245, 242, 235, 255])
            }
        });
        let r = analyze_line_art(&img, &VisionParams::default());
        assert!(r.strokes.len() <= 8, "quá nhiều nét: {}", r.strokes.len());
        assert!(r.raw_coverage > 0.97, "độ phủ {}", r.raw_coverage);
        // giấy trở thành trong suốt
        assert_eq!(r.ink.get_pixel(300, 120)[3], 0);
        // mực: phủ lên giấy cho lại đúng màu gốc (25)
        let p = r.ink.get_pixel(52, 150);
        let a = p[3] as f32 / 255.0;
        let back = p[0] as f32 * a + r.paper[0] * (1.0 - a);
        assert!(a > 0.8 && (back - 25.0).abs() < 2.0, "alpha {a}, ghép lại {back}");
    }
}
