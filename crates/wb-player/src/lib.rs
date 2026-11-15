//! Player WASM: app (Tauri) biên dịch dự án ở phía Rust native rồi gửi [`Bundle`] dạng nhị phân
//! (postcard, `ipc::Response`) sang đây; Player dựng [`Engine`] và vẽ từng khung vào bộ nhớ WASM để
//! canvas đọc thẳng (không chép qua JS).
//!
//! Cùng `wb-engine` với lúc xuất video → xem trước khớp video xuất (ADR-0002).
//! Build: `npm run build:wasm` (ra `src/wasm/pkg/`, không commit).

use wasm_bindgen::prelude::*;
use wb_engine::{Bundle, Engine};

#[wasm_bindgen]
pub struct Player {
    engine: Engine,
    width: u32,
    height: u32,
    scene_starts: Vec<f64>,
    scene_ends: Vec<f64>,
}

#[wasm_bindgen]
impl Player {
    /// Nạp gói biên dịch, render ở chiều rộng `width` (giữ tỉ lệ dự án).
    #[wasm_bindgen(constructor)]
    pub fn new(bundle: &[u8], width: u32) -> Result<Player, JsError> {
        let bundle = Bundle::from_bytes(bundle).map_err(|e| JsError::new(&e.to_string()))?;
        let (w, h) = bundle.output_size(Some(width));
        let engine = bundle
            .into_engine(Some(width))
            .map_err(|e| JsError::new(&e.to_string()))?;
        let tl = engine.timeline();
        let scene_starts = tl.scenes.iter().map(|s| s.start_ms).collect();
        let scene_ends = tl.scenes.iter().map(|s| s.end_ms).collect();
        Ok(Player {
            engine,
            width: w,
            height: h,
            scene_starts,
            scene_ends,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    #[wasm_bindgen(js_name = durationMs)]
    pub fn duration_ms(&self) -> f64 {
        self.engine.duration_ms()
    }

    /// Mốc bắt đầu từng scene (ms).
    #[wasm_bindgen(js_name = sceneStarts)]
    pub fn scene_starts(&self) -> Vec<f64> {
        self.scene_starts.clone()
    }

    /// Mốc kết thúc từng scene (ms).
    #[wasm_bindgen(js_name = sceneEnds)]
    pub fn scene_ends(&self) -> Vec<f64> {
        self.scene_ends.clone()
    }

    /// Vẽ khung tại `t_ms`; trả con trỏ tới RGBA (`frameLen()` byte) trong bộ nhớ WASM.
    /// Khung luôn đục (nền scene) nên premultiplied = RGBA thường → đưa thẳng vào `ImageData`.
    pub fn render(&mut self, t_ms: f64) -> *const u8 {
        self.engine.render(t_ms).data().as_ptr()
    }

    #[wasm_bindgen(js_name = frameLen)]
    pub fn frame_len(&self) -> usize {
        (self.width * self.height * 4) as usize
    }
}
