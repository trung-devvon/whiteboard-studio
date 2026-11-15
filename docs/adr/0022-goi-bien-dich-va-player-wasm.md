# ADR-0022: Gói biên dịch (Bundle) + Player WASM; dự án là thư mục `.wbproj`

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-15

## Bối cảnh
ADR-0002 chốt "một engine, hai đích": xem trước trong app (WASM) và xuất video (native) dùng cùng `wb-engine`. Khi làm editor (Giai đoạn 3) cần quyết định **dữ liệu gì** đi từ Rust native (nơi phân tích ảnh — cần file I/O, không chạy được trong WASM) sang Player WASM, và dự án được lưu trên đĩa ra sao.

## Quyết định
1. **Gói biên dịch `wb_engine::Bundle`** (postcard) chứa đúng đầu vào của Engine: lớp mực (RGBA premultiplied), `place`, DrawPlan, `Timing` từng lớp, ảnh tay đã chuẩn bị. **Timeline không nằm trong gói** — mỗi phía tự giải bằng `wb_timing::resolve` (tất định), nên không cần tuần tự hoá kiểu của `wb-timing`.
2. Phân tích + đóng gói nằm trong crate mới **`wb-compile`** (tách từ `wb-cli`), dùng chung cho CLI và app. App giữ một `AnalysisCache` (khoá = nội dung element + file nguồn): kéo/đổi cỡ element không phân tích lại.
3. Gói cho Player được **làm gọn** (`Bundle::fit_for_preview`): lớp mực lớn hơn nhiều so với cỡ hiển thị được thu nhỏ (giữ dư 1,5 lần), DrawPlan co cùng tỉ lệ, `place.scale` nhân ngược → vị trí và thời gian vẽ giữ nguyên. Demo Giai đoạn 2: 30 MB → 6,4 MB, khung lệch < 2% (test `preview_bundle_is_smaller_and_looks_the_same`).
4. IPC: `compile_preview` trả nhị phân `[u32 độ dài JSON][JSON báo cáo][Bundle]` qua `ipc::Response`; Player (`wb-player`, wasm-bindgen) vẽ thẳng vào bộ nhớ WASM, canvas đọc qua `ImageData`.
5. **Dự án = thư mục `*.wbproj`**: `project.json` (ghi atomic: file tạm → fsync → rename), `autosave.json` (chỉ tồn tại khi có thay đổi chưa lưu → dùng để khôi phục sau sự cố), `assets/` (ảnh nhập vào, tên kèm mã băm FNV-1a nội dung). Đọc file qua bước `migrate` theo `schema_version`.
6. Tài nguyên có sẵn của app được tham chiếu bằng `builtin:hands/…`, `builtin:fonts/…` (giải bởi `CompileContext`) → dự án không chứa đường dẫn tuyệt đối tới thư mục cài đặt.

## Phương án đã loại
- **Gửi khung hình đã render từ native** sang UI: đơn giản nhưng mỗi khung phải qua IPC (~2 MB ở 960×540) → không tua mượt; trái ADR-0002.
- **Gửi DrawPlan + đường dẫn ảnh, WASM tự giải mã ảnh**: WASM phải mang theo bộ giải PNG/JPEG/SVG và lặp lại bước phân tích → hai nơi tính, dễ lệch.
- **File dự án đơn (zip)**: ghi atomic và nhập ảnh phức tạp hơn; để dành cho "gói chia sẻ" `.wbz` sau này.

## Hệ quả
- Mỗi lần sửa dự án, app biên dịch lại và gửi lại cả gói (có đệm phân tích nên chủ yếu tốn thời gian đóng gói + truyền ~vài MB). Khi dự án lớn có thể cần gửi theo scene hoặc theo element thay đổi.
- `wb-player` bị ràng buộc build được cho `wasm32-unknown-unknown` (CI kiểm tra).
