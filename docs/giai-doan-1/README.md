# Giai đoạn 1 — Core Engine + CLI: "vẽ được ảnh line-art ra MP4"

> Nhánh: `phase-1-core-engine` · Kế hoạch gốc: `not_push_git/06-lo-trinh-va-tieu-chi-hoan-thanh.md` (Giai đoạn 1)

## 1. Kết quả theo tiêu chí hoàn thành (DoD)

| Tiêu chí | Kết quả |
|---|---|
| 10/10 ảnh line-art xuất MP4 đúng | ✅ 10 ảnh mẫu tự vẽ (`tests/fixtures/images/A`) → video 10 scene, 50,9 s |
| Ảnh cuối khớp ảnh gốc, sai lệch < 1% | ✅ test `final_frame_of_each_scene_matches_source_image` (mọi scene < 2,55/255) |
| Golden test cho khung giữa chừng | ✅ 4 khung trong `tests/golden/`, khớp cả bản debug lẫn release |
| Render 30 s video ≤ 60 s | ✅ 50,9 s video xuất trong 17,3 s (**2,9× thời gian thực**, NVENC) |
| Chụp bàn tay đầu tiên | ⏳ Dùng **tay SVG tạm** tự vẽ (`assets/hands/placeholder-marker`); ảnh tay chụp thật là việc của bạn (File 09 §3) |

Độ phủ của kế hoạch nét (trước pha hoàn thiện) trên 10 ảnh mẫu: **97,9 – 100%** (TB ≈ 99,4%). Phần còn thiếu (chủ yếu ở giao điểm, đầu mũi tên) hiện ra trong 300 ms "hoàn thiện" cuối mỗi element → khung cuối luôn đúng ảnh gốc.

## 2. Đã làm gì (theo crate)

| Crate | Nội dung | Test |
|---|---|---|
| `wb-model` | Mô hình dự án v1 (Project → Scene → Element ảnh, Transform "co vừa khung", Timing), DrawPlan có **chấm vụn** (`dots`), hằng `REVEAL_MARGIN` dùng chung | 7 |
| `wb-vision` | Pipeline **Nhóm A**: color-to-alpha (tách mực khỏi giấy, ghép lại đúng màu gốc) → ngưỡng → bỏ nhiễu → distance transform → **Guo–Hall** → đồ thị xương → nối nét qua giao điểm theo hướng thẳng nhất, tỉa gai | 12 |
| `wb-planner` | Kéo dài đầu nét, RDP giữ bán kính lớn nhất, Catmull-Rom **centripetal** giữ góc nhọn, gắn nét vụn thành chấm, thứ tự bằng R-Tree (2 đầu, cho đảo chiều) + **2-opt O(n²)** + thiên hướng vẽ trái→phải, trên→dưới | 6 |
| `wb-timing` | Thời lượng theo độ dài nét, nhấc bút theo quãng đường, ép tổng thời gian, pha vào/vẽ/nhấc/hoàn thiện, **minimum-jerk** | 4 |
| `wb-engine` | Reveal bằng **mặt nạ riêng cho element đang vẽ**, element xong được "nướng" vào nền; hoàn thiện nâng mặt nạ lên 100%; tay vào/ra khung, cung nhấc bút; tất định khi tua | 4 |
| `wb-export` | FFmpeg sidecar, **tự dò encoder** (nvenc→amf→qsv→openh264), kênh giới hạn, huỷ, log, cờ màu BT.709 | — |
| `wb-cli` | `wb render`, `wb quick`, `wb frame`, `wb analyze`; đọc PNG/JPEG/WebP/SVG; phân tích song song | 3 (e2e + golden) |

Tổng: **37 test**, clippy `-D warnings` sạch, `cargo deny check` OK cả 4 hạng mục, các crate engine build được cho wasm32.

## 3. Cách dùng CLI

```bash
cargo build -p wb-cli --profile bench-fast
```
Phân tích một ảnh, xem thứ tự nét (xanh → đỏ = trước → sau; đường chấm xám = nhấc bút):
```bash
target/bench-fast/wb analyze tests/fixtures/images/A/a01-ngoi-nha.svg -o debug.png
```
Xuất nhanh một ảnh bất kỳ của bạn thành video (tự dùng tay tạm):
```bash
target/bench-fast/wb quick duong-dan-anh.png -o video.mp4
```
Xuất dự án mẫu 10 scene:
```bash
target/bench-fast/wb render tests/fixtures/projects/demo-line-art.json -o demo.mp4
```
Một khung hình tại giây 2,5:
```bash
target/bench-fast/wb frame tests/fixtures/projects/demo-line-art.json --at 2.5 -o f.png
```

## 4. Phát hiện & quyết định trong giai đoạn này

1. **Zhang–Suen xoá mất dải chéo 45°** (tia mặt trời ảnh a08 chỉ còn 2 điểm sau khi thu nhỏ ảnh) → chuyển sang **Guo–Hall**. Có test hồi quy dùng đúng mặt nạ thật của tia đó. → [ADR-0020](../adr/0020-guo-hall-thinning.md)
2. **Đóng dấu pattern trực tiếp (spike S2) làm đậm gấp đôi** viền mực bán trong suốt ở chỗ nét chồng nhau → engine dùng mặt nạ phủ riêng + "nướng" khi xong. Khung cuối khớp **tuyệt đối** (test so từng byte).
3. **`skip_serializing_if` phá vỡ postcard** (định dạng nhị phân không tự mô tả) → bỏ, thêm test round-trip.
4. **`rustybuzz` và `ttf-parser` đã ngừng bảo trì** (RUSTSEC-2026-0206/0192) → `wb-text` ở Giai đoạn 2 dùng **`harfrust`** (HarfBuzz) + **`skrifa`** (Google Fonts). Spike S4 vẫn dùng bản cũ, được ghi ngoại lệ trong `deny.toml`.
5. 2-opt ban đầu O(n³) → cộng dồn còn O(n²): 0,5 s → 0,02 s trên 200 nét.

## 5. Hạn chế đã biết / việc tiếp theo

- **Mới thử trên ảnh vẽ bằng code (SVG)** — chưa có ảnh scan/chụp thật (giấy ố, ánh sáng không đều, nét bút bi mảnh). Có thể cần **ngưỡng thích nghi cục bộ** (Sauvola). → cần bộ ảnh test thật (`tests/fixtures/README.md`).
- Ở giao điểm, nét đang vẽ làm lộ một mẩu nhỏ của nét cắt ngang (đặc tính của reveal) — chấp nhận được, ghi nhận.
- Chưa có **cache DrawPlan** theo hash (mỗi lần render đều phân tích lại, ~130 ms/ảnh) — làm cùng ProjectStore ở Giai đoạn 3.
- Bàn tay vẽ lại bằng phép co giãn mỗi khung → có thể co sẵn một lần để render nhanh hơn ~15%.
- Logic "biên dịch dự án" đang nằm trong `wb-cli` (lib) — tách crate riêng khi app Tauri cần dùng.
