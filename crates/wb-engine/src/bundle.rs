//! **Gói biên dịch**: mọi thứ cần để dựng [`Engine`] — lớp mực, DrawPlan, Timing, bàn tay — ở dạng
//! tuần tự hoá được (postcard). App Tauri biên dịch dự án (phân tích ảnh, native) rồi gửi gói này dạng
//! nhị phân sang Player WASM; CLI dùng thẳng. Cùng gói + cùng code → Preview = Export (ADR-0002).
//!
//! Timeline **không** nằm trong gói: mỗi phía tự giải bằng `wb_timing::resolve` (tất định) → gói gọn hơn
//! và không phải tuần tự hoá kiểu của `wb-timing`.

use crate::{ElementAsset, Engine, HandSprite, Place, SceneAsset};
use serde::{Deserialize, Serialize};
use tiny_skia::{IntSize, Pixmap};
use wb_model::{DrawPlan, Timing};
use wb_timing::{ElementInput, SceneInput, Timeline, TimingParams};

/// Ảnh RGBA **premultiplied** (đúng bố cục của `tiny_skia::Pixmap`).
#[derive(Clone, Serialize, Deserialize)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl RawImage {
    pub fn from_pixmap(p: &Pixmap) -> Self {
        Self {
            width: p.width(),
            height: p.height(),
            data: p.data().to_vec(),
        }
    }

    pub fn to_pixmap(&self) -> Option<Pixmap> {
        Pixmap::from_vec(self.data.clone(), IntSize::from_wh(self.width, self.height)?)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BundleElement {
    pub ink: RawImage,
    /// (scale, ox, oy) — xem [`Place`].
    pub place: [f32; 3],
    pub plan: DrawPlan,
    pub timing: Timing,
    /// Mốc lời thoại (ms từ đầu scene) — chỉ lớp đầu của element có dấu `[n]`.
    pub anchor_ms: Option<f32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BundleScene {
    pub background: [u8; 4],
    pub tail_ms: u32,
    pub elements: Vec<BundleElement>,
    /// Độ dài giọng đọc của scene (ms), 0 = không có lời thoại.
    pub narration_ms: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BundleHand {
    pub image: RawImage,
    pub anchor: [f32; 2],
    pub display_height: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bundle {
    /// Cỡ khung xuất của dự án (px).
    pub width: u32,
    pub height: u32,
    pub reference_width: f32,
    pub draw_speed: f32,
    pub scenes: Vec<BundleScene>,
    pub hand: Option<BundleHand>,
}

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("gói biên dịch hỏng: {0}")]
    Decode(#[from] postcard::Error),
    #[error("ảnh trong gói sai kích thước")]
    BadImage,
}

/// Co toạ độ, bán kính và độ dài trong DrawPlan theo hệ số `f`.
fn scale_plan(plan: &mut DrawPlan, f: f32, w: u32, h: u32) {
    let sp = |p: &mut wb_model::PointW| {
        p.x *= f;
        p.y *= f;
        p.w *= f;
    };
    plan.source_width = w;
    plan.source_height = h;
    for s in &mut plan.strokes {
        s.points.iter_mut().for_each(sp);
        s.length *= f;
        for d in &mut s.dots {
            d.at *= f;
            sp(&mut d.point);
        }
    }
    plan.stats.outline_length *= f;
    plan.stats.fill_length *= f;
    plan.stats.travel_length *= f;
}

impl BundleHand {
    pub fn from_sprite(h: &HandSprite) -> Self {
        Self {
            image: RawImage::from_pixmap(&h.pixmap),
            anchor: [h.anchor_x, h.anchor_y],
            display_height: h.display_height,
        }
    }

    pub fn sprite(&self) -> Option<HandSprite> {
        Some(HandSprite {
            pixmap: self.image.to_pixmap()?,
            anchor_x: self.anchor[0],
            anchor_y: self.anchor[1],
            display_height: self.display_height,
        })
    }
}

impl Bundle {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("tuần tự hoá gói")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BundleError> {
        Ok(postcard::from_bytes(bytes)?)
    }

    /// Thu nhỏ trước ảnh tay về cỡ dùng được tới độ phân giải `max_width` → gói gửi qua IPC gọn hơn
    /// (ảnh tay gốc có thể tới 3500px).
    pub fn shrink_hand_for(&mut self, max_width: u32) {
        if let Some(h) = self.hand.as_mut().and_then(|h| h.sprite().map(|s| (h, s))) {
            let k = max_width as f32 / self.reference_width;
            let (slot, sprite) = h;
            if (sprite.pixmap.height() as f32) > sprite.display_height * k {
                *slot = BundleHand::from_sprite(&sprite.prepared(k));
            }
        }
    }

    /// Gọn gói cho Player ở chiều rộng `max_width`: thu nhỏ trước ảnh tay và **lớp mực** lớn hơn nhiều so
    /// với cỡ hiển thị (giữ dư 1,5 lần cho nét mịn). DrawPlan được co theo cùng tỉ lệ và `place.scale` nhân
    /// ngược lại → vị trí trên khung và thời gian vẽ (độ dài nét × scale) giữ nguyên.
    pub fn fit_for_preview(&mut self, max_width: u32) {
        self.shrink_hand_for(max_width);
        let k = max_width as f32 / self.reference_width;
        for el in self.scenes.iter_mut().flat_map(|s| s.elements.iter_mut()) {
            let shown = el.place[0] * k; // px ảnh lớp → px Player
            if shown >= 0.66 {
                continue;
            }
            let Some(ink) = el.ink.to_pixmap() else { continue };
            let small = crate::hand::resample(&ink, shown * 1.5);
            let f = small.width() as f32 / ink.width() as f32;
            el.ink = RawImage::from_pixmap(&small);
            el.place[0] /= f;
            scale_plan(&mut el.plan, f, small.width(), small.height());
        }
    }

    /// Cỡ khung đầu ra cho chiều rộng `out_width` (giữ tỉ lệ, chẵn — encoder H.264 cần cỡ chẵn).
    pub fn output_size(&self, out_width: Option<u32>) -> (u32, u32) {
        match out_width {
            Some(w) => (
                (w & !1).max(2),
                (((w as f32 * self.height as f32 / self.width as f32).round() as u32) & !1).max(2),
            ),
            None => (self.width, self.height),
        }
    }

    /// Chỉ giải Timeline (không dựng ảnh) — vd. để đặt giọng đọc đúng đầu mỗi scene.
    pub fn timeline(&self) -> Timeline {
        let inputs: Vec<SceneInput> = self
            .scenes
            .iter()
            .map(|sc| SceneInput {
                elements: sc
                    .elements
                    .iter()
                    .map(|el| ElementInput {
                        plan: &el.plan,
                        scale: el.place[0],
                        timing: el.timing,
                        anchor_ms: el.anchor_ms.map(f64::from),
                    })
                    .collect(),
                tail_ms: sc.tail_ms,
                min_ms: sc.narration_ms as f64,
            })
            .collect();
        wb_timing::resolve(&inputs, &self.timing_params())
    }

    fn timing_params(&self) -> TimingParams {
        TimingParams {
            draw_speed: self.draw_speed,
            ..TimingParams::default()
        }
    }

    /// Giải Timeline và dựng Engine ở chiều rộng `out_width` (mặc định: cỡ dự án).
    pub fn into_engine(self, out_width: Option<u32>) -> Result<Engine, BundleError> {
        let (out_w, out_h) = self.output_size(out_width);
        let params = self.timing_params();
        let mut scenes = Vec::with_capacity(self.scenes.len());
        let mut timings = Vec::with_capacity(self.scenes.len());
        for sc in self.scenes {
            let mut elements = Vec::with_capacity(sc.elements.len());
            let mut t = Vec::with_capacity(sc.elements.len());
            for el in sc.elements {
                let [scale, ox, oy] = el.place;
                elements.push(ElementAsset {
                    ink: el.ink.to_pixmap().ok_or(BundleError::BadImage)?,
                    place: Place { scale, ox, oy },
                    plan: el.plan,
                });
                t.push((el.timing, el.anchor_ms));
            }
            scenes.push(SceneAsset {
                background: sc.background,
                elements,
            });
            timings.push((t, sc.tail_ms, sc.narration_ms));
        }
        let inputs: Vec<SceneInput> = scenes
            .iter()
            .zip(&timings)
            .map(|(sc, (t, tail_ms, narration_ms))| SceneInput {
                elements: sc
                    .elements
                    .iter()
                    .zip(t)
                    .map(|(a, (timing, anchor))| ElementInput {
                        plan: &a.plan,
                        scale: a.place.scale,
                        timing: *timing,
                        anchor_ms: anchor.map(f64::from),
                    })
                    .collect(),
                tail_ms: *tail_ms,
                min_ms: *narration_ms as f64,
            })
            .collect();
        let timeline = wb_timing::resolve(&inputs, &params);
        let hand = match &self.hand {
            Some(h) => Some(h.sprite().ok_or(BundleError::BadImage)?),
            None => None,
        };
        Ok(Engine::new(out_w, out_h, self.reference_width, scenes, timeline, hand))
    }
}
