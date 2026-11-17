# ADR-0012: Nguồn giọng đọc (TTS) cho MVP

- **Trạng thái:** ✅ Chấp nhận (2026-11-18) — **Azure AI Speech**, giọng mặc định **HoaiMy**; Edge-TTS bị **loại**
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

## Quyết định cuối (2026-11-18)
- Chủ dự án chọn **Azure AI Speech**, gói **Free F0** (tài khoản Azure for Students), vùng `japaneast`
  (tài khoản sinh viên bị giới hạn vùng — Southeast Asia không được phép). Giọng mặc định: `vi-VN-HoaiMyNeural`.
- **Kênh gọi:** WebSocket chính thức `wss://<vùng>.tts.speech.microsoft.com/cognitiveservices/websocket/v1`,
  xác thực bằng key của người dùng (`Ocp-Apim-Subscription-Key`). REST `/cognitiveservices/v1` không trả mốc thời gian.
  Giao thức tin nhắn theo Speech SDK JavaScript mã nguồn mở (MIT) của Microsoft — không token ẩn, không vượt DRM.
- **Đồng bộ:** dấu `[n]` trong lời thoại → SSML `<bookmark mark="e<n>"/>` → Azure trả mốc → element n bắt đầu vẽ đúng mốc
  (`wb-timing`: chờ tới mốc; các element trước co thời lượng theo tỉ lệ cho kịp, tối thiểu 400 ms mỗi lớp).
  Mốc từng từ (WordBoundary) được lưu để làm phụ đề sau.
- **Key:** cất trong kho mật khẩu hệ điều hành (crate `keyring`, Windows Credential Manager); khi phát triển đọc
  `WB_AZURE_SPEECH_KEY` / `WB_AZURE_SPEECH_REGION` từ biến môi trường hoặc `.env` (đã gitignore). Không ghi vào dự án.
- **Đệm:** `generated/tts/<khoá>.wav|json` trong thư mục dự án; khoá = FNV-1a(giọng, tốc độ, lời) → chỉ gọi lại khi đổi.
- Đường offline (file audio + Whisper alignment) vẫn giữ trong kế hoạch, làm sau.
