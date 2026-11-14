//! Sắp thứ tự nét để giảm quãng nhấc bút: tham lam (R-Tree, xét cả 2 đầu) rồi cải thiện bằng 2-opt.
//! Mỗi phần tử kết quả: (chỉ số nét, có đảo chiều không).

use rstar::{RTree, primitives::GeomWithData};
use wb_model::PointW;

pub type Seq = Vec<(usize, bool)>;

/// Chi phí di chuyển bút từ `a` tới `b`. Đi ngược lên trên bị phạt nhẹ (người thường vẽ trên → dưới).
fn travel(a: PointW, b: PointW) -> f32 {
    let d = a.dist(&b);
    let up = (a.y - b.y).max(0.0);
    d + 0.15 * up
}

/// Chi phí hướng vẽ một nét: người thường kẻ **trái → phải** và **trên → dưới** → phạt chiều ngược lại.
fn dir_cost(ends: &[(PointW, PointW)], e: (usize, bool)) -> f32 {
    let (s, t) = (start_of(ends, e), end_of(ends, e));
    0.5 * ((s.x - t.x).max(0.0) + 0.6 * (s.y - t.y).max(0.0))
}

fn flip(e: (usize, bool)) -> (usize, bool) {
    (e.0, !e.1)
}

/// Láng giềng gần nhất tham lam. Bắt đầu ở đầu nét gần góc trên-trái nhất.
pub fn greedy(ends: &[(PointW, PointW)]) -> Seq {
    if ends.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::with_capacity(ends.len() * 2);
    for (i, (a, b)) in ends.iter().enumerate() {
        items.push(GeomWithData::new([a.x, a.y], (i, false)));
        items.push(GeomWithData::new([b.x, b.y], (i, true)));
    }
    let mut tree = RTree::bulk_load(items);
    let mut used = vec![false; ends.len()];
    let start = tree
        .iter()
        .min_by(|p, q| {
            let s = |g: &GeomWithData<[f32; 2], (usize, bool)>| g.geom()[1] + 0.5 * g.geom()[0];
            s(p).total_cmp(&s(q))
        })
        .map(|g| g.data)
        .unwrap();
    let mut seq = Vec::with_capacity(ends.len());
    let mut take = |tree: &mut RTree<GeomWithData<[f32; 2], (usize, bool)>>, (i, rev): (usize, bool)| {
        used[i] = true;
        let (a, b) = ends[i];
        tree.remove(&GeomWithData::new([a.x, a.y], (i, false)));
        tree.remove(&GeomWithData::new([b.x, b.y], (i, true)));
        seq.push((i, rev));
        if rev { a } else { b } // điểm kết thúc của nét vừa vẽ
    };
    let mut pos = take(&mut tree, start);
    while tree.size() > 0 {
        // xét vài ứng viên gần nhất, chọn theo chi phí có thiên hướng
        let best = tree
            .nearest_neighbor_iter([pos.x, pos.y])
            .take(6)
            .map(|g| {
                let p = PointW::new(g.geom()[0], g.geom()[1], 0.0);
                (g.data, travel(pos, p) + dir_cost(ends, g.data))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
            .unwrap();
        pos = take(&mut tree, best);
    }
    seq
}

fn start_of(ends: &[(PointW, PointW)], (i, rev): (usize, bool)) -> PointW {
    if rev { ends[i].1 } else { ends[i].0 }
}
fn end_of(ends: &[(PointW, PointW)], (i, rev): (usize, bool)) -> PointW {
    if rev { ends[i].0 } else { ends[i].1 }
}

/// Tổng chi phí của một thứ tự: di chuyển giữa các nét + hướng vẽ từng nét.
pub fn total_cost(ends: &[(PointW, PointW)], seq: &Seq) -> f32 {
    let travel_sum: f32 = seq
        .windows(2)
        .map(|w| travel(end_of(ends, w[0]), start_of(ends, w[1])))
        .sum();
    travel_sum + seq.iter().map(|&e| dir_cost(ends, e)).sum::<f32>()
}

/// 2-opt có hướng: đảo ngược một đoạn liên tiếp (và đảo chiều từng nét trong đoạn) nếu giảm quãng nhấc bút.
/// Với nhiều nét, chỉ xét đoạn dài tối đa `window` để giữ thời gian chạy hợp lý.
pub fn two_opt(ends: &[(PointW, PointW)], mut seq: Seq) -> Seq {
    let n = seq.len();
    if n < 3 {
        return seq;
    }
    let window = if n <= 1500 { n } else { 300 };
    for _pass in 0..8 {
        let mut improved = false;
        for i in 0..n - 1 {
            // chi phí bên trong đoạn [i, j] (thuận và ngược) — cộng dồn theo j → O(n²) thay vì O(n³)
            // (gồm cả chi phí hướng vẽ của từng nét — đảo đoạn thì mọi nét trong đoạn đổi chiều)
            let (mut inner_old, mut inner_new) = (dir_cost(ends, seq[i]), dir_cost(ends, flip(seq[i])));
            for j in i + 1..n.min(i + window) {
                inner_old += travel(end_of(ends, seq[j - 1]), start_of(ends, seq[j])) + dir_cost(ends, seq[j]);
                inner_new += travel(start_of(ends, seq[j]), end_of(ends, seq[j - 1])) + dir_cost(ends, flip(seq[j]));
                let (si, ej) = (start_of(ends, seq[i]), end_of(ends, seq[j]));
                let mut old = 0.0;
                let mut new = 0.0;
                if i > 0 {
                    let prev = end_of(ends, seq[i - 1]);
                    old += travel(prev, si);
                    new += travel(prev, ej);
                }
                if j + 1 < n {
                    let next = start_of(ends, seq[j + 1]);
                    old += travel(end_of(ends, seq[j]), next);
                    new += travel(si, next);
                }
                if new + inner_new + 1e-3 < old + inner_old {
                    seq[i..=j].reverse();
                    for e in &mut seq[i..=j] {
                        e.1 = !e.1;
                    }
                    improved = true;
                    break; // thứ tự đã đổi → tổng cộng dồn không còn đúng, chuyển sang i kế tiếp
                }
            }
        }
        if !improved {
            break;
        }
    }
    seq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_opt_never_worse_than_greedy() {
        // lưới nét ngắn rải rác (tất định)
        let mut s = 12345u32;
        let mut r = || {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            (s >> 8) as f32 / (1u32 << 24) as f32 * 500.0
        };
        let ends: Vec<(PointW, PointW)> = (0..200)
            .map(|_| {
                let (x, y) = (r(), r());
                (PointW::new(x, y, 1.0), PointW::new(x + 8.0, y + 3.0, 1.0))
            })
            .collect();
        let g = greedy(&ends);
        let t = two_opt(&ends, g.clone());
        let mut ids: Vec<usize> = t.iter().map(|e| e.0).collect();
        ids.sort();
        assert_eq!(ids, (0..200).collect::<Vec<_>>(), "phải là một hoán vị đầy đủ");
        assert!(total_cost(&ends, &t) <= total_cost(&ends, &g) + 1e-3);
    }

    #[test]
    fn isolated_horizontal_strokes_are_drawn_left_to_right() {
        // hai dòng chữ ngang xa nhau, khai báo theo chiều phải → trái
        let ends = vec![
            (PointW::new(300.0, 100.0, 1.0), PointW::new(100.0, 100.0, 1.0)),
            (PointW::new(300.0, 400.0, 1.0), PointW::new(100.0, 400.0, 1.0)),
        ];
        let seq = two_opt(&ends, greedy(&ends));
        for (i, rev) in seq {
            let (s, t) = if rev { (ends[i].1, ends[i].0) } else { ends[i] };
            assert!(s.x < t.x, "nét {i} vẽ từ phải sang trái");
        }
    }
}
