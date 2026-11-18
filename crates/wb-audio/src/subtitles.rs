//! Phụ đề từ lời thoại + mốc thời gian từng từ (WordBoundary của Azure).
//!
//! Azure trả từng từ *không kèm dấu câu*, nên chữ trên phụ đề lấy từ lời thoại gốc (giữ dấu, hoa/thường)
//! và chỉ lấy *thời điểm* từ Azure: ghép tuần tự từng từ của lời thoại với từ Azure trùng chữ;
//! từ không ghép được (số đọc thành nhiều từ…) thì nội suy theo hai từ kề bên.

use crate::WordMark;
use std::fmt::Write;

/// Một dòng phụ đề.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start_ms: f64,
    pub end_ms: f64,
    /// Tối đa 2 dòng, ngăn bằng `\n`.
    pub text: String,
}

/// Độ dài tối đa một dòng phụ đề (ký tự) — chuẩn phụ đề phổ biến ~42.
const LINE: usize = 42;
/// Một khung phụ đề tối đa 2 dòng.
const CUE: usize = 2 * LINE;
/// Phụ đề hiện thêm sau từ cuối / ít nhất (ms).
const HOLD_MS: f64 = 250.0;
const MIN_MS: f64 = 900.0;

/// Phụ đề cho một đoạn lời thoại (đã bỏ dấu `[n]`), thời gian cộng thêm `offset_ms` (đầu scene).
pub fn cues(spoken: &str, words: &[WordMark], duration_ms: f64, offset_ms: f64) -> Vec<Cue> {
    let tokens: Vec<&str> = spoken.split_whitespace().collect();
    if tokens.is_empty() {
        return vec![];
    }
    let times = align(&tokens, words, duration_ms);
    let mut out: Vec<Cue> = Vec::new();
    for range in chunks(&tokens) {
        let (s, e) = (times[range.start].0, times[range.end - 1].1);
        out.push(Cue {
            start_ms: offset_ms + s,
            end_ms: offset_ms + (e + HOLD_MS).max(s + MIN_MS),
            text: wrap(&tokens[range].join(" ")),
        });
    }
    // không chồng lên khung sau
    for i in 1..out.len() {
        let next = out[i].start_ms;
        let prev = &mut out[i - 1];
        prev.end_ms = prev.end_ms.min(next).max(prev.start_ms + 1.0);
    }
    out
}

/// (bắt đầu, kết thúc) ms của từng từ lời thoại.
fn align(tokens: &[&str], words: &[WordMark], duration_ms: f64) -> Vec<(f64, f64)> {
    let mut t: Vec<Option<(f64, f64)>> = vec![None; tokens.len()];
    let mut j = 0;
    for (i, tok) in tokens.iter().enumerate() {
        let key = norm(tok);
        if key.is_empty() {
            continue;
        }
        // tìm trong vài từ Azure kế tiếp (bỏ qua từ Azure tự thêm, vd. số đọc thành chữ)
        if let Some(k) = (j..words.len().min(j + 6)).find(|&k| norm(&words[k].text) == key) {
            t[i] = Some((words[k].start_ms, words[k].start_ms + words[k].duration_ms));
            j = k + 1;
        }
    }
    // từ không ghép được: chia đều khoảng trống giữa hai mốc đã biết theo số ký tự
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if let Some(x) = t[i] {
            out.push(x);
            i += 1;
            continue;
        }
        let end = (i..tokens.len()).find(|&k| t[k].is_some()).unwrap_or(tokens.len());
        let from = out.last().map_or(0.0, |x| x.1);
        let to = t.get(end).copied().flatten().map_or(duration_ms, |x| x.0).max(from);
        let weights: Vec<f64> = (i..end).map(|k| tokens[k].chars().count() as f64 + 1.0).collect();
        let total: f64 = weights.iter().sum();
        let mut c = from;
        for w in weights {
            let d = (to - from) * w / total;
            out.push((c, c + d));
            c += d;
        }
        i = end;
    }
    out
}

