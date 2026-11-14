//! Rút xương & dò nét: Zhang–Suen, distance transform, đồ thị xương → nét dài nối qua giao điểm.

use crate::binary::{Binary, neighbor_offsets};
use std::collections::HashSet;

/// Zhang–Suen thinning: xương 1px ở tâm nét. Chỉ duyệt các điểm còn bật (nhanh với ảnh thưa).
pub fn zhang_suen(src: &Binary) -> Binary {
    let mut img = src.clone();
    let off = neighbor_offsets(img.stride);
    let mut live = img.ones();
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut del = Vec::new();
            for &i in &live {
                if img.data[i] == 0 {
                    continue;
                }
                let n = off.map(|o| img.data[(i as isize + o) as usize] != 0);
                let b = n.iter().filter(|v| **v).count();
                if !(2..=6).contains(&b) {
                    continue;
                }
                let a = (0..8).filter(|&k| !n[k] && n[(k + 1) % 8]).count();
                if a != 1 {
                    continue;
                }
                let (p2, p4, p6, p8) = (n[0], n[2], n[4], n[6]);
                let ok = if pass == 0 {
                    !(p2 && p4 && p6) && !(p4 && p6 && p8)
                } else {
                    !(p2 && p4 && p8) && !(p2 && p6 && p8)
                };
                if ok {
                    del.push(i);
                }
            }
            changed |= !del.is_empty();
            for i in del {
                img.data[i] = 0;
            }
        }
        live.retain(|&i| img.data[i] != 0);
        if !changed {
            break;
        }
    }
    img
}

/// Guo–Hall thinning (1989). Khác Zhang–Suen ở chỗ **giữ được dải chéo 45°**: Zhang–Suen gặm đối xứng
/// hai phía dải chéo tới khi còn 2px rồi xoá sạch (lỗi gặp ở tia mặt trời ảnh mẫu a08).
pub fn guo_hall(src: &Binary) -> Binary {
    let mut img = src.clone();
    let off = neighbor_offsets(img.stride);
    let mut live = img.ones();
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut del = Vec::new();
            for &i in &live {
                if img.data[i] == 0 {
                    continue;
                }
                // p2..p9 = N, NE, E, SE, S, SW, W, NW
                let n = off.map(|o| img.data[(i as isize + o) as usize] != 0);
                let [p2, p3, p4, p5, p6, p7, p8, p9] = n;
                let c = (!p2 && (p3 || p4)) as u8
                    + (!p4 && (p5 || p6)) as u8
                    + (!p6 && (p7 || p8)) as u8
                    + (!p8 && (p9 || p2)) as u8;
                if c != 1 {
                    continue;
                }
                let n1 = (p9 || p2) as u8 + (p3 || p4) as u8 + (p5 || p6) as u8 + (p7 || p8) as u8;
                let n2 = (p2 || p3) as u8 + (p4 || p5) as u8 + (p6 || p7) as u8 + (p8 || p9) as u8;
                let nmin = n1.min(n2);
                if !(2..=3).contains(&nmin) {
                    continue;
                }
                let m = if pass == 0 {
                    (p6 || p7 || !p9) && p8
                } else {
                    (p2 || p3 || !p5) && p4
                };
                if !m {
                    del.push(i);
                }
            }
            changed |= !del.is_empty();
            for i in del {
                img.data[i] = 0;
            }
        }
        live.retain(|&i| img.data[i] != 0);
        if !changed {
            break;
        }
    }
    img
}

