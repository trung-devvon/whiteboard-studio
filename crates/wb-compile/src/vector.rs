//! **Nhóm E — SVG / icon vector** (kế hoạch File 02): không cần đoán đường tâm nét từ điểm ảnh.
//!
//! - Path có `stroke` → **dùng thẳng làm đường đi tay** (đã là đường tâm), bán kính = nửa độ dày nét.
//!   Đây là lý do icon dạng nét (Lucide, Tabler…) vẽ tay rất đẹp.
//! - Path chỉ có `fill` → vùng tô bằng cọ (lớp "tô màu"), vẽ sau lớp nét.
//! - SVG không có path nét nào (logo, icon tô kín) → rasterize rồi xử lý như ảnh màu.
//!
//! Lớp mực được rasterize từ chính SVG trên nền **trong suốt** → khung cuối đúng từng điểm ảnh.

use crate::analyze::{AnalyzedImage, ContentKind, ImageOptions, Layer, Placement, analyze_image};
use anyhow::{Context, Result};
use image::RgbaImage;
use resvg::tiny_skia::{self, FillRule, Mask, PathSegment, Transform};
use resvg::usvg::{self, Node};
use std::time::Instant;
use wb_model::{DrawPlan, DrawStyle, PipelineChoice, PlannedStroke, PointW, StrokeKind};
use wb_planner::PlannerParams;
use wb_planner::hatch::{HatchParams, hatch};
use wb_vision::binary::Binary;
use wb_vision::classify::ImageClass;

/// Cạnh dài khi rasterize SVG (px).
pub const SVG_RASTER_SIZE: f32 = 1600.0;

/// Một path đã quy về toạ độ ảnh raster.
struct VecPath {
    /// Các đường con (subpath) đã làm phẳng.
    polylines: Vec<Vec<(f32, f32)>>,
    /// Bán kính nét (px raster) nếu có stroke.
    stroke_radius: Option<f32>,
    /// Path có fill (để tô) — giữ nguyên dữ liệu + biến đổi để rasterize mặt nạ.
    fill: Option<(tiny_skia::Path, Transform, FillRule)>,
}

fn collect(group: &usvg::Group, s: f32, out: &mut Vec<VecPath>) {
    for node in group.children() {
        match node {
            Node::Group(g) => collect(g, s, out),
            Node::Text(t) => collect(t.flattened(), s, out),
            Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                let ts = p.abs_transform().post_scale(s, s);
                let scale = (ts.sx * ts.sy - ts.kx * ts.ky).abs().sqrt();
                let stroke_radius = p.stroke().map(|st| st.width().get() * scale / 2.0);
                let fill = p.fill().map(|f| {
                    let rule = match f.rule() {
                        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
                        usvg::FillRule::NonZero => FillRule::Winding,
                    };
                    (p.data().clone(), ts, rule)
                });
                out.push(VecPath {
                    polylines: flatten(p.data(), ts),
                    stroke_radius,
                    fill,
                });
            }
            Node::Image(_) => {}
        }
    }
}

