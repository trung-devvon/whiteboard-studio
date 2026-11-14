//! Ảnh nhị phân có **viền đệm 1px** (luôn là nền) → truy cập 8 láng giềng không cần kiểm tra biên.

/// Ảnh nhị phân `w×h` lưu trong mảng `(w+2)×(h+2)`.
#[derive(Clone, Debug)]
pub struct Binary {
    pub w: usize,
    pub h: usize,
    /// Độ rộng hàng có đệm (= w + 2).
    pub stride: usize,
    pub data: Vec<u8>,
}

/// Độ lệch chỉ số của 8 láng giềng theo thứ tự P2..P9 (N, NE, E, SE, S, SW, W, NW).
pub fn neighbor_offsets(stride: usize) -> [isize; 8] {
    let s = stride as isize;
    [-s, -s + 1, 1, s + 1, s, s - 1, -1, -s - 1]
}

impl Binary {
    pub fn new(w: usize, h: usize) -> Self {
        let stride = w + 2;
        Self {
            w,
            h,
            stride,
            data: vec![0; stride * (h + 2)],
        }
    }

    pub fn from_fn(w: usize, h: usize, f: impl Fn(usize, usize) -> bool) -> Self {
        let mut b = Self::new(w, h);
        for y in 0..h {
            for x in 0..w {
                if f(x, y) {
                    let i = b.idx(x, y);
                    b.data[i] = 1;
                }
            }
        }
        b
    }

    /// Chỉ số trong mảng đệm của điểm (x, y) không đệm.
    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        (y + 1) * self.stride + x + 1
    }

    /// Toạ độ (x, y) không đệm từ chỉ số đệm.
    #[inline]
    pub fn xy(&self, i: usize) -> (usize, usize) {
        (i % self.stride - 1, i / self.stride - 1)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> bool {
        self.data[self.idx(x, y)] != 0
    }

    pub fn count(&self) -> usize {
        self.data.iter().filter(|v| **v != 0).count()
    }

    /// Chỉ số (đệm) của mọi điểm bật.
    pub fn ones(&self) -> Vec<usize> {
        (0..self.data.len()).filter(|&i| self.data[i] != 0).collect()
    }

    /// Xoá các thành phần liên thông (8 hướng) có diện tích nhỏ hơn `min_area`.
    pub fn remove_small_components(&mut self, min_area: usize) {
        let off = neighbor_offsets(self.stride);
        let mut seen = vec![false; self.data.len()];
        for start in 0..self.data.len() {
            if self.data[start] == 0 || seen[start] {
                continue;
            }
            let mut comp = vec![start];
            seen[start] = true;
            let mut k = 0;
            while k < comp.len() {
                let i = comp[k];
                k += 1;
                for o in off {
                    let j = (i as isize + o) as usize;
                    if self.data[j] != 0 && !seen[j] {
                        seen[j] = true;
                        comp.push(j);
                    }
                }
            }
            if comp.len() < min_area {
                for i in comp {
                    self.data[i] = 0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padding_and_coordinates_roundtrip() {
        let b = Binary::from_fn(4, 3, |x, y| x == 3 && y == 2);
        assert_eq!(b.count(), 1);
        let i = b.ones()[0];
        assert_eq!(b.xy(i), (3, 2));
        assert!(b.get(3, 2));
    }

    #[test]
    fn removes_only_small_components() {
        // một chấm 1px và một đoạn 5px
        let mut b = Binary::from_fn(10, 3, |x, y| (x == 0 && y == 0) || (y == 2 && (3..8).contains(&x)));
        b.remove_small_components(3);
        assert_eq!(b.count(), 5);
        assert!(!b.get(0, 0));
    }
}
