//! "Biên dịch" dự án: đọc `project.json` → phân tích ảnh → lập kế hoạch nét → giải thời gian → [`Engine`].
//!
//! Đây là phần nối các crate lõi; CLI dùng nó, sau này app Tauri cũng sẽ dùng
//! (khi đó tách thành crate riêng, kèm cache DrawPlan theo hash — kế hoạch File 05 §4).

pub mod analyze;
pub mod debug;
pub mod text;
pub mod vector;

pub use analyze::{AnalyzedImage, ContentKind, ImageOptions, Layer, Placement, analyze_image, resolve_style};

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use serde::Deserialize;
use std::path::Path;
use wb_engine::{ElementAsset, Engine, HandSprite, Pixmap, Place, SceneAsset, pixmap_from_rgba};
use wb_model::{DrawStyle, ElementKind, HandMode, Project, REFERENCE_WIDTH, Timing, parse_hex_color};
use wb_timing::{ElementInput, SceneInput, TimingParams};

/// Báo cáo từng element sau khi biên dịch.
pub struct ElementReport {
    pub scene: String,
    pub id: String,
    pub source: (u32, u32),
    pub kind: ContentKind,
    pub style: DrawStyle,
    pub background_removed: bool,
    /// Tên các lớp vẽ, theo thứ tự.
    pub layers: Vec<&'static str>,
    pub strokes: u32,
    pub dots: u32,
    pub coverage: f32,
    pub analyze_ms: u128,
    pub draw_ms: f64,
}

pub struct Compiled {
    pub engine: Engine,
    pub project: Project,
    pub report: Vec<ElementReport>,
}

/// Chia thời gian của một element cho các lớp: lớp đầu nhận khoảng nghỉ, lớp cuối nhận pha hoàn thiện;
/// giữa các lớp nghỉ ngắn. `duration_ms` (nếu ép) chia đều.
fn layer_timing(t: Timing, i: usize, n: usize) -> Timing {
    if n <= 1 {
        return t;
    }
    Timing {
        gap_before_ms: if i == 0 { t.gap_before_ms } else { 150 },
        duration_ms: t.duration_ms.map(|d| d / n as u32),
        settle_ms: if i + 1 == n { t.settle_ms } else { 150 },
    }
}

/// Phân tích một element (ảnh hoặc chữ). Đường dẫn tương đối so với `base_dir`.
pub fn analyze_element(base_dir: &Path, el: &wb_model::Element) -> Result<AnalyzedImage> {
    match &el.kind {
        ElementKind::Image {
            src,
            pipeline,
            style,
            background,
            params,
        } => {
            let opt = ImageOptions {
                pipeline: *pipeline,
                style: *style,
                background: *background,
                params: *params,
            };
            analyze_file(&base_dir.join(src), &opt)
        }
        ElementKind::Text {
            text,
            font,
            size,
            color,
            align,
            line_height,
        } => {
            let font_path = text::resolve_font(base_dir, font.as_deref())?;
            let color = parse_hex_color(color).with_context(|| format!("màu chữ không hợp lệ: {color}"))?;
            text::analyze_text(
                &font_path,
                &text::TextOptions {
                    text,
                    size: *size,
                    color,
                    align: *align,
                    line_height: *line_height,
                    max_width: (el.transform.w > 0.0).then_some(el.transform.w),
                },
            )
        }
    }
}

/// Phân tích một file ảnh: SVG → nhóm E (đi theo path), còn lại → [`analyze_image`].
pub fn analyze_file(path: &Path, opt: &ImageOptions) -> Result<AnalyzedImage> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        let data = std::fs::read(path).with_context(|| format!("không đọc được {}", path.display()))?;
        return vector::analyze_svg(&data, opt);
    }
    Ok(analyze_image(&load_image(path)?, opt))
}

