# Tiến trình Whiteboard Studio

> Cập nhật: 2026-11-18 · Nhánh `phase-1-core-engine` · Chi tiết từng giai đoạn: `docs/giai-doan-*/README.md`

Ứng dụng desktop (Rust + Tauri) tạo video "bàn tay vẽ trên bảng trắng" từ ảnh và chữ, chạy hoàn toàn trên máy.

## 1. Đã làm được gì

| Giai đoạn | Nội dung | Trạng thái |
|---|---|---|
| 0 — Nền móng | Workspace, CI, kiểm tra giấy phép, 5 thử nghiệm kỹ thuật | ✅ |
| 1 — Lõi vẽ + dòng lệnh | Ảnh nét → bàn tay vẽ lại → xuất MP4 (FFmpeg) | ✅ |
| 2 — Chữ, màu, SVG | Viết tay tiếng Việt có dấu, tô màu bằng cọ, vẽ theo nét SVG, 2 kiểu tay | ✅ |
| 3 — Ứng dụng desktop | Editor đầy đủ (xem bên dưới) | 🔄 phần lớn xong |
| 4 — Giọng đọc & đồng bộ (MVP) | Giọng đọc, khớp nét vẽ với lời, tiếng bút, phụ đề | 🔄 phần 1 xong: giọng Azure + đồng bộ + video có tiếng |
| 5–8 | Studio nâng cao, AI xử lý ảnh khó, AI đạo diễn, phát hành | ⏳ |

## 2. App hiện có những tính năng gì

**Dự án**
- Màn hình chào gồm 3 lựa chọn:
  - "Thử dự án mẫu": tạo sẵn 2 cảnh để xem ngay;
  - "Dự án mới";
  - "Mở dự án".
- Danh sách mở gần đây và phần giải thích "dự án là gì".
- Mỗi dự án là một thư mục `.wbproj`. Lưu an toàn (không hỏng file khi mất điện), tự lưu mỗi 4 giây, hỏi khôi phục nếu lần trước app tắt đột ngột.
- Đóng cửa sổ khi chưa lưu thì app hỏi trước.

**Soạn cảnh**
- Thêm, xoá, đổi tên và sắp xếp cảnh. Mỗi cảnh có màu nền và thời gian giữ hình cuối riêng.
- Thêm **ảnh** (PNG/JPG/SVG), **chữ viết tay** (tiếng Việt có dấu, viết dấu sau như người thật) và **icon** (2.359 icon Lucide, tìm bằng tiếng Việt, có mũi tên).
- Kéo, đổi cỡ, bắt dính vào mép hoặc giữa khung và vào đối tượng khác, có đường gióng (giữ Alt để tắt). Đổi thứ tự vẽ.
- **Đổi màu nét** cho icon và ảnh nét (mới, xem mục 4).
- Undo/redo không giới hạn, phím tắt (Ctrl+Z/Y/S, Delete, Space…).

**Cách vẽ**
- Ảnh nét: bàn tay đi theo đúng từng nét. Ảnh màu: phác viền trước rồi tô màu bằng cọ. SVG/icon: dùng thẳng đường nét gốc.
- Bàn tay có 2 kiểu: "tới cổ tay" (mặc định) và "cả cánh tay". Có 3 bộ tay sẵn: bút nâu, bút chì xanh, bút chì tím.
- **Trình chỉnh bàn tay**: nhập ảnh tay của bạn (app tự xoá nền), chấm mũi bút, kẻ đường cổ tay, xem trước.

**Giọng đọc** (mới)
- Mỗi cảnh có ô **lời thoại**. Gõ `[1]`, `[2]`… để đối tượng số 1, 2 bắt đầu vẽ đúng lúc đọc tới chữ đó.
- Giọng **HoaiMy** (nữ, mặc định) hoặc **NamMinh** (nam), chỉnh tốc độ đọc. Giọng do Azure tạo bằng key của bạn.
- Key cất trong kho mật khẩu của Windows. Giọng đã tạo được lưu trong dự án, nên không tốn hạn mức khi mở lại.
- Xem trước có tiếng; video xuất ra có tiếng.

**Xem và xuất**
- Xem trước ngay trong app (Player chạy WebAssembly), có thanh tua và nút phát/dừng.
- Xuất MP4 có thanh tiến độ, huỷ giữa chừng và mở thư mục chứa video. Dùng card đồ hoạ (NVENC) nếu có. Video xuất khớp với bản xem trước.

**Giao diện**
- Phong cách "Giấy & Mực": tối giản, đúng 5 màu (trắng, be, cát, mực, đất nung), chữ luôn rõ.
- Quy tắc nằm trong `.claude/skills/whiteboard-ui/`, kèm script tự kiểm tra màu.

## 3. Những gì đã đổi so với kế hoạch ban đầu

| Ban đầu | Hiện tại | Lý do |
|---|---|---|
| Giọng đọc Edge-TTS (miễn phí) | **Azure AI Speech** gói F0 miễn phí (tài khoản sinh viên), giọng HoaiMy | Edge-TTS cần token chống sao chép (DRM), dùng là vi phạm |
| Tách xương nét bằng Zhang–Suen | **Guo–Hall** | Zhang–Suen làm mất nét chéo |
| Font viết tay có sẵn của Windows | Đóng gói **Patrick Hand** | Font Windows thiếu dấu tiếng Việt |
| Kéo dài cánh tay khi tay ngắn | **Không kéo dài**. Thêm kiểu "tới cổ tay" (cắt ở cổ tay, chừa chút cẳng tay) và đặt làm mặc định | Bạn thấy kéo dài giả trông không thật |
| Bàn tay chỉ là ảnh cầm bút | Thêm **bút chì không có tay** làm một lựa chọn | Theo yêu cầu của bạn |
| Mở app là vào thẳng mở/tạo dự án | Thêm **màn hình chào** có dự án mẫu và lời giải thích | Bạn thấy "mở/tạo dự án" khó hiểu |
| Icon Lucide + Tabler | Mới có **Lucide**, thêm tìm bằng tiếng Việt | Đủ dùng; Tabler để sau nếu cần |
| Giao diện mặc định (xanh dương/xám) | Phong cách **"Giấy & Mực" 5 màu** | Theo yêu cầu của bạn |
| Repo GitHub riêng tư | Repo **công khai** | Bạn chọn; ảnh tay/test là ảnh bạn tự tạo bằng AI |

## 4. Đang ở đâu, tiếp theo là gì

**Vừa xong:** Giai đoạn 4, phần 1 — giọng đọc và đồng bộ nét vẽ theo lời. Chi tiết: `docs/giai-doan-4/README.md`.

**Còn lại của Giai đoạn 4:**
- Tiếng bút khi vẽ.
- Phụ đề SRT.
- Thanh thời gian có sóng âm, kéo được các mốc `[n]`.
- Nhập file thu âm của bạn thay cho giọng máy.

**Còn lại của Giai đoạn 3:**
- Hiện thứ tự nét khi chọn ảnh.
- Tua ngược nhanh hơn.
- Giao diện tiếng Anh.
- Thử với người lạ.

**Quan trọng nhất:** bấm thử trong app thật (`npm run tauri dev`): nhập key, tạo giọng, nghe, xuất video có tiếng.
