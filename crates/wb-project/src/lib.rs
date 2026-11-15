//! Thư mục dự án `*.wbproj` trên đĩa:
//!
//! ```text
//! bai-hoc.wbproj/
//!   project.json     ← tài liệu dự án (nguồn sự thật)
//!   autosave.json    ← bản tự lưu; chỉ tồn tại khi có thay đổi chưa lưu
//!   assets/          ← ảnh/font người dùng nhập vào (bản sao, tên kèm mã băm nội dung)
//! ```
//!
//! - **Ghi atomic**: ghi ra file tạm cùng thư mục, `fsync`, rồi đổi tên đè lên file cũ → mất điện/kill
//!   app giữa chừng không bao giờ để lại `project.json` dở dang.
//! - **Autosave + khôi phục**: app gọi [`ProjectDir::autosave`] định kỳ khi có thay đổi. Lần mở sau, nếu
//!   `autosave.json` còn (tức lần trước chưa lưu mà app đã tắt), [`Opened::recovery`] chứa bản đó để hỏi
//!   người dùng có khôi phục không.
//! - **Migration**: file được đọc dạng JSON thô rồi nâng dần từng phiên bản schema lên hiện tại
//!   (`migrate`) — khung sẵn cho các thay đổi cấu trúc sau này.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use wb_model::{Element, ElementKind, Project, ProjectSettings, SCHEMA_VERSION, Scene, Timing, Transform};

pub const PROJECT_FILE: &str = "project.json";
pub const AUTOSAVE_FILE: &str = "autosave.json";
pub const ASSETS_DIR: &str = "assets";
/// Đuôi thư mục dự án.
pub const EXTENSION: &str = "wbproj";
/// Bàn tay mặc định của dự án mới (tài nguyên có sẵn của app).
pub const DEFAULT_HAND: &str = "builtin:hands/pen-brown";

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("file dự án không hợp lệ ({path}): {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("dự án dùng schema v{0} — app này chỉ hỗ trợ tới v{SCHEMA_VERSION}, hãy cập nhật app")]
    TooNew(u32),
    #[error("thiếu trường schema_version — không phải file dự án Whiteboard Studio?")]
    NoVersion,
    #[error("{0} đã có dự án — chọn thư mục khác")]
    AlreadyExists(PathBuf),
    #[error("{0} không có {PROJECT_FILE}")]
    NotAProject(PathBuf),
}

pub type Result<T> = std::result::Result<T, ProjectError>;

