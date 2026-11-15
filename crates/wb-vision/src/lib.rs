//! Xử lý ảnh: biến ảnh nguồn thành **lớp mực** (để reveal) + **đường tâm nét** (để bàn tay đi theo).
//!
//! - Nhóm A — line-art: [`lineart`].
//! - Ảnh màu / ảnh chụp (Giai đoạn 2, không AI): tách nền đồng màu [`background`], nét phác thảo DoG
//!   [`sketch`], tự nhận loại ảnh [`classify`]. Lớp màu được "tô" bằng cọ (hatching ở `wb-planner`).
//! - Ảnh chụp khó (tách chủ thể, photo→sketch bằng ONNX): Giai đoạn 6.
//!
//! Kế hoạch: `not_push_git/02-xu-ly-anh-va-duong-ve.md`.
//!
//! Toạ độ nét trả ra theo **px của ảnh nguồn**; bán kính `w` cũng theo px ảnh nguồn.

pub mod background;
pub mod binary;
pub mod classify;
pub mod coverage;
pub mod filters;
pub mod ink;
pub mod lineart;
pub mod skeleton;
pub mod sketch;

pub use lineart::{LineArtResult, analyze_line_art, strokes_from_mask};
