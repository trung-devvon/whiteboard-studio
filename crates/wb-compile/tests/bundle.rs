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

/// Đổi màu icon/ảnh nét: mọi điểm có mực thành màu đã chọn, và không phải phân tích lại ảnh.
#[test]
fn tint_recolors_image_ink_without_reanalysis() {
    let mut project = wb_project::sample_project();
    let ctx = CompileContext::new(std::env::temp_dir());
    let cache = AnalysisCache::new();
    prepare(&project, &ctx, Some(&cache)).unwrap();
    let n = cache.len();

    let mut images = Vec::new();
    for sc in &mut project.scenes {
        for el in &mut sc.elements {
            if let wb_model::ElementKind::Image { tint, .. } = &mut el.kind {
                *tint = Some("#C0392B".into());
                images.push(el.id.clone());
            }
        }
    }
    assert!(!images.is_empty());
    let p = prepare(&project, &ctx, Some(&cache)).unwrap();
    assert_eq!(cache.len(), n, "đổi màu không được phân tích lại");

    let mut checked = 0;
    for (scene, owners) in p.bundle.scenes.iter().zip(&p.owners) {
        for (el, &owner) in scene.elements.iter().zip(owners) {
            if !images.contains(&p.report[owner].id) {
                continue;
            }
            // dữ liệu nhân trước (premultiplied): kênh = màu × alpha / 255
            for px in el.ink.data.chunks_exact(4).filter(|px| px[3] >= 128) {
                let a = px[3] as i32;
                for (c, want) in px[..3].iter().zip([0xC0, 0x39, 0x2B]) {
                    assert!((*c as i32 - want * a / 255).abs() <= 2, "điểm {px:?} chưa đổi màu");
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 1000);
}

/// Lời thoại có dấu `[2]` + giọng đã tạo sẵn (giả lập, không gọi mạng) → element 2 bắt đầu vẽ đúng mốc,
/// scene dài ít nhất bằng giọng đọc, rãnh tiếng đặt đúng đầu scene.
#[test]
fn narration_markers_anchor_elements_and_build_audio_track() {
    let dir = std::env::temp_dir().join(format!("wb-narration-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let project = wb_project::sample_project();
    let ctx = CompileContext::new(&dir);

    // chưa tạo giọng → Missing, không có mốc
    let p = prepare(&project, &ctx, None).unwrap();
    assert!(
        p.narration
            .iter()
            .all(|n| n.state == wb_compile::NarrationState::Missing)
    );

    // giả lập giọng đã tạo cho cả 2 scene: dài 5 s, [2] ở 2.5 s
    for sc in &project.scenes {
        let key = wb_audio::narration::cache_key(
            sc.narration.as_deref().unwrap(),
            &project.settings.voice.name,
            project.settings.voice.rate,
        );
        let speech = wb_audio::Speech {
            duration_ms: 5000.0,
            words: vec![],
            bookmarks: vec![
                wb_audio::Bookmark {
                    name: "e1".into(),
                    at_ms: 0.0,
                },
                wb_audio::Bookmark {
                    name: "e2".into(),
                    at_ms: 2500.0,
                },
            ],
            pcm: vec![1000; 5 * wb_audio::SAMPLE_RATE as usize],
        };
        wb_audio::store::save(&dir, &key, &speech).unwrap();
    }
    let p = prepare(&project, &ctx, None).unwrap();
    assert!(p.narration.iter().all(|n| n.state == wb_compile::NarrationState::Ready));
    let tl = p.bundle.timeline();
    for (sc, owners) in tl.scenes.iter().zip(&p.owners) {
        // timeline tính theo lớp vẽ: tìm lớp đầu của element 2 (một element có thể nhiều lớp)
        let k = owners.iter().position(|&o| o != owners[0]).unwrap();
        let second = &sc.elements[k];
        eprintln!(
            "scene {:.0}–{:.0}: {:?} k={k}",
            sc.start_ms,
            sc.end_ms,
            sc.elements
                .iter()
                .map(|e| (e.draw_start_ms as u32, e.draw_end_ms as u32))
                .collect::<Vec<_>>()
        );
        assert!(
            (second.draw_start_ms - (sc.start_ms + 2500.0)).abs() < 1.0,
            "element 2 lệch mốc: {second:?}"
        );
        assert!(sc.elements[k - 1].end_ms <= second.enter_ms + 1e-6);
        assert!(sc.end_ms - sc.start_ms >= 5000.0);
    }
    let track = wb_compile::narration_track(&p.narration, &tl).unwrap();
    let at = |ms: f64| track[(ms * wb_audio::SAMPLE_RATE as f64 / 1000.0) as usize];
    let s2 = tl.scenes[1].start_ms;
    assert_eq!(at(10.0), 1000);
    assert_eq!(at(s2 - 10.0), 0, "khoảng lặng giữa hai scene");
    assert_eq!(at(s2 + 10.0), 1000);
    let _ = std::fs::remove_dir_all(&dir);
}