/// Làm phẳng path (đường cong → đoạn thẳng ~2px) và áp biến đổi.
fn flatten(path: &tiny_skia::Path, ts: Transform) -> Vec<Vec<(f32, f32)>> {
    let map = |p: tiny_skia::Point| (ts.sx * p.x + ts.kx * p.y + ts.tx, ts.ky * p.x + ts.sy * p.y + ts.ty);
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let mut start = (0.0, 0.0);
    let steps = |pts: &[(f32, f32)]| {
        let len: f32 = pts
            .windows(2)
            .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
            .sum();
        ((len / 2.0).ceil() as usize).clamp(2, 200)
    };
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
                start = map(p);
                cur.push(start);
            }
            PathSegment::LineTo(p) => cur.push(map(p)),
            PathSegment::QuadTo(c, p) => {
                let a = *cur.last().unwrap_or(&start);
                let (c, p) = (map(c), map(p));
                let n = steps(&[a, c, p]);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    cur.push((
                        u * u * a.0 + 2.0 * u * t * c.0 + t * t * p.0,
                        u * u * a.1 + 2.0 * u * t * c.1 + t * t * p.1,
                    ));
                }
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let a = *cur.last().unwrap_or(&start);
                let (c1, c2, p) = (map(c1), map(c2), map(p));
                let n = steps(&[a, c1, c2, p]);
                for i in 1..=n {
                    let t = i as f32 / n as f32;
                    let u = 1.0 - t;
                    let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    cur.push((
                        b0 * a.0 + b1 * c1.0 + b2 * c2.0 + b3 * p.0,
                        b0 * a.1 + b1 * c1.1 + b2 * c2.1 + b3 * p.1,
                    ));
                }
            }
            PathSegment::Close => {
                cur.push(start);
                if cur.len() > 1 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// Rasterize SVG lên nền trong suốt → RGBA alpha thẳng.
fn rasterize(tree: &usvg::Tree, s: f32) -> Result<RgbaImage> {
    let size = tree.size();
    let (w, h) = ((size.width() * s).round() as u32, (size.height() * s).round() as u32);
    let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1)).context("kích thước SVG")?;
    resvg::render(tree, Transform::from_scale(s, s), &mut pm.as_mut());
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        out.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(pm.width(), pm.height(), out).context("SVG rỗng")
}

/// Phân tích SVG theo nhóm E. Kiểu `color_brush` hoặc pipeline khác `vector`/`auto` → xử lý như ảnh raster.
pub fn analyze_svg(data: &[u8], opt: &ImageOptions) -> Result<AnalyzedImage> {
    let t0 = Instant::now();
    let tree = usvg::Tree::from_data(data, &usvg::Options::default())?;
    let size = tree.size();
    let s = SVG_RASTER_SIZE / size.width().max(size.height());
    let ink = rasterize(&tree, s)?;
    let (w, h) = ink.dimensions();

    let mut paths = Vec::new();
    collect(tree.root(), s, &mut paths);
    let has_stroke = paths.iter().any(|p| p.stroke_radius.is_some());
    let vector_ok = matches!(opt.pipeline, PipelineChoice::Auto | PipelineChoice::Vector)
        && opt.style != DrawStyle::ColorBrush
        && has_stroke;
    if !vector_ok {
        // icon tô kín / người dùng chọn kiểu khác → như ảnh màu
        let mut o = *opt;
        if o.pipeline == PipelineChoice::Vector {
            o.pipeline = PipelineChoice::Color;
        }
        let mut a = analyze_image(&ink, &o);
        a.millis = t0.elapsed().as_millis();
        return Ok(a);
    }

    // Lớp 1: nét — đường đi tay = chính các path có stroke
    let raw: Vec<Vec<PointW>> = paths
        .iter()
        .filter_map(|p| p.stroke_radius.map(|r| (p, r)))
        .flat_map(|(p, r)| {
            p.polylines
                .iter()
                .map(move |pl| pl.iter().map(|&(x, y)| PointW::new(x, y, r)).collect())
        })
        .collect();
    let pp = PlannerParams {
        simplify_eps: 0.4,
        min_stroke_length: 4.0,
        smooth_spacing: 3.0,
        corner_deg: 55.0,
    };
    let plan = wb_planner::plan(raw, w, h, StrokeKind::Outline, &pp);
    // lớp nét chỉ chứa phần stroke (fill hiện ở lớp tô)
    let fills: Vec<&(tiny_skia::Path, Transform, FillRule)> = paths.iter().filter_map(|p| p.fill.as_ref()).collect();
    let stroke_ink = if fills.is_empty() {
        ink.clone()
    } else {
        stroke_only(data, s)?
    };
    let stroke_mask = Binary::from_fn(w as usize, h as usize, |x, y| {
        stroke_ink.get_pixel(x as u32, y as u32)[3] > 64
    });
    let coverage = coverage_of(&stroke_mask, &plan);
    let mut layers = vec![Layer {
        name: "nét",
        ink: stroke_ink,
        plan,
        coverage,
    }];

    // Lớp 2: tô các vùng fill bằng cọ (lớp mực = toàn bộ SVG → khung cuối đúng)
    if !fills.is_empty() && opt.style != DrawStyle::Sketch {
        let mut mask = Mask::new(w, h).context("mặt nạ tô")?;
        for (path, ts, rule) in &fills {
            mask.fill_path(path, *rule, true, *ts);
        }
        let cells: Vec<bool> = mask.data().iter().map(|&a| a > 64).collect();
        let fill_bin = Binary::from_fn(w as usize, h as usize, |x, y| cells[y * w as usize + x]);
        let radius = (opt.params.brush_size.clamp(0.005, 0.2) * w.max(h) as f32).max(2.0);
        let strokes = hatch(&cells, w as usize, h as usize, &HatchParams::new(radius));
        let coverage = wb_vision::coverage::coverage(&fill_bin, &strokes);
        let plan = DrawPlan::new(
            w,
            h,
            strokes
                .into_iter()
                .map(|s| PlannedStroke::new(StrokeKind::Fill, s))
                .collect(),
        );
        layers.push(Layer {
            name: "tô màu",
            ink,
            plan,
            coverage,
        });
    }
    let style = if layers.len() > 1 {
        DrawStyle::SketchThenColor
    } else {
        DrawStyle::Sketch
    };
    Ok(AnalyzedImage {
        source: (w, h),
        kind: ContentKind::Image(ImageClass::Vector),
        placement: Placement::Fit,
        style,
        background_removed: true,
        work_scale: 1.0,
        layers,
        millis: t0.elapsed().as_millis(),
    })
}

