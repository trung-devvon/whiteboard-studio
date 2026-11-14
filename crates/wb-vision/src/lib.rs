//! Xử lý ảnh: biến ảnh nguồn thành **lớp mực** (để reveal) + **đường tâm nét** (để bàn tay đi theo).
//!
//! Giai đoạn 1 có pipeline **Nhóm A — line-art** ([`lineart`]). Các nhóm B–E ở Giai đoạn 2/6.
//! Kế hoạch: `not_push_git/02-xu-ly-anh-va-duong-ve.md`.
//!
//! Toạ độ nét trả ra theo **px của ảnh nguồn**; bán kính `w` cũng theo px ảnh nguồn.

pub mod binary;
pub mod coverage;
pub mod ink;
pub mod lineart;
pub mod skeleton;

pub use lineart::{LineArtResult, analyze_line_art};
