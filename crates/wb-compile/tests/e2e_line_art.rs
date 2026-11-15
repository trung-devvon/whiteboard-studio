//! Test đầu-cuối Giai đoạn 1 trên bộ ảnh mẫu line-art (`tests/fixtures/images/A`).
//!
//! - Độ phủ của kế hoạch nét ≥ 97% với mọi ảnh.
//! - Khung cuối mỗi scene khớp ảnh gốc đặt lên bảng trắng (sai lệch trung bình < 1%).
//! - Golden: khung giữa chừng không đổi ngoài ý muốn. Cập nhật có chủ đích: `UPDATE_GOLDEN=1 cargo test -p wb-cli`.

use std::path::{Path, PathBuf};
use tiny_skia::{Color, FilterQuality, Pixmap, PixmapPaint, Transform};
use wb_compile::analyze::ContentKind;
use wb_engine::pixmap_from_rgba;
use wb_model::ElementKind;
use wb_vision::classify::ImageClass;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join("tests/fixtures/images/A"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "svg"))
        .collect();
    v.sort();
    v
}

/// Sai lệch trung bình (0..255) giữa hai ảnh cùng kích thước, trên 3 kênh màu.
fn mean_abs_diff(a: &Pixmap, b: &Pixmap) -> f64 {
    assert_eq!((a.width(), a.height()), (b.width(), b.height()));
    let sum: u64 = a
        .data()
        .chunks_exact(4)
        .zip(b.data().chunks_exact(4))
        .map(|(p, q)| {
            (0..3)
                .map(|c| (p[c] as i32 - q[c] as i32).unsigned_abs() as u64)
                .sum::<u64>()
        })
        .sum();
    sum as f64 / (a.width() as f64 * a.height() as f64 * 3.0)
}

#[test]
fn every_fixture_is_covered_by_its_strokes() {
    let files = fixtures();
    assert_eq!(files.len(), 10, "kỳ vọng 10 ảnh mẫu line-art");
    for f in files {
        let img = wb_compile::load_image(&f).unwrap();
        let a = wb_compile::analyze_image(&img, &wb_compile::ImageOptions::default());
        let name = f.file_name().unwrap().to_string_lossy();
        assert_eq!(
            a.kind,
            ContentKind::Image(ImageClass::LineArt),
            "{name}: bộ phân loại phải nhận ra line-art"
        );
        assert_eq!(a.layers.len(), 1);
        assert!(a.coverage() >= 0.97, "{name}: độ phủ {:.2}%", a.coverage() * 100.0);
        assert!(a.stroke_count() <= 40, "{name}: quá nhiều nét ({})", a.stroke_count());
    }
}

#[test]
fn final_frame_of_each_scene_matches_source_image() {
    let project = root().join("tests/fixtures/projects/demo-line-art.json");
    let mut c = wb_compile::compile(&project, Some(640)).unwrap();
    let k = 640.0 / wb_model::REFERENCE_WIDTH;
    let ends: Vec<f64> = c.engine.timeline().scenes.iter().map(|s| s.end_ms - 1.0).collect();
    for (si, t) in ends.into_iter().enumerate() {
        let got = c.engine.render(t).clone();
        // kỳ vọng: bảng trắng + ảnh gốc (nền giấy trắng) đặt đúng chỗ
        let el = &c.project.scenes[si].elements[0];
        let ElementKind::Image { src, .. } = &el.kind else {
            panic!("demo chỉ có ảnh")
        };
        let img = wb_compile::load_image(&project.parent().unwrap().join(src)).unwrap();
        let src_pm = pixmap_from_rgba(img.width(), img.height(), img.as_raw()).unwrap();
        let (s, ox, oy) = el.transform.fit(img.width(), img.height());
        let mut expected = Pixmap::new(got.width(), got.height()).unwrap();
        expected.fill(Color::WHITE);
        let paint = PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        };
        expected.draw_pixmap(
            0,
            0,
            src_pm.as_ref(),
            &paint,
            Transform::from_row(s * k, 0.0, 0.0, s * k, ox * k, oy * k),
            None,
        );
        let d = mean_abs_diff(&got, &expected);
        assert!(d < 2.55, "scene {}: sai lệch trung bình {d:.3}/255 (≥ 1%)", si + 1);
    }
}

#[test]
fn golden_frames_mid_drawing() {
    let project = root().join("tests/fixtures/projects/demo-line-art.json");
    let mut c = wb_compile::compile(&project, Some(480)).unwrap();
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| v == "1");
    let dir = root().join("tests/golden");
    let picks: Vec<(usize, f64)> = vec![(0, 0.4), (0, 0.8), (2, 0.5), (7, 0.6)];
    let mut fails = Vec::new();
    for (si, frac) in picks {
        let el = &c.engine.timeline().scenes[si].elements[0];
        let t = el.draw_start_ms + (el.draw_end_ms - el.draw_start_ms) * frac;
        let got = c.engine.render(t).clone();
        let path = dir.join(format!("line-art-s{:02}-{:02}.png", si + 1, (frac * 100.0) as u32));
        if update || !path.exists() {
            got.save_png(&path).unwrap();
            eprintln!("đã ghi golden {}", path.display());
            continue;
        }
        let want = Pixmap::load_png(&path).unwrap();
        let d = mean_abs_diff(&got, &want);
        if d > 0.5 {
            let diff_path = dir.join(format!("{}.actual.png", path.file_stem().unwrap().to_string_lossy()));
            got.save_png(&diff_path).unwrap();
            fails.push(format!(
                "{} lệch {d:.3}/255 — xem {}",
                path.display(),
                diff_path.display()
            ));
        }
    }
    assert!(fails.is_empty(), "golden khác:\n{}", fails.join("\n"));
}
