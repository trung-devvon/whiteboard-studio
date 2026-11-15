# Giai đoạn 2 — Chữ viết tay, ảnh màu (tô nét) & SVG

> Nhánh: `phase-1-core-engine` (chưa tách nhánh riêng) · Kế hoạch gốc: `not_push_git/06-lo-trinh-va-tieu-chi-hoan-thanh.md` (Giai đoạn 2)

## 1. Kết quả theo tiêu chí hoàn thành (DoD)

| Tiêu chí | Kết quả |
|---|---|
| Câu "Chào mừng các bạn đến với bài học hôm nay" viết đúng dấu trên 3 font | ✅ test `dod_sentence_on_three_ofl_fonts`: Patrick Hand, Mali, Itim — **đúng số dấu**, độ phủ ≥ 99%, dấu viết sau thân chữ trong từng từ |
| 10 ảnh clipart + 10 icon vẽ đẹp (bạn tự chấm ≥ 8/10) | ⏳ **Cần bạn chấm.** Mới có 1 SVG màu tự vẽ (`E/e01`) + 1 ảnh đồ vật 2D của bạn. Thư viện icon (Lucide/Tabler) chưa đóng gói |
| Bộ phân loại đoán đúng ≥ 80% bộ test A/B/E | ✅ trên bộ hiện có: 10/10 line-art (test `every_fixture_is_covered_by_its_strokes`), ảnh đồ vật 2D → `color`, anime + quả thông → `photo`, SVG → `vector`. Bộ test còn nhỏ |

Demo: `wb render tests/fixtures/projects/demo-phase2.json -o demo.mp4` — chữ tiêu đề + bóng đèn SVG, rồi ngôi nhà SVG (vẽ nét → tô màu) + chú thích chữ đỏ 2 dòng. 25,5 s video xuất trong ~3 s ở 1280px.

## 2. Đã làm gì

### Bàn tay
- `assets/hands/pen-brown` (ảnh của bạn, 3500×3000) là **tay mặc định**; `pencil-purple` là lựa chọn thứ hai; `pencil-green` là **chỉ cây bút chì** (không có tay). Tay SVG tạm đã xoá.
- **Thu nhỏ một lần khi nạp** (gộp khối + bicubic) thay vì co ảnh 3500px ở mỗi khung → viền mịn, render nhanh hơn.
- **Hai kiểu tay** (`settings.hand_mode`, CLI `--hand-mode`):
  - `wrist` (**mặc định**) — bàn tay tới **cổ tay**, dư thêm một đoạn ngắn cẳng tay (`hand.json` → `"wrist": [[x,y],[x,y]]` = đường cắt), mép cắt mờ dần ~3% chiều cao ảnh. Bộ không khai báo `wrist` (vd. `pencil-green`) dùng nguyên ảnh.
  - `full` — nguyên ảnh tay + cẳng tay, phóng to `full_scale` = 1,4 → chỗ ảnh bị cắt ở cẳng tay thường nằm ngoài khung (vẫn có thể lộ khi vẽ sát góc trên-trái).
  - `settings.hand_scale` (CLI `--hand-scale`) chỉnh thêm cỡ tay cho cả hai kiểu.
  - (Đã thử và bỏ cách "kéo dài cánh tay ra ngoài khung" — trông dị.)
- **`wb remove-bg ảnh.png -o out.png [--tolerance 42] [--no-trim]`** — xoá nền đồng màu nối liền mép ảnh (dùng chung `wb-vision::background`), cắt gọn về vật thể và in độ dời để chỉnh `anchor`. Dùng để chuẩn bị ảnh tay/bút/sticker mới. Ngưỡng nhỏ hơn giữ chi tiết sáng sát mép (khâu kim loại trắng của bút chì).

### Ảnh màu & ảnh chụp — "tô nét" (không cần AI)
Mỗi ảnh giờ có thể gồm **nhiều lớp vẽ** nối tiếp; lớp trước xong được "nướng" vào nền, lớp sau phủ lên:

| Loại ảnh (tự nhận) | Kiểu vẽ mặc định | Lớp |
|---|---|---|
| `line_art` | `sketch` | nét gốc (như Giai đoạn 1) |
| `color` (clipart, minh hoạ, có nền đồng màu) | `sketch_then_color` | phác thảo chì → **tô màu bằng cọ** |
| `photo` (ảnh chụp, tranh kín khung) | `color_brush` | **tô màu bằng cọ** |
| `vector` (SVG có nét) | nét theo path (+ tô nếu có `fill`) | nét → tô màu |

