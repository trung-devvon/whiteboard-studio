use super::*;

const PATRICK: &[u8] = include_bytes!("../../../tests/fixtures/fonts/PatrickHand-Regular.ttf");
const MALI: &[u8] = include_bytes!("../../../tests/fixtures/fonts/Mali-Regular.ttf");
const ITIM: &[u8] = include_bytes!("../../../tests/fixtures/fonts/Itim-Regular.ttf");

/// Câu trong tiêu chí hoàn thành Giai đoạn 2 (DoD).
const DOD: &str = "Chào mừng các bạn đến với bài học hôm nay";

fn check(font_bytes: &[u8], name: &str, text: &str) -> TextResult {
    let font = Font::from_bytes(font_bytes.to_vec()).unwrap();
    let r = layout_text(&font, text, &TextLayout::default()).unwrap();
    let expected = expected_mark_count(text);
    assert_eq!(
        r.marks, expected,
        "{name}: tìm thấy {} dấu, kỳ vọng {expected}",
        r.marks
    );
    assert!(r.coverage >= 0.99, "{name}: độ phủ {:.2}%", r.coverage * 100.0);
    r
}

#[test]
fn dod_sentence_on_three_ofl_fonts() {
    for (bytes, name) in [(PATRICK, "Patrick Hand"), (MALI, "Mali"), (ITIM, "Itim")] {
        let r = check(bytes, name, DOD);
        // trong mỗi từ: mọi nét dấu đi sau mọi nét thân chữ → không có Text nào ngay sau Diacritic
        // trừ khi sang từ mới (dấu luôn là đuôi của từ)
        let kinds: Vec<StrokeKind> = r.strokes.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&StrokeKind::Diacritic), "{name}: phải có nét dấu");
        assert!(
            kinds.first() == Some(&StrokeKind::Text),
            "{name}: bắt đầu bằng thân chữ"
        );
    }
}

#[test]
fn tricky_vietnamese_lines() {
    for text in ["Tiếng Việt", "Học hỏi chăm chỉ", "Khuỷu tay ỉ ị ĩ", "Người ơi, ước mơ"] {
        check(PATRICK, "Patrick Hand", text);
    }
}

#[test]
fn wraps_to_max_width_and_aligns() {
    let font = Font::from_bytes(PATRICK.to_vec()).unwrap();
    let one = layout_text(&font, DOD, &TextLayout::default()).unwrap();
    let wrapped = layout_text(
        &font,
        DOD,
        &TextLayout {
            max_width: Some(500.0),
            align: Align::Center,
            ..TextLayout::default()
        },
    )
    .unwrap();
    assert!(wrapped.height > one.height * 2, "phải xuống nhiều dòng");
    let box_w = 500.0 * wrapped.raster_scale + 2.0 * wrapped.margin;
    assert!((wrapped.width as f32 - box_w).abs() <= 2.0);
}

#[test]
fn missing_glyphs_are_reported() {
    let font = Font::from_bytes(PATRICK.to_vec()).unwrap();
    let e = layout_text(&font, "Xin chào 你好", &TextLayout::default());
    assert!(matches!(e, Err(TextError::MissingGlyphs(s)) if s.contains('你')));
}
