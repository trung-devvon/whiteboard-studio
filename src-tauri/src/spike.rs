//! Lệnh tạm thời cho SPIKE S2 + S5 (Giai đoạn 0). XOÁ file này khi bắt đầu Giai đoạn 3.
//!
//! S5: so sánh gửi DrawPlan qua IPC dạng nhị phân (`ipc::Response`) vs JSON.
//! Chạy tự động: đặt biến môi trường `WB_SPIKE_AUTORUN=1` rồi `npm run tauri dev`
//! → frontend chạy đo, gửi báo cáo về đây, app tự thoát.

use std::time::Instant;
use tauri::{AppHandle, ipc::Response};
use wb_model::DrawPlan;

const W: u32 = 960;
const H: u32 = 540;

/// DrawPlan giả lập dạng nhị phân (postcard) — trả ArrayBuffer cho JS, không qua JSON.
#[tauri::command]
pub fn spike_plan_binary(strokes: u32) -> Response {
    let t = Instant::now();
    let bytes = s2_wasm_preview::synthetic_plan_bytes(strokes, 42, W, H);
    eprintln!(
        "[S5] sinh + postcard {} nét = {} byte trong {:?}",
        strokes,
        bytes.len(),
        t.elapsed()
    );
    Response::new(bytes)
}

/// Cùng DrawPlan nhưng trả JSON (để so sánh).
#[tauri::command]
pub fn spike_plan_json(strokes: u32) -> DrawPlan {
    s2_wasm_preview::synthetic_plan(strokes, 42, W, H)
}

#[tauri::command]
pub fn spike_autorun() -> bool {
    std::env::var("WB_SPIKE_AUTORUN").is_ok_and(|v| v == "1")
}

/// Ghi báo cáo đo vào `spikes/out/s2s5-report.json`; nếu chạy tự động thì thoát app.
#[tauri::command]
pub fn spike_report(app: AppHandle, report: serde_json::Value) -> Result<String, String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../spikes/out");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("s2s5-report.json");
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    eprintln!("[S2/S5] đã ghi {}", path.display());
    if spike_autorun() {
        app.exit(0);
    }
    Ok(path.display().to_string())
}