/// Chữ thường, bỏ dấu câu (giữ chữ có dấu tiếng Việt và số).
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Chia lời thoại thành các khung: hết câu (. ! ? …) thì sang khung mới; khung quá dài thì cắt
/// ở dấu phẩy gần giữa, không có thì cắt đều.
fn chunks(tokens: &[&str]) -> Vec<std::ops::Range<usize>> {
    let mut sentences = Vec::new();
    let mut start = 0;
    for (i, t) in tokens.iter().enumerate() {
        if t.ends_with(['.', '!', '?', '…', ':', ';']) || i + 1 == tokens.len() {
            sentences.push(start..i + 1);
            start = i + 1;
        }
    }
    let len = |r: &std::ops::Range<usize>| tokens[r.clone()].iter().map(|t| t.chars().count() + 1).sum::<usize>();
    let mut out = Vec::new();
    let mut stack: Vec<_> = sentences.into_iter().rev().collect();
    while let Some(r) = stack.pop() {
        if len(&r) <= CUE || r.len() < 2 {
            out.push(r);
            continue;
        }
        let mid = (r.start + r.end) / 2;
        // dấu phẩy gần giữa nhất, không thì cắt giữa
        let cut = (r.start..r.end - 1)
            .filter(|&i| tokens[i].ends_with(','))
            .min_by_key(|&i| i.abs_diff(mid))
            .map_or(mid, |i| i + 1);
        stack.push(cut..r.end);
        stack.push(r.start..cut);
    }
    out
}

/// Xuống dòng ở khoảng trắng gần giữa nếu dài hơn một dòng.
fn wrap(text: &str) -> String {
    if text.chars().count() <= LINE {
        return text.to_string();
    }
    let mid = text.chars().count() / 2;
    let cut = text
        .char_indices()
        .filter(|(_, c)| *c == ' ')
        .map(|(b, _)| b)
        .min_by_key(|&b| text[..b].chars().count().abs_diff(mid));
    match cut {
        Some(b) => format!("{}\n{}", &text[..b], &text[b + 1..]),
        None => text.to_string(),
    }
}

/// Định dạng SRT (SubRip).
pub fn to_srt(cues: &[Cue]) -> String {
    let ts = |ms: f64| {
        let ms = ms.max(0.0).round() as u64;
        format!(
            "{:02}:{:02}:{:02},{:03}",
            ms / 3_600_000,
            ms / 60_000 % 60,
            ms / 1000 % 60,
            ms % 1000
        )
    };
    let mut s = String::new();
    for (i, c) in cues.iter().enumerate() {
        let _ = write!(s, "{}\n{} --> {}\n{}\n\n", i + 1, ts(c.start_ms), ts(c.end_ms), c.text);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64) -> WordMark {
        WordMark {
            text: text.into(),
            start_ms: start,
            duration_ms: 200.0,
        }
    }

    #[test]
    fn cues_follow_sentences_keep_punctuation_and_use_word_times() {
        let words = [
            w("Xin", 100.0),
            w("chào", 300.0),
            w("các", 500.0),
            w("bạn", 700.0),
            // "2" được Azure đọc thành "hai"
            w("hôm", 1200.0),
            w("nay", 1400.0),
            w("học", 1600.0),
            w("bài", 1800.0),
            w("hai", 2000.0),
        ];
        let c = cues("Xin chào các bạn. Hôm nay học bài 2!", &words, 2500.0, 10_000.0);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].text, "Xin chào các bạn.");
        assert_eq!(c[0].start_ms, 10_100.0);
        assert_eq!(c[0].end_ms, 11_150.0); // từ cuối kết thúc 900 ms + giữ 250 ms
        assert_eq!(c[1].text, "Hôm nay học bài 2!");
        assert_eq!(c[1].start_ms, 11_200.0);
        // "2" không ghép được → nội suy tới hết đoạn
        assert!(c[1].end_ms >= 12_000.0);
    }

    #[test]
    fn long_sentences_split_at_commas_and_wrap_to_two_lines() {
        let text = "Mọi ý tưởng lớn đều bắt đầu từ một bóng đèn nhỏ, rồi lớn dần lên theo thời gian và công sức của chúng ta mỗi ngày";
        let c = cues(text, &[], 6000.0, 0.0);
        assert!(c.len() >= 2, "{c:?}");
        assert!(c[0].text.ends_with("nhỏ,"), "{:?}", c[0].text);
        for cue in &c {
            assert!(cue.text.lines().count() <= 2);
            assert!(
                cue.text.lines().all(|l| l.chars().count() <= LINE + 10),
                "{:?}",
                cue.text
            );
            assert!(cue.end_ms > cue.start_ms);
        }
        assert!(c.windows(2).all(|p| p[0].end_ms <= p[1].start_ms));
    }

    #[test]
    fn srt_format() {
        let s = to_srt(&[Cue {
            start_ms: 3_723_004.0,
            end_ms: 3_724_500.0,
            text: "a\nb".into(),
        }]);
        assert_eq!(s, "1\n01:02:03,004 --> 01:02:04,500\na\nb\n\n");
    }
}
