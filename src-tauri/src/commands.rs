//! Lệnh IPC. Mỗi lệnh chỉ chuyển tham số sang crate lõi (`wb-project`, `wb-compile`, `wb-export`)
//! và đổi lỗi thành chuỗi tiếng Việt cho giao diện.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::State;
use tauri::ipc::{Channel, Response};
use wb_compile::hands::{self, HandEntry, HandMeta};
use wb_compile::icons::IconIndex;
use wb_compile::{AnalysisCache, CompileContext, SceneNarration, narration_track, prepare};
use wb_model::Project;
use wb_project::ProjectDir;

pub struct AppState {
    assets_dir: Option<PathBuf>,
    /// Thư mục dữ liệu của app (thư viện tay người dùng…).
    user_dir: Option<PathBuf>,
    /// Đệm kết quả phân tích ảnh/chữ — kéo/đổi cỡ element không phải phân tích lại.
    cache: Arc<AnalysisCache>,
    /// Cờ huỷ của lần xuất video đang chạy.
    export_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// Chỉ mục tìm icon (nạp lần đầu dùng).
    icons: std::sync::OnceLock<Option<IconIndex>>,
}

impl AppState {
    pub fn new(assets_dir: Option<PathBuf>, user_dir: Option<PathBuf>) -> Self {
        Self {
            assets_dir,
            user_dir,
            cache: Arc::new(AnalysisCache::new()),
            export_cancel: Mutex::new(None),
            icons: std::sync::OnceLock::new(),
        }
    }

