//! Lời thoại → SSML. Dấu `[n]` trong lời thoại = element thứ n của scene bắt đầu vẽ đúng lúc đọc tới đó;
//! mỗi dấu thành một `<bookmark mark="e<n>"/>` để Azure báo lại mốc thời gian.

/// Một đoạn chữ hoặc một dấu trong lời thoại.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece<'a> {
    Text(&'a str),
    Marker(usize),
}

/// Tách lời thoại thành chữ và dấu `[n]` (n ≥ 1). `[abc]` không phải dấu → giữ nguyên là chữ.
pub fn parse(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let marker = after
            .find(']')
            .and_then(|close| after[..close].trim().parse::<usize>().ok().map(|n| (n, close)))
            .filter(|(n, _)| *n >= 1);
        match marker {
            Some((n, close)) => {
                if open > 0 {
                    out.push(Piece::Text(&rest[..open]));
                }
                out.push(Piece::Marker(n));
                rest = &after[close + 1..];
            }
            None => {
                out.push(Piece::Text(&rest[..open + 1]));
                rest = after;
            }
        }
    }
    if !rest.is_empty() {
        out.push(Piece::Text(rest));
    }
    out
}

/// Phần sẽ được đọc (bỏ các dấu `[n]`) — để kiểm tra lời thoại có chữ không.
pub fn spoken(text: &str) -> String {
    parse(text)
        .into_iter()
        .filter_map(|p| match p {
            Piece::Text(t) => Some(t),
            Piece::Marker(_) => None,
        })
        .collect()
}

pub fn bookmark_name(n: usize) -> String {
    format!("e{n}")
}

/// SSML cho Azure: giọng `voice`, tốc độ `rate` (1 = bình thường).
pub fn ssml(text: &str, voice: &str, rate: f32) -> String {
    let lang = voice.get(..5).unwrap_or("vi-VN");
    let pct = ((rate.clamp(0.5, 2.0) - 1.0) * 100.0).round() as i32;
    let mut body = String::new();
    for p in parse(text) {
        match p {
            Piece::Text(t) => body.push_str(&escape(t)),
            Piece::Marker(n) => body.push_str(&format!("<bookmark mark=\"{}\"/>", bookmark_name(n))),
        }
    }
    format!(
        "<speak version=\"1.0\" xmlns=\"http://www.w3.org/2001/10/synthesis\" xml:lang=\"{lang}\">\
         <voice name=\"{}\"><prosody rate=\"{pct:+}%\">{body}</prosody></voice></speak>",
        escape(voice)
    )
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Khoá đệm của một lần đọc: đổi chữ, dấu, giọng hay tốc độ → khoá mới → đọc lại.
pub fn cache_key(text: &str, voice: &str, rate: f32) -> String {
    // FNV-1a 64-bit — ổn định giữa các lần chạy (khác DefaultHasher)
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in format!("v1|{voice}|{rate:.3}|{}", text.trim()).bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_become_bookmarks_and_text_is_escaped() {
        assert_eq!(
            parse("Đây là [1] bóng đèn [2]và [x] nhà"),
            vec![
                Piece::Text("Đây là "),
                Piece::Marker(1),
                Piece::Text(" bóng đèn "),
                Piece::Marker(2),
                Piece::Text("và ["),
                Piece::Text("x] nhà"),
            ]
        );
        assert_eq!(spoken("A [1]B [0] C"), "A B [0] C");
        let s = ssml("Tôm & cua [1]<ngon>", "vi-VN-HoaiMyNeural", 1.2);
        assert!(s.contains("xml:lang=\"vi-VN\""));
        assert!(s.contains("rate=\"+20%\""));
        assert!(s.contains("Tôm &amp; cua <bookmark mark=\"e1\"/>&lt;ngon&gt;"));
        assert!(ssml("a", "vi-VN-NamMinhNeural", 0.9).contains("rate=\"-10%\""));
    }

    #[test]
    fn cache_key_changes_with_text_voice_and_rate() {
        let k = cache_key("xin chào [1]", "vi-VN-HoaiMyNeural", 1.0);
        assert_eq!(k, cache_key("  xin chào [1] ", "vi-VN-HoaiMyNeural", 1.0));
        assert_ne!(k, cache_key("xin chào [2]", "vi-VN-HoaiMyNeural", 1.0));
        assert_ne!(k, cache_key("xin chào [1]", "vi-VN-NamMinhNeural", 1.0));
        assert_ne!(k, cache_key("xin chào [1]", "vi-VN-HoaiMyNeural", 1.1));
    }
}
