# ADR-0002: Một engine Rust cho Preview (WASM) và Export (native)

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-14

## Bối cảnh
Nếu Preview vẽ bằng JS (PixiJS/Konva) còn Export vẽ bằng Rust, hai bên sẽ lệch nhau (easing, khử răng cưa, font, hiệu ứng). Rủi ro R1 (điểm 9/9).

## Quyết định
- `wb-engine` (Rust thuần, không I/O) biên dịch 2 đích: native cho Export, `wasm32` (wasm-bindgen) cho Player trong WebView.
- DrawPlan tính native, gửi sang frontend **một lần** dạng nhị phân `postcard` qua `tauri::ipc::Response` (ArrayBuffer), nạp thẳng vào WASM.
- Player vẽ vào `<canvas>` bằng `putImageData` trên view bộ nhớ WASM (không copy thêm).

Bằng chứng (S2/S5, WebView2): 1,5ms/khung @960×540 với 2.000 nét, 3,7ms với 10.000 nét; IPC nhị phân 5MB ≈ 27ms truyền, nhanh hơn JSON ~4,7×.

## Phương án đã loại
- Viết lại engine bằng JS/WebGL: hai bộ code, lệch kết quả.
- Render native rồi gửi từng frame lên UI: tốn băng thông IPC.
- DrawPlan qua JSON: chậm 4,7×, nặng 3,6×.

## Hệ quả
- CI phải kiểm tra các crate engine build được cho `wasm32-unknown-unknown`.
- Cần `wasm-bindgen-cli` đúng phiên bản crate `wasm-bindgen` (ghim `=0.2.130`).
- Có thể chuyển Player vào Web Worker + OffscreenCanvas sau nếu UI cần thêm thời gian rảnh (hiện chưa cần).
