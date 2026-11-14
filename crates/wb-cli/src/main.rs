//! `wb` — CLI headless của Whiteboard Studio.
//!
//! ```text
//! wb render  <project.json> -o out.mp4        xuất video từ file dự án
//! wb quick   <ảnh>          -o out.mp4        xuất nhanh 1 ảnh (không cần viết JSON)
//! wb frame   <project.json> --at 2.5 -o f.png  xuất 1 khung hình tại giây 2.5
//! wb analyze <ảnh>          -o debug.png       phân tích ảnh, xem thứ tự nét
//! ```

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::io::Write;
use std::path::{Path, PathBuf};
use wb_cli::{compile, debug::render_debug, save_png};
use wb_export::{Encoder, ExportOptions, export_mp4};
use wb_model::VisionParams;

#[derive(Parser)]
#[command(name = "wb", version, about = "Whiteboard Studio — công cụ dòng lệnh")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Xuất video MP4 từ file dự án
    Render {
        project: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        /// auto | nvenc | amf | qsv | openh264
        #[arg(long, default_value = "auto")]
        encoder: String,
        /// Ghi đè chiều rộng xuất (giữ tỉ lệ), vd 1280 để xem thử nhanh
        #[arg(long)]
        width: Option<u32>,
        /// Chỉ xuất từ giây … tới giây …
        #[arg(long)]
        from: Option<f64>,
        #[arg(long)]
        to: Option<f64>,
    },
    /// Xuất nhanh một ảnh line-art thành video (tự tạo dự án 1 scene)
    Quick {
        image: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        /// Thư mục bàn tay (chứa hand.json). Mặc định: assets/hands/placeholder-marker nếu tìm thấy
        #[arg(long)]
        hand: Option<PathBuf>,
        #[arg(long)]
        no_hand: bool,
        /// Ép thời gian vẽ (giây)
        #[arg(long)]
        duration: Option<f64>,
        #[arg(long, default_value = "auto")]
        encoder: String,
        #[arg(long)]
        width: Option<u32>,
    },
    /// Xuất một khung hình PNG tại thời điểm `--at` (giây)
    Frame {
        project: PathBuf,
        #[arg(long)]
        at: f64,
        #[arg(short, long)]
        out: PathBuf,
        #[arg(long)]
        width: Option<u32>,
    },
    /// Phân tích ảnh: số nét, độ phủ, ảnh debug thứ tự nét
    Analyze {
        image: PathBuf,
        /// Ảnh debug (PNG)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Ghi DrawPlan ra JSON
        #[arg(long)]
        json: Option<PathBuf>,
        #[arg(long)]
        sensitivity: Option<f32>,
        #[arg(long)]
        working_size: Option<u32>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Render {
            project,
            out,
            encoder,
            width,
            from,
            to,
        } => {
            let range = match (from, to) {
                (None, None) => None,
                (f, t) => Some((f.unwrap_or(0.0) * 1000.0, t.map(|t| t * 1000.0).unwrap_or(f64::MAX))),
            };
            render(&project, &out, &encoder, width, range)
        }
        Cmd::Quick {
            image,
            out,
            hand,
            no_hand,
            duration,
            encoder,
            width,
        } => {
            let project = quick_project(&image, hand, no_hand, duration)?;
            let r = render(&project, &out, &encoder, width, None);
            let _ = std::fs::remove_file(&project);
            r
        }
        Cmd::Frame {
            project,
            at,
            out,
            width,
        } => {
            let mut c = compile(&project, width)?;
            print_report(&c.report);
            save_png(c.engine.render(at * 1000.0), &out)?;
            println!("Đã ghi {}", out.display());
            Ok(())
        }
        Cmd::Analyze {
            image,
            out,
            json,
            sensitivity,
            working_size,
        } => {
            let img = wb_cli::load_image(&image)?;
            let mut params = VisionParams::default();
            if let Some(s) = sensitivity {
                params.sensitivity = s;
            }
            if let Some(w) = working_size {
                params.working_size = w;
            }
            let a = wb_cli::analyze_image(&img, &params);
            let st = a.plan.stats;
            println!(
                "Ảnh {}×{} · làm việc ở {:.0}% · {} ms",
                img.width(),
                img.height(),
                a.work_scale * 100.0,
                a.millis
            );
            println!(
                "  {} nét · {} chấm vụn · dài {:.0}px · nhấc bút {:.0}px · độ phủ {:.2}%",
                st.stroke_count,
                st.dot_count,
                st.outline_length,
                st.travel_length,
                a.coverage * 100.0
            );
            if let Some(out) = out {
                save_png(&render_debug(&a.ink, &a.plan, 1600), &out)?;
                println!("Ảnh debug: {}", out.display());
            }
            if let Some(j) = json {
                std::fs::write(&j, serde_json::to_string(&a.plan)?)?;
                println!("DrawPlan: {}", j.display());
            }
            Ok(())
        }
    }
}

