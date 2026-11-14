# Dữ liệu test

## `fonts/`
Font OFL có tiếng Việt dùng cho test chữ viết tay (kèm file giấy phép). Xem `assets/CREDITS.md`.

## `images/` — bộ ảnh mẫu (CẦN THU THẬP)

Đây là tài sản quý nhất để đo tiến bộ thuật toán ảnh. Mục tiêu ~50 ảnh:

| Thư mục | Nhóm | Số lượng | Ví dụ |
|---|---|---|---|
| `A/` | Line-art, icon đen trắng, sơ đồ, truyện tranh | 15 | Tranh nét bút bi tự vẽ rồi chụp/scan, sơ đồ vẽ tay |
| `B/` | Màu phẳng, logo, sticker, clipart | 10 | Hình minh hoạ flat, logo tự thiết kế |
| `C/` | Ảnh chụp người, phong cảnh, đồ vật | 10 | Ảnh tự chụp bằng điện thoại |
| `D/` | Ảnh có chữ nhỏ (slide, meme, sơ đồ chụp) | 5 | Chụp màn hình slide của chính bạn |
| `E/` | SVG / icon vector | 10 | Icon Lucide/Tabler, SVG tự vẽ |

**Quy tắc:**
- Ưu tiên ảnh **tự vẽ / tự chụp** hoặc **CC0** (ví dụ từ các kho ảnh CC0). Không lấy ảnh có bản quyền trên mạng.
- Mỗi ảnh ghi một dòng vào `images/SOURCES.md`: tên file · nguồn · giấy phép · nhóm kỳ vọng.
- Tên file không dấu, không khoảng trắng: `a01-so-do-may-chu.png`.

## `golden/`
Khung hình chuẩn cho golden test (sẽ có từ Giai đoạn 1). Chỉ cập nhật bằng lệnh có chủ đích (`UPDATE_GOLDEN=1`) và luôn xem lại ảnh khác biệt.
