//! Giải thời gian (Timeline) cho dự án: mỗi nét được vẽ trong khoảng nào, nhấc bút bao lâu,
//! và tại thời điểm `t` thì mũi bút đang ở đâu.
//!
//! Giai đoạn 1: element nối tiếp nhau, thời lượng tự tính theo độ dài nét (hoặc ép bằng `duration_ms`).
//! Đồng bộ theo lời thoại (marker) ở Giai đoạn 4. Kế hoạch: `not_push_git/04-…` §3.
//! Phải biên dịch được sang wasm32 (không I/O).

use wb_model::{DrawPlan, StrokeKind, Timing};

/// Tham số tốc độ (đơn vị px khung tham chiếu 1920×1080, ms).
#[derive(Debug, Clone, Copy)]
pub struct TimingParams {
    /// Tốc độ vẽ danh nghĩa (px/giây).
    pub draw_speed: f32,
    /// Tốc độ cọ tô (nét `Fill`, px/giây) — cọ rộng nên đi nhanh hơn bút.
    pub fill_speed: f32,
    /// Tốc độ di chuyển khi nhấc bút (px/giây).
    pub travel_speed: f32,
    /// Thời gian nhấc bút tối thiểu / tối đa giữa hai nét (ms).
    pub lift_min_ms: f64,
    pub lift_max_ms: f64,
    /// Giới hạn tổng thời gian vẽ một element khi tự tính (ms).
    pub min_element_ms: f64,
    pub max_element_ms: f64,
}

impl Default for TimingParams {
    fn default() -> Self {
        Self {
            draw_speed: 1400.0,
            fill_speed: 4200.0,
            travel_speed: 3500.0,
            lift_min_ms: 60.0,
            lift_max_ms: 220.0,
            min_element_ms: 1200.0,
            max_element_ms: 9000.0,
        }
    }
}

/// Đầu vào của một element: kế hoạch nét + tỉ lệ px ảnh nguồn → px khung tham chiếu.
pub struct ElementInput<'a> {
    pub plan: &'a DrawPlan,
    pub scale: f32,
    pub timing: Timing,
}

pub struct SceneInput<'a> {
    pub elements: Vec<ElementInput<'a>>,
    pub tail_ms: u32,
}

