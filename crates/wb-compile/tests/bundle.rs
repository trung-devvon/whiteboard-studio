//! Gói biên dịch (Bundle) và bộ nhớ đệm phân tích — nền tảng của Player trong app (Giai đoạn 3).

use std::path::{Path, PathBuf};
use wb_compile::{AnalysisCache, Bundle, CompileContext, prepare, read_project};

fn demo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/projects/demo-phase2.json")
}

/// Gửi gói qua "IPC" (postcard) rồi dựng Engine ở phía bên kia → khung hình y hệt Engine dựng trực tiếp.
/// Đây chính là cam kết Preview (WASM) = Export (native) của ADR-0002.
#[test]
fn bundle_roundtrip_renders_identical_frames() {
    let path = demo();
    let project = read_project(&path).unwrap();
    let ctx = CompileContext::new(path.parent().unwrap());
    let prepared = prepare(&project, &ctx, None).unwrap();
    let bytes = prepared.bundle.to_bytes();
    let (mut direct, _) = prepared.into_engine(Some(480)).unwrap();
    let mut remote = Bundle::from_bytes(&bytes).unwrap().into_engine(Some(480)).unwrap();
    assert_eq!(direct.duration_ms(), remote.duration_ms());
    let total = direct.duration_ms();
    for i in 0..5 {
        let t = total * (i as f64 * 0.23 + 0.05) % total;
        assert_eq!(
            direct.render(t).data(),
            remote.render(t).data(),
            "khung tại {t:.0}ms khác nhau"
        );
    }
}

/// Gói gọn cho Player (lớp mực thu nhỏ) nhẹ hơn nhiều nhưng khung hình gần như y hệt, thời lượng giữ nguyên.
#[test]
fn preview_bundle_is_smaller_and_looks_the_same() {
    let path = demo();
    let project = read_project(&path).unwrap();
    let ctx = CompileContext::new(path.parent().unwrap());
    let full = prepare(&project, &ctx, None).unwrap().bundle;
    let mut small = full.clone();
    small.fit_for_preview(480);
    let (a, b) = (full.to_bytes().len(), small.to_bytes().len());
    assert!(b * 3 < a, "gói gọn {b} byte, gói đầy đủ {a} byte");

    let mut e1 = full.into_engine(Some(480)).unwrap();
    let mut e2 = small.into_engine(Some(480)).unwrap();
    assert!((e1.duration_ms() - e2.duration_ms()).abs() < 5.0);
    let total = e1.duration_ms();
    for f in [0.2, 0.5, 0.8, 0.999] {
        let t = total * f;
        let (p, q) = (e1.render(t).data().to_vec(), e2.render(t).data().to_vec());
        let diff = p.iter().zip(&q).map(|(x, y)| x.abs_diff(*y) as f64).sum::<f64>() / p.len() as f64 / 255.0;
        assert!(diff < 0.02, "khác {:.2}% tại {f}", diff * 100.0);
    }
}

/// Kéo element đi chỗ khác không phân tích lại; đổi nội dung chữ thì có.
#[test]
fn cache_skips_reanalysis_when_only_position_changes() {
    let path = demo();
    let mut project = read_project(&path).unwrap();
    let ctx = CompileContext::new(path.parent().unwrap());
    let cache = AnalysisCache::new();
    prepare(&project, &ctx, Some(&cache)).unwrap();
    let n = cache.len();
    assert!(n >= 3);

    // chỉ dời ảnh (không đổi bề rộng chữ) → không có mục đệm mới
    for sc in &mut project.scenes {
        for el in &mut sc.elements {
            if matches!(el.kind, wb_model::ElementKind::Image { .. }) {
                el.transform.x += 50.0;
            }
        }
    }
    let p = prepare(&project, &ctx, Some(&cache)).unwrap();
    assert_eq!(cache.len(), n);
    assert!(p.report.iter().all(|r| r.coverage >= 0.99));

    // đổi chữ → phân tích lại đúng element đó
    if let wb_model::ElementKind::Text { text, .. } = &mut project.scenes[0].elements[0].kind {
        text.push('!');
    }
    prepare(&project, &ctx, Some(&cache)).unwrap();
    assert_eq!(cache.len(), n + 1);
}

/// Dự án mẫu ("Thử dự án mẫu" trên màn hình chào) chỉ dùng tài nguyên `builtin:` → biên dịch được ở bất cứ đâu.
#[test]
fn sample_project_compiles_from_builtin_assets() {
    let project = wb_project::sample_project();
    let ctx = CompileContext::new(std::env::temp_dir());
    let p = prepare(&project, &ctx, None).unwrap();
    assert_eq!(p.report.len(), 4);
    assert!(p.report.iter().all(|r| r.coverage >= 0.99));
    assert!(p.bundle.hand.is_some());
}

#[test]
fn builtin_paths_resolve_into_app_assets() {
    let ctx = CompileContext::new(std::env::temp_dir());
    let hand = ctx.resolve("builtin:hands/pen-brown").unwrap();
    assert!(hand.join("hand.json").is_file(), "{}", hand.display());
}
