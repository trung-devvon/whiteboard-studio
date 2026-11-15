//! Lệnh IPC. Mỗi lệnh chỉ chuyển tham số sang crate lõi (`wb-project`, `wb-compile`, `wb-export`)
//! và đổi lỗi thành chuỗi tiếng Việt cho giao diện.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::State;
use tauri::ipc::{Channel, Response};
use wb_compile::{AnalysisCache, CompileContext, prepare};
use wb_model::Project;
use wb_project::ProjectDir;

pub struct AppState {
    assets_dir: Option<PathBuf>,
    /// Đệm kết quả phân tích ảnh/chữ — kéo/đổi cỡ element không phải phân tích lại.
    cache: Arc<AnalysisCache>,
    /// Cờ huỷ của lần xuất video đang chạy.
    export_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new(assets_dir: Option<PathBuf>) -> Self {
        Self {
            assets_dir,
            cache: Arc::new(AnalysisCache::new()),
            export_cancel: Mutex::new(None),
        }
    }

    fn ctx(&self, root: &str) -> CompileContext {
        CompileContext {
            base_dir: PathBuf::from(root),
            assets_dir: self.assets_dir.clone(),
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

/// Tạo dự án mới (thư mục `*.wbproj`).
#[tauri::command]
pub fn project_create(path: String) -> CmdResult<OpenedDto> {
    let mut root = PathBuf::from(path);
    if root.extension().is_none_or(|e| e != wb_project::EXTENSION) {
        root.set_extension(wb_project::EXTENSION);
    }
    let project = wb_project::new_project();
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

#[derive(Serialize)]
pub struct HandInfo {
    /// Giá trị ghi vào `settings.hand`, vd. `builtin:hands/pen-brown`.
    path: String,
    name: String,
}

/// Các bộ tay có sẵn của app.
#[tauri::command]
pub fn builtin_hands(state: State<'_, AppState>) -> Vec<HandInfo> {
    let Some(assets) = state.ctx(".").assets() else {
        return vec![];
    };
    let mut out: Vec<HandInfo> = std::fs::read_dir(assets.join("hands"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let meta = std::fs::read_to_string(e.path().join("hand.json")).ok()?;
            let v: serde_json::Value = serde_json::from_str(&meta).ok()?;
            let id = e.file_name().to_string_lossy().into_owned();
            Some(HandInfo {
                name: v["name"].as_str().unwrap_or(&id).to_string(),
                path: format!("{}hands/{id}", wb_compile::BUILTIN_PREFIX),
            })
        })
        .collect();
    // tay mặc định lên đầu
    out.sort_by_key(|h| (h.path != wb_project::DEFAULT_HAND, h.path.clone()));
    out
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
}

/// Biên dịch dự án cho Player. Trả nhị phân: `[u32 LE độ dài JSON][JSON báo cáo][Bundle postcard]`.
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
        };
        let json = serde_json::to_vec(&header).map_err(err)?;
        let bundle = prepared.bundle.to_bytes();
        let mut out = Vec::with_capacity(4 + json.len() + bundle.len());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&bundle);
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
        let (mut engine, _) = prepare(&project, &ctx, Some(&cache))
            .and_then(|p| p.into_engine(width))
            .map_err(err_chain)?;
        let opts = wb_export::ExportOptions {
            fps: project.settings.fps,
            cancel,
            ..Default::default()
        };
        let mut last = Instant::now();
        let stats = wb_export::export_mp4(&mut engine, Path::new(&out), &opts, |frame, total| {
            // ~10 lần/giây là đủ cho thanh tiến độ
            if last.elapsed().as_millis() >= 100 || frame == total {
                last = Instant::now();
                let _ = on_event.send(ExportEvent::Progress { frame, total });
            }
        })
        .map_err(err)?;
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
