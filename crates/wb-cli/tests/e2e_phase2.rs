//! Kiểm thử đầu-cuối Giai đoạn 2: chữ viết tay + SVG đi theo path + lớp tô màu bằng cọ.

use std::path::Path;
use wb_cli::ContentKind;
use wb_model::DrawStyle;
use wb_vision::classify::ImageClass;

#[test]
fn demo_phase2_compiles_with_expected_layers_and_final_colors() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/projects/demo-phase2.json");
    let mut c = wb_cli::compile(&project, Some(640)).unwrap();

    let kinds: Vec<(ContentKind, Vec<&str>)> = c.report.iter().map(|r| (r.kind, r.layers.clone())).collect();
    assert_eq!(
        kinds,
        vec![
            (ContentKind::Text, vec!["chữ"]),
            (ContentKind::Image(ImageClass::Vector), vec!["nét"]),
            (ContentKind::Image(ImageClass::Vector), vec!["nét", "tô màu"]),
            (ContentKind::Text, vec!["chữ"]),
        ]
    );
    assert_eq!(c.report[2].style, DrawStyle::SketchThenColor);
    for r in &c.report {
        assert!(r.coverage >= 0.99, "{}: độ phủ {:.2}%", r.id, r.coverage * 100.0);
    }

    // khung cuối scene 2: mái nhà đỏ (#e4572e) và tường vàng đã hiện đủ
    let end = c.engine.timeline().scenes[1].end_ms - 1.0;
    let frame = c.engine.render(end).clone();
    let count = |pred: &dyn Fn(u8, u8, u8) -> bool| {
        frame
            .pixels()
            .iter()
            .filter(|p| pred(p.red(), p.green(), p.blue()))
            .count()
    };
    let roof = count(&|r, g, b| r > 200 && (70..110).contains(&g) && b < 70);
    let wall = count(&|r, g, b| r > 230 && g > 190 && (80..120).contains(&b));
    assert!(roof > 2000, "mái nhà phải hiện màu, có {roof} điểm");
    assert!(wall > 4000, "tường phải hiện màu, có {wall} điểm");

    // giữa lớp nét (trước khi tô): chưa có màu mái nhà
    let el = c.engine.timeline().scenes[1].elements[0].clone();
    let mid = (el.draw_start_ms + el.draw_end_ms) / 2.0;
    let f = c.engine.render(mid).clone();
    let roof_mid = f
        .pixels()
        .iter()
        .filter(|p| p.red() > 200 && (70..110).contains(&p.green()) && p.blue() < 70)
        .count();
    assert!(roof_mid < 50, "lớp nét chưa được lộ màu tô ({roof_mid})");
}

/// Mọi bộ tay trong `assets/hands` nạp được ở cả hai kiểu; kiểu `wrist` cắt bớt tay có cổ tay,
/// còn bộ chỉ có cây bút (không khai báo `wrist`) giữ nguyên ảnh.
#[test]
fn bundled_hands_load_in_both_modes() {
    use wb_model::HandMode;
    let hands = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/hands");
    for (name, has_wrist) in [("pen-brown", true), ("pencil-purple", true), ("pencil-green", false)] {
        let dir = hands.join(name);
        let full = wb_cli::load_hand(&dir, HandMode::Full, 1.0).unwrap();
        let wrist = wb_cli::load_hand(&dir, HandMode::Wrist, 1.0).unwrap();
        let area = |h: &wb_engine::HandSprite| h.pixmap.width() as u64 * h.pixmap.height() as u64;
        if has_wrist {
            assert!(area(&wrist) < area(&full), "{name}: kiểu wrist phải cắt bớt cẳng tay");
        } else {
            assert_eq!(area(&wrist), area(&full), "{name}: không có cổ tay → nguyên ảnh");
        }
        // mũi bút vẫn nằm trong phần ảnh còn lại
        let (x, y) = (wrist.anchor_x.round() as u32, wrist.anchor_y.round() as u32);
        assert!(
            x < wrist.pixmap.width() && y < wrist.pixmap.height(),
            "{name}: mũi bút ngoài ảnh"
        );
    }
}