/// Rasterize chỉ phần stroke: bỏ mọi fill rồi vẽ lại.
fn stroke_only(data: &[u8], s: f32) -> Result<RgbaImage> {
    // CSS ưu tiên hơn thuộc tính trình bày (fill="…") → ép mọi fill thành none
    let opt = usvg::Options {
        style_sheet: Some("* { fill: none !important; }".to_string()),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(data, &opt)?;
    rasterize(&tree, s)
}

fn coverage_of(mask: &Binary, plan: &DrawPlan) -> f32 {
    let strokes: Vec<Vec<PointW>> = plan
        .strokes
        .iter()
        .flat_map(|st| std::iter::once(st.points.clone()).chain(st.dots.iter().map(|d| vec![d.point])))
        .collect();
    wb_vision::coverage::coverage(mask, &strokes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stroked_svg_follows_paths_exactly() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
            <path d="M10 25 L90 25" stroke="#000" stroke-width="4" fill="none"/>
            <circle cx="50" cy="25" r="15" stroke="#000" stroke-width="2" fill="none"/>
        </svg>"##;
        let a = analyze_svg(svg, &ImageOptions::default()).unwrap();
        assert_eq!(a.kind, ContentKind::Image(ImageClass::Vector));
        assert_eq!(a.layers.len(), 1);
        let plan = &a.layers[0].plan;
        assert_eq!(plan.stats.stroke_count, 2, "một đường thẳng + một vòng tròn");
        // 1600/100 = 16 px/đơn vị: nét ngang dài ~80·16
        assert!(plan.stats.outline_length > 1200.0 + 2.0 * std::f32::consts::PI * 15.0 * 16.0 * 0.9);
        assert!(a.coverage() > 0.97, "độ phủ {}", a.coverage());
    }

    #[test]
    fn filled_shapes_get_a_brush_layer() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
            <rect x="20" y="20" width="60" height="60" fill="#e33" stroke="#000" stroke-width="3"/>
        </svg>"##;
        let a = analyze_svg(svg, &ImageOptions::default()).unwrap();
        assert_eq!(a.layers.len(), 2);
        assert_eq!(a.layers[1].name, "tô màu");
        assert!(a.layers[1].coverage > 0.99);
        // lớp nét không chứa màu đỏ của fill
        let p = a.layers[0].ink.get_pixel(800, 800);
        assert_eq!(p[3], 0);
    }
}
