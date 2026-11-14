//! Hình học nét: kéo dài đầu nét, RDP, làm mượt Catmull-Rom centripetal (giữ góc nhọn).

use wb_model::PointW;

/// Nét có khép kín không (đầu ≈ cuối).
pub fn is_closed(s: &[PointW]) -> bool {
    s.len() > 2 && s[0].dist(s.last().unwrap()) < 1.0
}

/// Kéo dài hai đầu nét hở theo tiếp tuyến một đoạn `k × bán kính` — bù phần thinning gặm mất.
/// (Lố ra ngoài nét vô hại: reveal chỉ hiện điểm có mực.)
pub fn extend_ends(s: &[PointW], k: f32) -> Vec<PointW> {
    if s.len() < 2 || is_closed(s) {
        return s.to_vec();
    }
    // hướng tiếp tuyến lấy từ điểm cách đầu nét vài px để tránh nhiễu bậc thang
    let tangent_from = |pts: &[PointW]| {
        let a = pts[0];
        let b = pts
            .iter()
            .find(|p| p.dist(&a) >= 4.0)
            .copied()
            .unwrap_or(pts[pts.len() - 1]);
        let d = a.dist(&b).max(1e-4);
        ((a.x - b.x) / d, (a.y - b.y) / d)
    };
    let (hx, hy) = tangent_from(s);
    let rev: Vec<PointW> = s.iter().rev().copied().collect();
    let (tx, ty) = tangent_from(&rev);
    let (a, z) = (s[0], s[s.len() - 1]);
    let mut out = Vec::with_capacity(s.len() + 2);
    out.push(PointW::new(a.x + hx * k * a.w, a.y + hy * k * a.w, a.w));
    out.extend_from_slice(s);
    out.push(PointW::new(z.x + tx * k * z.w, z.y + ty * k * z.w, z.w));
    out
}

/// Ramer–Douglas–Peucker. Bán kính của điểm bị bỏ được "gộp" vào điểm giữ lại (lấy max) để không hụt độ phủ.
pub fn rdp(pts: &[PointW], eps: f32) -> Vec<PointW> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((i, j)) = stack.pop() {
        if j <= i + 1 {
            continue;
        }
        let (a, b) = (pts[i], pts[j]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        let mut best = (i, 0.0f32);
        for (k, p) in pts.iter().enumerate().take(j).skip(i + 1) {
            let d = if len < 1e-6 {
                p.dist(&a)
            } else {
                ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len
            };
            if d > best.1 {
                best = (k, d);
            }
        }
        if best.1 > eps {
            keep[best.0] = true;
            stack.push((i, best.0));
            stack.push((best.0, j));
        }
    }
    // gộp bán kính: mỗi điểm giữ lại nhận max(w) của các điểm bị bỏ ngay sau nó
    let mut out: Vec<PointW> = Vec::new();
    let mut pending_w = 0.0f32;
    for (k, p) in pts.iter().enumerate() {
        pending_w = pending_w.max(p.w);
        if keep[k] {
            let mut q = *p;
            q.w = q.w.max(pending_w);
            if let Some(last) = out.last_mut() {
                last.w = last.w.max(pending_w);
            }
            out.push(q);
            pending_w = 0.0;
        }
    }
    out
}

