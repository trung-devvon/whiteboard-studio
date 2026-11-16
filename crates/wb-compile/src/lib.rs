//! "Biên dịch" dự án: tài liệu [`Project`] → phân tích ảnh/chữ → lớp vẽ + DrawPlan → [`Bundle`] → [`Engine`].
//!
//! Dùng chung cho CLI (`wb`) và app Tauri. App giữ một [`AnalysisCache`] để khi chỉ kéo/đổi cỡ element
//! thì không phải phân tích lại ảnh.

pub mod analyze;
pub mod cache;
pub mod debug;
pub mod hands;
pub mod text;
pub mod vector;

pub use analyze::{AnalyzedImage, ContentKind, ImageOptions, Layer, Placement, analyze_image, resolve_style};
pub use cache::AnalysisCache;
pub use wb_engine::Bundle;

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wb_engine::{BundleElement, BundleHand, BundleScene, Engine, HandSprite, Pixmap, RawImage, pixmap_from_rgba};
use wb_model::{DrawStyle, ElementKind, HandMode, Project, REFERENCE_WIDTH, Timing, parse_hex_color};
use wb_timing::Timeline;

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

/// Nơi tìm file của dự án.
#[derive(Debug, Clone)]
pub struct CompileContext {
    /// Thư mục chứa file dự án — đường dẫn tương đối tính từ đây.
    pub base_dir: PathBuf,
    /// Thư mục `assets/` của app (tay, font có sẵn). `None` → tự dò ngược lên từ `base_dir` / thư mục hiện tại.
    pub assets_dir: Option<PathBuf>,
    /// Thư mục dữ liệu của người dùng (tay tự thêm…) — cho đường dẫn `user:…`.
    pub user_dir: Option<PathBuf>,
}

/// Tiền tố đường dẫn tới tài nguyên có sẵn của app, vd. `"builtin:hands/pen-brown"`.
pub const BUILTIN_PREFIX: &str = "builtin:";
/// Tiền tố đường dẫn tới dữ liệu người dùng, vd. `"user:hands/tay-cua-toi"`.
pub const USER_PREFIX: &str = "user:";

impl CompileContext {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            assets_dir: None,
            user_dir: None,
        }
    }

    /// Thư mục `assets/` của app.
    pub fn assets(&self) -> Option<PathBuf> {
        self.assets_dir.clone().or_else(|| {
            [Some(self.base_dir.clone()), std::env::current_dir().ok()]
                .into_iter()
                .flatten()
                .flat_map(|d| d.ancestors().map(|a| a.join("assets")).collect::<Vec<_>>())
                .find(|a| a.join("hands").is_dir())
        })
    }

    /// Đổi đường dẫn trong dự án thành đường dẫn thật: `builtin:…` → trong `assets/`, còn lại tính từ `base_dir`.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        if let Some(r) = rel.strip_prefix(BUILTIN_PREFIX) {
            return Ok(self.assets().context("không tìm thấy thư mục assets/ của app")?.join(r));
        }
        if let Some(r) = rel.strip_prefix(USER_PREFIX) {
            return Ok(self
                .user_dir
                .as_ref()
                .context("chưa có thư mục dữ liệu người dùng")?
                .join(r));
        }
        Ok(self.base_dir.join(rel))
    }
}

/// Kết quả bước chuẩn bị: gói biên dịch + báo cáo.
pub struct Prepared {
    pub bundle: Bundle,
    pub report: Vec<ElementReport>,
    /// Với mỗi scene, chỉ số báo cáo của từng asset (một element nhiều lớp → nhiều asset).
    pub owners: Vec<Vec<usize>>,
}

impl Prepared {
    /// Dựng Engine và điền thời lượng vẽ thật vào báo cáo.
    pub fn into_engine(self, out_width: Option<u32>) -> Result<(Engine, Vec<ElementReport>)> {
        let Prepared {
            bundle,
            mut report,
            owners,
        } = self;
        let engine = bundle.into_engine(out_width)?;
        fill_draw_times(&mut report, &owners, engine.timeline());
        Ok((engine, report))
    }
}

