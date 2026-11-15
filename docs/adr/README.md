# Nhật ký quyết định kiến trúc (ADR)

Mỗi quyết định lớn = một file ngắn `NNNN-ten.md`: **Bối cảnh → Quyết định → Phương án đã loại → Hệ quả**.
Đổi ý thì tạo ADR mới "thay thế ADR-xxxx", không sửa nội dung ADR cũ (chỉ cập nhật dòng Trạng thái).
Mẫu: [`0000-mau.md`](0000-mau.md).

| ADR | Quyết định | Trạng thái |
|---|---|---|
| 0001 | Render bằng **Reveal Mask** (đóng dấu nét bằng pattern ảnh nguồn) | ✅ Chấp nhận — [0001](0001-render-reveal.md) |
| 0002 | **Một engine Rust** cho Preview (WASM) và Export (native); DrawPlan qua IPC nhị phân | ✅ Chấp nhận — [0002](0002-mot-engine-hai-dich.md) |
| 0003 | react-konva cho Editor | Đề xuất (chốt ở GĐ 3) |
| 0004 | Cargo workspace nhiều crate; `wb-engine` không I/O | ✅ Chấp nhận (đã dựng) |
| 0005 | Thời gian = ms số nguyên; toạ độ = px khung 1920×1080 | ✅ Chấp nhận (dùng trong `wb-model` v1) |
| 0006 | Dự án = thư mục `*.wbproj` | Đề xuất |
| 0007 | **FFmpeg sidecar bản LGPL**, thứ tự encoder nvenc → amf → qsv → openh264 | ✅ Chấp nhận — [0007](0007-ffmpeg-lgpl-sidecar.md) |
| 0008 | Kiểu TS sinh tự động bằng specta | ✅ Chấp nhận (đã chạy) |
| 0009 | Đồng bộ bằng marker trong lời thoại | Đề xuất |
| 0010 | Mặc định 30fps | ✅ Chấp nhận (= 0016) |
| 0011 | Model ONNX tải khi cần | Đề xuất |
| 0012 | **Nguồn giọng đọc** — Edge-TTS bị loại | ⏳ **Cần quyết định** — [0012](0012-nguon-giong-doc.md) |
| 0013 | Chữ = **xương glyph + reveal**, chỉ font OFL có tiếng Việt | ✅ Chấp nhận — [0013](0013-chu-xuong-glyph.md) |
| 0014 | Trộn âm thanh trong Rust | Đề xuất |
| 0015 | Chính sách giấy phép "Giữ cửa mở" | ✅ Chấp nhận (`deny.toml`) |
| 0016 | 1080p 30fps mặc định | ✅ Chấp nhận |
| 0017 | Windows trước, macOS ở GĐ 8 | ✅ Chấp nhận |
| 0018 | AI: key người dùng + OpenAI-compatible cho AI local | ✅ Chấp nhận |
| 0019 | Bàn tay: 1 ảnh chụp + 1 SVG + Trình chỉnh bàn tay | ✅ Chấp nhận |
| 0020 | Rút xương bằng **Guo–Hall** thay Zhang–Suen (giữ nét chéo 45°) | ✅ Chấp nhận — [0020](0020-guo-hall-thinning.md) |
| 0021 | Element **nhiều lớp vẽ** + "tô nét" bằng cọ cho ảnh màu / ảnh chụp | ✅ Chấp nhận — [0021](0021-lop-ve-va-to-net.md) |
| 0022 | **Gói biên dịch (Bundle) + Player WASM**; dự án là thư mục `.wbproj` (ghi atomic, autosave) | ✅ Chấp nhận — [0022](0022-goi-bien-dich-va-player-wasm.md) |

Chi tiết các ADR "Đề xuất" hiện nằm trong kế hoạch (`not_push_git/07-rui-ro-giay-phep-va-quyet-dinh.md`) và sẽ được tách thành file khi chốt.
