//! Vỏ Tauri mỏng của Whiteboard Studio. Logic nằm trong `crates/` — ở đây chỉ có lệnh IPC.

mod commands;

use commands::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Bản cài đặt: tay + font nằm trong thư mục tài nguyên. Bản dev: tự dò `assets/` từ thư mục repo.
            let assets_dir = app
                .path()
                .resource_dir()
                .ok()
                .map(|d| d.join("assets"))
                .filter(|d| d.join("hands").is_dir());
            app.manage(AppState::new(assets_dir));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::project_create,
            commands::project_open,
            commands::project_save,
            commands::project_autosave,
            commands::project_discard_autosave,
            commands::asset_import,
            commands::builtin_hands,
            commands::compile_preview,
            commands::export_video,
            commands::export_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("không khởi chạy được ứng dụng Tauri");
}
