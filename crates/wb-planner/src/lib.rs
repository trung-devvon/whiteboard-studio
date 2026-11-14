//! Biến danh sách nét thô thành [`DrawPlan`]: nét mượt, đủ phủ, sắp thứ tự sao cho bàn tay đi tự nhiên.
//!
//! 1. Kéo dài đầu nét (bù phần Zhang–Suen gặm mất) → [`geometry::extend_ends`]
//! 2. Đơn giản hoá RDP → làm mượt Catmull-Rom centripetal, giữ góc nhọn → [`geometry`]
//! 3. Nét vụn → "chấm" gắn vào nét lớn gần nhất, hiện ra khi bút đi ngang → [`attach_dots`]
//! 4. Thứ tự: láng giềng gần nhất trên R-Tree (xét cả 2 đầu, cho đảo chiều) + 2-opt → [`order`]
//!
//! Phải biên dịch được sang wasm32 (không I/O).

pub mod geometry;
pub mod order;

use rstar::{RTree, primitives::GeomWithData};
use wb_model::{DrawPlan, PlannedStroke, PointW, StrokeDot, StrokeKind, polyline_length};

/// Tham số planner (đơn vị: px ảnh nguồn).
#[derive(Debug, Clone, Copy)]
pub struct PlannerParams {
    /// Sai số RDP.
    pub simplify_eps: f32,
    /// Nét ngắn hơn ngưỡng này thành chấm vụn.
    pub min_stroke_length: f32,
    /// Khoảng cách điểm sau khi làm mượt.
    pub smooth_spacing: f32,
    /// Góc rẽ (độ) lớn hơn ngưỡng này được giữ là góc nhọn, không làm tròn.
    pub corner_deg: f32,
}

impl PlannerParams {
    /// Tham số mặc định theo tỉ lệ ảnh nguồn / ảnh làm việc (`1 / work_scale`).
    pub fn for_scale(src_per_work_px: f32, min_stroke_length_work: f32) -> Self {
        Self {
            simplify_eps: 0.7 * src_per_work_px,
            min_stroke_length: min_stroke_length_work * src_per_work_px,
            smooth_spacing: 3.0 * src_per_work_px,
            corner_deg: 55.0,
        }
    }
}

/// Lập kế hoạch vẽ từ các nét thô (px ảnh nguồn).
pub fn plan(raw: Vec<Vec<PointW>>, source_w: u32, source_h: u32, kind: StrokeKind, p: &PlannerParams) -> DrawPlan {
    // 1–2: hình học từng nét
    let shaped: Vec<Vec<PointW>> = raw
        .into_iter()
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s.len() == 1 {
                return s;
            }
            let s = geometry::extend_ends(&s, 0.9);
            let s = geometry::rdp(&s, p.simplify_eps);
            geometry::smooth(&s, p.smooth_spacing, p.corner_deg)
        })
        .collect();

    // 3: tách nét lớn / nét vụn
    let (major, micro): (Vec<_>, Vec<_>) = shaped
        .into_iter()
        .partition(|s| polyline_length(s) >= p.min_stroke_length);
    let (major, dots) = if major.is_empty() {
        (micro, Vec::new())
    } else {
        attach_dots(&major, micro)
    };

    // 4: thứ tự
    let ends: Vec<(PointW, PointW)> = major.iter().map(|s| (s[0], *s.last().unwrap())).collect();
    let seq = order::two_opt(&ends, order::greedy(&ends));

    let mut strokes = Vec::with_capacity(seq.len());
    for (idx, reversed) in seq {
        let mut pts = major[idx].clone();
        let len = polyline_length(&pts);
        let mut ds: Vec<StrokeDot> = dots
            .iter()
            .filter(|(i, _)| *i == idx)
            .map(|(_, d)| StrokeDot {
                at: if reversed { len - d.at } else { d.at },
                point: d.point,
            })
            .collect();
        if reversed {
            pts.reverse();
        }
        ds.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut s = PlannedStroke::new(kind, pts);
        s.dots = ds;
        strokes.push(s);
    }
    DrawPlan::new(source_w, source_h, strokes)
}

/// Gắn mỗi nét vụn vào điểm gần nhất của một nét lớn. Trả (nét lớn, [(chỉ số nét lớn, chấm)]).
pub fn attach_dots(major: &[Vec<PointW>], micro: Vec<Vec<PointW>>) -> (Vec<Vec<PointW>>, Vec<(usize, StrokeDot)>) {
    let mut items = Vec::new();
    for (i, s) in major.iter().enumerate() {
        let mut acc = 0.0;
        for (k, pt) in s.iter().enumerate() {
            if k > 0 {
                acc += s[k - 1].dist(pt);
            }
            items.push(GeomWithData::new([pt.x, pt.y], (i, acc)));
        }
    }
    let tree = RTree::bulk_load(items);
    let mut out = Vec::new();
    for m in micro {
        // chấm đại diện: tâm + bán kính bao trọn nét vụn
        let n = m.len() as f32;
        let (cx, cy) = m.iter().fold((0.0, 0.0), |(x, y), p| (x + p.x / n, y + p.y / n));
        let r = m
            .iter()
            .map(|p| ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt() + p.w)
            .fold(0.0f32, f32::max);
        if let Some(near) = tree.nearest_neighbor([cx, cy]) {
            let (i, at) = near.data;
            out.push((
                i,
                StrokeDot {
                    at,
                    point: PointW::new(cx, cy, r),
                },
            ));
        }
    }
    (major.to_vec(), out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<PointW> {
        (0..=20)
            .map(|k| {
                let t = k as f32 / 20.0;
                PointW::new(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, 2.0)
            })
            .collect()
    }

    #[test]
    fn plan_orders_reverses_and_attaches_dots() {
        // 3 đoạn ngang chồng lên nhau theo chiều dọc, đoạn giữa bị đảo hướng → bút nên đi zíc-zắc
        let raw = vec![
            line(0.0, 0.0, 100.0, 0.0),
            line(0.0, 20.0, 100.0, 20.0),
            line(0.0, 10.0, 100.0, 10.0),
            vec![PointW::new(50.0, 5.0, 1.5)], // chấm vụn
        ];
        let p = PlannerParams {
            simplify_eps: 0.5,
            min_stroke_length: 5.0,
            smooth_spacing: 3.0,
            corner_deg: 55.0,
        };
        let plan = plan(raw, 100, 30, StrokeKind::Outline, &p);
        assert_eq!(plan.strokes.len(), 3);
        assert_eq!(plan.stats.dot_count, 1);
        // tổng quãng nhấc bút nhỏ: zíc-zắc 0 → 10 → 20 (mỗi lần ~10px, cộng phần kéo dài đầu nét)
        assert!(plan.stats.travel_length < 30.0, "nhấc bút {}", plan.stats.travel_length);
        // nét đầu tiên bắt đầu ở góc trên-trái
        let first = plan.strokes[0].points[0];
        assert!(first.y < 1.0 && first.x < 5.0, "bắt đầu tại {first:?}");
    }
}