/// Khoảng thời gian vẽ một nét (ms tuyệt đối).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeSlot {
    pub t0: f64,
    pub t1: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementTimeline {
    /// Bắt đầu xuất hiện (tay bắt đầu đi vào) = kết thúc element trước + `gap_before_ms`.
    pub enter_ms: f64,
    /// Bắt đầu nét đầu tiên.
    pub draw_start_ms: f64,
    pub draw_end_ms: f64,
    /// Kết thúc pha "hoàn thiện" (phần chưa phủ hiện dần) — element coi như xong.
    pub end_ms: f64,
    pub slots: Vec<StrokeSlot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneTimeline {
    pub start_ms: f64,
    pub end_ms: f64,
    pub elements: Vec<ElementTimeline>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub scenes: Vec<SceneTimeline>,
    pub total_ms: f64,
}

/// Thời gian tay đi vào khung trước nét đầu tiên (ms) — lấy từ khoảng nghỉ `gap_before_ms`, tối đa giá trị này.
pub const MAX_ENTER_MS: f64 = 450.0;

/// Giải Timeline cho toàn dự án.
pub fn resolve(scenes: &[SceneInput], p: &TimingParams) -> Timeline {
    let mut t = 0.0f64;
    let mut out = Vec::with_capacity(scenes.len());
    for sc in scenes {
        let start = t;
        let mut els = Vec::with_capacity(sc.elements.len());
        for el in &sc.elements {
            let enter = t;
            let draw_start = t + el.timing.gap_before_ms as f64;
            // thời lượng thô từng nét và từng lần nhấc bút
            let strokes = &el.plan.strokes;
            let draw: Vec<f64> = strokes
                .iter()
                .map(|s| {
                    let speed = if s.kind == StrokeKind::Fill {
                        p.fill_speed
                    } else {
                        p.draw_speed
                    };
                    (s.length * el.scale / speed) as f64 * 1000.0
                })
                .collect();
            let lift: Vec<f64> = (0..strokes.len())
                .map(|i| {
                    if i == 0 {
                        return 0.0;
                    }
                    let (a, b) = (strokes[i - 1].points.last(), strokes[i].points.first());
                    let d = match (a, b) {
                        (Some(a), Some(b)) => a.dist(b) * el.scale,
                        _ => 0.0,
                    };
                    (p.lift_min_ms + (d / p.travel_speed) as f64 * 1000.0).min(p.lift_max_ms)
                })
                .collect();
            let raw: f64 = draw.iter().sum::<f64>() + lift.iter().sum::<f64>();
            let target = match el.timing.duration_ms {
                Some(ms) => ms as f64,
                None => raw.clamp(p.min_element_ms, p.max_element_ms),
            };
            let k = if raw > 0.0 { target / raw } else { 1.0 };
            let mut slots = Vec::with_capacity(strokes.len());
            let mut c = draw_start;
            for i in 0..strokes.len() {
                c += lift[i] * k;
                let t0 = c;
                c += draw[i] * k;
                slots.push(StrokeSlot { t0, t1: c });
            }
            let draw_end = if strokes.is_empty() { draw_start } else { c };
            let end = draw_end + el.timing.settle_ms as f64;
            els.push(ElementTimeline {
                enter_ms: enter,
                draw_start_ms: draw_start,
                draw_end_ms: draw_end,
                end_ms: end,
                slots,
            });
            t = end;
        }
        t += sc.tail_ms as f64;
        out.push(SceneTimeline {
            start_ms: start,
            end_ms: t,
            elements: els,
        });
    }
    Timeline {
        scenes: out,
        total_ms: t,
    }
}

/// Quỹ đạo minimum-jerk: s(τ) = 10τ³ − 15τ⁴ + 6τ⁵ — mô hình kinh điển của chuyển động tay người
/// (tăng tốc êm, giảm tốc êm, vận tốc lớn nhất ở giữa nét).
pub fn minimum_jerk(tau: f64) -> f64 {
    let t = tau.clamp(0.0, 1.0);
    t * t * t * (10.0 - 15.0 * t + 6.0 * t * t)
}

/// Trạng thái vẽ của một element tại thời điểm `t`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    /// Chưa tới lượt (hoặc tay đang đi vào): `u` ∈ [0,1] là tiến độ đi vào.
    Entering {
        u: f32,
    },
    /// Đang đặt bút trên nét `stroke`, đã đi được `s` px (độ dài cung, px ảnh nguồn).
    Drawing {
        stroke: usize,
        s: f32,
    },
    /// Nhấc bút từ cuối nét `stroke - 1` tới đầu nét `stroke`, `u` ∈ [0,1].
    Lifting {
        stroke: usize,
        u: f32,
    },
    /// Vẽ xong, đang hoàn thiện (`u` ∈ [0,1]) — tay rút ra.
    Settling {
        u: f32,
    },
    Done,
}

