//! Tiếng bút tự sinh (không dùng file âm thanh của bên thứ ba): tiếng ồn qua bộ lọc dải tần
//! giống ngòi bút cọ giấy, to/nhỏ theo vận tốc tay (minimum-jerk — cùng mô hình với chuyển động bút),
//! thêm "hạt" giấy ngẫu nhiên. Nét tô bằng cọ: trầm và nhẹ hơn. Tất định theo `seed`.

use crate::SAMPLE_RATE;

/// Một nét đang được vẽ trong khoảng [t0, t1) ms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenStroke {
    pub t0_ms: f64,
    pub t1_ms: f64,
    /// Nét tô màu bằng cọ (không phải nét bút).
    pub brush: bool,
}

/// Mức to nhất của tiếng bút khi `volume` = 1 (tỉ lệ biên độ tối đa) — nhỏ hơn giọng đọc nhiều.
const PEAK: f32 = 0.16;
/// Thời gian tăng/giảm tiếng ở đầu/cuối nét (ms) — tránh tiếng "tách".
const RAMP_MS: f64 = 8.0;

/// Rãnh tiếng bút dài `total_ms` (mono, [`SAMPLE_RATE`]). `volume` 0 = im lặng.
pub fn pen_track(total_ms: f64, strokes: &[PenStroke], seed: u32, volume: f32) -> Vec<i16> {
    let rate = SAMPLE_RATE as f64;
    let len = (total_ms.max(0.0) * rate / 1000.0).ceil() as usize;
    let mut out = vec![0i16; len];
    if volume <= 0.0 {
        return out;
    }
    let gain = PEAK * volume.min(2.0) * i16::MAX as f32;
    for (k, s) in strokes.iter().enumerate() {
        let a = (s.t0_ms.max(0.0) * rate / 1000.0) as usize;
        let b = ((s.t1_ms * rate / 1000.0) as usize).min(len);
        if b <= a {
            continue;
        }
        let n = b - a;
        let ramp = (RAMP_MS * rate / 1000.0).max(1.0);
        let mut rng = Rng::new(seed ^ (k as u32).wrapping_mul(0x9E37_79B9));
        // bút: dải ~3 kHz (sột soạt); cọ: thấp hơn ~1,2 kHz và nhỏ hơn
        let (f0, q, level) = if s.brush {
            (1200.0, 0.6, 0.45)
        } else {
            (3000.0, 0.8, 1.0)
        };
        let mut band = Biquad::band_pass(f0, q, rate);
        // "hạt" giấy: biên độ dao động chậm ngẫu nhiên (~25 Hz)
        let mut grain = Biquad::low_pass(25.0, 0.7, rate);
        for i in 0..n {
            let tau = i as f64 / n as f64;
            // vận tốc minimum-jerk chuẩn hoá về [0, 1] (đỉnh 1,875 ở giữa nét)
            let v = 30.0 * tau * tau * (1.0 - tau) * (1.0 - tau) / 1.875;
            let env = (i as f64 / ramp).min((n - i) as f64 / ramp).min(1.0);
            let g = 1.0 + 1.8 * grain.run(rng.next());
            let x = band.run(rng.next()) * (0.3 + 0.7 * v as f32) * env as f32 * g.max(0.2) * level;
            let o = &mut out[a + i];
            *o = o.saturating_add((x * gain).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }
    out
}

/// Bộ lọc bậc 2 (công thức RBJ "Audio EQ Cookbook").
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}

impl Biquad {
    fn band_pass(f0: f64, q: f64, rate: f64) -> Self {
        let w = std::f64::consts::TAU * f0 / rate;
        let alpha = w.sin() / (2.0 * q);
        Self::new([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * w.cos(), 1.0 - alpha])
    }

    fn low_pass(f0: f64, q: f64, rate: f64) -> Self {
        let w = std::f64::consts::TAU * f0 / rate;
        let (c, alpha) = (w.cos(), w.sin() / (2.0 * q));
        Self::new(
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        )
    }

    fn new(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b: b.map(|x| (x / a[0]) as f32),
            a: [(a[1] / a[0]) as f32, (a[2] / a[0]) as f32],
            z: [0.0; 2],
        }
    }

    /// Dạng chuyển vị II.
    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// xorshift32 → tiếng ồn trắng [-1, 1].
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(s: &[i16]) -> f64 {
        (s.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / s.len().max(1) as f64).sqrt()
    }

    #[test]
    fn sound_only_while_drawing_and_deterministic() {
        let strokes = [
            PenStroke {
                t0_ms: 100.0,
                t1_ms: 600.0,
                brush: false,
            },
            PenStroke {
                t0_ms: 800.0,
                t1_ms: 1300.0,
                brush: true,
            },
        ];
        let t = pen_track(1500.0, &strokes, 7, 1.0);
        assert_eq!(t.len(), 36_000);
        let at = |ms: f64| (ms * 24.0) as usize;
        assert!(t[..at(100.0)].iter().all(|&x| x == 0), "im lặng trước nét đầu");
        assert!(t[at(600.0)..at(800.0)].iter().all(|&x| x == 0), "im lặng khi nhấc bút");
        let pen = rms(&t[at(250.0)..at(450.0)]);
        let brush = rms(&t[at(950.0)..at(1150.0)]);
        assert!(pen > 300.0, "bút quá nhỏ: {pen}");
        assert!(brush < pen, "cọ phải nhỏ hơn bút: {brush} / {pen}");
        // to nhất ở giữa nét (vận tốc lớn nhất), nhỏ ở hai đầu
        assert!(rms(&t[at(330.0)..at(370.0)]) > rms(&t[at(110.0)..at(150.0)]));
        // tất định, và volume 0 = im lặng
        assert_eq!(t, pen_track(1500.0, &strokes, 7, 1.0));
        assert_ne!(t, pen_track(1500.0, &strokes, 8, 1.0));
        assert!(pen_track(1500.0, &strokes, 7, 0.0).iter().all(|&x| x == 0));
    }
}
