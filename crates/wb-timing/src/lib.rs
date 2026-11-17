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
    /// Mốc lời thoại: nét đầu bắt đầu đúng `anchor_ms` sau đầu scene (dấu `[n]` trong lời thoại).
    pub anchor_ms: Option<f64>,
}

pub struct SceneInput<'a> {
    pub elements: Vec<ElementInput<'a>>,
    pub tail_ms: u32,
    /// Scene kéo dài ít nhất chừng này (độ dài giọng đọc, ms). 0 = không có lời thoại.
    pub min_ms: f64,
}

/// Khoảng lặng sau khi đọc xong lời thoại trước khi sang scene kế (ms).
pub const NARRATION_TAIL_MS: f64 = 400.0;
/// Element bị rút ngắn để kịp mốc lời thoại kế tiếp thì vẫn vẽ ít nhất chừng này (ms).
pub const MIN_SQUEEZED_MS: f64 = 400.0;

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
        // thời lượng thô từng nét / nhấc bút và thời lượng "tự nhiên" của từng element
        let raws: Vec<RawElement> = sc.elements.iter().map(|el| RawElement::new(el, p)).collect();
        let mut els = Vec::with_capacity(sc.elements.len());
        for (i, el) in sc.elements.iter().enumerate() {
            let gap = el.timing.gap_before_ms as f64;
            // có mốc lời thoại muộn hơn → chờ tới mốc (tay vẫn đi vào trước nét đầu một khoảng `gap`)
            let draw_start = match el.anchor_ms {
                Some(a) => (t + gap).max(start + a),
                None => t + gap,
            };
            let enter = draw_start - gap;
            let r = &raws[i];
            let target = if el.timing.duration_ms.is_none() {
                squeezed(sc, &raws, i, start, draw_start)
            } else {
                r.natural
            };
            let k = if r.total > 0.0 { target / r.total } else { 1.0 };
            let mut slots = Vec::with_capacity(r.draw.len());
            let mut c = draw_start;
            for (lift, draw) in r.lift.iter().zip(&r.draw) {
                c += lift * k;
                let t0 = c;
                c += draw * k;
                slots.push(StrokeSlot { t0, t1: c });
            }
            let draw_end = if r.draw.is_empty() { draw_start } else { c };
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
        if sc.min_ms > 0.0 {
            t = t.max(start + sc.min_ms + NARRATION_TAIL_MS);
        }
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

/// Thời lượng thô của một element (trước khi co giãn).
struct RawElement {
    draw: Vec<f64>,
    lift: Vec<f64>,
    /// Tổng nét + nhấc bút.
    total: f64,
    /// Thời lượng định vẽ: ép bằng `duration_ms`, hoặc `total` kẹp trong [min, max].
    natural: f64,
}

impl RawElement {
    fn new(el: &ElementInput, p: &TimingParams) -> Self {
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
        let total = draw.iter().sum::<f64>() + lift.iter().sum::<f64>();
        let natural = match el.timing.duration_ms {
            Some(ms) => ms as f64,
            None => total.clamp(p.min_element_ms, p.max_element_ms),
        };
        Self {
            draw,
            lift,
            total,
            natural,
        }
    }
}

/// Thời lượng vẽ element `i` (tự tính) sao cho mọi element từ `i` tới trước mốc lời thoại kế tiếp
/// vẽ kịp: chỗ trống chia theo tỉ lệ thời lượng tự nhiên; element nào xuống dưới [`MIN_SQUEEZED_MS`]
/// thì giữ ở mức đó và phần còn lại chia cho các element khác (element ép `duration_ms` giữ nguyên).
/// Không có mốc phía sau, hoặc đủ chỗ → thời lượng tự nhiên.
fn squeezed(sc: &SceneInput, raws: &[RawElement], i: usize, scene_start: f64, draw_start: f64) -> f64 {
    let natural = raws[i].natural;
    let Some(j) = (i + 1..sc.elements.len()).find(|&j| sc.elements[j].anchor_ms.is_some()) else {
        return natural;
    };
    let deadline = scene_start + sc.elements[j].anchor_ms.unwrap_or_default();
    // từ nét đầu của i tới nét đầu của j: hoàn thiện + khoảng nghỉ (cố định) và thời gian vẽ
    let mut room = deadline - draw_start - sc.elements[j].timing.gap_before_ms as f64;
    let mut flex: Vec<f64> = Vec::new(); // thời lượng tự nhiên của các element co giãn được, [0] = i
    for k in i..j {
        let el = &sc.elements[k];
        room -= el.timing.settle_ms as f64;
        if k > i {
            room -= el.timing.gap_before_ms as f64;
        }
        match el.timing.duration_ms {
            Some(_) => room -= raws[k].natural,
            None => flex.push(raws[k].natural),
        }
    }
    let total: f64 = flex.iter().sum();
    if total <= room || total <= 0.0 {
        return natural;
    }
    // chia theo tỉ lệ; ai dưới mức tối thiểu → giữ mức tối thiểu, chia lại phần còn lại
    let floor = |n: f64| MIN_SQUEEZED_MS.min(n);
    let mut pinned = vec![false; flex.len()];
    loop {
        let fixed: f64 = flex
            .iter()
            .zip(&pinned)
            .filter(|(_, p)| **p)
            .map(|(n, _)| floor(*n))
            .sum();
        let free: f64 = flex.iter().zip(&pinned).filter(|(_, p)| !**p).map(|(n, _)| *n).sum();
        let f = ((room - fixed) / free.max(1e-9)).clamp(0.0, 1.0);
        let mut changed = false;
        for (n, p) in flex.iter().zip(pinned.iter_mut()) {
            if !*p && n * f < floor(*n) {
                *p = true;
                changed = true;
            }
        }
        if !changed {
            return if pinned[0] { floor(natural) } else { natural * f };
        }
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
                    anchor_ms: None,
                }],
                tail_ms: 1000,
                min_ms: 0.0,
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
                    anchor_ms: None,
                }],
                tail_ms: 0,
                min_ms: 0.0,
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
                    anchor_ms: None,
                }],
                tail_ms: 0,
                min_ms: 0.0,
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

    /// Lời thoại: element 2 bắt đầu đúng mốc `[2]`, element 1 vẽ nhanh hơn cho kịp, scene dài ít nhất bằng giọng đọc.
    #[test]
    fn narration_anchors_start_elements_on_their_words() {
        let pl = plan();
        let el = |anchor_ms| ElementInput {
            plan: &pl,
            scale: 1.0,
            timing: Timing::default(),
            anchor_ms,
        };
        let p = TimingParams::default();
        // không có mốc: element 1 cần ~1300 ms (min 1200) + 300 hoàn thiện
        let free = resolve(
            &[SceneInput {
                elements: vec![el(None), el(None)],
                tail_ms: 0,
                min_ms: 0.0,
            }],
            &p,
        );
        assert!(free.scenes[0].elements[1].draw_start_ms > 1800.0);

        // mốc [1] lúc 1000 ms, [2] lúc 2000 ms, giọng đọc dài 6000 ms
        let tl = resolve(
            &[SceneInput {
                elements: vec![el(Some(1000.0)), el(Some(2000.0))],
                tail_ms: 0,
                min_ms: 6000.0,
            }],
            &p,
        );
        let e = &tl.scenes[0].elements;
        assert_eq!(e[0].draw_start_ms, 1000.0);
        assert_eq!(e[0].enter_ms, 700.0); // tay vào trước nét đầu 300 ms
        assert!((e[1].draw_start_ms - 2000.0).abs() < 1e-6);
        assert!(
            e[0].end_ms <= e[1].enter_ms + 1e-6,
            "element 1 phải xong trước khi element 2 vào"
        );
        assert!(e[0].draw_end_ms - e[0].draw_start_ms >= MIN_SQUEEZED_MS - 1e-6);
        assert_eq!(tl.total_ms, 6000.0 + NARRATION_TAIL_MS);

        // mốc quá sớm (trước khi element trước kịp xong) → không giật lùi, vẽ nối tiếp
        let tl = resolve(
            &[SceneInput {
                elements: vec![el(None), el(Some(10.0))],
                tail_ms: 0,
                min_ms: 0.0,
            }],
            &p,
        );
        let e = &tl.scenes[0].elements;
        assert!(e[1].draw_start_ms >= e[0].end_ms);
    }
}
