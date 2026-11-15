//! **Tô nét (brush reveal)**: sinh đường cọ zíc-zắc phủ kín một vùng — kế hoạch File 02 §2.3.
//!
//! Lớp màu bị che; cọ lia qua lia lại, tới đâu ảnh hiện ra tới đó. Cách sinh đường:
//! 1. Xoay hệ toạ độ theo góc `angle` (hàng cọ hơi nghiêng trông tự nhiên hơn hàng ngang tuyệt đối).
//! 2. Chia vùng thành các **hàng** cách nhau `1,4·r` (cọ bán kính `r` phủ ±r → các hàng chồng nhau, không hở).
//!    Mỗi điểm của vùng thuộc đúng một hàng → mỗi hàng là các **đoạn** liên tục (khe hở < r được lấp).
//! 3. Nối đoạn của hàng sau vào đoạn chồng lên nó ở hàng trước → **chuỗi** (kiểu boustrophedon):
//!    cọ đi trái→phải rồi phải→trái… Vùng lõm / tách nhánh tự sinh chuỗi mới, nên cọ không quét xuyên
//!    qua chỗ trống quá xa.
//!
//! Đầu vào là mặt nạ dạng `&[bool]` (không phụ thuộc `wb-vision`); toạ độ ra theo px của mặt nạ.

use wb_model::PointW;

/// Tham số tô.
#[derive(Debug, Clone, Copy)]
pub struct HatchParams {
    /// Bán kính cọ (px mặt nạ).
    pub radius: f32,
    /// Góc hàng cọ (độ, dương = đi xuống khi sang phải).
    pub angle_deg: f32,
    /// Khoảng cách hàng / bán kính (≤ 2 để không hở; 1,4 = chồng 30%).
    pub row_spacing: f32,
}

impl HatchParams {
    pub fn new(radius: f32) -> Self {
        Self {
            radius: radius.max(1.0),
            angle_deg: -12.0,
            row_spacing: 1.4,
        }
    }
}

