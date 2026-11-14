//! Sinh file TypeScript từ kiểu Rust trong `wb-model`.
//!
//! Chạy: `npm run gen:bindings` (= `cargo run -p wb-model --features specta --bin gen-bindings`)
//! Kết quả: `src/bindings/model.ts` (KHÔNG sửa tay — CI kiểm tra file này khớp với Rust).

use specta::Types;
use specta_typescript::Typescript;
use wb_model::{
    DrawPlan, Element, ElementKind, PipelineChoice, PlanStats, PlannedStroke, PointW, Project, ProjectSettings, Scene,
    StrokeDot, StrokeKind, Timing, Transform, VisionParams,
};

fn main() {
    let types = Types::default()
        .register::<PointW>()
        .register::<StrokeKind>()
        .register::<PlannedStroke>()
        .register::<PlanStats>()
        .register::<DrawPlan>()
        .register::<StrokeDot>()
        .register::<Project>()
        .register::<ProjectSettings>()
        .register::<Scene>()
        .register::<Element>()
        .register::<ElementKind>()
        .register::<PipelineChoice>()
        .register::<VisionParams>()
        .register::<Transform>()
        .register::<Timing>();

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/bindings/model.ts");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    Typescript::default()
        .header("// ⚠ FILE SINH TỰ ĐỘNG từ crates/wb-model — không sửa tay.\n// Chạy lại: npm run gen:bindings\n")
        // specta_serde::Format: tôn trọng #[serde(rename_all = ...)] để TS khớp đúng JSON thật
        .export_to(&out, &types, specta_serde::Format)
        .expect("xuất TypeScript thất bại");
    println!("Đã ghi {}", out.display());
}
