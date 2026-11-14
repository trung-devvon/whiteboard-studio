//! Vỏ Tauri mỏng của Whiteboard Studio. Logic nằm trong `crates/` — ở đây chỉ có lệnh IPC.

mod spike; // TẠM — Giai đoạn 0 (S2/S5). Xoá ở Giai đoạn 3.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            spike::spike_plan_binary,
            spike::spike_plan_json,
            spike::spike_autorun,
            spike::spike_report,
        ])
        .run(tauri::generate_context!())
        .expect("không khởi chạy được ứng dụng Tauri");
}