fn io<T>(path: &Path, r: std::io::Result<T>) -> Result<T> {
    r.map_err(|source| ProjectError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Một thư mục dự án đã mở.
#[derive(Debug, Clone)]
pub struct ProjectDir {
    root: PathBuf,
}

/// Kết quả mở dự án.
#[derive(Debug)]
pub struct Opened {
    pub dir: ProjectDir,
    pub project: Project,
    /// Bản tự lưu khác bản đã lưu (lần trước app tắt khi chưa lưu) — hỏi người dùng có dùng không.
    pub recovery: Option<Project>,
}

impl ProjectDir {
    /// Tạo dự án mới tại `root` (thư mục được tạo nếu chưa có; không được chứa dự án khác).
    pub fn create(root: impl Into<PathBuf>, project: &Project) -> Result<Self> {
        let root = root.into();
        if root.join(PROJECT_FILE).exists() {
            return Err(ProjectError::AlreadyExists(root));
        }
        io(&root, fs::create_dir_all(root.join(ASSETS_DIR)))?;
        let dir = Self { root };
        dir.save(project)?;
        Ok(dir)
    }

    /// Mở dự án. Chấp nhận cả đường dẫn thư mục lẫn đường dẫn tới `project.json`.
    pub fn open(path: impl AsRef<Path>) -> Result<Opened> {
        let path = path.as_ref();
        let root = if path.is_file() {
            path.parent().unwrap_or(Path::new(".")).to_path_buf()
        } else {
            path.to_path_buf()
        };
        let file = root.join(PROJECT_FILE);
        if !file.is_file() {
            return Err(ProjectError::NotAProject(root));
        }
        let project = read_project(&file)?;
        let auto = root.join(AUTOSAVE_FILE);
        // bản tự lưu hỏng thì bỏ qua (không chặn việc mở dự án)
        let recovery = auto
            .is_file()
            .then(|| read_project(&auto).ok())
            .flatten()
            .filter(|p| *p != project);
        Ok(Opened {
            dir: Self { root },
            project,
            recovery,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn project_file(&self) -> PathBuf {
        self.root.join(PROJECT_FILE)
    }

    /// Lưu (atomic) và xoá bản tự lưu.
    pub fn save(&self, project: &Project) -> Result<()> {
        write_json_atomic(&self.project_file(), project)?;
        self.discard_autosave()
    }

    /// Ghi bản tự lưu (atomic). Không đụng tới `project.json`.
    pub fn autosave(&self, project: &Project) -> Result<()> {
        write_json_atomic(&self.root.join(AUTOSAVE_FILE), project)
    }

    pub fn discard_autosave(&self) -> Result<()> {
        let auto = self.root.join(AUTOSAVE_FILE);
        match fs::remove_file(&auto) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => io(&auto, Err(e)),
            _ => Ok(()),
        }
    }

    /// Chép file `src` vào `assets/` của dự án; trả đường dẫn tương đối để ghi vào element.
    /// Tên file kèm mã băm nội dung → nhập lại cùng ảnh không tạo bản sao mới, hai ảnh trùng tên không đè nhau.
    pub fn import_asset(&self, src: &Path) -> Result<String> {
        let data = io(src, fs::read(src))?;
        let stem = src
            .file_stem()
            .map(|s| sanitize(&s.to_string_lossy()))
            .unwrap_or_else(|| "tep".into());
        let ext = src
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        let name = format!("{stem}-{:016x}{ext}", fnv1a(&data));
        let dest = self.root.join(ASSETS_DIR).join(&name);
        if !dest.exists() {
            io(&self.root, fs::create_dir_all(self.root.join(ASSETS_DIR)))?;
            write_atomic(&dest, &data)?;
        }
        Ok(format!("{ASSETS_DIR}/{name}"))
    }
}

/// Đọc file dự án, nâng schema về hiện tại.
pub fn read_project(path: &Path) -> Result<Project> {
    let text = io(path, fs::read_to_string(path))?;
    let json = |source| ProjectError::Json {
        path: path.to_path_buf(),
        source,
    };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(json)?;
    migrate(value).map_err(|e| match e {
        MigrateError::Json(source) => json(source),
        MigrateError::Project(p) => p,
    })
}

enum MigrateError {
    Json(serde_json::Error),
    Project(ProjectError),
}

/// Nâng tài liệu JSON từ phiên bản schema cũ lên [`SCHEMA_VERSION`] rồi đọc thành [`Project`].
///
/// Mỗi lần đổi cấu trúc không tương thích: tăng `SCHEMA_VERSION` và thêm một bước `vN → vN+1` ở đây
/// (sửa trực tiếp trên `serde_json::Value`), kèm test với file mẫu của phiên bản cũ.
fn migrate(mut value: serde_json::Value) -> std::result::Result<Project, MigrateError> {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or(MigrateError::Project(ProjectError::NoVersion))? as u32;
    if version > SCHEMA_VERSION {
        return Err(MigrateError::Project(ProjectError::TooNew(version)));
    }
    // (chưa có bước nâng cấp nào: v1 là phiên bản đầu tiên)
    value["schema_version"] = SCHEMA_VERSION.into();
    serde_json::from_value(value).map_err(MigrateError::Json)
}

/// Ghi `bytes` vào `path` an toàn: file tạm cùng thư mục → fsync → đổi tên đè.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    let r = (|| -> std::io::Result<()> {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        // Windows: rename đè file đích (MoveFileExW + REPLACE_EXISTING)
        fs::rename(&tmp, path)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    io(path, r)
}

fn write_json_atomic(path: &Path, project: &Project) -> Result<()> {
    let text = serde_json::to_string_pretty(project).map_err(|source| ProjectError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    write_atomic(path, text.as_bytes())
}

/// Dự án mới: một scene trống, tay mặc định.
pub fn new_project() -> Project {
    Project {
        schema_version: SCHEMA_VERSION,
        settings: ProjectSettings {
            hand: Some(DEFAULT_HAND.into()),
            ..ProjectSettings::default()
        },
        scenes: vec![new_scene(1)],
    }
}

pub fn new_scene(n: usize) -> Scene {
    Scene {
        id: format!("s{n}"),
        name: Some(format!("Cảnh {n}")),
        background: None,
        elements: vec![],
        tail_ms: 1500,
    }
}

/// Element ảnh mới, đặt giữa khung tham chiếu.
pub fn new_image_element(id: String, src: String) -> Element {
    Element {
        id,
        kind: ElementKind::Image {
            src,
            pipeline: Default::default(),
            style: Default::default(),
            background: Default::default(),
            params: Default::default(),
        },
        transform: Transform {
            x: 460.0,
            y: 190.0,
            w: 1000.0,
            h: 700.0,
        },
        timing: Timing::default(),
    }
}

fn sanitize(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .take(40)
        .collect();
    if t.is_empty() { "tep".into() } else { t }
}

/// FNV-1a 64 bit — mã băm nội dung ổn định giữa các phiên bản (khác `DefaultHasher`).
fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests;
