//! Bộ tay: đọc/ghi `hand.json`, liệt kê thư viện tay, nhập ảnh tay mới (xoá nền + cắt gọn).
//! Dùng cho trình chỉnh bàn tay trong app.

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use wb_vision::background::{Border, analyze_border, remove_background};

pub const HAND_FILE: &str = "hand.json";

/// Nội dung `hand.json`. Toạ độ tính theo px của ảnh tay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HandMeta {
    #[serde(default)]
    pub name: String,
    pub file: String,
    /// Mũi bút.
    pub anchor: [f32; 2],
    /// Chiều cao hiển thị cả ảnh (px khung 1080p).
    pub display_height: f32,
    /// Hệ số phóng cho kiểu "cả cánh tay".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_scale: Option<f32>,
    /// Đường cắt cổ tay `[điểm 1, điểm 2]` — cho kiểu "tới cổ tay". Không có → luôn dùng nguyên ảnh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrist: Option<[[f32; 2]; 2]>,
    /// Các trường khác (id, kind, license, author…) — giữ nguyên khi ghi lại.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

pub fn read_meta(dir: &Path) -> Result<HandMeta> {
    let p = dir.join(HAND_FILE);
    let text = std::fs::read_to_string(&p).with_context(|| format!("không đọc được {}", p.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} không hợp lệ", p.display()))
}

pub fn write_meta(dir: &Path, meta: &HandMeta) -> Result<()> {
    let p = dir.join(HAND_FILE);
    let tmp = dir.join(".hand.json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(meta)?)?;
    std::fs::rename(&tmp, &p).with_context(|| format!("không ghi được {}", p.display()))
}

/// Một bộ tay trong thư viện.
#[derive(Debug, Clone, Serialize)]
pub struct HandEntry {
    /// Giá trị ghi vào `settings.hand` (vd. `builtin:hands/pen-brown`, `user:hands/tay-1`).
    pub path: String,
    pub name: String,
    /// Bộ tay người dùng tự thêm (sửa/xoá được).
    pub editable: bool,
    pub has_wrist: bool,
}

/// Liệt kê các bộ tay trong `root/hands/`, gắn tiền tố `prefix` (`builtin:` hoặc `user:`).
pub fn list_hands(root: &Path, prefix: &str, editable: bool) -> Vec<HandEntry> {
    let mut out: Vec<HandEntry> = std::fs::read_dir(root.join("hands"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let meta = read_meta(&e.path()).ok()?;
            let id = e.file_name().to_string_lossy().into_owned();
            Some(HandEntry {
                path: format!("{prefix}hands/{id}"),
                name: if meta.name.is_empty() { id } else { meta.name },
                editable,
                has_wrist: meta.wrist.is_some(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// Nhập ảnh tay mới vào `dest` (tạo thư mục): xoá nền đồng màu (nếu có), cắt gọn, đoán mũi bút.
/// Trả `hand.json` vừa ghi — người dùng chỉnh lại mũi bút / cổ tay trong trình chỉnh.
pub fn import_hand(src: &Path, dest: &Path, name: &str) -> Result<HandMeta> {
    let img = image::open(src)
        .with_context(|| format!("không mở được ảnh {}", src.display()))?
        .to_rgba8();
    let cleared = match analyze_border(&img) {
        Border::Uniform(bg) => remove_background(&img, bg),
        Border::Transparent => img,
        Border::Busy => bail!("ảnh có nền nhiều màu — hãy dùng ảnh tay trên nền trong suốt hoặc một màu"),
    };
    let trimmed = trim(&cleared).context("ảnh trống sau khi xoá nền")?;
    std::fs::create_dir_all(dest)?;
    trimmed.save(dest.join("hand.png"))?;
    let meta = HandMeta {
        name: name.to_string(),
        file: "hand.png".into(),
        anchor: guess_tip(&trimmed),
        display_height: 700.0,
        full_scale: Some(crate::DEFAULT_FULL_SCALE),
        wrist: None,
        extra: [("author".to_string(), serde_json::Value::from("người dùng"))]
            .into_iter()
            .collect(),
    };
    write_meta(dest, &meta)?;
    Ok(meta)
}

/// Cắt về hộp bao phần không trong suốt.
fn trim(img: &RgbaImage) -> Option<RgbaImage> {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 8 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    (x1 >= x0 && y1 >= y0).then(|| image::imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image())
}

/// Đoán mũi bút: điểm đục xa nhất về phía trái-dưới (tay phải cầm bút thường chĩa sang trái).
fn guess_tip(img: &RgbaImage) -> [f32; 2] {
    let mut best = (f32::MIN, [0.0, 0.0]);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] > 128 {
            let score = y as f32 * 0.35 - x as f32;
            if score > best.0 {
                best = (score, [x as f32, y as f32]);
            }
        }
    }
    best.1
}

/// Ảnh xem trước (PNG) của bộ tay, cạnh dài tối đa `max_side`. Trả (png, rộng gốc, cao gốc).
pub fn preview_png(dir: &Path, max_side: u32) -> Result<(Vec<u8>, u32, u32)> {
    let meta = read_meta(dir)?;
    let file: PathBuf = dir.join(&meta.file);
    let img = image::open(&file)
        .with_context(|| format!("không mở được {}", file.display()))?
        .to_rgba8();
    let (w, h) = img.dimensions();
    let s = (max_side as f32 / w.max(h) as f32).min(1.0);
    let small = if s < 1.0 {
        image::imageops::resize(
            &img,
            ((w as f32 * s).round() as u32).max(1),
            ((h as f32 * s).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        img
    };
    let mut png = Vec::new();
    small.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)?;
    Ok((png, w, h))
}

/// Tên thư mục an toàn từ tên hiển thị (bỏ dấu tiếng Việt).
pub fn slug(name: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let s: String = name
        .to_lowercase()
        .replace('đ', "d")
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "tay".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn import_removes_background_trims_and_guesses_tip() {
        // nền trắng, "cây bút" chéo từ (10,50) lên (50,10)
        let img = RgbaImage::from_fn(60, 60, |x, y| {
            let on = (x as i32 + y as i32 - 60).abs() <= 2 && (10..=50).contains(&x);
            if on {
                Rgba([30, 30, 30, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let dir = std::env::temp_dir().join(format!("wb-hand-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let src = dir.join("src.png");
        std::fs::create_dir_all(&dir).unwrap();
        img.save(&src).unwrap();
        let dest = dir.join("hands").join("thu");
        let meta = import_hand(&src, &dest, "Thử").unwrap();
        let out = image::open(dest.join("hand.png")).unwrap().to_rgba8();
        assert!(
            out.width() <= 46 && out.height() <= 46,
            "đã cắt gọn: {:?}",
            out.dimensions()
        );
        assert_eq!(
            out.get_pixel(out.width() - 1, out.height() - 1)[3],
            0,
            "nền đã trong suốt"
        );
        // mũi bút đoán ở đầu trái-dưới
        assert!(
            meta.anchor[0] < 5.0 && meta.anchor[1] > out.height() as f32 - 8.0,
            "{:?}",
            meta.anchor
        );
        // đọc lại + liệt kê
        assert_eq!(read_meta(&dest).unwrap(), meta);
        let list = list_hands(&dir, "user:", true);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].path, "user:hands/thu");
        assert!(!list[0].has_wrist);
    }

    #[test]
    fn meta_roundtrip_keeps_unknown_fields() {
        let text = r#"{"id":"x","name":"X","file":"hand.png","anchor":[1,2],"display_height":900,"license":"MIT"}"#;
        let m: HandMeta = serde_json::from_str(text).unwrap();
        let back = serde_json::to_value(&m).unwrap();
        assert_eq!(back["license"], "MIT");
        assert_eq!(back["id"], "x");
        assert!(back.get("wrist").is_none());
    }

    #[test]
    fn slug_is_ascii() {
        assert_eq!(slug("Tay trái — bút đỏ!"), "tay-trai-but-do");
        assert_eq!(slug("   "), "tay");
    }
}
