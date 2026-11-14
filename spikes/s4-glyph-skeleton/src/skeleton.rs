//! Thuật toán ảnh nhị phân cho spike: thành phần liên thông, Zhang–Suen, distance transform,
//! dò xương thành nét, RDP. (Bản chính thức sẽ nằm trong `wb-vision` ở Giai đoạn 1, kèm unit test.)

use tiny_skia::Pixmap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
    /// Bán kính nét tại điểm này (px)
    pub w: f32,
}
impl Pt {
    pub fn dist(&self, o: &Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    pub fn lerp(&self, o: &Pt, t: f32) -> Pt {
        Pt {
            x: self.x + (o.x - self.x) * t,
            y: self.y + (o.y - self.y) * t,
            w: self.w + (o.w - self.w) * t,
        }
    }
}

#[derive(Clone)]
pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub data: Vec<bool>,
}

pub struct Comp {
    pub area: usize,
    pub x_min: usize,
    pub y_min: usize,
    pub y_max: usize,
    pixels: Vec<usize>,
}

const N8: [(isize, isize); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

impl Bitmap {
    pub fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            data: vec![false; w * h],
        }
    }
    pub fn from_alpha(pm: &Pixmap, threshold: u8) -> Self {
        let data = pm.pixels().iter().map(|p| p.alpha() >= threshold).collect();
        Self {
            w: pm.width() as usize,
            h: pm.height() as usize,
            data,
        }
    }
    #[inline]
    pub fn get(&self, x: isize, y: isize) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.data[y as usize * self.w + x as usize]
    }
    pub fn or_assign(&mut self, o: &Bitmap) {
        for (a, b) in self.data.iter_mut().zip(&o.data) {
            *a |= *b;
        }
    }
    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| **v).count()
    }
    pub fn and_count(&self, o: &Bitmap) -> usize {
        self.data.iter().zip(&o.data).filter(|(a, b)| **a && **b).count()
    }
    /// Thành phần liên thông 8 hướng.
    pub fn components(&self) -> Vec<Comp> {
        let mut seen = vec![false; self.data.len()];
        let mut out = Vec::new();
        for start in 0..self.data.len() {
            if !self.data[start] || seen[start] {
                continue;
            }
            let mut stack = vec![start];
            seen[start] = true;
            let mut c = Comp {
                area: 0,
                x_min: usize::MAX,
                y_min: usize::MAX,
                y_max: 0,
                pixels: Vec::new(),
            };
            while let Some(i) = stack.pop() {
                let (x, y) = (i % self.w, i / self.w);
                c.area += 1;
                c.x_min = c.x_min.min(x);
                c.y_min = c.y_min.min(y);
                c.y_max = c.y_max.max(y);
                c.pixels.push(i);
                for (dx, dy) in N8 {
                    let (nx, ny) = (x as isize + dx, y as isize + dy);
                    if self.get(nx, ny) {
                        let j = ny as usize * self.w + nx as usize;
                        if !seen[j] {
                            seen[j] = true;
                            stack.push(j);
                        }
                    }
                }
            }
            out.push(c);
        }
        out
    }
    /// Tâm và bán kính lớn nhất của một thành phần (dùng cho chấm nhỏ: dấu chấm i, dấu nặng).
    pub fn dot_of(&self, c: &Comp, dist: &[f32]) -> Pt {
        let n = c.pixels.len().max(1) as f32;
        let (sx, sy) = c.pixels.iter().fold((0.0, 0.0), |(sx, sy), &i| {
            (sx + (i % self.w) as f32, sy + (i / self.w) as f32)
        });
        let w = c.pixels.iter().map(|&i| dist[i]).fold(1.0f32, f32::max);
        Pt {
            x: sx / n + 0.5,
            y: sy / n + 0.5,
            w,
        }
    }
    pub fn only_label(&self, c: &Comp) -> Bitmap {
        let mut b = Bitmap::new(self.w, self.h);
        for &i in &c.pixels {
            b.data[i] = true;
        }
        b
    }
    fn neighbors(&self, x: usize, y: usize) -> [bool; 8] {
        let mut n = [false; 8];
        for (k, (dx, dy)) in N8.iter().enumerate() {
            n[k] = self.get(x as isize + dx, y as isize + dy);
        }
        n
    }
}

/// Zhang–Suen thinning: xương 1px ở tâm nét.
pub fn zhang_suen(src: &Bitmap) -> Bitmap {
    let mut img = src.clone();
    loop {
        let mut changed = false;
        for pass in 0..2 {
            let mut del = Vec::new();
            for y in 0..img.h {
                for x in 0..img.w {
                    if !img.data[y * img.w + x] {
                        continue;
                    }
                    // P2..P9 theo thứ tự N8: 0=N,1=NE,2=E,3=SE,4=S,5=SW,6=W,7=NW
                    let n = img.neighbors(x, y);
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
                        del.push(y * img.w + x);
                    }
                }
            }
            changed |= !del.is_empty();
            for i in del {
                img.data[i] = false;
            }
        }
        if !changed {
            break;
        }
    }
    img
}

