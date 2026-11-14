# Kết quả 5 spike kiểm chứng — Giai đoạn 0

> Ngày chạy: 2026-11-14 · Máy: Intel i5-14600KF, 32GB RAM, NVIDIA RTX 5060 Ti, Windows 11, WebView2 (Edge 157)
> Kế hoạch gốc: `not_push_git/06-lo-trinh-va-tieu-chi-hoan-thanh.md` (Giai đoạn 0)
> Code spike: `spikes/` (thử nghiệm, sẽ được viết lại "sạch" trong các crate `wb-*` ở Giai đoạn 1)

## Tóm tắt

| Spike | Câu hỏi | Kết quả | Hệ quả |
|---|---|---|---|
| **S1** FFmpeg pipe | Rust → stdin FFmpeg LGPL → MP4, màu đúng? | ✅ **ĐẠT** — 6–8× thời gian thực, màu lệch 1/255 | ADR-007 chốt |
| **S2** WASM preview | tiny-skia trong WASM ≥ 30fps @960×540? | ✅ **ĐẠT vượt xa** — 1,5ms/khung (2.000 nét) | ADR-002 chốt |
| **S3** Edge-TTS | Lấy audio + WordBoundary tiếng Việt? | ⛔ **DỪNG** — rủi ro R2 đã xảy ra (xem bên dưới) | ADR-012 cần quyết định lại |
| **S4** Xương glyph | Viết tiếng Việt, dấu sau, reveal phủ kín? | ✅ **ĐẠT 9/9** trên 3 font OFL | ADR-013 chốt |
| **S5** IPC nhị phân | Gửi 5MB DrawPlan qua Tauri < 50ms? | ✅ **ĐẠT** — truyền ~27ms; JSON chậm 4,7× | ADR-002 (phần IPC) chốt |

---

## S1 — FFmpeg pipe (`spikes/s1-ffmpeg-pipe`)

Chạy: `cargo run -p s1-ffmpeg-pipe --profile bench-fast` · Video mẫu 10s 1080p30 (300 khung), ô màu kiểm tra + đường xoắn vẽ dần.

| Encoder | Kết quả | Tốc độ tổng | Ghi chú |
|---|---|---|---|
| `h264_nvenc` (GPU NVIDIA) | ✅ | 243 fps (8,1× thời gian thực) | Mặc định đề xuất khi có GPU NVIDIA |
| `libopenh264` (CPU, BSD) | ✅ | 235 fps (7,8×) | **Dự phòng phổ quát** — chạy mọi máy |
| `h264_mf` (Media Foundation, phần mềm) | ✅ | 209 fps (7,0×) | Có sẵn trong Windows |
| `h264_mf` (phần cứng) | ❌ | — | "format negotiation failed" — cần đầu vào nv12, để sau |
| `h264_amf` / `h264_qsv` | ❌ | — | Máy không có GPU AMD / iGPU Intel — đúng như dự kiến |

- Kiểm tra tự động: đủ 300 khung, ffprobe báo `yuv420p, bt709, bt709, bt709`, màu 6 ô mẫu sau khi giải mã lệch **tối đa 1/255**.
- **Phát hiện:** openh264 và Media Foundation **bỏ qua** cờ `-color_primaries/-color_trc` → phải thêm bitstream filter `h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1` để ghi thông tin màu vào luồng H.264. Đã đưa vào `spike-common`.
- FFmpeg dùng: build **n9.0 LGPL** (`--enable-version3` → LGPL v3), gọi dạng tiến trình riêng → phù hợp chính sách "Giữ cửa mở".
- ⚠ Nội dung test đơn giản; video thật (ảnh lớn, nhiều lớp) sẽ chậm hơn — đo lại ở Giai đoạn 1.
- **Chiến lược chọn encoder (đề xuất):** thử theo thứ tự `nvenc → amf → qsv → libopenh264`, nhớ kết quả trong cài đặt.

## S2 — WASM preview (`spikes/s2-wasm-preview` + `src/spike/SpikeLab.tsx`)

Chạy tự động: `WB_SPIKE_AUTORUN=1 npm run tauri dev` · Đo trong **WebView2 thật** của app.

