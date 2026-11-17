//! Tài liệu dự án (Project Document) — phiên bản đầu (Giai đoạn 1), chỉ đủ cho ảnh line-art.
//! Toạ độ: px của khung tham chiếu 1920×1080. Thời gian: mili-giây số nguyên.
//! Các trường mới luôn có `#[serde(default)]` để file cũ mở được trên app mới.

use serde::{Deserialize, Serialize};

/// Toàn bộ dự án video.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Project {
    pub schema_version: u32,
    #[serde(default)]
    pub settings: ProjectSettings,
    pub scenes: Vec<Scene>,
}

/// Cài đặt chung của dự án.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct ProjectSettings {
    /// Độ phân giải xuất (px). Mặc định 1920×1080.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Hạt giống cho mọi yếu tố ngẫu nhiên → video tất định.
    pub seed: u32,
    /// Màu nền bảng, dạng `#RRGGBB`.
    pub background: String,
    /// Tốc độ vẽ danh nghĩa (px/giây ở khung tham chiếu).
    pub draw_speed: f32,
    /// Bàn tay: đường dẫn tới thư mục chứa `hand.json` (tương đối so với file dự án). `None` = không vẽ tay.
    pub hand: Option<String>,
    /// Kiểu hiển thị bàn tay: cả cánh tay (to) hay chỉ tới cổ tay.
    pub hand_mode: HandMode,
    /// Hệ số cỡ tay người dùng chỉnh thêm (1 = cỡ mặc định của bộ tay).
    pub hand_scale: f32,
    /// Giọng đọc lời thoại (Giai đoạn 4).
    pub voice: VoiceSettings,
}

/// Giọng đọc lời thoại.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct VoiceSettings {
    /// Tên giọng của dịch vụ (Azure), vd. `vi-VN-HoaiMyNeural`.
    pub name: String,
    /// Tốc độ đọc (1 = bình thường, 0.5–2).
    pub rate: f32,
}

/// Giọng mặc định: HoaiMy (nữ, tiếng Việt).
pub const DEFAULT_VOICE: &str = "vi-VN-HoaiMyNeural";

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            name: DEFAULT_VOICE.into(),
            rate: 1.0,
        }
    }
}

/// Kiểu hiển thị bàn tay.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum HandMode {
    /// Nguyên ảnh tay + cẳng tay, hiển thị to (hệ số `full_scale` trong `hand.json`)
    /// để chỗ ảnh bị cắt ở cẳng tay thường nằm ngoài khung.
    Full,
    /// Chỉ bàn tay tới cổ tay (đường `wrist` trong `hand.json`), mép cắt mờ dần — **mặc định**.
    /// Bộ tay không khai báo `wrist` (vd. chỉ có cây bút) thì dùng nguyên ảnh.
    #[default]
    Wrist,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
            seed: 1,
            background: "#FFFFFF".into(),
            draw_speed: 1400.0,
            hand: None,
            hand_mode: HandMode::Wrist,
            hand_scale: 1.0,
            voice: VoiceSettings::default(),
        }
    }
}

/// Một phân cảnh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Scene {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// Ghi đè màu nền dự án cho scene này.
    #[serde(default)]
    pub background: Option<String>,
    pub elements: Vec<Element>,
    /// Thời gian giữ hình sau khi vẽ xong element cuối (ms).
    #[serde(default = "default_tail_ms")]
    pub tail_ms: u32,
    /// Lời thoại đọc trong scene. Dấu `[n]` = element thứ n (theo thứ tự vẽ) bắt đầu vẽ đúng lúc đọc tới đó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narration: Option<String>,
}

fn default_tail_ms() -> u32 {
    1500
}

/// Một đối tượng trên scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Element {
    pub id: String,
    pub kind: ElementKind,
    pub transform: Transform,
    #[serde(default)]
    pub timing: Timing,
}

/// Loại element: ảnh hoặc chữ viết tay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ElementKind {
    Image {
        /// Đường dẫn ảnh (tương đối so với file dự án).
        src: String,
        #[serde(default)]
        pipeline: PipelineChoice,
        /// Cách bàn tay làm ảnh hiện ra (phác thảo / tô màu bằng cọ…).
        #[serde(default)]
        style: DrawStyle,
        /// Nền đồng màu quanh vật thể: giữ hay xoá (ảnh màu / ảnh chụp).
        #[serde(default)]
        background: BackgroundMode,
        #[serde(default)]
        params: VisionParams,
        /// Đổi màu: mọi điểm có mực thành màu này (`#RRGGBB`), giữ độ trong.
        /// Hợp cho icon / ảnh nét một màu. `None` = giữ màu gốc.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tint: Option<String>,
    },
    /// Chữ viết tay. Khung `transform`: (x, y) = góc trên-trái dòng đầu, `w` = bề rộng tối đa (xuống dòng).
    Text {
        /// Nội dung (ký tự xuống dòng = xuống dòng).
        text: String,
        /// File font (.ttf/.otf, tương đối so với file dự án). `None` = font mặc định của app.
        #[serde(default)]
        font: Option<String>,
        /// Cỡ chữ (px khung tham chiếu).
        #[serde(default = "default_text_size")]
        size: f32,
        /// Màu chữ `#RRGGBB`.
        #[serde(default = "default_text_color")]
        color: String,
        #[serde(default)]
        align: TextAlign,
        /// Khoảng cách dòng / cỡ chữ.
        #[serde(default = "default_line_height")]
        line_height: f32,
    },
}

fn default_text_size() -> f32 {
    72.0
}
fn default_text_color() -> String {
    "#1D1D1F".into()
}
fn default_line_height() -> f32 {
    1.35
}