/// Khoảng cách (px) từ mỗi điểm chữ tới nền gần nhất — xấp xỉ chamfer 2 lượt (1, √2).
pub fn distance_transform(b: &Bitmap) -> Vec<f32> {
    let (w, h) = (b.w, b.h);
    let inf = 1e9f32;
    let mut d: Vec<f32> = b.data.iter().map(|v| if *v { inf } else { 0.0 }).collect();
    let s2 = std::f32::consts::SQRT_2;
    let at = |d: &Vec<f32>, x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
            0.0
        } else {
            d[y as usize * w + x as usize]
        }
    };
    for y in 0..h as isize {
        for x in 0..w as isize {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let m = (at(&d, x - 1, y) + 1.0)
                .min(at(&d, x, y - 1) + 1.0)
                .min(at(&d, x - 1, y - 1) + s2)
                .min(at(&d, x + 1, y - 1) + s2);
            d[i] = d[i].min(m);
        }
    }
    for y in (0..h as isize).rev() {
        for x in (0..w as isize).rev() {
            let i = y as usize * w + x as usize;
            if d[i] == 0.0 {
                continue;
            }
            let m = (at(&d, x + 1, y) + 1.0)
                .min(at(&d, x, y + 1) + 1.0)
                .min(at(&d, x + 1, y + 1) + s2)
                .min(at(&d, x - 1, y + 1) + s2);
            d[i] = d[i].min(m);
        }
    }
    d
}

/// Số lần chuyển 0→1 quanh điểm (crossing number): 1 = đầu mút, 2 = giữa nét, ≥3 = giao điểm.
fn crossing(skel: &Bitmap, x: usize, y: usize) -> usize {
    let n = skel.neighbors(x, y);
    let cn = (0..8).filter(|&k| !n[k] && n[(k + 1) % 8]).count();
    if cn == 0 && n.iter().any(|v| *v) { 1 } else { cn }
}

/// Dò xương thành danh sách nét (đường gấp khúc) — mỗi cạnh giữa hai nút là một nét; vòng kín cũng là một nét.
/// Tỉa gai: nhánh cụt ngắn hơn ~1.5× độ dày nét bị bỏ.
pub fn trace_strokes(skel: &Bitmap, dist: &[f32]) -> Vec<Vec<Pt>> {
    let (w, h) = (skel.w, skel.h);
    let mut is_node = vec![false; w * h];
    let mut any = false;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if skel.data[i] {
                any = true;
                let c = crossing(skel, x, y);
                is_node[i] = c != 2;
            }
        }
    }
    if !any {
        return Vec::new();
    }
    let mut visited = vec![false; w * h];
    let pt = |i: usize| Pt {
        x: (i % w) as f32 + 0.5,
        y: (i / w) as f32 + 0.5,
        w: dist[i].max(1.0),
    };
    let nbrs = |i: usize| -> Vec<usize> {
        let (x, y) = ((i % w) as isize, (i / w) as isize);
        // ưu tiên 4 hướng trước để tránh "bậc thang"
        let order = [0usize, 2, 4, 6, 1, 3, 5, 7];
        order
            .iter()
            .filter_map(|&k| {
                let (nx, ny) = (x + N8[k].0, y + N8[k].1);
                skel.get(nx, ny).then(|| ny as usize * w + nx as usize)
            })
            .collect()
    };
    let mut strokes: Vec<(Vec<Pt>, bool)> = Vec::new(); // (điểm, có đầu mút tự do)
    let mut used_pairs = std::collections::HashSet::new();
    for start in 0..w * h {
        if !skel.data[start] || !is_node[start] {
            continue;
        }
        for first in nbrs(start) {
            if (!is_node[first] && visited[first]) || used_pairs.contains(&(start.min(first), start.max(first))) {
                continue;
            }
            used_pairs.insert((start.min(first), start.max(first)));
            let mut path = vec![start, first];
            let mut prev = start;
            let mut cur = first;
            while !is_node[cur] {
                visited[cur] = true;
                let next = nbrs(cur)
                    .into_iter()
                    .find(|&n| n != prev && (!visited[n] || is_node[n]) && !path[..path.len() - 1].contains(&n));
                match next {
                    Some(n) => {
                        path.push(n);
                        prev = cur;
                        cur = n;
                    }
                    None => break,
                }
            }
            if path.len() >= 2 {
                used_pairs.insert((prev.min(cur), prev.max(cur)));
                let free_end = crossing(skel, start % w, start / w) == 1 || crossing(skel, cur % w, cur / w) == 1;
                strokes.push((path.iter().map(|&i| pt(i)).collect(), free_end));
            }
        }
    }
    // Vòng kín không có nút (chữ o): chọn điểm trên-trái làm điểm bắt đầu
    for start in 0..w * h {
        if skel.data[start] && !is_node[start] && !visited[start] {
            let mut path = vec![start];
            visited[start] = true;
            let mut cur = start;
            while let Some(n) = nbrs(cur).into_iter().find(|&n| !visited[n]) {
                visited[n] = true;
                path.push(n);
                cur = n;
            }
            path.push(start);
            strokes.push((path.iter().map(|&i| pt(i)).collect(), false));
        }
    }
    // Tỉa gai
    let mut out: Vec<Vec<Pt>> = Vec::new();
    for (s, free) in strokes {
        let len: f32 = s.windows(2).map(|p| p[0].dist(&p[1])).sum();
        let thick = s.iter().map(|p| p.w).fold(0.0f32, f32::max);
        if free && len < 1.5 * thick && s.len() > 1 {
            continue;
        }
        out.push(s);
    }
    out
}

/// Ramer–Douglas–Peucker: bỏ điểm thừa, giữ hình dạng trong sai số `eps` px.
pub fn rdp(pts: &[Pt], eps: f32) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (a, b) = (pts[0], pts[pts.len() - 1]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len = (dx * dx + dy * dy).sqrt();
    let mut best = (0, 0.0f32);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = if len < 1e-6 {
            p.dist(&a)
        } else {
            ((p.x - a.x) * dy - (p.y - a.y) * dx).abs() / len
        };
        if d > best.1 {
            best = (i, d);
        }
    }
    if best.1 > eps {
        let mut l = rdp(&pts[..=best.0], eps);
        let r = rdp(&pts[best.0..], eps);
        l.pop();
        l.extend(r);
        l
    } else {
        vec![a, b]
    }
}