| Số nét | TB/khung | p95 | Max | Phát thật (rAF) | Tua ngược về giữa |
|---|---|---|---|---|---|
| 2.000 | **1,5 ms** | 2,2 ms | 5,3 ms | 200 fps* | 94 ms |
| 10.000 | **3,7 ms** | 4,1 ms | 4,7 ms | 200 fps* | 446 ms |

\* bị giới hạn bởi tần số quét màn hình. Ngưỡng cần đạt: ≤ 33 ms/khung (30fps) → **dư ~20 lần**.

- **Kỹ thuật quyết định tốc độ:** không ghép mask cả khung mỗi frame; thay vào đó "đóng dấu" đoạn nét mới lên lớp tích lũy bằng **Pattern shader lấy từ ảnh nguồn** (`tiny_skia::Pattern`). Chỉ tốn công ở vùng nét mới. Đưa vào thiết kế `wb-engine` (ADR-001).
- Unit test: ở tiến độ 100%, **mọi điểm mực của ảnh nguồn đều đã được mở** (0 điểm sót).
- **Phát hiện:** tua ngược phải vẽ lại từ đầu (94–446ms) → Player cần **ảnh chụp trạng thái (keyframe snapshot)** mỗi ~2 giây hoặc mỗi đầu Scene để tua mượt. Thêm vào Giai đoạn 3.
- Kích thước WASM: ~480KB (chưa tối ưu, chưa `wasm-opt`).

## S3 — Edge-TTS ⛔ DỪNG CÓ CHỦ ĐÍCH

**Phát hiện:** khi đọc mã nguồn hiện tại của thư viện `edge-tts` (bản tham chiếu phổ biến nhất), Microsoft đã nâng cơ chế bảo vệ lên token **`Sec-MS-GEC` phiên bản 2**. Để gọi được, client phải sinh token bằng một **thuật toán SHA-256 bị chỉnh sửa và làm rối (obfuscated)**, với các hằng số được dịch ngược từ trình duyệt Edge.

**Vì sao dừng:**
1. Đây là **cơ chế kiểm soát truy cập** mà Microsoft cố ý dựng lên. Tự viết lại nó trong app = duy trì code vượt rào DRM → rủi ro pháp lý/điều khoản, đặc biệt nếu sau này open source hoặc bán (trái chính sách "Giữ cửa mở").
2. Microsoft đã đổi cơ chế ít nhất 2 lần → mỗi lần đổi, tính năng giọng đọc của app **chết** cho tới khi có bản vá.
3. Đúng rủi ro **R2** trong sổ rủi ro (khả năng 2/3 → giờ là **đã xảy ra**).

**Phương án thay thế (cần bạn chọn — xem `docs/adr/0012-nguon-giong-doc.md`):**

| Phương án | Chính thức? | Timestamp | Ghi chú |
|---|---|---|---|
| **Google Cloud TTS** | ✅ | `<mark name="x"/>` trong SSML → trả thời điểm từng mark (API v1beta1, `enableTimePointing`) | **Khớp hoàn hảo với thiết kế marker `{x}`** — không cần căn từng từ. Có giọng vi-VN. Cần tài khoản Google Cloud + API key, có hạn mức miễn phí hàng tháng |
| **Azure AI Speech** | ✅ | WordBoundary qua Speech SDK/WebSocket chính thức, hoặc Batch Synthesis | **Cùng giọng HoaiMy/NamMinh** như Edge. Có gói miễn phí. Không có SDK Rust chính thức |
| **ElevenLabs** | ✅ | API `with-timestamps` trả mốc từng ký tự | Giọng rất tự nhiên, trả phí |
| **File audio + Whisper alignment** | ✅ offline | Căn văn bản đã biết vào audio | Không cần key/mạng; chạy với **bất kỳ** nguồn audio nào (thu âm, Vbee, FPT.AI…) → cần spike riêng (S3b) |

## S4 — Xương glyph tiếng Việt (`spikes/s4-glyph-skeleton`)

Chạy: `cargo run -p s4-glyph-skeleton --profile bench-fast <font.ttf>` → ảnh debug + video demo trong `spikes/out/`.

