# ADR-0001: Render bằng Reveal Mask

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-14

## Bối cảnh
Hoạt ảnh bàn tay vẽ có 2 cách: (1) trích xuất nét rồi *vẽ lại* bằng đường vector; (2) dùng nét làm *mặt nạ* mở dần ảnh gốc. Cách (1) chỉ đẹp khi trích xuất hoàn hảo — không thực tế với ảnh đa dạng.

## Quyết định
Dùng **Reveal**: mỗi nét có đường tâm + bán kính `w` (từ distance transform). Khi tay đi qua, đoạn nét được "đóng dấu" lên **lớp tích lũy** bằng `tiny_skia::Pattern` lấy từ ảnh nguồn (không ghép mask cả khung mỗi frame). Bán kính đóng dấu = `w` + biên an toàn; đầu nét được kéo dài theo tiếp tuyến để bù phần Zhang–Suen gặm mất.

Bằng chứng: S2 — 1,5ms/khung (2.000 nét, WASM); test "mọi điểm mực đều được mở" đạt; S4 — độ phủ chữ ≥ 99,5%.

## Phương án đã loại
- Vẽ lại vector: kết quả cuối phụ thuộc chất lượng trích xuất.
- Ghép mask toàn khung mỗi frame: tốn công O(số pixel) mỗi frame thay vì O(phần nét mới).

## Hệ quả
- Ảnh hoàn thiện luôn giống 100% ảnh gốc.
- Tua ngược phải vẽ lại từ đầu → Player cần **keyframe snapshot** (mỗi ~2s / đầu Scene).
- Kiểu `stroke_render: vector` vẫn có thể thêm sau cho SVG/icon.
