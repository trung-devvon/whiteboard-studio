//! Dữ liệu dẫn xuất: kết quả "biên dịch" một Element thành danh sách nét đã sắp thứ tự.
//! Toạ độ nét tính theo **px của ảnh nguồn** (engine tự áp transform của Element).

use serde::{Deserialize, Serialize};

/// Biên an toàn (px ảnh nguồn) cộng vào bán kính nét khi đóng dấu reveal. Dùng chung cho engine và đo độ phủ.
pub const REVEAL_MARGIN: f32 = 1.5;

/// Một điểm trên đường tâm nét, kèm bán kính cọ `w` tại điểm đó (px).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PointW {
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

impl PointW {
    pub const fn new(x: f32, y: f32, w: f32) -> Self {
        Self { x, y, w }
    }
    pub fn dist(&self, o: &PointW) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    /// Nội suy tuyến tính (cả bán kính).
    pub fn lerp(&self, o: &PointW, t: f32) -> PointW {
        PointW {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
            w: self.w + (o.w - self.w) * t,
        }
    }
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

/// Chấm vụn gắn vào một nét lớn: hiện ra khi mũi bút đi qua vị trí `at` (độ dài cung, px) của nét đó.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct StrokeDot {
    pub at: f32,
    pub point: PointW,
}

/// Một nét đã được sắp thứ tự, sẵn sàng cho bộ giải thời gian và engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlannedStroke {
    pub kind: StrokeKind,
    pub points: Vec<PointW>,
    /// Độ dài cung (px), tính sẵn.
    pub length: f32,
    /// Chấm vụn hiện ra khi bút đi ngang (không bắt tay đi tới từng chấm).
    #[serde(default)]
    pub dots: Vec<StrokeDot>,
}

impl PlannedStroke {
    /// Tạo nét và tính sẵn độ dài cung.
    pub fn new(kind: StrokeKind, points: Vec<PointW>) -> Self {
        let length = polyline_length(&points);
        Self {
            kind,
            points,
            length,
            dots: Vec::new(),
        }
    }

    /// Điểm tại độ dài cung `s` (kẹp vào [0, length]).
    pub fn point_at(&self, s: f32) -> PointW {
        let mut acc = 0.0;
        for seg in self.points.windows(2) {
            let l = seg[0].dist(&seg[1]);
            if acc + l >= s && l > 0.0 {
                return seg[0].lerp(&seg[1], ((s - acc) / l).clamp(0.0, 1.0));
            }
            acc += l;
        }
        *self.points.last().expect("nét rỗng")
    }

    /// Các điểm của đoạn nét trong khoảng độ dài cung [a, b] (để "đóng dấu" phần mới vẽ).
    pub fn sub_path(&self, a: f32, b: f32) -> Vec<PointW> {
        let mut out = Vec::new();
        if b <= a || self.points.is_empty() {
            return out;
        }
        if self.points.len() == 1 {
            return vec![self.points[0]];
        }
        let mut acc = 0.0;
        for seg in self.points.windows(2) {
            let l = seg[0].dist(&seg[1]);
            let (s0, s1) = (acc, acc + l);
            if s1 >= a && s0 <= b {
                if out.is_empty() {
                    let t = if l > 0.0 { ((a - s0) / l).clamp(0.0, 1.0) } else { 0.0 };
                    out.push(seg[0].lerp(&seg[1], t));
                }
                let t = if l > 0.0 { ((b - s0) / l).clamp(0.0, 1.0) } else { 1.0 };
                out.push(seg[0].lerp(&seg[1], t));
            }
            acc = s1;
            if acc > b {
                break;
            }
        }
        out
    }
}

/// Thống kê dùng cho `wb-timing` (ngân sách thời gian).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PlanStats {
    pub stroke_count: u32,
    pub dot_count: u32,
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
            stats.dot_count += s.dots.len() as u32;
            match s.kind {
                StrokeKind::Fill => stats.fill_length += s.length,
                _ => stats.outline_length += s.length,
            }
            if let (Some(a), Some(b)) = (prev_end, s.points.first()) {
                stats.travel_length += a.dist(b);
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

/// Độ dài đường gấp khúc.
pub fn polyline_length(points: &[PointW]) -> f32 {
    points.windows(2).map(|w| w[0].dist(&w[1])).sum()
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
    fn point_at_and_sub_path() {
        let s = PlannedStroke::new(StrokeKind::Outline, vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
        assert_eq!(s.length, 20.0);
        let m = s.point_at(15.0);
        assert!((m.x - 10.0).abs() < 1e-5 && (m.y - 5.0).abs() < 1e-5);
        let sub = s.sub_path(5.0, 15.0);
        assert_eq!(sub.len(), 3); // (5,0) → (10,0) → (10,5)
        assert!((sub[0].x - 5.0).abs() < 1e-5);
        assert!((sub[2].y - 5.0).abs() < 1e-5);
        assert!((polyline_length(&sub) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn drawplan_roundtrips_through_postcard_with_and_without_dots() {
        // postcard không tự mô tả: mọi trường phải luôn được ghi (không dùng skip_serializing_if)
        let mut a = PlannedStroke::new(StrokeKind::Outline, vec![p(0.0, 0.0), p(5.0, 0.0)]);
        a.dots.push(StrokeDot {
            at: 2.0,
            point: p(2.0, 1.0),
        });
        let b = PlannedStroke::new(StrokeKind::Text, vec![p(1.0, 1.0)]);
        let plan = DrawPlan::new(10, 10, vec![a, b]);
        let bytes = postcard::to_allocvec(&plan).unwrap();
        let back: DrawPlan = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(plan, back);
    }

    #[test]
    fn stroke_kind_serializes_snake_case() {
        let json = serde_json::to_string(&StrokeKind::Diacritic).unwrap();
        assert_eq!(json, "\"diacritic\"");
    }
}
