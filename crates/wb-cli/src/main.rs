//! `wb` — CLI headless của Whiteboard Studio (khung Giai đoạn 0).

fn main() {
    println!(
        "wb {} — schema dự án v{}. Lệnh render/analyze sẽ có ở Giai đoạn 1.",
        env!("CARGO_PKG_VERSION"),
        wb_model::SCHEMA_VERSION
    );
}