/// Đọc và biên dịch dự án. `out_width`: ghi đè chiều rộng xuất (giữ tỉ lệ).
pub fn compile(project_path: &Path, out_width: Option<u32>) -> Result<Compiled> {
    let text =
        std::fs::read_to_string(project_path).with_context(|| format!("không đọc được {}", project_path.display()))?;
    let project: Project =
        serde_json::from_str(&text).with_context(|| format!("file dự án không hợp lệ: {}", project_path.display()))?;
    if project.schema_version > wb_model::SCHEMA_VERSION {
        bail!(
            "dự án dùng schema v{} — app chỉ hỗ trợ tới v{}",
            project.schema_version,
            wb_model::SCHEMA_VERSION
        );
    }
    let base_dir = project_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let set = &project.settings;
    let (out_w, out_h) = match out_width {
        Some(w) => (
            w & !1,
            ((w as f32 * set.height as f32 / set.width as f32).round() as u32) & !1,
        ),
        None => (set.width, set.height),
    };

    // Phân tích song song mọi element (mỗi element một luồng)
    let jobs: Vec<(usize, usize, &wb_model::Element)> = project
        .scenes
        .iter()
        .enumerate()
        .flat_map(|(si, sc)| sc.elements.iter().enumerate().map(move |(ei, el)| (si, ei, el)))
        .collect();
    let analyzed: Vec<Result<(usize, usize, AnalyzedImage)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|&(si, ei, el)| {
                let base_dir = &base_dir;
                scope.spawn(move || -> Result<(usize, usize, AnalyzedImage)> {
                    Ok((si, ei, analyze_element(base_dir, el)?))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("luồng phân tích panic"))
            .collect()
    });
    let mut per_scene: Vec<Vec<Option<AnalyzedImage>>> = project
        .scenes
        .iter()
        .map(|s| s.elements.iter().map(|_| None).collect())
        .collect();
    for r in analyzed {
        let (si, ei, a) = r?;
        per_scene[si][ei] = Some(a);
    }

    // Lắp ráp scene: mỗi lớp vẽ của một element thành một ElementAsset nối tiếp nhau
    let mut scenes = Vec::new();
    let mut report = Vec::new();
    // (scene, element) của từng asset, và Timing của nó
    let mut asset_owner: Vec<Vec<(usize, Timing)>> = Vec::new();
    for (si, sc) in project.scenes.iter().enumerate() {
        let bg =
            parse_hex_color(sc.background.as_deref().unwrap_or(&set.background)).context("màu nền không hợp lệ")?;
        let mut elements = Vec::new();
        let mut owners = Vec::new();
        for (ei, el) in sc.elements.iter().enumerate() {
            let a = per_scene[si][ei].take().unwrap();
            let (sw, sh) = a.source;
            // (px lớp đầu → px tham chiếu, góc trên-trái)
            let (scale, ox, oy) = match a.placement {
                Placement::Fit => el.transform.fit(sw, sh),
                Placement::Absolute { raster_scale, margin } => (
                    1.0 / raster_scale,
                    el.transform.x - margin / raster_scale,
                    el.transform.y - margin / raster_scale,
                ),
            };
            report.push(ElementReport {
                scene: sc.id.clone(),
                id: el.id.clone(),
                source: a.source,
                kind: a.kind,
                style: a.style,
                background_removed: a.background_removed,
                layers: a.layers.iter().map(|l| l.name).collect(),
                strokes: a.stroke_count(),
                dots: a.dot_count(),
                coverage: a.coverage(),
                analyze_ms: a.millis,
                draw_ms: 0.0,
            });
            let n = a.layers.len();
            for (li, layer) in a.layers.into_iter().enumerate() {
                let (lw, lh) = layer.ink.dimensions();
                let ink = pixmap_from_rgba(lw, lh, layer.ink.as_raw()).context("ảnh rỗng")?;
                elements.push(ElementAsset {
                    ink,
                    // lớp có thể ở độ phân giải khác ảnh nguồn → co giãn cho cùng khung
                    place: Place {
                        scale: scale * sw as f32 / lw as f32,
                        ox,
                        oy,
                    },
                    plan: layer.plan,
                });
                owners.push((report.len() - 1, layer_timing(el.timing, li, n)));
            }
        }
        scenes.push(SceneAsset {
            background: bg,
            elements,
        });
        asset_owner.push(owners);
    }
    let params = TimingParams {
        draw_speed: set.draw_speed,
        ..TimingParams::default()
    };
    let inputs: Vec<SceneInput> = project
        .scenes
        .iter()
        .zip(&scenes)
        .zip(&asset_owner)
        .map(|((sc, asset), owners)| SceneInput {
            elements: asset
                .elements
                .iter()
                .zip(owners)
                .map(|(a, (_, timing))| ElementInput {
                    plan: &a.plan,
                    scale: a.place.scale,
                    timing: *timing,
                })
                .collect(),
            tail_ms: sc.tail_ms,
        })
        .collect();
    let timeline = wb_timing::resolve(&inputs, &params);
    for (sc, owners) in timeline.scenes.iter().zip(&asset_owner) {
        for (el, (ri, _)) in sc.elements.iter().zip(owners) {
            report[*ri].draw_ms += el.draw_end_ms - el.draw_start_ms;
        }
    }
    let hand = match &set.hand {
        Some(rel) => Some(load_hand(&base_dir.join(rel), set.hand_mode, set.hand_scale)?),
        None => None,
    };
    let engine = Engine::new(out_w, out_h, REFERENCE_WIDTH, scenes, timeline, hand);
    Ok(Compiled {
        engine,
        project,
        report,
    })
}

