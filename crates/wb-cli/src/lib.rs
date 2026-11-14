//! "Biên dịch" dự án: đọc `project.json` → phân tích ảnh → lập kế hoạch nét → giải thời gian → [`Engine`].
//!
//! Đây là phần nối các crate lõi; CLI dùng nó, sau này app Tauri cũng sẽ dùng
//! (khi đó tách thành crate riêng, kèm cache DrawPlan theo hash — kế hoạch File 05 §4).

pub mod debug;

use anyhow::{Context, Result, bail};
use image::RgbaImage;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Instant;
use wb_engine::{ElementAsset, Engine, HandSprite, Pixmap, Place, SceneAsset, pixmap_from_rgba};
use wb_model::{DrawPlan, ElementKind, PointW, Project, REFERENCE_WIDTH, StrokeKind, VisionParams, parse_hex_color};
use wb_planner::PlannerParams;
use wb_timing::{ElementInput, SceneInput, TimingParams};
use wb_vision::binary::Binary;

/// Kết quả phân tích + lập kế hoạch cho một ảnh.
pub struct AnalyzedImage {
    pub ink: RgbaImage,
    pub plan: DrawPlan,
    /// Độ phủ của kế hoạch nét trên mặt nạ mực (0..1) — trước pha hoàn thiện.
    pub coverage: f32,
    pub work_scale: f32,
    pub ink_mask: Binary,
    pub millis: u128,
}

/// Phân tích một ảnh line-art và lập kế hoạch nét.
pub fn analyze_image(img: &RgbaImage, params: &VisionParams) -> AnalyzedImage {
    let t0 = Instant::now();
    let r = wb_vision::analyze_line_art(img, params);
    let pp = PlannerParams::for_scale(1.0 / r.work_scale, params.min_stroke_length);
    let plan = wb_planner::plan(r.strokes, img.width(), img.height(), StrokeKind::Outline, &pp);
    // độ phủ: quy nét về lưới làm việc rồi đo trên mặt nạ mực
    let s = r.work_scale;
    let work: Vec<Vec<PointW>> = plan
        .strokes
        .iter()
        .flat_map(|st| {
            let main: Vec<PointW> = st
                .points
                .iter()
                .map(|p| PointW::new(p.x * s, p.y * s, p.w * s))
                .collect();
            std::iter::once(main).chain(
                st.dots
                    .iter()
                    .map(move |d| vec![PointW::new(d.point.x * s, d.point.y * s, d.point.w * s)]),
            )
        })
        .collect();
    let coverage = wb_vision::coverage::coverage(&r.ink_mask, &work);
    AnalyzedImage {
        ink: r.ink,
        plan,
        coverage,
        work_scale: s,
        ink_mask: r.ink_mask,
        millis: t0.elapsed().as_millis(),
    }
}

/// Báo cáo từng element sau khi biên dịch.
pub struct ElementReport {
    pub scene: String,
    pub id: String,
    pub source: (u32, u32),
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

    // Phân tích song song mọi ảnh (mỗi ảnh một luồng)
    let jobs: Vec<(usize, usize, PathBuf, VisionParams)> = project
        .scenes
        .iter()
        .enumerate()
        .flat_map(|(si, sc)| {
            sc.elements.iter().enumerate().map(move |(ei, el)| {
                let ElementKind::Image { src, params, .. } = &el.kind;
                (si, ei, PathBuf::from(src), *params)
            })
        })
        .collect();
    let analyzed: Vec<Result<(usize, usize, AnalyzedImage)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(si, ei, src, params)| {
                let path = base_dir.join(src);
                scope.spawn(move || -> Result<(usize, usize, AnalyzedImage)> {
                    let img = load_image(&path)?;
                    Ok((*si, *ei, analyze_image(&img, params)))
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

    // Lắp ráp scene + giải thời gian
    let mut scenes = Vec::new();
    let mut report = Vec::new();
    for (si, sc) in project.scenes.iter().enumerate() {
        let bg =
            parse_hex_color(sc.background.as_deref().unwrap_or(&set.background)).context("màu nền không hợp lệ")?;
        let mut elements = Vec::new();
        for (ei, el) in sc.elements.iter().enumerate() {
            let a = per_scene[si][ei].take().unwrap();
            let (sw, sh) = a.ink.dimensions();
            let (scale, ox, oy) = el.transform.fit(sw, sh);
            report.push(ElementReport {
                scene: sc.id.clone(),
                id: el.id.clone(),
                source: (sw, sh),
                strokes: a.plan.stats.stroke_count,
                dots: a.plan.stats.dot_count,
                coverage: a.coverage,
                analyze_ms: a.millis,
                draw_ms: 0.0,
            });
            let ink = pixmap_from_rgba(sw, sh, a.ink.as_raw()).context("ảnh rỗng")?;
            elements.push(ElementAsset {
                ink,
                place: Place { scale, ox, oy },
                plan: a.plan,
            });
        }
        scenes.push(SceneAsset {
            background: bg,
            elements,
        });
    }
    let params = TimingParams {
        draw_speed: set.draw_speed,
        ..TimingParams::default()
    };
    let inputs: Vec<SceneInput> = project
        .scenes
        .iter()
        .zip(&scenes)
        .map(|(sc, asset)| SceneInput {
            elements: sc
                .elements
                .iter()
                .zip(&asset.elements)
                .map(|(el, a)| ElementInput {
                    plan: &a.plan,
                    scale: a.place.scale,
                    timing: el.timing,
                })
                .collect(),
            tail_ms: sc.tail_ms,
        })
        .collect();
    let timeline = wb_timing::resolve(&inputs, &params);
    let mut k = 0;
    for sc in &timeline.scenes {
        for el in &sc.elements {
            report[k].draw_ms = el.draw_end_ms - el.draw_start_ms;
            k += 1;
        }
    }
    let hand = match &set.hand {
        Some(rel) => Some(load_hand(&base_dir.join(rel))?),
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
}

/// Nạp bàn tay từ thư mục chứa `hand.json` (ảnh PNG hoặc SVG).
pub fn load_hand(dir: &Path) -> Result<HandSprite> {
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
    Ok(HandSprite {
        pixmap,
        anchor_x: meta.anchor[0] * unit,
        anchor_y: meta.anchor[1] * unit,
        display_height: meta.display_height,
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