/// Căn lề chữ trong khung.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Pipeline xử lý ảnh (xem kế hoạch File 02).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PipelineChoice {
    /// Tự nhận loại ảnh (`wb-vision::classify`).
    #[default]
    Auto,
    /// Nhóm A — nét vẽ trên nền sáng: bàn tay đi theo đường tâm nét.
    LineArt,
    /// Nhóm B — minh hoạ, clipart, sticker (thường có nền đồng màu / trong suốt).
    Color,
    /// Nhóm C — ảnh chụp, tranh tô kín khung.
    Photo,
    /// Nhóm E — SVG: bàn tay đi thẳng theo path có stroke (chỉ áp dụng cho file .svg).
    Vector,
}

/// Kiểu vẽ một ảnh (kế hoạch File 02, Nhóm C bước 5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum DrawStyle {
    /// Theo loại ảnh: line-art → `sketch`; ảnh màu → `sketch_then_color`; ảnh chụp → `color_brush`.
    #[default]
    Auto,
    /// Chỉ vẽ nét (line-art: nét gốc; ảnh màu/chụp: nét phác thảo chì — ảnh cuối là bản phác thảo).
    Sketch,
    /// Vẽ nét phác thảo trước, rồi cọ tô hiện màu ảnh gốc.
    SketchThenColor,
    /// Chỉ tô: lớp màu bị che, cọ lia qua lia lại tới đâu ảnh hiện tới đó.
    ColorBrush,
}

/// Xử lý nền đồng màu quanh vật thể (ảnh màu / ảnh chụp; line-art luôn tách giấy).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    /// Xoá nếu mép ảnh đồng màu (phông xanh, nền trắng clipart), ngược lại giữ.
    #[default]
    Auto,
    Keep,
    Remove,
}

/// Tham số trích xuất nét người dùng chỉnh được.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct VisionParams {
    /// Cạnh dài tối đa của ảnh khi phân tích (px). Lớn = chi tiết hơn, chậm hơn.
    pub working_size: u32,
    /// Độ nhạy nhận nét mực (0..1). Cao = bắt cả nét mờ.
    pub sensitivity: f32,
    /// Nét ngắn hơn ngưỡng này (px ở độ phân giải làm việc) thành "chấm vụn".
    pub min_stroke_length: f32,
    /// Bán kính cọ tô, tính theo tỉ lệ cạnh dài của ảnh (0.025 = 2,5%).
    pub brush_size: f32,
}

impl Default for VisionParams {
    fn default() -> Self {
        Self {
            working_size: 1400,
            sensitivity: 0.5,
            min_stroke_length: 6.0,
            brush_size: 0.025,
        }
    }
}

/// Khung đặt element (px khung tham chiếu). Ảnh được **co vừa** khung, giữ tỉ lệ, căn giữa.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Transform {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Transform {
    /// Tính (tỉ lệ, dịch x, dịch y) để đặt ảnh `src_w×src_h` vừa khung, giữ tỉ lệ, căn giữa.
    pub fn fit(&self, src_w: u32, src_h: u32) -> (f32, f32, f32) {
        let s = (self.w / src_w as f32).min(self.h / src_h as f32);
        let ox = self.x + (self.w - src_w as f32 * s) / 2.0;
        let oy = self.y + (self.h - src_h as f32 * s) / 2.0;
        (s, ox, oy)
    }
}

/// Thời gian của element (Giai đoạn 1: nối tiếp nhau; marker lời thoại ở Giai đoạn 4).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Timing {
    /// Nghỉ trước khi bắt đầu vẽ (ms).
    pub gap_before_ms: u32,
    /// Ép tổng thời gian vẽ (ms). `None` = tự tính theo độ dài nét.
    pub duration_ms: Option<u32>,
    /// Thời gian "hoàn thiện": phần chưa được nét phủ hiện dần cho khớp ảnh gốc (ms).
    pub settle_ms: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            gap_before_ms: 300,
            duration_ms: None,
            settle_ms: 300,
        }
    }
}

/// Đọc màu `#RRGGBB` / `#RRGGBBAA` → RGBA.
pub fn parse_hex_color(s: &str) -> Option<[u8; 4]> {
    let h = s.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_project_json_uses_defaults() {
        let json = r#"{
            "schema_version": 1,
            "scenes": [{ "id": "s1", "elements": [{
                "id": "img1",
                "kind": { "type": "image", "src": "a.png" },
                "transform": { "x": 0, "y": 0, "w": 1920, "h": 1080 }
            }]}]
        }"#;
        let p: Project = serde_json::from_str(json).unwrap();
        assert_eq!(p.settings.fps, 30);
        assert_eq!(p.scenes[0].tail_ms, 1500);
        let el = &p.scenes[0].elements[0];
        assert_eq!(el.timing.gap_before_ms, 300);
        let ElementKind::Image {
            pipeline,
            style,
            background,
            params,
            ..
        } = &el.kind
        else {
            panic!("phải là ảnh");
        };
        assert_eq!(*pipeline, PipelineChoice::Auto);
        assert_eq!(*style, DrawStyle::Auto);
        assert_eq!(*background, BackgroundMode::Auto);
        assert_eq!(params.working_size, 1400);
    }

    #[test]
    fn fit_keeps_aspect_and_centers() {
        let t = Transform {
            x: 100.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        };
        let (s, ox, oy) = t.fit(200, 200); // ảnh vuông vào khung 2:1
        assert_eq!(s, 2.5);
        assert_eq!(oy, 0.0);
        assert_eq!(ox, 100.0 + 250.0);
    }

    #[test]
    fn hex_colors() {
        assert_eq!(parse_hex_color("#FF8000"), Some([255, 128, 0, 255]));
        assert_eq!(parse_hex_color("#00000080"), Some([0, 0, 0, 128]));
        assert_eq!(parse_hex_color("red"), None);
    }
}