/// Sinh các nét cọ phủ kín mặt nạ `mask` (`w×h`, hàng nối tiếp). Thứ tự: trên → dưới.
pub fn hatch(mask: &[bool], w: usize, h: usize, p: &HatchParams) -> Vec<Vec<PointW>> {
    assert_eq!(mask.len(), w * h);
    let r = p.radius;
    let (sin, cos) = p.angle_deg.to_radians().sin_cos();
    // toạ độ xoay: u dọc hàng, v vuông góc
    let to_uv = |x: f32, y: f32| (x * cos + y * sin, -x * sin + y * cos);
    let to_xy = |u: f32, v: f32| (u * cos - v * sin, u * sin + v * cos);

    let (mut u0, mut u1, mut v0, mut v1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                let (u, v) = to_uv(x as f32 + 0.5, y as f32 + 0.5);
                u0 = u0.min(u);
                u1 = u1.max(u);
                v0 = v0.min(v);
                v1 = v1.max(v);
            }
        }
    }
    if u0 > u1 {
        return Vec::new();
    }
    let spacing = (r * p.row_spacing).max(1.0);
    let rows = ((v1 - v0) / spacing).floor() as usize + 1;
    let cols = (u1 - u0).ceil() as usize + 2;
    let mut grid = vec![false; rows * cols];
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] {
                let (u, v) = to_uv(x as f32 + 0.5, y as f32 + 0.5);
                let j = (((v - v0) / spacing).floor() as usize).min(rows - 1);
                let c = ((u - u0).floor() as usize).min(cols - 1);
                grid[j * cols + c] = true;
            }
        }
    }

    // đoạn của từng hàng (khe < r được lấp)
    let gap = r.ceil() as usize;
    let mut row_spans: Vec<Vec<(f32, f32)>> = Vec::with_capacity(rows);
    for j in 0..rows {
        let line = &grid[j * cols..(j + 1) * cols];
        let mut spans: Vec<(usize, usize)> = Vec::new();
        let mut c = 0;
        while c < cols {
            if !line[c] {
                c += 1;
                continue;
            }
            let a = c;
            while c < cols && line[c] {
                c += 1;
            }
            match spans.last_mut() {
                Some(last) if a - last.1 <= gap => last.1 = c,
                _ => spans.push((a, c)),
            }
        }
        row_spans.push(spans.into_iter().map(|(a, b)| (u0 + a as f32, u0 + b as f32)).collect());
    }

    // nối thành chuỗi
    struct Chain {
        row: usize,
        spans: Vec<(usize, f32, f32)>,
    }
    let mut open: Vec<Chain> = Vec::new();
    let mut done: Vec<Chain> = Vec::new();
    for (j, spans) in row_spans.iter().enumerate() {
        // chuỗi không nối tiếp được ở hàng này → đóng
        let (keep, close): (Vec<Chain>, Vec<Chain>) = open.into_iter().partition(|c| c.row + 1 == j);
        done.extend(close);
        let mut prev = keep;
        let mut next: Vec<Chain> = Vec::new();
        for &(a, b) in spans {
            let hit = prev.iter().position(|c| {
                let (_, pa, pb) = *c.spans.last().unwrap();
                a <= pb + r && b >= pa - r
            });
            match hit {
                Some(i) => {
                    let mut c = prev.swap_remove(i);
                    c.row = j;
                    c.spans.push((j, a, b));
                    next.push(c);
                }
                None => next.push(Chain {
                    row: j,
                    spans: vec![(j, a, b)],
                }),
            }
        }
        done.extend(prev);
        open = next;
    }
    done.extend(open);
    done.sort_by(|a, b| {
        let (ja, ua, _) = a.spans[0];
        let (jb, ub, _) = b.spans[0];
        ja.cmp(&jb).then(ua.total_cmp(&ub))
    });

    // chuỗi → đường cọ (thu đầu đoạn vào 0,4r: đĩa cọ vẫn phủ tới mép đoạn)
    let inset = 0.4 * r;
    done.into_iter()
        .map(|c| {
            let mut pts = Vec::with_capacity(c.spans.len() * 2);
            for (k, &(j, a, b)) in c.spans.iter().enumerate() {
                let v = v0 + (j as f32 + 0.5) * spacing;
                let (a, b) = if b - a > 2.0 * inset {
                    (a + inset, b - inset)
                } else {
                    let m = (a + b) / 2.0;
                    (m, m)
                };
                let (s, e) = if k % 2 == 0 { (a, b) } else { (b, a) };
                for u in [s, e] {
                    let (x, y) = to_xy(u, v);
                    let pt = PointW::new(x, y, r);
                    if pts.last() != Some(&pt) {
                        pts.push(pt);
                    }
                }
            }
            pts
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tỉ lệ điểm mặt nạ nằm trong bán kính cọ của ít nhất một đoạn.
    fn covered(mask: &[bool], w: usize, strokes: &[Vec<PointW>]) -> f32 {
        let mut hit = 0;
        let mut total = 0;
        for (i, &m) in mask.iter().enumerate() {
            if !m {
                continue;
            }
            total += 1;
            let (px, py) = ((i % w) as f32 + 0.5, (i / w) as f32 + 0.5);
            let ok = strokes.iter().any(|s| {
                s.windows(2)
                    .any(|seg| dist_seg(px, py, seg[0], seg[1]) <= seg[0].w + 0.01)
                    || (s.len() == 1 && ((px - s[0].x).powi(2) + (py - s[0].y).powi(2)).sqrt() <= s[0].w)
            });
            if ok {
                hit += 1;
            }
        }
        hit as f32 / total.max(1) as f32
    }

    fn dist_seg(px: f32, py: f32, a: PointW, b: PointW) -> f32 {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 {
            (((px - a.x) * dx + (py - a.y) * dy) / l2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        ((px - a.x - t * dx).powi(2) + (py - a.y - t * dy).powi(2)).sqrt()
    }

    #[test]
    fn rectangle_is_one_zigzag_and_fully_covered() {
        let (w, h) = (300, 200);
        let mask: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                (40..260).contains(&x) && (30..170).contains(&y)
            })
            .collect();
        let s = hatch(&mask, w, h, &HatchParams::new(12.0));
        assert_eq!(s.len(), 1, "hình chữ nhật phải là một đường zíc-zắc liền");
        assert!(s[0].len() > 10);
        let c = covered(&mask, w, &s);
        assert!(c > 0.995, "độ phủ {c}");
    }

    #[test]
    fn separate_blobs_get_separate_strokes() {
        let (w, h) = (300, 100);
        let mask: Vec<bool> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                (20..80).contains(&y) && ((10..90).contains(&x) || (200..290).contains(&x))
            })
            .collect();
        let s = hatch(&mask, w, h, &HatchParams::new(8.0));
        assert!(s.len() >= 2);
        assert!(covered(&mask, w, &s) > 0.99);
    }
}
