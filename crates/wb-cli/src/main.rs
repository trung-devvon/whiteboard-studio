//! `wb` — CLI headless của Whiteboard Studio.
//!
//! ```text
//! wb render  <project.json> -o out.mp4        xuất video từ file dự án
//! wb quick   <ảnh>          -o out.mp4        xuất nhanh 1 ảnh (không cần viết JSON)
//! wb frame   <project.json> --at 2.5 -o f.png  xuất 1 khung hình tại giây 2.5
//! wb analyze <ảnh>          -o debug.png       phân tích ảnh, xem loại ảnh + thứ tự nét
//! ```
//!
//! `quick` và `analyze` nhận thêm `--pipeline auto|line_art|color|photo`,
//! `--style auto|sketch|sketch_then_color|color_brush`, `--background auto|keep|remove`, `--brush-size 0.025`.

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use serde::de::DeserializeOwned;
use std::io::Write;
use std::path::{Path, PathBuf};
use wb_compile::{ImageOptions, compile, debug::render_debug, save_png};
use wb_export::{Encoder, ExportOptions, export_mp4};
use wb_model::VisionParams;

#[derive(Parser)]
#[command(name = "wb", version, about = "Whiteboard Studio — công cụ dòng lệnh")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

/// Tuỳ chọn cách vẽ một ảnh (giống các trường của element ảnh trong dự án).
#[derive(Args, Clone)]
struct StyleArgs {
    /// Loại ảnh: auto | line_art | color | photo | vector (chỉ SVG)
    #[arg(long, default_value = "auto")]
    pipeline: String,
    /// Kiểu vẽ: auto | sketch | sketch_then_color | color_brush
    #[arg(long, default_value = "auto")]
    style: String,
    /// Nền đồng màu: auto | keep | remove
    #[arg(long, default_value = "auto")]
    background: String,
    /// Bán kính cọ tô (tỉ lệ cạnh dài ảnh, mặc định 0.025)
    #[arg(long)]
    brush_size: Option<f32>,
    /// Độ nhạy nhận nét (0..1)
    #[arg(long)]
    sensitivity: Option<f32>,
    /// Cạnh dài khi phân tích (px)
    #[arg(long)]
    working_size: Option<u32>,
}

/// Đọc một giá trị enum theo tên JSON (snake_case), vd "color_brush".
fn parse_enum<T: DeserializeOwned>(what: &str, s: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .with_context(|| format!("{what} không hợp lệ: {s}"))
}

