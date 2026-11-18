# Bàn giao dự án Whiteboard Studio

> Cập nhật: 2026-11-18 · Nhánh làm việc `phase-1-core-engine` (đã gộp vào `main`).
> Dành cho phiên làm việc mới hoặc AI khác. Đọc kèm `AGENTS.md` (kiến trúc, lệnh) và `CLAUDE.md` (luật bắt buộc).

## 1. Dự án là gì
App desktop Windows tạo video **"bàn tay vẽ trên bảng trắng"**:
- Đầu vào: ảnh, chữ viết tay tiếng Việt, icon.
- Bàn tay vẽ lại từng nét. Giọng đọc (Azure) khớp với nét vẽ, có tiếng bút và phụ đề.
- Xuất MP4.
- Chạy trên máy, chỉ phần giọng đọc gọi mạng.
- Chủ dự án là sinh viên, nói tiếng Việt, dùng gói Azure for Students (Speech F0, vùng `japaneast`).

## 2. Đã có gì (tóm tắt — chi tiết ở `docs/TIEN-TRINH.md`)
| Giai đoạn | Trạng thái |
|---|---|
| 0. Nền móng, thử nghiệm kỹ thuật | ✅ |
| 1. Lõi vẽ + CLI (ảnh nét → MP4) | ✅ |
| 2. Chữ viết tay tiếng Việt, tô màu bằng cọ, SVG, 2 kiểu tay | ✅ |
| 3. App desktop: dự án `.wbproj`, editor, icon, bắt dính, trình chỉnh tay, giao diện "Giấy & Mực" | 🔄 gần xong |
| 4. Giọng đọc Azure + đồng bộ `[n]`, tiếng bút, phụ đề | 🔄 phần 1–2 xong |
| 5–8. Studio nâng cao, AI ảnh khó, AI đạo diễn, phát hành | ⏳ |

**Dự án mẫu** có sẵn trên màn hình chào (`crates/wb-project/src/sample*.json`, danh sách `SAMPLES`):
1. *Bài học đầu tiên*: chữ, bóng đèn, ngôi nhà tô màu.
2. *3 bước tiết kiệm tiền*: icon đổi màu, mũi tên, lời thoại theo từng bước.
3. *Vòng tuần hoàn của nước*: sơ đồ 8 hình vẽ theo lời, bút chì, giọng nam.

## 3. Luồng làm việc của người dùng
1. Mở app. Chọn một dự án mẫu, hoặc **Dự án mới** (chọn nơi lưu, app tạo thư mục `.wbproj`).
2. Thêm cảnh (bảng trái). Thêm **Ảnh / Chữ / Icon**, kéo thả (có bắt dính), chỉnh ở bảng phải (Inspector).
3. Gõ **lời thoại** cho từng cảnh. Dấu `[n]` là hình số n bắt đầu vẽ đúng lúc đọc tới.
4. Bấm **Tạo giọng đọc**: lần đầu nhập key Azure, key được cất trong Windows Credential Manager.
5. Bấm ▶ để xem trước (có tiếng), rồi **Xuất video**: ra MP4 kèm phụ đề nhúng và file `.srt`.

## 4. Việc tiếp theo (theo thứ tự ưu tiên)
1. **Thử DoD bản MVP (Giai đoạn 4):** video 2 phút, 8 cảnh, có giọng; hình bắt đầu vẽ lệch ≤ 150 ms so với dấu `[n]`; xuất 1080p30 có tiếng bút và phụ đề; chạy được trên 2 máy Windows.
2. **Thanh thời gian (timeline):** sóng âm giọng đọc, thấy và kéo được các dấu `[n]`, cảnh báo khi hình vào trễ so với lời (mốc quá sát).
3. **Nhập file thu âm** thay giọng máy, kèm Whisper alignment (chạy offline, crate `whisper-rs`, giấy phép phải kiểm).
4. Phần còn lại của Giai đoạn 3:
   - hiện thứ tự nét khi chọn ảnh;
   - keyframe để tua ngược nhanh;
   - giao diện tiếng Anh (i18n);
   - gói theo cảnh khi dự án lớn;
   - nhờ một người chưa từng dùng thử tự làm video 3 cảnh.
5. Giai đoạn 5:
   - Order Brush (tự chọn vùng vẽ trước/sau);
   - camera pan/zoom;
   - chuyển cảnh;
   - nhiều kiểu tay;
   - mẫu bố cục.
6. Giai đoạn 6–8:
   - AI cho ảnh chụp (tách nền, ảnh → nét; model tải về, không đóng gói sẵn);
   - AI viết kịch bản và dàn dựng (`wb-dsl`, `wb-ai`, key trong keyring);
   - đóng gói, phát hành.

## 5. Quyết định đã chốt (đừng làm lại)
- Tách xương nét bằng **Guo–Hall** (ADR-0020). Font mặc định **Patrick Hand**: font Windows thiếu dấu tiếng Việt, và font này cũng không có các ký tự như `→`.
- Bàn tay: kiểu **tới cổ tay** là mặc định, chừa chút cẳng tay. **Không kéo dài tay giả**: chủ dự án đã từ chối.
- **Giọng đọc = Azure AI Speech** qua WebSocket chính thức + key của người dùng (ADR-0012). **Không dùng Edge-TTS**.
- Gói dữ liệu cho Player là nhị phân (postcard), không dùng JSON (ADR-0002, ADR-0022). Xem trước và xuất video dùng chung một Engine.
- Giao diện 5 màu "Giấy & Mực": skill `.claude/skills/whiteboard-ui/`. Chủ dự án có thể chỉnh lại sau khi dùng thực tế.
- Repo GitHub **công khai**. Ảnh tay và ảnh test do chủ dự án tự tạo bằng AI, được phép push. Video, ảnh test sinh ra thì không push (để trong `not_push_git/` hoặc xoá).

## 6. Cách làm việc với chủ dự án
- Trả lời **tiếng Việt**, giải thích đã làm gì và vì sao. Giao diện phải dễ hiểu.
- **Không tự commit/push**. Viết nội dung commit vào `not_push_git/commit-msg.txt`, rồi đưa lệnh để chủ dự án tự chạy:
  ```bash
  git add -A
  git commit -F not_push_git/commit-msg.txt
  git push origin phase-1-core-engine
  git checkout main
  git merge --ff-only phase-1-core-engine
  git push origin main
  git checkout phase-1-core-engine
  ```
- Không xin key và không đọc file `.env`. Key nằm trong `.env` (đã gitignore) hoặc trong Credential Manager.
- Mỗi bước xong:
  - cập nhật `docs/TIEN-TRINH.md` (ngắn) và `docs/giai-doan-N/README.md`;
  - nếu có quyết định mới thì ghi thêm ADR.
- Tắt server dev (cổng 1420) trước khi kết thúc lượt, để `npm run tauri dev` của chủ dự án chạy được.

## 7. Câu mở đầu gợi ý cho phiên mới
> Đọc `AGENTS.md`, `CLAUDE.md` và `docs/BAN-GIAO.md`, rồi làm việc tiếp theo ở mục 4. Giải thích bằng tiếng Việt, đừng tự commit.
