# ADR-0013: Chữ viết tay = xương glyph + reveal, chỉ font OFL có tiếng Việt

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-14

## Bối cảnh
Font thường lưu *đường bao* chữ; đi theo đường bao thì tay vẽ vòng quanh mép chữ. Cần đường tâm + thứ tự viết tự nhiên + dấu thanh viết sau.

## Quyết định
Pipeline (`wb-text`, đã kiểm chứng ở S4):
1. Chuẩn hoá NFC → **kiểm tra font có đủ glyph** (thiếu → báo lỗi, không vẽ ô vuông).
2. Shaping bằng `rustybuzz`.
3. Rasterize từng glyph (em ≈ 220px) → thành phần liên thông → **phân loại dấu**: nằm hẳn trên 0,8×x-height hoặc dưới baseline, *và* glyph có thân chạm baseline (không xét kích thước).
4. Zhang–Suen + distance transform → nét có bán kính; **kéo dài đầu nét** theo tiếp tuyến; chấm nhỏ → "nét chấm".
5. Thứ tự `per_word`: hết thân chữ của từ (trái → phải, láng giềng gần nhất) rồi mới tới dấu.
6. Reveal glyph thật.

Bằng chứng (S4): Patrick Hand, Mali, Itim — 9/9 dòng đạt, dấu phát hiện đủ, độ phủ 99,5–99,97%.

## Phương án đã loại
- Đi theo outline: sai với font thường.
- Font một nét (Hershey…): ít kiểu, dấu tiếng Việt kém — giữ làm tuỳ chọn sau.
- Font viết tay của Windows (Segoe Print, Ink Free, Comic Sans): **thiếu glyph tiếng Việt**.

## Hệ quả
- Danh sách font đóng gói: bắt đầu từ Patrick Hand, Mali, Itim (OFL, `tests/fixtures/fonts/`).
- Dấu dính liền thân chữ (tuỳ font) không tách được — ghi chú hạn chế.
- Chiều nét trong chữ cái mới ở mức heuristic — cải thiện ở Giai đoạn 2.
