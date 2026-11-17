//! Trộn các đoạn giọng vào một rãnh tiếng dài bằng cả video.

/// Rãnh dài `total_ms` ở tần số `rate`; mỗi đoạn `(bắt_đầu_ms, mẫu)` được cộng vào (bão hoà, không tràn số).
pub fn mix(total_ms: f64, rate: u32, clips: &[(f64, &[i16])]) -> Vec<i16> {
    let len = (total_ms.max(0.0) * rate as f64 / 1000.0).ceil() as usize;
    let mut out = vec![0i16; len];
    for (start_ms, pcm) in clips {
        let at = (start_ms.max(0.0) * rate as f64 / 1000.0).round() as usize;
        for (dst, src) in out.iter_mut().skip(at).zip(pcm.iter()) {
            *dst = dst.saturating_add(*src);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn clips_land_at_their_start_and_are_cut_at_the_end() {
        let a = [100i16; 10];
        let b = [i16::MAX; 10];
        let out = super::mix(1.0, 20_000, &[(0.0, &a), (0.25, &b)]);
        assert_eq!(out.len(), 20);
        assert_eq!(out[0], 100);
        assert_eq!(out[5], i16::MAX); // 100 + MAX bão hoà
        assert_eq!(out[12], i16::MAX);
        assert_eq!(out[19], 0);
    }
}
