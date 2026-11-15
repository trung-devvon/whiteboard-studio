//! Tách **nền đồng màu** (phông xanh, nền trắng của clipart, giấy màu…) khỏi ảnh màu.
//!
//! Khác `ink::color_to_alpha` (dùng cho line-art, coi *mọi* điểm giống màu giấy là giấy), ở đây chỉ
//! những vùng giống màu nền **và nối liền với mép ảnh** mới bị xoá ("đũa thần" từ mép). Nhờ vậy
//! phần bên trong vật thể có màu gần màu nền (lá xanh trên phông xanh, ô cửa sổ trắng) được giữ.

use crate::filters::color_dist;
use image::{Rgba, RgbaImage};
use std::collections::VecDeque;

/// Kết quả xem xét mép ảnh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Border {
    /// Mép ảnh phần lớn đã trong suốt (sticker, PNG đã xoá phông).
    Transparent,
    /// Mép ảnh phần lớn cùng một màu → đó là màu nền.
    Uniform([u8; 3]),
    /// Mép ảnh nhiều màu (ảnh chụp, tranh full khung) → không có nền để xoá.
    Busy,
}

/// Ngưỡng khác màu (RGB Euclid) để coi là "cùng màu nền".
pub const BG_TOLERANCE: f32 = 42.0;

/// Xem mép ảnh để đoán nền.
pub fn analyze_border(img: &RgbaImage) -> Border {
    let (w, h) = img.dimensions();
    if w < 3 || h < 3 {
        return Border::Busy;
    }
    let mut px = Vec::new();
    for x in 0..w {
        px.push(*img.get_pixel(x, 0));
        px.push(*img.get_pixel(x, h - 1));
    }
    for y in 1..h - 1 {
        px.push(*img.get_pixel(0, y));
        px.push(*img.get_pixel(w - 1, y));
    }
    let transparent = px.iter().filter(|p| p[3] < 32).count();
    if transparent * 10 >= px.len() * 6 {
        return Border::Transparent;
    }
    let opaque: Vec<[u8; 3]> = px.iter().filter(|p| p[3] >= 32).map(|p| [p[0], p[1], p[2]]).collect();
    // màu nền = trung vị từng kênh
    let median = |ch: usize| {
        let mut v: Vec<u8> = opaque.iter().map(|c| c[ch]).collect();
        v.sort_unstable();
        v[v.len() / 2]
    };
    let m = [median(0), median(1), median(2)];
    let close = opaque.iter().filter(|c| color_dist(**c, m) < BG_TOLERANCE).count();
    // phông thật gần như đồng màu toàn bộ mép; 80% tránh nhầm ảnh chụp có góc tối
    if close * 10 >= opaque.len() * 8 {
        Border::Uniform(m)
    } else {
        Border::Busy
    }
}

/// Xoá vùng màu `bg` nối liền với mép ảnh. Mép vật thể được làm mềm bằng color-to-alpha theo màu nền
/// (giữ viền khử răng cưa, không để lại quầng màu nền).
pub fn remove_background(img: &RgbaImage, bg: [u8; 3]) -> RgbaImage {
    remove_background_with(img, bg, BG_TOLERANCE)
}

/// Như [`remove_background`] nhưng tự chọn ngưỡng khác màu `tolerance` (RGB Euclid). Ngưỡng nhỏ giữ
/// được chi tiết sáng gần màu nền nằm sát mép vật thể (vd. khâu kim loại trắng của bút chì).
pub fn remove_background_with(img: &RgbaImage, bg: [u8; 3], tolerance: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (wu, hu) = (w as usize, h as usize);
    let is_bg_color = |p: &Rgba<u8>| p[3] < 32 || color_dist([p[0], p[1], p[2]], bg) < tolerance;
    let mut is_bg = vec![false; wu * hu];
    let mut queue = VecDeque::new();
    let seed = |x: u32, y: u32, is_bg: &mut Vec<bool>, q: &mut VecDeque<(u32, u32)>| {
        let i = y as usize * wu + x as usize;
        if !is_bg[i] && is_bg_color(img.get_pixel(x, y)) {
            is_bg[i] = true;
            q.push_back((x, y));
        }
    };
    for x in 0..w {
        seed(x, 0, &mut is_bg, &mut queue);
        seed(x, h - 1, &mut is_bg, &mut queue);
    }
    for y in 0..h {
        seed(0, y, &mut is_bg, &mut queue);
        seed(w - 1, y, &mut is_bg, &mut queue);
    }
    while let Some((x, y)) = queue.pop_front() {
        let (xi, yi) = (x as i64, y as i64);
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (xi + dx, yi + dy);
            if nx >= 0 && ny >= 0 && nx < w as i64 && ny < h as i64 {
                seed(nx as u32, ny as u32, &mut is_bg, &mut queue);
            }
        }
    }

    let bgf = bg.map(|v| v as f32);
    let mut out = img.clone();
    for y in 0..hu {
        for x in 0..wu {
            let i = y * wu + x;
            let p = out.get_pixel_mut(x as u32, y as u32);
            if is_bg[i] {
                *p = Rgba([0, 0, 0, 0]);
                continue;
            }
            // pixel sát nền: tách phần màu nền lẫn vào (khử răng cưa)
            let near_bg = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                nx >= 0 && ny >= 0 && nx < wu as i64 && ny < hu as i64 && is_bg[ny as usize * wu + nx as usize]
            });
            if near_bg {
                *p = unmix(*p, bgf);
            }
        }
    }
    out
}