impl StyleArgs {
    fn options(&self) -> Result<ImageOptions> {
        let mut params = VisionParams::default();
        if let Some(v) = self.brush_size {
            params.brush_size = v;
        }
        if let Some(v) = self.sensitivity {
            params.sensitivity = v;
        }
        if let Some(v) = self.working_size {
            params.working_size = v;
        }
        Ok(ImageOptions {
            pipeline: parse_enum("--pipeline", &self.pipeline)?,
            style: parse_enum("--style", &self.style)?,
            background: parse_enum("--background", &self.background)?,
            params,
        })
    }
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
    /// Xuất nhanh một ảnh thành video (tự tạo dự án 1 scene)
    Quick {
        image: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        /// Thư mục bàn tay (chứa hand.json). Mặc định: assets/hands/pen-brown nếu tìm thấy
        #[arg(long)]
        hand: Option<PathBuf>,
        #[arg(long)]
        no_hand: bool,
        /// Kiểu tay: wrist (bàn tay tới cổ tay, mặc định) | full (cả cánh tay, to)
        #[arg(long, default_value = "wrist")]
        hand_mode: String,
        /// Hệ số cỡ tay (1 = mặc định)
        #[arg(long, default_value_t = 1.0)]
        hand_scale: f32,
        /// Ép thời gian vẽ (giây)
        #[arg(long)]
        duration: Option<f64>,
        #[arg(long, default_value = "auto")]
        encoder: String,
        #[arg(long)]
        width: Option<u32>,
        #[command(flatten)]
        style: StyleArgs,
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
    /// Ghi gói biên dịch (Bundle, postcard) — đúng dữ liệu app gửi sang Player WASM. Dùng để soi/gỡ lỗi Player.
    Bundle {
        project: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        /// Gọn gói (ảnh tay, lớp mực) cho Player ở chiều rộng này
        #[arg(long, default_value_t = 960)]
        width: u32,
    },
    /// Xoá nền đồng màu (nối liền mép ảnh) → PNG trong suốt, cắt gọn về vật thể.
    /// Dùng để chuẩn bị ảnh bàn tay / cây bút / sticker.
    RemoveBg {
        image: PathBuf,
        #[arg(short, long)]
        out: PathBuf,
        /// Ngưỡng khác màu so với nền (RGB, 0–441). Nhỏ hơn → giữ chi tiết sáng sát mép vật thể
        #[arg(long, default_value_t = 42.0)]
        tolerance: f32,
        /// Không cắt gọn về hộp bao vật thể
        #[arg(long)]
        no_trim: bool,
    },
    /// Phân tích ảnh: loại ảnh, số nét, độ phủ, ảnh debug thứ tự nét
    Analyze {
        image: PathBuf,
        /// Ảnh debug (PNG). Nhiều lớp → thêm hậu tố -1, -2…
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Ghi DrawPlan (mảng, mỗi lớp một plan) ra JSON
        #[arg(long)]
        json: Option<PathBuf>,
        #[command(flatten)]
        style: StyleArgs,
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
            hand_mode,
            hand_scale,
            duration,
            encoder,
            width,
            style,
        } => {
            let hand_mode: wb_model::HandMode = parse_enum("--hand-mode", &hand_mode)?;
            let project = quick_project(
                &image,
                hand,
                no_hand,
                (hand_mode, hand_scale),
                duration,
                &style.options()?,
            )?;
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
        Cmd::Bundle { project, out, width } => {
            let doc = wb_compile::read_project(&project)?;
            let ctx = wb_compile::CompileContext::new(project.parent().unwrap_or(Path::new(".")));
            let mut p = wb_compile::prepare(&doc, &ctx, None)?;
            p.bundle.fit_for_preview(width);
            let bytes = p.bundle.to_bytes();
            std::fs::write(&out, &bytes).with_context(|| format!("không ghi được {}", out.display()))?;
            println!("Đã ghi {} ({:.1} MB)", out.display(), bytes.len() as f64 / 1e6);
            Ok(())
        }
        Cmd::RemoveBg {
            image,
            out,
            tolerance,
            no_trim,
        } => remove_bg(&image, &out, tolerance, !no_trim),
        Cmd::Analyze {
            image,
            out,
            json,
            style,
        } => {
            let a = wb_compile::analyze_file(&image, &style.options()?)?;
            println!(
                "Ảnh {}×{} · loại {:?} · kiểu vẽ {:?}{} · làm việc ở {:.0}% · {} ms",
                a.source.0,
                a.source.1,
                a.kind,
                a.style,
                if a.background_removed {
                    " · đã tách nền"
                } else {
                    ""
                },
                a.work_scale * 100.0,
                a.millis
            );
            let many = a.layers.len() > 1;
            for (i, l) in a.layers.iter().enumerate() {
                let st = l.plan.stats;
                println!(
                    "  Lớp {} ({}): {} nét · {} chấm vụn · nét {:.0}px · tô {:.0}px · nhấc bút {:.0}px · độ phủ {:.2}%",
                    i + 1,
                    l.name,
                    st.stroke_count,
                    st.dot_count,
                    st.outline_length,
                    st.fill_length,
                    st.travel_length,
                    l.coverage * 100.0
                );
                if let Some(out) = &out {
                    let path = if many { suffixed(out, i + 1) } else { out.clone() };
                    save_png(&render_debug(&l.ink, &l.plan, 1600), &path)?;
                    println!("    ảnh debug: {}", path.display());
                }
            }
            if let Some(j) = json {
                let plans: Vec<_> = a.layers.iter().map(|l| &l.plan).collect();
                std::fs::write(&j, serde_json::to_string(&plans)?)?;
                println!("DrawPlan: {}", j.display());
            }
            Ok(())
        }
    }
}

/// `debug.png` → `debug-2.png`
fn suffixed(p: &Path, i: usize) -> PathBuf {
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or("png".into());
    p.with_file_name(format!("{stem}-{i}.{ext}"))
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

fn print_report(report: &[wb_compile::ElementReport]) {
    for r in report {
        println!(
            "  [{}/{}] {}×{} · {:?} → {:?}{} · lớp: {} · {} nét + {} chấm · phủ {:.2}% · phân tích {} ms · vẽ {:.1}s",
            r.scene,
            r.id,
            r.source.0,
            r.source.1,
            r.kind,
            r.style,
            if r.background_removed { " (tách nền)" } else { "" },
            r.layers.join(" → "),
            r.strokes,
            r.dots,
            r.coverage * 100.0,
            r.analyze_ms,
            r.draw_ms / 1000.0
        );
    }
}

/// Tạo file dự án tạm cho lệnh `quick`.
fn quick_project(
    image: &Path,
    hand: Option<PathBuf>,
    no_hand: bool,
    (hand_mode, hand_scale): (wb_model::HandMode, f32),
    duration: Option<f64>,
    opt: &ImageOptions,
) -> Result<PathBuf> {
    let image = std::fs::canonicalize(image).with_context(|| format!("không thấy ảnh {}", image.display()))?;
    let hand = if no_hand {
        None
    } else {
        hand.map(|h| std::fs::canonicalize(h).unwrap_or_default()).or_else(|| {
            std::env::current_dir()
                .ok()?
                .ancestors()
                .map(|d| d.join("assets/hands/pen-brown"))
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
        "settings": { "hand": hand.as_deref().map(to_s), "hand_mode": hand_mode, "hand_scale": hand_scale },
        "scenes": [{
            "id": "s1",
            "elements": [{
                "id": "img",
                "kind": {
                    "type": "image",
                    "src": to_s(&image),
                    "pipeline": opt.pipeline,
                    "style": opt.style,
                    "background": opt.background,
                    "params": opt.params
                },
                "transform": { "x": 120, "y": 80, "w": 1680, "h": 920 },
                "timing": timing
            }]
        }]
    });
    let path = std::env::temp_dir().join(format!("wb-quick-{}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_string_pretty(&doc)?)?;
    Ok(path)
}

/// `wb remove-bg`: xoá nền đồng màu, tuỳ chọn cắt gọn, in độ dời để chỉnh toạ độ trong `hand.json`.
fn remove_bg(image: &Path, out: &Path, tolerance: f32, trim: bool) -> Result<()> {
    use wb_vision::background::{Border, analyze_border, remove_background_with};
    let img = image::open(image)
        .with_context(|| format!("không mở được {}", image.display()))?
        .to_rgba8();
    let cleared = match analyze_border(&img) {
        Border::Uniform(bg) => {
            println!("Nền: RGB({}, {}, {}) · ngưỡng {tolerance}", bg[0], bg[1], bg[2]);
            remove_background_with(&img, bg, tolerance)
        }
        Border::Transparent => {
            println!("Ảnh đã có nền trong suốt — chỉ cắt gọn");
            img
        }
        Border::Busy => bail!("mép ảnh nhiều màu — không tìm được nền đồng màu để xoá"),
    };
    let result = if trim {
        let (w, h) = cleared.dimensions();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for (x, y, p) in cleared.enumerate_pixels() {
            if p[3] > 8 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        if x1 < x0 {
            bail!("ảnh trống sau khi xoá nền — thử giảm --tolerance");
        }
        println!(
            "Cắt gọn: {}×{} → {}×{} (bớt trái {x0}px, trên {y0}px — trừ khỏi toạ độ đo trên ảnh gốc)",
            w,
            h,
            x1 - x0 + 1,
            y1 - y0 + 1
        );
        image::imageops::crop_imm(&cleared, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
    } else {
        cleared
    };
    result
        .save(out)
        .with_context(|| format!("không ghi được {}", out.display()))?;
    println!("Đã ghi {}", out.display());
    Ok(())
}