#[derive(Deserialize)]
struct HandMeta {
    file: String,
    anchor: [f32; 2],
    display_height: f32,
    /// Hệ số phóng khi dùng kiểu "cả cánh tay" (mặc định 1,4).
    #[serde(default)]
    full_scale: Option<f32>,
    /// Đường cổ tay `[điểm mép trên, điểm mép dưới]` (px ảnh) — cho kiểu "tới cổ tay".
    #[serde(default)]
    wrist: Option<[[f32; 2]; 2]>,
}

/// Hệ số phóng mặc định của kiểu "cả cánh tay".
pub const DEFAULT_FULL_SCALE: f32 = 1.4;

/// Nạp bàn tay từ thư mục chứa `hand.json` (ảnh PNG hoặc SVG) theo kiểu hiển thị và hệ số cỡ.
pub fn load_hand(dir: &Path, mode: HandMode, scale: f32) -> Result<HandSprite> {
    let meta_path = dir.join("hand.json");
    let meta: HandMeta = serde_json::from_str(
        &std::fs::read_to_string(&meta_path).with_context(|| format!("không đọc được {}", meta_path.display()))?,
    )?;
    let file = dir.join(&meta.file);
    let (pixmap, unit) = if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        // rasterize SVG ở độ phân giải đủ nét cho 4K
        let data = std::fs::read(&file)?;
        let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;
        let size = tree.size();
        let scale = 1600.0 / size.height();
        let (w, h) = (
            (size.width() * scale).ceil() as u32,
            (size.height() * scale).ceil() as u32,
        );
        let mut pm = resvg::tiny_skia::Pixmap::new(w, h).context("kích thước SVG")?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(scale, scale),
            &mut pm.as_mut(),
        );
        let ours = Pixmap::from_vec(pm.data().to_vec(), tiny_skia::IntSize::from_wh(w, h).unwrap()).unwrap();
        (ours, scale) // anchor trong hand.json tính theo đơn vị SVG
    } else {
        let img = image::open(&file)
            .with_context(|| format!("không mở được {}", file.display()))?
            .to_rgba8();
        (pixmap_from_rgba(img.width(), img.height(), img.as_raw()).unwrap(), 1.0)
    };
    let scale = scale.clamp(0.2, 5.0);
    let hand = HandSprite {
        anchor_x: meta.anchor[0] * unit,
        anchor_y: meta.anchor[1] * unit,
        display_height: meta.display_height * scale,
        pixmap,
    };
    Ok(match mode {
        HandMode::Full => HandSprite {
            display_height: hand.display_height * meta.full_scale.unwrap_or(DEFAULT_FULL_SCALE),
            ..hand
        },
        HandMode::Wrist => match meta.wrist {
            Some([a, b]) => {
                // mép cắt mờ dần trong ~3% chiều cao ảnh tay
                let feather = hand.pixmap.height() as f32 * 0.03;
                hand.crop_at_wrist([[a[0] * unit, a[1] * unit], [b[0] * unit, b[1] * unit]], feather)
            }
            // không có cổ tay (vd. chỉ cây bút) → dùng nguyên ảnh
            None => hand,
        },
    })
}

/// Mở ảnh PNG/JPEG/WebP, hoặc SVG (rasterize lên **nền trắng**, cạnh dài 1600px, xử lý như line-art).
/// (SVG dạng vector thật — đi thẳng theo `<path>` — là Nhóm E, Giai đoạn 2.)
pub fn load_image(path: &Path) -> Result<RgbaImage> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg")) {
        let data = std::fs::read(path).with_context(|| format!("không đọc được {}", path.display()))?;
        let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default())?;
        let size = tree.size();
        let s = 1600.0 / size.width().max(size.height());
        let (w, h) = ((size.width() * s).round() as u32, (size.height() * s).round() as u32);
        let mut pm = resvg::tiny_skia::Pixmap::new(w, h).context("kích thước SVG")?;
        pm.fill(resvg::tiny_skia::Color::WHITE);
        resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(s, s), &mut pm.as_mut());
        // nền đục → premultiplied == alpha thẳng
        return RgbaImage::from_raw(w, h, pm.take()).context("SVG rỗng");
    }
    Ok(image::open(path)
        .with_context(|| format!("không mở được ảnh {}", path.display()))?
        .to_rgba8())
}

/// Ghi Pixmap (premultiplied, nền đục) ra PNG.
pub fn save_png(p: &Pixmap, path: &Path) -> Result<()> {
    p.save_png(path)
        .with_context(|| format!("không ghi được {}", path.display()))
}
