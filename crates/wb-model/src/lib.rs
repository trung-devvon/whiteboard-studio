//! Kiểu dữ liệu dùng chung của Whiteboard Studio.
//!
//! Đây là **nguồn sự thật** cho mọi kiểu dữ liệu: kiểu TypeScript ở `src/bindings/`
//! được sinh tự động từ crate này (`cargo run -p wb-model --features specta --bin gen-bindings`).
//! Kế hoạch chi tiết: `not_push_git/05-mo-hinh-du-lieu-va-dinh-dang-du-an.md`.
//!
//! Trạng thái Giai đoạn 0: mới có các kiểu tối thiểu phục vụ spike (nét vẽ, DrawPlan).
//! Project/Scene/Element đầy đủ sẽ được thêm ở Giai đoạn 1.

use serde::{Deserialize, Serialize};

/// Phiên bản cấu trúc file dự án. Tăng khi đổi cấu trúc không tương thích (kèm migration).
pub const SCHEMA_VERSION: u32 = 1;

/// Khung tham chiếu: mọi toạ độ trong tài liệu dự án tính theo px của khung 1920×1080.
pub const REFERENCE_WIDTH: f32 = 1920.0;
pub const REFERENCE_HEIGHT: f32 = 1080.0;

/// Một điểm trên đường tâm nét, kèm bán kính cọ `w` tại điểm đó (px).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PointW {
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// Loại nét — quyết định bút/cọ, tốc độ và thứ tự vẽ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum StrokeKind {
    Outline,
    Fill,
    Text,
    Diacritic,
}

/// Một nét đã được sắp thứ tự, sẵn sàng cho bộ giải thời gian và engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlannedStroke {
    pub kind: StrokeKind,
    pub points: Vec<PointW>,
    /// Độ dài cung (px), tính sẵn.
    pub length: f32,
}

impl PlannedStroke {
    /// Tạo nét và tính sẵn độ dài cung.
    pub fn new(kind: StrokeKind, points: Vec<PointW>) -> Self {
        let length = polyline_length(&points);
        Self { kind, points, length }
    }
}

/// Thống kê dùng cho `wb-timing` (ngân sách thời gian).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlanStats {
    pub stroke_count: u32,
    pub outline_length: f32,
    pub fill_length: f32,
    /// Tổng quãng đường nhấc bút giữa các nét (px).
    pub travel_length: f32,
}

/// Kết quả "biên dịch" một Element thành danh sách nét đã sắp xếp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct DrawPlan {
    /// Kích thước ảnh nguồn dùng để reveal (px).
    pub source_width: u32,
    pub source_height: u32,
    pub strokes: Vec<PlannedStroke>,
    pub stats: PlanStats,
}

impl DrawPlan {
    /// Tạo DrawPlan và tính thống kê từ danh sách nét (đã sắp thứ tự).
    pub fn new(source_width: u32, source_height: u32, strokes: Vec<PlannedStroke>) -> Self {
        let mut stats = PlanStats {
            stroke_count: strokes.len() as u32,
            ..PlanStats::default()
        };
        let mut prev_end: Option<PointW> = None;
        for s in &strokes {
            match s.kind {
                StrokeKind::Fill => stats.fill_length += s.length,
                _ => stats.outline_length += s.length,
            }
            if let (Some(a), Some(b)) = (prev_end, s.points.first()) {
                stats.travel_length += dist(a, *b);
            }
            prev_end = s.points.last().copied();
        }
        Self {
            source_width,
            source_height,
            strokes,
            stats,
        }
    }
}

fn dist(a: PointW, b: PointW) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// Độ dài đường gấp khúc.
pub fn polyline_length(points: &[PointW]) -> f32 {
    points.windows(2).map(|w| dist(w[0], w[1])).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> PointW {
        PointW { x, y, w: 1.0 }
    }

    #[test]
    fn stats_count_lengths_and_travel() {
        let a = PlannedStroke::new(StrokeKind::Outline, vec![p(0.0, 0.0), p(3.0, 4.0)]);
        let b = PlannedStroke::new(StrokeKind::Fill, vec![p(3.0, 10.0), p(3.0, 20.0)]);
        let plan = DrawPlan::new(100, 100, vec![a, b]);
        assert_eq!(plan.stats.stroke_count, 2);
        assert_eq!(plan.stats.outline_length, 5.0);
        assert_eq!(plan.stats.fill_length, 10.0);
        assert_eq!(plan.stats.travel_length, 6.0);
    }

    #[test]
    fn stroke_kind_serializes_snake_case() {
        let json = serde_json::to_string(&StrokeKind::Diacritic).unwrap();
        assert_eq!(json, "\"diacritic\"");
    }
}