fn fill_draw_times(report: &mut [ElementReport], owners: &[Vec<usize>], timeline: &Timeline) {
    for (sc, owners) in timeline.scenes.iter().zip(owners) {
        for (el, ri) in sc.elements.iter().zip(owners) {
            report[*ri].draw_ms += el.draw_end_ms - el.draw_start_ms;
        }
    }
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

/// Đường dẫn font cho element chữ: font khai báo, hoặc font mặc định trong `assets/`.
pub fn font_path(ctx: &CompileContext, font: Option<&str>) -> Result<PathBuf> {
    match font {
        Some(f) => ctx.resolve(f),
        None => match ctx.assets() {
            Some(a) if a.join(text::DEFAULT_FONT_IN_ASSETS).is_file() => Ok(a.join(text::DEFAULT_FONT_IN_ASSETS)),
            _ => text::resolve_font(&ctx.base_dir, None),
        },
    }
}

/// Phân tích một element (ảnh hoặc chữ).
pub fn analyze_element(ctx: &CompileContext, el: &wb_model::Element) -> Result<AnalyzedImage> {
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
            analyze_file(&ctx.resolve(src)?, &opt)
        }
        ElementKind::Text {
            text,
            font,
            size,
            color,
            align,
            line_height,
        } => {
            let font_path = font_path(ctx, font.as_deref())?;
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

/// Đọc file dự án JSON và kiểm tra phiên bản schema.
pub fn read_project(project_path: &Path) -> Result<Project> {
    let text =
        std::fs::read_to_string(project_path).with_context(|| format!("không đọc được {}", project_path.display()))?;
    let project: Project =
        serde_json::from_str(&text).with_context(|| format!("file dự án không hợp lệ: {}", project_path.display()))?;
    check_schema(&project)?;
    Ok(project)
}

pub fn check_schema(project: &Project) -> Result<()> {
    if project.schema_version > wb_model::SCHEMA_VERSION {
        bail!(
            "dự án dùng schema v{} — app chỉ hỗ trợ tới v{}",
            project.schema_version,
            wb_model::SCHEMA_VERSION
        );
    }
    Ok(())
}

/// Đọc và biên dịch file dự án. `out_width`: ghi đè chiều rộng xuất (giữ tỉ lệ).
pub fn compile(project_path: &Path, out_width: Option<u32>) -> Result<Compiled> {
    let project = read_project(project_path)?;
    let ctx = CompileContext::new(project_path.parent().unwrap_or(Path::new(".")));
    let (engine, report) = prepare(&project, &ctx, None)?.into_engine(out_width)?;
    Ok(Compiled {
        engine,
        project,
        report,
    })
}

/// Phân tích mọi element (song song, dùng `cache` nếu có) và đóng gói thành [`Bundle`].
pub fn prepare(project: &Project, ctx: &CompileContext, cache: Option<&AnalysisCache>) -> Result<Prepared> {
    check_schema(project)?;
    let set = &project.settings;

    // Phân tích song song mọi element (mỗi element một luồng)
    let jobs: Vec<(usize, usize, &wb_model::Element)> = project
        .scenes
        .iter()
        .enumerate()
        .flat_map(|(si, sc)| sc.elements.iter().enumerate().map(move |(ei, el)| (si, ei, el)))
        .collect();
    type Job = Result<(usize, usize, Arc<AnalyzedImage>)>;
    let analyzed: Vec<Job> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|&(si, ei, el)| {
                scope.spawn(move || -> Job {
                    let a = match cache {
                        Some(c) => c.get_or_analyze(ctx, el)?,
                        None => Arc::new(analyze_element(ctx, el)?),
                    };
                    Ok((si, ei, a))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("luồng phân tích panic"))
            .collect()
    });
    let mut per_scene: Vec<Vec<Option<Arc<AnalyzedImage>>>> = project
        .scenes
        .iter()
        .map(|s| s.elements.iter().map(|_| None).collect())
        .collect();
    for r in analyzed {
        let (si, ei, a) = r?;
        per_scene[si][ei] = Some(a);
    }

    // Lắp ráp scene: mỗi lớp vẽ của một element thành một asset nối tiếp nhau
    let mut scenes = Vec::new();
    let mut report = Vec::new();
    let mut owners = Vec::new();
    for (si, sc) in project.scenes.iter().enumerate() {
        let bg =
            parse_hex_color(sc.background.as_deref().unwrap_or(&set.background)).context("màu nền không hợp lệ")?;
        let mut elements = Vec::new();
        let mut scene_owners = Vec::new();
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
            for (li, layer) in a.layers.iter().enumerate() {
                let (lw, lh) = layer.ink.dimensions();
                let ink = pixmap_from_rgba(lw, lh, layer.ink.as_raw()).context("ảnh rỗng")?;
                elements.push(BundleElement {
                    ink: RawImage::from_pixmap(&ink),
                    // lớp có thể ở độ phân giải khác ảnh nguồn → co giãn cho cùng khung
                    place: [scale * sw as f32 / lw as f32, ox, oy],
                    plan: layer.plan.clone(),
                    timing: layer_timing(el.timing, li, n),
                });
                scene_owners.push(report.len() - 1);
            }
        }
        scenes.push(BundleScene {
            background: bg,
            tail_ms: sc.tail_ms,
            elements,
        });
        owners.push(scene_owners);
    }
    let hand = match &set.hand {
        Some(rel) => Some(BundleHand::from_sprite(&load_hand(
            &ctx.resolve(rel)?,
            set.hand_mode,
            set.hand_scale,
        )?)),
        None => None,
    };
    Ok(Prepared {
        bundle: Bundle {
            width: set.width,
            height: set.height,
            reference_width: REFERENCE_WIDTH,
            draw_speed: set.draw_speed,
            scenes,
            hand,
        },
        report,
        owners,
    })
}

/// Hệ số phóng mặc định của kiểu "cả cánh tay".
pub const DEFAULT_FULL_SCALE: f32 = 1.4;

/// Nạp bàn tay từ thư mục chứa `hand.json` (ảnh PNG hoặc SVG) theo kiểu hiển thị và hệ số cỡ.
pub fn load_hand(dir: &Path, mode: HandMode, scale: f32) -> Result<HandSprite> {
    let meta = hands::read_meta(dir)?;
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
