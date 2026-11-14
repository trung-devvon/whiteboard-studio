# ADR-0007: FFmpeg sidecar bản LGPL, chọn encoder theo phần cứng

- **Trạng thái:** Chấp nhận
- **Ngày:** 2026-11-14

## Bối cảnh
Cần mã hoá MP4 H.264 nhanh, màu đúng, không "nhiễm" GPL vào code (chính sách "Giữ cửa mở", ADR-0015).

## Quyết định
- Đóng gói **FFmpeg bản LGPL** (không libx264), gọi dạng **tiến trình riêng**, nhận khung RGBA qua stdin, kênh có giới hạn (backpressure).
- Thứ tự thử encoder: `h264_nvenc` → `h264_amf` → `h264_qsv` → `libopenh264` (dự phòng mọi máy); nhớ encoder chạy được trong cài đặt.
- Luôn chuyển màu bằng `scale=out_color_matrix=bt709:out_range=tv` **và** ghi VUI bằng `-bsf:v h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1` (openh264/Media Foundation bỏ qua cờ màu thông thường).

Bằng chứng (S1): nvenc 8,1× thời gian thực, openh264 7,8×, màu lệch tối đa 1/255, đủ 300/300 khung.

## Phương án đã loại
- Bản GPL có libx264: chất lượng/tốc độ CPU tốt hơn nhưng ép GPL.
- Ghi PNG tạm ra đĩa rồi ghép: chậm, tốn SSD.

## Hệ quả
- FFmpeg n9.0 LGPL v3 (`--enable-version3`) — ghi credits + link mã nguồn khi phát hành.
- `h264_mf` phần cứng cần đầu vào nv12 → để sau.
- Kích thước bộ cài +~170MB (zip) — cân nhắc bản FFmpeg tối giản ở Giai đoạn 8.