    fn ctx(&self, root: &str) -> CompileContext {
        CompileContext {
            base_dir: PathBuf::from(root),
            assets_dir: self.assets_dir.clone(),
            user_dir: self.user_dir.clone(),
        }
    }
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// Lỗi anyhow: in cả chuỗi nguyên nhân ("không đọc được …: không tìm thấy file").
fn err_chain(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[derive(Serialize)]
pub struct OpenedDto {
    root: String,
    project: Project,
    recovery: Option<Project>,
}

/// Tạo dự án mới (thư mục `*.wbproj`). `sample` = điền sẵn dự án mẫu để thử.
#[tauri::command]
pub fn project_create(path: String, sample: bool) -> CmdResult<OpenedDto> {
    let mut root = PathBuf::from(path);
    if root.extension().is_none_or(|e| e != wb_project::EXTENSION) {
        root.set_extension(wb_project::EXTENSION);
    }
    let project = if sample {
        wb_project::sample_project()
    } else {
        wb_project::new_project()
    };
    let dir = ProjectDir::create(&root, &project).map_err(err)?;
    Ok(OpenedDto {
        root: dir.root().display().to_string(),
        project,
        recovery: None,
    })
}

#[tauri::command]
pub fn project_open(path: String) -> CmdResult<OpenedDto> {
    let o = ProjectDir::open(&path).map_err(err)?;
    Ok(OpenedDto {
        root: o.dir.root().display().to_string(),
        project: o.project,
        recovery: o.recovery,
    })
}

fn dir(root: &str) -> CmdResult<ProjectDir> {
    Ok(ProjectDir::open(root).map_err(err)?.dir)
}

#[tauri::command]
pub fn project_save(root: String, project: Project) -> CmdResult<()> {
    dir(&root)?.save(&project).map_err(err)
}

#[tauri::command]
pub fn project_autosave(root: String, project: Project) -> CmdResult<()> {
    dir(&root)?.autosave(&project).map_err(err)
}

#[tauri::command]
pub fn project_discard_autosave(root: String) -> CmdResult<()> {
    dir(&root)?.discard_autosave().map_err(err)
}

/// Chép ảnh/font vào `assets/` của dự án, trả đường dẫn tương đối.
#[tauri::command]
pub fn asset_import(root: String, src: String) -> CmdResult<String> {
    dir(&root)?.import_asset(Path::new(&src)).map_err(err)
}

/// Thư viện tay: tay có sẵn của app + tay người dùng tự thêm (`user:hands/…`).
#[tauri::command]
pub fn hands_list(state: State<'_, AppState>) -> Vec<HandEntry> {
    let mut out = Vec::new();
    if let Some(assets) = state.ctx(".").assets() {
        out.extend(hands::list_hands(&assets, wb_compile::BUILTIN_PREFIX, false));
    }
    if let Some(user) = &state.user_dir {
        out.extend(hands::list_hands(user, wb_compile::USER_PREFIX, true));
    }
    // tay mặc định lên đầu, rồi tay có sẵn, rồi tay người dùng
    out.sort_by_key(|h| (h.path != wb_project::DEFAULT_HAND, h.editable, h.path.clone()));
    out
}

fn user_hands_dir(state: &AppState) -> CmdResult<PathBuf> {
    Ok(state
        .user_dir
        .as_ref()
        .ok_or("chưa có thư mục dữ liệu người dùng")?
        .join("hands"))
}

/// Thư mục trống mới trong thư viện tay người dùng, tên theo `name`.
fn new_hand_dir(state: &AppState, name: &str) -> CmdResult<(String, PathBuf)> {
    let base = user_hands_dir(state)?;
    let slug = hands::slug(name);
    let mut id = slug.clone();
    let mut n = 2;
    while base.join(&id).exists() {
        id = format!("{slug}-{n}");
        n += 1;
    }
    Ok((format!("{}hands/{id}", wb_compile::USER_PREFIX), base.join(id)))
}

/// Nhập ảnh tay: xoá nền, cắt gọn, đoán mũi bút → bộ tay mới của người dùng. Trả đường dẫn `user:hands/…`.
#[tauri::command]
pub async fn hand_import(state: State<'_, AppState>, src: String, name: String) -> CmdResult<String> {
    let (path, dir) = new_hand_dir(&state, &name)?;
    tauri::async_runtime::spawn_blocking(move || {
        hands::import_hand(Path::new(&src), &dir, &name).map_err(|e| {
            let _ = std::fs::remove_dir_all(&dir);
            err_chain(e)
        })
    })
    .await
    .map_err(err)??;
    Ok(path)
}

/// Sao chép một bộ tay (vd. tay có sẵn) vào thư viện người dùng để chỉnh.
#[tauri::command]
pub fn hand_duplicate(state: State<'_, AppState>, path: String) -> CmdResult<String> {
    let src = state.ctx(".").resolve(&path).map_err(err_chain)?;
    let mut meta = hands::read_meta(&src).map_err(err_chain)?;
    meta.name = format!("{} (bản sao)", meta.name);
    let (new_path, dir) = new_hand_dir(&state, &meta.name)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    std::fs::copy(src.join(&meta.file), dir.join(&meta.file)).map_err(err)?;
    hands::write_meta(&dir, &meta).map_err(err_chain)?;
    Ok(new_path)
}

#[derive(Serialize)]
pub struct HandDetail {
    meta: HandMeta,
    width: u32,
    height: u32,
    editable: bool,
}

#[tauri::command]
pub fn hand_load(state: State<'_, AppState>, path: String) -> CmdResult<HandDetail> {
    let dir = state.ctx(".").resolve(&path).map_err(err_chain)?;
    let meta = hands::read_meta(&dir).map_err(err_chain)?;
    let (width, height) = image::image_dimensions(dir.join(&meta.file)).map_err(err)?;
    Ok(HandDetail {
        meta,
        width,
        height,
        editable: path.starts_with(wb_compile::USER_PREFIX),
    })
}

/// Ảnh xem trước (PNG, cạnh dài ≤ `max_side`) của bộ tay.
#[tauri::command]
pub async fn hand_image(state: State<'_, AppState>, path: String, max_side: u32) -> CmdResult<Response> {
    let dir = state.ctx(".").resolve(&path).map_err(err_chain)?;
    tauri::async_runtime::spawn_blocking(move || {
        hands::preview_png(&dir, max_side)
            .map(|(png, _, _)| Response::new(png))
            .map_err(err_chain)
    })
    .await
    .map_err(err)?
}

/// Lưu mũi bút / đường cổ tay / cỡ … của bộ tay người dùng.
#[tauri::command]
pub fn hand_save(state: State<'_, AppState>, path: String, meta: HandMeta) -> CmdResult<()> {
    if !path.starts_with(wb_compile::USER_PREFIX) {
        return Err("tay có sẵn của app không sửa được — hãy tạo bản sao".into());
    }
    let dir = state.ctx(".").resolve(&path).map_err(err_chain)?;
    hands::write_meta(&dir, &meta).map_err(err_chain)
}

#[tauri::command]
pub fn hand_delete(state: State<'_, AppState>, path: String) -> CmdResult<()> {
    if !path.starts_with(wb_compile::USER_PREFIX) {
        return Err("không xoá được tay có sẵn của app".into());
    }
    let dir = state.ctx(".").resolve(&path).map_err(err_chain)?;
    std::fs::remove_dir_all(&dir).map_err(err)
}

#[derive(Serialize)]
pub struct IconDto {
    name: String,
    /// Giá trị ghi vào element ảnh (`builtin:icons/lucide/icons/….svg`).
    src: String,
    /// Nội dung SVG để hiện ô xem trước.
    svg: String,
}

/// Tìm icon có sẵn (tiếng Anh hoặc tiếng Việt). Chuỗi rỗng → danh sách gợi ý.
#[tauri::command]
pub fn icons_search(state: State<'_, AppState>, query: String, limit: Option<usize>) -> CmdResult<Vec<IconDto>> {
    let assets = state.ctx(".").assets();
    let index = state.icons.get_or_init(|| {
        assets
            .as_ref()
            .and_then(|a| IconIndex::load(&wb_compile::icons::lucide_dir(a)).ok())
    });
    let (Some(index), Some(assets)) = (index, assets) else {
        return Err("không tìm thấy thư viện icon (assets/icons/lucide)".into());
    };
    Ok(index
        .search(&query, limit.unwrap_or(120))
        .into_iter()
        .filter_map(|h| {
            let path = assets.join(h.src.strip_prefix(wb_compile::BUILTIN_PREFIX)?);
            Some(IconDto {
                svg: std::fs::read_to_string(path).ok()?,
                name: h.name,
                src: h.src,
            })
        })
        .collect())
}

#[derive(Serialize)]
struct ElementInfo {
    scene: String,
    id: String,
    kind: String,
    style: String,
    layers: Vec<&'static str>,
    strokes: u32,
    coverage: f32,
    analyze_ms: u128,
}

#[derive(Serialize)]
struct PreviewHeader {
    elements: Vec<ElementInfo>,
    compile_ms: u128,
    /// Trạng thái giọng đọc từng scene.
    narration: Vec<SceneNarration>,
    /// Gói có kèm rãnh tiếng WAV ở cuối không.
    has_audio: bool,
}

/// Biên dịch dự án cho Player. Trả nhị phân:
/// `[u32 LE độ dài JSON][JSON báo cáo][u32 LE độ dài Bundle][Bundle postcard][WAV giọng đọc nếu `has_audio`]`.
/// Phân tích ảnh chạy ở luồng nền (không chặn giao diện) và dùng đệm.
#[tauri::command]
pub async fn compile_preview(
    state: State<'_, AppState>,
    root: String,
    project: Project,
    width: u32,
) -> CmdResult<Response> {
    let ctx = state.ctx(&root);
    let cache = state.cache.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<Response> {
        let t0 = Instant::now();
        let mut prepared = prepare(&project, &ctx, Some(&cache)).map_err(err_chain)?;
        let audio = narration_track(&prepared.narration, &prepared.bundle.timeline());
        prepared.bundle.fit_for_preview(width);
        let header = PreviewHeader {
            elements: prepared
                .report
                .iter()
                .map(|r| ElementInfo {
                    scene: r.scene.clone(),
                    id: r.id.clone(),
                    kind: format!("{:?}", r.kind),
                    style: format!("{:?}", r.style),
                    layers: r.layers.clone(),
                    strokes: r.strokes,
                    coverage: r.coverage,
                    analyze_ms: r.analyze_ms,
                })
                .collect(),
            compile_ms: t0.elapsed().as_millis(),
            narration: prepared.narration.clone(),
            has_audio: audio.is_some(),
        };
        let json = serde_json::to_vec(&header).map_err(err)?;
        let bundle = prepared.bundle.to_bytes();
        let wav = audio
            .map(|pcm| wb_audio::wav::encode(&pcm, wb_audio::SAMPLE_RATE))
            .unwrap_or_default();
        let mut out = Vec::with_capacity(8 + json.len() + bundle.len() + wav.len());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
        out.extend_from_slice(&bundle);
        out.extend_from_slice(&wav);
        Ok(Response::new(out))
    })
    .await
    .map_err(err)?
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExportEvent {
    /// Đang phân tích ảnh / dựng engine.
    Preparing,
    Progress {
        frame: u64,
        total: u64,
    },
    Done {
        path: String,
        seconds: f64,
        encoder: String,
    },
}

/// Xuất MP4. Tiến độ gửi qua `on_event`; huỷ bằng `export_cancel`.
#[tauri::command]
pub async fn export_video(
    state: State<'_, AppState>,
    root: String,
    project: Project,
    out: String,
    width: Option<u32>,
    on_event: Channel<ExportEvent>,
) -> CmdResult<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    *state.export_cancel.lock().unwrap() = Some(cancel.clone());
    let ctx = state.ctx(&root);
    let cache = state.cache.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> CmdResult<()> {
        let _ = on_event.send(ExportEvent::Preparing);
        let prepared = prepare(&project, &ctx, Some(&cache)).map_err(err_chain)?;
        let narration = prepared.narration.clone();
        let (mut engine, _) = prepared.into_engine(width).map_err(err_chain)?;
        // giọng đọc → WAV tạm, FFmpeg trộn vào video
        let wav = std::env::temp_dir().join(format!("wb-export-{}.wav", std::process::id()));
        let audio = match narration_track(&narration, engine.timeline()) {
            Some(pcm) => {
                wb_audio::wav::write(&wav, &pcm, wb_audio::SAMPLE_RATE).map_err(err)?;
                Some(wav.clone())
            }
            None => None,
        };
        let opts = wb_export::ExportOptions {
            fps: project.settings.fps,
            cancel,
            audio,
            ..Default::default()
        };
        let mut last = Instant::now();
        let res = wb_export::export_mp4(&mut engine, Path::new(&out), &opts, |frame, total| {
            // ~10 lần/giây là đủ cho thanh tiến độ
            if last.elapsed().as_millis() >= 100 || frame == total {
                last = Instant::now();
                let _ = on_event.send(ExportEvent::Progress { frame, total });
            }
        });
        let _ = std::fs::remove_file(&wav);
        let stats = res.map_err(err)?;
        let _ = on_event.send(ExportEvent::Done {
            path: out,
            seconds: stats.seconds,
            encoder: format!("{:?}", stats.encoder),
        });
        Ok(())
    })
    .await
    .map_err(err)?;
    *state.export_cancel.lock().unwrap() = None;
    result
}

