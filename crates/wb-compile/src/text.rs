//! Element chữ → lớp vẽ "chữ" (dùng `wb-text`).

use crate::analyze::{AnalyzedImage, ContentKind, Layer, Placement};
use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::time::Instant;
use wb_model::{DrawPlan, DrawStyle, TextAlign};
use wb_text::{Align, Font, TextLayout, layout_text};

/// Font mặc định đóng gói cùng app (OFL, có đủ dấu tiếng Việt).
pub const DEFAULT_FONT: &str = "assets/fonts/PatrickHand-Regular.ttf";
/// Như [`DEFAULT_FONT`] nhưng tính từ trong thư mục `assets/`.
pub const DEFAULT_FONT_IN_ASSETS: &str = "fonts/PatrickHand-Regular.ttf";

/// Tìm font: đường dẫn người dùng (tương đối `base_dir`) hoặc font mặc định trong `assets/`
/// (tìm ngược lên từ `base_dir` rồi từ thư mục hiện tại).
pub fn resolve_font(base_dir: &Path, font: Option<&str>) -> Result<PathBuf> {
    if let Some(f) = font {
        return Ok(base_dir.join(f));
    }
    let starts = [Some(base_dir.to_path_buf()), std::env::current_dir().ok()];
    starts
        .iter()
        .flatten()
        .flat_map(|d| d.ancestors().map(|a| a.join(DEFAULT_FONT)).collect::<Vec<_>>())
        .find(|p| p.is_file())
        .with_context(|| format!("không tìm thấy font mặc định {DEFAULT_FONT}"))
}

pub struct TextOptions<'a> {
    pub text: &'a str,
    pub size: f32,
    pub color: [u8; 4],
    pub align: TextAlign,
    pub line_height: f32,
    /// Bề rộng tối đa (px khung tham chiếu).
    pub max_width: Option<f32>,
}

pub fn analyze_text(font_path: &Path, o: &TextOptions) -> Result<AnalyzedImage> {
    let t0 = Instant::now();
    let bytes = std::fs::read(font_path).with_context(|| format!("không đọc được font {}", font_path.display()))?;
    let font = Font::from_bytes(bytes)?;
    let r = layout_text(
        &font,
        o.text,
        &TextLayout {
            size: o.size,
            line_height: o.line_height,
            max_width: o.max_width,
            align: match o.align {
                TextAlign::Left => Align::Left,
                TextAlign::Center => Align::Center,
                TextAlign::Right => Align::Right,
            },
        },
    )?;
    let [cr, cg, cb, ca] = o.color;
    let ink = RgbaImage::from_fn(r.width, r.height, |x, y| {
        let a = r.alpha[(y * r.width + x) as usize] as u32 * ca as u32 / 255;
        Rgba([cr, cg, cb, a as u8])
    });
    let plan = DrawPlan::new(r.width, r.height, r.strokes);
    Ok(AnalyzedImage {
        source: (r.width, r.height),
        kind: ContentKind::Text,
        placement: Placement::Absolute {
            raster_scale: r.raster_scale,
            margin: r.margin,
        },
        style: DrawStyle::Sketch,
        background_removed: true,
        work_scale: 1.0,
        layers: vec![Layer {
            name: "chữ",
            ink,
            plan,
            coverage: r.coverage,
        }],
        millis: t0.elapsed().as_millis(),
    })
}