/// Khoảng cách (px) từ mỗi điểm bật tới nền gần nhất — chamfer 2 lượt (1, √2). Chỉ số theo mảng đệm.
pub fn distance_transform(b: &Binary) -> Vec<f32> {
    let s = b.stride;
    let rows = b.h + 2;
    let d2 = std::f32::consts::SQRT_2;
    let mut d: Vec<f32> = b.data.iter().map(|v| if *v != 0 { f32::MAX } else { 0.0 }).collect();
    for y in 1..rows - 1 {
        for x in 1..s - 1 {
            let i = y * s + x;
            if d[i] > 0.0 {
                let m = (d[i - 1] + 1.0)
                    .min(d[i - s] + 1.0)
                    .min(d[i - s - 1] + d2)
                    .min(d[i - s + 1] + d2);
                d[i] = d[i].min(m);
            }
        }
    }
    for y in (1..rows - 1).rev() {
        for x in (1..s - 1).rev() {
            let i = y * s + x;
            if d[i] > 0.0 {
                let m = (d[i + 1] + 1.0)
                    .min(d[i + s] + 1.0)
                    .min(d[i + s + 1] + d2)
                    .min(d[i + s - 1] + d2);
                d[i] = d[i].min(m);
            }
        }
    }
    d
}

/// Một nét thô: điểm (x, y, bán kính) theo px không đệm, tâm điểm ảnh.
pub type RawStroke = Vec<[f32; 3]>;

struct Edge {
    a: usize,
    b: usize,
    px: Vec<usize>,
    alive: bool,
}

