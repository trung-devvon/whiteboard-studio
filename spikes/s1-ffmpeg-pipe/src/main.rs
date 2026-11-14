//! SPIKE S1 — Rust → stdin FFmpeg (bản LGPL) → MP4, thử mọi encoder H.264 khả dụng.
//!
//! Câu hỏi: pipe RGBA từ Rust vào FFmpeg LGPL có chạy ổn không? Encoder nào dùng được trên máy này?
//! Tốc độ bao nhiêu? Màu có đúng (BT.709) không?
//! Đạt nếu: xuất 10s 1080p30 mở được, đủ 300 frame, màu các ô mẫu lệch ≤ 12/255.
//!
//! Chạy: `cargo run -p s1-ffmpeg-pipe --profile bench-fast`

use anyhow::Result;
use spike_common::{FfmpegWriter, out_dir, run_tool};
use std::sync::mpsc::sync_channel;
use std::time::Instant;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

const W: u32 = 1920;
const H: u32 = 1080;
const FPS: u32 = 30;
const SECONDS: u32 = 10;

/// Ô màu mẫu: (tên, RGB) — dùng để kiểm tra màu sau khi mã hoá/giải mã.
const PATCHES: [(&str, [u8; 3]); 6] = [
    ("đỏ", [255, 0, 0]),
    ("xanh lá", [0, 255, 0]),
    ("xanh dương", [0, 0, 255]),
    ("xám 50%", [128, 128, 128]),
    ("gần đen", [20, 20, 20]),
    ("cam thương hiệu", [230, 120, 30]),
];
const PATCH: f32 = 160.0;

fn patch_rect(i: usize) -> Rect {
    Rect::from_xywh(80.0 + i as f32 * (PATCH + 40.0), 60.0, PATCH, PATCH).unwrap()
}

/// Điểm trên đường xoắn ốc tại tham số u ∈ [0,1].
fn spiral(u: f32) -> (f32, f32) {
    let turns = 6.0;
    let a = u * turns * std::f32::consts::TAU;
    let r = 40.0 + u * 360.0;
    (W as f32 * 0.5 + r * a.cos(), H as f32 * 0.6 + r * a.sin() * 0.8)
}

/// Renderer có trạng thái giống engine thật: lớp "mực" tích lũy, mỗi frame chỉ vẽ thêm đoạn mới.
struct Renderer {
    ink: Pixmap,
    frame: Pixmap,
    last_u: f32,
}

impl Renderer {
    fn new() -> Self {
        let mut ink = Pixmap::new(W, H).unwrap();
        ink.fill(Color::WHITE);
        let mut p = Paint::default();
        for (i, (_, c)) in PATCHES.iter().enumerate() {
            p.set_color_rgba8(c[0], c[1], c[2], 255);
            ink.fill_rect(patch_rect(i), &p, Transform::identity(), None);
        }
        Self {
            frame: ink.clone(),
            ink,
            last_u: 0.0,
        }
    }

    fn render(&mut self, frame_idx: u32) -> &[u8] {
        let total = (FPS * SECONDS) as f32;
        let u = ((frame_idx + 1) as f32 / (total * 0.9)).min(1.0);
        // 1) đóng dấu đoạn nét mới vào lớp mực (tích lũy)
        if u > self.last_u {
            let mut pb = PathBuilder::new();
            let steps = 40;
            for k in 0..=steps {
                let uu = self.last_u + (u - self.last_u) * k as f32 / steps as f32;
                let (x, y) = spiral(uu);
                if k == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(25, 25, 30, 255);
                paint.anti_alias = true;
                let stroke = Stroke {
                    width: 7.0,
                    line_cap: tiny_skia::LineCap::Round,
                    ..Stroke::default()
                };
                self.ink
                    .stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
            self.last_u = u;
        }
        // 2) frame = mực + "bàn tay" giả (hình tròn ở mũi bút) — tay không được ghi vào lớp mực
        self.frame.data_mut().copy_from_slice(self.ink.data());
        let (x, y) = spiral(u);
        if let Some(c) = PathBuilder::from_circle(x + 30.0, y + 30.0, 36.0) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(240, 200, 170, 230);
            paint.anti_alias = true;
            self.frame
                .fill_path(&c, &paint, FillRule::Winding, Transform::identity(), None);
        }
        self.frame.data()
    }
}

struct Outcome {
    encoder: &'static str,
    ok: bool,
    detail: String,
}