/// Tra trạng thái element tại `t` (ms tuyệt đối).
pub fn progress_at(el: &ElementTimeline, plan: &DrawPlan, t: f64) -> Progress {
    if t >= el.end_ms {
        return Progress::Done;
    }
    if t >= el.draw_end_ms {
        let d = (el.end_ms - el.draw_end_ms).max(1e-6);
        return Progress::Settling {
            u: ((t - el.draw_end_ms) / d) as f32,
        };
    }
    if t < el.draw_start_ms || el.slots.is_empty() {
        let d = (el.draw_start_ms - el.enter_ms).max(1e-6);
        return Progress::Entering {
            u: ((t - el.enter_ms) / d).clamp(0.0, 1.0) as f32,
        };
    }
    // nét đầu tiên có t0 > t → đang nhấc bút tới nét đó; ngược lại đang vẽ nét cuối cùng có t0 ≤ t
    let i = el.slots.partition_point(|s| s.t0 <= t);
    if i == 0 {
        return Progress::Entering { u: 1.0 };
    }
    let k = i - 1;
    let slot = el.slots[k];
    if t < slot.t1 {
        let tau = (t - slot.t0) / (slot.t1 - slot.t0).max(1e-9);
        let len = plan.strokes[k].length as f64;
        return Progress::Drawing {
            stroke: k,
            s: (len * minimum_jerk(tau)) as f32,
        };
    }
    // giữa slot k và k+1
    let next = el.slots[k + 1];
    let u = (t - slot.t1) / (next.t0 - slot.t1).max(1e-9);
    Progress::Lifting {
        stroke: k + 1,
        u: u.clamp(0.0, 1.0) as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wb_model::{PlannedStroke, PointW, StrokeKind};

    fn plan() -> DrawPlan {
        let s = |x0: f32, x1: f32| {
            PlannedStroke::new(
                StrokeKind::Outline,
                vec![PointW::new(x0, 0.0, 1.0), PointW::new(x1, 0.0, 1.0)],
            )
        };
        DrawPlan::new(1000, 100, vec![s(0.0, 700.0), s(800.0, 1500.0)])
    }

    #[test]
    fn resolves_sequential_slots_and_scene_end() {
        let pl = plan();
        let p = TimingParams {
            min_element_ms: 0.0,
            ..TimingParams::default()
        };
        let tl = resolve(
            &[SceneInput {
                elements: vec![ElementInput {
                    plan: &pl,
                    scale: 1.0,
                    timing: Timing::default(),
                }],
                tail_ms: 1000,
            }],
            &p,
        );
        let el = &tl.scenes[0].elements[0];
        assert_eq!(el.draw_start_ms, 300.0);
        assert!((el.slots[0].t1 - el.slots[0].t0 - 500.0).abs() < 1e-6); // 700px / 1400px/s
        assert!(el.slots[1].t0 > el.slots[0].t1); // có nhấc bút
        assert_eq!(tl.total_ms, el.end_ms + 1000.0);
    }

    #[test]
    fn forced_duration_scales_everything() {
        let pl = plan();
        let timing = Timing {
            duration_ms: Some(4000),
            ..Timing::default()
        };
        let tl = resolve(
            &[SceneInput {
                elements: vec![ElementInput {
                    plan: &pl,
                    scale: 1.0,
                    timing,
                }],
                tail_ms: 0,
            }],
            &TimingParams::default(),
        );
        let el = &tl.scenes[0].elements[0];
        assert!((el.draw_end_ms - el.draw_start_ms - 4000.0).abs() < 1e-6);
    }

    #[test]
    fn progress_walks_through_all_phases() {
        let pl = plan();
        let tl = resolve(
            &[SceneInput {
                elements: vec![ElementInput {
                    plan: &pl,
                    scale: 1.0,
                    timing: Timing::default(),
                }],
                tail_ms: 0,
            }],
            &TimingParams {
                min_element_ms: 0.0,
                ..TimingParams::default()
            },
        );
        let el = &tl.scenes[0].elements[0];
        assert!(matches!(progress_at(el, &pl, 100.0), Progress::Entering { .. }));
        match progress_at(el, &pl, (el.slots[0].t0 + el.slots[0].t1) / 2.0) {
            Progress::Drawing { stroke: 0, s } => assert!(
                (s - 350.0).abs() < 1.0,
                "giữa nét theo minimum-jerk = nửa độ dài, có {s}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            progress_at(el, &pl, (el.slots[0].t1 + el.slots[1].t0) / 2.0),
            Progress::Lifting { stroke: 1, .. }
        ));
        assert!(matches!(
            progress_at(el, &pl, el.draw_end_ms + 1.0),
            Progress::Settling { .. }
        ));
        assert_eq!(progress_at(el, &pl, el.end_ms), Progress::Done);
    }

    #[test]
    fn minimum_jerk_is_smooth_and_symmetric() {
        assert_eq!(minimum_jerk(0.0), 0.0);
        assert_eq!(minimum_jerk(1.0), 1.0);
        assert!((minimum_jerk(0.5) - 0.5).abs() < 1e-12);
        assert!((minimum_jerk(0.2) + minimum_jerk(0.8) - 1.0).abs() < 1e-12);
    }
}