| Font (OFL) | "Tiếng Việt" | "Chào mừng các bạn" | "Học hỏi chăm chỉ" |
|---|---|---|---|
| Patrick Hand | ✅ dấu 6/6 · phủ 99,52% | ✅ 4/4 · 99,81% | ✅ 5/5 · 99,89% |
| Mali | ✅ 6/6 · 99,93% | ✅ 4/4 · 99,81% | ✅ 5/5 · 99,97% |
| Itim | ✅ 6/6 · 99,68% | ✅ 4/4 · 99,97% | ✅ 5/5 · 99,95% |

Mọi dòng: **nét dấu luôn đi sau thân chữ** trong cùng từ (chế độ `per_word`). Thời gian xử lý 80–230ms/dòng (chưa tối ưu, chưa song song).

**Phát hiện quan trọng:**
1. **Font viết tay có sẵn của Windows không dùng được cho tiếng Việt** — Segoe Print, Ink Free, Comic Sans đều **thiếu glyph** ế, ệ, ừ, ạ, ọ, ỏ, ỉ (hiện ô vuông). → Engine phải **kiểm tra font đủ ký tự** trước khi dùng và báo lỗi rõ; chỉ đóng gói font OFL có subset Vietnamese.
2. **Quy tắc tách dấu** đúng: một thành phần là dấu khi nó *nằm hẳn* trên ~0,8×x-height (hoặc dưới baseline) **và** glyph có phần thân chạm baseline — **không** dựa vào kích thước (dấu hỏi trên "ỉ" to gần bằng thân chữ i).
3. **Chữ i có dấu (ì í ỉ ĩ) không còn chấm** — dấu thay chỗ chấm; riêng **ị** vẫn giữ chấm trên.
4. **Zhang–Suen luôn "gặm" đầu nét** một đoạn ≈ bán kính nét → phải **kéo dài hai đầu nét** theo tiếp tuyến (độ phủ tăng từ ~98,5% lên ≥ 99,5%).
5. **Chấm nhỏ** (chấm i, dấu nặng) có xương chỉ 1 điểm → phải sinh "nét chấm" riêng.
6. Hạn chế đã biết: nếu font vẽ dấu **dính liền** thân chữ, dấu sẽ bị coi là thân chữ (không tách được) — chấp nhận, ghi chú cho người dùng.

## S5 — IPC nhị phân (`src-tauri/src/spike.rs`)

| DrawPlan | Nhị phân (postcard) | JSON | Nạp vào WASM** |
|---|---|---|---|
| 2.000 nét — 0,49 MB (JSON 1,8 MB) | **10,5 ms** | 45 ms | 88 ms |
| 20.000 nét — 4,92 MB (JSON 17,9 MB) | **93 ms** (trong đó ~66 ms là *sinh dữ liệu* ở bản debug) → **truyền ≈ 27 ms** | 439 ms | 793 ms |
| 80.000 nét — 19,7 MB (JSON 71,6 MB) | 375 ms | 1.799 ms | 3.246 ms |

\*\* gồm giải mã + dựng ảnh nguồn bằng cách vẽ lại toàn bộ nét (chỉ có ở spike — app thật dùng ảnh của người dùng).

- **Kết luận:** nhị phân nhanh hơn JSON **~4,7 lần**, nhỏ hơn **~3,6 lần** → DrawPlan luôn đi qua `ipc::Response` (ADR-002).
- **Phát hiện phụ (sinh kiểu TS):** `specta` xuất `f32` thành `number | null` (vì JSON không biểu diễn được NaN/Infinity). Giai đoạn 1 cần quyết định: dùng kiểu số "hữu hạn" riêng, hoặc chấp nhận và kiểm tra ở frontend.

---

## Việc Giai đoạn 0 còn lại (không chặn Giai đoạn 1)

- [ ] **Chọn nguồn giọng đọc** thay Edge-TTS (ADR-012) → spike S3' với provider được chọn (cần API key) và/hoặc S3b (Whisper alignment, offline).
- [ ] **Bộ ảnh test 50 tấm** (A15 · B10 · C10 · D5 · E10) — cần bạn thu thập (ảnh tự vẽ/tự chụp/CC0), xem `tests/fixtures/README.md`.
- [ ] Đẩy repo lên GitHub (private) để CI chạy lần đầu.
