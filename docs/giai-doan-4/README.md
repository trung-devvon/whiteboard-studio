# Giai đoạn 4 — Giọng đọc & đồng bộ (phần 1)

> Nhánh: `phase-1-core-engine` · Quyết định: [ADR-0012](../adr/0012-nguon-giong-doc.md) · Kế hoạch: `not_push_git/06-…` (Giai đoạn 4)

## 1. Đã làm (2026-11-18)
| Hạng mục | Trạng thái |
|---|---|
| Lời thoại cho từng cảnh (`Scene.narration`), dấu `[n]` = element n bắt đầu vẽ đúng lúc đọc tới | ✅ |
| Giọng đọc của dự án (`settings.voice`: `vi-VN-HoaiMyNeural` mặc định, tốc độ 50–200%) | ✅ |
| Crate `wb-audio`: client Azure (WebSocket chính thức), SSML + bookmark, mốc từng từ, WAV, trộn tiếng, đệm `generated/tts/` | ✅ 10 test (1 test gọi Azure thật, chạy bằng `--ignored`) |
| `wb-timing`: neo element vào mốc lời thoại, co thời lượng các element trước cho kịp, scene dài ít nhất bằng giọng đọc | ✅ |
| Xuất MP4 có tiếng (AAC) — app và CLI | ✅ |
| Xem trước có tiếng (Web Audio, phát/dừng/tua theo Player) | ✅ |
| Cài đặt key: kho mật khẩu Windows (`keyring`), thử key trước khi lưu, nghe thử giọng | ✅ |
| CLI `wb tts <project.json>` | ✅ |
| Dự án mẫu có sẵn lời thoại 2 cảnh | ✅ |
| Marker kéo được trên waveform, timeline UI | ⏳ |
| Tiếng bút (SFX), phụ đề SRT, chuẩn hoá âm lượng | ⏳ |
| Import file audio + Whisper alignment (đường offline) | ⏳ |

## 2. Dùng thử
1. Mở app → *Thử dự án mẫu* (đã có lời thoại 2 cảnh) hoặc gõ lời thoại ở bảng bên phải khi chọn một cảnh.
2. Bấm **Tạo giọng đọc**. Lần đầu chưa có key → hộp *Cài đặt giọng đọc*: dán KEY 1 + vùng (`japaneast`) → *Lưu key*
   (app đọc thử "Xin chào." để kiểm tra key, đúng mới lưu). Khi phát triển có thể dùng `.env` thay thế:
   ```text
   WB_AZURE_SPEECH_KEY=...
   WB_AZURE_SPEECH_REGION=japaneast
   ```
3. Bấm ▶: nghe giọng, element số 2 bắt đầu vẽ đúng lúc đọc tới `[2]`. *Xuất video* → MP4 có tiếng.
4. CLI: `wb tts project.json` rồi `wb render project.json -o out.mp4`.

## 3. Cách đồng bộ
```text
"[1] Chào mừng… [2] Mọi ý tưởng…"  →  SSML <bookmark mark="e1"/>…<bookmark mark="e2"/>
Azure trả: âm thanh PCM 24 kHz + Bookmark{e2 @ 2375 ms} + WordBoundary từng từ
wb-timing: element 2 vẽ từ (đầu cảnh + 2375 ms); element 1 co lại cho kịp (≥ 400 ms/lớp)
cảnh kéo dài ≥ độ dài giọng + 400 ms; rãnh tiếng = giọng mỗi cảnh đặt đúng đầu cảnh
```
- Mốc quá sát (vd. ngôi nhà 2 lớp nhưng `[2]` chỉ sau 1,65 s) → element sau vào trễ một chút thay vì vẽ nhanh vô lý.
- Đổi lời, giọng hay tốc độ → khoá đệm mới → cảnh đó báo "Chưa tạo giọng"; giọng cũ không bị gọi lại.

## 4. Kiểm chứng
- Gọi Azure thật (HoaiMy, `japaneast`): câu 5,3 s, 13 mốc từ, `[1]` = 1138 ms, `[2]` = 2975 ms.
- Dự án mẫu → `wb tts` (151 ký tự, chạy lại: 0 ký tự nhờ đệm) → `wb render`: MP4 15,8 s, H.264 + AAC; cảnh 1 element 2 đúng mốc 2375 ms.
- Test `narration_markers_anchor_elements_and_build_audio_track` (giọng giả lập, không mạng): element 2 đúng mốc ±1 ms ở cả 2 cảnh, kể cả khi element 1 có 2 lớp.
- Golden Giai đoạn 1–2 không đổi. Tổng 79 test; clippy, tsc, `cargo deny`, kiểm tra màu sạch.
- Giao diện thử trong trình duyệt (mock): ô lời thoại, trạng thái, nút tạo giọng, hộp cài đặt. **Chưa thử trong app Tauri thật** (lưu key vào Credential Manager, phát tiếng, xuất có tiếng qua app).
