# Giai đoạn 3 — Ứng dụng Tauri & Editor (phần 1–2)

> Nhánh: `phase-1-core-engine` · Kế hoạch gốc: `not_push_git/06-lo-trinh-va-tieu-chi-hoan-thanh.md` (Giai đoạn 3) · Quyết định: [ADR-0022](../adr/0022-goi-bien-dich-va-player-wasm.md)

## 1. Tiến độ theo kế hoạch

| Hạng mục | Trạng thái |
|---|---|
| ProjectStore: tạo/mở/lưu `*.wbproj`, ghi atomic, autosave, khôi phục sau sự cố, khung migration | ✅ `wb-project` (5 test) |
| Editor: danh sách scene, thêm ảnh/chữ, kéo/đổi cỡ (Konva Transformer), Inspector | ✅ |
| Thêm icon / mũi tên, snap & đường gióng | ⏳ chưa làm |
| Undo/redo (zundo), phím tắt | ✅ Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z, Ctrl+S, Delete, Space, Esc |
| i18n khung (vi/en) | ⏳ chưa làm (giao diện hiện chỉ tiếng Việt) |
| Phân tích ảnh chạy nền + tiến độ qua Channel | ✅ biên dịch ở luồng nền (`spawn_blocking`), có đệm phân tích; xuất video báo tiến độ qua `Channel` |
| Overlay thứ tự nét | ⏳ mới đánh số thứ tự element, chưa vẽ thứ tự nét |
| Player WASM + thanh tua | ✅ `wb-player`; tua ngược dựng lại scene từ đầu (chưa có keyframe snapshot) |
| Hộp thoại Xuất video (tiến độ, huỷ, mở thư mục) | ✅ |
| Trình chỉnh bàn tay (import, xoá phông, chấm mũi bút/cổ tay) | ✅ phần 2 — xem mục 6 |
| Màn hình chào dễ hiểu: dự án mẫu, mở gần đây, giải thích "dự án là gì" | ✅ phần 2 |
| Hỏi lưu khi đóng cửa sổ | ✅ phần 2 |

**DoD** (người lạ tự làm video 3 scene; Player khớp video xuất < 2%; kill app không mất dữ liệu):
- Khớp Player/xuất: cùng một gói → khung **giống hệt từng byte** (test `bundle_roundtrip_renders_identical_frames`); gói đã làm gọn cho Player lệch < 2% (test `preview_bundle_is_smaller_and_looks_the_same`).
- Không mất dữ liệu: `project.json` ghi atomic; thay đổi chưa lưu được tự lưu sau 4 s vào `autosave.json`, lần mở sau hỏi khôi phục (test `autosave_is_offered_as_recovery_until_saved`).
- Thử với người lạ: ⏳ chưa làm.

## 2. Kiến trúc

```text
React editor ──invoke(JSON)──► src-tauri/commands.rs (vỏ mỏng)
   │                               ├─ wb-project  : mở/lưu/autosave/nhập ảnh
   │                               ├─ wb-compile  : phân tích (có đệm) → Bundle
   │                               └─ wb-export   : Engine → FFmpeg (tiến độ qua Channel)
   │◄── ipc::Response (nhị phân: báo cáo JSON + Bundle postcard)
   └─ wb-player (WASM) : Bundle → Engine → khung RGBA → canvas
```

- **`wb-compile`** (tách từ `wb-cli`): `prepare(project, ctx, cache) → Prepared { bundle, report }`; `compile(path)` cho CLI. `CompileContext` giải đường dẫn tương đối theo thư mục dự án và `builtin:…` theo thư mục `assets/` của app.
- **`wb_engine::Bundle`**: đầu vào của Engine ở dạng tuần tự hoá được; `into_engine(width)` tự giải Timeline. `fit_for_preview(960)` thu nhỏ lớp mực và ảnh tay cho Player (demo: 30 MB → 6,4 MB).
- **`wb-player`**: `new Player(bundle, width)`, `render(t)` trả con trỏ RGBA trong bộ nhớ WASM, `sceneStarts/sceneEnds` cho thanh tua.
- **Frontend** (`src/`): `store.ts` (zustand + zundo, lịch sử chỉ theo dõi tài liệu dự án), `api.ts` (lệnh Tauri), `player.ts`, `components/` (Stage, ScenePanel, Inspector, Timebar, ExportDialog). Sửa trong Inspector chỉ ghi khi rời ô/Enter → mỗi lần sửa một bước undo, không biên dịch lại theo từng phím.

## 3. Dùng thử