/// Góc rẽ (độ) tại điểm `b` khi đi a → b → c.
fn turn_deg(a: PointW, b: PointW, c: PointW) -> f32 {
    let (ux, uy) = (b.x - a.x, b.y - a.y);
    let (vx, vy) = (c.x - b.x, c.y - b.y);
    let (lu, lv) = ((ux * ux + uy * uy).sqrt(), (vx * vx + vy * vy).sqrt());
    if lu < 1e-6 || lv < 1e-6 {
        return 0.0;
    }
    ((ux * vx + uy * vy) / (lu * lv)).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Làm mượt: chia nét tại các góc nhọn, mỗi đoạn nội suy Catmull-Rom **centripetal** (α = 0.5,
/// không tạo vòng xoắn/vượt quá như bản uniform), lấy mẫu mỗi `spacing` px.
pub fn smooth(pts: &[PointW], spacing: f32, corner_deg: f32) -> Vec<PointW> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    // chia tại góc nhọn
    let mut chains: Vec<Vec<PointW>> = vec![vec![pts[0]]];
    for k in 1..pts.len() - 1 {
        chains.last_mut().unwrap().push(pts[k]);
        if turn_deg(pts[k - 1], pts[k], pts[k + 1]) > corner_deg {
            chains.push(vec![pts[k]]);
        }
    }
    chains.last_mut().unwrap().push(pts[pts.len() - 1]);

    let mut out: Vec<PointW> = Vec::new();
    for chain in chains {
        let n = chain.len();
        let seg_start = if out.is_empty() { 0 } else { 1 };
        if n < 3 {
            out.extend_from_slice(&chain[seg_start.min(n)..]);
            continue;
        }
        for i in 0..n - 1 {
            let p0 = if i == 0 { chain[0] } else { chain[i - 1] };
            let (p1, p2) = (chain[i], chain[i + 1]);
            let p3 = if i + 2 < n { chain[i + 2] } else { chain[n - 1] };
            let steps = ((p1.dist(&p2) / spacing).ceil() as usize).clamp(1, 64);
            let from = if i == 0 && seg_start == 0 { 0 } else { 1 };
            for s in from..=steps {
                out.push(centripetal(p0, p1, p2, p3, s as f32 / steps as f32));
            }
        }
    }
    out
}

/// Một điểm Catmull-Rom centripetal giữa p1 và p2 (thuật toán Barry–Goldman).
fn centripetal(p0: PointW, p1: PointW, p2: PointW, p3: PointW, u: f32) -> PointW {
    let knot = |a: PointW, b: PointW| a.dist(&b).sqrt().max(1e-4);
    let t0 = 0.0;
    let t1 = t0 + knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + (t2 - t1) * u;
    let mix = |a: PointW, b: PointW, ta: f32, tb: f32| {
        let k = if (tb - ta).abs() < 1e-6 {
            0.0
        } else {
            (t - ta) / (tb - ta)
        };
        a.lerp(&b, k)
    };
    let a1 = mix(p0, p1, t0, t1);
    let a2 = mix(p1, p2, t1, t2);
    let a3 = mix(p2, p3, t2, t3);
    let b1 = mix(a1, a2, t0, t2);
    let b2 = mix(a2, a3, t1, t3);
    let mut c = mix(b1, b2, t1, t2);
    // bán kính nội suy tuyến tính giữa p1, p2 (không để spline làm "phồng/hẹp")
    c.w = p1.w + (p2.w - p1.w) * u;
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdp_keeps_corner_and_max_radius() {
        let pts: Vec<PointW> = (0..=10)
            .map(|i| PointW::new(i as f32, 0.0, if i == 5 { 4.0 } else { 1.0 }))
            .chain((1..=10).map(|i| PointW::new(10.0, i as f32, 1.0)))
            .collect();
        let r = rdp(&pts, 0.5);
        assert_eq!(r.len(), 3, "{r:?}");
        assert!((r[1].x - 10.0).abs() < 1e-5 && r[1].y.abs() < 1e-5);
        assert!(r[0].w >= 4.0 || r[1].w >= 4.0, "bán kính lớn bị mất");
    }

    #[test]
    fn smooth_passes_through_control_points_and_keeps_sharp_corner() {
        let pts = vec![
            PointW::new(0.0, 0.0, 1.0),
            PointW::new(20.0, 5.0, 1.0),
            PointW::new(40.0, 0.0, 1.0),
            PointW::new(40.0, 40.0, 1.0), // góc 90°+ → giữ nhọn
        ];
        let s = smooth(&pts, 2.0, 55.0);
        for c in &pts {
            assert!(s.iter().any(|p| p.dist(c) < 1e-3), "thiếu điểm điều khiển {c:?}");
        }
        // đoạn sau góc nhọn là đường thẳng: mọi điểm có x = 40 khi y > 0
        assert!(
            s.iter()
                .filter(|p| p.y > 0.5 && p.x > 39.0)
                .all(|p| (p.x - 40.0).abs() < 1e-3)
        );
    }

    #[test]
    fn extend_ends_moves_endpoints_outward() {
        let s = vec![PointW::new(10.0, 0.0, 2.0), PointW::new(20.0, 0.0, 2.0)];
        let e = extend_ends(&s, 1.0);
        assert!((e[0].x - 8.0).abs() < 1e-4 && (e[3].x - 22.0).abs() < 1e-4);
    }
}
