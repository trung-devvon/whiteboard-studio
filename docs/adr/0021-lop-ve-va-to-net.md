# ADR-0021: Element nhiều lớp vẽ + "tô nét" bằng cọ cho ảnh màu / ảnh chụp

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-15

## Bối cảnh
Thử ảnh thật (anime, ảnh chụp quả thông, đồ vật 2D) qua pipeline line-art: khung cuối vẫn đúng nhưng **quá trình vẽ** xấu — mảng tối bị coi là mực, bàn tay đi ngoằn ngoèo theo xương của mảng màu. Kế hoạch (File 02) có ý tưởng "phác thảo rồi tô" và cọ zíc-zắc (§2.3) nhưng chưa có mô hình dữ liệu cho việc một ảnh được vẽ **nhiều lượt**.

## Quyết định
1. Một element ảnh được phân tích thành **danh sách lớp vẽ** (`Layer`: lớp mực riêng + DrawPlan riêng, có thể ở độ phân giải khác ảnh nguồn). Khi biên dịch, mỗi lớp thành một `ElementAsset` nối tiếp nhau — **engine không đổi**: lớp trước xong được nướng vào nền, lớp sau phủ lên. Khung cuối = lớp cuối = ảnh gốc (trừ kiểu `sketch` cho ảnh màu, khi đó khung cuối là bản phác thảo).
2. Lớp **tô màu** dùng nét loại `Fill` bán kính lớn (2,5% cạnh dài) đi zíc-zắc (`wb-planner::hatch`), có tốc độ riêng trong `wb-timing`.
3. Kiểu vẽ là tuỳ chọn của element (`style`), mặc định theo loại ảnh tự nhận (`pipeline: auto`): ảnh chụp → `color_brush` (người dùng ưu tiên cách này: tránh được việc phải "hiểu" ảnh chụp).

## Phương án đã loại
- Thêm khái niệm "pass" vào engine: phức tạp hơn mà không lợi gì so với nối tiếp ElementAsset.
- Crossfade / hiện mờ dần: trông "rẻ" (đã ghi trong kế hoạch).

## Hệ quả
- Timeline có nhiều mục cho một element → báo cáo cộng dồn thời lượng; marker lời thoại (Giai đoạn 4) cần gắn vào **element**, không vào từng lớp.
- Cache DrawPlan (Giai đoạn 3) phải lưu theo lớp.
