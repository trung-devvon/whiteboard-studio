use super::*;
use wb_model::{PlannedStroke, PointW, StrokeKind, Timing};
use wb_timing::{ElementInput, SceneInput, TimingParams, resolve};

/// Ảnh nguồn 200×100: hai vạch ngang đen dày 6px trên nền trong suốt.
fn asset() -> ElementAsset {
    let (w, h) = (200u32, 100u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 20..180 {
            if (27..33).contains(&y) || (67..73).contains(&y) {
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(&[10, 10, 10, 255]);
            }
        }
    }
    let line = |y: f32| {
        PlannedStroke::new(
            StrokeKind::Outline,
            (0..=16).map(|k| PointW::new(20.0 + k as f32 * 10.0, y, 3.0)).collect(),
        )
    };
    ElementAsset {
        ink: pixmap_from_rgba(w, h, &rgba).unwrap(),
        place: Place {
            scale: 2.0,
            ox: 100.0,
            oy: 50.0,
        },
        plan: DrawPlan::new(w, h, vec![line(30.0), line(70.0)]),
    }
}

fn engine(with_hand: bool) -> Engine {
    let el = asset();
    let tl = resolve(
        &[SceneInput {
            elements: vec![ElementInput {
                plan: &el.plan,
                scale: el.place.scale,
                timing: Timing::default(),
                anchor_ms: None,
            }],
            tail_ms: 500,
            min_ms: 0.0,
        }],
        &TimingParams {
            min_element_ms: 0.0,
            ..TimingParams::default()
        },
    );
    let hand = with_hand.then(|| HandSprite {
        pixmap: {
            let mut p = Pixmap::new(20, 40).unwrap();
            p.fill(Color::from_rgba8(255, 0, 0, 255));
            p
        },
        anchor_x: 0.0,
        anchor_y: 0.0,
        display_height: 40.0,
    });
    Engine::new(
        640,
        360,
        640.0,
        vec![SceneAsset {
            background: [255, 255, 255, 255],
            elements: vec![el],
        }],
        tl,
        hand,
    )
}

fn dark_pixels(p: &Pixmap) -> usize {
    p.pixels().iter().filter(|c| c.red() < 60 && c.green() < 60).count()
}

#[test]
fn final_frame_equals_ink_composited_on_board() {
    let mut e = engine(false);
    let end = e.duration_ms() - 1.0;
    let got = e.render(end).clone();
    // kỳ vọng: nền trắng + lớp mực đặt đúng chỗ
    let mut expected = Pixmap::new(640, 360).unwrap();
    expected.fill(Color::WHITE);
    let el = asset();
    let rect = screen_rect(&el, 1.0, 640, 360).unwrap();
    let ink = place_ink(&el, 1.0, rect);
    expected.draw_pixmap(
        rect.x(),
        rect.y(),
        ink.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
    assert_eq!(got.data(), expected.data(), "khung cuối phải khớp tuyệt đối");
}

#[test]
fn reveal_grows_monotonically_while_drawing() {
    let mut e = engine(false);
    let el = &e.timeline().scenes[0].elements[0];
    let times: Vec<f64> = (0..=10)
        .map(|i| el.draw_start_ms + (el.draw_end_ms - el.draw_start_ms) * i as f64 / 10.0)
        .collect();
    let mut prev = 0;
    for t in times {
        let n = dark_pixels(e.render(t));
        assert!(n >= prev, "độ phủ giảm tại t={t}: {n} < {prev}");
        prev = n;
    }
    assert!(prev > 0);
}

#[test]
fn seeking_backwards_gives_same_frame_as_sequential() {
    let mut a = engine(true);
    let mid = {
        let el = &a.timeline().scenes[0].elements[0];
        (el.slots[1].t0 + el.slots[1].t1) / 2.0
    };
    for i in 0..60 {
        a.render(mid * i as f64 / 60.0);
    }
    let seq = a.render(mid).clone();
    let mut b = engine(true);
    b.render(a.duration_ms() - 1.0); // nhảy tới cuối rồi tua ngược
    let seek = b.render(mid).clone();
    assert_eq!(seq.data(), seek.data(), "render phải tất định bất kể thứ tự gọi");
}

#[test]
fn hand_is_drawn_while_drawing_and_gone_at_end() {
    let mut e = engine(true);
    let el = e.timeline().scenes[0].elements[0].clone();
    let red = |p: &Pixmap| p.pixels().iter().filter(|c| c.red() > 200 && c.green() < 50).count();
    assert!(
        red(e.render((el.slots[0].t0 + el.slots[0].t1) / 2.0)) > 100,
        "tay phải hiện khi đang vẽ"
    );
    assert_eq!(red(e.render(e.duration_ms() - 1.0)), 0, "tay đã rút khi xong");
}