/// Color-to-alpha cho một pixel theo màu nền `bg` (xem `ink::color_to_alpha`).
fn unmix(src: Rgba<u8>, bg: [f32; 3]) -> Rgba<u8> {
    let c = [src[0] as f32, src[1] as f32, src[2] as f32];
    let mut a = 0.0f32;
    for ch in 0..3 {
        let p = bg[ch].clamp(1.0, 254.0);
        let need = if c[ch] < p {
            (p - c[ch]) / p
        } else {
            (c[ch] - p) / (255.0 - p)
        };
        a = a.max(need);
    }
    let a = a.clamp(0.0, 1.0);
    if a <= 1.0 / 255.0 {
        return Rgba([0, 0, 0, 0]);
    }
    let k = [0, 1, 2].map(|ch| (bg[ch] + (c[ch] - bg[ch]) / a).round().clamp(0.0, 255.0) as u8);
    Rgba([k[0], k[1], k[2], (a * src[3] as f32).round() as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phông xanh lá, một ô vuông đỏ ở giữa có "cửa sổ" xanh lá bên trong.
    fn sample() -> RgbaImage {
        RgbaImage::from_fn(60, 60, |x, y| {
            let inside = (15..45).contains(&x) && (15..45).contains(&y);
            let window = (25..35).contains(&x) && (25..35).contains(&y);
            if inside && !window {
                Rgba([200, 30, 30, 255])
            } else {
                Rgba([40, 120, 60, 255])
            }
        })
    }

    #[test]
    fn uniform_border_is_detected() {
        assert_eq!(analyze_border(&sample()), Border::Uniform([40, 120, 60]));
        let busy = RgbaImage::from_fn(40, 40, |x, y| Rgba([(x * 6) as u8, (y * 6) as u8, 128, 255]));
        assert_eq!(analyze_border(&busy), Border::Busy);
        let clear = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 0, 0]));
        assert_eq!(analyze_border(&clear), Border::Transparent);
    }

    #[test]
    fn only_background_connected_to_edges_is_removed() {
        let out = remove_background(&sample(), [40, 120, 60]);
        assert_eq!(out.get_pixel(2, 2)[3], 0, "nền ngoài → trong suốt");
        assert_eq!(out.get_pixel(30, 30)[3], 255, "cửa sổ bên trong vật thể → giữ");
        assert_eq!(out.get_pixel(20, 20)[3], 255, "vật thể giữ nguyên");
    }

    #[test]
    fn lower_tolerance_keeps_light_detail_touching_the_background() {
        // nền trắng, dải xám rất nhạt (235) sát mép vật thể đỏ — như khâu kim loại của bút chì
        let img = RgbaImage::from_fn(40, 20, |x, _| match x {
            10..20 => Rgba([235, 235, 235, 255]),
            20..30 => Rgba([200, 30, 30, 255]),
            _ => Rgba([255, 255, 255, 255]),
        });
        assert_eq!(remove_background(&img, [255, 255, 255]).get_pixel(15, 10)[3], 0);
        let kept = remove_background_with(&img, [255, 255, 255], 20.0);
        assert_eq!(kept.get_pixel(15, 10)[3], 255, "ngưỡng nhỏ → giữ dải sáng");
        assert_eq!(kept.get_pixel(2, 10)[3], 0, "nền vẫn bị xoá");
    }
}
