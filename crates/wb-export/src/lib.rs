//! Xuất video: render từng khung bằng `wb-engine`, đẩy RGBA qua stdin vào FFmpeg (tiến trình riêng, bản LGPL).
//! ADR-0007: thứ tự encoder nvenc → amf → qsv → openh264; luôn ghi thông tin màu BT.709.

mod ffmpeg;

pub use ffmpeg::{Encoder, FfmpegError, find_ffmpeg, probe_encoder};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::time::Instant;
use wb_engine::Engine;

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Ffmpeg(#[from] FfmpegError),
    #[error("đã huỷ xuất video")]
    Cancelled,
    #[error("lỗi ghi file: {0}")]
    Io(#[from] std::io::Error),
}

pub struct ExportOptions {
    pub fps: u32,
    pub encoder: Encoder,
    /// Bitrate video (bit/giây).
    pub bitrate: u32,
    /// Chỉ xuất khoảng [from, to) ms (để xem thử nhanh). `None` = toàn bộ.
    pub range_ms: Option<(f64, f64)>,
    pub ffmpeg: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            fps: 30,
            encoder: Encoder::Auto,
            bitrate: 8_000_000,
            range_ms: None,
            ffmpeg: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportStats {
    pub frames: u64,
    pub encoder: Encoder,
    pub seconds: f64,
    /// Thời gian chỉ dành cho render khung (không tính chờ FFmpeg).
    pub render_seconds: f64,
    pub log_path: PathBuf,
}

/// Xuất toàn bộ (hoặc một khoảng) dự án ra MP4. `progress(khung_đã_xong, tổng_khung)`.
pub fn export_mp4(
    engine: &mut Engine,
    out: &Path,
    opts: &ExportOptions,
    mut progress: impl FnMut(u64, u64),
) -> Result<ExportStats, ExportError> {
    let t_start = Instant::now();
    let ffmpeg = match &opts.ffmpeg {
        Some(p) => p.clone(),
        None => find_ffmpeg().ok_or(FfmpegError::NotFound)?,
    };
    let encoder = match opts.encoder {
        Encoder::Auto => probe_encoder(&ffmpeg)?,
        e => e,
    };
    let (from, to) = opts.range_ms.unwrap_or((0.0, engine.duration_ms()));
    let total = (((to - from) / 1000.0) * opts.fps as f64).ceil().max(1.0) as u64;
    let (w, h) = {
        let f = engine.render(from);
        (f.width(), f.height())
    };

    let log_path = out.with_extension("ffmpeg.log");
    let mut child = ffmpeg::spawn_encoder(&ffmpeg, w, h, opts.fps, encoder, opts.bitrate, out, &log_path)?;
    let mut stdin = child.stdin.take().expect("stdin");

    // Luồng ghi riêng + kênh giới hạn 6 khung (backpressure: RAM không phình khi FFmpeg chậm)
    let (tx, rx) = sync_channel::<Vec<u8>>(6);
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        use std::io::Write;
        for frame in rx {
            stdin.write_all(&frame)?;
        }
        Ok(())
    });

    let mut render_s = 0.0;
    let mut result: Result<(), ExportError> = Ok(());
    for i in 0..total {
        if opts.cancel.load(Ordering::Relaxed) {
            result = Err(ExportError::Cancelled);
            break;
        }
        let t = from + i as f64 * 1000.0 / opts.fps as f64;
        let r0 = Instant::now();
        let data = engine.render(t).data().to_vec();
        render_s += r0.elapsed().as_secs_f64();
        if tx.send(data).is_err() {
            break; // luồng ghi đã dừng (FFmpeg lỗi) — xem trạng thái bên dưới
        }
        progress(i + 1, total);
    }
    drop(tx);
    let write_res = writer.join().expect("luồng ghi panic");
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_file(out);
        return Err(ExportError::Cancelled);
    }
    let status = child.wait()?;
    if !status.success() || write_res.is_err() {
        return Err(FfmpegError::Failed { log: log_path }.into());
    }
    Ok(ExportStats {
        frames: total,
        encoder,
        seconds: t_start.elapsed().as_secs_f64(),
        render_seconds: render_s,
        log_path,
    })
}