fn render(project: &Path, out: &Path, encoder: &str, width: Option<u32>, range: Option<(f64, f64)>) -> Result<()> {
    let t0 = std::time::Instant::now();
    let mut c = compile(project, width)?;
    print_report(&c.report);
    let compile_s = t0.elapsed().as_secs_f64();
    let enc = Encoder::parse(encoder).with_context(|| format!("encoder không hợp lệ: {encoder}"))?;
    let total_ms = c.engine.duration_ms();
    let opts = ExportOptions {
        fps: c.project.settings.fps,
        encoder: enc,
        range_ms: range.map(|(a, b)| (a, b.min(total_ms))),
        ..ExportOptions::default()
    };
    let stats = export_mp4(&mut c.engine, out, &opts, |done, total| {
        if done % 15 == 0 || done == total {
            print!("\r  Đang xuất: {:>3}% ({done}/{total} khung)", done * 100 / total);
            let _ = std::io::stdout().flush();
        }
    })?;
    let video_s = stats.frames as f64 / opts.fps as f64;
    println!(
        "\nXong: {} · video {:.1}s · phân tích {:.1}s + xuất {:.1}s (render {:.1}s) · {:.1}× thời gian thực · encoder {}",
        out.display(),
        video_s,
        compile_s,
        stats.seconds,
        stats.render_seconds,
        video_s / (compile_s + stats.seconds),
        stats.encoder.ffmpeg_name()
    );
    Ok(())
}

fn print_report(report: &[wb_cli::ElementReport]) {
    for r in report {
        println!(
            "  [{}/{}] {}×{} · {} nét + {} chấm · phủ {:.2}% · phân tích {} ms · vẽ {:.1}s",
            r.scene,
            r.id,
            r.source.0,
            r.source.1,
            r.strokes,
            r.dots,
            r.coverage * 100.0,
            r.analyze_ms,
            r.draw_ms / 1000.0
        );
    }
}

/// Tạo file dự án tạm cho lệnh `quick`.
fn quick_project(image: &Path, hand: Option<PathBuf>, no_hand: bool, duration: Option<f64>) -> Result<PathBuf> {
    let image = std::fs::canonicalize(image).with_context(|| format!("không thấy ảnh {}", image.display()))?;
    let hand = if no_hand {
        None
    } else {
        hand.map(|h| std::fs::canonicalize(h).unwrap_or_default()).or_else(|| {
            std::env::current_dir()
                .ok()?
                .ancestors()
                .map(|d| d.join("assets/hands/placeholder-marker"))
                .find(|d| d.join("hand.json").is_file())
        })
    };
    let to_s = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let mut timing = serde_json::json!({});
    if let Some(d) = duration {
        timing["duration_ms"] = serde_json::json!((d * 1000.0) as u32);
    }
    let doc = serde_json::json!({
        "schema_version": 1,
        "settings": { "hand": hand.as_deref().map(to_s) },
        "scenes": [{
            "id": "s1",
            "elements": [{
                "id": "img",
                "kind": { "type": "image", "src": to_s(&image) },
                "transform": { "x": 120, "y": 80, "w": 1680, "h": 920 },
                "timing": timing
            }]
        }]
    });
    let path = std::env::temp_dir().join(format!("wb-quick-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_string_pretty(&doc)?)?;
    Ok(path)
}
