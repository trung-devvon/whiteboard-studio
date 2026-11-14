//! Kiểu dữ liệu dùng chung của Whiteboard Studio.
//!
//! Đây là **nguồn sự thật** cho mọi kiểu dữ liệu: kiểu TypeScript ở `src/bindings/`
//! được sinh tự động từ crate này (`npm run gen:bindings`).
//! Kế hoạch chi tiết: `not_push_git/05-mo-hinh-du-lieu-va-dinh-dang-du-an.md`.
//!
//! - [`project`]: tài liệu dự án người dùng sửa (Project → Scene → Element).
//! - [`plan`]: dữ liệu dẫn xuất — nét đã sắp thứ tự ([`DrawPlan`]).

pub mod plan;
pub mod project;

pub use plan::*;
pub use project::*;

/// Phiên bản cấu trúc file dự án. Tăng khi đổi cấu trúc không tương thích (kèm migration).
pub const SCHEMA_VERSION: u32 = 1;

/// Khung tham chiếu: mọi toạ độ trong tài liệu dự án tính theo px của khung 1920×1080.
pub const REFERENCE_WIDTH: f32 = 1920.0;
pub const REFERENCE_HEIGHT: f32 = 1080.0;
