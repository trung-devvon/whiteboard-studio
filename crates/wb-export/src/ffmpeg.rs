//! Tìm, dò và chạy FFmpeg.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    /// Tự dò theo thứ tự nvenc → amf → qsv → openh264.
    Auto,
    Nvenc,
    Amf,
    Qsv,
    OpenH264,
}

impl Encoder {
    pub fn ffmpeg_name(self) -> &'static str {
        match self {
            Encoder::Auto | Encoder::OpenH264 => "libopenh264",
            Encoder::Nvenc => "h264_nvenc",
            Encoder::Amf => "h264_amf",
            Encoder::Qsv => "h264_qsv",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "auto" => Self::Auto,
            "nvenc" | "h264_nvenc" => Self::Nvenc,
            "amf" | "h264_amf" => Self::Amf,
            "qsv" | "h264_qsv" => Self::Qsv,
            "openh264" | "libopenh264" => Self::OpenH264,
            _ => return None,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("không tìm thấy FFmpeg — đặt vào tools/ffmpeg/ hoặc khai báo biến môi trường WB_FFMPEG")]
    NotFound,
    #[error("không encoder H.264 nào chạy được trên máy này")]
    NoEncoder,
    #[error("không chạy được FFmpeg: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("FFmpeg báo lỗi — xem log: {}", log.display())]
    Failed { log: PathBuf },
}

/// Tìm FFmpeg: `WB_FFMPEG` → `tools/ffmpeg/*/bin/` (tìm ngược từ thư mục hiện tại và thư mục chứa file chạy) → PATH.
pub fn find_ffmpeg() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("WB_FFMPEG") {
        return Some(PathBuf::from(p));
    }
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut roots = Vec::new();
    if let Ok(d) = std::env::current_dir() {
        roots.push(d);
    }
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        roots.push(d);
    }
    for root in roots {
        for dir in root.ancestors() {
            if let Ok(entries) = std::fs::read_dir(dir.join("tools").join("ffmpeg")) {
                for e in entries.flatten() {
                    let cand = e.path().join("bin").join(exe);
                    if cand.is_file() {
                        return Some(cand);
                    }
                }
            }
        }
    }
    Command::new(exe)
        .arg("-version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| PathBuf::from(exe))
}

/// Thử từng encoder bằng một video đen 0,2s → trả encoder đầu tiên chạy được (nhớ cho cả phiên).
pub fn probe_encoder(ffmpeg: &Path) -> Result<Encoder, FfmpegError> {
    static CACHE: OnceLock<Option<Encoder>> = OnceLock::new();
    let found = *CACHE.get_or_init(|| {
        [Encoder::Nvenc, Encoder::Amf, Encoder::Qsv, Encoder::OpenH264]
            .into_iter()
            .find(|e| {
                Command::new(ffmpeg)
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "lavfi",
                        "-i",
                        "color=c=black:s=256x256:d=0.2",
                        "-c:v",
                        e.ffmpeg_name(),
                        "-f",
                        "null",
                        "-",
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success())
            })
    });
    found.ok_or(FfmpegError::NoEncoder)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_encoder(
    ffmpeg: &Path,
    w: u32,
    h: u32,
    fps: u32,
    enc: Encoder,
    bitrate: u32,
    audio: Option<(&Path, f64)>,
    out: &Path,
    log: &Path,
) -> Result<Child, FfmpegError> {
    let log_file = std::fs::File::create(log)?;
    let mut cmd = Command::new(ffmpeg);
    cmd.args([
        "-hide_banner",
        "-y",
        "-loglevel",
        "info",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
    ])
    .args(["-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-"]);
    // giọng đọc: WAV thứ hai, cắt từ đầu khoảng xuất; AAC (encoder có sẵn trong bản LGPL)
    if let Some((wav, from_ms)) = audio {
        cmd.args(["-ss", &format!("{:.3}", from_ms / 1000.0), "-i"])
            .arg(wav)
            .args([
                "-map",
                "0:v:0",
                "-map",
                "1:a:0",
                "-c:a",
                "aac",
                "-b:a",
                "160k",
                "-shortest",
            ]);
    }
    cmd
        // RGB → YUV đúng ma trận BT.709, dải TV (tránh video nhạt/lệch màu)
        .args(["-vf", "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p"])
        .args(["-c:v", enc.ffmpeg_name(), "-b:v", &bitrate.to_string()]);
    if enc == Encoder::Nvenc {
        cmd.args(["-preset", "p5"]);
    }
    cmd.args([
        "-colorspace",
        "bt709",
        "-color_primaries",
        "bt709",
        "-color_trc",
        "bt709",
        "-color_range",
        "tv",
    ])
    // openh264/Media Foundation bỏ qua cờ màu ở trên → ghi thẳng vào VUI H.264 (phát hiện ở spike S1)
    .args([
        "-bsf:v",
        "h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1:video_full_range_flag=0",
    ])
    .args(["-movflags", "+faststart"])
    .arg(out)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::from(log_file));
    Ok(cmd.spawn()?)
}
