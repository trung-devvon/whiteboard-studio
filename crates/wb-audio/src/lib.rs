//! Giọng đọc lời thoại (Giai đoạn 4): gọi Azure AI Speech lấy âm thanh + mốc thời gian từng từ và
//! từng dấu `[n]`, lưu đệm trong thư mục dự án, đọc/ghi WAV, trộn tiếng cho video.
//!
//! Chỉ chạy native (mạng + file) — không nằm trong nhóm crate WASM.
//! Quyết định chọn dịch vụ: ADR-0012 (Edge-TTS bị loại; dùng Azure bằng key của chính người dùng).

pub mod azure;
pub mod credentials;
pub mod mix;
pub mod narration;
pub mod sfx;
pub mod store;
pub mod subtitles;
pub mod wav;

pub use azure::{AzureConfig, TtsError, synthesize};

use serde::{Deserialize, Serialize};

/// Tần số lấy mẫu của giọng đọc (Hz, mono 16-bit).
pub const SAMPLE_RATE: u32 = 24_000;

/// Kết quả đọc một đoạn lời thoại.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Speech {
    /// Độ dài âm thanh (ms).
    pub duration_ms: f64,
    /// Mốc thời gian từng từ (dùng cho phụ đề, hiển thị).
    pub words: Vec<WordMark>,
    /// Mốc của các dấu trong lời thoại (`e1`, `e2`… = element 1, 2…).
    pub bookmarks: Vec<Bookmark>,
    /// Mẫu âm thanh (mono, [`SAMPLE_RATE`]). Lưu riêng thành file WAV.
    #[serde(skip)]
    pub pcm: Vec<i16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WordMark {
    pub text: String,
    pub start_ms: f64,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub at_ms: f64,
}

impl Speech {
    /// Thời điểm (ms tính từ đầu đoạn) element thứ `n` (1 = đầu tiên) bắt đầu vẽ, nếu lời thoại có dấu `[n]`.
    pub fn anchor_of(&self, n: usize) -> Option<f64> {
        let name = narration::bookmark_name(n);
        self.bookmarks.iter().find(|b| b.name == name).map(|b| b.at_ms)
    }
}