#[tauri::command]
pub fn export_cancel(state: State<'_, AppState>) {
    if let Some(c) = state.export_cancel.lock().unwrap().as_ref() {
        c.store(true, Ordering::Relaxed);
    }
}

// ───────────────────────── Giọng đọc (Giai đoạn 4) ─────────────────────────

/// Trạng thái key Azure — không bao giờ trả key về giao diện.
#[derive(Serialize)]
pub struct VoiceStatus {
    configured: bool,
    /// `keyring` (nhập trong app) hoặc `env` (file `.env` khi phát triển).
    source: Option<wb_audio::credentials::Source>,
    region: Option<String>,
}

#[tauri::command]
pub fn voice_status() -> VoiceStatus {
    match wb_audio::credentials::load() {
        Some((cfg, source)) => VoiceStatus {
            configured: true,
            source: Some(source),
            region: Some(cfg.region),
        },
        None => VoiceStatus {
            configured: false,
            source: None,
            region: None,
        },
    }
}

/// Lưu key + vùng vào kho mật khẩu Windows sau khi thử đọc một câu ngắn (key sai → không lưu).
#[tauri::command]
pub async fn voice_save(key: String, region: String) -> CmdResult<VoiceStatus> {
    let cfg = wb_audio::AzureConfig {
        key: key.trim().to_string(),
        region: region.trim().to_lowercase(),
    };
    if cfg.key.is_empty() || cfg.region.is_empty() {
        return Err("cần nhập cả key và vùng (region)".into());
    }
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<VoiceStatus> {
        let ssml = wb_audio::narration::ssml("Xin chào.", wb_model::DEFAULT_VOICE, 1.0);
        wb_audio::synthesize(&cfg, &ssml).map_err(err)?;
        wb_audio::credentials::save(&cfg).map_err(|e| format!("không lưu được vào kho mật khẩu: {e}"))?;
        Ok(voice_status())
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub fn voice_clear() -> CmdResult<VoiceStatus> {
    wb_audio::credentials::clear().map_err(err)?;
    Ok(voice_status())
}

/// Đọc thử một câu bằng giọng/tốc độ đang chọn → WAV để giao diện phát.
#[tauri::command]
pub async fn voice_preview(voice: String, rate: f32, text: String) -> CmdResult<Response> {
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<Response> {
        let (cfg, _) = wb_audio::credentials::load().ok_or_else(|| wb_audio::TtsError::NoKey.to_string())?;
        let text: String = text.chars().take(200).collect();
        let speech = wb_audio::synthesize(&cfg, &wb_audio::narration::ssml(&text, &voice, rate)).map_err(err)?;
        Ok(Response::new(wb_audio::wav::encode(&speech.pcm, wb_audio::SAMPLE_RATE)))
    })
    .await
    .map_err(err)?
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrateEvent {
    Progress { done: usize, total: usize },
}

/// Tạo giọng cho các scene có lời thoại mà chưa có giọng (lưu vào `generated/tts/` của dự án).
#[tauri::command]
pub async fn narration_generate(
    state: State<'_, AppState>,
    root: String,
    project: Project,
    on_event: Channel<NarrateEvent>,
) -> CmdResult<wb_compile::narrate::NarrateSummary> {
    let ctx = state.ctx(&root);
    tauri::async_runtime::spawn_blocking(move || {
        let (cfg, _) = wb_audio::credentials::load().ok_or_else(|| wb_audio::TtsError::NoKey.to_string())?;
        wb_compile::narrate::generate(&project, &ctx, &cfg, |done, total| {
            let _ = on_event.send(NarrateEvent::Progress { done, total });
        })
        .map_err(err_chain)
    })
    .await
    .map_err(err)?
}
