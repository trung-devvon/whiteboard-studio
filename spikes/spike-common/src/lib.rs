//! Tiện ích dùng chung cho spike: tìm FFmpeg, mở tiến trình FFmpeg nhận khung RGBA qua stdin.
//! (Phiên bản thật sẽ nằm trong `wb-export` ở Giai đoạn 1.)

use anyhow::{Context, Result, bail};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

/// Thư mục gốc workspace (spikes/<tên>/ → ../..).
pub fn workspace_root() -> PathBuf {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    here.parent().and_then(Path::parent).unwrap().to_path_buf()
}

/// Thư mục đầu ra của spike: `spikes/out/` (đã gitignore).
pub fn out_dir() -> PathBuf {
    let d = workspace_root().join("spikes").join("out");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Tìm ffmpeg: biến môi trường `WB_FFMPEG` → `tools/ffmpeg/*/bin/ffmpeg(.exe)` → PATH.
pub fn find_tool(name: &str) -> Result<PathBuf> {
    let env_key = format!("WB_{}", name.to_uppercase());
    if let Ok(p) = std::env::var(&env_key) {
        return Ok(PathBuf::from(p));
    }
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let tools = workspace_root().join("tools").join("ffmpeg");
    if let Ok(entries) = std::fs::read_dir(&tools) {
        for e in entries.flatten() {
            let cand = e.path().join("bin").join(&exe);
            if cand.exists() {
                return Ok(cand);
            }
        }
    }
    Ok(PathBuf::from(exe)) // hy vọng có trong PATH
}

/// Tiến trình FFmpeg nhận khung RGBA thô qua stdin.
pub struct FfmpegWriter {
    child: Child,
    stdin: Option<ChildStdin>,
    pub stderr_path: PathBuf,
}

impl FfmpegWriter {
    /// `encoder_args`: ví dụ `["-c:v", "h264_nvenc", "-b:v", "8M"]`.
    pub fn spawn(width: u32, height: u32, fps: u32, encoder_args: &[&str], out: &Path) -> Result<Self> {
        let ffmpeg = find_tool("ffmpeg")?;
        let stderr_path = out.with_extension("ffmpeg.log");
        let log = std::fs::File::create(&stderr_path)?;
        let size = format!("{width}x{height}");
        let rate = fps.to_string();
        let mut args: Vec<&str> = vec![
            "-hide_banner",
            "-y",
            "-loglevel",
            "info",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &size,
            "-r",
            &rate,
            "-i",
            "-",
            // Chuyển RGB → YUV đúng ma trận BT.709, dải TV — tránh video bị nhạt/lệch màu
            "-vf",
            "scale=out_color_matrix=bt709:out_range=tv,format=yuv420p",
        ];
        args.extend_from_slice(encoder_args);
        args.extend_from_slice(&[
            "-colorspace",
            "bt709",
            "-color_primaries",
            "bt709",
            "-color_trc",
            "bt709",
            "-color_range",
            "tv",
            // Một số encoder (openh264, MediaFoundation) bỏ qua cờ màu ở trên → ghi thẳng vào VUI của H.264
            "-bsf:v",
            "h264_metadata=colour_primaries=1:transfer_characteristics=1:matrix_coefficients=1:video_full_range_flag=0",
            "-movflags",
            "+faststart",
        ]);
        let out_s = out.to_string_lossy().to_string();
        let mut child = Command::new(&ffmpeg)
            .args(&args)
            .arg(&out_s)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .spawn()
            .with_context(|| format!("không chạy được {}", ffmpeg.display()))?;
        let stdin = child.stdin.take();
        Ok(Self {
            child,
            stdin,
            stderr_path,
        })
    }

    pub fn write_frame(&mut self, rgba: &[u8]) -> Result<()> {
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(rgba)
            .context("FFmpeg đã đóng stdin (xem file .ffmpeg.log)")
    }

    pub fn finish(mut self) -> Result<()> {
        drop(self.stdin.take());
        let status = self.child.wait()?;
        if !status.success() {
            bail!("FFmpeg thoát với mã {status} — xem {}", self.stderr_path.display());
        }
        Ok(())
    }
}

/// Chạy một lệnh FFmpeg/ffprobe ngắn và trả stdout.
pub fn run_tool(name: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(find_tool(name)?).args(args).output()?;
    if !out.status.success() {
        bail!("{name} lỗi: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
