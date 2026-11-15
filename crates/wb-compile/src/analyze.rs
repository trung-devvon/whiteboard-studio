//! Phân tích một ảnh → các **lớp vẽ** (layer) theo loại ảnh và kiểu vẽ.
//!
//! | Loại ảnh | Kiểu vẽ mặc định | Các lớp |
//! |---|---|---|
//! | line-art | `sketch` | nét gốc (bàn tay đi theo đường tâm nét) |
//! | ảnh màu (clipart, minh hoạ) | `sketch_then_color` | nét phác thảo chì → cọ tô hiện màu |
//! | ảnh chụp / tranh kín khung | `color_brush` | cọ tô hiện màu ("tô nét") |
//!
//! Mỗi lớp có lớp mực **riêng** (có thể ở độ phân giải làm việc) và kế hoạch nét riêng; engine vẽ
//! nối tiếp: lớp trước xong được "nướng" vào nền, lớp sau phủ lên. Khung cuối = lớp cuối.

use image::RgbaImage;
use std::time::Instant;
use wb_model::{BackgroundMode, DrawPlan, DrawStyle, PipelineChoice, PlannedStroke, PointW, StrokeKind, VisionParams};
use wb_planner::PlannerParams;
use wb_planner::hatch::{HatchParams, hatch};
use wb_vision::background::{Border, analyze_border, remove_background};
use wb_vision::binary::Binary;
use wb_vision::classify::{ImageClass, classify};
use wb_vision::filters::to_working;

/// Tuỳ chọn xử lý một ảnh (khớp các trường của `ElementKind::Image`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ImageOptions {
    pub pipeline: PipelineChoice,
    pub style: DrawStyle,
    pub background: BackgroundMode,
    pub params: VisionParams,
}

/// Một lớp vẽ.
pub struct Layer {
    /// Tên hiển thị: "nét", "phác thảo", "tô màu".
    pub name: &'static str,
    /// Lớp mực (alpha thẳng). Có thể nhỏ hơn ảnh nguồn — engine tự co giãn cho vừa khung.
    pub ink: RgbaImage,
    /// Kế hoạch nét theo px của `ink`.
    pub plan: DrawPlan,
    /// Độ phủ của nét trên mặt nạ cần vẽ (0..1).
    pub coverage: f32,
}

/// Loại nội dung của element (để báo cáo).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Image(ImageClass),
    Text,
}

/// Cách đặt các lớp lên khung.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Ảnh: co vừa khung `transform`, giữ tỉ lệ, căn giữa.
    Fit,
    /// Chữ: đúng cỡ; `raster_scale` = px lớp / px khung tham chiếu, `margin` = lề (px lớp) trước góc trên-trái.
    Absolute { raster_scale: f32, margin: f32 },
}

pub struct AnalyzedImage {
    /// Kích thước nội dung (px lớp đầu tiên với ảnh nguồn; với chữ: px raster).
    pub source: (u32, u32),
    pub kind: ContentKind,
    pub placement: Placement,
    pub style: DrawStyle,
    /// Đã xoá nền đồng màu.
    pub background_removed: bool,
    /// Tỉ lệ độ phân giải làm việc / ảnh nguồn.
    pub work_scale: f32,
    pub layers: Vec<Layer>,
    pub millis: u128,
}

impl AnalyzedImage {
    /// Độ phủ thấp nhất trong các lớp.
    pub fn coverage(&self) -> f32 {
        self.layers.iter().map(|l| l.coverage).fold(1.0, f32::min)
    }
    pub fn stroke_count(&self) -> u32 {
        self.layers.iter().map(|l| l.plan.stats.stroke_count).sum()
    }
    pub fn dot_count(&self) -> u32 {
        self.layers.iter().map(|l| l.plan.stats.dot_count).sum()
    }
}

/// Kiểu vẽ thực tế khi người dùng để `Auto`.
pub fn resolve_style(class: ImageClass, style: DrawStyle) -> DrawStyle {
    match (style, class) {
        (DrawStyle::Auto, ImageClass::LineArt) => DrawStyle::Sketch,
        (DrawStyle::Auto, ImageClass::Color) => DrawStyle::SketchThenColor,
        (DrawStyle::Auto, ImageClass::Photo) => DrawStyle::ColorBrush,
        (DrawStyle::Auto, ImageClass::Vector) => DrawStyle::SketchThenColor,
        // line-art: "phác thảo rồi tô" chính là vẽ nét gốc
        (DrawStyle::SketchThenColor, ImageClass::LineArt) => DrawStyle::Sketch,
        (s, _) => s,
    }
}

