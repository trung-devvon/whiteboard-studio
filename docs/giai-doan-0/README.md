# Giai đoạn 0 — Nền móng & Spike kiểm chứng

**Mục tiêu:** dập tắt các rủi ro kỹ thuật lớn nhất *trước khi* viết code thật, và dựng sẵn bộ khung dự án (workspace, CI, kiểm tra giấy phép, sinh kiểu TypeScript).
Kết quả chi tiết: [`ket-qua-spike.md`](ket-qua-spike.md).

## 1. Đã làm những gì

| Hạng mục | Ở đâu | Trạng thái |
|---|---|---|
| Cargo workspace 11 crate `wb-*` (khung rỗng) | `Cargo.toml`, `crates/` | ✅ |
| Kiểu dữ liệu đầu tiên (nét, DrawPlan) + test | `crates/wb-model` | ✅ |
| Sinh kiểu TypeScript từ Rust (specta) | `npm run gen:bindings` → `src/bindings/model.ts` | ✅ |
| App Tauri đổi tên + đưa vào workspace | `src-tauri/` | ✅ |
| Kiểm tra giấy phép thư viện ("Giữ cửa mở") | `deny.toml` → `cargo deny check` | ✅ licenses ok |
| Định dạng code, lint | `rustfmt.toml`, clippy `-D warnings` | ✅ sạch |
| CI GitHub Actions | `.github/workflows/ci.yml` | ✅ viết xong, chưa chạy (chưa có repo GitHub) |
| ADR (nhật ký quyết định) | `docs/adr/` | ✅ |
| Ghi nguồn tài nguyên | `assets/CREDITS.md` | ✅ |
| 5 spike | `spikes/` | S1 S2 S4 S5 ✅ · S3 ⛔ cần quyết định |

## 2. Cài đặt môi trường (máy mới)

Cần có: Rust (stable), Node 20+, Git. Sau đó:

```bash
rustup target add wasm32-unknown-unknown
```
```bash
cargo install cargo-deny --locked
```
```bash
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```
```bash
npm install
```

**FFmpeg** (không commit vào git — thư mục `tools/` đã gitignore): tải bản **win64 LGPL** (ví dụ `ffmpeg-n9.0-latest-win64-lgpl-9.0.zip`) từ GitHub Releases của dự án FFmpeg-Builds (BtbN), giải nén vào `tools/ffmpeg/`. Các spike tự tìm `tools/ffmpeg/*/bin/ffmpeg.exe`, hoặc đặt biến `WB_FFMPEG`.

## 3. Lệnh hay dùng

| Việc | Lệnh |
|---|---|
| Chạy app (dev) | `npm run tauri dev` |
| Test toàn bộ Rust | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Kiểm tra giấy phép | `cargo deny check` |
| Sinh lại kiểu TS | `npm run gen:bindings` |
| Dựng gói WASM cho Spike Lab | `npm run build:wasm` |
| Spike S1 | `cargo run -p s1-ffmpeg-pipe --profile bench-fast` |
| Spike S4 | `cargo run -p s4-glyph-skeleton --profile bench-fast tests/fixtures/fonts/Mali-Regular.ttf` |
| Spike S2+S5 tự động | `WB_SPIKE_AUTORUN=1 npm run tauri dev` (PowerShell: `$env:WB_SPIKE_AUTORUN=1; npm run tauri dev`) |

Đầu ra spike (video, ảnh debug, báo cáo) nằm trong `spikes/out/` (gitignore).

## 4. Code spike là "code thử" — dọn dẹp khi nào?

- `spikes/*` và `src/spike/`, `src-tauri/src/spike.rs`: **giữ lại làm tài liệu tham khảo** trong Giai đoạn 1–2 (thuật toán S4 sẽ được viết lại có test trong `wb-vision`/`wb-text`, kỹ thuật reveal S2 vào `wb-engine`, pipe S1 vào `wb-export`).
- **Xoá ở đầu Giai đoạn 3** (khi làm Editor thật) — xem các chú thích `TẠM` trong code.
