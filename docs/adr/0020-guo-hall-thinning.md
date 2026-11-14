# ADR-0020: Rút xương bằng Guo–Hall thay cho Zhang–Suen

- **Trạng thái:** Chấp nhận (bổ sung cho ADR-0013 phần thinning)
- **Ngày:** 2026-11-15

## Bối cảnh
Kế hoạch và spike S4 dùng Zhang–Suen. Ở Giai đoạn 1, ảnh mẫu `a08-may-mat-troi` (tia mặt trời chéo 45°, nét 5px, ảnh thu nhỏ về 87,5%) chỉ đạt 94,7% độ phủ: Zhang–Suen gặm đối xứng hai phía dải chéo tới khi còn 2px rồi **xoá sạch** — cả tia chỉ còn 2 điểm xương.

## Quyết định
Dùng **Guo–Hall (1989)** cho pipeline line-art (`wb_vision::skeleton::guo_hall`). Giữ `zhang_suen` trong code để so sánh/kiểm thử.

Bằng chứng: test hồi quy `regression_a08_ray_zhang_suen_erases_it_guo_hall_keeps_it` dùng **đúng mặt nạ thật** của tia a08 (`testdata_ray_a08.txt`): Zhang–Suen < 10px, Guo–Hall > 40px. Độ phủ a08 tăng 94,7% → 100%; các thanh ngang/dấu cộng cho kết quả tương đương.

## Phương án đã loại
- Zhang–Suen + biến thể Lü–Wang: vẫn có trường hợp mất nét chéo.
- Không thu nhỏ ảnh (làm việc ở độ phân giải gốc): tránh được ca này nhưng chậm hơn nhiều với ảnh lớn và không chữa gốc rễ.

## Hệ quả
- Vài ảnh giảm nhẹ độ phủ (a01 100% → 98,6%) — phần thiếu được pha hoàn thiện xử lý.
- Spike S4 (chữ) vẫn dùng Zhang–Suen; `wb-text` ở Giai đoạn 2 dùng Guo–Hall và chạy lại bộ test chữ.
