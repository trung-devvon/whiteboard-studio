# ADR-0012: Nguồn giọng đọc (TTS) cho MVP

- **Trạng thái:** ⏳ Cần quyết định — Edge-TTS đã bị **loại**
- **Ngày:** 2026-11-14

## Bối cảnh
Kế hoạch v1.1 chọn Edge-TTS (miễn phí, có WordBoundary) làm giọng đọc MVP. Spike S3 phát hiện dịch vụ này giờ yêu cầu token `Sec-MS-GEC` phiên bản 2, sinh bằng một biến thể SHA-256 bị làm rối, dịch ngược từ trình duyệt. Tích hợp = tự duy trì code vượt cơ chế kiểm soát truy cập của Microsoft; cơ chế này đã thay đổi nhiều lần và sẽ còn đổi.

## Quyết định (một phần)
- **Không** tích hợp Edge-TTS vào app (không viết, không nhúng code sinh token).
- Giữ trait `TtsProvider` (File 04 kế hoạch) — mọi provider đều trả `audio + mốc thời gian`.
- **Luôn có đường offline:** import file audio bất kỳ + **Whisper forced alignment** (cần spike S3b).

## Cần chọn: provider chính thức mặc định
| Phương án | Ưu | Nhược |
|---|---|---|
| Google Cloud TTS | SSML `<mark/>` trả thời điểm từng marker → khớp thẳng thiết kế `{marker}`; có giọng vi-VN; hạn mức miễn phí hàng tháng | Cần tài khoản Google Cloud + bật billing để lấy key |
| Azure AI Speech | Cùng giọng HoaiMy/NamMinh như Edge; có gói miễn phí; WordBoundary chính thức | Không có SDK Rust; phải tự viết client WebSocket/REST theo tài liệu chính thức |
| ElevenLabs | Giọng tự nhiên nhất, API `with-timestamps` | Trả phí, hạn mức miễn phí nhỏ |
| Chỉ offline (file + Whisper) | Miễn phí, không mạng, không key | Người dùng phải tự tạo audio ở công cụ khác |

## Hạn mức miễn phí (tra cứu 2026-11-15 — cần kiểm lại trên trang giá chính thức trước khi chốt)
| Dịch vụ | Miễn phí | Tiếng Việt | Mốc thời gian để đồng bộ |
|---|---|---|---|
| Azure AI Speech (gói F0) | ~0,5 triệu ký tự/tháng giọng neural; hết thì bị chặn, không tự tính tiền | Có (vi-VN, vd. HoaiMy, NamMinh) | WordBoundary chính thức |
| Google Cloud TTS | ~4 triệu ký tự/tháng giọng Standard; khoảng 1 triệu/tháng cho giọng cao cấp (Chirp 3 HD…) | Có (vi-VN) | SSML `<mark/>` |
| FPT.AI | ~100.000 ký tự/tháng | Giọng Việt bản địa | Không có mốc từng từ (cần Whisper alignment) |
| Gemini API (TTS preview) | Vài chục yêu cầu/ngày, thay đổi theo thời điểm | Có (đa ngôn ngữ) | Không có (cần Whisper alignment) |

Gợi ý cho giai đoạn thử nghiệm: **Azure F0** (đủ dùng thử, có mốc từng từ, giọng Việt tốt, không phát sinh phí khi vượt hạn mức) — vẫn cần tài khoản Azure. Chờ chủ dự án chốt.

## Hệ quả
- Rủi ro R2 chuyển sang trạng thái "đã xảy ra" — đã xử lý bằng cách loại bỏ sớm (rẻ hơn nhiều so với phát hiện ở Giai đoạn 4).
- Giai đoạn 4 cần thêm: màn hình nhập key TTS (lưu trong keyring), spike S3' với provider được chọn.
