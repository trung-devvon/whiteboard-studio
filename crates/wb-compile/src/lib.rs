//! "Biên dịch" dự án: tài liệu [`Project`] → phân tích ảnh/chữ → lớp vẽ + DrawPlan → [`Bundle`] → [`Engine`].
//!
//! Dùng chung cho CLI (`wb`) và app Tauri. App giữ một [`AnalysisCache`] để khi chỉ kéo/đổi cỡ element
//! thì không phải phân tích lại ảnh.

pub mod analyze;
pub mod cache;
pub mod debug;
pub mod hands;
pub mod icons;
pub mod narrate;
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
    /// Giọng đọc + tiếng bút + phụ đề.
    pub sound: Soundtrack,
    pub narration: Vec<SceneNarration>,
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
    /// Giọng đọc của từng scene (theo thứ tự scene).
    pub narration: Vec<SceneNarration>,
    /// Cài đặt tiếng bút + hạt giống ngẫu nhiên của dự án (tiếng bút tất định).
    pub sound: wb_model::SoundSettings,
    pub seed: u32,
}

/// Âm thanh + phụ đề của cả video.
#[derive(Debug, Clone, Default)]
pub struct Soundtrack {
    /// Giọng đọc + tiếng bút (mono, `wb_audio::SAMPLE_RATE`). `None` = video câm.
    pub audio: Option<Vec<i16>>,
    pub subtitles: Vec<wb_audio::subtitles::Cue>,
}

impl Soundtrack {
    /// File phụ cho bước xuất video `out`: rãnh tiếng → `<out>.sound.wav` (tạm, xoá sau khi xuất)
    /// và phụ đề → `<out>.srt` (giữ lại — dùng để tải lên YouTube…, `subtitles` = false thì không ghi).
    pub fn write_for_export(&self, out: &Path, subtitles: bool) -> Result<ExportFiles> {
        let mut f = ExportFiles::default();
        if let Some(pcm) = &self.audio {
            let wav = out.with_extension("sound.wav");
            wb_audio::wav::write(&wav, pcm, wb_audio::SAMPLE_RATE).context("không ghi được file tiếng tạm")?;
            f.audio = Some(wav);
        }
        if subtitles && !self.subtitles.is_empty() {
            let srt = out.with_extension("srt");
            std::fs::write(&srt, wb_audio::subtitles::to_srt(&self.subtitles))
                .with_context(|| format!("không ghi được {}", srt.display()))?;
            f.subtitles = Some(srt);
        }
        Ok(f)
    }
}

/// File phụ đã ghi cho một lần xuất video.
#[derive(Debug, Default)]
pub struct ExportFiles {
    pub audio: Option<PathBuf>,
    pub subtitles: Option<PathBuf>,
}