- `wb-vision::classify` — đặc trưng đơn giản: tỉ lệ giấy/trong suốt, độ đậm màu của mực, số ô màu, mép ảnh đồng màu.
- `wb-vision::background` — xoá **nền đồng màu nối liền mép ảnh** (đũa thần từ mép), giữ phần bên trong vật thể; mép được khử răng cưa bằng color-to-alpha.
- `wb-vision::sketch` — nét phác thảo bằng **Difference of Gaussians** (họ XDoG), lớp phác thảo màu chì có độ đậm mềm.
- `wb-planner::hatch` — **cọ zíc-zắc** (boustrophedon): chia vùng thành hàng cách 1,4·r, nối đoạn chồng nhau thành chuỗi lia qua lia lại; vùng lõm/tách rời tự sinh chuỗi mới. Độ phủ 100% trên mọi ảnh thử.
- `wb-timing`: nét tô (`Fill`) có tốc độ riêng `fill_speed` (4200 px/s) — cọ rộng đi nhanh hơn bút.
- Người dùng đổi được: `pipeline` (`auto|line_art|color|photo|vector`), `style` (`auto|sketch|sketch_then_color|color_brush`), `background` (`auto|keep|remove`), `params.brush_size`.

### SVG / icon (Nhóm E)
- `wb-cli::vector` — path có `stroke` **dùng thẳng làm đường đi tay** (bán kính = nửa độ dày nét); path có `fill` → lớp tô bằng cọ. SVG không có nét nào → xử lý như ảnh màu.
- Lớp mực rasterize từ chính SVG trên nền trong suốt → khung cuối đúng từng điểm ảnh. 10 ảnh mẫu A giờ đi theo path: **độ phủ 100%**, phân tích ~15 ms (trước ~85 ms).

### Chữ viết tay (`wb-text`)
- `harfrust` 0.16 (shaping) + `skrifa` 0.50 (đường bao glyph) — thay `rustybuzz`/`ttf-parser` đã ngừng bảo trì.
- NFC → kiểm tra font có đủ glyph (báo lỗi rõ thay vì vẽ ô vuông) → shaping từng từ → **xuống dòng theo bề rộng khung**, căn trái/giữa/phải → mỗi glyph tách thành phần liên thông → **dấu** = phần nằm hẳn trên x-height / dưới baseline của glyph có thân → Guo–Hall → nét; chấm nhỏ → "nét chấm".
- Thứ tự viết: theo từ, **thân chữ trước, dấu sau**.
- Element mới `{"type": "text", "text", "font", "size", "color", "align", "line_height"}`; font mặc định `assets/fonts/PatrickHand-Regular.ttf` (OFL). Biên dịch được sang wasm32.

### CLI
`wb quick` / `wb analyze` nhận thêm `--pipeline`, `--style`, `--background`, `--brush-size`; `analyze` in loại ảnh + từng lớp và ghi ảnh debug cho mỗi lớp (`-1.png`, `-2.png`).

## 3. Kiểm chứng
- 55 test (thêm: hatch 2, classify 1, background 3, sketch 1, filters 1, tay 2, vector 2, chữ 4, e2e Giai đoạn 2 2 — gồm nạp mọi bộ tay ở cả hai kiểu).
- Golden cập nhật có chủ đích (tay mới + SVG đi theo path) — đã xem lại từng ảnh.
- `cargo clippy -D warnings`, `cargo deny check`, build wasm32 (gồm `wb-text`), `npx tsc --noEmit`: sạch.

## 4. Hạn chế đã biết / việc còn lại
- **Phác thảo ảnh chụp/tranh vẽ kín** (anime, quả thông) còn rời rạc — vì thế mặc định ảnh chụp là `color_brush`. Nét đẹp cần model photo→sketch (Giai đoạn 6).
- Chưa đóng gói **thư viện icon** (Lucide ISC / Tabler MIT) và chỉ mục tìm kiếm; chưa có tuỳ chọn **rung nét kiểu vẽ tay** cho icon.
- Chưa có pipeline màu phẳng đầy đủ theo kế hoạch (K-Means + tô từng mảng màu theo thứ tự) — hiện tô cả vật thể một lượt.
- Ảnh tay, bút chì và 3 ảnh test (anime, ảnh chụp, đồ vật 2D) do tác giả dự án tự tạo bằng AI — dùng tự do (xem `assets/CREDITS.md`).