/// Dò xương thành danh sách nét:
/// 1. Phân loại điểm theo crossing number: 1 = đầu mút, ≥3 = giao điểm (gom cụm), 2 = thân nét.
/// 2. Dò các cạnh giữa hai nút; vòng kín không có nút cũng là một cạnh.
/// 3. Tỉa gai: cạnh cụt ngắn hơn `spur_factor ×` độ dày nét tại giao điểm.
/// 4. Nối cạnh qua giao điểm theo hướng **thẳng nhất** (góc rẽ < `max_turn_deg`) → nét dài, tự nhiên.
pub fn trace_strokes(skel: &Binary, dist: &[f32], spur_factor: f32, max_turn_deg: f32) -> Vec<RawStroke> {
    let off = neighbor_offsets(skel.stride);
    // thứ tự ưu tiên khi đi: 4 hướng trước (tránh "bậc thang"), rồi chéo
    let order = [off[0], off[2], off[4], off[6], off[1], off[3], off[5], off[7]];
    let on = |i: usize| skel.data[i] != 0;
    let pixels = skel.ones();

    // 1) phân loại + gom cụm giao điểm
    let mut node_of = vec![usize::MAX; skel.data.len()];
    let mut nodes: Vec<Vec<usize>> = Vec::new();
    let crossing = |i: usize| {
        let n = off.map(|o| on((i as isize + o) as usize));
        let cn = (0..8).filter(|&k| !n[k] && n[(k + 1) % 8]).count();
        let deg = n.iter().filter(|v| **v).count();
        (cn, deg)
    };
    let mut dots = Vec::new();
    for &i in &pixels {
        let (cn, deg) = crossing(i);
        if deg == 0 {
            dots.push(i);
        } else if cn != 2 && node_of[i] == usize::MAX {
            if cn == 1 {
                node_of[i] = nodes.len();
                nodes.push(vec![i]);
            } else {
                // cụm giao điểm: BFS qua các điểm cũng là giao điểm (cn ≥ 3 hoặc cn = 0)
                let id = nodes.len();
                let mut cluster = vec![i];
                node_of[i] = id;
                let mut k = 0;
                while k < cluster.len() {
                    let c = cluster[k];
                    k += 1;
                    for o in off {
                        let j = (c as isize + o) as usize;
                        if on(j) && node_of[j] == usize::MAX {
                            let (cj, _) = crossing(j);
                            if cj != 2 && cj != 1 {
                                node_of[j] = id;
                                cluster.push(j);
                            }
                        }
                    }
                }
                nodes.push(cluster);
            }
        }
    }

    // 2) dò cạnh
    let mut visited = vec![false; skel.data.len()];
    let mut edges: Vec<Edge> = Vec::new();
    let mut pairs = HashSet::new();
    for (nid, cluster) in nodes.clone().iter().enumerate() {
        for &p in cluster {
            for o in order {
                let q = (p as isize + o) as usize;
                if !on(q) {
                    continue;
                }
                if node_of[q] != usize::MAX {
                    if node_of[q] != nid && pairs.insert((p.min(q), p.max(q))) {
                        edges.push(Edge {
                            a: nid,
                            b: node_of[q],
                            px: vec![p, q],
                            alive: true,
                        });
                    }
                    continue;
                }
                if visited[q] {
                    continue;
                }
                visited[q] = true;
                let mut path = vec![p, q];
                let (mut prev, mut cur) = (p, q);
                let end_node = loop {
                    // ưu tiên kết thúc ở một nút kề (trừ khi vừa rời chính nút xuất phát)
                    let to_node = order.iter().map(|o| (cur as isize + o) as usize).find(|&r| {
                        r != prev && on(r) && node_of[r] != usize::MAX && !(node_of[r] == nid && path.len() <= 2)
                    });
                    if let Some(r) = to_node {
                        path.push(r);
                        break Some(node_of[r]);
                    }
                    let next = order
                        .iter()
                        .map(|o| (cur as isize + o) as usize)
                        .find(|&r| on(r) && node_of[r] == usize::MAX && !visited[r]);
                    match next {
                        Some(r) => {
                            visited[r] = true;
                            path.push(r);
                            prev = cur;
                            cur = r;
                        }
                        None => break None,
                    }
                };
                let b = end_node.unwrap_or_else(|| {
                    // đi vào ngõ cụt không được đánh dấu là đầu mút → tạo nút giả
                    let id = nodes.len();
                    nodes.push(vec![cur]);
                    id
                });
                edges.push(Edge {
                    a: nid,
                    b,
                    px: path,
                    alive: true,
                });
            }
        }
    }
    // vòng kín không có nút (chữ o): bắt đầu từ điểm trên-trái
    for &i in &pixels {
        if node_of[i] == usize::MAX && !visited[i] && !dots.contains(&i) {
            visited[i] = true;
            let mut path = vec![i];
            let mut cur = i;
            while let Some(r) = order
                .iter()
                .map(|o| (cur as isize + o) as usize)
                .find(|&r| on(r) && !visited[r] && node_of[r] == usize::MAX)
            {
                visited[r] = true;
                path.push(r);
                cur = r;
            }
            path.push(i);
            let id = nodes.len();
            nodes.push(vec![i]);
            edges.push(Edge {
                a: id,
                b: id,
                px: path,
                alive: true,
            });
        }
    }

    // 3) tỉa gai
    let degree = |edges: &[Edge], n: usize| {
        edges
            .iter()
            .filter(|e| e.alive)
            .map(|e| (e.a == n) as usize + (e.b == n) as usize)
            .sum::<usize>()
    };
    for _ in 0..2 {
        let degs: Vec<usize> = (0..nodes.len()).map(|n| degree(&edges, n)).collect();
        for e in edges.iter_mut().filter(|e| e.alive && e.a != e.b) {
            let (da, db) = (degs[e.a], degs[e.b]);
            if (da == 1 && db >= 3) || (db == 1 && da >= 3) {
                let junction_px = if da >= 3 { e.px[0] } else { *e.px.last().unwrap() };
                let thick = dist[junction_px].max(1.0);
                if (e.px.len() as f32) < spur_factor * thick {
                    e.alive = false;
                }
            }
        }
    }

    // 4) nối cạnh qua giao điểm
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (k, e) in edges.iter().enumerate().filter(|(_, e)| e.alive) {
        adj[e.a].push(k);
        if e.b != e.a {
            adj[e.b].push(k);
        }
    }
    let xy = |i: usize| {
        let (x, y) = skel.xy(i);
        (x as f32, y as f32)
    };
    let dir = |px: &[usize], at_end: bool| {
        let n = px.len();
        let k = 8.min(n - 1);
        let (a, b) = if at_end {
            (px[n - 1 - k], px[n - 1])
        } else {
            (px[k], px[0])
        };
        let ((ax, ay), (bx, by)) = (xy(a), xy(b));
        let (dx, dy) = (bx - ax, by - ay);
        let l = (dx * dx + dy * dy).sqrt().max(1e-6);
        (dx / l, dy / l)
    };
    let mut used = vec![false; edges.len()];
    let cos_max = max_turn_deg.to_radians().cos();
    // nút xuất phát: đầu mút trước (trên-trái trước), rồi các nút còn lại
    let mut starts: Vec<usize> = (0..nodes.len()).collect();
    starts.sort_by_key(|&n| {
        let d = adj[n].len();
        let (x, y) = xy(nodes[n][0]);
        (if d == 1 { 0 } else { 1 }, (y as i64) * 4 + x as i64)
    });
    let mut out = Vec::new();
    for &start in &starts {
        while let Some(&k) = adj[start].iter().find(|&&k| !used[k]) {
            used[k] = true;
            let e = &edges[k];
            let mut path: Vec<usize> = if e.a == start {
                e.px.clone()
            } else {
                e.px.iter().rev().copied().collect()
            };
            let mut node = if e.a == start { e.b } else { e.a };
            loop {
                let (ix, iy) = dir(&path, true);
                let best = adj[node]
                    .iter()
                    .filter(|&&k| !used[k])
                    .map(|&k| {
                        let e = &edges[k];
                        let fwd = e.a == node;
                        let px: Vec<usize> = if fwd {
                            e.px.clone()
                        } else {
                            e.px.iter().rev().copied().collect()
                        };
                        // hướng đi ra: từ đầu cạnh vào trong (đảo dấu của dir(..., false))
                        let (ox, oy) = dir(&px, false);
                        let cos = ix * -ox + iy * -oy;
                        (k, cos, px, if fwd { e.b } else { e.a })
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1));
                match best {
                    Some((k, cos, px, next)) if cos >= cos_max => {
                        used[k] = true;
                        path.extend_from_slice(&px[1..]);
                        node = next;
                    }
                    _ => break,
                }
            }
            out.push(
                path.iter()
                    .map(|&i| {
                        let (x, y) = xy(i);
                        [x + 0.5, y + 0.5, dist[i].max(1.0)]
                    })
                    .collect::<RawStroke>(),
            );
        }
    }
    for i in dots {
        let (x, y) = xy(i);
        out.push(vec![[x + 0.5, y + 0.5, dist[i].max(1.0)]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thick_line(w: usize, h: usize, f: impl Fn(f32, f32) -> bool) -> Binary {
        Binary::from_fn(w, h, |x, y| f(x as f32 + 0.5, y as f32 + 0.5))
    }

    #[test]
    fn thinning_a_thick_bar_gives_one_centerline() {
        // thanh ngang dày 7px
        let b = thick_line(60, 20, |x, y| (5.0..55.0).contains(&x) && (7.0..14.0).contains(&y));
        let s = zhang_suen(&b);
        // mỗi cột ở giữa chỉ còn đúng 1 điểm
        for x in 15..45 {
            let n = (0..20).filter(|&y| s.get(x, y)).count();
            assert_eq!(n, 1, "cột {x} có {n} điểm");
        }
    }

    #[test]
    fn distance_transform_center_is_half_width() {
        let b = thick_line(40, 20, |x, y| (2.0..38.0).contains(&x) && (5.0..14.0).contains(&y)); // dày 9px
        let d = distance_transform(&b);
        let center = d[b.idx(20, 9)];
        assert!((4.0..=5.5).contains(&center), "tâm = {center}");
    }

    #[test]
    fn cross_becomes_two_straight_strokes() {
        // dấu "+" : hai thanh dày 5px cắt nhau
        let b = thick_line(80, 80, |x, y| {
            ((38.0..43.0).contains(&x) && (5.0..75.0).contains(&y))
                || ((38.0..43.0).contains(&y) && (5.0..75.0).contains(&x))
        });
        let skel = zhang_suen(&b);
        let d = distance_transform(&b);
        let strokes = trace_strokes(&skel, &d, 1.5, 60.0);
        assert_eq!(
            strokes.len(),
            2,
            "kỳ vọng 2 nét thẳng xuyên qua giao điểm, có {}",
            strokes.len()
        );
        for s in &strokes {
            let (a, b) = (s[0], s[s.len() - 1]);
            let len = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            assert!(len > 55.0, "nét quá ngắn: {len}");
        }
    }

    fn longest(strokes: &[RawStroke]) -> f32 {
        strokes
            .iter()
            .map(|s| {
                s.windows(2)
                    .map(|w| ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt())
                    .sum::<f32>()
            })
            .fold(0.0f32, f32::max)
    }

    /// Dải chéo đúng 45° (|x − y| < nửa độ rộng) — trường hợp Zhang–Suen xoá sạch.
    fn diagonal_band(half: f32) -> Binary {
        thick_line(80, 80, |x, y| {
            (x - y).abs() < half && (10.0..70.0).contains(&((x + y) / 2.0))
        })
    }

    #[test]
    fn guo_hall_keeps_45_degree_bands_of_any_width() {
        for half in [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0] {
            let b = diagonal_band(half);
            let s = trace_strokes(&guo_hall(&b), &distance_transform(&b), 1.5, 60.0);
            let l = longest(&s);
            assert!(l > 60.0, "nửa độ rộng {half}: nét dài nhất chỉ {l:.1}px");
        }
    }

    /// Mặt nạ THẬT của một tia mặt trời trong ảnh mẫu a08 (sau khi thu nhỏ về độ phân giải làm việc).
    fn real_ray_a08() -> Binary {
        let rows: Vec<&str> = include_str!("testdata_ray_a08.txt").lines().collect();
        let w = rows.iter().map(|r| r.len()).max().unwrap() + 4;
        Binary::from_fn(w, rows.len() + 4, |x, y| {
            y >= 2 && x >= 2 && rows.get(y - 2).and_then(|r| r.as_bytes().get(x - 2)) == Some(&b'.')
        })
    }

    #[test]
    fn regression_a08_ray_zhang_suen_erases_it_guo_hall_keeps_it() {
        let b = real_ray_a08();
        let d = distance_transform(&b);
        let zs = longest(&trace_strokes(&zhang_suen(&b), &d, 1.5, 60.0));
        let gh = longest(&trace_strokes(&guo_hall(&b), &d, 1.5, 60.0));
        assert!(
            zs < 10.0,
            "Zhang–Suen giờ không lỗi nữa ({zs:.1}px)? → xem lại ADR-0013"
        );
        assert!(gh > 40.0, "Guo–Hall chỉ giữ được {gh:.1}px");
    }

    #[test]
    fn guo_hall_thins_bar_and_cross_like_zhang_suen() {
        let bar = thick_line(60, 20, |x, y| (5.0..55.0).contains(&x) && (7.0..14.0).contains(&y));
        let s = guo_hall(&bar);
        for x in 15..45 {
            assert_eq!((0..20).filter(|&y| s.get(x, y)).count(), 1, "cột {x}");
        }
        let cross = thick_line(80, 80, |x, y| {
            ((38.0..43.0).contains(&x) && (5.0..75.0).contains(&y))
                || ((38.0..43.0).contains(&y) && (5.0..75.0).contains(&x))
        });
        let st = trace_strokes(&guo_hall(&cross), &distance_transform(&cross), 1.5, 60.0);
        assert_eq!(st.len(), 2);
    }

    #[test]
    fn ring_without_nodes_is_one_closed_stroke() {
        let b = thick_line(60, 60, |x, y| {
            let r = ((x - 30.0).powi(2) + (y - 30.0).powi(2)).sqrt();
            (17.0..22.0).contains(&r)
        });
        let skel = zhang_suen(&b);
        let d = distance_transform(&b);
        let strokes = trace_strokes(&skel, &d, 1.5, 60.0);
        assert_eq!(strokes.len(), 1);
        let s = &strokes[0];
        let (a, b) = (s[0], s[s.len() - 1]);
        assert!(
            (a[0] - b[0]).abs() < 1.5 && (a[1] - b[1]).abs() < 1.5,
            "vòng phải khép kín"
        );
    }
}