```bash
npm install
npm run build:wasm      # dựng src/wasm/pkg (cần wasm-bindgen-cli 0.2.130)
npm run tauri dev
```
"Dự án mới…" → chọn chỗ lưu thư mục `.wbproj` → "＋ Ảnh" / "＋ Chữ" → kéo/đổi cỡ trên khung → Space để xem → "Xuất video".
Ảnh nhập được chép vào `assets/` của dự án. Tay mặc định: `builtin:hands/pen-brown`, kiểu `wrist`.

**Xem giao diện trong trình duyệt** (không cần Tauri, lệnh Rust được giả lập):
```bash
cargo build -p wb-cli --profile bench-fast
target/bench-fast/wb bundle tests/fixtures/projects/demo-phase2.json -o target/preview-bundle.bin
npm run dev             # mở http://localhost:1420 → "Mở dự án…"
```

## 4. Kiểm chứng
- Test mới: `wb-project` 5, `wb-compile/tests/bundle.rs` 4 (gói khứ hồi khớp từng byte, gói gọn lệch < 2%, đệm không phân tích lại khi chỉ dời ảnh, `builtin:`).
- Golden Giai đoạn 1–2 **không đổi** sau khi tách `wb-compile` (kết quả render giữ nguyên).
- Giao diện đã thử trong trình duyệt (mock): mở dự án, chọn cảnh/element, kéo → X/Y cập nhật, Ctrl+Z hoàn tác, phát (bàn tay hiện), thêm chữ, mở hộp xuất video. App Tauri thật build và khởi chạy được; **chưa bấm thử trong cửa sổ app thật** (lưu, nhập ảnh, xuất qua hộp thoại hệ thống).
- Gỡ bỏ code spike Giai đoạn 0 (`spikes/`, `src/spike`, `src-tauri/src/spike.rs`) cùng 2 ngoại lệ `cargo deny` của rustybuzz/ttf-parser.

## 5. Việc còn lại của Giai đoạn 3
1. Icon (Lucide/Tabler) + mũi tên; snap & đường gióng.
2. Overlay thứ tự nét khi chọn element; keyframe snapshot để tua ngược nhanh.
3. i18n vi/en.
4. Gửi gói theo scene thay vì cả dự án khi dự án lớn.
5. Thử DoD với một người chưa từng dùng app.

## 6. Phần 2 (2026-11-15): màn hình chào, đóng cửa sổ, trình chỉnh bàn tay
- **Màn hình chào**: 3 lựa chọn — *Thử dự án mẫu* (tạo sẵn 2 cảnh dùng ảnh có sẵn `builtin:samples/…`, bấm ▶ xem ngay), *Dự án mới* (trống), *Mở dự án…*; danh sách **mở gần đây**; mục "Dự án là gì?" giải thích thư mục `.wbproj`. Nút ⌂ trên thanh công cụ để quay lại.
- **Đóng cửa sổ khi chưa lưu** → hỏi "Lưu rồi đóng" / "Đóng, không lưu" (không lưu thì xoá bản tự lưu).
- **Trình chỉnh bàn tay** (Inspector → "Quản lý / thêm bàn tay…"):
  - Thư viện = tay có sẵn của app (`builtin:hands/…`, chỉ đọc) + **tay của bạn** (`user:hands/…`, nằm trong thư mục dữ liệu app `%APPDATA%/io.github.whiteboardstudio.app/hands/`).
  - *Nhập ảnh tay…*: tự xoá nền đồng màu, cắt gọn, đoán mũi bút (`wb_compile::hands::import_hand`).
  - *Chấm mũi bút*, *Kẻ đường cổ tay* (2 điểm; vùng sẽ cắt được tô đỏ mờ), *Cao khi hiện*, *Phóng kiểu cả tay* → **Lưu** ghi `hand.json` → xem trước tự cập nhật.
  - Tay có sẵn: *Sao chép để chỉnh*. *Dùng cho dự án* đặt `settings.hand`.
- Test mới: `hands` (nhập ảnh/cắt gọn/đoán mũi bút, giữ trường lạ trong `hand.json`, tên thư mục bỏ dấu), dự án mẫu biên dịch được (`sample_project_compiles_from_builtin_assets`). Tổng 68 test.
- Đã thử trong trình duyệt (mock): màn hình chào, mở trình chỉnh tay, chọn tay, kẻ lại đường cổ tay. **Chưa thử trong app Tauri thật** (hộp thoại chọn file, lưu `hand.json`, hỏi khi đóng cửa sổ).