/// Phân tích ảnh theo tuỳ chọn.
pub fn analyze_image(img: &RgbaImage, opt: &ImageOptions) -> AnalyzedImage {
    let t0 = Instant::now();
    let class = match opt.pipeline {
        PipelineChoice::Auto => classify(img).class,
        PipelineChoice::LineArt => ImageClass::LineArt,
        // ảnh raster không có path → "vector" coi như ảnh màu
        PipelineChoice::Color | PipelineChoice::Vector => ImageClass::Color,
        PipelineChoice::Photo => ImageClass::Photo,
    };
    let style = resolve_style(class, opt.style);
    let params = &opt.params;
    let (sw, sh) = img.dimensions();

    let mut layers = Vec::new();
    let mut background_removed = false;
    let work_scale;
    if class == ImageClass::LineArt {
        let r = wb_vision::analyze_line_art(img, params);
        work_scale = r.work_scale;
        background_removed = true;
        if style == DrawStyle::ColorBrush {
            let ink_alpha = alpha_mask(&r.ink, work_scale);
            layers.push(brush_layer(r.ink, &ink_alpha, work_scale, params));
        } else {
            let pp = PlannerParams::for_scale(1.0 / r.work_scale, params.min_stroke_length);
            let plan = wb_planner::plan(r.strokes, sw, sh, StrokeKind::Outline, &pp);
            let coverage = coverage_on(&r.ink_mask, &plan, r.work_scale);
            layers.push(Layer {
                name: "nét",
                ink: r.ink,
                plan,
                coverage,
            });
        }
    } else {
        // 1. nền
        let border = analyze_border(img);
        let color = match (opt.background, border) {
            (BackgroundMode::Keep, _) => img.clone(),
            (_, Border::Uniform(bg)) => {
                background_removed = true;
                remove_background(img, bg)
            }
            (_, Border::Transparent) => {
                background_removed = true;
                img.clone()
            }
            (_, Border::Busy) => img.clone(),
        };
        let (work, s) = to_working(&color, params.working_size);
        work_scale = s;

        // 2. nét phác thảo (ở độ phân giải làm việc)
        if matches!(style, DrawStyle::Sketch | DrawStyle::SketchThenColor) {
            let sk = wb_vision::sketch::sketch(&work, params.sensitivity);
            let (raw, _) = wb_vision::strokes_from_mask(&sk.mask);
            let pp = PlannerParams::for_scale(1.0, params.min_stroke_length);
            let plan = wb_planner::plan(raw, work.width(), work.height(), StrokeKind::Outline, &pp);
            let coverage = coverage_on(&sk.mask, &plan, 1.0);
            layers.push(Layer {
                name: "phác thảo",
                ink: sk.layer,
                plan,
                coverage,
            });
        }
        // 3. tô màu
        if matches!(style, DrawStyle::SketchThenColor | DrawStyle::ColorBrush) {
            let mask = Binary::from_fn(work.width() as usize, work.height() as usize, |x, y| {
                work.get_pixel(x as u32, y as u32)[3] > 8
            });
            layers.push(brush_layer(color, &mask, work_scale, params));
        }
    }
    AnalyzedImage {
        source: (sw, sh),
        kind: ContentKind::Image(class),
        placement: Placement::Fit,
        style,
        background_removed,
        work_scale,
        layers,
        millis: t0.elapsed().as_millis(),
    }
}

/// Mặt nạ "có mực" của lớp mực ở độ phân giải làm việc.
fn alpha_mask(ink: &RgbaImage, work_scale: f32) -> Binary {
    let w = ((ink.width() as f32 * work_scale).round() as usize).max(1);
    let h = ((ink.height() as f32 * work_scale).round() as usize).max(1);
    let inv = 1.0 / work_scale;
    Binary::from_fn(w, h, |x, y| {
        // lấy mẫu thô: đủ cho việc chọn vùng tô (cọ to hơn nhiều so với 1px)
        let sx = ((x as f32 + 0.5) * inv) as u32;
        let sy = ((y as f32 + 0.5) * inv) as u32;
        ink.get_pixel(sx.min(ink.width() - 1), sy.min(ink.height() - 1))[3] > 8
    })
}

/// Lớp tô màu: cọ zíc-zắc phủ `mask` (độ phân giải làm việc), reveal `ink` (độ phân giải nguồn).
fn brush_layer(ink: RgbaImage, mask: &Binary, work_scale: f32, params: &VisionParams) -> Layer {
    let (w, h) = (mask.w, mask.h);
    let cells: Vec<bool> = (0..w * h).map(|i| mask.get(i % w, i / w)).collect();
    let radius = (params.brush_size.clamp(0.005, 0.2) * w.max(h) as f32).max(2.0);
    let work_strokes = hatch(&cells, w, h, &HatchParams::new(radius));
    let coverage = wb_vision::coverage::coverage(mask, &work_strokes);
    let inv = 1.0 / work_scale;
    let strokes: Vec<PlannedStroke> = work_strokes
        .into_iter()
        .map(|s| {
            PlannedStroke::new(
                StrokeKind::Fill,
                s.into_iter()
                    .map(|p| PointW::new(p.x * inv, p.y * inv, p.w * inv))
                    .collect(),
            )
        })
        .collect();
    let plan = DrawPlan::new(ink.width(), ink.height(), strokes);
    Layer {
        name: "tô màu",
        ink,
        plan,
        coverage,
    }
}

/// Độ phủ của kế hoạch nét (kể cả chấm vụn) trên mặt nạ; `s` = px mặt nạ / px kế hoạch.
fn coverage_on(mask: &Binary, plan: &DrawPlan, s: f32) -> f32 {
    let work: Vec<Vec<PointW>> = plan
        .strokes
        .iter()
        .flat_map(|st| {
            let main: Vec<PointW> = st
                .points
                .iter()
                .map(|p| PointW::new(p.x * s, p.y * s, p.w * s))
                .collect();
            std::iter::once(main).chain(
                st.dots
                    .iter()
                    .map(move |d| vec![PointW::new(d.point.x * s, d.point.y * s, d.point.w * s)]),
            )
        })
        .collect();
    wb_vision::coverage::coverage(mask, &work)
}