fn try_encoder(name: &'static str, args: &[&str]) -> Outcome {
    let out = out_dir().join(format!("s1_{name}.mp4"));
    let t0 = Instant::now();
    let res: Result<(f64, f64)> = (|| {
        let mut ff = FfmpegWriter::spawn(W, H, FPS, args, &out)?;
        // Luồng render riêng → kênh có giới hạn 8 frame (backpressure) → luồng ghi
        let (tx, rx) = sync_channel::<Vec<u8>>(8);
        let render_thread = std::thread::spawn(move || {
            let mut r = Renderer::new();
            let mut busy = 0.0f64;
            for i in 0..FPS * SECONDS {
                let t = Instant::now();
                let buf = r.render(i).to_vec();
                busy += t.elapsed().as_secs_f64();
                if tx.send(buf).is_err() {
                    break;
                }
            }
            busy
        });
        for frame in rx {
            ff.write_frame(&frame)?;
        }
        let busy = render_thread.join().unwrap();
        ff.finish()?;
        Ok((busy, t0.elapsed().as_secs_f64()))
    })();
    match res {
        Err(e) => Outcome {
            encoder: name,
            ok: false,
            detail: first_line(&e.to_string()),
        },
        Ok((render_s, total_s)) => {
            let frames = (FPS * SECONDS) as f64;
            let verify = verify(&out).unwrap_or_else(|e| format!("KIỂM TRA LỖI: {e}"));
            let ok = !verify.contains("LỖI") && !verify.contains("SAI");
            Outcome {
                encoder: name,
                ok,
                detail: format!(
                    "tổng {total_s:.2}s ({:.0} fps, {:.1}× thời gian thực) · render-only {:.0} fps · {}",
                    frames / total_s,
                    SECONDS as f64 / total_s,
                    frames / render_s,
                    verify
                ),
            }
        }
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

/// Kiểm tra file: số frame, cờ màu, và màu các ô mẫu sau khi giải mã như một trình phát đúng chuẩn.
fn verify(path: &std::path::Path) -> Result<String> {
    let p = path.to_string_lossy();
    let probe = run_tool(
        "ffprobe",
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_packets",
            "-show_entries",
            "stream=nb_read_packets,pix_fmt,color_space,color_primaries,color_transfer",
            "-of",
            "csv=p=0",
            &p,
        ],
    )?;
    let probe = probe.trim().to_string();
    let frames_ok = probe.contains(&format!("{}", FPS * SECONDS));
    // Giải mã frame cuối về RGB với ma trận BT.709 dải TV
    let raw = std::process::Command::new(spike_common::find_tool("ffmpeg")?)
        .args([
            "-v",
            "error",
            "-sseof",
            "-0.2",
            "-i",
            &p,
            "-frames:v",
            "1",
            "-vf",
            "scale=in_color_matrix=bt709:in_range=tv,format=rgb24",
            "-f",
            "rawvideo",
            "-",
        ])
        .output()?
        .stdout;
    if raw.len() != (W * H * 3) as usize {
        anyhow::bail!("không đọc được frame cuối ({} byte)", raw.len());
    }
    let mut max_err = 0i32;
    let mut worst = "";
    for (i, (name, c)) in PATCHES.iter().enumerate() {
        let r = patch_rect(i);
        let (cx, cy) = ((r.x() + r.width() / 2.0) as usize, (r.y() + r.height() / 2.0) as usize);
        let o = (cy * W as usize + cx) * 3;
        for ch in 0..3 {
            let e = (raw[o + ch] as i32 - c[ch] as i32).abs();
            if e > max_err {
                max_err = e;
                worst = name;
            }
        }
    }
    let color_ok = max_err <= 12;
    Ok(format!(
        "ffprobe[{probe}] frame {} · màu lệch tối đa {max_err}/255 ({worst}) {}",
        if frames_ok { "ĐỦ" } else { "SAI SỐ FRAME" },
        if color_ok { "OK" } else { "SAI MÀU" }
    ))
}

fn main() -> Result<()> {
    println!(
        "SPIKE S1 — {}x{} {}fps {}s → {}",
        W,
        H,
        FPS,
        SECONDS,
        out_dir().display()
    );
    let candidates: Vec<(&'static str, Vec<&str>)> = vec![
        ("h264_nvenc", vec!["-c:v", "h264_nvenc", "-preset", "p5", "-b:v", "8M"]),
        ("h264_amf", vec!["-c:v", "h264_amf", "-b:v", "8M"]),
        ("h264_qsv", vec!["-c:v", "h264_qsv", "-b:v", "8M"]),
        ("h264_mf", vec!["-c:v", "h264_mf", "-b:v", "8M", "-hw_encoding", "1"]),
        ("h264_mf_sw", vec!["-c:v", "h264_mf", "-b:v", "8M"]),
        ("libopenh264", vec!["-c:v", "libopenh264", "-b:v", "8M"]),
    ];
    let mut results = Vec::new();
    for (name, args) in &candidates {
        print!("  • {name:<12} … ");
        use std::io::Write as _;
        std::io::stdout().flush().ok();
        let o = try_encoder(name, args);
        println!("{} {}", if o.ok { "ĐẠT " } else { "TRƯỢT" }, o.detail);
        results.push(o);
    }
    let report: String = results
        .iter()
        .map(|o| {
            format!(
                "| {} | {} | {} |\n",
                o.encoder,
                if o.ok { "✅" } else { "❌" },
                o.detail.replace('|', "/")
            )
        })
        .collect();
    std::fs::write(
        out_dir().join("s1-report.md"),
        format!("| Encoder | Kết quả | Chi tiết |\n|---|---|---|\n{report}"),
    )?;
    println!("Báo cáo: {}", out_dir().join("s1-report.md").display());
    Ok(())
}