impl ExportFiles {
    /// Xoá file tiếng tạm (giữ file phụ đề).
    pub fn cleanup(&self) {
        if let Some(p) = &self.audio {
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Prepared {
    /// Dựng rãnh tiếng + phụ đề (giải Timeline từ gói — không cần dựng Engine).
    pub fn soundtrack(&self) -> Soundtrack {
        soundtrack(
            &self.bundle,
            &self.bundle.timeline(),
            &self.narration,
            &self.sound,
            self.seed,
        )
    }
}

/// Rãnh tiếng = giọng đọc (đúng đầu mỗi scene) + tiếng bút (đúng từng nét); phụ đề theo mốc từng từ.
pub fn soundtrack(
    bundle: &Bundle,
    timeline: &Timeline,
    narration: &[SceneNarration],
    sound: &wb_model::SoundSettings,
    seed: u32,
) -> Soundtrack {
    let voice = narration_track(narration, timeline);
    let strokes: Vec<wb_audio::sfx::PenStroke> = bundle
        .scenes
        .iter()
        .zip(&timeline.scenes)
        .flat_map(|(bs, ts)| bs.elements.iter().zip(&ts.elements))
        .flat_map(|(be, te)| be.plan.strokes.iter().zip(&te.slots))
        .map(|(s, slot)| wb_audio::sfx::PenStroke {
            t0_ms: slot.t0,
            t1_ms: slot.t1,
            brush: s.kind == wb_model::StrokeKind::Fill,
        })
        .collect();
    let pen = (sound.pen_volume > 0.0 && !strokes.is_empty())
        .then(|| wb_audio::sfx::pen_track(timeline.total_ms, &strokes, seed, sound.pen_volume));
    let audio = match (voice, pen) {
        (Some(mut v), Some(p)) => {
            for (a, b) in v.iter_mut().zip(&p) {
                *a = a.saturating_add(*b);
            }
            Some(v)
        }
        (v, p) => v.or(p),
    };
    let subtitles = narration
        .iter()
        .zip(&timeline.scenes)
        .filter_map(|(n, sc)| {
            let s = n.speech.as_ref()?;
            Some(wb_audio::subtitles::cues(
                &n.spoken,
                &s.words,
                s.duration_ms,
                sc.start_ms,
            ))
        })
        .flatten()
        .collect();
    Soundtrack { audio, subtitles }
}

/// Trạng thái giọng đọc của một scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrationState {
    /// Scene không có lời thoại.
    None,
    /// Đã có giọng đọc khớp lời thoại hiện tại.
    Ready,
    /// Có lời thoại nhưng chưa tạo giọng (hoặc lời/giọng đã đổi) → bấm "Tạo giọng đọc".
    Missing,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SceneNarration {
    pub scene: String,
    pub state: NarrationState,
    /// Khoá đệm `generated/tts/<key>` (rỗng nếu không có lời thoại).
    pub key: String,
    pub duration_ms: f64,
    /// Số `n` của các dấu `[n]` không trỏ tới element nào trong scene.
    pub unknown_markers: Vec<usize>,
    #[serde(skip)]
    pub speech: Option<Arc<wb_audio::Speech>>,
    /// Phần được đọc (bỏ dấu `[n]`) — chữ của phụ đề.
    #[serde(skip)]
    pub spoken: String,
}

impl SceneNarration {
    /// Lời thoại của scene → khoá đệm + giọng đã tạo (nếu có trong `generated/tts/`).
    pub fn load(ctx: &CompileContext, sc: &wb_model::Scene, voice: &wb_model::VoiceSettings) -> Self {
        let text = sc.narration.as_deref().unwrap_or("");
        let spoken = wb_audio::narration::spoken(text).trim().to_string();
        if spoken.is_empty() {
            return Self {
                scene: sc.id.clone(),
                state: NarrationState::None,
                key: String::new(),
                duration_ms: 0.0,
                unknown_markers: vec![],
                speech: None,
                spoken,
            };
        }
        let key = wb_audio::narration::cache_key(text, &voice.name, voice.rate);
        let unknown_markers = wb_audio::narration::parse(text)
            .into_iter()
            .filter_map(|p| match p {
                wb_audio::narration::Piece::Marker(n) if n > sc.elements.len() => Some(n),
                _ => None,
            })
            .collect();
        let speech = wb_audio::store::load(&ctx.base_dir, &key).ok().map(Arc::new);
        Self {
            scene: sc.id.clone(),
            state: if speech.is_some() {
                NarrationState::Ready
            } else {
                NarrationState::Missing
            },
            duration_ms: speech.as_ref().map_or(0.0, |s| s.duration_ms),
            key,
            unknown_markers,
            speech,
            spoken,
        }
    }
}

/// Rãnh tiếng cả video: giọng mỗi scene đặt đúng đầu scene đó. `None` nếu không scene nào có giọng.
pub fn narration_track(narration: &[SceneNarration], timeline: &Timeline) -> Option<Vec<i16>> {
    let clips: Vec<(f64, &[i16])> = narration
        .iter()
        .zip(&timeline.scenes)
        .filter_map(|(n, sc)| n.speech.as_ref().map(|s| (sc.start_ms, s.pcm.as_slice())))
        .collect();
    (!clips.is_empty()).then(|| wb_audio::mix::mix(timeline.total_ms, wb_audio::SAMPLE_RATE, &clips))
}

impl Prepared {
    /// Dựng Engine và điền thời lượng vẽ thật vào báo cáo.
    pub fn into_engine(self, out_width: Option<u32>) -> Result<(Engine, Vec<ElementReport>)> {
        let Prepared {
            bundle,
            mut report,
            owners,
            ..
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

/// Màu đổi (`tint`) của element ảnh, nếu có.
fn tint_of(el: &wb_model::Element) -> Result<Option<[u8; 4]>> {
    match &el.kind {
        ElementKind::Image { tint: Some(t), .. } => parse_hex_color(t)
            .map(Some)
            .with_context(|| format!("màu đổi không hợp lệ: {t}")),
        _ => Ok(None),
    }
}

/// Đổi mọi điểm có mực sang một màu, giữ độ trong (RGBA thẳng, không nhân trước).
/// Chạy lúc lắp gói, sau đệm phân tích → đổi màu không phải phân tích lại ảnh.
pub fn tinted(rgba: &[u8], color: [u8; 4]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        px[..3].copy_from_slice(&color[..3]);
    }
    out
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
            ..
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
    let prepared = prepare(&project, &ctx, None)?;
    let narration = prepared.narration.clone();
    let sound = prepared.soundtrack();
    let (engine, report) = prepared.into_engine(out_width)?;
    Ok(Compiled {
        engine,
        project,
        report,
        sound,
        narration,
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
    let mut narration = Vec::new();
    for (si, sc) in project.scenes.iter().enumerate() {
        let bg =
            parse_hex_color(sc.background.as_deref().unwrap_or(&set.background)).context("màu nền không hợp lệ")?;
        let mut elements = Vec::new();
        let mut scene_owners = Vec::new();
        let speech = SceneNarration::load(ctx, sc, &set.voice);
        for (ei, el) in sc.elements.iter().enumerate() {
            let anchor = speech.speech.as_ref().and_then(|s| s.anchor_of(ei + 1));
            let a = per_scene[si][ei].take().unwrap();
            let tint = tint_of(el)?;
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
                let ink = match tint {
                    Some(c) => pixmap_from_rgba(lw, lh, &tinted(layer.ink.as_raw(), c)),
                    None => pixmap_from_rgba(lw, lh, layer.ink.as_raw()),
                }
                .context("ảnh rỗng")?;
                elements.push(BundleElement {
                    ink: RawImage::from_pixmap(&ink),
                    // lớp có thể ở độ phân giải khác ảnh nguồn → co giãn cho cùng khung
                    place: [scale * sw as f32 / lw as f32, ox, oy],
                    plan: layer.plan.clone(),
                    timing: layer_timing(el.timing, li, n),
                    anchor_ms: if li == 0 { anchor.map(|a| a as f32) } else { None },
                });
                scene_owners.push(report.len() - 1);
            }
        }
        scenes.push(BundleScene {
            background: bg,
            tail_ms: sc.tail_ms,
            elements,
            narration_ms: speech.duration_ms as f32,
        });
        owners.push(scene_owners);
        narration.push(speech);
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
        narration,
        sound: set.sound.clone(),
        seed: set.seed,
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
